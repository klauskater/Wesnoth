use std::path::{Path, PathBuf};
use wesnoth_engine::{
    adventure::Adventure,
    game::{self, Game},
    value::Value,
};

fn scripts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts")
}

fn cell_code(value: &Value) -> &str {
    value.get("terrain").and_then(Value::as_str).unwrap()
}

fn campaign_game(chapter: usize) -> Game {
    let scripts = scripts();
    let manifest = std::fs::read_to_string(scripts.join("adventures/two_brothers.wml")).unwrap();
    let adventure = Adventure::parse(&manifest).unwrap();
    Game::start(
        game::load_adventure_resources(
            scripts,
            &adventure,
            &adventure.scenarios[chapter],
            None,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn loads_first_battle() {
    let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
    assert_eq!(game.snapshot().unwrap().map.cells.len(), 48);
    assert_eq!(
        game.map_tiles().get("grassland").unwrap().color,
        [111, 145, 77]
    );
    let terrain =
        wesnoth_engine::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
            .unwrap();
    let grass_count = game
        .map()
        .cells
        .iter()
        .filter(|code| {
            matches!(
                wesnoth_engine::terrain::visual_codes(cell_code(code)).0,
                "grassland" | "Gg"
            )
        })
        .count();
    assert_eq!(
        terrain
            .ground()
            .iter()
            .filter(|sprite| { sprite.family == "grassland" && sprite.local_order == -1000 })
            .count(),
        grass_count
    );
    let forest_count = game
        .map()
        .cells
        .iter()
        .filter(|code| cell_code(code) == "forest")
        .count();
    assert_eq!(
        terrain
            .world()
            .iter()
            .filter(|sprite| sprite.family == "forest")
            .count(),
        forest_count
    );
    assert!(
        terrain
            .world()
            .iter()
            .all(|sprite| sprite.frames.assets[0].starts_with("forest:mixed-summer"))
    );
    assert!(
        terrain
            .ground()
            .iter()
            .filter(|sprite| sprite.local_order == -1000)
            .map(|sprite| &sprite.frames.assets[0])
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1
    );
    assert_eq!(
        terrain,
        wesnoth_engine::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
            .unwrap()
    );
    assert_eq!(game.dialog(&game.start_dialog).unwrap().len(), 2);
    assert_eq!(game.revision(), 1);
}

#[test]
fn loads_dirt_bases_and_sprite_based_grass_transitions() {
    let game = campaign_game(0);
    let terrain =
        wesnoth_engine::terrain_scene::TerrainScene::from_lua(game.map(), game.terrain_scripts())
            .unwrap();
    let dirt_count = game
        .map()
        .cells
        .iter()
        .filter(|code| wesnoth_engine::terrain::visual_codes(cell_code(code)).0 == "Re")
        .count();
    assert_eq!(
        terrain
            .ground()
            .iter()
            .filter(|sprite| { sprite.family == "dirt" && sprite.local_order == -1000 })
            .count(),
        dirt_count
    );
    let grassland_count = game
        .map()
        .cells
        .iter()
        .filter(|code| {
            matches!(
                wesnoth_engine::terrain::visual_codes(cell_code(code)).0,
                "grassland" | "Gg" | "Gs" | "Gd" | "Gll"
            )
        })
        .count();
    assert_eq!(
        terrain
            .ground()
            .iter()
            .filter(|sprite| { sprite.family == "grassland" && sprite.local_order == -1000 })
            .count(),
        grassland_count
    );
    assert!(terrain.ground().iter().any(|sprite| {
        sprite.family == "grassland" && sprite.frames.assets[0].starts_with("grassland:green-")
    }));
    assert!(
        !terrain
            .ground()
            .iter()
            .any(|sprite| sprite.frames.assets[0].starts_with("dirt:dirt-"))
    );
    let missing_assets = terrain
        .assets()
        .values()
        .filter(|path| !Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file())
        .collect::<Vec<_>>();
    assert!(missing_assets.is_empty(), "{missing_assets:?}");
}

#[test]
fn loads_original_road_assets_for_each_campaign_map() {
    for (chapter, scenario) in [
        "scenarios/rooting_out_a_mage.wml",
        "scenarios/the_chase.wml",
        "scenarios/guarded_castle.wml",
        "scenarios/return_to_the_village.wml",
    ]
    .into_iter()
    .enumerate()
    {
        let game = campaign_game(chapter);
        assert_eq!(game.terrain_scripts()[0].family, "road");
        let terrain = wesnoth_engine::terrain_scene::TerrainScene::from_lua(
            game.map(),
            game.terrain_scripts(),
        )
        .unwrap();
        let road_count = game
            .map()
            .cells
            .iter()
            .filter(|code| {
                matches!(
                    wesnoth_engine::terrain::visual_codes(cell_code(code)).0,
                    "Rd" | "Rr" | "Rp"
                )
            })
            .count();
        assert_eq!(
            terrain
                .ground()
                .iter()
                .filter(|sprite| sprite.family == "road" && sprite.local_order == -1000)
                .count(),
            road_count,
            "{scenario}"
        );
        assert!(terrain.assets().iter().all(|(id, path)| {
            !id.starts_with("road:") || Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file()
        }));
    }
}

#[test]
fn loads_original_decorations_for_each_campaign_map() {
    let decoration_codes = ["Efm", "Gvs", "Es", "Em", "Edb", "Eff", "Wm"];
    for (chapter, scenario) in [
        "scenarios/rooting_out_a_mage.wml",
        "scenarios/the_chase.wml",
        "scenarios/guarded_castle.wml",
        "scenarios/return_to_the_village.wml",
    ]
    .into_iter()
    .enumerate()
    {
        let game = campaign_game(chapter);
        let terrain = wesnoth_engine::terrain_scene::TerrainScene::from_lua(
            game.map(),
            game.terrain_scripts(),
        )
        .unwrap();
        let decoration_sprites = terrain
            .ground()
            .iter()
            .filter(|sprite| sprite.family == "decorations")
            .collect::<Vec<_>>();

        for (index, code) in game.map().cells.iter().enumerate() {
            let (_, overlay) = wesnoth_engine::terrain::visual_codes(cell_code(code));
            if decoration_codes.contains(&overlay) {
                let position = wesnoth_engine::engine::Position {
                    x: (index % game.map().width + 1) as i64,
                    y: (index / game.map().width + 1) as i64,
                };
                assert!(
                    decoration_sprites
                        .iter()
                        .any(|sprite| sprite.anchor == position),
                    "{scenario}: no decoration sprite for {} at {position:?}",
                    cell_code(code)
                );
            }
        }

        assert!(decoration_sprites.iter().all(|sprite| {
            sprite.frames.assets.iter().all(|asset| {
                let path = terrain.assets().get(asset).unwrap();
                Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file()
            })
        }));
        for windmill in decoration_sprites
            .iter()
            .filter(|sprite| sprite.frames.assets[0].starts_with("decorations:windmill-"))
        {
            assert_eq!(windmill.frames.assets.len(), 18);
            assert!(matches!(windmill.frames.frame_ms, 30 | 50));
        }
    }
}

#[test]
fn loads_original_hills_mountains_and_peaks_for_each_campaign_map() {
    let mut checked_long_ranges = 0;
    for (chapter, scenario) in [
        "scenarios/rooting_out_a_mage.wml",
        "scenarios/the_chase.wml",
        "scenarios/guarded_castle.wml",
        "scenarios/return_to_the_village.wml",
    ]
    .into_iter()
    .enumerate()
    {
        let game = campaign_game(chapter);
        let terrain = wesnoth_engine::terrain_scene::TerrainScene::from_lua(
            game.map(),
            game.terrain_scripts(),
        )
        .unwrap();
        let elevated_count = game
            .map()
            .cells
            .iter()
            .filter(|code| {
                matches!(
                    wesnoth_engine::terrain::visual_codes(cell_code(code)).0,
                    "Hh" | "Hd" | "Mm"
                )
            })
            .count();
        assert_eq!(
            terrain
                .ground()
                .iter()
                .filter(|sprite| sprite.family == "hills" && sprite.local_order == -1000)
                .count(),
            elevated_count,
            "{scenario}"
        );
        let desert_count = game
            .map()
            .cells
            .iter()
            .filter(|code| wesnoth_engine::terrain::visual_codes(cell_code(code)).0 == "Hd")
            .count();
        assert_eq!(
            terrain
                .ground()
                .iter()
                .filter(|sprite| {
                    sprite.family == "hills"
                        && sprite.local_order == -1000
                        && sprite.frames.assets[0].starts_with("hills:desert")
                })
                .count(),
            desert_count,
            "{scenario}"
        );
        if game
            .map()
            .cells
            .iter()
            .any(|code| wesnoth_engine::terrain::visual_codes(cell_code(code)).0 == "Mm")
        {
            assert!(terrain.world().iter().any(|sprite| {
                sprite.family == "hills" && sprite.frames.assets[0].starts_with("hills:basic")
            }));
        }
        assert!(terrain.world().iter().all(|sprite| {
            let asset = sprite.frames.assets[0].as_str();
            (!matches!(asset, "hills:basic" | "hills:basic2" | "hills:basic3")
                && !asset.starts_with("hills:basic-castle-"))
                || sprite.offset == [-90.0, -108.0]
        }));
        for sprite in terrain.world().iter().filter(|sprite| {
            matches!(
                sprite.frames.assets[0].as_str(),
                "hills:basic_range3_1" | "hills:basic_range4_1"
            )
        }) {
            assert_eq!(
                sprite.clip_hexes.len(),
                21,
                "{scenario}: mountain range at {:?} must include original wildcard padding",
                sprite.anchor
            );
            assert!(sprite.clip_hexes.contains(&sprite.anchor));
            assert!(
                sprite
                    .clip_hexes
                    .iter()
                    .filter(|position| {
                        game.map()
                            .raw(**position)
                            .is_ok_and(|code| wesnoth_engine::terrain::visual_codes(code).0 == "Mm")
                    })
                    .count()
                    >= 8
            );
            checked_long_ranges += 1;
        }

        let peak_positions = game
            .map()
            .cells
            .iter()
            .enumerate()
            .filter(|(_, code)| {
                wesnoth_engine::terrain::visual_codes(cell_code(code)).1 == "Xm"
            })
            .map(|(index, _)| wesnoth_engine::engine::Position {
                x: (index % game.map().width + 1) as i64,
                y: (index / game.map().width + 1) as i64,
            })
            .collect::<Vec<_>>();
        for position in peak_positions {
            let cloud = terrain
                .world()
                .iter()
                .find(|sprite| {
                    sprite.family == "hills"
                        && sprite.anchor == position
                        && sprite.frames.assets[0].starts_with("hills:cloud")
                })
                .unwrap();
            assert_eq!(cloud.clip_hexes.len(), 7);
        }

        let missing_assets = terrain
            .assets()
            .iter()
            .filter(|(id, path)| {
                id.starts_with("hills:")
                    && !Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file()
            })
            .collect::<Vec<_>>();
        assert!(missing_assets.is_empty(), "{scenario}: {missing_assets:?}");
    }
    assert!(checked_long_ranges > 0);
}

#[test]
fn mountain_images_use_the_original_wml_origin_and_layers() {
    use wesnoth_engine::{
        engine::Map,
        terrain_scene::{TerrainScene, TerrainScript},
    };
    let map = Map {
        width: 32,
        height: 32,
        cells: vec!["Mm^Xm".into(); 32 * 32],
    };
    let scene = TerrainScene::from_lua(
        &map,
        &[TerrainScript {
            family: "hills".into(),
            codes: vec!["Mm".into(), "Xm".into()],
            source: include_str!("../scripts/terrain/hills.lua").into(),
        }],
    )
    .unwrap();
    // Global WML images begin at the builder map's top-left. Offsets include
    // the first numbered cell's 36px half-hex and staggered column.
    for (name, offset) in [
        ("basic_range3_1", [-144.0, -108.0]),
        ("basic_range4_1", [-252.0, -108.0]),
        ("basic_range1_1", [-90.0, -144.0]),
        ("basic_range2_1", [-198.0, -144.0]),
        ("basic5_1", [-144.0, -108.0]),
        ("basic6_1", [-144.0, -108.0]),
        ("peak_range1_1", [-144.0, -108.0]),
        ("peak_large1", [-90.0, -144.0]),
        ("peak", [-90.0, -108.0]),
        ("peak2", [-72.0, -72.0]),
        ("peak3", [-72.0, -72.0]),
        ("peak4", [-72.0, -72.0]),
        ("peak5", [-72.0, -72.0]),
        ("cloud", [-72.0, -72.0]),
    ] {
        let matching: Vec<_> = scene
            .world()
            .iter()
            .filter(|sprite| sprite.frames.assets[0] == format!("hills:{name}"))
            .collect();
        assert!(!matching.is_empty(), "fixture must exercise {name}");
        for sprite in matching {
            assert_eq!(sprite.offset, offset, "{name}");
        }
    }
    // Positive WML layers must remain above mountains even on earlier rows.
    assert!(
        scene
            .world()
            .windows(2)
            .all(|pair| pair[0].local_order <= pair[1].local_order)
    );
}

#[test]
fn mountain_clip_hexes_match_upstream_builder_maps_in_pixel_space() {
    use std::collections::BTreeSet;
    use wesnoth_engine::{
        engine::Map,
        terrain_scene::{TerrainScene, TerrainScript},
    };
    let source =
        include_str!("../../Wesnoth-upstream/data/core/terrain-graphics/new-mountains.cfg");
    let scene = TerrainScene::from_lua(
        &Map {
            width: 32,
            height: 32,
            cells: vec!["Mm^Xm".into(); 1024],
        },
        &[TerrainScript {
            family: "hills".into(),
            codes: vec!["Mm".into()],
            source: include_str!("../scripts/terrain/hills.lua").into(),
        }],
    )
    .unwrap();
    for (asset, macro_name) in [
        ("basic5_1", "NEW:MOUNTAINS_2x2"),
        ("basic6_1", "NEW:MOUNTAINS_2x2"),
        ("basic_range1_1", "NEW:MOUNTAINS_1x3_NW_SE"),
        ("basic_range2_1", "NEW:MOUNTAINS_1x3_SW_NE"),
        ("basic_range3_1", "NEW:MOUNTAINS_2x4_NW_SE"),
        ("basic_range4_1", "NEW:MOUNTAINS_2x4_SW_NE"),
        ("peak_large1", "NEW:PEAKS_LARGE"),
        ("peak_range1_1", "NEW:PEAKS_1x2_SW_NE"),
    ] {
        let definition = source
            .split(&format!("#define {macro_name} "))
            .nth(1)
            .unwrap();
        let builder_map = definition
            .split("map=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let mut expected = BTreeSet::new();
        for (row, line) in builder_map.trim().lines().enumerate() {
            let odd = line.trim_start().starts_with(',');
            for (column, cell) in line
                .split(',')
                .filter(|cell| !cell.trim().is_empty())
                .enumerate()
            {
                if cell.trim() != "." {
                    let x = column as i64 * 2 + i64::from(odd);
                    let y = row as i64 / 2;
                    expected.insert((x * 54 + 36, y * 72 + (x % 2) * 36 + 36));
                }
            }
        }
        let mut columns = BTreeSet::new();
        for sprite in scene
            .world()
            .iter()
            .filter(|s| s.frames.assets[0] == format!("hills:{asset}"))
        {
            columns.insert(sprite.anchor.x % 2);
            let center_y = |x: i64, y: i64| (y - 1) * 72 - if x % 2 == 0 { 36 } else { 0 };
            let actual: BTreeSet<_> = sprite
                .clip_hexes
                .iter()
                .map(|p| {
                    (
                        (p.x - sprite.anchor.x) * 54 - sprite.offset[0] as i64,
                        center_y(p.x, p.y)
                            - center_y(sprite.anchor.x, sprite.anchor.y)
                            - sprite.offset[1] as i64,
                    )
                })
                .collect();
            assert_eq!(actual, expected, "{asset} at {:?}", sprite.anchor);
        }
        assert_eq!(
            columns.len(),
            2,
            "exercise both column parities for {asset}"
        );
    }
}
