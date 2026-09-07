use std::{collections::BTreeSet, sync::OnceLock};

use crate::engine::{Map, Position};
use crate::terrain_rules::{TerrainRule, compose, load_rules};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualKind {
    Base,
    Overlay,
    Edge(usize),
    Transition(usize),
    TransitionRun(u8),
    CastleConvex(usize),
    CastleConcave(usize),
    KeepConvex(usize),
    KeepConcave(usize),
    BridgeEnd(usize),
    MountainRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualTile {
    pub position: Position,
    pub image: &'static str,
    pub kind: VisualKind,
    pub layer: i16,
}

pub fn gameplay_type(code: &str) -> &'static str {
    let (base, overlay) = split(code);
    if overlay.starts_with('V') {
        "village"
    } else if overlay.starts_with('F') {
        "forest"
    } else if base.starts_with('K') {
        "keep"
    } else if base.starts_with('C') {
        "castle"
    } else if base.starts_with('W') || base == "Ss" {
        "water"
    } else if base.starts_with('H') || base.starts_with('M') {
        "hills"
    } else {
        match code {
            "forest" => "forest",
            "hills" => "hills",
            "water" => "water",
            "castle" => "castle",
            "keep" => "keep",
            "village" => "village",
            _ => "grassland",
        }
    }
}

pub fn build_visuals(map: &Map) -> Vec<VisualTile> {
    let mut result = Vec::with_capacity(map.cells.len() * 2);
    for y in 1..=map.height as i64 {
        for x in 1..=map.width as i64 {
            let position = Position { x, y };
            let code = map.raw(position).unwrap_or("Gg");
            let (base, overlay) = split(code);
            if !rule_managed_base(base) {
                result.push(VisualTile {
                    position,
                    image: base_image(base),
                    kind: VisualKind::Base,
                    layer: -1000,
                });
            }
            if !overlay.starts_with('B')
                && let Some(image) = overlay_image(
                    overlay,
                    position,
                    forest_needs_small(map, position, overlay),
                )
            {
                result.push(VisualTile {
                    position,
                    image,
                    kind: VisualKind::Overlay,
                    layer: overlay_layer(overlay),
                });
            }
            let priority = transition_priority(base);
            let mut green_run = 0u8;
            for (direction, neighbor) in neighbors(position).into_iter().enumerate() {
                let Ok(neighbor) = map.raw(neighbor) else {
                    continue;
                };
                let neighbor_base = split(neighbor).0;
                if transition_priority(neighbor_base) > priority
                    && let Some(image) = transition_image(neighbor_base)
                {
                    if image == "transition-ocean" {
                        continue;
                    }
                    if image == "transition-grass-green" {
                        green_run |= 1 << direction;
                        continue;
                    }
                    result.push(VisualTile {
                        position,
                        image,
                        kind: VisualKind::Transition(direction),
                        layer: -400 + transition_priority(neighbor_base),
                    });
                }
            }
            if green_run != 0 {
                result.push(VisualTile {
                    position,
                    image: "transition-grass-green",
                    kind: VisualKind::TransitionRun(green_run),
                    layer: -400 + transition_priority("Gg"),
                });
            }
        }
    }
    result.extend(rule_visuals(map));
    result.sort_by_key(|tile| tile.layer);
    result
}

fn rule_managed_base(base: &str) -> bool {
    base == "Gg" || matches!(base, "Wo" | "Ww" | "Wwf" | "Wwg" | "Ce" | "Ke")
}

