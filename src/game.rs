use std::{collections::BTreeMap, path::Path};

use crate::{
    engine::{
        Map,
        protocol::{Command, Interaction},
        session::{Dispatch, InteractionDelivery, Session},
    },
    map::MapTiles,
    terrain_scene::TerrainScript,
};

mod campaign;
mod compat;
mod load;
mod persistence;

pub use campaign::CampaignState;
pub use compat::{DialogLine, GameSnapshot};

pub struct Game {
    pub id: String,
    pub name: String,
    pub start_dialog: String,
    session: Session,
    map_tiles: MapTiles,
    terrain_scripts: Vec<TerrainScript>,
    scene_assets: BTreeMap<String, String>,
    dialogs: BTreeMap<String, Vec<DialogLine>>,
    compat: compat::State,
}

impl Game {
    pub fn load(scripts: impl AsRef<Path>, scenario_path: &str) -> Result<Self, String> {
        Self::load_with_campaign(scripts, scenario_path, None)
    }

    pub fn load_with_campaign(
        scripts: impl AsRef<Path>,
        scenario_path: &str,
        campaign: Option<&CampaignState>,
    ) -> Result<Self, String> {
        let read = load::filesystem(scripts)?;
        Self::load_with_campaign_from(scenario_path, campaign, &read)
    }

    pub fn load_from(
        scenario_path: &str,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::load_with_campaign_from(scenario_path, None, read)
    }

    pub fn load_with_campaign_from(
        scenario_path: &str,
        campaign: Option<&CampaignState>,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::build_from(scenario_path, campaign, read, None)
    }

    fn build_from(
        scenario_path: &str,
        campaign: Option<&CampaignState>,
        read: &impl Fn(&str) -> Result<String, String>,
        saved: Option<&[u8]>,
    ) -> Result<Self, String> {
        let loaded = load::scenario(scenario_path, campaign, read, saved)?;

        Ok(Self {
            id: loaded.id,
            name: loaded.name,
            start_dialog: loaded.start_dialog,
            session: loaded.session,
            map_tiles: loaded.map_tiles,
            terrain_scripts: loaded.terrain_scripts,
            scene_assets: loaded.scene_assets,
            dialogs: loaded.dialogs,
            compat: compat::State::default(),
        })
    }

    pub fn view_snapshot(
        &mut self,
        viewer: &str,
    ) -> Result<crate::engine::protocol::ViewSnapshot, String> {
        self.session.snapshot(viewer)
    }

    pub fn dispatch_command(&mut self, viewer: &str, command: Command) -> Dispatch {
        self.session.dispatch(viewer, command)
    }

    pub fn interact(&mut self, viewer: &str, interaction: Interaction) -> InteractionDelivery {
        self.session.interact(viewer, interaction)
    }

    pub fn map(&self) -> &Map {
        &self.session.world().map
    }

    pub fn map_tiles(&self) -> &MapTiles {
        &self.map_tiles
    }

    pub fn terrain_scripts(&self) -> &[TerrainScript] {
        &self.terrain_scripts
    }

    pub fn scene_assets(&self) -> &BTreeMap<String, String> {
        &self.scene_assets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use std::path::PathBuf;

    fn scripts() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts")
    }

    #[test]
    fn successful_commands_advance_the_ui_revision() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        game.acknowledge_dialog().unwrap();
        let before = game.revision();
        let before_view = game.view_snapshot("time-test").unwrap();
        let before_time = before_view
            .blocks
            .iter()
            .find(|block| block.id == "time")
            .and_then(|block| crate::engine::protocol::ui_block(&block.content).ok())
            .and_then(|root| root.children[1].asset.clone())
            .unwrap();
        game.execute("end_turn", Value::Nil).unwrap();
        assert!(game.revision() > before);
        let after_view = game.view_snapshot("time-test").unwrap();
        let after_time = after_view
            .blocks
            .iter()
            .find(|block| block.id == "time")
            .and_then(|block| crate::engine::protocol::ui_block(&block.content).ok())
            .and_then(|root| root.children[1].asset.clone())
            .unwrap();
        assert_ne!(after_time, before_time);
    }

    #[test]
    fn package_builds_generic_view_blocks() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let view = game.view_snapshot("test").unwrap();
        assert_eq!(
            view.blocks
                .iter()
                .map(|block| block.id.as_str())
                .collect::<Vec<_>>(),
            [
                "assets",
                "dialog",
                "hud",
                "map",
                "navigation",
                "objects",
                "scene",
                "status",
                "summary",
                "time"
            ]
        );
        let hud = view.blocks.iter().find(|block| block.id == "hud").unwrap();
        let root = crate::engine::protocol::ui_block(&hud.content).unwrap();
        assert_eq!(root.children[0].action.as_ref().unwrap().action, "end_turn");
        let time = view.blocks.iter().find(|block| block.id == "time").unwrap();
        let root = crate::engine::protocol::ui_block(&time.content).unwrap();
        assert_eq!(
            root.children[1].kind,
            crate::engine::protocol::UiKind::Image
        );
        assert!(
            root.children[1]
                .asset
                .as_deref()
                .unwrap()
                .starts_with("ui/time/")
        );
        assert!(matches!(time.content.get("map_tint"), Some(Value::List(tint)) if tint.len() == 4));
        let scene = view
            .blocks
            .iter()
            .find(|block| block.id == "scene")
            .unwrap();
        let items = crate::engine::protocol::scene_block(&scene.content).unwrap();
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| {
            matches!(item.layer.as_str(), "ground" | "world")
                && item.frames.first() == Some(&item.asset)
        }));
    }

    #[test]
    fn filesystem_loader_rejects_paths_outside_scripts() {
        let error = Game::load(scripts(), "../Cargo.toml").err().unwrap();
        assert!(error.contains("invalid package path"));
    }

    #[test]
    fn load_save_rejects_invalid_restored_state() {
        let game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let mut save: serde_json::Value = serde_json::from_str(&game.save().unwrap()).unwrap();
        save["store"]["world"]["state"]
            .as_object_mut()
            .unwrap()
            .remove("scenario");
        let source = serde_json::to_string(&save).unwrap();
        let error = Game::load_save(scripts(), &source).err().unwrap();
        assert!(error.contains("invalid save state"));
    }
}
