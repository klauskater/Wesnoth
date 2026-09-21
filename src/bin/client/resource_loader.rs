//! Loads one selected adventure and splits its immutable resources between
//! the engine and the interface before either subsystem is started.

use std::collections::BTreeMap;

#[cfg(not(target_os = "android"))]
use std::{fs, path::Path};

use wesnoth_engine::{
    adventure::Adventure,
    engine::protocol::ViewSnapshot,
    game::{self, Game},
    value::Value,
};

use crate::connection::{Connection, Delivery};

pub struct GameContext {
    pub engine: EngineContext,
    pub interface: InterfaceContext,
}

pub struct EngineContext {
    pub game: Game,
    pub connection: Connection,
}

pub struct InterfaceContext {
    pub initial_view: ViewSnapshot,
    pub assets: BTreeMap<String, Vec<u8>>,
    pub font: Vec<u8>,
}

pub struct ResourceLoader;

impl ResourceLoader {
    pub fn load(adventure: &Adventure) -> Result<GameContext, String> {
        let scenario = adventure
            .scenarios
            .first()
            .ok_or_else(|| format!("adventure {} has no scenarios", adventure.id))?;
        let mut game = load_game(adventure, scenario)?;
        let mut connection = Connection::default();
        connection.request_snapshot(&mut game);
        let initial_view = take_initial_view(connection.receive())?;
        let assets = load_assets(&initial_view)?;
        let font = read_asset("assets/fonts/DejaVuSans.ttf")?;

        Ok(GameContext {
            engine: EngineContext { game, connection },
            interface: InterfaceContext {
                initial_view,
                assets,
                font,
            },
        })
    }
}

fn take_initial_view(deliveries: Vec<Delivery>) -> Result<ViewSnapshot, String> {
    let mut snapshot = None;
    for delivery in deliveries {
        match delivery {
            Delivery::ViewSnapshot(view) if snapshot.is_none() => snapshot = Some(view),
            Delivery::Error(error) => return Err(error.message),
            Delivery::ViewSnapshot(_) => return Err("engine returned two initial views".into()),
            Delivery::CommandResult(_) | Delivery::ViewUpdate(_) => {
                return Err("engine did not start with a view snapshot".into());
            }
        }
    }
    snapshot.ok_or_else(|| "engine returned no initial view".into())
}

fn load_assets(snapshot: &ViewSnapshot) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let registry = snapshot
        .blocks
        .iter()
        .find(|block| block.id == "assets")
        .map(|block| &block.content)
        .ok_or("presentation has no asset registry")?;
    if registry.get("schema").and_then(Value::as_str) != Some("assets") {
        return Err("asset block has an unsupported schema".into());
    }
    let Value::List(items) = registry.get("items").ok_or("asset block has no items")? else {
        return Err("asset items must be a list".into());
    };

    let mut assets = BTreeMap::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or("asset id must be a string")?;
        let path = item
            .get("path")
            .and_then(Value::as_str)
            .ok_or("asset path must be a string")?;
        let bytes = read_asset(path).map_err(|error| format!("cannot load asset {id}: {error}"))?;
        if assets.insert(id.to_owned(), bytes).is_some() {
            return Err(format!("duplicate asset id: {id}"));
        }
    }
    Ok(assets)
}

#[cfg(not(target_os = "android"))]
fn load_game(
    adventure: &Adventure,
    scenario: &wesnoth_engine::adventure::AdventureScenario,
) -> Result<Game, String> {
    let resources = game::load_adventure_resources(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        adventure,
        scenario,
        None,
    )?;
    Game::start(resources)
}

#[cfg(target_os = "android")]
fn load_game(
    adventure: &Adventure,
    scenario: &wesnoth_engine::adventure::AdventureScenario,
) -> Result<Game, String> {
    let resources = game::load_adventure_resources_from(
        adventure,
        scenario,
        None,
        &wesnoth_engine::embedded::read,
    )?;
    Game::start(resources)
}

#[cfg(not(target_os = "android"))]
fn read_asset(path: &str) -> Result<Vec<u8>, String> {
    let root = fs::canonicalize(env!("CARGO_MANIFEST_DIR"))
        .map_err(|error| format!("cannot open resource root: {error}"))?;
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!("invalid asset path: {path}"));
    }
    let full = fs::canonicalize(root.join(relative)).map_err(|error| format!("{path}: {error}"))?;
    if !full.starts_with(&root) {
        return Err(format!("asset path escapes resource root: {path}"));
    }
    fs::read(full).map_err(|error| format!("{path}: {error}"))
}

#[cfg(target_os = "android")]
fn read_asset(path: &str) -> Result<Vec<u8>, String> {
    wesnoth_engine::embedded::read_bytes(path).map(<[u8]>::to_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_adventure_is_loaded_before_engine_and_interface_start() {
        let adventure = Adventure::parse(include_str!(
            "../../../scripts/adventures/two_brothers.wml"
        ))
        .unwrap();
        let resources = ResourceLoader::load(&adventure).unwrap();

        assert_eq!(resources.engine.game.id, "01_rooting_out_a_mage");
        assert!(
            resources
                .interface
                .initial_view
                .blocks
                .iter()
                .any(|block| block.id == "map")
        );
        assert!(!resources.interface.assets.is_empty());
        assert!(resources.interface.assets.values().all(|bytes| !bytes.is_empty()));
        assert!(!resources.interface.font.is_empty());
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn asset_paths_cannot_escape_the_resource_root() {
        assert!(read_asset("../Cargo.toml").is_err());
    }
}
