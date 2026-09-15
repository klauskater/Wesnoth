//! Embedded protocol adapter and the client's command correlation owner.
//!
//! Contract: `contracts/target/modules/client/connection.md`.

use std::collections::VecDeque;

use wesnoth_engine::{
    engine::protocol::{
        Command, CommandResult, Error, Interaction, InteractionKind, ViewSnapshot, ViewUpdate,
    },
    game::Game,
    value::Value,
};

pub type RequestId = String;
const VIEWER: &str = "connection:local";

#[derive(Clone, Debug, PartialEq)]
pub enum Delivery {
    CommandResult(CommandResult),
    ViewSnapshot(ViewSnapshot),
    ViewUpdate(ViewUpdate),
    Error(Error),
}

pub struct Connection {
    next_request: u64,
    world_revision: u64,
    view_revision: u64,
    inbox: VecDeque<Delivery>,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            next_request: 1,
            world_revision: 0,
            view_revision: 0,
            inbox: VecDeque::new(),
        }
    }
}

impl Connection {
    pub fn world_revision(&self) -> u64 {
        self.world_revision
    }

    pub fn command(&mut self, game: &mut Game, action: &str, payload: Value) -> RequestId {
        let request = self.request_id("command");
        self.send_command(
            game,
            Command {
                command_id: request.clone(),
                expected_world_revision: self.world_revision,
                action: action.into(),
                payload,
            },
        );
        request
    }

    pub fn interaction(
        &mut self,
        game: &mut Game,
        kind: InteractionKind,
        target: Value,
        payload: Value,
    ) -> RequestId {
        let request = self.request_id("interaction");
        self.send_interaction(
            game,
            Interaction {
                interaction_id: request.clone(),
                expected_view_revision: self.view_revision,
                kind,
                target,
                payload,
            },
        );
        request
    }

    pub fn send_command(&mut self, game: &mut Game, command: Command) -> RequestId {
        let request = command.command_id.clone();
        let dispatch = game.dispatch_command(VIEWER, command);
        self.world_revision = dispatch.result.world_revision;
        self.inbox
            .push_back(Delivery::CommandResult(dispatch.result));
        if let Some(update) = dispatch.view {
            self.view_revision = update.view_revision;
            self.inbox.push_back(Delivery::ViewUpdate(update));
        }
        if let Some(error) = dispatch.presentation_error {
            self.inbox.push_back(Delivery::Error(error));
        }
        request
    }

    pub fn send_interaction(&mut self, game: &mut Game, interaction: Interaction) -> RequestId {
        let request = interaction.interaction_id.clone();
        let delivery = game.interact(VIEWER, interaction);
        if let Some(command) = delivery.command {
            self.world_revision = command.result.world_revision;
            self.inbox
                .push_back(Delivery::CommandResult(command.result));
            if let Some(update) = command.view {
                self.view_revision = update.view_revision;
                self.inbox.push_back(Delivery::ViewUpdate(update));
            }
            if let Some(error) = command.presentation_error {
                self.inbox.push_back(Delivery::Error(error));
            }
        }
        if let Some(update) = delivery.view {
            self.view_revision = update.view_revision;
            self.inbox.push_back(Delivery::ViewUpdate(update));
        }
        if let Some(error) = delivery.error {
            self.inbox.push_back(Delivery::Error(error));
        }
        request
    }

    pub fn request_snapshot(&mut self, game: &mut Game) {
        match game.view_snapshot(VIEWER) {
            Ok(snapshot) => {
                self.world_revision = snapshot.world_revision;
                self.view_revision = snapshot.view_revision;
                self.inbox.push_back(Delivery::ViewSnapshot(snapshot));
            }
            Err(message) => self
                .inbox
                .push_back(Delivery::Error(Error::new("snapshot_error", message))),
        }
    }

    pub fn receive(&mut self) -> Vec<Delivery> {
        self.inbox.drain(..).collect()
    }

