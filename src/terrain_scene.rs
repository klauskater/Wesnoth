use std::collections::BTreeMap;

use mlua::{Lua, LuaOptions, StdLib, Table};

use crate::{
    engine::{Map, Position},
    terrain::visual_codes,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerrainScript {
    pub family: String,
    pub codes: Vec<String>,
    pub source: String,
}

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
    pub clip_hexes: Vec<Position>,
    pub image_mods: ImageModifiers,
}

/// WML image operations, applied once when preparing textures, before drawing.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImageModifiers {
    pub crop: Option<[u32; 4]>,
    pub masks: Vec<String>,
    pub opacity: u8,
}

impl Default for ImageModifiers {
    fn default() -> Self {
        Self {
            crop: None,
            masks: Vec::new(),
            opacity: 255,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerrainScene {
    assets: BTreeMap<String, String>,
    ground: Vec<PlacedSprite>,
    world: Vec<PlacedSprite>,
}

impl TerrainScene {
    pub fn from_lua(map: &Map, scripts: &[TerrainScript]) -> Result<Self, String> {
        let mut assets = Vec::new();
        let mut sprites = Vec::new();
        for (family_order, script) in scripts.iter().enumerate() {
            let family_order = i16::try_from(family_order)
                .map_err(|_| "too many terrain render scripts".to_owned())?;
            run_script(map, script, family_order, &mut assets, &mut sprites)?;
        }
        Self::compile(assets, sprites)
    }

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
            for mask in &sprite.image_mods.masks {
                if !asset_paths.contains_key(mask) {
                    return Err(format!("unknown sprite mask: {mask}"));
                }
            }
            match sprite.pass {
                TerrainPass::Ground => ground.push(sprite),
                TerrainPass::World => world.push(sprite),
            }
        }
        ground.sort_by_key(|sprite| {
            (
                sprite.local_order,
                sprite.family_order,
                sprite.anchor.y,
                sprite.anchor.x,
            )
        });
        world.sort_by(|left, right| {
            left.local_order
                .cmp(&right.local_order)
                .then_with(|| world_depth(left).total_cmp(&world_depth(right)))
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

fn run_script(
    map: &Map,
    script: &TerrainScript,
    family_order: i16,
    assets: &mut Vec<SpriteAsset>,
    sprites: &mut Vec<PlacedSprite>,
) -> Result<(), String> {
    let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())
        .map_err(|error| error.to_string())?;
    for unsafe_global in ["io", "os", "package", "dofile", "loadfile", "require"] {
        lua.globals()
            .set(unsafe_global, mlua::Value::Nil)
            .map_err(|error| error.to_string())?;
    }
    let render: mlua::Function = lua
        .load(&script.source)
        .set_name(&script.family)
        .eval()
        .map_err(|error| format!("terrain renderer {}: {error}", script.family))?;
    let output: Table = render
        .call(map_table(&lua, map, &script.codes).map_err(|error| error.to_string())?)
        .map_err(|error| format!("terrain renderer {}: {error}", script.family))?;

    let asset_table: Table = output
        .get("assets")
        .map_err(|error| format!("terrain renderer {} assets: {error}", script.family))?;
    for pair in asset_table.pairs::<String, String>() {
        let (id, path) =
            pair.map_err(|error| format!("terrain renderer {} assets: {error}", script.family))?;
        assets.push(SpriteAsset {
            id: asset_id(&script.family, &id),
            path,
        });
    }

    read_commands(output.get("static"), false, script, family_order, sprites)?;
    read_commands(output.get("animated"), true, script, family_order, sprites)?;
    Ok(())
}

fn map_table(lua: &Lua, map: &Map, codes: &[String]) -> mlua::Result<Table> {
    let result = lua.create_table()?;
    result.set("width", map.width)?;
    result.set("height", map.height)?;

    let rows = lua.create_table()?;
    let tiles = lua.create_table()?;
    let mut family_index = 1;
    for y in 1..=map.height {
        let row = lua.create_table()?;
        for x in 1..=map.width {
            let code = &map.cells[(y - 1) * map.width + x - 1];
            row.set(x, code.as_str())?;
            let (base, overlay) = visual_codes(code);
            if let Some(terrain) = matching_code(codes, base, overlay) {
                let tile = lua.create_table()?;
                tile.set("x", x)?;
                tile.set("y", y)?;
                tile.set("code", code.as_str())?;
                tile.set("terrain", terrain)?;
                tiles.set(family_index, tile)?;
                family_index += 1;
            }
        }
        rows.set(y, row)?;
    }
    result.set("cells", rows)?;
    result.set("tiles", tiles)?;
    Ok(result)
}

fn matching_code<'a>(codes: &[String], base: &'a str, overlay: &'a str) -> Option<&'a str> {
    [base, overlay]
        .into_iter()
        .find(|code| !code.is_empty() && codes.iter().any(|candidate| candidate == code))
}

fn read_commands(
    table: mlua::Result<Option<Table>>,
    animated: bool,
    script: &TerrainScript,
    family_order: i16,
    sprites: &mut Vec<PlacedSprite>,
) -> Result<(), String> {
    let Some(table) =
        table.map_err(|error| format!("terrain renderer {} commands: {error}", script.family))?
    else {
        return Ok(());
    };
    for command in table.sequence_values::<Table>() {
        let command = command
            .map_err(|error| format!("terrain renderer {} command: {error}", script.family))?;
        sprites.push(read_command(
            &command,
            animated,
            &script.family,
            family_order,
        )?);
    }
    Ok(())
}

fn read_command(
    command: &Table,
    animated: bool,
    family: &str,
    family_order: i16,
) -> Result<PlacedSprite, String> {
    let get = |name| {
        command
            .get::<i64>(name)
            .map_err(|error| format!("terrain renderer {family} command.{name}: {error}"))
    };
    let pass = match command
        .get::<Option<String>>("pass")
        .map_err(|error| format!("terrain renderer {family} command.pass: {error}"))?
        .as_deref()
        .unwrap_or("ground")
    {
        "ground" => TerrainPass::Ground,
        "world" => TerrainPass::World,
        other => return Err(format!("terrain renderer {family}: invalid pass {other}")),
    };
    let frames = if animated {
        let values: Table = command
            .get("frames")
            .map_err(|error| format!("terrain renderer {family} command.frames: {error}"))?;
        let assets = values
            .sequence_values::<String>()
            .map(|value| value.map(|id| asset_id(family, &id)))
            .collect::<mlua::Result<Vec<_>>>()
            .map_err(|error| format!("terrain renderer {family} command.frames: {error}"))?;
        if assets.len() < 2 {
            return Err(format!(
                "terrain renderer {family}: animated command needs at least two frames"
            ));
        }
        SpriteFrames {
            assets,
            frame_ms: u32::try_from(get("frame_ms")?)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("terrain renderer {family}: invalid frame_ms"))?,
            phase_ms: u32::try_from(optional_i64(command, family, "phase_ms")?.unwrap_or(0))
                .map_err(|_| format!("terrain renderer {family}: invalid phase_ms"))?,
        }
    } else {
        let asset: String = command
            .get("asset")
            .map_err(|error| format!("terrain renderer {family} command.asset: {error}"))?;
        SpriteFrames::still(asset_id(family, &asset))
    };
    let clip_hexes = command
        .get::<Option<Table>>("clips")
        .map_err(|error| format!("terrain renderer {family} command.clips: {error}"))?
        .map(|clips| {
            clips
                .sequence_values::<Table>()
                .map(|clip| {
                    let clip = clip.map_err(|error| {
                        format!("terrain renderer {family} command.clips: {error}")
                    })?;
                    Ok(Position {
                        x: clip.get("x").map_err(|error| {
                            format!("terrain renderer {family} command.clips.x: {error}")
                        })?,
                        y: clip.get("y").map_err(|error| {
                            format!("terrain renderer {family} command.clips.y: {error}")
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    let crop = command
        .get::<Option<Vec<u32>>>("crop")
        .map_err(|error| error.to_string())?
        .map(|values| {
            <[u32; 4]>::try_from(values).map_err(|_| "crop needs x,y,width,height".to_owned())
        })
        .transpose()?;
    if crop.is_some_and(|rect| rect[2] == 0 || rect[3] == 0) {
        return Err("crop dimensions must be positive".into());
    }
    let masks = command
        .get::<Option<Vec<String>>>("masks")
        .map_err(|error| error.to_string())?
        .unwrap_or_default()
        .into_iter()
        .map(|id| asset_id(family, &id))
        .collect();
    let opacity = command
        .get::<Option<u8>>("opacity")
        .map_err(|error| error.to_string())?
        .unwrap_or(255);
    Ok(PlacedSprite {
        family: family.into(),
        pass,
        anchor: Position {
            x: get("x")?,
            y: get("y")?,
        },
        offset: [
            optional_f32(command, family, "offset_x")?.unwrap_or(0.0),
            optional_f32(command, family, "offset_y")?.unwrap_or(0.0),
        ],
        baseline: optional_f32(command, family, "baseline")?.unwrap_or(0.0),
        family_order,
        local_order: i16::try_from(optional_i64(command, family, "order")?.unwrap_or(0))
            .map_err(|_| format!("terrain renderer {family}: invalid order"))?,
        frames,
        clip_hexes,
        image_mods: ImageModifiers {
            crop,
            masks,
            opacity,
        },
    })
}

fn optional_i64(command: &Table, family: &str, name: &str) -> Result<Option<i64>, String> {
    command
        .get(name)
        .map_err(|error| format!("terrain renderer {family} command.{name}: {error}"))
}

fn optional_f32(command: &Table, family: &str, name: &str) -> Result<Option<f32>, String> {
    command
        .get(name)
        .map_err(|error| format!("terrain renderer {family} command.{name}: {error}"))
}

fn asset_id(family: &str, id: &str) -> String {
    format!("{family}:{id}")
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
            image_mods: ImageModifiers::default(),
            pass,
            anchor: Position { x: 1, y: 1 },
            offset: [0.0, 0.0],
            baseline: 0.0,
            family_order,
            local_order,
            frames: SpriteFrames::still("tile"),
            clip_hexes: Vec::new(),
        }
    }

    #[test]
    fn validates_assets_and_orders_ground_by_layer_before_family() {
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
        assert_eq!(scene.ground()[0].local_order, 0);
        assert_eq!(scene.ground()[0].family_order, 1);
        assert_eq!(scene.ground()[1].local_order, 2);
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

    #[test]
    fn lua_renderer_receives_only_its_family_and_marks_animation() {
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Gg".into(), "Ww^Bw|".into()],
        };
        let scene = TerrainScene::from_lua(
            &map,
            &[TerrainScript {
                family: "grassland".into(),
                codes: vec!["Gg".into()],
                source: r#"
                    return function(map)
                        local static = {}
                        for _, tile in ipairs(map.tiles) do
                            static[#static + 1] = {
                                asset = "base", x = tile.x, y = tile.y
                            }
                        end
                        return {
                            assets = { base = "grass.png", a = "a.png", b = "b.png" },
                            static = static,
                            animated = {{ frames = {"a", "b"}, frame_ms = 80, x = 1, y = 1 }}
                        }
                    end
                "#
                .into(),
            }],
        )
        .unwrap();
        assert_eq!(scene.ground().len(), 2);
        assert_eq!(scene.ground()[0].anchor, Position { x: 1, y: 1 });
        assert_eq!(scene.ground()[0].frames.assets, ["grassland:base"]);
        assert_eq!(
            scene.ground()[1].frames.assets,
            ["grassland:a", "grassland:b"]
        );
        assert_eq!(scene.assets()["grassland:base"], "grass.png");
    }

    #[test]
    fn grass_renderer_crosses_the_boundary_only_toward_dirt() {
        let map = Map {
            width: 2,
            height: 2,
            cells: vec!["Re".into(), "Gg".into(), "Gg".into(), "Re".into()],
        };
        let scene = TerrainScene::from_lua(
            &map,
            &[
                TerrainScript {
                    family: "dirt".into(),
                    codes: vec!["Re".into()],
                    source: include_str!("../scripts/terrain/dirt.lua").into(),
                },
                TerrainScript {
                    family: "grassland".into(),
                    codes: vec!["Gg".into()],
                    source: include_str!("../scripts/terrain/grass.lua").into(),
                },
            ],
        )
        .unwrap();
        assert!(scene.ground().iter().any(|sprite| {
            sprite.anchor == Position { x: 1, y: 1 }
                && sprite.frames.assets == ["grassland:green-medium-ne"]
        }));
        assert!(
            !scene
                .ground()
                .iter()
                .any(|sprite| sprite.frames.assets[0].starts_with("dirt:dirt-"))
        );
    }

    #[test]
    fn grass_renderer_uses_each_original_subtype_and_transition_priority() {
        let map = Map {
            width: 4,
            height: 1,
            cells: vec!["Gg".into(), "Gs".into(), "Gd".into(), "Gll".into()],
        };
        let scene = TerrainScene::from_lua(
            &map,
            &[TerrainScript {
                family: "grassland".into(),
                codes: vec!["Gg".into(), "Gs".into(), "Gd".into(), "Gll".into()],
                source: include_str!("../scripts/terrain/grass.lua").into(),
            }],
        )
        .unwrap();

        for stem in ["green", "semi-dry", "dry", "leaf-litter"] {
            assert!(scene.ground().iter().any(|sprite| {
                sprite.local_order == -1000
                    && sprite.frames.assets[0].starts_with(&format!("grassland:{stem}"))
            }));
        }
        for (order, stem) in [
            (-250, "semi-dry-long"),
            (-252, "dry-long"),
            (-254, "leaf-litter-long"),
            (-255, "dry-long"),
            (-256, "green-long"),
        ] {
            assert!(scene.ground().iter().any(|sprite| {
                sprite.local_order == order
                    && sprite.frames.assets[0].starts_with(&format!("grassland:{stem}-"))
            }));
        }
    }

    #[test]
    fn forest_renderer_uses_dense_sparse_small_and_great_tree_rules() {
        let render = |center: &str, north: &str| {
            let mut cells = vec!["Gg".to_owned(); 9];
            cells[1] = north.to_owned();
            cells[4] = center.to_owned();
            TerrainScene::from_lua(
                &Map {
                    width: 3,
                    height: 3,
                    cells,
                },
                &[TerrainScript {
                    family: "forest".into(),
                    codes: vec![
                        "Fp".into(),
                        "Fds".into(),
                        "Fdw".into(),
                        "Fms".into(),
                        "Fmw".into(),
                        "Fet".into(),
                    ],
                    source: include_str!("../scripts/terrain/forest.lua").into(),
                }],
            )
            .unwrap()
        };

        let dense = render("Gs^Fms", "Gg");
        assert_eq!(dense.world().len(), 1);
        assert!(dense.world()[0].frames.assets[0].starts_with("forest:mixed-summer"));
        assert!(!dense.world()[0].frames.assets[0].contains("small"));

        let sparse = render("Hh^Fp", "Gg");
        assert!(sparse.world()[0].frames.assets[0].starts_with("forest:pine-sparse"));
        assert!(!sparse.world()[0].frames.assets[0].contains("small"));

        let small = render("Gs^Fds", "Ww");
        assert!(small.world()[0].frames.assets[0].starts_with("forest:deciduous-summer-small"));

        let great = render("Gg^Fet", "Gg");
        assert!(great.world()[0].frames.assets[0].starts_with("forest:great-tree"));
        assert_eq!(great.world()[0].offset, [-36.0, -86.0]);
    }

    #[test]
    fn road_renderer_uses_original_bases_and_keeps_grass_above_road_edges() {
        let map = Map {
            width: 4,
            height: 1,
            cells: vec!["Gg".into(), "Rr".into(), "Rp".into(), "Rd".into()],
        };
        let scene = TerrainScene::from_lua(
            &map,
            &[
                TerrainScript {
                    family: "road".into(),
                    codes: vec!["Rd".into(), "Rr".into(), "Rp".into()],
                    source: include_str!("../scripts/terrain/road.lua").into(),
                },
                TerrainScript {
                    family: "grassland".into(),
                    codes: vec!["Gg".into()],
                    source: include_str!("../scripts/terrain/grass.lua").into(),
                },
            ],
        )
        .unwrap();

        for stem in ["road", "stone-path", "desert-road"] {
            assert!(scene.ground().iter().any(|sprite| {
                sprite.local_order == -1000
                    && sprite.frames.assets[0].starts_with(&format!("road:{stem}"))
            }));
        }
        assert!(!scene.ground().iter().any(|sprite| {
            sprite.anchor == Position { x: 1, y: 1 } && sprite.local_order != -1000
        }));
        assert!(
            scene
                .ground()
                .iter()
                .any(|sprite| sprite.local_order == -320)
        );
        assert!(
            scene
                .ground()
                .iter()
                .any(|sprite| sprite.local_order == -322)
        );
        assert!(
            scene
                .ground()
                .iter()
                .any(|sprite| sprite.local_order == -370)
        );
        assert!(scene.ground().iter().any(|sprite| {
            sprite.anchor == Position { x: 2, y: 1 }
                && sprite.frames.assets[0].starts_with("grassland:green-medium-")
        }));
    }
}
