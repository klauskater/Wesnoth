use crate::map_renderer::hex_center;
use macroquad::prelude::*;
use wesnoth_engine::{engine::Position, value::Value};

pub struct MoveAnimation {
    pub unit: String,
    points: Vec<Vec2>,
    started: f64,
    teleport: bool,
}

impl MoveAnimation {
    pub fn from_event(event: &Value, started: f64) -> Option<Self> {
        if event.get("type")?.as_str()? != "object_moved" {
            return None;
        }
        let from = event.get("from").and_then(position)?;
        let to = event.get("to").and_then(position)?;
        let mut points = vec![hex_center(from.x, from.y)];
        if let Some(Value::List(path)) = event.get("path") {
            for step in path {
                let p = position(step)?;
                points.push(hex_center(p.x, p.y));
            }
        }
        let end = hex_center(to.x, to.y);
        if points.last() != Some(&end) {
            points.push(end);
        }
        Some(Self {
            unit: event.get("object")?.as_str()?.into(),
            points,
            started,
            teleport: event.get("teleported") == Some(&Value::Bool(true)),
        })
    }

    fn duration(&self) -> f64 {
        if self.teleport {
            0.18
        } else {
            (self.points.len().saturating_sub(1) as f64 * 0.14).max(0.14)
        }
    }
    pub fn finished(&self, now: f64) -> bool {
        now - self.started >= self.duration()
    }
    pub fn facing(&self, now: f64) -> Option<&'static str> {
        if self.teleport || self.points.len() < 2 {
            return None;
        }
        let index = (((now - self.started).max(0.0) / 0.14) as usize).min(self.points.len() - 2);
        Some(direction(self.points[index + 1] - self.points[index]))
    }
    pub fn position(&self, now: f64) -> Vec2 {
        let progress = ((now - self.started) / self.duration()).clamp(0.0, 1.0) as f32;
        if self.teleport {
            return if progress < 0.5 {
                self.points[0]
            } else {
                *self.points.last().unwrap()
            };
        }
        let step = progress * (self.points.len() - 1) as f32;
        let index = step.floor() as usize;
        self.points[index].lerp(
            self.points[(index + 1).min(self.points.len() - 1)],
            step.fract(),
        )
    }
}

pub fn direction(delta: Vec2) -> &'static str {
    if delta.x == 0.0 {
        if delta.y < 0.0 { "n" } else { "s" }
    } else if delta.x > 0.0 {
        if delta.y < 0.0 { "ne" } else { "se" }
    } else if delta.y < 0.0 {
        "nw"
    } else {
        "sw"
    }
}

fn position(value: &Value) -> Option<Position> {
    Some(Position {
        x: value.get("x")?.as_i64()?,
        y: value.get("y")?.as_i64()?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_follows_all_six_directions_on_both_column_parities() {
        for x in [2, 3] {
            let origin = Position { x, y: 4 };
            let upper = if x % 2 == 0 { 3 } else { 4 };
            for (target, expected) in [
                (Position { x, y: 3 }, "n"),
                (Position { x: x + 1, y: upper }, "ne"),
                (
                    Position {
                        x: x + 1,
                        y: upper + 1,
                    },
                    "se",
                ),
                (Position { x, y: 5 }, "s"),
                (
                    Position {
                        x: x - 1,
                        y: upper + 1,
                    },
                    "sw",
                ),
                (Position { x: x - 1, y: upper }, "nw"),
            ] {
                let animation = MoveAnimation {
                    unit: "test".into(),
                    points: vec![
                        hex_center(origin.x, origin.y),
                        hex_center(target.x, target.y),
                    ],
                    started: 0.0,
                    teleport: false,
                };
                assert_eq!(animation.facing(0.07), Some(expected));
                assert_eq!(animation.facing(1.0), Some(expected));
            }
        }
    }
}
