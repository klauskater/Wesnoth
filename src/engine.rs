use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use mlua::{Lua, LuaOptions, StdLib, Table};

use crate::value::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Debug)]
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<String>,
}

impl Map {
    pub fn get(&self, position: Position) -> Result<&str, String> {
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
            let r = row - (q - (q & 1)) / 2;
            (q, r, -q - r)
        }
        let a = axial(a);
        let b = axial(b);
        ((a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs()) / 2 == 1
    }
}

#[derive(Clone, Debug)]
pub struct Object {
    pub id: String,
    pub properties: BTreeMap<String, Value>,
}

impl Object {
    fn snapshot(&self) -> Value {
        let mut values = self.properties.clone();
        values.insert("id".to_owned(), Value::String(self.id.clone()));
        Value::Map(values)
    }
}

#[derive(Clone, Debug)]
pub struct World {
    pub map: Map,
    pub objects: BTreeMap<String, Object>,
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
    world: World,
    random: DeterministicRandom,
}

pub struct Engine {
    pub world: World,
    random: DeterministicRandom,
    rule_source: String,
}

impl Engine {
    pub fn new(world: World, seed: u64, rule_source: String) -> Self {
        Self {
            world,
            random: DeterministicRandom::new(seed),
            rule_source,
        }
    }

    pub fn execute(&mut self, function: &str, command: Value) -> Result<Value, String> {
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())
            .map_err(|error| error.to_string())?;
        for unsafe_global in ["io", "os", "package", "dofile", "loadfile", "require"] {
            lua.globals()
                .set(unsafe_global, mlua::Value::Nil)
                .map_err(|error| error.to_string())?;
        }

        let transaction = Rc::new(RefCell::new(Transaction {
            world: self.world.clone(),
            random: self.random.clone(),
        }));
        let context = create_context(&lua, Rc::clone(&transaction)).map_err(|e| e.to_string())?;
        let module: Table = lua
            .load(&self.rule_source)
            .set_name("basic_combat.lua")
            .eval()
            .map_err(|error| error.to_string())?;
        let resolve: mlua::Function = module.get(function).map_err(|e| e.to_string())?;
        let result = resolve
            .call::<mlua::Value>((context, command.to_lua(&lua).map_err(|e| e.to_string())?))
            .and_then(Value::from_lua)
            .map_err(|error| error.to_string())?;

        let committed = transaction.borrow().clone();
        self.world = committed.world;
        self.random = committed.random;
        Ok(result)
    }
}

fn create_context(lua: &Lua, transaction: Rc<RefCell<Transaction>>) -> mlua::Result<Table> {
    let context = lua.create_table()?;

    let objects = lua.create_table()?;
    let state = Rc::clone(&transaction);
    objects.set(
        "get",
        lua.create_function(move |lua, (_self, id): (Table, String)| {
            let state = state.borrow();
            match state.world.objects.get(&id) {
                Some(object) => object.snapshot().to_lua(lua),
                None => Ok(mlua::Value::Nil),
            }
        })?,
    )?;
    let state = Rc::clone(&transaction);
    objects.set(
        "set",
        lua.create_function(
            move |_lua, (_self, id, property, value): (Table, String, String, mlua::Value)| {
                let value = Value::from_lua(value)?;
                let mut state = state.borrow_mut();
                let object = state
                    .world
                    .objects
                    .get_mut(&id)
                    .ok_or_else(|| mlua::Error::runtime(format!("unknown object: {id}")))?;
                object.properties.insert(property, value);
                Ok(())
            },
        )?,
    )?;
    context.set("objects", objects)?;

    let map = lua.create_table()?;
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
    fn detects_hex_neighbors() {
        let map = Map {
            width: 5,
            height: 3,
            cells: vec!["g".into(); 15],
        };
        assert!(map.are_adjacent(Position { x: 2, y: 2 }, Position { x: 3, y: 2 }));
        assert!(!map.are_adjacent(Position { x: 1, y: 1 }, Position { x: 3, y: 1 }));
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
