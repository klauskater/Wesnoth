use std::path::Path;

use super::Game;
use crate::{
    engine::{persistence as codec, resources::Package},
    value::Value,
};

impl Game {
    pub fn save(&self) -> Result<String, String> {
        String::from_utf8(self.session.save().map_err(|error| error.message)?)
            .map_err(|error| format!("save is not UTF-8: {error}"))
    }

    pub fn load_save(scripts: impl AsRef<Path>, source: &str) -> Result<Self, String> {
        let read = super::load::filesystem(scripts)?;
        Self::load_save_from(source, &read)
    }

    pub fn load_save_from(
        source: &str,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let package = Package::open(read)?;
        let snapshot = codec::decode(source.as_bytes(), &package.manifest().identity())
            .map_err(|error| error.message)?;
        let scenario_path = snapshot
            .world
            .state
            .get("session:scenario_path")
            .and_then(Value::as_str)
            .ok_or_else(|| "save has no scenario path".to_owned())?
            .to_owned();
        let game = Self::build_from(&scenario_path, None, read, Some(source.as_bytes()))
            .map_err(|error| format!("invalid save state: {error}"))?;
        game.validate_restored_state()?;
        Ok(game)
    }
}
