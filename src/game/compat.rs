use std::collections::BTreeMap;

use super::Game;
use crate::{
    engine::{
        Map,
        protocol::{Command, CommandStatus, Interaction, InteractionKind, ViewUpdate},
    },
    value::Value,
};

pub(super) struct State {
    next_command_id: u64,
    next_query_id: u64,
    query_view_revision: u64,
    latest_view: Option<ViewUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogLine {
    pub speaker: String,
    pub text: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            next_command_id: 1,
            next_query_id: 1,
            query_view_revision: 0,
            latest_view: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameSnapshot {
    pub map: Map,
    pub objects: Value,
}

impl Game {
    pub fn dialog(&self, id: &str) -> Result<&[DialogLine], String> {
        self.dialogs
            .get(id)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("unknown dialog: {id}"))
    }

    pub fn start_events(&self) -> Result<Vec<Value>, String> {
        let Some(dialog) = self
            .session
            .world()
            .data
            .get("pending_dialog")
            .and_then(Value::as_str)
        else {
            return Ok(Vec::new());
        };
        Ok(vec![self.dialog_event(dialog)?])
    }

    pub fn snapshot(&mut self) -> Result<GameSnapshot, String> {
        let objects = self.query("snapshot", Value::Nil)?;
        Ok(GameSnapshot {
            map: self.session.world().map.clone(),
            objects,
        })
    }

    pub fn take_view_update(&mut self) -> Option<ViewUpdate> {
        self.compat.latest_view.take()
    }

    pub fn query(&mut self, action: &str, request: Value) -> Result<Value, String> {
        let query_id = self.compat.next_query_id;
        self.compat.next_query_id = self
            .compat
            .next_query_id
            .checked_add(1)
            .ok_or_else(|| "query id exhausted".to_owned())?;
        let interaction_id = format!("query:{query_id}");
        let delivery = self.session.interact(
            "query",
            Interaction {
                interaction_id: interaction_id.clone(),
                expected_view_revision: self.compat.query_view_revision,
                kind: InteractionKind::Activate,
                target: Value::Nil,
                payload: Value::Map(BTreeMap::from([
                    ("query".into(), Value::String(action.into())),
                    ("payload".into(), request),
                ])),
            },
        );
        if let Some(error) = delivery.error {
            return Err(error.message);
        }
        if delivery.command.is_some() {
            return Err("query interaction unexpectedly produced a command".into());
        }
        let update = delivery
            .view
            .ok_or_else(|| "query interaction did not update the view".to_owned())?;
        self.compat.query_view_revision = update.view_revision;
        let content = update
            .replace_blocks
            .into_iter()
            .find(|block| block.id == "query")
            .ok_or_else(|| "query interaction did not produce a query view block".to_owned())?
            .content;
        if content.get("interaction_id").and_then(Value::as_str) != Some(&interaction_id) {
            return Err("query view block has a mismatched interaction id".into());
        }
        content
            .get("value")
            .cloned()
            .ok_or_else(|| "query view block has no value".to_owned())
    }

    pub fn revision(&self) -> u64 {
        self.session.world_revision()
    }

    pub fn acknowledge_dialog(&mut self) -> Result<(), String> {
        if !self.session.world().data.contains_key("pending_dialog") {
            return Err("no dialog is waiting for the UI".into());
        }
        let command_id = self.compat.next_command_id;
        self.compat.next_command_id = self.compat.next_command_id.saturating_add(1);
        let dispatch = self.dispatch_command(
            "compat",
            Command {
                command_id: format!("game:{command_id}"),
                expected_world_revision: self.session.world_revision(),
                action: "dismiss_dialog".into(),
                payload: Value::Nil,
            },
        );
        if let Some(error) = dispatch.result.error {
            return Err(error.message);
        }
        self.compat.latest_view = dispatch.view;
        Ok(())
    }

    pub fn execute(&mut self, function: &str, command: Value) -> Result<Vec<Value>, String> {
        let command_id = self.compat.next_command_id;
        self.compat.next_command_id = self
            .compat
            .next_command_id
            .checked_add(1)
            .ok_or_else(|| "command id exhausted".to_owned())?;
        let dispatch = self.dispatch_command(
            "compat",
            Command {
                command_id: format!("game:{command_id}"),
                expected_world_revision: self.session.world_revision(),
                action: function.into(),
                payload: command,
            },
        );
        if dispatch.result.status == CommandStatus::Rejected {
            return Err(dispatch
                .result
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "command rejected".into()));
        }
        self.compat.latest_view = dispatch.view;
        Ok(match dispatch.events.unwrap_or(Value::List(Vec::new())) {
            Value::List(events) => events,
            event => vec![event],
        })
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

    pub(super) fn validate_restored_state(&self) -> Result<(), String> {
        self.start_events()
            .map_err(|error| format!("invalid save dialog: {error}"))?;
        Ok(())
    }
}
