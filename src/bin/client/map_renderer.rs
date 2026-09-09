use macroquad::prelude::*;
use wesnoth_engine::{engine::Map, game::MapTiles};

const HEX_RADIUS: f32 = 1.0;

struct Cell {
    center: Vec2,
    color: Color,
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
                    center: hex_center(x, y),
                    color,
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

    pub fn draw(&self) {
        let size = self.max - self.min;
        let scale = ((screen_width() - 32.0) / size.x)
            .min((screen_height() - 32.0) / size.y)
            .max(1.0);
        let offset =
            (vec2(screen_width(), screen_height()) - size * scale) / 2.0 - self.min * scale;

        for cell in &self.cells {
            let center = offset + cell.center * scale;
            draw_hex(center, HEX_RADIUS * scale, cell.color);
            draw_hex_lines(
                center,
                HEX_RADIUS * scale,
                (scale * 0.035).clamp(0.5, 1.5),
                Color::from_rgba(15, 20, 24, 110),
            );
        }
    }
}

fn hex_center(x: i64, y: i64) -> Vec2 {
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
}
