use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use crate::{
    engine::{
        Map, Object, World, persistence,
        protocol::{Command, CommandStatus, Interaction},
        resources::Package,
        session::{Dispatch, InteractionDelivery, Session},
    },
    map::{MapTiles, load_map},
    terrain_scene::{TerrainScene, TerrainScript},
    value::Value,
    wml::{self, Node},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogLine {
    pub speaker: String,
    pub text: String,
}

pub struct Game {
    pub id: String,
    pub name: String,
    pub start_dialog: String,
    engine: Session,
    map_tiles: MapTiles,
    terrain_scripts: Vec<TerrainScript>,
    scene_assets: BTreeMap<String, String>,
    dialogs: BTreeMap<String, Vec<DialogLine>>,
    pending_dialog: Option<String>,
    next_command_id: u64,
    next_query_id: u64,
    query_view_revision: u64,
    latest_view: Option<crate::engine::protocol::ViewUpdate>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameSnapshot {
    pub map: Map,
    pub objects: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CampaignState {
    pub units: Vec<Value>,
    pub gold: i64,
    pub variables: BTreeMap<String, Value>,
}

impl Game {
    pub fn load(scripts: impl AsRef<Path>, scenario_path: &str) -> Result<Self, String> {
        Self::load_with_campaign(scripts, scenario_path, None)
    }

    pub fn load_with_campaign(
        scripts: impl AsRef<Path>,
        scenario_path: &str,
        campaign: Option<&CampaignState>,
    ) -> Result<Self, String> {
        let scripts = fs::canonicalize(scripts.as_ref())
            .map_err(|error| format!("cannot open scripts directory: {error}"))?;
        Self::load_with_campaign_from(scenario_path, campaign, &|path| {
            let requested = Path::new(path);
            if requested.is_absolute() {
                return Err(format!("resource path must be relative: {path}"));
            }
            let full = fs::canonicalize(scripts.join(requested))
                .map_err(|error| format!("cannot read {path}: {error}"))?;
            if !full.starts_with(&scripts) {
                return Err(format!("resource path escapes scripts directory: {path}"));
            }
            fs::read_to_string(full).map_err(|error| format!("cannot read {path}: {error}"))
        })
    }

    pub fn load_from(
        scenario_path: &str,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::load_with_campaign_from(scenario_path, None, read)
    }

    pub fn load_with_campaign_from(
        scenario_path: &str,
        campaign: Option<&CampaignState>,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::build_from(scenario_path, campaign, read, None)
    }

    fn build_from(
        scenario_path: &str,
        campaign: Option<&CampaignState>,
        read: &impl Fn(&str) -> Result<String, String>,
        saved: Option<&[u8]>,
    ) -> Result<Self, String> {
        let package = Package::open(read)?;
        let package_identity = package.manifest().identity();
        let mut package_assets: BTreeSet<_> = package.manifest().assets.keys().cloned().collect();
        let entry = package.manifest().entry.clone();
        let module_sources = package.module_sources()?;
        let read = &|path: &str| package.read_text(path);
        let scenario_document = read_wml_from(scenario_path, read)?;
        let scenario = one_root(&scenario_document, "scenario")?;
        let resources = scenario.child("resources")?;

        let map_document = read_wml_from(resources.attribute("map")?, read)?;
        let map_node = one_root(&map_document, "map")?;
        let map = load_map(map_node)?;
        let (map_tiles, terrain_scripts) =
            crate::map::load_resources(&map, resources.attribute("map_objects")?, read)?;
        let terrain_scene = TerrainScene::from_lua(&map, &terrain_scripts)?;
        package_assets.extend(terrain_scene.assets().keys().cloned());

        let mut types = BTreeMap::new();
        let mut races = BTreeMap::new();
        let mut traits = BTreeMap::new();
        for path in split_paths(resources.attribute("unit_types")?) {
            let roots = read_wml_from(path, read)?;
            for root in &roots {
                let id = root.attribute("id")?.to_owned();
                match root.name.as_str() {
                    "unit_type" => {
                        if types.insert(id.clone(), node_properties(root)).is_some() {
                            return Err(format!("duplicate unit type id: {id}"));
                        }
                    }
                    "race" => {
                        if races.insert(id.clone(), node_properties(root)).is_some() {
                            return Err(format!("duplicate race id: {id}"));
                        }
                    }
                    "trait" => {
                        if traits.insert(id.clone(), node_properties(root)).is_some() {
                            return Err(format!("duplicate global trait id: {id}"));
                        }
                    }
                    other => {
                        return Err(format!(
                            "unsupported [{other}] in unit type resource: {path}"
                        ));
                    }
                }
            }
        }

        let mut objects = BTreeMap::new();
        for unit in scenario.children_named("unit") {
            let id = unit.attribute("id")?.to_owned();
            if objects.contains_key(&id) {
                return Err(format!("duplicate object id: {id}"));
            }
            let type_id = unit.attribute("type")?;
            let carried = campaign.and_then(|state| {
                state.units.iter().find_map(|unit| {
                    (unit.get("id").and_then(Value::as_str) == Some(&id))
                        .then(|| unit.as_map().cloned())
                        .flatten()
                })
            });
            let mut properties = carried
                .clone()
                .unwrap_or_else(|| types.get(type_id).cloned().unwrap_or_default());
            if properties.is_empty() {
                return Err(format!("unknown unit type: {type_id}"));
            }
            properties.remove("id");
            let scenario_properties = types
                .get(type_id)
                .cloned()
                .ok_or_else(|| format!("unknown unit type: {type_id}"))?;
            for (key, value) in scenario_properties {
                properties.entry(key).or_insert(value);
            }
            let mut scenario_properties = node_properties(unit);
            if carried.is_some() {
                scenario_properties.remove("type");
            }
            properties.extend(scenario_properties);
            objects.insert(id.clone(), Object { id, properties });
        }

        let dialogs = load_dialogs(&read_wml_from(resources.attribute("dialogs")?, read)?)?;
        for path in split_paths(resources.attribute("rules")?) {
            let module = path
                .strip_suffix(".lua")
                .ok_or_else(|| format!("rule module must end with .lua: {path}"))?
                .replace('/', ".");
            package.module(&module)?;
        }
        let seed = parse_i64(scenario, "random_seed")? as u64;

        let unit_types = Value::Map(
            types
                .iter()
                .map(|(id, properties)| (id.clone(), Value::Map(properties.clone())))
                .collect(),
        );
        let races = Value::Map(
            races
                .into_iter()
                .map(|(id, properties)| (id, Value::Map(properties)))
                .collect(),
        );
        let traits = Value::Map(
            traits
                .into_iter()
                .map(|(id, properties)| (id, Value::Map(properties)))
                .collect(),
        );
        let recall = campaign.map_or_else(Vec::new, |state| {
            state
                .units
                .iter()
                .filter(|unit| {
                    let id = unit.get("id").and_then(Value::as_str);
                    id.is_some_and(|id| !objects.contains_key(id))
                })
                .cloned()
                .collect()
        });
        let villages = map
            .cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| crate::terrain::gameplay_type(cell) == "village")
            .map(|(index, _)| {
                Value::Map(BTreeMap::from([
                    ("x".into(), Value::Integer((index % map.width + 1) as i64)),
                    ("y".into(), Value::Integer((index / map.width + 1) as i64)),
                ]))
            })
            .collect();
        let mut initial_state = BTreeMap::from([
            ("scenario".into(), Value::Map(node_properties(scenario))),
            ("unit_types".into(), unit_types),
            ("races".into(), races),
            ("traits".into(), traits),
            ("recall".into(), Value::List(recall)),
            ("villages".into(), Value::List(villages)),
            (
                "pending_dialog".into(),
                Value::String(scenario.attribute("on_start_dialog")?.into()),
            ),
            (
                "session:scenario_path".into(),
                Value::String(scenario_path.into()),
            ),
            ("presentation:scene".into(), terrain_scene.presentation()),
        ]);
        if let Some(campaign) = campaign {
            initial_state.extend(campaign.variables.clone());
        }
        let session_id = format!("{}:{scenario_path}", package_identity.package_id);
        let world = World {
            map,
            objects,
            state: initial_state,
        };
        let request = match (campaign, scenario.attributes.get("campaign_side")) {
            (Some(campaign), Some(side)) => Value::Map(BTreeMap::from([
                ("campaign_side".into(), Value::String(side.clone())),
                ("campaign_gold".into(), Value::Integer(campaign.gold)),
            ])),
            _ => Value::Nil,
        };
        let engine = if let Some(bytes) = saved {
            Session::restore_entry(
                session_id,
                package_identity,
                package_assets,
                world,
                seed,
                module_sources,
                &entry,
                bytes,
            )?
        } else {
            Session::open(
                session_id,
                package_identity,
                package_assets,
                world,
                seed,
                module_sources,
                &entry,
                request,
            )?
        };

        Ok(Self {
            id: scenario.attribute("id")?.into(),
            name: scenario.attribute("name")?.into(),
            start_dialog: scenario.attribute("on_start_dialog")?.into(),
            engine,
            map_tiles,
            terrain_scripts,
            scene_assets: terrain_scene.assets().clone(),
            dialogs,
            pending_dialog: Some(scenario.attribute("on_start_dialog")?.into()),
            next_command_id: 1,
            next_query_id: 1,
            query_view_revision: 0,
            latest_view: None,
        })
    }

    pub fn save(&self) -> Result<String, String> {
        String::from_utf8(self.engine.save().map_err(|error| error.message)?)
            .map_err(|error| format!("save is not UTF-8: {error}"))
    }

    pub fn load_save(scripts: impl AsRef<Path>, source: &str) -> Result<Self, String> {
        let scripts = fs::canonicalize(scripts.as_ref())
            .map_err(|error| format!("cannot open scripts directory: {error}"))?;
        Self::load_save_from(source, &|path| {
            let requested = Path::new(path);
            if requested.is_absolute() {
                return Err(format!("resource path must be relative: {path}"));
            }
            let full = fs::canonicalize(scripts.join(requested))
                .map_err(|error| format!("cannot read {path}: {error}"))?;
            if !full.starts_with(&scripts) {
                return Err(format!("resource path escapes scripts directory: {path}"));
            }
            fs::read_to_string(full).map_err(|error| format!("cannot read {path}: {error}"))
        })
    }

    pub fn load_save_from(
        source: &str,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let package = Package::open(read)?;
        let snapshot = persistence::decode(source.as_bytes(), &package.manifest().identity())
            .map_err(|error| error.message)?;
        let scenario_path = snapshot
            .world
            .state
            .get("session:scenario_path")
            .and_then(Value::as_str)
            .ok_or_else(|| "save has no scenario path".to_owned())?
            .to_owned();
        let pending_dialog = snapshot
            .world
            .state
            .get("pending_dialog")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mut game = Self::build_from(&scenario_path, None, read, Some(source.as_bytes()))
            .map_err(|error| format!("invalid save state: {error}"))?;
        game.pending_dialog = pending_dialog;
        game.validate_restored_state()?;
        Ok(game)
    }

    pub fn campaign_state(&self) -> Result<CampaignState, String> {
        let scenario = self
            .engine
            .world()
            .state
            .get("scenario")
            .and_then(Value::as_map)
            .ok_or_else(|| "game state has no valid scenario".to_owned())?;
        let side = scenario
            .get("campaign_side")
            .and_then(Value::as_str)
            .ok_or_else(|| "scenario has no campaign_side".to_owned())?;
        let units = self
            .engine
            .world()
            .objects
            .values()
            .filter(|object| object.properties.get("side").and_then(Value::as_str) == Some(side))
            .map(|object| {
                let mut values = object.properties.clone();
                values.insert("id".into(), Value::String(object.id.clone()));
                Value::Map(values)
            })
            .chain(match self.engine.world().state.get("recall") {
                Some(Value::List(units)) => units.clone(),
                _ => Vec::new(),
            })
            .collect();
        let percentage = scenario
            .get("carryover_percentage")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let gold = self
            .engine
            .world()
            .state
            .get(&format!("gold:{side}"))
            .and_then(Value::as_i64)
            .unwrap_or(0)
            * percentage
            / 100;
        let variables = self
            .engine
            .world()
            .state
            .iter()
            .filter(|(key, _)| key.starts_with("campaign:"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Ok(CampaignState {
            units,
            gold,
            variables,
        })
    }

    pub fn next_scenario(&self) -> Option<&str> {
        self.engine
            .world()
            .state
            .get("scenario")?
            .get("next_scenario")?
            .as_str()
    }

    pub fn dialog(&self, id: &str) -> Result<&[DialogLine], String> {
        self.dialogs
            .get(id)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("unknown dialog: {id}"))
    }

    pub fn start_events(&self) -> Result<Vec<Value>, String> {
        match &self.pending_dialog {
            Some(dialog) => Ok(vec![self.dialog_event(dialog)?]),
            None => Ok(Vec::new()),
        }
    }

    pub fn snapshot(&mut self) -> Result<GameSnapshot, String> {
        let objects = self.query("snapshot", Value::Nil)?;
        Ok(GameSnapshot {
            map: self.engine.world().map.clone(),
            objects,
        })
    }

    pub fn view_snapshot(&mut self) -> Result<crate::engine::protocol::ViewSnapshot, String> {
        self.engine.snapshot("local")
    }

    pub fn take_view_update(&mut self) -> Option<crate::engine::protocol::ViewUpdate> {
        self.latest_view.take()
    }

    pub fn dispatch_command(&mut self, command: Command) -> Dispatch {
        let dismissing_dialog = command.action == "dismiss_dialog";
        let mut dispatch = self.engine.dispatch("local", command);
        if dispatch.result.status != CommandStatus::Committed {
            return dispatch;
        }
        if dismissing_dialog {
            self.pending_dialog = None;
        }
        let mut events = match dispatch.events.take().unwrap_or(Value::List(Vec::new())) {
            Value::List(events) => events,
            event => vec![event],
        };
        if let Some(dialog) = events.iter().find_map(|event| {
            event
                .get("dialog")
                .and_then(Value::as_str)
                .map(str::to_owned)
        }) {
            self.pending_dialog = Some(dialog.clone());
            if let Ok(event) = self.dialog_event(&dialog) {
                events.push(event);
            }
        }
        dispatch.events = Some(Value::List(events));
        dispatch
    }

    pub fn interact(&mut self, interaction: Interaction) -> InteractionDelivery {
        self.engine.interact("local", interaction)
    }

    pub fn map(&self) -> &Map {
        &self.engine.world().map
    }

    pub fn map_tiles(&self) -> &MapTiles {
        &self.map_tiles
    }

    pub fn terrain_scripts(&self) -> &[TerrainScript] {
        &self.terrain_scripts
    }

    pub fn scene_assets(&self) -> &BTreeMap<String, String> {
        &self.scene_assets
    }

    pub fn query(&mut self, action: &str, request: Value) -> Result<Value, String> {
        let query_id = self.next_query_id;
        self.next_query_id = self
            .next_query_id
            .checked_add(1)
            .ok_or_else(|| "query id exhausted".to_owned())?;
        let interaction_id = format!("query:{query_id}");
        let delivery = self.engine.interact(
            "query",
            Interaction {
                interaction_id: interaction_id.clone(),
                expected_view_revision: self.query_view_revision,
                kind: crate::engine::protocol::InteractionKind::Activate,
                target: Value::Nil,
                payload: Value::Map(BTreeMap::from([
                    ("query".into(), Value::String(action.into())),
                    ("payload".into(), request),
                ])),
            },
        );
        if let Some(error) = delivery.error {
            return Err(error.message);
        }
        if delivery.command.is_some() {
            return Err("query interaction unexpectedly produced a command".into());
        }
        let update = delivery
            .view
            .ok_or_else(|| "query interaction did not update the view".to_owned())?;
        self.query_view_revision = update.view_revision;
        let content = update
            .replace_blocks
            .into_iter()
            .find(|block| block.id == "query")
            .ok_or_else(|| "query interaction did not produce a query view block".to_owned())?
            .content;
        if content.get("interaction_id").and_then(Value::as_str) != Some(&interaction_id) {
            return Err("query view block has a mismatched interaction id".into());
        }
        content
            .get("value")
            .cloned()
            .ok_or_else(|| "query view block has no value".to_owned())
    }

    pub fn revision(&self) -> u64 {
        self.engine.world_revision()
    }

    pub fn acknowledge_dialog(&mut self) -> Result<(), String> {
        if self.pending_dialog.is_none() {
            return Err("no dialog is waiting for the UI".into());
        }
        let command_id = self.next_command_id;
        self.next_command_id = self.next_command_id.saturating_add(1);
        let dispatch = self.dispatch_command(Command {
            command_id: format!("game:{command_id}"),
            expected_world_revision: self.engine.world_revision(),
            action: "dismiss_dialog".into(),
            payload: Value::Nil,
        });
        if let Some(error) = dispatch.result.error {
            return Err(error.message);
        }
        self.latest_view = dispatch.view;
        Ok(())
    }

    pub fn execute(&mut self, function: &str, command: Value) -> Result<Vec<Value>, String> {
        let command_id = self.next_command_id;
        self.next_command_id = self
            .next_command_id
            .checked_add(1)
            .ok_or_else(|| "command id exhausted".to_owned())?;
        let dispatch = self.dispatch_command(Command {
            command_id: format!("game:{command_id}"),
            expected_world_revision: self.engine.world_revision(),
            action: function.into(),
            payload: command,
        });
        if dispatch.result.status == CommandStatus::Rejected {
            return Err(dispatch
                .result
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "command rejected".into()));
        }
        self.latest_view = dispatch.view;
        let result = dispatch.events.unwrap_or(Value::List(Vec::new()));
        let events = match result {
            Value::List(events) => events,
            event => vec![event],
        };
        Ok(events)
    }

    fn dialog_event(&self, dialog: &str) -> Result<Value, String> {
        let lines = self
            .dialog(dialog)?
            .iter()
            .map(|line| {
                Value::Map(BTreeMap::from([
                    ("speaker".into(), Value::String(line.speaker.clone())),
                    ("text".into(), Value::String(line.text.clone())),
                ]))
            })
            .collect();
        Ok(Value::Map(BTreeMap::from([
            ("type".into(), Value::String("dialog_requested".into())),
            ("dialog".into(), Value::String(dialog.into())),
            ("lines".into(), Value::List(lines)),
        ])))
    }

    fn validate_restored_state(&self) -> Result<(), String> {
        self.start_events()
            .map_err(|error| format!("invalid save dialog: {error}"))?;
        Ok(())
    }
}

fn read_wml_from(
    path: &str,
    read: &impl Fn(&str) -> Result<String, String>,
) -> Result<Vec<Node>, String> {
    wml::parse(&read(path)?).map_err(|error| format!("{path}: {error}"))
}

fn one_root<'a>(nodes: &'a [Node], name: &str) -> Result<&'a Node, String> {
    let mut matching = nodes.iter().filter(|node| node.name == name);
    let node = matching.next().ok_or_else(|| format!("missing [{name}]"))?;
    if matching.next().is_some() {
        return Err(format!("expected one [{name}]"));
    }
    Ok(node)
}

