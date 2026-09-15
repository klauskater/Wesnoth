use std::collections::BTreeMap;

use super::Game;
use crate::value::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CampaignState {
    pub units: Vec<Value>,
    pub gold: i64,
    pub variables: BTreeMap<String, Value>,
}

impl Game {
    pub fn campaign_state(&mut self) -> Result<CampaignState, String> {
        let Value::Map(mut value) = self.query("campaign_state", Value::Nil)? else {
            return Err("campaign_state query must return a record".into());
        };
        let units = match value.remove("units") {
            Some(Value::List(units)) => units,
            _ => return Err("campaign_state.units must be a list".into()),
        };
        let gold = value
            .remove("gold")
            .and_then(|value| value.as_i64())
            .ok_or_else(|| "campaign_state.gold must be an integer".to_owned())?;
        let variables = match value.remove("variables") {
            Some(Value::Map(variables)) => variables,
            _ => return Err("campaign_state.variables must be a record".into()),
        };
        Ok(CampaignState {
            units,
            gold,
            variables,
        })
    }

    pub fn next_scenario(&mut self) -> Result<Option<String>, String> {
        match self.query("next_scenario", Value::Nil)? {
            Value::Null => Ok(None),
            Value::String(path) => Ok(Some(path)),
            _ => Err("next_scenario query must return a string or null".into()),
        }
    }
}
