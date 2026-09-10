//! Преобразование игровых кодов карты в независимые визуальные слои.
//!
//! Сценарий хранит один код на гекс, например `Ww^Bw|`: `Ww` — базовая
//! мелкая вода, `Bw|` — наложение вертикального деревянного моста. Игровая
//! логика видит тип через `gameplay_type`, а отрисовка через `build_visuals`
//! превращает тот же код в несколько `VisualTile`: основу, переходы берегов,
//! мост, его торцы, стены, лес и прочий декор.
//!
//! Здесь нет PNG и вызовов рисования. `image` — стабильный идентификатор,
//! который Android-клиент сопоставит с загруженной текстурой.

use std::{collections::BTreeSet, sync::OnceLock};

use crate::engine::{Map, Position};
use crate::terrain_rules::{TerrainRule, compose, load_rules};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualKind {
    /// Сплошная подложка гекса: трава, вода, грунт и т. п.
    Base,
    /// Объект поверх основы: лес, деревня, мост, украшение.
    Overlay,
    /// Односторонняя граница воды; число — направление 0..5.
    Edge(usize),
    /// Переход одного типа поверхности в другой; число — направление 0..5.
    Transition(usize),
    /// Несколько соседних переходов, собранных в один исходный PNG.
    TransitionRun(u8),
    CastleConvex(usize),
    CastleConcave(usize),
    KeepConvex(usize),
    KeepConcave(usize),
    /// Окончание моста. Направления всегда: N, NE, SE, S, SW, NW.
    BridgeEnd(usize),
    MountainRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualTile {
    /// Гекс-якорь. Большой PNG может выходить далеко за его границы.
    pub position: Position,
    /// Ключ текстуры из `AndroidArt`, а не файловый путь.
    pub image: &'static str,
    /// Подсказывает рендереру, какой набор направленных текстур выбрать.
    pub kind: VisualKind,
    /// Глобальный порядок: меньшие значения рисуются раньше и оказываются снизу.
    pub layer: i16,
}

pub fn gameplay_type(code: &str) -> &'static str {
    let (base, overlay) = visual_codes(code);
    if overlay.starts_with('B') {
        "grassland"
    } else if overlay.starts_with('V') {
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
    // Первый проход обслуживает простые тайлы, которым не нужен сложный WML:
    // базовые поверхности, одиночные оверлеи и часть переходов между базами.
    let mut result = Vec::with_capacity(map.cells.len() * 2);
    for y in 1..=map.height as i64 {
        for x in 1..=map.width as i64 {
            let position = Position { x, y };
            let code = map.raw(position).unwrap_or("Gg");
            let (base, overlay) = visual_codes(code);
            // Типы из rule_managed_base будут добавлены terrain-правилами.
            // Если нарисовать их и здесь, один гекс получит две основы.
            if !rule_managed_base(base) {
                result.push(VisualTile {
                    position,
                    image: if base == "Mm" {
                        mountain_single_image(position)
                    } else {
                        base_image(base)
                    },
                    kind: VisualKind::Base,
                    layer: if base == "Mm" { 8 } else { -1000 },
                });
            }
            // Мосты начинаются с B и зависят от направления и соседей, поэтому
            // проходят только через WML ниже. Леса/деревни/декор можно выбрать
            // непосредственно по коду оверлея.
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
            if let Some(image) = base_overlay_image(base) {
                result.push(VisualTile {
                    position,
                    image,
                    kind: VisualKind::Overlay,
                    layer: 0,
                });
            }
            // Переход рисуется на гексе с меньшим приоритетом поверхности в
            // сторону соседа с большим приоритетом. Направления перечислены в
            // neighbors() строго как N, NE, SE, S, SW, NW.
            let priority = transition_priority(base);
            let mut green_run = 0u8;
            for (direction, neighbor) in neighbors(position).into_iter().enumerate() {
                let Ok(neighbor) = map.raw(neighbor) else {
                    continue;
                };
                let neighbor_base = visual_codes(neighbor).0;
                if !is_water(base) && matches!(neighbor_base, "Ww" | "Wwf" | "Wwg") {
                    result.push(VisualTile {
                        position: neighbors(position)[direction],
                        image: if base == "Ds" {
                            "transition-beach"
                        } else {
                            "shore"
                        },
                        kind: VisualKind::Transition((direction + 3) % 6),
                        layer: -482,
                    });
                    continue;
                }
                if matches!(base, "Ww" | "Wwf" | "Wwg") && !is_water(neighbor_base) {
                    continue;
                }
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
    // Деревянные мосты требуют отдельной фазы раскладки: соседний мост может
    // согнуть исходную ось гекса. Именно так оригинальный TRACK_AWAY соединяет
    // разные коды вроде Bw/ и Bw| в одну непрерывную конструкцию.
    result.extend(wood_bridge_visuals(map));
    let ranged_mountains = add_mountain_ranges(map, &mut result);
    result.retain(|tile| {
        !tile.image.starts_with("mountains-single-")
            || !ranged_mountains.contains(&(tile.position.x, tile.position.y))
    });
    // Стены всех C*/K* строятся одним обходом; семейство спрайтов выбирается
    // по точному WML-коду замка или руин.
    add_castle_walls(map, &mut result);
    // Второй проход исполняет правила для воды и каменных мостов. После этого
    // все элементы карты сортируются единой шкалой слоёв, а не по гексам.
    result.extend(rule_visuals(map));
    result.sort_by_key(|tile| (tile.layer, visual_depth(tile)));
    result
}

fn visual_depth(tile: &VisualTile) -> (u8, i64) {
    if tile.image.starts_with("mountains-single-") {
        return (0, hex_screen_y(tile.position) - 18);
    }
    if matches!(tile.kind, VisualKind::MountainRange) {
        let anchor_y = mountain_range_anchor(tile.image).unwrap().1 as i64;
        return (
            0,
            hex_screen_y(tile.position) - anchor_y + mountain_range_base_y(tile.image).unwrap(),
        );
    }
    let corner = match tile.kind {
        VisualKind::CastleConvex(corner)
        | VisualKind::CastleConcave(corner)
        | VisualKind::KeepConvex(corner)
        | VisualKind::KeepConcave(corner) => corner,
        _ => return (0, i64::MIN),
    };
    let keep_pass = u8::from(matches!(
        tile.kind,
        VisualKind::KeepConvex(_) | VisualKind::KeepConcave(_)
    ));
    let anchor_y = castle_wall_anchor(corner).1 as i64;
    (keep_pass, hex_screen_y(tile.position) - anchor_y)
}

fn hex_screen_y(position: Position) -> i64 {
    (position.y - 1) * 72 - i64::from(position.x % 2 == 0) * 36
}

fn rule_managed_base(base: &str) -> bool {
    base == "Gg" || matches!(base, "Wo" | "Ww" | "Wwf" | "Wwg")
}

fn rule_visuals(map: &Map) -> Vec<VisualTile> {
    // include_str! встраивает WML в бинарник. OnceLock гарантирует, что парсер
    // работает один раз, а не каждый кадр; compose пока всё ещё выполняется
    // при каждом build_visuals и является отдельной целью для оптимизации.
    static RULES: OnceLock<Vec<TerrainRule>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        load_rules(include_str!("../scripts/terrain/graphics.wml"))
            .expect("bundled terrain rules must be valid")
    });
    compose(map, rules, 0)
        .into_iter()
        .filter_map(|image| {
            // В WML хранятся конкретные имена с суффиксом направления, а
            // VisualTile разделяет их на семейство текстур и индекс 0..5.
            // Неизвестное имя здесь отбрасывается и на экран не попадёт.
            let (name, kind) = match image.path.as_str() {
                "grass-green" => ("grass-green", VisualKind::Base),
                "water" => ("water", VisualKind::Base),
                "ocean" => ("ocean", VisualKind::Base),
                "dirt" => ("dirt", VisualKind::Base),
                "stone-bridge-n-s" => ("stone-bridge-n-s", VisualKind::Overlay),
                "stone-bridge-ne-sw" => ("stone-bridge-ne-sw", VisualKind::Overlay),
                "stone-bridge-se-nw" => ("stone-bridge-se-nw", VisualKind::Overlay),
                // Деревянные мосты уже собраны wood_bridge_visuals. Старые
                // прямолинейные WML-правила оставлены как справочник, но их
                // результат здесь намеренно не принимается.
                path if path.starts_with("wood-bridge") => return None,
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

fn direction(path: &str, prefix: &str) -> Option<usize> {
    // Этот порядок обязан совпадать с массивами текстур в AndroidArt и с
    // порядком neighbors(). Любая перестановка даст зеркальные стороны.
    ["n", "ne", "se", "s", "sw", "nw"]
        .iter()
        .position(|name| *name == path.trim_start_matches(prefix))
}

fn add_castle_walls(map: &Map, result: &mut Vec<VisualTile>) {
    let mut flags = BTreeSet::new();
    // Крепость первой занимает общие углы, заменяя на них башни замка.
    // При финальной сортировке вся крепость всё равно рисуется после стен.
    for keep_pass in [true, false] {
        for convex in [true, false] {
            for y in 1..=map.height as i64 {
                for x in 1..=map.width as i64 {
                    let position = Position { x, y };
                    let base = visual_codes(map.raw(position).unwrap_or("Gg")).0;
                    let belongs = if keep_pass {
                        is_raised_keep(base)
                    } else {
                        is_castle(base)
                    };
                    if belongs != convex {
                        continue;
                    }
                    let adjacent = neighbors(position);
                    let neighbor_bases = adjacent
                        .map(|neighbor| map.raw(neighbor).ok().map(|value| visual_codes(value).0));
                    let connected = neighbor_bases.map(|value| {
                        value.is_some_and(|base| {
                            if keep_pass {
                                is_raised_keep(base)
                            } else {
                                is_castle(base)
                            }
                        })
                    });
                    let keep = neighbor_bases.map(|value| value.is_some_and(is_keep));
                    for corner in 0..6 {
                        let next = (corner + 1) % 6;
                        let matches = if convex {
                            !connected[corner] && !connected[next]
                        } else {
                            connected[corner] && connected[next]
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
                        let wall_base = if convex {
                            base
                        } else {
                            neighbor_bases[corner]
                                .or(neighbor_bases[next])
                                .unwrap_or("C")
                        };
                        let family = castle_wall_family(wall_base);
                        let use_keep = keep_pass || is_keep(base) || keep[corner] || keep[next];
                        result.push(VisualTile {
                            position,
                            image: match (keep_pass, family, use_keep, convex) {
                                (true, "ruin", _, true) => "ruinkeep1-convex",
                                (true, "ruin", _, false) => "ruinkeep1-concave",
                                (true, _, _, true) => "keep-convex",
                                (true, _, _, false) => "keep-concave",
                                (_, "encampment", _, true) => "encampment-convex",
                                (_, "encampment", _, false) => "encampment-concave",
                                (_, "ruin", _, true) => "ruin-convex",
                                (_, "ruin", _, false) => "ruin-concave",
                                (_, "sunken-ruin", _, true) => "sunken-ruin-convex",
                                (_, "sunken-ruin", _, false) => "sunken-ruin-concave",
                                (_, _, true, true) => "keep-convex",
                                (_, _, true, false) => "keep-concave",
                                (_, _, false, true) => "castle-convex",
                                (_, _, false, false) => "castle-concave",
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
}

fn castle_wall_family(base: &str) -> &'static str {
    match base {
        "Ce" | "Ke" => "encampment",
        "Chr" | "Khr" => "ruin",
        "Chw" => "sunken-ruin",
        _ => "castle",
    }
}

fn add_mountain_ranges(map: &Map, result: &mut Vec<VisualTile>) -> BTreeSet<(i64, i64)> {
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
                    for position in [start, second, third] {
                        claimed.insert((position.x, position.y));
                    }
                    for image in images {
                        result.push(VisualTile {
                            position: start,
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
    claimed
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
    for image in images.iter().copied() {
        result.push(VisualTile {
            position: positions[0],
            image,
            kind: VisualKind::MountainRange,
            layer: 8,
        });
    }
    true
}

pub fn mountain_range_anchor(image: &str) -> Option<(f32, f32)> {
    Some(match image {
        value if value.starts_with("mountain-long-se-") => (90.0, 144.0),
        value if value.starts_with("mountain-long-ne-") => (90.0, 216.0),
        value if value.starts_with("mountain-range-se-") => (90.0, 144.0),
        value if value.starts_with("mountain-range-ne-") => (90.0, 216.0),
        value if value.starts_with("mountain-cluster-") => (90.0, 144.0),
        _ => return None,
    })
}

fn mountain_range_base_y(image: &str) -> Option<i64> {
    let index = image.rsplit_once('-')?.1.parse::<usize>().ok()? - 1;
    Some(match image {
        value if value.starts_with("mountain-long-se-") => [107, 107, 73, 108, 144][index],
        value if value.starts_with("mountain-long-ne-") => [144, 108, 73, 107, 107][index],
        value if value.starts_with("mountain-range-se-") => [107, 107, 144][index],
        value if value.starts_with("mountain-range-ne-") => [144, 107, 107][index],
        value if value.starts_with("mountain-cluster-") => [107, 107, 107][index],
        _ => return None,
    })
}

fn is_mountain(map: &Map, position: Position) -> bool {
    map.raw(position)
        .is_ok_and(|value| visual_codes(value).0 == "Mm")
}

fn mountain_single_image(position: Position) -> &'static str {
    [
        "mountains-single-1",
        "mountains-single-2",
        "mountains-single-3",
    ][((position.x * 17 + position.y * 31).unsigned_abs() % 3) as usize]
}

fn is_castle(base: &str) -> bool {
    base.starts_with('C') || base.starts_with('K') || matches!(base, "castle" | "keep")
}

fn is_keep(base: &str) -> bool {
    base.starts_with('K') || base == "keep"
}

fn is_raised_keep(base: &str) -> bool {
    is_keep(base) && !base.starts_with("Ke")
}

pub fn visual_codes(code: &str) -> (&str, &str) {
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
        "Hd" => "hills-dry",
        "Mm" => "mountains-single-1",
        "Ss" => "swamp",
        "Sm" => "swamp-mud",
        "Wo" => "ocean",
        "Wwr" | "Wwrg" => "reef-gray",
        value if value.starts_with('W') => "water",
        "Ce" | "Ke" => "dirt",
        "Chr" => "stone-path",
        "Chw" => "sunken-cobbles",
        "Khr" => "keep-cobbles",
        "Cme" => "aquatic-camp-floor",
        "Cud" => "dwarven-castle-floor",
        "Kud" => "dwarven-keep-floor",
        "Cvr" => "elven-ruin-ground",
        "Kvr" => "elven-ruin-keep",
        value if value.starts_with('K') => "keep-ground",
        value if value.starts_with('C') => "castle-ground",
        "Rd" => "road-desert",
        "Rr" => "road-cobbles",
        "Iwo" => "interior-wood-ruined",
        "Uu" => "cave-floor",
        "Ql" => "lava",
        "Xu" => "cave-wall",
        "Xoa" => "ancient-wall",
        "Xos" => "stone-wall",
        "castle" => "castle-ground",
        "keep" => "keep-ground",
        _ => "grass-green",
    }
}

fn base_overlay_image(base: &str) -> Option<&'static str> {
    match base {
        "Ke" => Some("encampment-tent"),
        _ => None,
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
        value if value.starts_with("Fdw") && small_forest => "forest-winter-small",
        value if value.starts_with("Fmw") && small_forest => "forest-mixed-winter-small",
        value if value.starts_with("Fdw") => "forest-winter",
        value if value.starts_with("Fmw") => "forest-mixed-winter",
        value if value.starts_with("Fet") => "great-tree",
        value if value.starts_with("Vhcr") => "village-human-city-ruin",
        value if value.starts_with("Vhhr") => "village-human-hills-ruin",
        value if value.starts_with("Vhh") => "village-hills",
        value if value.starts_with("Vhr") => "village-human-ruin",
        value if value.starts_with("Vhs") => "village-swamp",
        value if value.starts_with("Vh") => "village-human",
        value if value.starts_with("Ve") => "village-elven",
        value if value.starts_with("Vwm") => "village-windmill",
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
        "Edt" => "detritus-trash",
        "Dr" => "rubble",
        "Ewf" => "water-flowers",
        "Ewl" => "water-lilies",
        "Ewsh" => "seashells",
        "Wkf" => "kelp",
        "Gvs" => "farm",
        "Wm" => "windmill",
        "Ecf" => "campfire",
        "Eb" => "brazier",
        "Ebn" => "brazier-lit",
        "Efs" => "wall-fire",
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
            let (base, overlay) = visual_codes(value);
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

fn wood_bridge_visuals(map: &Map) -> Vec<VisualTile> {
    let mut result = Vec::new();
    for y in 1..=map.height as i64 {
        for x in 1..=map.width as i64 {
            let position = Position { x, y };
            let (_, overlay) = visual_codes(map.raw(position).unwrap_or(""));
            if !overlay.starts_with("Bw") {
                continue;
            }

            let around = neighbors(position);
            let adjacent = around.map(|neighbor| {
                map.raw(neighbor)
                    .is_ok_and(|code| visual_codes(code).1.starts_with("Bw"))
            });
            let (first, second) = bridge_axis(overlay);
            let mut connected = [false; 6];
            connected[first] = true;
            connected[second] = true;

            // TRACK_AWAY оригинала: если соседний мост касается не штатного
            // выхода, ближайший свободный конец оси сгибается к общей грани.
            // Обработка на обоих гексах заодно даёт обратную связь TRACK_FINAL.
            for direction in 0..6 {
                if !adjacent[direction] || connected[direction] {
                    continue;
                }
                let distance = |axis: usize| {
                    let delta = axis.abs_diff(direction);
                    delta.min(6 - delta)
                };
                let displaced = if distance(first) < distance(second) {
                    first
                } else {
                    second
                };
                if !adjacent[displaced] {
                    connected[displaced] = false;
                }
                connected[direction] = true;
            }

            result.push(VisualTile {
                position,
                image: wood_bridge_image(overlay, connected),
                kind: VisualKind::Overlay,
                layer: -10,
            });
            for direction in 0..6 {
                if !connected[direction] || adjacent[direction] {
                    continue;
                }
                let water = map
                    .raw(around[direction])
                    .is_ok_and(|code| is_water(visual_codes(code).0));
                result.push(VisualTile {
                    // В оригинальном TRACK_BORDER изображение принадлежит
                    // соседнему береговому гексу и смотрит назад на мост.
                    // Если оставить якорь на мосту, короткий end/dock лежит
                    // внутри воды и визуально не дотягивается до берега.
                    position: around[direction],
                    image: if water {
                        "wood-bridge-dock"
                    } else {
                        "wood-bridge-end"
                    },
                    kind: VisualKind::BridgeEnd((direction + 3) % 6),
                    layer: -9,
                });
            }
        }
    }
    result
}

fn wood_bridge_image(overlay: &str, connected: [bool; 6]) -> &'static str {
    let directions = connected
        .iter()
        .enumerate()
        .filter_map(|(direction, connected)| connected.then_some(direction))
        .collect::<Vec<_>>();
    match directions.as_slice() {
        [0, 2] => "wood-bridge-n-se",
        [1, 3] => "wood-bridge-ne-s",
        [2, 4] => "wood-bridge-se-sw",
        [3, 5] => "wood-bridge-s-nw",
        [0, 4] => "wood-bridge-sw-n",
        [1, 5] => "wood-bridge-nw-ne",
        [0, 2, 4] => "wood-bridge-n-se-sw",
        [1, 3, 5] => "wood-bridge-ne-s-nw",
        _ => bridge_image(overlay),
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
    // В оригинальной карте чётные 1-based столбцы подняты на полгекса.
    // Поэтому их диагональные соседи справа и слева лежат строкой выше.
    let up = if position.x % 2 == 0 { -1 } else { 0 };
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

/// Точка замкового гекса внутри большого PNG стены.
///
/// Оригинальный `NEW:CASTLEWALL_INTERNAL_P` рисует один спрайт по шаблону из
/// трёх гексов. После каждого поворота шаблон сдвигается к левому верхнему
/// углу, поэтому замковый гекс оказывается в разных местах изображения.
/// `base=54,72` отвечает только за порядок слоёв и не является этой точкой.
pub fn castle_wall_anchor(corner: usize) -> (f32, f32) {
    match corner % 6 {
        0 | 1 => (36.0, 108.0), // tr, r
        2 => (36.0, 36.0),      // br
        3 | 4 => (90.0, 72.0),  // bl, l
        _ => (90.0, 144.0),     // tl
    }
}

/// Обрезает большой замковый PNG той же гекс-маской, которой оригинальный
/// движок обрабатывает три клетки правила `NEW:CASTLEWALL_INTERNAL_P`.
pub fn mask_castle_wall(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    hex_mask: &[u8],
    corner: usize,
) {
    debug_assert_eq!(pixels.len(), width * height * 4);
    debug_assert_eq!(hex_mask.len(), 72 * 72 * 4);
    let (anchor_x, anchor_y) = castle_wall_anchor(corner);
    let directions = [
        (0, -72),
        (54, -36),
        (54, 36),
        (0, 72),
        (-54, 36),
        (-54, -36),
    ];
    let anchor = (anchor_x as i32, anchor_y as i32);
    let centers = [
        anchor,
        (
            anchor.0 + directions[corner % 6].0,
            anchor.1 + directions[corner % 6].1,
        ),
        (
            anchor.0 + directions[(corner + 1) % 6].0,
            anchor.1 + directions[(corner + 1) % 6].1,
        ),
    ];

    for y in 0..height {
        for x in 0..width {
            let mask_alpha = centers
                .iter()
                .filter_map(|&(center_x, center_y)| {
                    let mask_x = x as i32 - (center_x - 36);
                    let mask_y = y as i32 - (center_y - 36);
                    (0..72).contains(&mask_x).then_some(())?;
                    (0..72).contains(&mask_y).then_some(())?;
                    Some(hex_mask[((mask_y * 72 + mask_x) * 4 + 3) as usize])
                })
                .max()
                .unwrap_or(0);
            let alpha = &mut pixels[(y * width + x) * 4 + 3];
            *alpha = (*alpha).min(mask_alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn castle_wall_mask_keeps_only_the_three_rule_hexes() {
        let mut pixels = vec![255; 126 * 180 * 4];
        let mut mask = vec![0; 72 * 72 * 4];
        mask[(36 * 72 + 36) * 4 + 3] = 127;

        mask_castle_wall(&mut pixels, 126, 180, &mask, 0);

        assert_eq!(pixels[(108 * 126 + 36) * 4 + 3], 127);
        assert_eq!(pixels[(36 * 126 + 36) * 4 + 3], 127);
        assert_eq!(pixels[(72 * 126 + 90) * 4 + 3], 127);
        assert_eq!(pixels[(179 * 126 + 125) * 4 + 3], 0);
    }

    #[test]
    fn wesnoth_codes_keep_gameplay_and_visual_layers_separate() {
        assert_eq!(gameplay_type("Gs^Fms"), "forest");
        assert_eq!(gameplay_type("Ww^Bw/"), "grassland");
        assert_eq!(gameplay_type("Wo^Bw\\"), "grassland");
        assert_eq!(gameplay_type("Ww^Bsb|"), "grassland");
        assert_eq!(gameplay_type("1 Ke"), "keep");
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Gg^Vh".into(), "Ww".into()],
        };
        let visuals = build_visuals(&map);
        assert!(visuals.iter().any(|tile| tile.image == "village-human"));
        assert!(visuals.iter().any(|tile| {
            tile.position == Position { x: 2, y: 1 }
                && tile.image == "shore"
                && tile.kind == VisualKind::Transition(4)
        }));
        assert!(!visuals.iter().any(|tile| {
            tile.position == Position { x: 2, y: 1 } && tile.image == "transition-grass-green"
        }));
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

        let ruins = Map {
            width: 3,
            height: 1,
            cells: vec!["Chr".into(), "Khr".into(), "Chw".into()],
        };
        let visuals = build_visuals(&ruins);
        for image in [
            "stone-path",
            "keep-cobbles",
            "sunken-cobbles",
            "ruin-convex",
            "sunken-ruin-convex",
        ] {
            assert!(visuals.iter().any(|tile| tile.image == image));
        }
        assert!(
            build_visuals(&map)
                .iter()
                .any(|tile| tile.image == "encampment-tent")
        );
    }

    #[test]
    fn keep_inside_castle_gets_its_own_wall_ring() {
        let map = Map {
            width: 3,
            height: 3,
            cells: vec![
                "Chr".into(),
                "Chr".into(),
                "Chr".into(),
                "Chr".into(),
                "Khr".into(),
                "Chr".into(),
                "Chr".into(),
                "Chr".into(),
                "Chr".into(),
            ],
        };
        let visuals = build_visuals(&map);

        assert_eq!(
            visuals
                .iter()
                .filter(|tile| {
                    tile.position == Position { x: 2, y: 2 }
                        && tile.image == "ruinkeep1-convex"
                        && matches!(tile.kind, VisualKind::KeepConvex(_))
                })
                .count(),
            6
        );

        let edge_map = Map {
            width: 2,
            height: 1,
            cells: vec!["Khr".into(), "Chr".into()],
        };
        let edge_visuals = build_visuals(&edge_map);
        assert_eq!(
            edge_visuals
                .iter()
                .filter(|tile| {
                    tile.position == Position { x: 1, y: 1 } && tile.image == "ruinkeep1-convex"
                })
                .count(),
            6
        );
        assert!(!edge_visuals.iter().any(|tile| {
            tile.position == Position { x: 1, y: 1 }
                && matches!(
                    tile.kind,
                    VisualKind::CastleConvex(_) | VisualKind::CastleConcave(_)
                )
        }));
    }

    #[test]
    fn castle_walls_are_drawn_from_top_to_bottom() {
        let map = Map {
            width: 3,
            height: 3,
            cells: vec![
                "Ch".into(),
                "Ch".into(),
                "Ch".into(),
                "Ch".into(),
                "Kh".into(),
                "Ch".into(),
                "Ch".into(),
                "Ch".into(),
                "Ch".into(),
            ],
        };
        let walls = build_visuals(&map)
            .into_iter()
            .filter(|tile| {
                matches!(
                    tile.kind,
                    VisualKind::CastleConvex(_)
                        | VisualKind::CastleConcave(_)
                        | VisualKind::KeepConvex(_)
                        | VisualKind::KeepConcave(_)
                )
            })
            .collect::<Vec<_>>();

        assert!(walls.windows(2).all(|pair| {
            let left = visual_depth(&pair[0]);
            let right = visual_depth(&pair[1]);
            left <= right
        }));
        assert!(walls.windows(2).all(|pair| {
            !matches!(
                pair[0].kind,
                VisualKind::KeepConvex(_) | VisualKind::KeepConcave(_)
            ) || matches!(
                pair[1].kind,
                VisualKind::KeepConvex(_) | VisualKind::KeepConcave(_)
            )
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
        assert_eq!(ends, vec![3, 0]);
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
    fn adjacent_bridges_on_different_axes_form_one_bent_bridge() {
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Ww^Bw/".into(), "Ww^Bw|".into()],
        };
        let visuals = build_visuals(&map);
        assert!(visuals.iter().any(|tile| {
            tile.position == Position { x: 1, y: 1 } && tile.image == "wood-bridge-ne-sw"
        }));
        assert!(visuals.iter().any(|tile| {
            tile.position == Position { x: 2, y: 1 } && tile.image == "wood-bridge-sw-n"
        }));
        assert_eq!(
            visuals
                .iter()
                .filter(|tile| matches!(tile.kind, VisualKind::BridgeEnd(_)))
                .count(),
            2
        );
        assert!(visuals.iter().any(|tile| {
            tile.position == Position { x: 2, y: 0 } && tile.kind == VisualKind::BridgeEnd(3)
        }));
    }

    #[test]
    fn three_hex_bridge_from_first_scenario_is_continuous() {
        let map = Map {
            width: 3,
            height: 2,
            cells: vec![
                "Wo^Bw|".into(),
                "Ww".into(),
                "Ww".into(),
                "Ww".into(),
                "Wo^Bw\\".into(),
                "Ww^Bw\\".into(),
            ],
        };
        let visuals = build_visuals(&map);
        for (position, image) in [
            (Position { x: 1, y: 1 }, "wood-bridge-n-se"),
            (Position { x: 2, y: 2 }, "wood-bridge-se-nw"),
            (Position { x: 3, y: 2 }, "wood-bridge-se-nw"),
        ] {
            assert!(
                visuals
                    .iter()
                    .any(|tile| tile.position == position && tile.image == image)
            );
        }
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
        assert_eq!(ends, vec!["wood-bridge-end", "wood-bridge-dock"]);
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
    fn sand_uses_beach_on_every_edge_touching_shallow_water() {
        let map = Map {
            width: 2,
            height: 2,
            cells: vec!["Ds".into(), "Ww".into(), "Ww".into(), "Ds".into()],
        };
        let visuals = build_visuals(&map);
        let beach_edges = visuals
            .iter()
            .filter(|tile| tile.image == "transition-beach")
            .count();

        assert_eq!(beach_edges, 4);
        assert!(!visuals.iter().any(|tile| tile.image == "shore"));
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
                .filter(|tile| tile.image.starts_with("mountains-single-"))
                .count(),
            3
        );
        assert!(
            !visuals
                .iter()
                .any(|tile| matches!(tile.kind, VisualKind::MountainRange))
        );
    }

    #[test]
    fn mountain_range_layers_share_one_geometric_anchor() {
        let map = Map {
            width: 3,
            height: 2,
            cells: vec![
                "Mm".into(),
                "Gg".into(),
                "Gg".into(),
                "Gg".into(),
                "Mm".into(),
                "Mm".into(),
            ],
        };
        let ranges = build_visuals(&map)
            .into_iter()
            .filter(|tile| matches!(tile.kind, VisualKind::MountainRange))
            .collect::<Vec<_>>();

        assert_eq!(ranges.len(), 3);
        assert!(
            ranges
                .iter()
                .all(|tile| tile.position == ranges[0].position)
        );
        assert!(
            ranges
                .iter()
                .all(|tile| mountain_range_anchor(tile.image) == Some((90.0, 144.0)))
        );

        let mountain_depths = build_visuals(&map)
            .iter()
            .filter(|tile| {
                tile.image.starts_with("mountains-single-")
                    || matches!(tile.kind, VisualKind::MountainRange)
            })
            .map(visual_depth)
            .collect::<Vec<_>>();
        assert!(mountain_depths.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn stone_path_extends_into_neighboring_ground() {
        let map = Map {
            width: 3,
            height: 1,
            cells: vec!["Gg".into(), "Rp".into(), "Gg".into()],
        };
        let visuals = build_visuals(&map);

        assert_eq!(
            visuals
                .iter()
                .filter(|tile| {
                    tile.position != Position { x: 2, y: 1 }
                        && tile.image == "transition-grass-green"
                })
                .count(),
            0
        );
        assert!(visuals.iter().any(|tile| {
            tile.position != Position { x: 2, y: 1 } && tile.image == "transition-stone-path"
        }));
    }

    #[test]
    fn every_two_brothers_terrain_family_has_a_visual() {
        for base in [
            "Hd", "Sm", "Wwr", "Wwrg", "Cme", "Cud", "Kud", "Cvr", "Kvr", "Rd", "Rr", "Iwo", "Uu",
            "Ql", "Xu", "Xoa", "Xos",
        ] {
            assert_ne!(
                base_image(base),
                "grass-green",
                "missing base visual for {base}"
            );
        }
        for overlay in [
            "Fdw", "Fmw", "Fet", "Vhr", "Vhs", "Ve", "Vwm", "Edt", "Dr", "Ewl", "Ewsh", "Wkf",
            "Ecf", "Eb", "Ebn", "Efs",
        ] {
            assert!(
                overlay_image(overlay, Position { x: 1, y: 1 }, false).is_some(),
                "missing overlay visual for {overlay}"
            );
        }
    }
}
