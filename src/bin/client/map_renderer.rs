use std::collections::BTreeSet;

use macroquad::prelude::*;
use wesnoth_engine::{
    engine::{Map, Position},
    map::MapTiles,
    terrain::{gameplay_type, visual_codes},
};

use crate::map_viewport::MapViewport;

const HEX_RADIUS: f32 = 1.0;

struct Cell {
    position: Position,
    center: Vec2,
    color: Color,
    code: String,
}

/// Immutable, client-side cache built once when a scenario is opened.
pub struct MapRenderer {
    cells: Vec<Cell>,
    min: Vec2,
    max: Vec2,
}

impl MapRenderer {
    pub fn new(map: &Map, tiles: &MapTiles) -> Self {
        let mut cells = Vec::with_capacity(map.cells.len());
        for y in 1..=map.height as i64 {
            for x in 1..=map.width as i64 {
                let code = map.raw(wesnoth_engine::engine::Position { x, y }).unwrap();
                let color = tiles
                    .get(code)
                    .map(|tile| Color::from_rgba(tile.color[0], tile.color[1], tile.color[2], 255))
                    .unwrap_or(MAGENTA);
                cells.push(Cell {
                    position: Position { x, y },
                    center: hex_center(x, y),
                    color,
                    code: code.to_owned(),
                });
            }
        }
        let min = cells.iter().fold(vec2(f32::MAX, f32::MAX), |min, cell| {
            min.min(cell.center - vec2(HEX_RADIUS, HEX_RADIUS))
        });
        let max = cells.iter().fold(vec2(f32::MIN, f32::MIN), |max, cell| {
            max.max(cell.center + vec2(HEX_RADIUS, HEX_RADIUS))
        });
        Self { cells, min, max }
    }

    pub fn draw_base(&self, viewport: &MapViewport, tint: Color) {
        let screen = vec2(screen_width(), screen_height());
        for cell in &self.cells {
            let center = viewport.project(cell.center, screen);
            let color = Color::new(
                cell.color.r * tint.r,
                cell.color.g * tint.g,
                cell.color.b * tint.b,
                cell.color.a,
            );
            draw_hex(center, HEX_RADIUS * viewport.zoom(), color);
        }
    }

    pub fn draw_unreachable(
        &self,
        viewport: &MapViewport,
        reachable: Option<&BTreeSet<(i64, i64)>>,
    ) {
        let Some(reachable) = reachable else {
            return;
        };
        let screen = vec2(screen_width(), screen_height());
        for cell in &self.cells {
            if !reachable.contains(&(cell.position.x, cell.position.y)) {
                draw_hex(
                    viewport.project(cell.center, screen),
                    HEX_RADIUS * viewport.zoom(),
                    Color::from_rgba(0, 0, 0, 145),
                );
            }
        }
    }

    pub fn draw_grid(&self, viewport: &MapViewport, selected: Option<Position>) {
        let screen = vec2(screen_width(), screen_height());
        for cell in &self.cells {
            let center = viewport.project(cell.center, screen);
            let selected = selected == Some(cell.position);
            draw_hex_lines(
                center,
                HEX_RADIUS * viewport.zoom(),
                if selected {
                    3.0
                } else {
                    (viewport.zoom() * 0.035).clamp(0.5, 1.5)
                },
                if selected {
                    GOLD
                } else {
                    Color::from_rgba(15, 20, 24, 110)
                },
            );
        }
    }

