use wesnoth_engine::{
    engine::{Map, Position},
    terrain_scene::{TerrainScene, TerrainScript},
};

fn scene(map: &Map) -> TerrainScene {
    TerrainScene::from_lua(
        map,
        &[
            TerrainScript {
                family: "road".into(),
                codes: ["Rr", "Rp", "Rd"].map(String::from).to_vec(),
                source: include_str!("../scripts/terrain/road.lua").into(),
            },
            TerrainScript {
                family: "water".into(),
                codes: ["Ww", "Ds", "Ss", "Sm"].map(String::from).to_vec(),
                source: include_str!("../scripts/terrain/water.lua").into(),
            },
            TerrainScript {
                family: "grassland".into(),
                codes: ["Gg", "Gs", "Gd", "Gll"].map(String::from).to_vec(),
                source: include_str!("../scripts/terrain/grass.lua").into(),
            },
            TerrainScript {
                family: "decorations".into(),
                codes: ["Ewl", "Ewf"].map(String::from).to_vec(),
                source: include_str!("../scripts/terrain/decorations.lua").into(),
            },
        ],
    )
    .unwrap()
}

#[test]
fn grass_covers_sand_sand_covers_roads_and_swamp_covers_water() {
    let map = Map {
        width: 8,
        height: 1,
        cells: ["Gg", "Ds", "Rp", "Ds", "Gs", "Ss", "Ww", "Sm"]
            .map(String::from)
            .to_vec(),
    };
    let scene = scene(&map);
    let mut sand_edges = 0;
    let mut grass_edges = 0;
    let mut swamp_edges = 0;
    for sprite in scene.ground() {
        let asset = &sprite.frames.assets[0];
        assert!(
            !(asset.starts_with("road:")
                && sprite.local_order > -1000
                && map.raw(sprite.anchor).unwrap() == "Ds")
        );
        if asset.starts_with("water:sand/beach-") {
            assert_eq!(map.raw(sprite.anchor).unwrap(), "Rp");
            assert_eq!(sprite.local_order, -319);
            sand_edges += 1;
        }
        if asset.starts_with("grassland:")
            && sprite.local_order > -1000
            && map.raw(sprite.anchor).unwrap() == "Ds"
        {
            grass_edges += 1;
        }
        if asset.starts_with("water:swamp/water-")
            && sprite.local_order == -230
            && map.raw(sprite.anchor).unwrap() == "Ww"
        {
            swamp_edges += 1;
        }
    }
    assert!(sand_edges > 0 && grass_edges > 0 && swamp_edges > 0);
    for x in [6, 8] {
        assert!(
            scene
                .ground()
                .iter()
                .any(|s| s.anchor == Position { x, y: 1 }
                    && s.local_order == -1000
                    && s.frames.assets[0].starts_with("water:swamp/"))
        );
    }
    assert!(
        scene
            .ground()
            .iter()
            .any(|s| s.local_order == -85 && s.frames.assets[0].starts_with("water:swamp/reed"))
    );
}

#[test]
fn swamp_covers_every_grass_type_without_reverse_spill() {
    for grass in ["Gg", "Gs", "Gd", "Gll"] {
        for swamp in ["Ss", "Sm"] {
            let map = Map {
                width: 5,
                height: 5,
                cells: (0..25)
                    .map(|i| if i == 12 { swamp } else { grass }.into())
                    .collect(),
            };
            let scene = scene(&map);
            assert!(
                !scene
                    .ground()
                    .iter()
                    .any(|s| s.family == "grassland" && map.raw(s.anchor).unwrap() == swamp),
                "{grass} spills onto {swamp}"
            );
            assert!(
                swamp == "Sm"
                    || scene.ground().iter().any(|s| s.family == "water"
                        && map.raw(s.anchor).unwrap() == grass
                        && s.frames.assets[0].contains("swamp/")),
                "{swamp} must cover {grass}"
            );
        }
    }
}

#[test]
fn lilies_use_original_size_layer_and_shore_variants() {
    for overlay in ["Ewl", "Ewf"] {
        for neighbor in ["Ww", "Ss", "Gg", "Hh", "Ww^Vm"] {
            let small = !matches!(neighbor, "Ww" | "Ss");
            let mut map = Map {
                width: 5,
                height: 5,
                cells: vec!["Ww".into(); 25],
            };
            map.cells[12] = format!("Ww^{overlay}");
            map.cells[7] = neighbor.into();
            let scene = scene(&map);
            let lilies: Vec<_> = scene
                .ground()
                .iter()
                .filter(|s| s.family == "decorations")
                .collect();
            assert_eq!(lilies.len(), 1);
            let lily = lilies[0];
            assert_eq!(lily.local_order, -86);
            assert_eq!(lily.offset, if small { [-36.0; 2] } else { [-55.0; 2] });
            let asset = &lily.frames.assets[0];
            assert_eq!(asset.contains("-small"), small);
            assert_eq!(asset.contains("-flower"), overlay == "Ewf");
            assert!(std::path::Path::new(&scene.assets()[asset]).is_file());
        }
    }
}

#[test]
fn island_and_lagoon_have_all_six_wave_corners() {
    for (center, surround, shape) in [("Ds", "Ww", "convex"), ("Ww", "Ds", "concave")] {
        for x in [2, 3] {
            let mut map = Map {
                width: 5,
                height: 5,
                cells: vec![surround.into(); 25],
            };
            map.cells[2 * 5 + x as usize - 1] = center.into();
            let scene = scene(&map);
            let waves: Vec<_> = scene
                .ground()
                .iter()
                .filter(|s| {
                    s.anchor == Position { x, y: 3 }
                        && s.frames.assets[0] == format!("water:water/waves-{shape}-A01")
                })
                .collect();
            assert_eq!(waves.len(), 6, "{shape}, column {x}");
            for corner in ["tr", "r", "br", "bl", "l", "tl"] {
                assert!(
                    waves
                        .iter()
                        .any(|s| s.image_mods.masks == [format!("water:masks/7hex-{corner}")])
                );
            }
            assert!(waves.iter().all(|s| s.local_order == -499
                && s.frames.assets.len() == 13
                && s.frames.frame_ms == 200
                && s.clip_hexes.len() == 7));
        }
    }
}
