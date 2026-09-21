//! Scene block preparation and rendering.
//!
//! Contract: `contracts/target/modules/client/scene.md`.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::path::Path;

use macroquad::prelude::*;
#[cfg(test)]
use wesnoth_engine::terrain_scene::TerrainScene;
use wesnoth_engine::{
    engine::protocol::SceneItem,
    terrain_scene::{ImageModifiers, PlacedSprite, SpriteFrames, TerrainPass},
    value::Value,
};

use crate::{
    map_renderer::{hex_center, point_in_hex},
    map_viewport::MapViewport,
};

const REFERENCE_RADIUS: f32 = 36.0;

pub struct SpriteRenderer {
    textures: Vec<Texture2D>,
    ground: Vec<DrawCommand>,
    world: Vec<DrawCommand>,
    hit_regions: Vec<(String, wesnoth_engine::engine::Position)>,
}

struct DrawCommand {
    position: Vec2,
    frames: PreparedFrames,
    clip_hexes: Vec<Vec2>,
    tint: Color,
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
    pub fn from_memory(
        items: &[SceneItem],
        registry: &Value,
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, String> {
        let assets = asset_registry(registry)?;
        let mut images = BTreeMap::new();
        for id in wanted_assets(items) {
            let bytes = resources
                .get(id)
                .ok_or_else(|| format!("scene asset was not loaded: {id}"))?;
            let image = Image::from_file_with_format(bytes, None)
                .map_err(|error| format!("cannot decode sprite {id}: {error}"))?;
            if !assets.contains_key(id) {
                return Err(format!("scene references unknown asset: {id}"));
            }
            images.insert(id.clone(), image);
        }
        Self::from_images(items, images)
    }

    fn from_images(items: &[SceneItem], images: BTreeMap<String, Image>) -> Result<Self, String> {
        if let Some(item) = items
            .iter()
            .find(|item| item.layer != "ground" && item.layer != "world")
        {
            return Err(format!(
                "scene item {} uses unsupported layer: {}",
                item.id, item.layer
            ));
        }
        let mut textures = Vec::new();
        let mut ground = scene_items(items, "ground")?;
        let mut world = scene_items(items, "world")?;
        ground.sort_by_key(|(order, id, _, _)| (*order, id.clone()));
        world.sort_by_key(|(order, id, _, _)| (*order, id.clone()));
        let (ground, ground_tints): (Vec<_>, Vec<_>) = ground
            .into_iter()
            .map(|(_, _, sprite, tint)| (sprite, tint))
            .unzip();
        let (world, world_tints): (Vec<_>, Vec<_>) = world
            .into_iter()
            .map(|(_, _, sprite, tint)| (sprite, tint))
            .unzip();
        let mut cache = BTreeMap::new();
        let mut ground = prepare(&ground, &images, &mut textures, &mut cache)?;
        let mut world = prepare(&world, &images, &mut textures, &mut cache)?;
        ground
            .iter_mut()
            .zip(ground_tints)
            .for_each(|(command, tint)| command.tint = tint);
        world
            .iter_mut()
            .zip(world_tints)
            .for_each(|(command, tint)| command.tint = tint);
        let hit_regions = items
            .iter()
            .filter_map(|item| item.hit_id.as_ref().map(|id| (id.clone(), item.anchor)))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .collect();
        Ok(Self {
            textures,
            ground,
            world,
            hit_regions,
        })
    }

    pub fn draw_ground(&self, viewport: &MapViewport, elapsed_ms: u64, tint: Color) {
        for command in &self.ground {
            self.draw(command, viewport, elapsed_ms, tint);
        }
    }

    pub fn draw_world(&self, viewport: &MapViewport, elapsed_ms: u64, tint: Color) {
        for command in &self.world {
            self.draw(command, viewport, elapsed_ms, tint);
        }
    }

    pub fn hit_at(&self, point: Vec2) -> Option<(&str, wesnoth_engine::engine::Position)> {
        hit_region_at(&self.hit_regions, point).map(|(id, position)| (id.as_str(), *position))
    }

    fn draw(&self, command: &DrawCommand, viewport: &MapViewport, elapsed_ms: u64, tint: Color) {
        let frame = command.frames.at(elapsed_ms);
        let tint = Color::new(
            tint.r * command.tint.r,
            tint.g * command.tint.g,
            tint.b * command.tint.b,
            tint.a * command.tint.a,
        );
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
                tint,
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
                // Wildcard padding in terrain rules can extend outside the PNG.
                // Clip geometry too: out-of-range UVs smear the border texels.
                let points = clip_to_rect(points.to_vec(), position, position + size);
                if points.len() < 3 {
                    continue;
                }
                let first = vertices.len() as u16;
                let vertex = |point: Vec2| {
                    Vertex::new(
                        point.x,
                        point.y,
                        0.0,
                        (point.x - position.x) / size.x,
                        (point.y - position.y) / size.y,
                        tint,
                    )
                };
                let count = points.len() as u16;
                vertices.extend(points.into_iter().map(vertex));
                for corner in 1..count - 1 {
                    indices.extend([first, first + corner, first + corner + 1]);
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

fn wanted_assets(items: &[SceneItem]) -> BTreeSet<&String> {
    items
        .iter()
        .flat_map(|item| item.frames.iter().chain(&item.masks))
        .collect()
}

fn asset_registry(value: &Value) -> Result<BTreeMap<String, String>, String> {
    if value.get("schema").and_then(Value::as_str) != Some("assets") {
        return Err("asset block has an unsupported schema".into());
    }
    let Value::List(items) = value.get("items").ok_or("asset block has no items")? else {
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
        if assets.insert(id.to_owned(), path.to_owned()).is_some() {
            return Err(format!("duplicate asset id: {id}"));
        }
    }
    Ok(assets)
}

fn hit_region_at(
    regions: &[(String, wesnoth_engine::engine::Position)],
    point: Vec2,
) -> Option<&(String, wesnoth_engine::engine::Position)> {
    regions
        .iter()
        .filter(|(_, position)| point_in_hex(point - hex_center(position.x, position.y)))
        .min_by(|(_, left), (_, right)| {
            hex_center(left.x, left.y)
                .distance_squared(point)
                .total_cmp(&hex_center(right.x, right.y).distance_squared(point))
        })
}

fn scene_items(
    items: &[SceneItem],
    layer: &str,
) -> Result<Vec<(i64, String, PlacedSprite, Color)>, String> {
    items
        .iter()
        .filter(|item| item.layer == layer)
        .map(|item| {
            let offset = [item.offset[0] as f32, item.offset[1] as f32];
            if offset.iter().any(|value| !value.is_finite()) {
                return Err(format!("scene item {} offset exceeds f32", item.id));
            }
            Ok((
                item.order,
                item.id.clone(),
                PlacedSprite {
                    family: item.id.clone(),
                    pass: if layer == "ground" {
                        TerrainPass::Ground
                    } else {
                        TerrainPass::World
                    },
                    anchor: item.anchor,
                    offset,
                    baseline: 0.0,
                    family_order: 0,
                    local_order: 0,
                    frames: SpriteFrames {
                        assets: item.frames.clone(),
                        frame_ms: item.frame_ms,
                        phase_ms: item.phase_ms,
                    },
                    clip_hexes: item.clips.clone(),
                    image_mods: ImageModifiers {
                        crop: item.crop,
                        masks: item.masks.clone(),
                        opacity: item.opacity,
                    },
                },
                Color::new(
                    item.tint[0] as f32,
                    item.tint[1] as f32,
                    item.tint[2] as f32,
                    item.tint[3] as f32,
                ),
            ))
        })
        .collect()
}

fn clip_to_rect(mut points: Vec<Vec2>, min: Vec2, max: Vec2) -> Vec<Vec2> {
    for (axis, edge, sign) in [
        (0, min.x, 1.0),
        (0, max.x, -1.0),
        (1, min.y, 1.0),
        (1, max.y, -1.0),
    ] {
        let mut clipped = Vec::new();
        if let Some(mut previous) = points.last().copied() {
            for current in points {
                let previous_distance = (previous[axis] - edge) * sign;
                let distance = (current[axis] - edge) * sign;
                if (previous_distance >= 0.0) != (distance >= 0.0) {
                    clipped.push(
                        previous.lerp(current, previous_distance / (previous_distance - distance)),
                    );
                }
                if distance >= 0.0 {
                    clipped.push(current);
                }
                previous = current;
            }
        }
        points = clipped;
    }
    points
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
    images: &BTreeMap<String, Image>,
    textures: &mut Vec<Texture2D>,
    cache: &mut BTreeMap<(String, ImageModifiers), usize>,
) -> Result<Vec<DrawCommand>, String> {
    sprites
        .iter()
        .map(|sprite| {
            let mut frames = Vec::new();
            for asset in &sprite.frames.assets {
                let key = (asset.clone(), sprite.image_mods.clone());
                let texture = if let Some(index) = cache.get(&key) {
                    *index
                } else {
                    let source = images
                        .get(asset)
                        .ok_or_else(|| format!("missing scene asset: {asset}"))?;
                    let image = modify_image(source, &sprite.image_mods, images)?;
                    let texture = Texture2D::from_image(&image);
                    texture.set_filter(FilterMode::Nearest);
                    let index = textures.len();
                    textures.push(texture);
                    cache.insert(key, index);
                    index
                };
                frames.push(Frame {
                    texture,
                    size: vec2(textures[texture].width(), textures[texture].height())
                        / REFERENCE_RADIUS,
                });
            }
            let frames = if frames.len() == 1 {
                PreparedFrames::Still(frames.remove(0))
            } else {
                PreparedFrames::Animated {
                    frames,
                    frame_ms: sprite.frames.frame_ms,
                    phase_ms: sprite.frames.phase_ms,
                }
            };
            Ok(DrawCommand {
                position: hex_center(sprite.anchor.x, sprite.anchor.y)
                    + vec2(sprite.offset[0], sprite.offset[1]) / REFERENCE_RADIUS,
                frames,
                clip_hexes: sprite
                    .clip_hexes
                    .iter()
                    .map(|p| hex_center(p.x, p.y))
                    .collect(),
                tint: WHITE,
            })
        })
        .collect()
}

fn modify_image(
    source: &Image,
    mods: &ImageModifiers,
    images: &BTreeMap<String, Image>,
) -> Result<Image, String> {
    let [x, y, width, height] =
        mods.crop
            .unwrap_or([0, 0, source.width as u32, source.height as u32]);
    if x.checked_add(width)
        .is_none_or(|right| right > source.width as u32)
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > source.height as u32)
    {
        return Err("sprite crop extends outside the source image".into());
    }
    let masks = mods
        .masks
        .iter()
        .map(|id| images.get(id).ok_or_else(|| format!("missing mask: {id}")))
        .collect::<Result<Vec<_>, _>>()?;
    if masks
        .iter()
        .any(|mask| mask.width as u32 != width || mask.height as u32 != height)
    {
        return Err("sprite mask dimensions do not match cropped image".into());
    }
    let mut bytes = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (((y + row) * source.width as u32 + x) * 4) as usize;
        bytes.extend_from_slice(&source.bytes[start..start + width as usize * 4]);
    }
    for (i, pixel) in bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        // WML ~BLIT unions masks with source-over alpha; ~MASK intersects
        // with the source alpha, then ~O scales the resulting opacity.
        let mut alpha = if masks.is_empty() { 255u32 } else { 0 };
        for mask in &masks {
            let next = mask.bytes[i * 4 + 3] as u32;
            alpha = next + alpha * (255 - next) / 255;
        }
        pixel[3] = (u32::from(pixel[3]).min(alpha) * u32::from(mods.opacity) / 255) as u8;
    }
    Ok(Image {
        bytes,
        width: width as u16,
        height: height as u16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_registry_is_read_from_the_view() {
        let registry = Value::Map(BTreeMap::from([
            ("schema".into(), Value::String("assets".into())),
            (
                "items".into(),
                Value::List(vec![Value::Map(BTreeMap::from([
                    ("id".into(), Value::String("terrain/grass".into())),
                    ("path".into(), Value::String("assets/grass.png".into())),
                ]))]),
            ),
        ]));
        assert_eq!(
            asset_registry(&registry).unwrap()["terrain/grass"],
            "assets/grass.png"
        );
    }

    #[test]
    fn hit_regions_follow_the_same_hex_geometry_as_rendering() {
        let regions = vec![
            (
                "cell:1:1".into(),
                wesnoth_engine::engine::Position { x: 1, y: 1 },
            ),
            (
                "cell:2:1".into(),
                wesnoth_engine::engine::Position { x: 2, y: 1 },
            ),
        ];
        assert_eq!(
            hit_region_at(&regions, hex_center(2, 1)).map(|region| region.0.as_str()),
            Some("cell:2:1")
        );
        assert!(hit_region_at(&regions, vec2(100.0, 100.0)).is_none());
    }

    #[test]
    fn image_operations_follow_wml_crop_mask_and_opacity_order() {
        let source = Image {
            width: 2,
            height: 1,
            bytes: vec![10, 20, 30, 255, 40, 50, 60, 200],
        };
        let images = BTreeMap::from([
            (
                "a".into(),
                Image {
                    width: 1,
                    height: 1,
                    bytes: vec![0, 0, 0, 128],
                },
            ),
            (
                "b".into(),
                Image {
                    width: 1,
                    height: 1,
                    bytes: vec![0, 0, 0, 128],
                },
            ),
        ]);
        let mut mods = ImageModifiers {
            crop: Some([1, 0, 1, 1]),
            masks: vec!["a".into(), "b".into()],
            opacity: 127,
        };
        assert_eq!(
            modify_image(&source, &mods, &images).unwrap().bytes,
            [40, 50, 60, 95]
        );
        mods.crop = Some([2, 0, 1, 1]);
        assert!(modify_image(&source, &mods, &images).is_err());
    }

    #[test]
    fn campaign_water_frames_and_masks_can_be_prepared_without_a_gpu() {
        use wesnoth_engine::{adventure::Adventure, game};
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = std::fs::read_to_string(root.join("scripts/adventures/two_brothers.wml"))
            .unwrap();
        let adventure = Adventure::parse(&manifest).unwrap();
        let mut checked = std::collections::BTreeSet::new();
        for chapter in &adventure.scenarios {
            let resources = game::load_adventure_resources(
                root.join("scripts"),
                &adventure,
                chapter,
                None,
            )
            .unwrap();
            let game = game::Game::start(resources).unwrap();
            let scene = TerrainScene::from_lua(game.map(), game.terrain_scripts()).unwrap();
            let images: BTreeMap<_, _> = scene
                .assets()
                .iter()
                .map(|(id, path)| {
                    let bytes = std::fs::read(root.join(path)).unwrap();
                    (
                        id.clone(),
                        Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).unwrap(),
                    )
                })
                .collect();
            for sprite in scene.ground().iter().filter(|s| s.family == "water") {
                for asset in &sprite.frames.assets {
                    if checked.insert((asset.clone(), sprite.image_mods.clone())) {
                        let image =
                            modify_image(&images[asset], &sprite.image_mods, &images).unwrap();
                        assert!(!image.bytes.is_empty());
                    }
                }
            }
        }
        assert!(checked.len() > 38);
    }

    #[test]
    fn water_atlas_stays_continuous_across_the_clients_staggered_columns() {
        use wesnoth_engine::{
            engine::Map,
            terrain_scene::{TerrainScene, TerrainScript},
        };
        let scene = TerrainScene::from_lua(
            &Map {
                width: 12,
                height: 4,
                cells: vec!["Ww".into(); 48],
            },
            &[TerrainScript {
                family: "water".into(),
                codes: vec!["Ww".into()],
                source: include_str!("../../../scripts/terrain/water.lua").into(),
            }],
        )
        .unwrap();
        let mut crops = std::collections::BTreeSet::new();
        for sprite in scene.ground().iter().filter(|s| s.local_order == -999) {
            let crop = sprite.image_mods.crop.unwrap();
            crops.insert(crop);
            let center = hex_center(sprite.anchor.x, sprite.anchor.y) * REFERENCE_RADIUS;
            // The original sheet repeats every 324x144 pixels, with an 18x36
            // overlap. Adjacent hexes must sample the same continuous plane.
            assert_eq!((crop[0] as i64 - center.x as i64).rem_euclid(324), 0);
            assert_eq!((crop[1] as i64 - center.y as i64).rem_euclid(144), 0);
            assert_eq!(&crop[2..], &[72, 72]);
        }
        assert_eq!(crops.len(), 12);
    }

    #[test]
    fn clip_padding_cannot_sample_outside_the_texture() {
        let polygon = vec![
            vec2(-2.0, 0.5),
            vec2(0.5, -2.0),
            vec2(3.0, 0.5),
            vec2(0.5, 3.0),
        ];
        let clipped = clip_to_rect(polygon, Vec2::ZERO, Vec2::ONE);
        assert_eq!(clipped.len(), 4);
        assert!(
            clipped
                .iter()
                .all(|p| p.x >= 0.0 && p.y >= 0.0 && p.x <= 1.0 && p.y <= 1.0)
        );
        assert!(
            clip_to_rect(
                vec![vec2(2.0, 2.0), vec2(3.0, 2.0), vec2(2.0, 3.0)],
                Vec2::ZERO,
                Vec2::ONE
            )
            .is_empty()
        );
    }

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
