use std::path::Path;

use macroquad::prelude::*;
use wesnoth_engine::{engine::Position, game::Game};

use crate::{
    map_renderer::MapRenderer, map_viewport::MapViewport, sprite_renderer::SpriteRenderer,
};

pub enum Action {
    None,
    Back,
}

pub struct GameScreen {
    game: Game,
    map: MapRenderer,
    viewport: MapViewport,
    sprites: SpriteRenderer,
    selected_hex: Option<Position>,
}

impl GameScreen {
    pub async fn new(game: Game) -> Result<Self, String> {
        let map = MapRenderer::new(game.map(), game.map_tiles());
        let (min, max) = map.bounds();
        let viewport = MapViewport::new(min, max, vec2(screen_width(), screen_height()));
        let sprites =
            SpriteRenderer::load(game.terrain_scene(), Path::new(env!("CARGO_MANIFEST_DIR")))
                .await?;
        Ok(Self {
            game,
            map,
            viewport,
            sprites,
            selected_hex: None,
        })
    }

    pub fn draw(&mut self) -> Action {
        clear_background(Color::from_rgba(12, 17, 22, 255));
        let screen = vec2(screen_width(), screen_height());
        if let Some(point) = self.viewport.update(screen) {
            self.selected_hex = self.map.hex_at(point);
        }

        let elapsed_ms = (get_time() * 1000.0) as u64;
        self.map.draw_base(&self.viewport);
        self.sprites.draw_ground(
            self.game.terrain_scene().ground(),
            &self.viewport,
            elapsed_ms,
        );
        self.sprites.draw_world(
            self.game.terrain_scene().world(),
            &self.viewport,
            elapsed_ms,
        );
        self.map.draw_grid(&self.viewport, self.selected_hex);

        if is_key_pressed(KeyCode::Escape) {
            Action::Back
        } else {
            Action::None
        }
    }
}
