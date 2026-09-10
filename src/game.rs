use std::{collections::BTreeMap, fs, path::Path};

use crate::{
    engine::{Engine, Map, Object, World},
    terrain_scene::TerrainScript,
    value::Value,
    wml::{self, Node},
};
use serde::{Deserialize, Serialize};

const SAVE_VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
struct SavedGame {
    version: u32,
    scenario_path: String,
    objects: BTreeMap<String, Object>,
    state: BTreeMap<String, Value>,
    random_state: u64,
    pending_dialog: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogLine {
    pub speaker: String,
    pub text: String,
}

pub struct Game {
    pub id: String,
    pub name: String,
    pub start_dialog: String,
    engine: Engine,
    map_tiles: MapTiles,
    terrain_scripts: Vec<TerrainScript>,
    dialogs: BTreeMap<String, Vec<DialogLine>>,
    pending_dialog: Option<String>,
    scenario_path: String,
    revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapTile {
    pub color: [u8; 3],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapTiles(BTreeMap<String, MapTile>);

impl MapTiles {
    pub fn get(&self, terrain_code: &str) -> Option<&MapTile> {
        self.0.get(crate::terrain::gameplay_type(terrain_code))
    }
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
        let scenario_document = read_wml_from(scenario_path, read)?;
        let scenario = one_root(&scenario_document, "scenario")?;
        let resources = scenario.child("resources")?;

        let map_document = read_wml_from(resources.attribute("map")?, read)?;
        let map_node = one_root(&map_document, "map")?;
        let map = load_map(map_node)?;
        let mut map_tiles = MapTiles::default();
        let mut terrain_scripts = Vec::new();
        for path in split_paths(resources.attribute("map_objects")?) {
            let roots = read_wml_from(path, read)?;
            let map_object = one_root(&roots, "map_object")?;
            let id = map_object.attribute("id")?.to_owned();
            let color = map_object
                .attributes
                .get("color")
                .map(|value| parse_color(value))
                .transpose()?
                .unwrap_or([96, 96, 96]);
            if map_tiles.0.insert(id.clone(), MapTile { color }).is_some() {
                return Err(format!("duplicate map object id: {id}"));
            }
            if let Some(renderer) = map_object.attributes.get("renderer") {
                terrain_scripts.push(TerrainScript {
                    family: id,
                    codes: split_paths(map_object.attribute("codes")?)
                        .map(str::to_owned)
                        .collect(),
                    source: read(renderer).map_err(|error| {
                        format!("cannot read terrain renderer {renderer}: {error}")
                    })?,
                });
            }
        }
        for cell in &map.cells {
            if map_tiles.get(cell).is_none() {
                return Err(format!("map uses unknown map object: {cell}"));
            }
        }

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
        let rule_source = split_paths(resources.attribute("rules")?)
            .map(read)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
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
        ]);
        if let Some(campaign) = campaign {
            initial_state.extend(campaign.variables.clone());
        }
        let mut engine = Engine::new(
            World {
                map,
                objects,
                state: initial_state,
            },
            seed,
            rule_source,
        )?;
        engine.execute("initialize", Value::Nil)?;
        if let (Some(campaign), Some(side)) = (campaign, scenario.attributes.get("campaign_side")) {
            let key = format!("gold:{side}");
            let starting = engine
                .world
                .state
                .get(&key)
                .and_then(Value::as_i64)
                .unwrap_or(0);
            engine
                .world
                .state
                .insert(key, Value::Integer(starting.max(campaign.gold)));
        }

        Ok(Self {
            id: scenario.attribute("id")?.into(),
            name: scenario.attribute("name")?.into(),
            start_dialog: scenario.attribute("on_start_dialog")?.into(),
            engine,
            map_tiles,
            terrain_scripts,
            dialogs,
            pending_dialog: Some(scenario.attribute("on_start_dialog")?.into()),
            scenario_path: scenario_path.into(),
            revision: 0,
        })
    }

