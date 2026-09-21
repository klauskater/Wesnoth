use wesnoth_engine::{
    adventure::Adventure,
    engine::{Map, Position},
    game::{self, Game},
    terrain_scene::{TerrainScene, TerrainScript},
};

fn campaign_game(chapter: usize) -> Game {
    let manifest = std::fs::read_to_string("scripts/adventures/two_brothers.wml").unwrap();
    let adventure = Adventure::parse(&manifest).unwrap();
    Game::start(
        game::load_adventure_resources(
            "scripts",
            &adventure,
            &adventure.scenarios[chapter],
            None,
        )
        .unwrap(),
    )
    .unwrap()
}

fn scene(map: &Map) -> TerrainScene {
    TerrainScene::from_lua(
        map,
        &[TerrainScript {
            family: "bridges".into(),
            codes: ["Bw|", "Bw/", "Bw\\", "Bsb|", "Bsb/", "Bsb\\"]
                .map(String::from)
                .to_vec(),
            source: include_str!("../scripts/terrain/bridges.lua").into(),
        }],
    )
    .unwrap()
}

#[test]
fn campaign_bridges_load_original_assets() {
    let mut count = 0;
    for chapter in 0..4 {
        let game = campaign_game(chapter);
        let scene = TerrainScene::from_lua(game.map(), game.terrain_scripts()).unwrap();
        for s in scene.ground().iter().filter(|s| s.family == "bridges") {
            assert_eq!(s.local_order, 0);
            assert!(std::path::Path::new(&scene.assets()[&s.frames.assets[0]]).is_file());
            count += 1;
        }
    }
    assert!(count > 10);
}

#[test]
fn wooden_bend_has_no_end_caps_between_connected_sections() {
    let map = Map {
        width: 3,
        height: 2,
        cells: ["Wo^Bw|", "Ww", "Ww", "Ww", "Wo^Bw\\", "Ww^Bw\\"]
            .map(Into::into)
            .to_vec(),
    };
    let scene = scene(&map);
    for (x, y, name) in [
        (1, 1, "wood-n-se"),
        (2, 2, "wood-se-nw"),
        (3, 2, "wood-se-nw"),
    ] {
        assert!(
            scene.ground().iter().any(|s| s.anchor == Position { x, y }
                && s.frames.assets[0] == format!("bridges:{name}"))
        );
    }
    assert!(
        !scene
            .ground()
            .iter()
            .any(|s| s.frames.assets[0].contains("dock") || s.frames.assets[0].contains("end"))
    );
}

#[test]
fn all_axes_use_docks_on_water_ramps_on_land_and_no_ramps_on_castles() {
    for overlay in ["Bw|", "Bw/", "Bw\\"] {
        for (base, ends) in [
            ("Ww", "wood-dock-"),
            ("Ss", "wood-dock-"),
            ("Gg", "wood-end-"),
            ("Ch", ""),
        ] {
            let mut map = Map {
                width: 5,
                height: 5,
                cells: vec![base.into(); 25],
            };
            map.cells[12] = format!("Ww^{overlay}").into();
            let scene = scene(&map);
            assert_eq!(scene.ground().len(), if ends.is_empty() { 1 } else { 3 });
            for s in scene
                .ground()
                .iter()
                .filter(|s| s.anchor != Position { x: 3, y: 3 })
            {
                assert!(s.frames.assets[0].starts_with(&format!("bridges:{ends}")));
                assert_eq!(s.offset, [-36.0, -36.0]);
            }
        }
    }
}

#[test]
fn stone_spans_have_one_image_per_join_and_original_sized_ends() {
    for (overlay, next) in [("Bsb|", (3, 4)), ("Bsb/", (2, 4)), ("Bsb\\", (4, 4))] {
        for base in ["Ww", "Gg", "Ch", "Xu"] {
            let mut map = Map {
                width: 6,
                height: 6,
                cells: vec![base.into(); 36],
            };
            map.cells[14] = format!("Ww^{overlay}").into();
            map.cells[(next.1 - 1) * 6 + next.0 - 1] = format!("Ww^{overlay}").into();
            let scene = scene(&map);
            assert_eq!(scene.ground().len(), 3, "{overlay} on {base}");
            for s in scene.ground() {
                assert!(
                    std::path::Path::new(&scene.assets()[&s.frames.assets[0]]).is_file(),
                    "{:?}",
                    s.frames.assets
                );
            }
        }
    }
}