fn rule_visuals(map: &Map) -> Vec<VisualTile> {
    static RULES: OnceLock<Vec<TerrainRule>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        load_rules(include_str!("../scripts/terrain/graphics.wml"))
            .expect("bundled terrain rules must be valid")
    });
    compose(map, rules, 0)
        .into_iter()
        .filter_map(|image| {
            let (name, kind) = match image.path.as_str() {
                "grass-green" => ("grass-green", VisualKind::Base),
                "water" => ("water", VisualKind::Base),
                "ocean" => ("ocean", VisualKind::Base),
                "dirt" => ("dirt", VisualKind::Base),
                "wood-bridge-n-s" => ("wood-bridge-n-s", VisualKind::Overlay),
                "wood-bridge-ne-sw" => ("wood-bridge-ne-sw", VisualKind::Overlay),
                "wood-bridge-se-nw" => ("wood-bridge-se-nw", VisualKind::Overlay),
                "stone-bridge-n-s" => ("stone-bridge-n-s", VisualKind::Overlay),
                "stone-bridge-ne-sw" => ("stone-bridge-ne-sw", VisualKind::Overlay),
                "stone-bridge-se-nw" => ("stone-bridge-se-nw", VisualKind::Overlay),
                "wood-bridge-three-n" => ("wood-bridge-n-se-sw", VisualKind::Overlay),
                "wood-bridge-three-ne" => ("wood-bridge-ne-s-nw", VisualKind::Overlay),
                "wood-bridge-corner-n" => ("wood-bridge-n-se", VisualKind::Overlay),
                "wood-bridge-corner-ne" => ("wood-bridge-ne-s", VisualKind::Overlay),
                "wood-bridge-corner-se" => ("wood-bridge-se-sw", VisualKind::Overlay),
                "wood-bridge-corner-s" => ("wood-bridge-s-nw", VisualKind::Overlay),
                "wood-bridge-corner-sw" => ("wood-bridge-sw-n", VisualKind::Overlay),
                "wood-bridge-corner-nw" => ("wood-bridge-nw-ne", VisualKind::Overlay),
                path if path.starts_with("wood-bridge-end-") => (
                    "wood-bridge-end",
                    VisualKind::BridgeEnd(direction(path, "wood-bridge-end-")?),
                ),
                path if path.starts_with("wood-bridge-dock-") => (
                    "wood-bridge-dock",
                    VisualKind::BridgeEnd(direction(path, "wood-bridge-dock-")?),
                ),
                path if path.starts_with("stone-bridge-end-") => (
                    "stone-bridge-end",
                    VisualKind::BridgeEnd(direction(path, "stone-bridge-end-")?),
                ),
                path if path.starts_with("transition-ocean-") => {
                    let direction = match path.trim_start_matches("transition-ocean-") {
                        "n" => 0,
                        "ne" => 1,
                        "se" => 2,
                        "s" => 3,
                        "sw" => 4,
                        "nw" => 5,
                        _ => return None,
                    };
                    ("transition-ocean", VisualKind::Transition(direction))
                }
                path if path.starts_with("encampment-convex-") => (
                    "encampment-convex",
                    VisualKind::CastleConvex(castle_corner(path, "encampment-convex-")?),
                ),
                path if path.starts_with("encampment-concave-") => (
                    "encampment-concave",
                    VisualKind::CastleConcave(castle_corner(path, "encampment-concave-")?),
                ),
                _ => return None,
            };
            Some(VisualTile {
                position: image.anchor,
                image: name,
                kind,
                layer: image.layer,
            })
        })
        .collect()
}

fn castle_corner(path: &str, prefix: &str) -> Option<usize> {
    ["tr", "r", "br", "bl", "l", "tl"]
        .iter()
        .position(|name| *name == path.trim_start_matches(prefix))
}

fn direction(path: &str, prefix: &str) -> Option<usize> {
    ["n", "ne", "se", "s", "sw", "nw"]
        .iter()
        .position(|name| *name == path.trim_start_matches(prefix))
}