    pub fn save(&self) -> Result<String, String> {
        let (objects, state, random_state) = self.engine.saved_state();
        serde_json::to_string_pretty(&SavedGame {
            version: SAVE_VERSION,
            scenario_path: self.scenario_path.clone(),
            objects,
            state,
            random_state,
            pending_dialog: self.pending_dialog.clone(),
        })
        .map_err(|error| format!("cannot encode save: {error}"))
    }

    pub fn load_save(scripts: impl AsRef<Path>, source: &str) -> Result<Self, String> {
        let saved: SavedGame =
            serde_json::from_str(source).map_err(|error| format!("cannot decode save: {error}"))?;
        if saved.version != SAVE_VERSION {
            return Err(format!(
                "unsupported save version: {} (expected {SAVE_VERSION})",
                saved.version
            ));
        }
        let mut game = Self::load(scripts, &saved.scenario_path)?;
        game.engine
            .restore_state(saved.objects, saved.state, saved.random_state);
        game.pending_dialog = saved.pending_dialog;
        game.validate_restored_state()?;
        Ok(game)
    }

    pub fn load_save_from(
        source: &str,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let saved: SavedGame =
            serde_json::from_str(source).map_err(|error| format!("cannot decode save: {error}"))?;
        if saved.version != SAVE_VERSION {
            return Err(format!(
                "unsupported save version: {} (expected {SAVE_VERSION})",
                saved.version
            ));
        }
        let mut game = Self::load_from(&saved.scenario_path, read)?;
        game.engine
            .restore_state(saved.objects, saved.state, saved.random_state);
        game.pending_dialog = saved.pending_dialog;
        game.validate_restored_state()?;
        Ok(game)
    }

