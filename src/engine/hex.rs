//! Hex-grid geometry without terrain or game semantics.
//!
//! Owns coordinates and conversions only. Contract:
//! `contracts/target/modules/engine/hex.md`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub width: usize,
    pub height: usize,
}

impl Bounds {
    pub fn new(width: usize, height: usize) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("hex bounds must be positive".into());
        }
        Ok(Self { width, height })
    }

    pub fn contains(self, position: Position) -> bool {
        position.x >= 1
            && position.y >= 1
            && position.x <= self.width as i64
            && position.y <= self.height as i64
    }

    pub fn neighbors(self, position: Position) -> Result<Vec<Position>, String> {
        if !self.contains(position) {
            return Err(format!(
                "position ({}, {}) is outside the bounds",
                position.x, position.y
            ));
        }
        Ok(neighbors(position)
            .into_iter()
            .filter(|candidate| self.contains(*candidate))
            .collect())
    }
}

pub fn neighbors(position: Position) -> [Position; 6] {
    let diagonal_y = if position.x % 2 == 0 { -1 } else { 1 };
    [
        (-1, 0),
        (-1, diagonal_y),
        (0, -1),
        (0, 1),
        (1, 0),
        (1, diagonal_y),
    ]
    .map(|(x, y)| Position {
        x: position.x + x,
        y: position.y + y,
    })
}

pub fn distance(a: Position, b: Position) -> i64 {
    fn axial(position: Position) -> (i64, i64, i64) {
        let q = position.x - 1;
        let row = position.y - 1;
        // WML uses 1-based upper odd-q coordinates.
        let r = row - (q + (q & 1)) / 2;
        (q, r, -q - r)
    }
    let a = axial(a);
    let b = axial(b);
    ((a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs()) / 2
}

pub fn adjacent(a: Position, b: Position) -> bool {
    distance(a, b) == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_is_consistent_at_edges_and_both_column_parities() {
        let bounds = Bounds::new(5, 3).unwrap();
        for position in [Position { x: 2, y: 2 }, Position { x: 3, y: 2 }] {
            let neighbors = bounds.neighbors(position).unwrap();
            assert_eq!(neighbors.len(), 6);
            assert!(
                neighbors
                    .iter()
                    .all(|neighbor| adjacent(position, *neighbor))
            );
        }
        assert_eq!(bounds.neighbors(Position { x: 1, y: 1 }).unwrap().len(), 3);
        assert_eq!(
            distance(Position { x: 1, y: 1 }, Position { x: 3, y: 1 }),
            2
        );
        assert!(bounds.neighbors(Position { x: 0, y: 1 }).is_err());
    }
}
