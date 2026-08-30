use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    engine::{Engine, Map, Object, Position, World},
    value::Value,
    wml::{self, Node},
};

#[derive(Clone, Debug)]
pub struct DialogLine {
    pub speaker: String,
    pub text: String,
}

#[derive(Clone, Debug)]
struct Objective {
    result: String,
    when: String,
    object: String,
    dialog: String,
}

pub struct Game {
    pub id: String,
    pub name: String,
    pub start_dialog: String,
    pub engine: Engine,
    dialogs: BTreeMap<String, Vec<DialogLine>>,
    objectives: Vec<Objective>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub result: String,
    pub dialog: String,
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

        let mut types = BTreeMap::new();
        for path in split_paths(resources.attribute("unit_types")?) {
            let roots = read_wml(&scripts.join(path))?;
            let unit_type = one_root(&roots, "unit_type")?;
            types.insert(
                unit_type.attribute("id")?.to_owned(),
                load_unit_type(unit_type)?,
            );
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
            properties.insert("type".into(), Value::String(type_id.into()));
            properties.insert("side".into(), Value::String(unit.attribute("side")?.into()));
            properties.insert(
                "position".into(),
                position_value(parse_i64(unit, "x")?, parse_i64(unit, "y")?),
            );
            let max_hp = properties
                .get("max_hitpoints")
                .and_then(Value::as_i64)
                .ok_or_else(|| format!("unit type {type_id} has no max_hitpoints"))?;
            properties.insert("hitpoints".into(), Value::Integer(max_hp));
            objects.insert(id.clone(), Object { id, properties });
        }
        validate_positions(&map, &objects)?;

        let dialogs = load_dialogs(&read_wml(&resource_path(scripts, resources, "dialogs")?)?)?;
        let rule_source = fs::read_to_string(resource_path(scripts, resources, "rules")?)
            .map_err(|error| format!("cannot read Lua rule: {error}"))?;
        let objectives = scenario
            .children_named("objective")
            .map(|node| {
                Ok(Objective {
                    result: node.attribute("result")?.into(),
                    when: node.attribute("when")?.into(),
                    object: node.attribute("unit")?.into(),
                    dialog: node.attribute("dialog")?.into(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let seed = parse_i64(scenario, "random_seed")? as u64;

        Ok(Self {
            id: scenario.attribute("id")?.into(),
            name: scenario.attribute("name")?.into(),
            start_dialog: scenario.attribute("on_start_dialog")?.into(),
            engine: Engine::new(World { map, objects }, seed, rule_source),
            dialogs,
            objectives,
        })
    }

    pub fn dialog(&self, id: &str) -> Result<&[DialogLine], String> {
        self.dialogs
            .get(id)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("unknown dialog: {id}"))
    }

    pub fn attack(
        &mut self,
        attacker: &str,
        defender: &str,
        weapon: &str,
    ) -> Result<(Value, Option<Outcome>), String> {
        let command = Value::Map(BTreeMap::from([
            ("attacker".into(), Value::String(attacker.into())),
            ("defender".into(), Value::String(defender.into())),
            ("weapon".into(), Value::String(weapon.into())),
        ]));
        let result = self.engine.execute("resolve", command)?;
        let defeated = result.get("defeated").and_then(Value::as_str);
        let outcome = defeated.and_then(|id| {
            self.objectives
                .iter()
                .find(|objective| objective.when == "unit_defeated" && objective.object == id)
                .map(|objective| Outcome {
                    result: objective.result.clone(),
                    dialog: objective.dialog.clone(),
                })
        });
        Ok((result, outcome))
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

fn load_unit_type(node: &Node) -> Result<BTreeMap<String, Value>, String> {
    let mut properties = BTreeMap::new();
    properties.insert("name".into(), Value::String(node.attribute("name")?.into()));
    properties.insert(
        "max_hitpoints".into(),
        Value::Integer(parse_i64(node, "max_hitpoints")?),
    );

    let attacks = node
        .children_named("attack")
        .map(attributes_to_value)
        .collect::<Result<Vec<_>, String>>()?;
    if attacks.is_empty() {
        return Err(format!(
            "unit type {} has no attacks",
            node.attribute("id")?
        ));
    }
    properties.insert("attacks".into(), Value::List(attacks));
    properties.insert(
        "defense".into(),
        attributes_to_value(node.child("defense")?)?,
    );
    properties.insert(
        "movement_costs".into(),
        attributes_to_value(node.child("movement_costs")?)?,
    );
    Ok(properties)
}

fn attributes_to_value(node: &Node) -> Result<Value, String> {
    let values = node
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
    Ok(Value::Map(values))
}

fn position_value(x: i64, y: i64) -> Value {
    Value::Map(BTreeMap::from([
        ("x".into(), Value::Integer(x)),
        ("y".into(), Value::Integer(y)),
    ]))
}

fn validate_positions(map: &Map, objects: &BTreeMap<String, Object>) -> Result<(), String> {
    let mut occupied = BTreeMap::new();
    for object in objects.values() {
        let position = object.properties["position"].as_map().unwrap();
        let position = Position {
            x: position["x"].as_i64().unwrap(),
            y: position["y"].as_i64().unwrap(),
        };
        map.get(position)?;
        if let Some(other) = occupied.insert((position.x, position.y), &object.id) {
            return Err(format!(
                "objects {other} and {} share a position",
                object.id
            ));
        }
    }
    Ok(())
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
        assert_eq!(game.engine.world.map.cells.len(), 15);
        assert_eq!(game.engine.world.objects.len(), 2);
        assert_eq!(game.dialog(&game.start_dialog).unwrap().len(), 2);
    }

    #[test]
    fn executes_combat_rule() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let (result, _) = game.attack("alice", "bob", "sword").unwrap();
        assert_eq!(
            result.get("type").and_then(Value::as_str),
            Some("battle_resolved")
        );
        assert!(matches!(result.get("strikes"), Some(Value::List(values)) if !values.is_empty()));
    }

    #[test]
    fn failed_rule_rolls_back_state() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let before = game.engine.world.objects["bob"].properties["hitpoints"].clone();
        assert!(game.attack("alice", "bob", "missing").is_err());
        assert_eq!(
            game.engine.world.objects["bob"].properties["hitpoints"],
            before
        );
    }
}
