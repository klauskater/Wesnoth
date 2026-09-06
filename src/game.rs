use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use crate::{
    engine::{Engine, Map, Object, World},
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
    dialogs: BTreeMap<String, Vec<DialogLine>>,
    pending_dialog: Option<String>,
    scenario_path: String,
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
        let scripts = scripts.as_ref();
        Self::load_with_campaign_from(scenario_path, campaign, &|path| {
            fs::read_to_string(scripts.join(path))
                .map_err(|error| format!("cannot read {path}: {error}"))
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
        let mut map_objects = BTreeSet::new();
        for path in split_paths(resources.attribute("map_objects")?) {
            let roots = read_wml_from(path, read)?;
            let map_object = one_root(&roots, "map_object")?;
            let id = map_object.attribute("id")?.to_owned();
            if !map_objects.insert(id.clone()) {
                return Err(format!("duplicate map object id: {id}"));
            }
        }
        for cell in &map.cells {
            if !map_objects.contains(cell) {
                return Err(format!("map uses unknown map object: {cell}"));
            }
        }

        let mut types = BTreeMap::new();
        for path in split_paths(resources.attribute("unit_types")?) {
            let roots = read_wml_from(path, read)?;
            let unit_type = one_root(&roots, "unit_type")?;
            let id = unit_type.attribute("id")?.to_owned();
            if types
                .insert(id.clone(), node_properties(unit_type))
                .is_some()
            {
                return Err(format!("duplicate unit type id: {id}"));
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
        let mut initial_state = BTreeMap::from([
            ("scenario".into(), Value::Map(node_properties(scenario))),
            ("unit_types".into(), unit_types),
            ("recall".into(), Value::List(recall)),
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
        );
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
            dialogs,
            pending_dialog: Some(scenario.attribute("on_start_dialog")?.into()),
            scenario_path: scenario_path.into(),
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
        Ok(game)
    }

    pub fn campaign_state(&self) -> Result<CampaignState, String> {
        let scenario = self.engine.world.state["scenario"].as_map().unwrap();
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
        let mut engine = self.engine.clone();
        let objects = engine.execute("snapshot", Value::Nil)?;
        Ok(GameSnapshot {
            map: self.engine.world.map.clone(),
            objects,
        })
    }

    pub fn query(&self, function: &str, command: Value) -> Result<Value, String> {
        let mut engine = self.engine.clone();
        engine.execute(function, command)
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

fn load_map(node: &Node) -> Result<Map, String> {
    let width = parse_i64(node, "width")? as usize;
    let height = parse_i64(node, "height")? as usize;
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
        assert_eq!(game.dialog(&game.start_dialog).unwrap().len(), 2);
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
}
