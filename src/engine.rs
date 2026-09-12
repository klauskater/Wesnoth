use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use mlua::{Lua, LuaOptions, StdLib, Table};
use serde::{Deserialize, Serialize};

use crate::value::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<String>,
}

impl Map {
    pub fn get(&self, position: Position) -> Result<&str, String> {
        Ok(crate::terrain::gameplay_type(self.raw(position)?))
    }

    pub fn raw(&self, position: Position) -> Result<&str, String> {
        if position.x < 1
            || position.y < 1
            || position.x as usize > self.width
            || position.y as usize > self.height
        {
            return Err(format!(
                "position ({}, {}) is outside the map",
                position.x, position.y
            ));
        }
        let index = (position.y as usize - 1) * self.width + position.x as usize - 1;
        Ok(&self.cells[index])
    }

    pub fn are_adjacent(&self, a: Position, b: Position) -> bool {
        fn axial(position: Position) -> (i64, i64, i64) {
            let q = position.x - 1;
            let row = position.y - 1;
            // В WML чётный 1-based столбец расположен на полгекса выше
            // нечётного. В zero-based координатах это верхний odd-q.
            let r = row - (q + (q & 1)) / 2;
            (q, r, -q - r)
        }
        let a = axial(a);
        let b = axial(b);
        ((a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs()) / 2 == 1
    }

    pub fn neighbors(&self, position: Position) -> Vec<Position> {
        self.neighbors_iter(position).collect()
    }

    pub fn neighbors_iter(&self, position: Position) -> impl Iterator<Item = Position> + '_ {
        let diagonal_y = if position.x % 2 == 0 { -1 } else { 1 };
        [
            (-1, 0),
            (-1, diagonal_y),
            (0, -1),
            (0, 1),
            (1, 0),
            (1, diagonal_y),
        ]
        .into_iter()
        .map(move |(x, y)| Position {
            x: position.x + x,
            y: position.y + y,
        })
        .filter(|candidate| {
            candidate.x >= 1
                && candidate.y >= 1
                && candidate.x <= self.width as i64
                && candidate.y <= self.height as i64
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Object {
    pub id: String,
    pub properties: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct World {
    pub map: Map,
    pub objects: BTreeMap<String, Object>,
    pub state: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct DeterministicRandom {
    state: u64,
}

impl DeterministicRandom {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn integer(&mut self, min: i64, max: i64) -> Result<i64, String> {
        if min > max {
            return Err(format!("invalid random range {min}..{max}"));
        }
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        Ok(min + ((self.state >> 32) % (max - min + 1) as u64) as i64)
    }
}

#[derive(Clone)]
struct Transaction {
    world: Rc<World>,
    random: DeterministicRandom,
}

#[derive(Clone)]
pub struct Engine {
    /// Shared snapshot; transactions copy it only on their first write.
    pub world: Rc<World>,
    random: DeterministicRandom,
    lua: Rc<Lua>,
    shared_state: Rc<BTreeMap<String, Value>>,
}

impl Engine {
    pub fn new(mut world: World, seed: u64, rule_source: String) -> Result<Self, String> {
        // ponytail: immutable catalogs are shared between transactions. Cloning
        // them for every click made the full unit catalog dominate input latency.
        let shared_state = ["unit_types", "races", "traits"]
            .into_iter()
            .filter_map(|key| world.state.remove(key).map(|value| (key.into(), value)))
            .collect();
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())
            .map_err(|error| error.to_string())?;
        for unsafe_global in ["io", "os", "package", "dofile", "loadfile", "require"] {
            lua.globals()
                .set(unsafe_global, mlua::Value::Nil)
                .map_err(|error| error.to_string())?;
        }
        let module: Table = lua
            .load(&rule_source)
            .set_name("scenario_rules.lua")
            .eval()
            .map_err(|error| error.to_string())?;
        lua.set_named_registry_value("wesnoth.rule_module", module)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            world: Rc::new(world),
            random: DeterministicRandom::new(seed),
            lua: Rc::new(lua),
            shared_state: Rc::new(shared_state),
        })
    }

    pub(crate) fn saved_state(&self) -> (BTreeMap<String, Object>, BTreeMap<String, Value>, u64) {
        (
            self.world.objects.clone(),
            self.world.state.clone(),
            self.random.state,
        )
    }

    pub(crate) fn restore_state(
        &mut self,
        objects: BTreeMap<String, Object>,
        state: BTreeMap<String, Value>,
        random_state: u64,
    ) {
        let world = Rc::make_mut(&mut self.world);
        world.objects = objects;
        world.state = state;
        self.random.state = random_state;
    }

    fn evaluate(&self, function: &str, command: Value) -> Result<(Value, Transaction), String> {
        let lua = Rc::clone(&self.lua);

        let transaction = Rc::new(RefCell::new(Transaction {
            world: self.world.clone(),
            random: self.random.clone(),
        }));
        let context = create_context(&lua, Rc::clone(&transaction), Rc::clone(&self.shared_state))
            .map_err(|e| e.to_string())?;
        let module: Table = lua
            .named_registry_value("wesnoth.rule_module")
            .map_err(|error| error.to_string())?;
        let resolve: mlua::Function = module.get(function).map_err(|e| e.to_string())?;
        let result = resolve
            .call::<mlua::Value>((context, command.to_lua(&lua).map_err(|e| e.to_string())?))
            .and_then(Value::from_lua)
            .map_err(|error| error.to_string())?;

        let committed = transaction.borrow().clone();
        Ok((result, committed))
    }

    pub fn query(&self, function: &str, command: Value) -> Result<Value, String> {
        self.evaluate(function, command).map(|(result, _)| result)
    }

    pub fn execute(&mut self, function: &str, command: Value) -> Result<Value, String> {
        let (result, committed) = self.evaluate(function, command)?;
        self.world = committed.world;
        self.random = committed.random;
        Ok(result)
    }
}

fn create_context(
    lua: &Lua,
    transaction: Rc<RefCell<Transaction>>,
    shared_state: Rc<BTreeMap<String, Value>>,
) -> mlua::Result<Table> {
    let context = lua.create_table()?;

    let objects = lua.create_table()?;
    let state = Rc::clone(&transaction);
    objects.set(
        "get",
        lua.create_function(
            move |lua,
                  (_self, id, fields, children): (
                Table,
                String,
                Option<Vec<String>>,
                Option<Vec<String>>,
            )| {
                let state = state.borrow();
                match state.world.objects.get(&id) {
                    Some(object) => {
                        let table = project(
                            lua,
                            &object.properties,
                            fields.as_deref(),
                            children.as_deref(),
                        )?;
                        table.set("id", object.id.as_str())?;
                        Ok(mlua::Value::Table(table))
                    }
                    None => Ok(mlua::Value::Nil),
                }
            },
        )?,
    )?;
    let state = Rc::clone(&transaction);
    objects.set(
        "set",
        lua.create_function(
            move |_lua, (_self, id, property, value): (Table, String, String, mlua::Value)| {
                let value = Value::from_lua(value)?;
                let mut state = state.borrow_mut();
                let object = Rc::make_mut(&mut state.world)
                    .objects
                    .get_mut(&id)
                    .ok_or_else(|| mlua::Error::runtime(format!("unknown object: {id}")))?;
                object.properties.insert(property, value);
                Ok(())
            },
        )?,
    )?;
    let state = Rc::clone(&transaction);
    objects.set(
        "add",
        lua.create_function(move |_lua, (_self, value): (Table, mlua::Value)| {
            let Value::Map(mut properties) = Value::from_lua(value)? else {
                return Err(mlua::Error::runtime("object must be a map"));
            };
            let Some(Value::String(id)) = properties.remove("id") else {
                return Err(mlua::Error::runtime("object must have a string id"));
            };
            let mut state = state.borrow_mut();
            if state.world.objects.contains_key(&id) {
                return Err(mlua::Error::runtime(format!("duplicate object id: {id}")));
            }
            Rc::make_mut(&mut state.world)
                .objects
                .insert(id.clone(), Object { id, properties });
            Ok(())
        })?,
    )?;
    let state = Rc::clone(&transaction);
    objects.set(
        "remove",
        lua.create_function(move |_lua, (_self, id): (Table, String)| {
            Rc::make_mut(&mut state.borrow_mut().world)
                .objects
                .remove(&id)
                .map(|_| ())
                .ok_or_else(|| mlua::Error::runtime(format!("unknown object: {id}")))
        })?,
    )?;
    let state = Rc::clone(&transaction);
    objects.set(
        "all",
        lua.create_function(move |lua, (_self, fields, children): (Table, Option<Vec<String>>, Option<Vec<String>>)| {
            let state = state.borrow();
            let list = lua.create_table_with_capacity(state.world.objects.len(), 0)?;
            for (index, object) in state.world.objects.values().enumerate() {
                let table = project(lua, &object.properties, fields.as_deref(), children.as_deref())?;
                table.set("id", object.id.as_str())?;
                list.raw_set(index + 1, table)?;
            }
            Ok(list)
        })?,
    )?;
    context.set("objects", objects)?;

    let state_api = lua.create_table()?;
    let state = Rc::clone(&transaction);
    state_api.set(
        "get",
        lua.create_function(
            move |lua,
                  (_self, key, fields, children): (
                Table,
                String,
                Option<Vec<String>>,
                Option<Vec<String>>,
            )| {
                let state = state.borrow();
                let value = state
                    .world
                    .state
                    .get(&key)
                    .or_else(|| shared_state.get(&key))
                    .unwrap_or(&Value::Nil);
                if fields.is_some() || children.is_some() {
                    let properties = value
                        .as_map()
                        .ok_or_else(|| mlua::Error::runtime("projected state must be a map"))?;
                    Ok(mlua::Value::Table(project(
                        lua,
                        properties,
                        fields.as_deref(),
                        children.as_deref(),
                    )?))
                } else {
                    value.to_lua(lua)
                }
            },
        )?,
    )?;
    let state = Rc::clone(&transaction);
    state_api.set(
        "set",
        lua.create_function(
            move |_lua, (_self, key, value): (Table, String, mlua::Value)| {
                Rc::make_mut(&mut state.borrow_mut().world)
                    .state
                    .insert(key, Value::from_lua(value)?);
                Ok(())
            },
        )?,
    )?;
    context.set("state", state_api)?;

    crate::combat::install(lua, &context)?;
    let map = lua.create_table()?;
    let state = Rc::clone(&transaction);
    map.set(
        "search",
        lua.create_function(move |lua, (_self, request): (Table, Table)| {
            crate::pathfinding::search_lua(lua, &state.borrow().world.map, request)
        })?,
    )?;
    let state = Rc::clone(&transaction);
    map.set(
        "get",
        lua.create_function(move |_lua, (_self, position): (Table, Table)| {
            let position = read_position(&position)?;
            state
                .borrow()
                .world
                .map
                .get(position)
                .map(str::to_owned)
                .map_err(mlua::Error::runtime)
        })?,
    )?;
    let state = Rc::clone(&transaction);
    map.set(
        "are_adjacent",
        lua.create_function(move |_lua, (_self, a, b): (Table, Table, Table)| {
            Ok(state
                .borrow()
                .world
                .map
                .are_adjacent(read_position(&a)?, read_position(&b)?))
        })?,
    )?;
    let state = Rc::clone(&transaction);
    map.set(
        "neighbors",
        lua.create_function(move |lua, (_self, position): (Table, Table)| {
            let neighbors = state
                .borrow()
                .world
                .map
                .neighbors(read_position(&position)?)
                .into_iter()
                .map(|position| {
                    Value::Map(BTreeMap::from([
                        ("x".into(), Value::Integer(position.x)),
                        ("y".into(), Value::Integer(position.y)),
                    ]))
                })
                .collect();
            Value::List(neighbors).to_lua(lua)
        })?,
    )?;
    context.set("map", map)?;

    let random = lua.create_table()?;
    random.set(
        "integer",
        lua.create_function(move |_lua, (_self, min, max): (Table, i64, i64)| {
            transaction
                .borrow_mut()
                .random
                .integer(min, max)
                .map_err(mlua::Error::runtime)
        })?,
    )?;
    context.set("random", random)?;
    Ok(context)
}

// Serialize only requested fields directly from the borrowed snapshot. No intermediate
// Value tree, object clone, or full __children export is needed for projections.
fn project(
    lua: &Lua,
    properties: &BTreeMap<String, Value>,
    fields: Option<&[String]>,
    children: Option<&[String]>,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    if let Some(fields) = fields {
        for field in fields {
            if let Some(value) = properties.get(field) {
                table.set(field.as_str(), value.to_lua(lua)?)?;
            }
        }
    } else {
        for (key, value) in properties {
            if key != "__children" || children.is_none() {
                table.set(key.as_str(), value.to_lua(lua)?)?;
            }
        }
    }
    if let Some(children) = children {
        let selected = lua.create_table()?;
        if let Some(source) = properties.get("__children").and_then(Value::as_map) {
            for name in children {
                if let Some(value) = source.get(name) {
                    selected.set(name.as_str(), value.to_lua(lua)?)?;
                }
            }
        }
        table.set("__children", selected)?;
    }
    Ok(table)
}

fn read_position(table: &Table) -> mlua::Result<Position> {
    Ok(Position {
        x: table.get("x")?,
        y: table.get("y")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_share_world_and_copy_on_write_preserves_rollback_and_random() {
        let world = World {
            map: Map {
                width: 1,
                height: 1,
                cells: vec!["grassland".into()],
            },
            objects: BTreeMap::from([(
                "unit".into(),
                Object {
                    id: "unit".into(),
                    properties: BTreeMap::from([
                        ("hp".into(), Value::Integer(10)),
                        (
                            "large_unused_data".into(),
                            Value::String("x".repeat(100_000)),
                        ),
                    ]),
                },
            )]),
            state: BTreeMap::new(),
        };
        let mut engine = Engine::new(
            world,
            42,
            r#"
            return {
                read = function(c)
                    local unit = c.objects:get("unit", {"hp"})
                    assert(unit.large_unused_data == nil)
                    local all = c.objects:all({"hp"})
                    assert(all[1].large_unused_data == nil)
                    return unit.hp
                end,
                write = function(c, fail)
                    c.objects:set("unit", "hp", 5)
                    c.state:set("changed", true)
                    local random = c.random:integer(1, 100000)
                    if fail then error("rollback") end
                    return random
                end,
                random = function(c) return c.random:integer(1, 100000) end,
            }
        "#
            .into(),
        )
        .unwrap();
        let original = Rc::clone(&engine.world);
        let (value, transaction) = engine.evaluate("read", Value::Nil).unwrap();
        assert_eq!(value, Value::Integer(10));
        assert!(
            Rc::ptr_eq(&original, &transaction.world),
            "read queries must not clone the world"
        );
        let random = engine.query("random", Value::Nil).unwrap();
        assert_eq!(engine.query("write", Value::Bool(false)).unwrap(), random);
        assert!(Rc::ptr_eq(&original, &engine.world));
        assert_eq!(
            engine.query("read", Value::Nil).unwrap(),
            Value::Integer(10)
        );
        assert!(engine.execute("write", Value::Bool(true)).is_err());
        assert!(Rc::ptr_eq(&original, &engine.world));
        assert_eq!(engine.query("random", Value::Nil).unwrap(), random);
        assert_eq!(engine.execute("write", Value::Bool(false)).unwrap(), random);
        assert!(!Rc::ptr_eq(&original, &engine.world));
        assert_eq!(
            original.objects["unit"].properties["hp"],
            Value::Integer(10)
        );
        assert_eq!(
            engine.world.objects["unit"].properties["hp"],
            Value::Integer(5)
        );
        assert_eq!(engine.world.state["changed"], Value::Bool(true));
    }

    #[test]
    fn detects_hex_neighbors() {
        let map = Map {
            width: 5,
            height: 3,
            cells: vec!["g".into(); 15],
        };
        assert!(map.are_adjacent(Position { x: 2, y: 2 }, Position { x: 3, y: 2 }));
        assert!(map.are_adjacent(Position { x: 2, y: 2 }, Position { x: 1, y: 1 }));
        assert!(!map.are_adjacent(Position { x: 2, y: 2 }, Position { x: 1, y: 3 }));
        assert!(!map.are_adjacent(Position { x: 1, y: 1 }, Position { x: 3, y: 1 }));
        for position in [Position { x: 2, y: 2 }, Position { x: 3, y: 2 }] {
            let neighbors = map.neighbors(position);
            assert_eq!(neighbors.len(), 6);
            assert!(
                neighbors
                    .iter()
                    .all(|neighbor| map.are_adjacent(position, *neighbor))
            );
        }
        assert_eq!(map.neighbors(Position { x: 1, y: 1 }).len(), 3);
    }

    #[test]
    fn random_is_repeatable() {
        let mut a = DeterministicRandom::new(1);
        let mut b = DeterministicRandom::new(1);
        let left: Vec<_> = (0..5).map(|_| a.integer(1, 100).unwrap()).collect();
        let right: Vec<_> = (0..5).map(|_| b.integer(1, 100).unwrap()).collect();
        assert_eq!(left, right);
    }
}
