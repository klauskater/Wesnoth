use std::{collections::BTreeMap, path::Path};

use macroquad::prelude::*;
use wesnoth_engine::{
    engine::Map,
    terrain_scene::{PlacedSprite, SpriteFrames, TerrainScene, TerrainScript},
};

use crate::{map_renderer::hex_center, map_viewport::MapViewport};

const REFERENCE_RADIUS: f32 = 36.0;

pub struct SpriteRenderer {
    textures: Vec<Texture2D>,
    ground: Vec<DrawCommand>,
    world: Vec<DrawCommand>,
}

struct DrawCommand {
    position: Vec2,
    frames: PreparedFrames,
    clip_hexes: Vec<Vec2>,
}

enum PreparedFrames {
    Still(Frame),
    Animated {
        frames: Vec<Frame>,
        frame_ms: u32,
        phase_ms: u32,
    },
}

struct Frame {
    texture: usize,
    size: Vec2,
}

impl SpriteRenderer {
    pub async fn load(map: &Map, scripts: &[TerrainScript], root: &Path) -> Result<Self, String> {
        let scene = TerrainScene::from_lua(map, scripts)?;
        let mut textures = Vec::new();
        let mut texture_indices = BTreeMap::new();
        for (id, relative) in scene.assets() {
            let path = root.join(relative);
            let path = path
                .to_str()
                .ok_or_else(|| format!("sprite path is not UTF-8: {}", path.display()))?;
            let texture = load_texture(path)
                .await
                .map_err(|error| format!("cannot load sprite {id} from {path}: {error}"))?;
            texture.set_filter(FilterMode::Nearest);
            texture_indices.insert(id.clone(), textures.len());
            textures.push(texture);
        }
        let ground = prepare(scene.ground(), &texture_indices, &textures)?;
        let world = prepare(scene.world(), &texture_indices, &textures)?;
        Ok(Self {
            textures,
            ground,
            world,
        })
    }

    pub fn draw_ground(&self, viewport: &MapViewport, elapsed_ms: u64) {
        for command in &self.ground {
            self.draw(command, viewport, elapsed_ms);
        }
    }

    pub fn draw_world(&self, viewport: &MapViewport, elapsed_ms: u64) {
        for command in &self.world {
            self.draw(command, viewport, elapsed_ms);
        }
    }

    fn draw(&self, command: &DrawCommand, viewport: &MapViewport, elapsed_ms: u64) {
        let frame = command.frames.at(elapsed_ms);
        let texture = &self.textures[frame.texture];
        let screen = vec2(screen_width(), screen_height());
        let position = viewport.project(command.position, screen);
        let size = frame.size * viewport.zoom();
        if position.x > screen.x
            || position.y > screen.y
            || position.x + size.x < 0.0
            || position.y + size.y < 0.0
        {
            return;
        }
        if command.clip_hexes.is_empty() {
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
        } else {
            let mut vertices = Vec::with_capacity(command.clip_hexes.len() * 7);
            let mut indices = Vec::with_capacity(command.clip_hexes.len() * 18);
            for clip in &command.clip_hexes {
                let center = viewport.project(*clip, screen);
                let radius = viewport.zoom();
                let points = [
                    center + vec2(-radius, 0.0),
                    center + vec2(-radius * 0.5, -radius),
                    center + vec2(radius * 0.5, -radius),
                    center + vec2(radius, 0.0),
                    center + vec2(radius * 0.5, radius),
                    center + vec2(-radius * 0.5, radius),
                ];
                let first = vertices.len() as u16;
                let vertex = |point: Vec2| {
                    Vertex::new(
                        point.x,
                        point.y,
                        0.0,
                        (point.x - position.x) / size.x,
                        (point.y - position.y) / size.y,
                        WHITE,
                    )
                };
                vertices.push(vertex(center));
                vertices.extend(points.map(vertex));
                for corner in 0..6 {
                    indices.extend([first, first + corner + 1, first + (corner + 1) % 6 + 1]);
                }
            }
            draw_mesh(&Mesh {
                vertices,
                indices,
                texture: Some(texture.clone()),
            });
        }
    }
}

impl PreparedFrames {
    fn at(&self, elapsed_ms: u64) -> &Frame {
        match self {
            Self::Still(frame) => frame,
            Self::Animated {
                frames,
                frame_ms,
                phase_ms,
            } => {
                let index = ((elapsed_ms + u64::from(*phase_ms)) / u64::from(*frame_ms))
                    % frames.len() as u64;
                &frames[index as usize]
            }
        }
    }
}

fn prepare(
    sprites: &[PlacedSprite],
    texture_indices: &BTreeMap<String, usize>,
    textures: &[Texture2D],
) -> Result<Vec<DrawCommand>, String> {
    sprites
        .iter()
        .map(|sprite| {
            let frames = prepare_frames(&sprite.frames, texture_indices, textures)?;
            let offset = vec2(sprite.offset[0], sprite.offset[1]) / REFERENCE_RADIUS;
            Ok(DrawCommand {
                position: hex_center(sprite.anchor.x, sprite.anchor.y) + offset,
                frames,
                clip_hexes: sprite
                    .clip_hexes
                    .iter()
                    .map(|position| hex_center(position.x, position.y))
                    .collect(),
            })
        })
        .collect()
}

fn prepare_frames(
    source: &SpriteFrames,
    texture_indices: &BTreeMap<String, usize>,
    textures: &[Texture2D],
) -> Result<PreparedFrames, String> {
    let mut frames = source
        .assets
        .iter()
        .map(|asset| {
            let texture = *texture_indices
                .get(asset)
                .ok_or_else(|| format!("sprite texture was not loaded: {asset}"))?;
            Ok(Frame {
                texture,
                size: vec2(textures[texture].width(), textures[texture].height())
                    / REFERENCE_RADIUS,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if frames.len() == 1 {
        Ok(PreparedFrames::Still(frames.remove(0)))
    } else {
        Ok(PreparedFrames::Animated {
            frames,
            frame_ms: source.frame_ms,
            phase_ms: source.phase_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_animation_selects_only_its_current_frame() {
        let frames = PreparedFrames::Animated {
            frames: vec![
                Frame {
                    texture: 3,
                    size: Vec2::ZERO,
                },
                Frame {
                    texture: 7,
                    size: Vec2::ZERO,
                },
            ],
            frame_ms: 100,
            phase_ms: 50,
        };
        assert_eq!(frames.at(0).texture, 3);
        assert_eq!(frames.at(50).texture, 7);
        assert_eq!(frames.at(250).texture, 7);
    }
}
