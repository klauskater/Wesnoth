//! Отдельное окно для отладки отрисовки террейна без запуска сценария.
//!
//! Запуск из корня проекта:
//! `cargo run --bin terrain-viewer -- rooting_out_a_mage`
//! Также можно передать полный или относительный путь к WML-файлу карты.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::PathBuf,
};

use macroquad::prelude::*;
use wesnoth_engine::{
    engine::Map,
    map::load_map,
    terrain::{
        VisualKind, VisualTile, build_visuals, castle_wall_anchor, mask_castle_wall,
        mountain_range_anchor,
    },
    wml,
};

const HEX_RADIUS: f32 = 36.0;
const PANEL_WIDTH: f32 = 400.0;
const TOP_BAR_HEIGHT: f32 = 34.0;
const PANEL_ROW_HEIGHT: f32 = 92.0;
const DIRECTIONS: [&str; 6] = ["n", "ne", "se", "s", "sw", "nw"];

fn window_conf() -> Conf {
    Conf {
        window_title: "Wesnoth terrain viewer".into(),
        window_width: 1280,
        window_height: 800,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let argument = match std::env::args().nth(1) {
        Some(argument) => argument,
        None => {
            eprintln!("usage: cargo run --bin terrain-viewer -- <map-name-or-path>");
            return;
        }
    };
    let path = resolve_map_path(&argument);
    let map = match read_map(&path) {
        Ok(map) => map,
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            return;
        }
    };
    let visuals = build_visuals(&map);
    let sprite_entries = sprite_entries(&map, &visuals);
    let sprite_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../Wesnoth-upstream/data/core/images/terrain");
    let castle_hex_mask = Image::from_file_with_format(
        include_bytes!("../../../Wesnoth-upstream/data/core/images/terrain/alphamask.png"),
        Some(ImageFormat::Png),
    )
    .expect("bundled terrain mask must be a PNG");
    let mut textures = HashMap::new();
    for tile in &visuals {
        let Some(relative) = sprite_path(tile) else {
            eprintln!("no sprite mapping for {} {:?}", tile.image, tile.kind);
            continue;
        };
        if textures.contains_key(&relative) {
            continue;
        }
        let absolute = sprite_root.join(&relative);
        let texture = if let Some(corner) = castle_wall_corner(tile.kind) {
            match load_image(&absolute.to_string_lossy()).await {
                Ok(mut image) => {
                    let (width, height) = (image.width as usize, image.height as usize);
                    mask_castle_wall(
                        &mut image.bytes,
                        width,
                        height,
                        &castle_hex_mask.bytes,
                        corner,
                    );
                    Some(Texture2D::from_image(&image))
                }
                Err(error) => {
                    eprintln!("cannot load {}: {error}", absolute.display());
                    None
                }
            }
        } else {
            match load_texture(&absolute.to_string_lossy()).await {
                Ok(texture) => Some(texture),
                Err(error) => {
                    eprintln!("cannot load {}: {error}", absolute.display());
                    None
                }
            }
        };
        if let Some(texture) = texture {
            texture.set_filter(FilterMode::Nearest);
            textures.insert(relative, texture);
        }
    }

    let mut camera = Camera::fit(&map);
    let mut drag_point = None;
    let mut show_grid = true;
    let mut panel_scroll = 0.0_f32;
    loop {
        clear_background(Color::from_rgba(20, 23, 22, 255));

        let mouse = Vec2::from(mouse_position());
        let over_panel = mouse.x >= screen_width() - PANEL_WIDTH;
        if is_mouse_button_pressed(MouseButton::Left) && !over_panel {
            drag_point = Some(mouse);
        }
        if is_mouse_button_down(MouseButton::Left)
            && let Some(previous) = drag_point.replace(mouse)
        {
            camera.offset += mouse - previous;
        }
        if is_mouse_button_released(MouseButton::Left) {
            drag_point = None;
        }
        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            if over_panel {
                let content_height = sprite_entries.len() as f32 * PANEL_ROW_HEIGHT;
                let visible_height = (screen_height() - TOP_BAR_HEIGHT - 62.0).max(1.0);
                let max_scroll = (content_height - visible_height).max(0.0);
                // Как и масштаб, прокрутка не должна зависеть от того,
                // присылает система 1 или 120 единиц за одно деление колеса.
                panel_scroll = (panel_scroll - wheel.signum() * 32.0).clamp(0.0, max_scroll);
            } else {
                // На Windows одно деление колеса иногда приходит как 120,
                // поэтому используем только знак и делаем маленький шаг.
                camera.zoom_at(mouse, 1.06_f32.powf(wheel.signum()));
            }
        }
        if is_key_pressed(KeyCode::R) {
            camera = Camera::fit(&map);
        }
        if is_key_pressed(KeyCode::G) {
            show_grid = !show_grid;
        }

        for y in 1..=map.height as i64 {
            for x in 1..=map.width as i64 {
                let center = camera.project(hex_center(x, y));
                draw_poly(
                    center.x,
                    center.y,
                    6,
                    HEX_RADIUS * camera.scale,
                    0.0,
                    Color::from_rgba(45, 65, 40, 255),
                );
            }
        }
        for tile in &visuals {
            let Some(relative) = sprite_path(tile) else {
                continue;
            };
            let Some(texture) = textures.get(&relative) else {
                continue;
            };
            let center = camera.project(hex_center(tile.position.x, tile.position.y));
            let texture_size = vec2(texture.width(), texture.height());
            let anchor = terrain_anchor(tile.kind, tile.image, texture_size) * camera.scale;
            draw_texture_ex(
                texture,
                center.x - anchor.x,
                center.y - anchor.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(texture_size * camera.scale),
                    ..Default::default()
                },
            );
        }
        if show_grid {
            for y in 1..=map.height as i64 {
                for x in 1..=map.width as i64 {
                    let center = camera.project(hex_center(x, y));
                    draw_poly_lines(
                        center.x,
                        center.y,
                        6,
                        HEX_RADIUS * camera.scale,
                        0.0,
                        1.0,
                        Color::from_rgba(20, 28, 20, 180),
                    );
                }
            }
        }
        // Подпись берётся прямо из Map без split/нормализации, поэтому на
        // гексе видно ровно `Ww^Bw|`, `1 Ke` и прочие значения из WML-карты.
        for y in 1..=map.height as i64 {
            for x in 1..=map.width as i64 {
                let Some(code) = map.raw(wesnoth_engine::engine::Position { x, y }).ok() else {
                    continue;
                };
                let center = camera.project(hex_center(x, y));
                let font_size = (12.0 * camera.scale).clamp(7.0, 15.0);
                let dimensions = measure_text(code, None, font_size as u16, 1.0);
                draw_text(
                    code,
                    center.x - dimensions.width / 2.0,
                    center.y + dimensions.height / 2.0,
                    font_size,
                    Color::from_rgba(255, 224, 64, 255),
                );
            }
        }

        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            TOP_BAR_HEIGHT,
            Color::from_rgba(12, 14, 14, 225),
        );
        draw_text(
            format!(
                "{}  |  drag: LMB  zoom: wheel  fit: R  grid: G  |  {:.0}%",
                path.file_name().unwrap_or_default().to_string_lossy(),
                camera.scale * 100.0
            ),
            12.0,
            23.0,
            20.0,
            WHITE,
        );
        draw_sprite_panel(&sprite_entries, &textures, panel_scroll);
        next_frame().await;
    }
}