fn add_castle_walls(map: &Map, result: &mut Vec<VisualTile>) {
    let mut flags = BTreeSet::new();
    for convex in [true, false] {
        for y in 1..=map.height as i64 {
            for x in 1..=map.width as i64 {
                let position = Position { x, y };
                let base = split(map.raw(position).unwrap_or("Gg")).0;
                if is_castle(base) != convex {
                    continue;
                }
                let adjacent = neighbors(position);
                let neighbor_bases =
                    adjacent.map(|neighbor| map.raw(neighbor).ok().map(|value| split(value).0));
                let castle = neighbor_bases.map(|value| value.is_some_and(is_castle));
                let keep = neighbor_bases.map(|value| value.is_some_and(is_keep));
                for corner in 0..6 {
                    let next = (corner + 1) % 6;
                    let matches = if convex {
                        !castle[corner] && !castle[next]
                    } else {
                        castle[corner] && castle[next]
                    };
                    if !matches {
                        continue;
                    }
                    let claims = [
                        (position.x, position.y, corner),
                        (adjacent[corner].x, adjacent[corner].y, (corner + 2) % 6),
                        (adjacent[next].x, adjacent[next].y, (corner + 4) % 6),
                    ];
                    if claims.iter().any(|claim| flags.contains(claim)) {
                        continue;
                    }
                    flags.extend(claims);
                    let use_keep = is_keep(base) || keep[corner] || keep[next];
                    result.push(VisualTile {
                        position,
                        image: match (use_keep, convex) {
                            (true, true) => "keep-convex",
                            (true, false) => "keep-concave",
                            (false, true) => "castle-convex",
                            (false, false) => "castle-concave",
                        },
                        kind: match (use_keep, convex) {
                            (true, true) => VisualKind::KeepConvex(corner),
                            (true, false) => VisualKind::KeepConcave(corner),
                            (false, true) => VisualKind::CastleConvex(corner),
                            (false, false) => VisualKind::CastleConcave(corner),
                        },
                        layer: 20,
                    });
                }
            }
        }
    }
}

fn add_mountain_ranges(map: &Map, result: &mut Vec<VisualTile>) {
    let mut claimed = BTreeSet::new();
    for (direction, side, images) in [
        (
            2,
            1,
            [
                "mountain-long-se-1",
                "mountain-long-se-2",
                "mountain-long-se-3",
                "mountain-long-se-4",
                "mountain-long-se-5",
            ],
        ),
        (
            1,
            2,
            [
                "mountain-long-ne-1",
                "mountain-long-ne-2",
                "mountain-long-ne-3",
                "mountain-long-ne-4",
                "mountain-long-ne-5",
            ],
        ),
    ] {
        for y in 1..=map.height as i64 {
            for x in 1..=map.width as i64 {
                let first = Position { x, y };
                let second = neighbors(first)[direction];
                let third = neighbors(second)[direction];
                let fourth = neighbors(third)[direction];
                let positions = [
                    first,
                    second,
                    neighbors(second)[side],
                    third,
                    neighbors(third)[side],
                    fourth,
                ];
                claim_mountains(map, &mut claimed, result, &positions, &images);
            }
        }
    }
    for y in 1..=map.height as i64 {
        for x in 1..=map.width as i64 {
            let first = Position { x, y };
            let east = neighbors(first)[1];
            let southeast = neighbors(first)[2];
            let far = neighbors(east)[2];
            let images = if (x + y) & 1 == 0 {
                [
                    "mountain-cluster-a-1",
                    "mountain-cluster-a-2",
                    "mountain-cluster-a-3",
                ]
            } else {
                [
                    "mountain-cluster-b-1",
                    "mountain-cluster-b-2",
                    "mountain-cluster-b-3",
                ]
            };
            claim_mountains(
                map,
                &mut claimed,
                result,
                &[first, east, southeast, far],
                &images,
            );
        }
    }
    for y in 1..=map.height as i64 {
        for x in 1..=map.width as i64 {
            let start = Position { x, y };
            if claimed.contains(&(x, y)) || !is_mountain(map, start) {
                continue;
            }
            for (direction, images) in [
                (
                    2,
                    [
                        "mountain-range-se-1",
                        "mountain-range-se-2",
                        "mountain-range-se-3",
                    ],
                ),
                (
                    1,
                    [
                        "mountain-range-ne-1",
                        "mountain-range-ne-2",
                        "mountain-range-ne-3",
                    ],
                ),
            ] {
                let second = neighbors(start)[direction];
                let third = neighbors(second)[direction];
                if is_mountain(map, second)
                    && is_mountain(map, third)
                    && !claimed.contains(&(second.x, second.y))
                    && !claimed.contains(&(third.x, third.y))
                {
                    for (position, image) in [start, second, third].into_iter().zip(images) {
                        claimed.insert((position.x, position.y));
                        result.push(VisualTile {
                            position,
                            image,
                            kind: VisualKind::MountainRange,
                            layer: 8,
                        });
                    }
                    break;
                }
            }
        }
    }
}

