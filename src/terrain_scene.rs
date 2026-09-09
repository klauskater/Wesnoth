use std::collections::BTreeMap;

use crate::engine::Position;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainPass {
    Ground,
    World,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteAsset {
    pub id: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteFrames {
    pub assets: Vec<String>,
    pub frame_ms: u32,
    pub phase_ms: u32,
}

impl SpriteFrames {
    pub fn still(asset: impl Into<String>) -> Self {
        Self {
            assets: vec![asset.into()],
            frame_ms: 1,
            phase_ms: 0,
        }
    }

    pub fn asset_at(&self, elapsed_ms: u64) -> &str {
        let frame = ((elapsed_ms + u64::from(self.phase_ms)) / u64::from(self.frame_ms))
            % self.assets.len() as u64;
        &self.assets[frame as usize]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedSprite {
    pub family: String,
    pub pass: TerrainPass,
    pub anchor: Position,
    /// Pixel offset from the center of a 72px-high reference hex.
    pub offset: [f32; 2],
    pub baseline: f32,
    pub family_order: i16,
    pub local_order: i16,
    pub frames: SpriteFrames,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerrainScene {
    assets: BTreeMap<String, String>,
    ground: Vec<PlacedSprite>,
    world: Vec<PlacedSprite>,
}

impl TerrainScene {
    pub fn compile(
        assets: impl IntoIterator<Item = SpriteAsset>,
        sprites: impl IntoIterator<Item = PlacedSprite>,
    ) -> Result<Self, String> {
        let mut asset_paths = BTreeMap::new();
        for asset in assets {
            if asset_paths.insert(asset.id.clone(), asset.path).is_some() {
                return Err(format!("duplicate sprite asset: {}", asset.id));
            }
        }

        let mut ground = Vec::new();
        let mut world = Vec::new();
        for sprite in sprites {
            if sprite.frames.assets.is_empty() || sprite.frames.frame_ms == 0 {
                return Err(format!(
                    "invalid frames for terrain family {}",
                    sprite.family
                ));
            }
            if let Some(missing) = sprite
                .frames
                .assets
                .iter()
                .find(|asset| !asset_paths.contains_key(*asset))
            {
                return Err(format!("unknown sprite asset: {missing}"));
            }
            match sprite.pass {
                TerrainPass::Ground => ground.push(sprite),
                TerrainPass::World => world.push(sprite),
            }
        }
        ground.sort_by_key(|sprite| {
            (
                sprite.family_order,
                sprite.local_order,
                sprite.anchor.y,
                sprite.anchor.x,
            )
        });
        world.sort_by(|left, right| {
            world_depth(left)
                .total_cmp(&world_depth(right))
                .then(left.family_order.cmp(&right.family_order))
                .then(left.local_order.cmp(&right.local_order))
                .then(left.anchor.x.cmp(&right.anchor.x))
        });
        Ok(Self {
            assets: asset_paths,
            ground,
            world,
        })
    }

    pub fn assets(&self) -> &BTreeMap<String, String> {
        &self.assets
    }

    pub fn ground(&self) -> &[PlacedSprite] {
        &self.ground
    }

    pub fn world(&self) -> &[PlacedSprite] {
        &self.world
    }
}

fn world_depth(sprite: &PlacedSprite) -> f32 {
    let hex_y =
        (sprite.anchor.y - 1) as f32 * 72.0 - if sprite.anchor.x % 2 == 0 { 36.0 } else { 0.0 };
    hex_y + sprite.baseline
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite(pass: TerrainPass, family_order: i16, local_order: i16) -> PlacedSprite {
        PlacedSprite {
            family: "test".into(),
            pass,
            anchor: Position { x: 1, y: 1 },
            offset: [0.0, 0.0],
            baseline: 0.0,
            family_order,
            local_order,
            frames: SpriteFrames::still("tile"),
        }
    }

    #[test]
    fn validates_assets_and_orders_ground_within_families() {
        let mut front = sprite(TerrainPass::World, 0, 0);
        front.anchor.y = 2;
        let scene = TerrainScene::compile(
            [SpriteAsset {
                id: "tile".into(),
                path: "tile.png".into(),
            }],
            [
                front,
                sprite(TerrainPass::World, 0, 0),
                sprite(TerrainPass::Ground, 1, 0),
                sprite(TerrainPass::Ground, 0, 2),
            ],
        )
        .unwrap();
        assert_eq!(scene.ground()[0].local_order, 2);
        assert_eq!(scene.world()[0].anchor.y, 1);
        assert_eq!(scene.world()[1].anchor.y, 2);
        assert_eq!(
            SpriteFrames {
                assets: vec!["a".into(), "b".into()],
                frame_ms: 100,
                phase_ms: 0
            }
            .asset_at(100),
            "b"
        );
    }
}
