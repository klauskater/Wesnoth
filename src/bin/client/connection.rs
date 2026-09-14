//! Embedded protocol adapter and the client's command correlation owner.
//!
//! Contract: `contracts/target/modules/client/connection.md`.

use std::collections::VecDeque;

use wesnoth_engine::{
    engine::protocol::{Command, CommandResult, Error, Interaction, ViewSnapshot, ViewUpdate},
    game::Game,
    value::Value,
};

pub type RequestId = String;

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

    pub fn query(
        &mut self,
        game: &mut Game,
        action: &str,
        payload: Value,
    ) -> Result<Value, String> {
        game.query(action, payload)
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

    pub fn send_command(&mut self, game: &mut Game, command: Command) -> RequestId {
        let request = command.command_id.clone();
        let dispatch = game.dispatch_command(command);
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

    #[allow(dead_code)] // Public contract entry; input migration is the next client slice.
    pub fn send_interaction(&mut self, game: &mut Game, interaction: Interaction) -> RequestId {
        let request = interaction.interaction_id.clone();
        let delivery = game.interact(interaction);
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
        match game.view_snapshot() {
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
    use std::path::Path;
    use wesnoth_engine::engine::protocol::CommandStatus;

    #[test]
    fn preserves_result_then_view_order_and_snapshot_recovers() {
        let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");
        let mut game = Game::load(scripts, "scenarios/first_battle.wml").unwrap();
        game.acknowledge_dialog().unwrap();
        let mut connection = Connection::default();
        connection.request_snapshot(&mut game);
        assert!(matches!(
            connection.receive().as_slice(),
            [Delivery::ViewSnapshot(_)]
        ));

        connection.command(&mut game, "end_turn", Value::Nil);
        let deliveries = connection.receive();
        assert!(matches!(
            deliveries.as_slice(),
            [Delivery::CommandResult(result), Delivery::ViewUpdate(_)]
                if result.status == CommandStatus::Committed
        ));
    }
}