fn claim_mountains(
    map: &Map,
    claimed: &mut BTreeSet<(i64, i64)>,
    result: &mut Vec<VisualTile>,
    positions: &[Position],
    images: &[&'static str],
) -> bool {
    if positions
        .iter()
        .any(|position| claimed.contains(&(position.x, position.y)) || !is_mountain(map, *position))
    {
        return false;
    }
    for position in positions {
        claimed.insert((position.x, position.y));
    }
    for (position, image) in positions.iter().copied().zip(images.iter().copied()) {
        result.push(VisualTile {
            position,
            image,
            kind: VisualKind::MountainRange,
            layer: 8,
        });
    }
    true
}

fn is_mountain(map: &Map, position: Position) -> bool {
    map.raw(position).is_ok_and(|value| split(value).0 == "Mm")
}

fn is_castle(base: &str) -> bool {
    base.starts_with('C') || base.starts_with('K')
}

fn is_keep(base: &str) -> bool {
    base.starts_with('K') || base == "keep"
}

fn split(code: &str) -> (&str, &str) {
    let code = code.split_whitespace().last().unwrap_or(code);
    code.split_once('^').unwrap_or((code, ""))
}

fn is_water(base: &str) -> bool {
    base.starts_with('W') || base == "Ss"
}

fn transition_priority(base: &str) -> i16 {
    match base {
        "Ww" | "Wwf" | "Wwg" | "Ss" => 0,
        "Wo" => 1,
        "Ds" => 10,
        "Gd" => 20,
        "Gs" => 21,
        "Gg" => 22,
        "Gll" => 23,
        "Re" => 30,
        "Rp" => 31,
        "Hh" => 40,
        _ => -1,
    }
}

fn transition_image(base: &str) -> Option<&'static str> {
    Some(match base {
        "Wo" => "transition-ocean",
        "Ds" => "transition-beach",
        "Gd" => "transition-grass-dry",
        "Gs" => "transition-grass-semi-dry",
        "Gg" => "transition-grass-green",
        "Gll" => "transition-leaf-litter",
        "Re" => "transition-dirt",
        "Rp" => "transition-stone-path",
        "Hh" => "transition-hills",
        "Ss" => "transition-swamp",
        _ => return None,
    })
}

fn base_image(base: &str) -> &'static str {
    match base {
        "Gg" | "grassland" => "grass-green",
        "Gs" => "grass-semi-dry",
        "Gd" => "grass-dry",
        "Gll" => "leaf-litter",
        "Re" => "dirt",
        "Rp" => "stone-path",
        "Ds" => "beach",
        "Hh" | "hills" => "hills-regular",
        "Mm" => "mountains",
        "Ss" => "swamp",
        "Wo" => "ocean",
        value if value.starts_with('W') => "water",
        "Ce" | "Ke" => "dirt",
        value if value.starts_with('K') => "keep-ground",
        value if value.starts_with('C') => "castle-ground",
        "castle" => "castle-ground",
        "keep" => "keep-ground",
        _ => "grass-green",
    }
}

fn overlay_image(overlay: &str, position: Position, small_forest: bool) -> Option<&'static str> {
    let variant = ((position.x * 17 + position.y * 31).unsigned_abs() % 3) as usize;
    Some(match overlay {
        value if value.starts_with("Fms") && small_forest => "forest-mixed-small",
        value if value.starts_with("Fds") && small_forest => "forest-summer-small",
        value if value.starts_with("Fp") && small_forest => "forest-pine-small",
        value if value.starts_with("Fms") => {
            ["forest-mixed-1", "forest-mixed-2", "forest-mixed-3"][variant]
        }
        value if value.starts_with("Fds") => {
            ["forest-summer-1", "forest-summer-2", "forest-summer-3"][variant]
        }
        value if value.starts_with("Fp") => {
            ["forest-pine-1", "forest-pine-2", "forest-pine-3"][variant]
        }
        value if value.starts_with("Vhh") => "village-hills",
        value if value.starts_with("Vhcr") => "village-human-city-ruin",
        value if value.starts_with("Vhhr") => "village-human-hills-ruin",
        value if value.starts_with("Vh") => "village-human",
        value if value.starts_with("Vc") => "village-hut",
        value if value.starts_with("Vl") => "village-log-cabin",
        value if value.starts_with("Vct") => "village-camp",
        value if value.starts_with("Bw") => bridge_image(overlay),
        value if value.starts_with("Bsb") => bridge_image(overlay),
        "Xm" => "mountain-wall",
        "Efm" => "flowers-mixed",
        "Eff" => "flowers-farm",
        "Em" => "mushrooms",
        "Es" => "stones",
        "Edb" => "detritus",
        "Ewf" => "water-flowers",
        "Gvs" => "farm",
        "Wm" => "windmill",
        "" => return None,
        _ => return None,
    })
}

