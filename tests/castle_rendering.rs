use wesnoth_engine::{
    engine::Map,
    game::Game,
    terrain_scene::{TerrainPass, TerrainScene, TerrainScript},
};
fn scene(map: &Map) -> TerrainScene {
    TerrainScene::from_lua(
        map,
        &[TerrainScript {
            family: "castle".into(),
            codes: [
                "Ch", "Kh", "Chr", "Khr", "Ce", "Ke", "Chw", "Cvr", "Kvr", "Cud", "Kud",
            ]
            .map(String::from)
            .to_vec(),
            source: include_str!("../scripts/terrain/castles.lua").into(),
        }],
    )
    .unwrap()
}
#[test]
fn castle_corners_and_keep_courtyard_use_distinct_original_rules() {
    for x in [2, 3] {
        let mut map = Map {
            width: 5,
            height: 5,
            cells: vec!["Gg".into(); 25],
        };
        map.cells[10 + x - 1] = "Ch".into();
        let isolated = scene(&map);
        assert_eq!(isolated.world().len(), 6);
        assert!(
            isolated
                .world()
                .iter()
                .all(|s| s.frames.assets[0].contains("castle-convex-") && s.clip_hexes.len() == 3)
        );
        map.cells.fill("Ch".into());
        map.cells[10 + x - 1] = "Kh".into();
        let courtyard = scene(&map);
        assert_eq!(
            courtyard
                .world()
                .iter()
                .filter(|s| s.frames.assets[0].contains("keep-castle-convex-"))
                .count(),
            6
        );
    }
}
#[test]
fn adjacent_castles_do_not_draw_walls_at_the_shared_edge() {
    let mut map = Map {
        width: 5,
        height: 5,
        cells: vec!["Gg".into(); 25],
    };
    map.cells[12] = "Ch".into();
    map.cells[17] = "Ch".into();
    let scene = scene(&map);
    assert_eq!(scene.world().len(), 10);
    assert_eq!(
        scene
            .world()
            .iter()
            .filter(|s| s.frames.assets[0].contains("concave"))
            .count(),
        2
    );
}
#[test]
fn keep_meets_outer_castle_with_clockwise_and_counterclockwise_sections() {
    let mut map = Map {
        width: 5,
        height: 5,
        cells: vec!["Gg".into(); 25],
    };
    map.cells[12] = "Kh".into();
    map.cells[7] = "Ch".into();
    let scene = scene(&map);
    for suffix in ["keep-castle-cw-tr", "keep-castle-ccw-tl"] {
        assert!(
            scene
                .world()
                .iter()
                .any(|s| s.frames.assets[0].ends_with(suffix))
        );
    }
    map.cells[7] = "Xu".into();
    let scene = self::scene(&map);
    assert!(
        scene
            .world()
            .iter()
            .any(|s| s.baseline == 107.0 + s.offset[1])
    );
}

#[test]
fn dry_castle_floor_blends_into_flooded_floor_in_every_direction() {
    for x in [2, 3] {
        for dry in ["Ch", "Chr"] {
            let mut map = Map {
                width: 5,
                height: 5,
                cells: vec![dry.into(); 25],
            };
            map.cells[10 + x - 1] = "Chw".into();
            let rendered = scene(&map);
            let edges: Vec<_> = rendered
                .ground()
                .iter()
                .filter(|s| s.local_order == -300)
                .collect();
            assert_eq!(edges.len(), 1);
            assert_eq!(
                edges[0].frames.assets[0],
                "castle:flat/road-n-ne-se-s-sw-nw"
            );
            assert_eq!(map.raw(edges[0].anchor).unwrap(), "Chw");
            assert!(std::path::Path::new(&rendered.assets()[&edges[0].frames.assets[0]]).is_file());
        }
    }
}

#[test]
fn campaign_castles_resolve_assets_and_world_depth() {
    for name in [
        "rooting_out_a_mage",
        "the_chase",
        "guarded_castle",
        "return_to_the_village",
    ] {
        let game = Game::load("scripts", &format!("scenarios/{name}.wml")).unwrap();
        let scene = TerrainScene::from_lua(game.map(), game.terrain_scripts()).unwrap();
        assert!(scene.world().iter().any(|s| s.family == "castle"));
        for s in scene
            .ground()
            .iter()
            .chain(scene.world())
            .filter(|s| s.family == "castle")
        {
            assert!(std::path::Path::new(&scene.assets()[&s.frames.assets[0]]).is_file());
            if !s.clip_hexes.is_empty() {
                assert_eq!(s.pass, TerrainPass::World);
                assert!([26.0, 72.0, 74.0, 107.0].contains(&(s.baseline - s.offset[1])));
            }
        }
    }
}