    fn request_id(&mut self, kind: &str) -> RequestId {
        let id = format!("local:{kind}:{}", self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;
    use wesnoth_engine::engine::protocol::CommandStatus;

    fn position(x: i64, y: i64) -> Value {
        Value::Map(BTreeMap::from([
            ("x".into(), Value::Integer(x)),
            ("y".into(), Value::Integer(y)),
        ]))
    }

    #[test]
    fn preserves_result_then_view_order_and_snapshot_recovers() {
        let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");
        let mut game = Game::load(scripts, "scenarios/first_battle.wml").unwrap();
        let mut connection = Connection::default();
        connection.request_snapshot(&mut game);
        let snapshot = connection.receive();
        assert!(matches!(snapshot.as_slice(), [Delivery::ViewSnapshot(_)]));
        let Delivery::ViewSnapshot(snapshot) = &snapshot[0] else {
            unreachable!()
        };
        assert!(snapshot.blocks.iter().any(|block| block.id == "dialog"));
        assert!(snapshot.blocks.iter().any(|block| block.id == "map"));
        assert!(snapshot.blocks.iter().any(|block| block.id == "assets"));
        assert!(snapshot.blocks.iter().any(|block| block.id == "navigation"));
        let end_turn = snapshot
            .blocks
            .iter()
            .find(|block| block.id == "hud")
            .and_then(|block| wesnoth_engine::engine::protocol::ui_block(&block.content).ok())
            .and_then(|root| root.children[0].action.clone())
            .unwrap();
        let advance = snapshot
            .blocks
            .iter()
            .find(|block| block.id == "dialog")
            .and_then(|block| wesnoth_engine::engine::protocol::ui_block(&block.content).ok())
            .and_then(|root| root.children[2].action.clone())
            .unwrap();
        connection.interaction(
            &mut game,
            InteractionKind::Activate,
            Value::String(advance.action.clone()),
            Value::Map(BTreeMap::from([
                ("action".into(), Value::String(advance.action)),
                ("payload".into(), advance.payload),
            ])),
        );
        let page = connection.receive();
        let [Delivery::ViewUpdate(page)] = page.as_slice() else {
            panic!("first dialog page must only update viewer state")
        };
        let advance = page
            .replace_blocks
            .iter()
            .find(|block| block.id == "dialog")
            .and_then(|block| wesnoth_engine::engine::protocol::ui_block(&block.content).ok())
            .and_then(|root| root.children[2].action.clone())
            .unwrap();
        connection.interaction(
            &mut game,
            InteractionKind::Activate,
            Value::String(advance.action.clone()),
            Value::Map(BTreeMap::from([
                ("action".into(), Value::String(advance.action)),
                ("payload".into(), advance.payload),
            ])),
        );
        let deliveries = connection.receive();
        assert!(matches!(
            deliveries.as_slice(),
            [Delivery::CommandResult(result), Delivery::ViewUpdate(update)]
                if result.status == CommandStatus::Committed
                    && update.remove_ids.iter().any(|id| id == "dialog")
        ));

        connection.interaction(
            &mut game,
            InteractionKind::Activate,
            Value::String(end_turn.action.clone()),
            Value::Map(BTreeMap::from([
                ("action".into(), Value::String(end_turn.action)),
                ("payload".into(), end_turn.payload),
            ])),
        );
        let deliveries = connection.receive();
        assert!(matches!(
            deliveries.as_slice(),
            [Delivery::CommandResult(result), Delivery::ViewUpdate(_)]
                if result.status == CommandStatus::Committed
        ));
    }

    #[test]
    fn cell_interactions_select_preview_and_confirm_movement() {
        let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");
        let mut game = Game::load(scripts, "scenarios/outpost_defense.wml").unwrap();
        let mut connection = Connection::default();
        connection.request_snapshot(&mut game);
        connection.receive();
        connection.command(&mut game, "dismiss_dialog", Value::Nil);
        connection.receive();

        connection.interaction(
            &mut game,
            InteractionKind::CellClick,
            position(2, 4),
            Value::Nil,
        );
        let selected = connection.receive();
        assert!(matches!(selected.as_slice(), [Delivery::ViewUpdate(_)]));
        let Delivery::ViewUpdate(selected) = &selected[0] else {
            unreachable!()
        };
        let selection = selected
            .replace_blocks
            .iter()
            .find(|block| block.id == "selection")
            .unwrap();
        assert_eq!(
            selection.content.get("object").and_then(Value::as_str),
            Some("eren")
        );
        assert!(matches!(
            selection.content.get("terrain").and_then(|value| value.get("label")),
            Some(Value::String(label)) if !label.is_empty()
        ));
        let unit = selected
            .replace_blocks
            .iter()
            .find(|block| block.id == "unit")
            .and_then(|block| wesnoth_engine::engine::protocol::ui_block(&block.content).ok())
            .unwrap();
        assert!(unit.children.iter().any(|node| {
            node.text
                .as_deref()
                .is_some_and(|text| text.starts_with("Здоровье:"))
        }));
        assert!(
            selected
                .replace_blocks
                .iter()
                .find(|block| block.id == "navigation")
                .and_then(|block| block.content.get("next_unit"))
                .and_then(|unit| unit.get("position"))
                .is_some()
        );

        connection.interaction(
            &mut game,
            InteractionKind::CellClick,
            position(5, 2),
            Value::Nil,
        );
        let preview = connection.receive();
        let Delivery::ViewUpdate(preview) = &preview[0] else {
            panic!("first destination click must only update the view")
        };
        let selection = preview
            .replace_blocks
            .iter()
            .find(|block| block.id == "selection")
            .unwrap();
        assert!(matches!(selection.content.get("path"), Some(Value::List(path)) if path.len() > 2));
        assert!(selection.content.get("defense").is_some());

        connection.interaction(
            &mut game,
            InteractionKind::CellClick,
            position(5, 2),
            Value::Nil,
        );
        let moved = connection.receive();
        assert!(matches!(
            moved.as_slice(),
            [Delivery::CommandResult(result), Delivery::ViewUpdate(update)]
                if result.status == CommandStatus::Committed
                    && update.effects.iter().any(|effect| effect.content.get("type")
                        .and_then(Value::as_str) == Some("object_moved"))
        ));
    }

    #[test]
    fn recruit_ui_action_is_interpreted_then_committed() {
        let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");
        let mut game = Game::load(scripts, "scenarios/outpost_defense.wml").unwrap();
        let mut connection = Connection::default();
        connection.request_snapshot(&mut game);
        let snapshot = connection.receive();
        let Delivery::ViewSnapshot(snapshot) = &snapshot[0] else {
            unreachable!()
        };
        let recruit_hex = snapshot
            .blocks
            .iter()
            .find(|block| block.id == "status")
            .and_then(|block| block.content.get("recruit_hexes"))
            .and_then(|value| match value {
                Value::List(values) => values.first(),
                _ => None,
            })
            .cloned()
            .unwrap();
        connection.command(&mut game, "dismiss_dialog", Value::Nil);
        connection.receive();
        connection.interaction(
            &mut game,
            InteractionKind::CellClick,
            recruit_hex,
            Value::Nil,
        );
        let selected = connection.receive();
        let Delivery::ViewUpdate(selected) = &selected[0] else {
            unreachable!()
        };
        let recruit = selected
            .replace_blocks
            .iter()
            .find(|block| block.id == "recruit")
            .unwrap();
        let root = wesnoth_engine::engine::protocol::ui_block(&recruit.content).unwrap();
        let action = root
            .children
            .iter()
            .find(|node| node.enabled && node.action.is_some())
            .and_then(|node| node.action.clone())
            .unwrap();
        connection.interaction(
            &mut game,
            InteractionKind::Activate,
            Value::String(action.action.clone()),
            Value::Map(BTreeMap::from([
                ("action".into(), Value::String(action.action)),
                ("payload".into(), action.payload),
            ])),
        );
        assert!(matches!(
            connection.receive().as_slice(),
            [Delivery::CommandResult(result), Delivery::ViewUpdate(_)]
                if result.status == CommandStatus::Committed
        ));
    }
}
