//! Базовые механизмы игрового движка.
//!
//! [`Engine`] хранит мир и запускает изменяющие сценарии только по событию
//! игрока или тику времени. Формирование интерфейса и обработка экранного ввода
//! не являются обязанностями движка.

mod core;
pub mod data_format;
pub mod hex;
pub mod pathfinding;
pub mod persistence;
pub mod protocol;
pub mod resources;
mod runtime;
pub mod session;
pub mod store;
pub mod value;

pub use core::{Engine, EngineChange, EngineError};
pub use hex::Position;
pub use store::{Entity, Map, World};
