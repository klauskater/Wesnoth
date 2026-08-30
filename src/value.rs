use std::collections::BTreeMap;

use mlua::{Lua, Table, Value as LuaValue};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Integer(i64),
    String(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl Value {
    pub fn to_lua(&self, lua: &Lua) -> mlua::Result<LuaValue> {
        Ok(match self {
            Self::Nil => LuaValue::Nil,
            Self::Bool(value) => LuaValue::Boolean(*value),
            Self::Integer(value) => LuaValue::Integer(*value),
            Self::String(value) => LuaValue::String(lua.create_string(value)?),
            Self::List(values) => {
                let table = lua.create_table()?;
                for (index, value) in values.iter().enumerate() {
                    table.set(index + 1, value.to_lua(lua)?)?;
                }
                LuaValue::Table(table)
            }
            Self::Map(values) => {
                let table = lua.create_table()?;
                for (key, value) in values {
                    table.set(key.as_str(), value.to_lua(lua)?)?;
                }
                LuaValue::Table(table)
            }
        })
    }

    pub fn from_lua(value: LuaValue) -> mlua::Result<Self> {
        Ok(match value {
            LuaValue::Nil => Self::Nil,
            LuaValue::Boolean(value) => Self::Bool(value),
            LuaValue::Integer(value) => Self::Integer(value),
            LuaValue::Number(value) if value.fract() == 0.0 => Self::Integer(value as i64),
            LuaValue::Number(value) => Self::String(value.to_string()),
            LuaValue::String(value) => Self::String(value.to_str()?.to_owned()),
            LuaValue::Table(table) => Self::from_table(table)?,
            other => {
                return Err(mlua::Error::runtime(format!(
                    "unsupported Lua value: {}",
                    other.type_name()
                )));
            }
        })
    }

    fn from_table(table: Table) -> mlua::Result<Self> {
        let length = table.raw_len();
        let mut list = Vec::with_capacity(length);
        let mut only_list = length > 0;
        for index in 1..=length {
            let value: LuaValue = table.raw_get(index)?;
            if matches!(value, LuaValue::Nil) {
                only_list = false;
                break;
            }
            list.push(Self::from_lua(value)?);
        }

        if only_list {
            let mut count = 0;
            for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                pair?;
                count += 1;
            }
            if count == length {
                return Ok(Self::List(list));
            }
        }

        let mut map = BTreeMap::new();
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;
            let key = match key {
                LuaValue::String(value) => value.to_str()?.to_owned(),
                LuaValue::Integer(value) => value.to_string(),
                _ => return Err(mlua::Error::runtime("Lua map key must be a string")),
            };
            map.insert(key, Self::from_lua(value)?);
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
}
