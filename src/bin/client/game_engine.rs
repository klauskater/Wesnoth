//! Running game engine adapter.
//!
//! Owns the authoritative game session and translates semantic player input
//! into engine protocol interactions. It has no UI or window dependencies.

use std::collections::BTreeMap;

use wesnoth_engine::{
    engine::{
        Position,
        protocol::{Action as UiAction, InteractionKind},
    },
    value::Value,
};

use crate::{
    connection::{Connection, Delivery},
    resource_loader::EngineContext,
};

pub enum GameEvent {
    Activate(UiAction),
    CellClick(Position),
}

pub struct GameEngine {
    game: wesnoth_engine::game::Game,
    connection: Connection,
}

impl GameEngine {
    pub fn start(context: EngineContext) -> Self {
        Self {
            game: context.game,
            connection: context.connection,
        }
    }

    pub fn process(&mut self, events: Vec<GameEvent>) -> Vec<Delivery> {
        for event in events {
            match event {
                GameEvent::Activate(action) => self.activate(action),
                GameEvent::CellClick(position) => {
                    self.connection.interaction(
                        &mut self.game,
                        InteractionKind::CellClick,
                        position_value(position),
                        Value::Nil,
                    );
                }
            }
        }
        self.connection.receive()
    }

    pub fn snapshot(&mut self) -> Vec<Delivery> {
        self.connection.request_snapshot(&mut self.game);
        self.connection.receive()
    }

    fn activate(&mut self, action: UiAction) {
        let name = action.action.clone();
        self.connection.interaction(
            &mut self.game,
            InteractionKind::Activate,
            Value::String(name.clone()),
            Value::Map(BTreeMap::from([
                ("action".into(), Value::String(name)),
                ("payload".into(), action.payload),
            ])),
        );
    }
}

fn position_value(position: Position) -> Value {
    Value::Map(BTreeMap::from([
        ("x".into(), Value::Integer(position.x)),
        ("y".into(), Value::Integer(position.y)),
    ]))
}