fn forest_needs_small(map: &Map, position: Position, overlay: &str) -> bool {
    if !overlay.starts_with('F') {
        return false;
    }
    neighbors(position).into_iter().any(|neighbor| {
        map.raw(neighbor).is_ok_and(|value| {
            let (base, overlay) = split(value);
            is_water(base)
                || base.starts_with('C')
                || base.starts_with('K')
                || base.starts_with('M')
                || overlay.starts_with('V')
        })
    })
}

fn bridge_axis(overlay: &str) -> (usize, usize) {
    if overlay.ends_with('|') {
        (0, 3)
    } else if overlay.ends_with('/') {
        (1, 4)
    } else {
        (2, 5)
    }
}

fn bridge_image(overlay: &str) -> &'static str {
    if overlay.starts_with("Bsb") {
        if overlay.ends_with('|') {
            "stone-bridge-n-s"
        } else if overlay.ends_with('/') {
            "stone-bridge-ne-sw"
        } else {
            "stone-bridge-se-nw"
        }
    } else if overlay.ends_with('|') {
        "wood-bridge-n-s"
    } else if overlay.ends_with('/') {
        "wood-bridge-ne-sw"
    } else {
        "wood-bridge-se-nw"
    }
}

fn overlay_layer(overlay: &str) -> i16 {
    if overlay.starts_with('B') {
        -10
    } else if overlay.starts_with('F') {
        10
    } else {
        0
    }
}

