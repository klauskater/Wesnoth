//! Serializable values at Rust/Lua and protocol boundaries.
//!
//! Contains no game-specific field semantics. Contract:
//! `contracts/target/modules/engine/value.md`.

use std::{
    collections::{BTreeMap, HashSet},
    ffi::c_void,
};

use mlua::{Function, Lua, Table, Value as LuaValue};
use serde::{Deserialize, Serialize};

const VALUE_KIND: &str = "__wesnoth_value_kind";
const NULL_REGISTRY_KEY: &str = "wesnoth.value.null";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Value {
    #[serde(alias = "Nil")]
    Null,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl Value {
    #[allow(non_upper_case_globals)]
    pub const Nil: Self = Self::Null;

    pub fn to_lua(&self, lua: &Lua) -> mlua::Result<LuaValue> {
        Ok(match self {
            Self::Null => LuaValue::Table(null_value(lua)?),
            Self::Bool(value) => LuaValue::Boolean(*value),
            Self::Integer(value) => LuaValue::Integer(*value),
            Self::Number(value) if value.is_finite() => LuaValue::Number(*value),
            Self::Number(_) => return Err(mlua::Error::runtime("number must be finite")),
            Self::String(value) => LuaValue::String(lua.create_string(value)?),
            Self::List(values) => {
                let table = lua.create_table()?;
                let metatable = lua.create_table()?;
                metatable.raw_set(VALUE_KIND, "list")?;
                table.set_metatable(Some(metatable));
                for (index, value) in values.iter().enumerate() {
                    table.set(index + 1, value.to_lua(lua)?)?;
                }
                LuaValue::Table(table)
            }
            Self::Map(values) => {
                let table = lua.create_table()?;
                let metatable = lua.create_table()?;
                metatable.raw_set(VALUE_KIND, "record")?;
                table.set_metatable(Some(metatable));
                for (key, value) in values {
                    table.set(key.as_str(), value.to_lua(lua)?)?;
                }
                LuaValue::Table(table)
            }
        })
    }

    pub fn from_lua(value: LuaValue) -> mlua::Result<Self> {
        Self::from_lua_inner(value, &mut HashSet::new())
    }

    fn from_lua_inner(
        value: LuaValue,
        visiting: &mut HashSet<*const c_void>,
    ) -> mlua::Result<Self> {
        Ok(match value {
            LuaValue::Nil => Self::Null,
            LuaValue::Boolean(value) => Self::Bool(value),
            LuaValue::Integer(value) => Self::Integer(value),
            LuaValue::Number(value) if value.is_finite() => Self::Number(value),
            LuaValue::Number(_) => return Err(mlua::Error::runtime("number must be finite")),
            LuaValue::String(value) => Self::String(value.to_str()?.to_owned()),
            LuaValue::Table(table) => Self::from_table(table, visiting)?,
            other => {
                return Err(mlua::Error::runtime(format!(
                    "unsupported Lua value: {}",
                    other.type_name()
                )));
            }
        })
    }

    fn from_table(table: Table, visiting: &mut HashSet<*const c_void>) -> mlua::Result<Self> {
        let pointer = table.to_pointer();
        if !visiting.insert(pointer) {
            return Err(mlua::Error::runtime("cyclic Lua table"));
        }
        let value = Self::from_table_inner(table, visiting);
        visiting.remove(&pointer);
        value
    }

    fn from_table_inner(table: Table, visiting: &mut HashSet<*const c_void>) -> mlua::Result<Self> {
        let kind = table
            .metatable()
            .map(|metatable| metatable.raw_get::<Option<String>>(VALUE_KIND))
            .transpose()?
            .flatten();
        if kind.as_deref() == Some("null") {
            if table.raw_len() != 0 || table.clone().pairs::<LuaValue, LuaValue>().next().is_some()
            {
                return Err(mlua::Error::runtime("null value must not contain fields"));
            }
            return Ok(Self::Null);
        }
        let length = table.raw_len();
        let mut list = Vec::with_capacity(length);
        for index in 1..=length {
            let value: LuaValue = table.raw_get(index)?;
            if matches!(value, LuaValue::Nil) {
                return Err(mlua::Error::runtime("Lua list cannot contain holes"));
            }
            list.push(Self::from_lua_inner(value, visiting)?);
        }

        let mut count = 0;
        for pair in table.clone().pairs::<LuaValue, LuaValue>() {
            pair?;
            count += 1;
        }
        let is_list = kind.as_deref() == Some("list") || (kind.is_none() && length > 0);
        if is_list {
            if count != length {
                return Err(mlua::Error::runtime("ambiguous mixed Lua table"));
            }
            return Ok(Self::List(list));
        }

        let mut map = BTreeMap::new();
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;
            let key = match key {
                LuaValue::String(value) => value.to_str()?.to_owned(),
                _ => return Err(mlua::Error::runtime("Lua map key must be a string")),
            };
            map.insert(key, Self::from_lua_inner(value, visiting)?);
        }
        Ok(Self::Map(map))
    }

    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map()?.get(key)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }
}

pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    let api = lua.create_table()?;
    api.set("null", null_value(lua)?)?;
    for kind in ["list", "record"] {
        let constructor: Function = lua.create_function(move |lua, table: Option<Table>| {
            let table = table.unwrap_or(lua.create_table()?);
            mark(lua, &table, kind)?;
            Ok(table)
        })?;
        api.set(kind, constructor)?;
    }
    let wesnoth = lua
        .globals()
        .get::<Option<Table>>("wesnoth")?
        .unwrap_or(lua.create_table()?);
    wesnoth.set("value", api)?;
    lua.globals().set("wesnoth", wesnoth)
}

fn null_value(lua: &Lua) -> mlua::Result<Table> {
    if let Ok(value) = lua.named_registry_value(NULL_REGISTRY_KEY) {
        return Ok(value);
    }
    let value = lua.create_table()?;
    mark(lua, &value, "null")?;
    lua.set_named_registry_value(NULL_REGISTRY_KEY, value.clone())?;
    Ok(value)
}

fn mark(lua: &Lua, table: &Table, kind: &str) -> mlua::Result<()> {
    let metatable = match table.metatable() {
        Some(metatable) => metatable,
        None => lua.create_table()?,
    };
    metatable.raw_set(VALUE_KIND, kind)?;
    metatable.raw_set("__metatable", false)?;
    table.set_metatable(Some(metatable));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_values_keep_numeric_and_collection_types_across_lua() {
        let lua = Lua::new();
        let value = Value::Map(BTreeMap::from([
            ("integer".into(), Value::Integer(7)),
            ("number".into(), Value::Number(0.25)),
            ("empty_list".into(), Value::List(Vec::new())),
            ("empty_record".into(), Value::Map(BTreeMap::new())),
            ("null".into(), Value::Null),
        ]));
        assert_eq!(Value::from_lua(value.to_lua(&lua).unwrap()).unwrap(), value);
        assert!(Value::Number(f64::NAN).to_lua(&lua).is_err());
    }

    #[test]
    fn explicit_lua_constructors_preserve_empty_values_and_null_fields() {
        let lua = Lua::new();
        install(&lua).unwrap();
        let result: LuaValue = lua
            .load(
                "return wesnoth.value.record({missing=wesnoth.value.null, items=wesnoth.value.list()})",
            )
            .eval()
            .unwrap();
        assert_eq!(
            Value::from_lua(result).unwrap(),
            Value::Map(BTreeMap::from([
                ("items".into(), Value::List(Vec::new())),
                ("missing".into(), Value::Null),
            ]))
        );
    }

    #[test]
    fn rejects_cycles_and_ambiguous_mixed_tables() {
        let lua = Lua::new();
        let cyclic = lua.create_table().unwrap();
        cyclic.set("self", cyclic.clone()).unwrap();
        assert!(Value::from_lua(LuaValue::Table(cyclic)).is_err());

        let mixed = lua.create_table().unwrap();
        mixed.raw_set(1, "item").unwrap();
        mixed.raw_set("field", "value").unwrap();
        assert!(Value::from_lua(LuaValue::Table(mixed)).is_err());
    }
}
