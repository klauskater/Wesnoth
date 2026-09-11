use macroquad::prelude::*;
use wesnoth_engine::{
    engine::{Map, Position},
    map::MapTiles,
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

    pub fn draw_base(&self, viewport: &MapViewport) {
        let screen = vec2(screen_width(), screen_height());
        for cell in &self.cells {
            let center = viewport.project(cell.center, screen);
            draw_hex(center, HEX_RADIUS * viewport.zoom(), cell.color);
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

    pub fn bounds(&self) -> (Vec2, Vec2) {
        (self.min, self.max)
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
