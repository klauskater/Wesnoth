//! Sandboxed Lua execution over store capabilities.
//!
//! Owns one persistent VM and no authoritative game state. Contract:
//! `contracts/target/modules/engine/runtime.md`.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

use mlua::{HookTriggers, Lua, LuaOptions, RegistryKey, StdLib, Table, VmState};

const INSTRUCTION_TICKS: u32 = 100_000;
const INSTRUCTIONS_PER_TICK: u32 = 1_000;

use super::{
    Position,
    store::{Address, Operation, Store, Transaction, World},
    value::{self, Value},
};

#[derive(Clone)]
pub(crate) struct Runtime {
    lua: Rc<Lua>,
    instruction_ticks: Rc<Cell<u32>>,
}

#[derive(Debug)]
pub(crate) enum RuntimeError {
    BudgetExceeded,
    Script(String),
}

impl RuntimeError {
    fn script(error: impl ToString) -> Self {
        let message = error.to_string();
        if message.contains("Lua instruction limit exceeded") {
            Self::BudgetExceeded
        } else {
            Self::Script(message)
        }
    }

    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::BudgetExceeded => "budget_exceeded",
            Self::Script(_) => "script_error",
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BudgetExceeded => formatter.write_str("Lua instruction limit exceeded"),
            Self::Script(message) => formatter.write_str(message),
        }
    }
}

impl Runtime {
    #[cfg(test)]
    fn new(world: World, seed: u64, rule_source: String) -> Result<(Self, Store), String> {
        Self::load_entry(
            world,
            seed,
            BTreeMap::from([("test.entry".into(), rule_source)]),
            "test.entry",
        )
    }

    pub(crate) fn load_entry(
        world: World,
        seed: u64,
        module_sources: BTreeMap<String, String>,
        entry: &str,
    ) -> Result<(Self, Store), String> {
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())
            .map_err(|error| error.to_string())?;
        for unsafe_global in ["io", "os", "package", "dofile", "loadfile", "require"] {
            lua.globals()
                .set(unsafe_global, mlua::Value::Nil)
                .map_err(|error| error.to_string())?;
        }
        value::install(&lua).map_err(|error| error.to_string())?;
        install_require(&lua, module_sources).map_err(|error| error.to_string())?;
        let require: mlua::Function = lua.globals().get("require").map_err(|e| e.to_string())?;
        let module: Table = require.call(entry).map_err(|error| error.to_string())?;
        lua.set_named_registry_value("wesnoth.rule_module", module)
            .map_err(|error| error.to_string())?;
        let instruction_ticks = Rc::new(Cell::new(INSTRUCTION_TICKS));
        let remaining = Rc::clone(&instruction_ticks);
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(INSTRUCTIONS_PER_TICK),
            move |_, _| {
                let ticks = remaining.get();
                if ticks == 0 {
                    Err(mlua::Error::runtime("Lua instruction limit exceeded"))
                } else {
                    remaining.set(ticks - 1);
                    Ok(VmState::Continue)
                }
            },
        );
        Ok((
            Self {
                lua: Rc::new(lua),
                instruction_ticks,
            },
            Store::new(world, seed)?,
        ))
    }

    fn evaluate(
        &self,
        transaction: Transaction,
        function: &str,
        command: Value,
        writable: bool,
    ) -> Result<(Value, Transaction), RuntimeError> {
        let lua = Rc::clone(&self.lua);
        self.instruction_ticks.set(INSTRUCTION_TICKS);

        let transaction = Rc::new(RefCell::new(transaction));
        let active = Rc::new(Cell::new(true));
        let _guard = ActiveGuard(Rc::clone(&active));
        let context = create_context(&lua, Rc::clone(&transaction), active, writable)
            .map_err(RuntimeError::script)?;
        let module: Table = lua
            .named_registry_value("wesnoth.rule_module")
            .map_err(RuntimeError::script)?;
        let resolve: mlua::Function = module.get(function).map_err(RuntimeError::script)?;
        let result = resolve
            .call::<mlua::Value>((context, command.to_lua(&lua).map_err(RuntimeError::script)?))
            .and_then(Value::from_lua)
            .map_err(RuntimeError::script)?;

        let committed = transaction.borrow().clone();
        Ok((result, committed))
    }

    fn invoke_read(
        &self,
        transaction: Transaction,
        function: &str,
        command: Value,
    ) -> Result<Value, RuntimeError> {
        self.evaluate(transaction, function, command, false)
            .map(|(result, _)| result)
    }

    fn invoke_write(
        &self,
        transaction: Transaction,
        function: &str,
        command: Value,
    ) -> Result<(Value, Transaction), RuntimeError> {
        self.evaluate(transaction, function, command, true)
    }

    pub(crate) fn initialize(
        &self,
        transaction: Transaction,
        request: Value,
    ) -> Result<(Value, Transaction), RuntimeError> {
        self.invoke_write(transaction, "initialize", request)
    }

    pub(crate) fn dispatch(
        &self,
        transaction: Transaction,
        command: Value,
    ) -> Result<(Value, Transaction), RuntimeError> {
        self.invoke_write(transaction, "dispatch", command)
    }

    pub(crate) fn present(
        &self,
        transaction: Transaction,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        self.invoke_read(transaction, "present", request)
    }

    pub(crate) fn interact(
        &self,
        transaction: Transaction,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        self.invoke_read(transaction, "interact", request)
    }

    pub(crate) fn validate_restored(&self, transaction: Transaction) -> Result<(), RuntimeError> {
        self.invoke_read(transaction, "validate_restored", Value::Nil)
            .map(drop)
    }
}