    pub fn campaign_state(&self) -> Result<CampaignState, String> {
        let scenario = self
            .engine
            .world
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
            .world
            .objects
            .values()
            .filter(|object| object.properties.get("side").and_then(Value::as_str) == Some(side))
            .map(|object| {
                let mut values = object.properties.clone();
                values.insert("id".into(), Value::String(object.id.clone()));
                Value::Map(values)
            })
            .chain(match self.engine.world.state.get("recall") {
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
            .world
            .state
            .get(&format!("gold:{side}"))
            .and_then(Value::as_i64)
            .unwrap_or(0)
            * percentage
            / 100;
        let variables = self
            .engine
            .world
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
            .world
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

    pub fn snapshot(&self) -> Result<GameSnapshot, String> {
        let objects = self.engine.query("snapshot", Value::Nil)?;
        Ok(GameSnapshot {
            map: self.engine.world.map.clone(),
            objects,
        })
    }

    pub fn map(&self) -> &Map {
        &self.engine.world.map
    }

    pub fn map_tiles(&self) -> &MapTiles {
        &self.map_tiles
    }

    pub fn terrain_scripts(&self) -> &[TerrainScript] {
        &self.terrain_scripts
    }

    pub fn query(&self, function: &str, command: Value) -> Result<Value, String> {
        self.engine.query(function, command)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn acknowledge_dialog(&mut self) -> Result<(), String> {
        if self.pending_dialog.take().is_none() {
            return Err("no dialog is waiting for the UI".into());
        }
        Ok(())
    }

    pub fn execute(&mut self, function: &str, command: Value) -> Result<Vec<Value>, String> {
        if let Some(dialog) = &self.pending_dialog {
            return Err(format!("dialog {dialog} is waiting for the UI"));
        }
        let result = self.engine.execute(function, command)?;
        self.revision = self.revision.wrapping_add(1);
        let mut events = match result {
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
            events.push(self.dialog_event(&dialog)?);
        }
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
        self.query("status", Value::Nil)
            .map_err(|error| format!("invalid save state: {error}"))?;
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

/// Собирает игровую карту из WML-узла `[map]`.
///
/// Функция публична, чтобы отладочные инструменты читали карту точно так же,
/// как игра, не заводя второй парсер формата `data=<<...>>`.
pub fn load_map(node: &Node) -> Result<Map, String> {
    let width = usize::try_from(parse_i64(node, "width")?)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "[map].width must be a positive integer".to_owned())?;
    let height = usize::try_from(parse_i64(node, "height")?)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "[map].height must be a positive integer".to_owned())?;
    let rows: Vec<Vec<String>> = node
        .attribute("data")?
        .lines()
        .map(|line| line.split(',').map(|cell| cell.trim().to_owned()).collect())
        .collect();
    if rows.len() != height || rows.iter().any(|row| row.len() != width) {
        return Err(format!("map data does not match {width}x{height}"));
    }
    Ok(Map {
        width,
        height,
        cells: rows.into_iter().flatten().collect(),
    })
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

fn parse_color(value: &str) -> Result<[u8; 3], String> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 {
        return Err(format!("invalid map object color: {value}"));
    }
    let channel = |start| {
        u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| format!("invalid map object color: {value}"))
    };
    Ok([channel(0)?, channel(2)?, channel(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scripts() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts")
    }

    #[test]
    fn loads_first_battle() {
        let game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        assert_eq!(game.snapshot().unwrap().map.cells.len(), 48);
        assert_eq!(
            game.map_tiles().get("grassland").unwrap().color,
            [111, 145, 77]
        );
        let terrain =
            crate::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
                .unwrap();
        let grass_count = game
            .map()
            .cells
            .iter()
            .filter(|code| matches!(crate::terrain::visual_codes(code).0, "grassland" | "Gg"))
            .count();
        assert_eq!(
            terrain
                .ground()
                .iter()
                .filter(|sprite| { sprite.family == "grassland" && sprite.local_order == -1000 })
                .count(),
            grass_count
        );
        let forest_count = game
            .map()
            .cells
            .iter()
            .filter(|code| code.as_str() == "forest")
            .count();
        assert_eq!(
            terrain
                .world()
                .iter()
                .filter(|sprite| sprite.family == "forest")
                .count(),
            forest_count
        );
        assert!(
            terrain
                .world()
                .iter()
                .all(|sprite| sprite.frames.assets[0].starts_with("forest:mixed-summer"))
        );
        assert!(
            terrain
                .ground()
                .iter()
                .filter(|sprite| sprite.local_order == -1000)
                .map(|sprite| &sprite.frames.assets[0])
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1
        );
        assert_eq!(
            terrain,
            crate::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
                .unwrap()
        );
        assert_eq!(game.dialog(&game.start_dialog).unwrap().len(), 2);
        assert_eq!(game.revision(), 0);
    }

    #[test]
    fn loads_dirt_bases_and_sprite_based_grass_transitions() {
        let game = Game::load(scripts(), "scenarios/rooting_out_a_mage.wml").unwrap();
        let terrain =
            crate::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
                .unwrap();
        let dirt_count = game
            .map()
            .cells
            .iter()
            .filter(|code| crate::terrain::visual_codes(code).0 == "Re")
            .count();
        assert_eq!(
            terrain
                .ground()
                .iter()
                .filter(|sprite| { sprite.family == "dirt" && sprite.local_order == -1000 })
                .count(),
            dirt_count
        );
        let grassland_count = game
            .map()
            .cells
            .iter()
            .filter(|code| {
                matches!(
                    crate::terrain::visual_codes(code).0,
                    "grassland" | "Gg" | "Gs" | "Gd" | "Gll"
                )
            })
            .count();
        assert_eq!(
            terrain
                .ground()
                .iter()
                .filter(|sprite| { sprite.family == "grassland" && sprite.local_order == -1000 })
                .count(),
            grassland_count
        );
        assert!(terrain.ground().iter().any(|sprite| {
            sprite.family == "grassland" && sprite.frames.assets[0].starts_with("grassland:green-")
        }));
        assert!(
            !terrain
                .ground()
                .iter()
                .any(|sprite| sprite.frames.assets[0].starts_with("dirt:dirt-"))
        );
        let missing_assets = terrain
            .assets()
            .values()
            .filter(|path| !Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file())
            .collect::<Vec<_>>();
        assert!(missing_assets.is_empty(), "{missing_assets:?}");
    }

    #[test]
    fn loads_original_road_assets_for_each_campaign_map() {
        for scenario in [
            "scenarios/rooting_out_a_mage.wml",
            "scenarios/the_chase.wml",
            "scenarios/guarded_castle.wml",
            "scenarios/return_to_the_village.wml",
        ] {
            let game = Game::load(scripts(), scenario).unwrap();
            assert_eq!(game.terrain_scripts()[0].family, "road");
            let terrain =
                crate::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
                    .unwrap();
            let road_count = game
                .map()
                .cells
                .iter()
                .filter(|code| matches!(crate::terrain::visual_codes(code).0, "Rd" | "Rr" | "Rp"))
                .count();
            assert_eq!(
                terrain
                    .ground()
                    .iter()
                    .filter(|sprite| sprite.family == "road" && sprite.local_order == -1000)
                    .count(),
                road_count,
                "{scenario}"
            );
            assert!(terrain.assets().iter().all(|(id, path)| {
                !id.starts_with("road:")
                    || Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file()
            }));
        }
    }

    #[test]
    fn loads_original_decorations_for_each_campaign_map() {
        let decoration_codes = ["Efm", "Gvs", "Es", "Em", "Edb", "Eff", "Wm"];
        for scenario in [
            "scenarios/rooting_out_a_mage.wml",
            "scenarios/the_chase.wml",
            "scenarios/guarded_castle.wml",
            "scenarios/return_to_the_village.wml",
        ] {
            let game = Game::load(scripts(), scenario).unwrap();
            let terrain =
                crate::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
                    .unwrap();
            let decoration_sprites = terrain
                .ground()
                .iter()
                .filter(|sprite| sprite.family == "decorations")
                .collect::<Vec<_>>();

            for (index, code) in game.map().cells.iter().enumerate() {
                let (_, overlay) = crate::terrain::visual_codes(code);
                if decoration_codes.contains(&overlay) {
                    let position = crate::engine::Position {
                        x: (index % game.map().width + 1) as i64,
                        y: (index / game.map().width + 1) as i64,
                    };
                    assert!(
                        decoration_sprites
                            .iter()
                            .any(|sprite| sprite.anchor == position),
                        "{scenario}: no decoration sprite for {code} at {position:?}"
                    );
                }
            }

            assert!(decoration_sprites.iter().all(|sprite| {
                sprite.frames.assets.iter().all(|asset| {
                    let path = terrain.assets().get(asset).unwrap();
                    Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file()
                })
            }));
            for windmill in decoration_sprites
                .iter()
                .filter(|sprite| sprite.frames.assets[0].starts_with("decorations:windmill-"))
            {
                assert_eq!(windmill.frames.assets.len(), 18);
                assert!(matches!(windmill.frames.frame_ms, 30 | 50));
            }
        }
    }

    #[test]
    fn loads_original_hills_mountains_and_peaks_for_each_campaign_map() {
        let mut checked_long_ranges = 0;
        for scenario in [
            "scenarios/rooting_out_a_mage.wml",
            "scenarios/the_chase.wml",
            "scenarios/guarded_castle.wml",
            "scenarios/return_to_the_village.wml",
        ] {
            let game = Game::load(scripts(), scenario).unwrap();
            let terrain =
                crate::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
                    .unwrap();
            let elevated_count = game
                .map()
                .cells
                .iter()
                .filter(|code| matches!(crate::terrain::visual_codes(code).0, "Hh" | "Hd" | "Mm"))
                .count();
            assert_eq!(
                terrain
                    .ground()
                    .iter()
                    .filter(|sprite| sprite.family == "hills" && sprite.local_order == -1000)
                    .count(),
                elevated_count,
                "{scenario}"
            );
            let desert_count = game
                .map()
                .cells
                .iter()
                .filter(|code| crate::terrain::visual_codes(code).0 == "Hd")
                .count();
            assert_eq!(
                terrain
                    .ground()
                    .iter()
                    .filter(|sprite| {
                        sprite.family == "hills"
                            && sprite.local_order == -1000
                            && sprite.frames.assets[0].starts_with("hills:desert")
                    })
                    .count(),
                desert_count,
                "{scenario}"
            );
            if game
                .map()
                .cells
                .iter()
                .any(|code| crate::terrain::visual_codes(code).0 == "Mm")
            {
                assert!(terrain.world().iter().any(|sprite| {
                    sprite.family == "hills" && sprite.frames.assets[0].starts_with("hills:basic")
                }));
            }
            assert!(terrain.world().iter().all(|sprite| {
                let asset = sprite.frames.assets[0].as_str();
                (!matches!(asset, "hills:basic" | "hills:basic2" | "hills:basic3")
                    && !asset.starts_with("hills:basic-castle-"))
                    || sprite.offset == [-90.0, -108.0]
            }));
            for sprite in terrain.world().iter().filter(|sprite| {
                matches!(
                    sprite.frames.assets[0].as_str(),
                    "hills:basic_range3_1" | "hills:basic_range4_1"
                )
            }) {
                let (direction, side) = if sprite.frames.assets[0] == "hills:basic_range3_1" {
                    (2, 1)
                } else {
                    (1, 2)
                };
                let first = sprite.anchor;
                let first_neighbors = crate::terrain::neighbors(first);
                let second = first_neighbors[direction];
                let second_neighbors = crate::terrain::neighbors(second);
                let third = second_neighbors[direction];
                let third_neighbors = crate::terrain::neighbors(third);
                let fourth = third_neighbors[direction];
                let positions = [
                    first,
                    first_neighbors[side],
                    second,
                    second_neighbors[side],
                    third,
                    third_neighbors[side],
                    fourth,
                    crate::terrain::neighbors(fourth)[side],
                ];
                assert_eq!(
                    sprite.clip_hexes.as_slice(),
                    positions.as_slice(),
                    "{scenario}: mountain range at {:?} has the wrong clip mask",
                    sprite.anchor
                );
                for position in positions {
                    assert_eq!(
                        game.map()
                            .raw(position)
                            .ok()
                            .map(crate::terrain::visual_codes)
                            .map(|codes| codes.0),
                        Some("Mm"),
                        "{scenario}: mountain range at {:?} spills through {position:?}",
                        sprite.anchor
                    );
                }
                checked_long_ranges += 1;
            }

            let peak_positions = game
                .map()
                .cells
                .iter()
                .enumerate()
                .filter(|(_, code)| crate::terrain::visual_codes(code).1 == "Xm")
                .map(|(index, _)| crate::engine::Position {
                    x: (index % game.map().width + 1) as i64,
                    y: (index / game.map().width + 1) as i64,
                })
                .collect::<Vec<_>>();
            for position in peak_positions {
                assert!(terrain.world().iter().any(|sprite| {
                    sprite.family == "hills"
                        && sprite.anchor == position
                        && sprite.frames.assets[0].starts_with("hills:cloud")
                }));
            }

            let missing_assets = terrain
                .assets()
                .iter()
                .filter(|(id, path)| {
                    id.starts_with("hills:")
                        && !Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file()
                })
                .collect::<Vec<_>>();
            assert!(missing_assets.is_empty(), "{scenario}: {missing_assets:?}");
        }
        assert!(checked_long_ranges > 0);
    }

    #[test]
    fn successful_commands_advance_the_ui_revision() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        game.acknowledge_dialog().unwrap();
        game.execute("end_turn", Value::Nil).unwrap();
        assert_eq!(game.revision(), 1);
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
    fn rejects_invalid_map_dimensions() {
        for dimensions in ["width=0\nheight=1", "width=1\nheight=-1"] {
            let source = format!("[map]\n{dimensions}\ndata=<<\ngrassland\n>>\n[/map]");
            let nodes = wml::parse(&source).unwrap();
            assert!(load_map(&nodes[0]).is_err());
        }
    }

    #[test]
    fn filesystem_loader_rejects_paths_outside_scripts() {
        let error = Game::load(scripts(), "../Cargo.toml").err().unwrap();
        assert!(error.contains("escapes scripts directory"));
    }

    #[test]
    fn load_save_rejects_invalid_restored_state() {
        let game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let mut save: serde_json::Value = serde_json::from_str(&game.save().unwrap()).unwrap();
        save["state"].as_object_mut().unwrap().remove("scenario");
        let source = serde_json::to_string(&save).unwrap();
        let error = Game::load_save(scripts(), &source).err().unwrap();
        assert!(error.contains("invalid save state"));
    }
}