struct Camera {
    offset: Vec2,
    scale: f32,
}

impl Camera {
    fn fit(map: &Map) -> Self {
        let size = vec2(
            (map.width.saturating_sub(1) as f32) * 54.0 + 72.0,
            (map.height.saturating_sub(1) as f32) * 72.0 + 108.0,
        );
        let map_width = (screen_width() - PANEL_WIDTH).max(100.0);
        let scale = ((map_width - 40.0) / size.x)
            .min((screen_height() - TOP_BAR_HEIGHT - 40.0) / size.y)
            .clamp(0.2, 2.0);
        Self {
            offset: vec2(
                (map_width - size.x * scale) / 2.0,
                TOP_BAR_HEIGHT + (screen_height() - TOP_BAR_HEIGHT - size.y * scale) / 2.0,
            ),
            scale,
        }
    }

    fn project(&self, point: Vec2) -> Vec2 {
        self.offset + point * self.scale
    }

    fn zoom_at(&mut self, screen_point: Vec2, factor: f32) {
        let world_point = (screen_point - self.offset) / self.scale;
        self.scale = (self.scale * factor).clamp(0.15, 4.0);
        self.offset = screen_point - world_point * self.scale;
    }
}

fn resolve_map_path(argument: &str) -> PathBuf {
    let direct = PathBuf::from(argument);
    if direct.is_file() {
        return direct;
    }
    let name = if argument.ends_with(".wml") {
        argument.to_owned()
    } else {
        format!("{argument}.wml")
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts/maps")
        .join(name)
}

fn read_map(path: &PathBuf) -> Result<Map, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("map") {
        let rows = source
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                line.split(',')
                    .map(|cell| cell.trim().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let width = rows.first().map_or(0, Vec::len);
        if width == 0 || rows.iter().any(|row| row.len() != width) {
            return Err("raw map has inconsistent row widths".to_owned());
        }
        return Ok(Map {
            width,
            height: rows.len(),
            cells: rows.into_iter().flatten().map(Into::into).collect(),
        });
    }
    let document = wml::parse(&source)?;
    let node = document
        .iter()
        .find(|node| node.name == "map")
        .ok_or_else(|| "file has no [map] root".to_owned())?;
    load_map(node)
}

#[derive(Debug, PartialEq, Eq)]
struct SpriteEntry {
    relative: String,
    image: &'static str,
    codes: Vec<String>,
    count: usize,
}

fn sprite_entries(map: &Map, visuals: &[VisualTile]) -> Vec<SpriteEntry> {
    let mut entries: BTreeMap<String, (&'static str, BTreeSet<String>, usize)> = BTreeMap::new();
    for tile in visuals {
        let Some(relative) = sprite_path(tile) else {
            continue;
        };
        let code = map.raw(tile.position).unwrap_or("_offmap").to_owned();
        let entry = entries
            .entry(relative)
            .or_insert_with(|| (tile.image, BTreeSet::new(), 0));
        entry.1.insert(code);
        entry.2 += 1;
    }
    entries
        .into_iter()
        .map(|(relative, (image, codes, count))| SpriteEntry {
            relative,
            image,
            codes: codes.into_iter().collect(),
            count,
        })
        .collect()
}

fn draw_sprite_panel(sprites: &[SpriteEntry], textures: &HashMap<String, Texture2D>, scroll: f32) {
    let x = screen_width() - PANEL_WIDTH;
    let top = TOP_BAR_HEIGHT;
    let height = screen_height() - top;
    draw_rectangle(
        x,
        top,
        PANEL_WIDTH,
        height,
        Color::from_rgba(28, 30, 29, 250),
    );
    draw_line(
        x,
        top,
        x,
        screen_height(),
        2.0,
        Color::from_rgba(92, 96, 90, 255),
    );
    draw_text(
        format!("Rendered sprites ({})", sprites.len()),
        x + 14.0,
        top + 27.0,
        22.0,
        Color::from_rgba(238, 211, 144, 255),
    );
    draw_text(
        "preview / file / WML codes / uses",
        x + 14.0,
        top + 51.0,
        16.0,
        GRAY,
    );

    let list_top = top + 62.0;
    let visible_height = (screen_height() - list_top).max(1.0);
    for (index, sprite) in sprites.iter().enumerate() {
        let y = list_top + index as f32 * PANEL_ROW_HEIGHT - scroll;
        if y + PANEL_ROW_HEIGHT < list_top || y >= screen_height() {
            continue;
        }
        if index % 2 == 0 {
            draw_rectangle(
                x + 7.0,
                y,
                PANEL_WIDTH - 14.0,
                PANEL_ROW_HEIGHT,
                Color::from_rgba(39, 42, 40, 245),
            );
        }
        let preview = Rect::new(x + 13.0, y + 8.0, 76.0, 76.0);
        draw_rectangle(
            preview.x,
            preview.y,
            preview.w,
            preview.h,
            Color::from_rgba(17, 20, 19, 255),
        );
        if let Some(texture) = textures.get(&sprite.relative) {
            let scale = (preview.w / texture.width())
                .min(preview.h / texture.height())
                .min(1.0);
            let size = vec2(texture.width(), texture.height()) * scale;
            draw_texture_ex(
                texture,
                preview.x + (preview.w - size.x) / 2.0,
                preview.y + (preview.h - size.y) / 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(size),
                    ..Default::default()
                },
            );
        }
        let text_x = x + 101.0;
        draw_text(
            ellipsize(&sprite.relative, 39),
            text_x,
            y + 24.0,
            17.0,
            WHITE,
        );
        draw_text(
            ellipsize(&format!("key: {}", sprite.image), 42),
            text_x,
            y + 47.0,
            16.0,
            Color::from_rgba(178, 196, 178, 255),
        );
        draw_text(
            ellipsize(&format!("WML: {}", sprite.codes.join(", ")), 42),
            text_x,
            y + 70.0,
            16.0,
            GRAY,
        );
        let count_text = format!("x{}", sprite.count);
        let count_width = measure_text(&count_text, None, 18, 1.0).width;
        draw_text(
            &count_text,
            screen_width() - 18.0 - count_width,
            y + 86.0,
            18.0,
            Color::from_rgba(170, 205, 170, 255),
        );
    }

    let content_height = sprites.len() as f32 * PANEL_ROW_HEIGHT;
    let max_scroll = (content_height - visible_height).max(0.0);
    if max_scroll > 0.0 {
        let track_height = visible_height - 8.0;
        let thumb_height = (visible_height / content_height * track_height).max(28.0);
        let thumb_y = list_top + 4.0 + scroll / max_scroll * (track_height - thumb_height);
        draw_rectangle(
            screen_width() - 6.0,
            thumb_y,
            4.0,
            thumb_height,
            Color::from_rgba(180, 180, 170, 210),
        );
    }
}

fn ellipsize(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    value.chars().take(max_chars - 1).collect::<String>() + "…"
}

fn hex_center(x: i64, y: i64) -> Vec2 {
    let column = x as f32 - 1.0;
    let row = y as f32 - 1.0;
    vec2(
        36.0 + column * 54.0,
        72.0 + row * 72.0 - (x % 2 == 0) as u8 as f32 * 36.0,
    )
}

fn terrain_anchor(kind: VisualKind, image: &str, size: Vec2) -> Vec2 {
    match kind {
        VisualKind::CastleConvex(corner)
        | VisualKind::CastleConcave(corner)
        | VisualKind::KeepConvex(corner)
        | VisualKind::KeepConcave(corner) => {
            let (x, y) = castle_wall_anchor(corner);
            vec2(x, y)
        }
        VisualKind::Base if image.starts_with("mountains-single-") => vec2(90.0, 108.0),
        VisualKind::MountainRange => {
            let (x, y) = mountain_range_anchor(image).unwrap();
            vec2(x, y)
        }
        VisualKind::BridgeEnd(0 | 3) if image == "stone-bridge-end" => vec2(90.0, 108.0),
        VisualKind::BridgeEnd(1 | 4) if image == "stone-bridge-end" => vec2(63.0, 90.0),
        VisualKind::BridgeEnd(2 | 5) if image == "stone-bridge-end" => vec2(63.0, 126.0),
        _ => size / 2.0,
    }
}

fn castle_wall_corner(kind: VisualKind) -> Option<usize> {
    match kind {
        VisualKind::CastleConvex(corner)
        | VisualKind::CastleConcave(corner)
        | VisualKind::KeepConvex(corner)
        | VisualKind::KeepConcave(corner) => Some(corner),
        _ => None,
    }
}

fn sprite_path(tile: &VisualTile) -> Option<String> {
    let direction = |index: usize| DIRECTIONS.get(index).copied();
    let path = match tile.kind {
        VisualKind::Edge(index) => format!("water/coast-tropical-A01-{}.png", direction(index)?),
        VisualKind::Transition(index) => {
            format!("{}-{}.png", transition_stem(tile.image)?, direction(index)?)
        }
        VisualKind::TransitionRun(mask) => {
            let suffix =
                green_run_suffix(mask).unwrap_or(DIRECTIONS[mask.trailing_zeros().min(5) as usize]);
            format!("grass/green-{suffix}.png")
        }
        VisualKind::CastleConvex(corner)
        | VisualKind::CastleConcave(corner)
        | VisualKind::KeepConvex(corner)
        | VisualKind::KeepConcave(corner) => {
            format!("{}-{}.png", wall_stem(tile.image)?, castle_corner(corner)?)
        }
        VisualKind::BridgeEnd(index) => match tile.image {
            "stone-bridge-end" => format!("bridge/stonebridge-{}.png", direction(index)?),
            "wood-bridge-dock" => format!("bridge/wood-dock-{}.png", direction(index)?),
            _ => format!("bridge/wood-end-{}.png", direction(index)?),
        },
        VisualKind::MountainRange => mountain_range_path(tile.image)?.to_owned(),
        _ => simple_sprite(tile.image)?.to_owned(),
    };
    Some(path)
}

fn mountain_range_path(image: &str) -> Option<&'static str> {
    Some(match image {
        "mountain-long-se-1" => "mountains/basic_range3_1.png",
        "mountain-long-se-2" => "mountains/basic_range3_2.png",
        "mountain-long-se-3" => "mountains/basic_range3_3.png",
        "mountain-long-se-4" => "mountains/basic_range3_4.png",
        "mountain-long-se-5" => "mountains/basic_range3_5.png",
        "mountain-long-ne-1" => "mountains/basic_range4_1.png",
        "mountain-long-ne-2" => "mountains/basic_range4_2.png",
        "mountain-long-ne-3" => "mountains/basic_range4_3.png",
        "mountain-long-ne-4" => "mountains/basic_range4_4.png",
        "mountain-long-ne-5" => "mountains/basic_range4_5.png",
        "mountain-range-se-1" => "mountains/basic_range1_1.png",
        "mountain-range-se-2" => "mountains/basic_range1_2.png",
        "mountain-range-se-3" => "mountains/basic_range1_3.png",
        "mountain-range-ne-1" => "mountains/basic_range2_1.png",
        "mountain-range-ne-2" => "mountains/basic_range2_2.png",
        "mountain-range-ne-3" => "mountains/basic_range2_3.png",
        "mountain-cluster-a-1" => "mountains/basic5_1.png",
        "mountain-cluster-a-2" => "mountains/basic5_2.png",
        "mountain-cluster-a-3" => "mountains/basic5_3.png",
        "mountain-cluster-b-1" => "mountains/basic6_1.png",
        "mountain-cluster-b-2" => "mountains/basic6_2.png",
        "mountain-cluster-b-3" => "mountains/basic6_3.png",
        _ => return None,
    })
}