struct ActiveGuard(Rc<Cell<bool>>);

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

fn ensure_active(active: &Cell<bool>) -> mlua::Result<()> {
    if active.get() {
        Ok(())
    } else {
        Err(mlua::Error::runtime("expired invocation context"))
    }
}

fn install_require(lua: &Lua, sources: BTreeMap<String, String>) -> mlua::Result<()> {
    let sources = Rc::new(sources);
    let cache: Rc<RefCell<BTreeMap<String, RegistryKey>>> = Rc::new(RefCell::new(BTreeMap::new()));
    let loading = Rc::new(RefCell::new(Vec::<String>::new()));
    let require = lua.create_function(move |lua, name: String| {
        if let Some(key) = cache.borrow().get(&name) {
            return lua.registry_value::<Table>(key);
        }
        if let Some(start) = loading.borrow().iter().position(|module| module == &name) {
            let mut cycle = loading.borrow()[start..].to_vec();
            cycle.push(name);
            return Err(mlua::Error::runtime(format!(
                "cyclic require: {}",
                cycle.join(" -> ")
            )));
        }
        let source = sources
            .get(&name)
            .ok_or_else(|| mlua::Error::runtime(format!("unknown package module: {name}")))?;
        loading.borrow_mut().push(name.clone());
        let result = lua
            .load(source)
            .set_name(format!("{}.lua", name.replace('.', "/")))
            .eval::<Table>();
        loading.borrow_mut().pop();
        let module = result?;
        let key = lua.create_registry_value(module.clone())?;
        cache.borrow_mut().insert(name, key);
        Ok(module)
    })?;
    lua.globals().set("require", require)
}

