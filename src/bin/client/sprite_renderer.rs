use std::{collections::BTreeMap, path::Path};

use macroquad::prelude::*;
use wesnoth_engine::terrain_scene::{PlacedSprite, TerrainScene};

use crate::{map_renderer::hex_center, map_viewport::MapViewport};

const REFERENCE_RADIUS: f32 = 36.0;

pub struct SpriteRenderer {
    textures: BTreeMap<String, Texture2D>,
}

impl SpriteRenderer {
    pub async fn load(scene: &TerrainScene, root: &Path) -> Result<Self, String> {
        let mut textures = BTreeMap::new();
        for (id, relative) in scene.assets() {
            let path = root.join(relative);
            let path = path
                .to_str()
                .ok_or_else(|| format!("sprite path is not UTF-8: {}", path.display()))?;
            let texture = load_texture(path)
                .await
                .map_err(|error| format!("cannot load sprite {id} from {path}: {error}"))?;
            texture.set_filter(FilterMode::Nearest);
            textures.insert(id.clone(), texture);
        }
        Ok(Self { textures })
    }

    pub fn draw_ground<'a>(
        &self,
        sprites: impl IntoIterator<Item = &'a PlacedSprite>,
        viewport: &MapViewport,
        elapsed_ms: u64,
    ) {
        for sprite in sprites {
            self.draw_sprite(sprite, viewport, elapsed_ms);
        }
    }

    pub fn draw_world<'a>(
        &self,
        sprites: impl IntoIterator<Item = &'a PlacedSprite>,
        viewport: &MapViewport,
        elapsed_ms: u64,
    ) {
        for sprite in sprites {
            self.draw_sprite(sprite, viewport, elapsed_ms);
        }
    }

    fn draw_sprite(&self, sprite: &PlacedSprite, viewport: &MapViewport, elapsed_ms: u64) {
        let Some(texture) = self.textures.get(sprite.frames.asset_at(elapsed_ms)) else {
            return;
        };
        let screen = vec2(screen_width(), screen_height());
        let offset = vec2(sprite.offset[0], sprite.offset[1]) / REFERENCE_RADIUS;
        let position = viewport.project(
            hex_center(sprite.anchor.x, sprite.anchor.y) + offset,
            screen,
        );
        let size = vec2(texture.width(), texture.height()) / REFERENCE_RADIUS * viewport.zoom();
        if position.x > screen.x
            || position.y > screen.y
            || position.x + size.x < 0.0
            || position.y + size.y < 0.0
        {
            return;
        }
        draw_texture_ex(
            texture,
            position.x,
            position.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(size),
                ..Default::default()
            },
        );
    }
}