    pub fn draw_codes(&self, viewport: &MapViewport) {
        let screen = vec2(screen_width(), screen_height());
        let font_size = (viewport.zoom() * 0.34).clamp(10.0, 18.0) as u16;
        for cell in &self.cells {
            let center = viewport.project(cell.center, screen);
            let size = measure_text(&cell.code, None, font_size, 1.0);
            let x = center.x - size.width * 0.5;
            let y = center.y + size.height * 0.5;
            draw_text_ex(
                &cell.code,
                x + 1.0,
                y + 1.0,
                TextParams {
                    font_size,
                    color: Color::from_rgba(0, 0, 0, 210),
                    ..Default::default()
                },
            );
            draw_text_ex(
                &cell.code,
                x,
                y,
                TextParams {
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }
    }

    pub fn draw_route(
        &self,
        viewport: &MapViewport,
        path: &[Position],
        footsteps: &[Texture2D],
        defense: Option<i64>,
        font: &Font,
    ) {
        if path.len() < 2 {
            return;
        }
        let screen = vec2(screen_width(), screen_height());
        let radius = viewport.zoom();
        let project = |p: Position| viewport.project(hex_center(p.x, p.y), screen);
        for pair in path.windows(2) {
            let delta = hex_center(pair[1].x, pair[1].y) - hex_center(pair[0].x, pair[0].y);
            // Teleports have no walking trail across the intervening cells.
            if delta.length_squared() > 4.01 {
                continue;
            }
            let (index, flip) = match crate::movement::direction(delta) {
                "n" => (0, false),
                "ne" => (1, false),
                "se" => (2, false),
                "s" => (0, true),
                "sw" => (1, true),
                _ => (2, true),
            };
            for (p, half) in [(pair[0], 3), (pair[1], 0)] {
                let center = project(p);
                draw_texture_ex(
                    &footsteps[index + half],
                    center.x - radius,
                    center.y - radius,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                        flip_x: flip,
                        flip_y: flip,
                        ..Default::default()
                    },
                );
            }
        }
        let end = project(*path.last().unwrap());
        draw_hex_lines(end, radius * 0.85, 3.0, GOLD);
        if let Some(defense) = defense {
            let label = format!("{defense}%");
            let font_size = (radius * 0.55).clamp(14.0, 30.0) as u16;
            let size = measure_text(&label, Some(font), font_size, 1.0);
            let x = end.x - size.width / 2.0;
            let y = end.y + size.height / 2.0;
            draw_rectangle(
                x - 4.0,
                y - size.height - 3.0,
                size.width + 8.0,
                size.height + 6.0,
                Color::from_rgba(0, 0, 0, 210),
            );
            draw_text_ex(
                &label,
                x,
                y,
                TextParams {
                    font: Some(font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        (self.min, self.max)
    }

    pub fn draw_minimap(&self, rect: Rect) {
        let size = self.max - self.min;
        let scale = (rect.w / size.x).min(rect.h / size.y);
        let offset = vec2(rect.x, rect.y) + (vec2(rect.w, rect.h) - size * scale) / 2.0;
        for cell in &self.cells {
            draw_hex(
                offset + (cell.center - self.min) * scale,
                scale,
                terrain_info(&cell.code).1,
            );
        }
    }

    pub fn hex_at(&self, point: Vec2) -> Option<Position> {
        self.cells
            .iter()
            .filter(|cell| point_in_hex(point - cell.center))
            .min_by(|left, right| {
                left.center
                    .distance_squared(point)
                    .total_cmp(&right.center.distance_squared(point))
            })
            .map(|cell| cell.position)
    }
}

pub fn time_tint(time: &str) -> Color {
    match time {
        "dawn" => Color::from_rgba(230, 240, 255, 255),
        "dusk" => Color::from_rgba(255, 235, 220, 255),
        "first_watch" | "second_watch" => Color::new(
            180.0 / 255.0 * 0.9,
            210.0 / 255.0 * 0.9,
            242.0 / 255.0 * 0.9,
            1.0,
        ),
        _ => WHITE,
    }
}

pub fn terrain_info(code: &str) -> (&'static str, Color) {
    let (base, overlay) = visual_codes(code);
    let (name, rgb) = if overlay.starts_with('B') {
        ("Мост", [166, 140, 101])
    } else if overlay.starts_with('V') || code == "village" {
        ("Деревня", [190, 156, 95])
    } else if overlay.starts_with('F') || code == "forest" {
        ("Лес", [36, 86, 42])
    } else if base.starts_with('K') || code == "keep" {
        ("Цитадель", [178, 154, 104])
    } else if base.starts_with('C') || code == "castle" {
        ("Замок", [141, 129, 112])
    } else if base.starts_with("Wo") {
        ("Глубокая вода", [28, 61, 108])
    } else if base.starts_with('S') {
        ("Болото", [75, 102, 78])
    } else if base.starts_with('D') {
        ("Песок", [202, 180, 123])
    } else if base.starts_with('A') {
        ("Снег", [203, 221, 228])
    } else if base.starts_with('M') {
        ("Горы", [125, 125, 126])
    } else if base.starts_with('R') {
        ("Дорога", [156, 133, 103])
    } else {
        match gameplay_type(code) {
            "water" => ("Мелководье", [53, 107, 147]),
            "hills" => ("Холмы", [139, 117, 77]),
            _ => ("Равнина", [111, 145, 77]),
        }
    };
    (name, Color::from_rgba(rgb[0], rgb[1], rgb[2], 255))
}

pub fn hex_center(x: i64, y: i64) -> Vec2 {
    let column = x as f32 - 1.0;
    let row = y as f32 - 1.0;
    vec2(column * 1.5, row * 2.0 - if x % 2 == 0 { 1.0 } else { 0.0 })
}

fn vertices(center: Vec2, radius: f32) -> [Vec2; 6] {
    [
        center + vec2(-radius, 0.0),
        center + vec2(-radius * 0.5, -radius),
        center + vec2(radius * 0.5, -radius),
        center + vec2(radius, 0.0),
        center + vec2(radius * 0.5, radius),
        center + vec2(-radius * 0.5, radius),
    ]
}

fn point_in_hex(point: Vec2) -> bool {
    let point = point.abs();
    point.y <= HEX_RADIUS && point.x <= HEX_RADIUS - point.y * 0.5
}

fn draw_hex(center: Vec2, radius: f32, color: Color) {
    let vertices = vertices(center, radius);
    for index in 0..6 {
        draw_triangle(center, vertices[index], vertices[(index + 1) % 6], color);
    }
}

fn draw_hex_lines(center: Vec2, radius: f32, thickness: f32, color: Color) {
    let vertices = vertices(center, radius);
    for index in 0..6 {
        draw_line(
            vertices[index].x,
            vertices[index].y,
            vertices[(index + 1) % 6].x,
            vertices[(index + 1) % 6].y,
            thickness,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daylight_is_neutral_and_night_is_darker_than_twilight() {
        assert_eq!(time_tint("morning"), WHITE);
        assert_eq!(time_tint("afternoon"), WHITE);
        assert!(time_tint("dawn").b > time_tint("dawn").r);
        assert!(time_tint("dusk").r > time_tint("dusk").b);
        for night in ["first_watch", "second_watch"] {
            let tint = time_tint(night);
            assert!(tint.r < time_tint("dawn").r && tint.g < time_tint("dusk").g);
            assert_eq!(tint.a, 1.0);
        }
    }

    #[test]
    fn neighboring_columns_are_vertically_staggered() {
        assert_eq!(hex_center(1, 1), vec2(0.0, 0.0));
        assert_eq!(hex_center(2, 1), vec2(1.5, -1.0));
    }

    #[test]
    fn hit_test_rejects_a_point_outside_the_sloped_corner() {
        assert!(point_in_hex(vec2(0.0, 0.0)));
        assert!(!point_in_hex(vec2(0.9, 0.9)));
    }
}