fn create_context(
    lua: &Lua,
    transaction: Rc<RefCell<Transaction>>,
    active: Rc<Cell<bool>>,
    writable: bool,
) -> mlua::Result<Table> {
    let context = lua.create_table()?;

    let objects = lua.create_table()?;
    let state = Rc::clone(&transaction);
    let invocation = Rc::clone(&active);
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
                ensure_active(&invocation)?;
                let state = state.borrow();
                match state.object(&id) {
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
    if writable {
        let state = Rc::clone(&transaction);
        let invocation = Rc::clone(&active);
        objects.set(
            "set",
            lua.create_function(
                move |_lua, (_self, id, property, value): (Table, String, String, mlua::Value)| {
                    ensure_active(&invocation)?;
                    let value = Value::from_lua(value)?;
                    state
                        .borrow_mut()
                        .apply(&[Operation::SetField {
                            address: Address::object(id),
                            field: property,
                            value,
                        }])
                        .map_err(mlua::Error::runtime)
                },
            )?,
        )?;
        let state = Rc::clone(&transaction);
        let invocation = Rc::clone(&active);
        objects.set(
            "add",
            lua.create_function(move |_lua, (_self, value): (Table, mlua::Value)| {
                ensure_active(&invocation)?;
                let Value::Map(mut properties) = Value::from_lua(value)? else {
                    return Err(mlua::Error::runtime("object must be a map"));
                };
                let Some(Value::String(id)) = properties.remove("id") else {
                    return Err(mlua::Error::runtime("object must have a string id"));
                };
                state
                    .borrow_mut()
                    .apply(&[Operation::Insert {
                        address: Address::object(id),
                        value: Value::Map(properties),
                    }])
                    .map_err(mlua::Error::runtime)
            })?,
        )?;
        let state = Rc::clone(&transaction);
        let invocation = Rc::clone(&active);
        objects.set(
            "remove",
            lua.create_function(move |_lua, (_self, id): (Table, String)| {
                ensure_active(&invocation)?;
                state
                    .borrow_mut()
                    .apply(&[Operation::Remove {
                        address: Address::object(id),
                    }])
                    .map_err(mlua::Error::runtime)
            })?,
        )?;
    }
    let state = Rc::clone(&transaction);
    let invocation = Rc::clone(&active);
    objects.set(
        "all",
        lua.create_function(move |lua, (_self, fields, children): (Table, Option<Vec<String>>, Option<Vec<String>>)| {
            ensure_active(&invocation)?;
            let state = state.borrow();
            let list = lua.create_table()?;
            for (index, object) in state.objects().enumerate() {
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
    let invocation = Rc::clone(&active);
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
                ensure_active(&invocation)?;
                let state = state.borrow();
                let Some(value) = state.state(&key) else {
                    return Ok(mlua::Value::Nil);
                };
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
    let invocation = Rc::clone(&active);
    state_api.set(
        "all",
        lua.create_function(move |lua, _self: Table| {
            ensure_active(&invocation)?;
            let state = state.borrow();
            let values = lua.create_table()?;
            for (key, value) in state.states() {
                values.set(key, value.to_lua(lua)?)?;
            }
            Ok(values)
        })?,
    )?;
    if writable {
        let state = Rc::clone(&transaction);
        let invocation = Rc::clone(&active);
        state_api.set(
            "set",
            lua.create_function(
                move |_lua, (_self, key, value): (Table, String, mlua::Value)| {
                    ensure_active(&invocation)?;
                    let mut state = state.borrow_mut();
                    let address = Address::state(key);
                    let exists = state.get(&address).map_err(mlua::Error::runtime)?.is_some();
                    let operation = match (value, exists) {
                        (mlua::Value::Nil, false) => return Ok(()),
                        (mlua::Value::Nil, true) => Operation::Remove { address },
                        (value, true) => Operation::Update {
                            address,
                            value: Value::from_lua(value)?,
                        },
                        (value, false) => Operation::Insert {
                            address,
                            value: Value::from_lua(value)?,
                        },
                    };
                    state.apply(&[operation]).map_err(mlua::Error::runtime)
                },
            )?,
        )?;
    }
    context.set("state", state_api)?;

    let map = lua.create_table()?;
    let state = Rc::clone(&transaction);
    let invocation = Rc::clone(&active);
    map.set(
        "search",
        lua.create_function(move |lua, (_self, request): (Table, Table)| {
            ensure_active(&invocation)?;
            let bounds = state
                .borrow()
                .map()
                .bounds()
                .map_err(mlua::Error::runtime)?;
            crate::pathfinding::search_lua(lua, bounds, request)
        })?,
    )?;
    let state = Rc::clone(&transaction);
    let invocation = Rc::clone(&active);
    map.set(
        "cells",
        lua.create_function(move |lua, _self: Table| {
            ensure_active(&invocation)?;
            let state = state.borrow();
            let map = state.map();
            let cells = lua.create_table_with_capacity(map.cells.len(), 0)?;
            for (index, value) in map.cells.iter().enumerate() {
                let item = lua.create_table_with_capacity(0, 2)?;
                item.set(
                    "position",
                    Value::Map(BTreeMap::from([
                        ("x".into(), Value::Integer((index % map.width + 1) as i64)),
                        ("y".into(), Value::Integer((index / map.width + 1) as i64)),
                    ]))
                    .to_lua(lua)?,
                )?;
                item.set("value", value.as_str())?;
                cells.raw_set(index + 1, item)?;
            }
            Ok(cells)
        })?,
    )?;
    let state = Rc::clone(&transaction);
    let invocation = Rc::clone(&active);
    map.set(
        "get",
        lua.create_function(move |_lua, (_self, position): (Table, Table)| {
            ensure_active(&invocation)?;
            let position = read_position(&position)?;
            state
                .borrow()
                .map()
                .raw(position)
                .map(str::to_owned)
                .map_err(mlua::Error::runtime)
        })?,
    )?;
    let state = Rc::clone(&transaction);
    let invocation = Rc::clone(&active);
    map.set(
        "are_adjacent",
        lua.create_function(move |_lua, (_self, a, b): (Table, Table, Table)| {
            ensure_active(&invocation)?;
            Ok(state
                .borrow()
                .map()
                .are_adjacent(read_position(&a)?, read_position(&b)?))
        })?,
    )?;
    let state = Rc::clone(&transaction);
    let invocation = Rc::clone(&active);
    map.set(
        "neighbors",
        lua.create_function(move |lua, (_self, position): (Table, Table)| {
            ensure_active(&invocation)?;
            let neighbors = state
                .borrow()
                .map()
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

    if writable {
        let random = lua.create_table()?;
        let invocation = Rc::clone(&active);
        random.set(
            "integer",
            lua.create_function(move |_lua, (_self, min, max): (Table, i64, i64)| {
                ensure_active(&invocation)?;
                transaction
                    .borrow_mut()
                    .random(min, max)
                    .map_err(mlua::Error::runtime)
            })?,
        )?;
        context.set("random", random)?;
    }
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
    use crate::engine::{Map, Object};

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
        let (engine, mut store) = Runtime::new(
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
                retain = function(c) saved_objects = c.objects return true end,
                use_retained = function() return saved_objects:get("unit") end,
                spin = function() while true do end end,
            }
        "#
            .into(),
        )
        .unwrap();
        let original = store.world() as *const World;
        let original_hp = store.world().objects["unit"].properties["hp"].clone();
        let (value, transaction) = engine
            .evaluate(
                store.begin(store.revision()).unwrap(),
                "read",
                Value::Nil,
                false,
            )
            .unwrap();
        assert_eq!(value, Value::Integer(10));
        drop(transaction);
        assert_eq!(original, store.world() as *const World);
        assert!(
            engine
                .invoke_read(store.begin(store.revision()).unwrap(), "random", Value::Nil)
                .is_err()
        );
        assert!(
            engine
                .invoke_read(
                    store.begin(store.revision()).unwrap(),
                    "write",
                    Value::Bool(false)
                )
                .is_err()
        );
        assert_eq!(original, store.world() as *const World);
        assert_eq!(
            engine
                .invoke_read(store.begin(store.revision()).unwrap(), "read", Value::Nil)
                .unwrap(),
            Value::Integer(10)
        );
        assert!(
            engine
                .invoke_write(
                    store.begin(store.revision()).unwrap(),
                    "write",
                    Value::Bool(true)
                )
                .is_err()
        );
        assert_eq!(original, store.world() as *const World);
        let (random, transaction) = engine
            .invoke_write(
                store.begin(store.revision()).unwrap(),
                "write",
                Value::Bool(false),
            )
            .unwrap();
        store.commit(transaction).unwrap();
        assert!(matches!(random, Value::Integer(1..=100_000)));
        assert_ne!(original, store.world() as *const World);
        assert_eq!(original_hp, Value::Integer(10));
        assert_eq!(
            store.world().objects["unit"].properties["hp"],
            Value::Integer(5)
        );
        assert_eq!(store.world().state["changed"], Value::Bool(true));
        let (_, transaction) = engine
            .invoke_write(store.begin(store.revision()).unwrap(), "retain", Value::Nil)
            .unwrap();
        store.commit(transaction).unwrap();
        assert!(
            engine
                .invoke_read(
                    store.begin(store.revision()).unwrap(),
                    "use_retained",
                    Value::Nil
                )
                .is_err()
        );
        let limit = engine
            .invoke_read(store.begin(store.revision()).unwrap(), "spin", Value::Nil)
            .unwrap_err();
        assert!(matches!(limit, RuntimeError::BudgetExceeded));
        assert_eq!(
            engine
                .invoke_read(store.begin(store.revision()).unwrap(), "read", Value::Nil)
                .unwrap(),
            Value::Integer(5)
        );
    }

    #[test]
    fn require_caches_exports_and_reports_cycles() {
        let world = || World {
            map: Map {
                width: 1,
                height: 1,
                cells: vec!["grassland".into()],
            },
            objects: BTreeMap::new(),
            state: BTreeMap::new(),
        };
        let (runtime, store) = Runtime::load_entry(
            world(),
            1,
            BTreeMap::from([
                (
                    "game.init".into(),
                    "local a=require('rules.shared'); local b=require('rules.shared'); assert(rawequal(a,b)); return { read=function() return a.value end }".into(),
                ),
                ("rules.shared".into(), "return { value=7 }".into()),
            ]),
            "game.init",
        )
        .unwrap();
        assert_eq!(
            runtime
                .invoke_read(store.begin(store.revision()).unwrap(), "read", Value::Null)
                .unwrap(),
            Value::Integer(7)
        );

        let error = Runtime::load_entry(
            world(),
            1,
            BTreeMap::from([
                ("game.init".into(), "return require('rules.a')".into()),
                ("rules.a".into(), "return require('rules.b')".into()),
                ("rules.b".into(), "return require('rules.a')".into()),
            ]),
            "game.init",
        )
        .err()
        .unwrap();
        assert!(error.contains("rules.a -> rules.b -> rules.a"));
    }
}