fn castle_corner(corner: usize) -> Option<&'static str> {
    ["tl", "tr", "r", "br", "bl", "l"]
        .get((corner + 1) % 6)
        .copied()
}

fn wall_stem(image: &str) -> Option<&'static str> {
    Some(match image {
        "castle-convex" => "castle/castle-convex",
        "castle-concave" => "castle/castle-concave",
        "keep-convex" => "castle/keep-convex",
        "keep-concave" => "castle/keep-concave",
        "ruinkeep1-convex" => "castle/ruinkeep1-convex",
        "ruinkeep1-concave" => "castle/ruinkeep1-concave",
        "encampment-convex" => "castle/encampment/regular-convex",
        "encampment-concave" => "castle/encampment/regular-concave",
        "ruin-convex" => "castle/ruin-convex",
        "ruin-concave" => "castle/ruin-concave",
        "sunken-ruin-convex" => "castle/sunken-ruin-convex",
        "sunken-ruin-concave" => "castle/sunken-ruin-concave",
        _ => return None,
    })
}

fn transition_stem(image: &str) -> Option<&'static str> {
    Some(match image {
        "transition-beach" => "sand/beach",
        "transition-grass-dry" => "grass/dry",
        "transition-grass-semi-dry" => "grass/semi-dry",
        "transition-grass-green" => "grass/green",
        "transition-leaf-litter" => "grass/leaf-litter",
        "transition-dirt" => "flat/dirt",
        "transition-stone-path" => "flat/stone-path",
        "transition-hills" => "hills/regular",
        "shore" => "hills/dry-to-water",
        "transition-swamp" => "swamp/water",
        "transition-ocean" => "water/ocean-A01",
        _ => return None,
    })
}