pub fn neighbors(position: Position) -> [Position; 6] {
    let up = if position.x % 2 == 0 { 0 } else { -1 };
    [
        Position {
            x: position.x,
            y: position.y - 1,
        },
        Position {
            x: position.x + 1,
            y: position.y + up,
        },
        Position {
            x: position.x + 1,
            y: position.y + up + 1,
        },
        Position {
            x: position.x,
            y: position.y + 1,
        },
        Position {
            x: position.x - 1,
            y: position.y + up + 1,
        },
        Position {
            x: position.x - 1,
            y: position.y + up,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wesnoth_codes_keep_gameplay_and_visual_layers_separate() {
        assert_eq!(gameplay_type("Gs^Fms"), "forest");
        assert_eq!(gameplay_type("Ww^Bw/"), "water");
        assert_eq!(gameplay_type("1 Ke"), "keep");
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Gg^Vh".into(), "Ww".into()],
        };
        let visuals = build_visuals(&map);
        assert!(visuals.iter().any(|tile| tile.image == "village-human"));
        assert!(
            !visuals
                .iter()
                .any(|tile| matches!(tile.kind, VisualKind::Edge(_)))
        );
        assert!(visuals.iter().any(|tile| {
            tile.position == Position { x: 2, y: 1 } && tile.image == "transition-grass-green"
        }));
        assert!(
            visuals
                .iter()
                .any(|tile| matches!(tile.kind, VisualKind::TransitionRun(_)))
        );
    }

    #[test]
    fn castle_walls_only_cover_the_exposed_sides() {
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Ce".into(), "Ke".into()],
        };
        let visuals = build_visuals(&map);
        assert_eq!(
            visuals
                .iter()
                .filter(|tile| matches!(tile.kind, VisualKind::Base))
                .count(),
            2
        );
        let walls = visuals
            .iter()
            .filter(|tile| {
                matches!(
                    tile.kind,
                    VisualKind::CastleConvex(_) | VisualKind::KeepConvex(_)
                )
            })
            .count();
        assert_eq!(walls, 8);
        assert!(visuals.iter().all(|tile| {
            !matches!(
                tile.kind,
                VisualKind::CastleConvex(_)
                    | VisualKind::CastleConcave(_)
                    | VisualKind::KeepConvex(_)
                    | VisualKind::KeepConcave(_)
            ) || matches!(map.get(tile.position), Ok("castle" | "keep"))
        }));
    }

    #[test]
    fn forests_shrink_at_hard_edges_and_bridges_get_axis_aligned_ends() {
        let map = Map {
            width: 3,
            height: 1,
            cells: vec!["Ww".into(), "Gg^Fp".into(), "Ww^Bw|".into()],
        };
        let visuals = build_visuals(&map);
        assert!(visuals.iter().any(|tile| tile.image == "forest-pine-small"));
        let ends = visuals
            .iter()
            .filter_map(|tile| match tile.kind {
                VisualKind::BridgeEnd(direction) => Some(direction),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(ends, vec![0, 3]);
        assert!(visuals.iter().any(|tile| tile.image == "wood-bridge-n-s"));
    }

    #[test]
    fn connected_bridge_tiles_do_not_get_ends_at_the_join() {
        let map = Map {
            width: 1,
            height: 2,
            cells: vec!["Ww^Bw|".into(), "Ww^Bw|".into()],
        };
        let ends = build_visuals(&map)
            .into_iter()
            .filter(|tile| matches!(tile.kind, VisualKind::BridgeEnd(_)))
            .count();
        assert_eq!(ends, 2);
    }

    #[test]
    fn adjacent_bridges_on_different_axes_keep_their_straight_sprites() {
        let map = Map {
            width: 3,
            height: 3,
            cells: vec![
                "Gg".into(),
                "Ww^Bw|".into(),
                "Gg".into(),
                "Ww^Bw/".into(),
                "Ww^Bw|".into(),
                "Gg".into(),
                "Gg".into(),
                "Gg".into(),
                "Ww^Bw\\".into(),
            ],
        };
        let center = build_visuals(&map)
            .into_iter()
            .filter(|tile| tile.position == Position { x: 2, y: 2 })
            .map(|tile| tile.image)
            .collect::<Vec<_>>();
        assert!(center.contains(&"wood-bridge-n-s"));
        assert!(
            !center
                .iter()
                .any(|image| image.contains("corner") || image.contains("three"))
        );
    }

    #[test]
    fn wooden_bridge_uses_docks_in_water_and_ramps_on_land() {
        let map = Map {
            width: 1,
            height: 3,
            cells: vec!["Gg".into(), "Ww^Bw|".into(), "Ww".into()],
        };
        let ends = build_visuals(&map)
            .into_iter()
            .filter(|tile| matches!(tile.kind, VisualKind::BridgeEnd(_)))
            .map(|tile| tile.image)
            .collect::<Vec<_>>();
        assert_eq!(ends, vec!["wood-bridge-dock", "wood-bridge-end"]);
    }

    #[test]
    fn deep_water_blends_into_shallow_water() {
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Wo".into(), "Ww".into()],
        };
        let visuals = build_visuals(&map);
        assert!(visuals.iter().any(|tile| {
            tile.position == Position { x: 2, y: 1 } && tile.image == "transition-ocean"
        }));
        assert_eq!(
            visuals
                .iter()
                .filter(|tile| matches!(tile.kind, VisualKind::Base))
                .count(),
            2
        );
    }

    #[test]
    fn mountains_fall_back_to_one_correctly_anchored_base_each() {
        let map = Map {
            width: 3,
            height: 2,
            cells: vec![
                "Mm".into(),
                "Mm".into(),
                "Gg".into(),
                "Gg".into(),
                "Gg".into(),
                "Mm".into(),
            ],
        };
        let visuals = build_visuals(&map);
        assert_eq!(
            visuals
                .iter()
                .filter(|tile| tile.image == "mountains")
                .count(),
            3
        );
        assert!(
            !visuals
                .iter()
                .any(|tile| matches!(tile.kind, VisualKind::MountainRange))
        );
    }
}
