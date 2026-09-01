use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    engine::{Engine, Map, Object, World},
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
    engine: Engine,
    dialogs: BTreeMap<String, Vec<DialogLine>>,
    pending_dialog: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameSnapshot {
    pub map: Map,
    pub objects: Value,
}

impl Game {
    pub fn load(scripts: impl AsRef<Path>, scenario_path: &str) -> Result<Self, String> {
        let scripts = scripts.as_ref();
        let scenario_document = read_wml(&scripts.join(scenario_path))?;
        let scenario = one_root(&scenario_document, "scenario")?;
        let resources = scenario.child("resources")?;

        let map_document = read_wml(&resource_path(scripts, resources, "map")?)?;
        let map_node = one_root(&map_document, "map")?;
        let map = load_map(map_node)?;
        let mut map_objects = BTreeSet::new();
        for path in split_paths(resources.attribute("map_objects")?) {
            let roots = read_wml(&scripts.join(path))?;
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
            let roots = read_wml(&scripts.join(path))?;
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
            let mut properties = types
                .get(type_id)
                .cloned()
                .ok_or_else(|| format!("unknown unit type: {type_id}"))?;
            properties.extend(node_properties(unit));
            objects.insert(id.clone(), Object { id, properties });
        }

        let dialogs = load_dialogs(&read_wml(&resource_path(scripts, resources, "dialogs")?)?)?;
        let rule_source = split_paths(resources.attribute("rules")?)
            .map(|path| {
                fs::read_to_string(scripts.join(path))
                    .map_err(|error| format!("cannot read Lua rule {path}: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        let seed = parse_i64(scenario, "random_seed")? as u64;

        let unit_types = Value::Map(
            types
                .iter()
                .map(|(id, properties)| (id.clone(), Value::Map(properties.clone())))
                .collect(),
        );
        let mut engine = Engine::new(
            World {
                map,
                objects,
                state: BTreeMap::from([
                    ("scenario".into(), Value::Map(node_properties(scenario))),
                    ("unit_types".into(), unit_types),
                ]),
            },
            seed,
            rule_source,
        );
        engine.execute("initialize", Value::Nil)?;

        Ok(Self {
            id: scenario.attribute("id")?.into(),
            name: scenario.attribute("name")?.into(),
            start_dialog: scenario.attribute("on_start_dialog")?.into(),
            engine,
            dialogs,
            pending_dialog: Some(scenario.attribute("on_start_dialog")?.into()),
        })
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

fn read_wml(path: &Path) -> Result<Vec<Node>, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    wml::parse(&source).map_err(|error| format!("{}: {error}", path.display()))
}

fn one_root<'a>(nodes: &'a [Node], name: &str) -> Result<&'a Node, String> {
    let mut matching = nodes.iter().filter(|node| node.name == name);
    let node = matching.next().ok_or_else(|| format!("missing [{name}]"))?;
    if matching.next().is_some() {
        return Err(format!("expected one [{name}]"));
    }
    Ok(node)
}

fn resource_path(scripts: &Path, resources: &Node, name: &str) -> Result<PathBuf, String> {
    Ok(scripts.join(resources.attribute(name)?))
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