fn green_run_suffix(mask: u8) -> Option<&'static str> {
    Some(match mask {
        1 => "n",
        2 => "ne",
        4 => "se",
        8 => "s",
        16 => "sw",
        32 => "nw",
        3 => "n-ne",
        6 => "ne-se",
        12 => "se-s",
        24 => "s-sw",
        48 => "sw-nw",
        33 => "nw-n",
        7 => "n-ne-se",
        14 => "ne-se-s",
        28 => "se-s-sw",
        56 => "s-sw-nw",
        49 => "sw-nw-n",
        35 => "nw-n-ne",
        59 => "s-sw-nw-n-ne",
        55 => "sw-nw-n-ne-se",
        63 => "s-sw-nw-n-ne-se",
        _ => return None,
    })
}

fn simple_sprite(image: &str) -> Option<&'static str> {
    Some(match image {
        "grass-green" => "grass/green.png",
        "grass-semi-dry" => "grass/semi-dry.png",
        "grass-dry" => "grass/dry.png",
        "leaf-litter" => "grass/leaf-litter.png",
        "dirt" => "flat/dirt.png",
        "stone-path" => "flat/stone-path.png",
        "beach" => "sand/beach.png",
        "hills-regular" => "hills/regular.png",
        "hills-dry" => "hills/dry.png",
        "mountains-single-1" => "mountains/basic.png",
        "mountains-single-2" => "mountains/basic2.png",
        "mountains-single-3" => "mountains/basic3.png",
        "mountain-wall" => "mountains/basic-castle-n.png",
        "swamp" => "swamp/water.png",
        "swamp-mud" => "swamp/mud.png",
        "ocean" => "water/ocean-A01.png",
        "water" => "water/coast-tile.png",
        "reef-gray" => "water/reef-gray-tile.png",
        "castle-ground" => "castle/castle-tile.png",
        "keep-ground" => "castle/keep-tile.png",
        "sunken-cobbles" => "castle/aquatic-castle/cobbles.png",
        "keep-cobbles" => "castle/cobbles-keep.png",
        "aquatic-camp-floor" => "castle/aquatic-camp/floor.png",
        "dwarven-castle-floor" => "castle/dwarven-castle-floor.png",
        "dwarven-keep-floor" => "castle/dwarven-keep-floor.png",
        "elven-ruin-ground" => "castle/elven-ruin/grounds.png",
        "elven-ruin-keep" => "castle/elven-ruin/keep.png",
        "road-desert" => "flat/desert-road.png",
        "road-cobbles" => "flat/road.png",
        "interior-wood-ruined" => "interior/wood-ruined.png",
        "cave-floor" => "cave/floor6.png",
        "lava" => "unwalkable/lava-A01.png",
        "cave-wall" => "cave/wall-rough-tile.png",
        "ancient-wall" => "walls/stone/ancient/wall-stone-tile.png",
        "stone-wall" => "walls/stone/wall-stone-tile.png",
        "encampment-tent" => "castle/encampment/tent.png",
        "forest-mixed-1" | "forest-mixed-3" => "forest/mixed-summer.png",
        "forest-mixed-2" => "forest/mixed-summer2.png",
        "forest-summer-1" => "forest/deciduous-summer.png",
        "forest-summer-2" => "forest/deciduous-summer2.png",
        "forest-summer-3" => "forest/deciduous-summer3.png",
        "forest-pine-1" => "forest/pine.png",
        "forest-pine-2" => "forest/pine2.png",
        "forest-pine-3" => "forest/pine3.png",
        "forest-mixed-small" => "forest/mixed-summer-small.png",
        "forest-summer-small" => "forest/deciduous-summer-small.png",
        "forest-pine-small" => "forest/pine-small.png",
        "forest-winter" => "forest/deciduous-winter.png",
        "forest-winter-small" => "forest/deciduous-winter-small.png",
        "forest-mixed-winter" => "forest/mixed-winter.png",
        "forest-mixed-winter-small" => "forest/mixed-winter-small.png",
        "great-tree" => "forest/great-tree.png",
        "village-human" => "village/human.png",
        "village-human-ruin" => "village/human-cottage-ruin.png",
        "village-human-city-ruin" => "village/human-city-ruin.png",
        "village-human-hills-ruin" => "village/human-hills-ruin.png",
        "village-hills" => "village/human-hills.png",
        "village-swamp" => "village/swampwater.png",
        "village-elven" => "village/elven.png",
        "village-windmill" => "misc/windmill-A01.png",
        "village-hut" => "village/hut.png",
        "village-log-cabin" => "village/log-cabin.png",
        "village-camp" => "village/camp.png",
        "wood-bridge-n-s" => "bridge/wood-n-s.png",
        "wood-bridge-ne-sw" => "bridge/wood-ne-sw.png",
        "wood-bridge-se-nw" => "bridge/wood-se-nw.png",
        "wood-bridge-n-se-sw" => "bridge/wood-n-se-sw.png",
        "wood-bridge-ne-s-nw" => "bridge/wood-ne-s-nw.png",
        "wood-bridge-n-se" => "bridge/wood-n-se.png",
        "wood-bridge-ne-s" => "bridge/wood-ne-s.png",
        "wood-bridge-se-sw" => "bridge/wood-se-sw.png",
        "wood-bridge-s-nw" => "bridge/wood-s-nw.png",
        "wood-bridge-sw-n" => "bridge/wood-sw-n.png",
        "wood-bridge-nw-ne" => "bridge/wood-nw-ne.png",
        "stone-bridge-n-s" => "bridge/stonebridge-n-s-tile.png",
        "stone-bridge-ne-sw" => "bridge/stonebridge-ne-sw-tile.png",
        "stone-bridge-se-nw" => "bridge/stonebridge-se-nw-tile.png",
        "flowers-mixed" | "flowers-farm" => "embellishments/flowers-mixed.png",
        "mushrooms" => "embellishments/mushroom.png",
        "stones" => "embellishments/stones-small.png",
        "detritus" => "misc/detritus/liter.png",
        "detritus-trash" => "misc/detritus/trashA-1.png",
        "rubble" => "misc/rubble.png",
        "water-flowers" => "embellishments/water-lilies-flower.png",
        "water-lilies" => "embellishments/water-lilies.png",
        "seashells" => "embellishments/seashells.png",
        "kelp" => "water/seaweed/kelp-1.png",
        "farm" => "embellishments/farm-veg-spring.png",
        "windmill" => "misc/windmill-A01.png",
        "campfire" => "misc/fire-A01.png",
        "brazier" => "misc/brazier-embellishment.png",
        "brazier-lit" => "misc/brazier-A01.png",
        "wall-fire" => "walls/stone/flames/flames-tile.png",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wesnoth_engine::engine::Position;

    #[test]
    fn resolves_directional_bridge_sprite() {
        let tile = VisualTile {
            position: Position { x: 1, y: 1 },
            image: "wood-bridge-dock",
            kind: VisualKind::BridgeEnd(4),
            layer: -9,
        };
        assert_eq!(
            sprite_path(&tile).as_deref(),
            Some("bridge/wood-dock-sw.png")
        );
    }

    #[test]
    fn every_bundled_map_visual_resolves_to_an_existing_sprite() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let sprite_root = project.join("../Wesnoth-upstream/data/core/images/terrain");
        for directory in ["scripts/maps", "scripts/maps/upstream-two-brothers"] {
            for entry in fs::read_dir(project.join(directory)).unwrap() {
                let path = entry.unwrap().path();
                if !matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("wml" | "map")
                ) {
                    continue;
                }
                let map = read_map(&path).unwrap();
                for tile in build_visuals(&map) {
                    let relative = sprite_path(&tile).unwrap_or_else(|| {
                        panic!("{}: no mapping for {}", path.display(), tile.image)
                    });
                    assert!(
                        sprite_root.join(&relative).is_file(),
                        "{}: missing {relative}",
                        path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn sprite_panel_groups_equal_files_and_keeps_source_codes() {
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Ww".into(), "Wo".into()],
        };
        let visuals = vec![
            VisualTile {
                position: Position { x: 1, y: 1 },
                image: "water",
                kind: VisualKind::Base,
                layer: -1000,
            },
            VisualTile {
                position: Position { x: 2, y: 1 },
                image: "water",
                kind: VisualKind::Base,
                layer: -1000,
            },
        ];
        let entries = sprite_entries(&map, &visuals);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative, "water/coast-tile.png");
        assert_eq!(entries[0].codes, ["Wo", "Ww"]);
        assert_eq!(entries[0].count, 2);
    }
}