fn split_paths(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_i64(node: &Node, name: &str) -> Result<i64, String> {
    node.attribute(name)?
        .parse()
        .map_err(|_| format!("[{}].{name} must be an integer", node.name))
}
fn node_properties(node: &Node) -> BTreeMap<String, Value> {
    let mut values: BTreeMap<String, Value> = node
        .attributes
        .iter()
        .map(|(key, value)| {
            let value = value
                .parse::<i64>()
                .map(Value::Integer)
                .unwrap_or_else(|_| Value::String(value.clone()));
            (key.clone(), value)
        })
        .collect();
    let mut children: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for child in &node.children {
        children
            .entry(child.name.clone())
            .or_default()
            .push(Value::Map(node_properties(child)));
    }
    if !children.is_empty() {
        values.insert(
            "__children".into(),
            Value::Map(
                children
                    .into_iter()
                    .map(|(name, nodes)| (name, Value::List(nodes)))
                    .collect(),
            ),
        );
    }
    values
}

fn load_dialogs(nodes: &[Node]) -> Result<BTreeMap<String, Vec<DialogLine>>, String> {
    let mut dialogs = BTreeMap::new();
    for dialog in nodes.iter().filter(|node| node.name == "dialog") {
        let id = dialog.attribute("id")?.to_owned();
        let lines = dialog
            .children_named("line")
            .map(|line| {
                Ok(DialogLine {
                    speaker: line.attribute("speaker")?.into(),
                    text: line.attribute("text")?.into(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if lines.is_empty() {
            return Err(format!("dialog {id} has no lines"));
        }
        if dialogs.insert(id.clone(), lines).is_some() {
            return Err(format!("duplicate dialog id: {id}"));
        }
    }
    Ok(dialogs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scripts() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts")
    }

    #[test]
    fn successful_commands_advance_the_ui_revision() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        game.acknowledge_dialog().unwrap();
        let before = game.revision();
        game.execute("end_turn", Value::Nil).unwrap();
        assert!(game.revision() > before);
    }

    #[test]
    fn package_builds_generic_view_blocks() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let view = game.view_snapshot().unwrap();
        assert_eq!(
            view.blocks
                .iter()
                .map(|block| block.id.as_str())
                .collect::<Vec<_>>(),
            ["hud", "objects", "scene", "status"]
        );
        let hud = view.blocks.iter().find(|block| block.id == "hud").unwrap();
        let root = crate::engine::protocol::ui_block(&hud.content).unwrap();
        assert_eq!(root.children[0].action.as_ref().unwrap().action, "end_turn");
        let scene = view
            .blocks
            .iter()
            .find(|block| block.id == "scene")
            .unwrap();
        let items = crate::engine::protocol::scene_block(&scene.content).unwrap();
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| {
            matches!(item.layer.as_str(), "ground" | "world")
                && item.frames.first() == Some(&item.asset)
        }));
    }

    #[test]
    fn transfers_arbitrary_wml_properties_without_a_schema() {
        let nodes = wml::parse(
            "[thing]\nanswer=42\nlabel=custom\n[extra]\nflag=yes\n[/extra]\n[extra]\nflag=no\n[/extra]\n[/thing]",
        )
        .unwrap();
        let properties = node_properties(&nodes[0]);
        assert_eq!(properties["answer"], Value::Integer(42));
        assert_eq!(properties["label"], Value::String("custom".into()));
        let children = properties["__children"].as_map().unwrap();
        assert!(matches!(&children["extra"], Value::List(values) if values.len() == 2));
    }

    #[test]
    fn filesystem_loader_rejects_paths_outside_scripts() {
        let error = Game::load(scripts(), "../Cargo.toml").err().unwrap();
        assert!(error.contains("invalid package path"));
    }

    #[test]
    fn load_save_rejects_invalid_restored_state() {
        let game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let mut save: serde_json::Value = serde_json::from_str(&game.save().unwrap()).unwrap();
        save["store"]["world"]["state"]
            .as_object_mut()
            .unwrap()
            .remove("scenario");
        let source = serde_json::to_string(&save).unwrap();
        let error = Game::load_save(scripts(), &source).err().unwrap();
        assert!(error.contains("invalid save state"));
    }
}
