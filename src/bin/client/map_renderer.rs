use std::collections::BTreeSet;

use macroquad::prelude::*;
use wesnoth_engine::{
    engine::Position,
    terrain::{gameplay_type, visual_codes},
    value::Value,
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
    pub fn from_view(value: &Value) -> Result<Self, String> {
        if value.get("schema").and_then(Value::as_str) != Some("map") {
            return Err("map block has an unsupported schema".into());
        }
        let Value::List(items) = value.get("cells").ok_or("map block has no cells")? else {
            return Err("map cells must be a list".into());
        };
        let mut cells = Vec::with_capacity(items.len());
        for item in items {
            let position = item.get("position").ok_or("map cell has no position")?;
            let x = position
                .get("x")
                .and_then(Value::as_i64)
                .ok_or("map cell x must be an integer")?;
            let y = position
                .get("y")
                .and_then(Value::as_i64)
                .ok_or("map cell y must be an integer")?;
            let code = item
                .get("code")
                .and_then(Value::as_str)
                .ok_or("map cell code must be a string")?;
            let Value::List(channels) = item.get("color").ok_or("map cell has no color")? else {
                return Err("map cell color must be a list".into());
            };
            let [red, green, blue] = channels.as_slice() else {
                return Err("map cell color must have three channels".into());
            };
            let channel = |value: &Value| {
                value
                    .as_i64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| "map color channel must be in 0..255".to_owned())
            };
            cells.push(Cell {
                position: Position { x, y },
                center: hex_center(x, y),
                color: Color::from_rgba(channel(red)?, channel(green)?, channel(blue)?, 255),
                code: code.to_owned(),
            });
        }
        if cells.is_empty() {
            return Err("map block has no cells".into());
        }
        let min = cells.iter().fold(vec2(f32::MAX, f32::MAX), |min, cell| {
            min.min(cell.center - vec2(HEX_RADIUS, HEX_RADIUS))
        });
        let max = cells.iter().fold(vec2(f32::MIN, f32::MIN), |max, cell| {
            max.max(cell.center + vec2(HEX_RADIUS, HEX_RADIUS))
        });
        Ok(Self { cells, min, max })
    }

    pub fn tiles(&self) -> impl Iterator<Item = (Position, &str)> {
        self.cells
            .iter()
            .map(|cell| (cell.position, cell.code.as_str()))
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
                terrain_color(&cell.code),
            );
        }
    }
}

fn terrain_color(code: &str) -> Color {
    let (base, overlay) = visual_codes(code);
    let rgb = if overlay.starts_with('B') {
        [166, 140, 101]
    } else if overlay.starts_with('V') || code == "village" {
        [190, 156, 95]
    } else if overlay.starts_with('F') || code == "forest" {
        [36, 86, 42]
    } else if base.starts_with('K') || code == "keep" {
        [178, 154, 104]
    } else if base.starts_with('C') || code == "castle" {
        [141, 129, 112]
    } else if base.starts_with("Wo") {
        [28, 61, 108]
    } else if base.starts_with('S') {
        [75, 102, 78]
    } else if base.starts_with('D') {
        [202, 180, 123]
    } else if base.starts_with('A') {
        [203, 221, 228]
    } else if base.starts_with('M') {
        [125, 125, 126]
    } else if base.starts_with('R') {
        [156, 133, 103]
    } else {
        match gameplay_type(code) {
            "water" => [53, 107, 147],
            "hills" => [139, 117, 77],
            _ => [111, 145, 77],
        }
    };
    Color::from_rgba(rgb[0], rgb[1], rgb[2], 255)
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

pub(crate) fn point_in_hex(point: Vec2) -> bool {
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
    use std::path::Path;
    use wesnoth_engine::game::Game;

    #[test]
    fn presented_map_builds_the_client_cache() {
        let mut game = Game::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/first_battle.wml",
        )
        .unwrap();
        let snapshot = game.view_snapshot("map-test").unwrap();
        let block = snapshot
            .blocks
            .iter()
            .find(|block| block.id == "map")
            .unwrap();
        let map = MapRenderer::from_view(&block.content).unwrap();
        assert!(
            map.tiles()
                .any(|(position, code)| position == Position { x: 1, y: 1 } && code == "grassland")
        );
        assert_eq!(map.tiles().count(), 8 * 6);
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
