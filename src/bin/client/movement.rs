use crate::connection::Connection;
use crate::map_renderer::hex_center;
use macroquad::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use wesnoth_engine::{engine::Position, game::Game, value::Value};

#[derive(Default)]
pub struct MovePreview {
    unit: String,
    revision: u64,
    path: Vec<Position>,
}

impl MovePreview {
    pub fn path(&self) -> &[Position] {
        &self.path
    }
    pub fn clear(&mut self) {
        self.path.clear();
    }

    /// Returns a move command only on a second tap of the current endpoint.
    /// Invalid taps leave the existing preview untouched and do not query Lua.
    pub fn tap(
        &mut self,
        connection: &mut Connection,
        game: &mut Game,
        unit: &str,
        origin: Position,
        target: Position,
        reachable: &BTreeSet<(i64, i64)>,
    ) -> Result<Option<Value>, String> {
        if origin == target || !reachable.contains(&(target.x, target.y)) {
            return Ok(None);
        }
        let command = Value::Map(BTreeMap::from([
            ("object".into(), Value::String(unit.into())),
            ("destination".into(), position_value(target)),
        ]));
        if self.unit == unit
            && self.revision == connection.world_revision()
            && self.path.first() == Some(&origin)
            && self.path.last() == Some(&target)
        {
            self.clear();
            return Ok(Some(command));
        }
        let response = connection.query(game, "reachable", command.clone())?;
        let Value::List(cells) = response else {
            return Ok(None);
        };
        let Some(cell) = cells
            .iter()
            .find(|cell| cell.get("position").and_then(position) == Some(target))
        else {
            return Ok(None);
        };
        let Some(Value::List(steps)) = cell.get("path") else {
            return Err("movement route has no path".into());
        };
        let mut path = vec![origin];
        for step in steps {
            path.push(position(step).ok_or("invalid movement route position")?);
        }
        if path.len() < 2 || path.last() != Some(&target) {
            return Err("movement route does not reach destination".into());
        }
        self.unit = unit.into();
        self.revision = connection.world_revision();
        self.path = path;
        Ok(None)
    }
}

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
fn position_value(p: Position) -> Value {
    Value::Map(BTreeMap::from([
        ("x".into(), Value::Integer(p.x)),
        ("y".into(), Value::Integer(p.y)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn game() -> Game {
        let mut game = Game::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/outpost_defense.wml",
        )
        .unwrap();
        game.acknowledge_dialog().unwrap();
        game
    }
    fn reachable(connection: &mut Connection, game: &mut Game) -> BTreeSet<(i64, i64)> {
        let response = connection
            .query(
                game,
                "reachable",
                Value::Map(BTreeMap::from([
                    ("object".into(), Value::String("eren".into())),
                    ("paths".into(), Value::Bool(false)),
                ])),
            )
            .unwrap();
        match response {
            Value::List(cells) => cells
                .iter()
                .filter_map(|cell| cell.get("position").and_then(position))
                .map(|p| (p.x, p.y))
                .collect(),
            _ => BTreeSet::new(),
        }
    }

    #[test]
    fn two_taps_move_and_invalid_taps_preserve_the_preview() {
        let mut game = game();
        let mut connection = Connection::default();
        let origin = Position { x: 2, y: 4 };
        let target = Position { x: 5, y: 2 };
        let allowed = reachable(&mut connection, &mut game);
        let mut preview = MovePreview::default();
        let saved = game.save().unwrap();
        assert!(
            preview
                .tap(&mut connection, &mut game, "eren", origin, target, &allowed)
                .unwrap()
                .is_none()
        );
        assert_eq!(preview.path().first(), Some(&origin));
        assert_eq!(preview.path().last(), Some(&target));
        let path = preview.path().to_vec();
        assert!(path.len() > 2);
        // Outside the map, occupied enemy cells and the selected cell are no-ops.
        for invalid in [
            Position { x: 100, y: 100 },
            Position { x: 10, y: 4 },
            origin,
        ] {
            assert!(
                preview
                    .tap(
                        &mut connection,
                        &mut game,
                        "eren",
                        origin,
                        invalid,
                        &allowed
                    )
                    .unwrap()
                    .is_none()
            );
            assert_eq!(preview.path(), path);
        }
        assert_eq!(
            game.save().unwrap(),
            saved,
            "preview must not change gameplay state"
        );
        let command = preview
            .tap(&mut connection, &mut game, "eren", origin, target, &allowed)
            .unwrap()
            .unwrap();
        assert!(preview.path().is_empty());
        let events = game.execute("move", command).unwrap();
        let event = events
            .iter()
            .find(|e| e.get("type").and_then(Value::as_str) == Some("object_moved"))
            .unwrap();
        assert_eq!(event.get("to").and_then(position), Some(target));
        let Value::List(actual_path) = event.get("path").unwrap() else {
            panic!("missing route")
        };
        assert_eq!(
            actual_path.iter().filter_map(position).collect::<Vec<_>>(),
            path[1..]
        );
        assert_eq!(
            event.get("movement_points"),
            Some(&Value::Integer(0)),
            "capturing a village consumes the remaining movement"
        );
        assert!(
            preview
                .tap(&mut connection, &mut game, "eren", target, origin, &allowed)
                .unwrap()
                .is_none()
        );
        assert!(preview.path().is_empty());

        let animation = MoveAnimation::from_event(event, 10.0).unwrap();
        assert_eq!(animation.unit, "eren");
        let final_direction = direction(
            hex_center(target.x, target.y)
                - hex_center(path[path.len() - 2].x, path[path.len() - 2].y),
        );
        assert_eq!(animation.facing(100.0), Some(final_direction));
        let Value::List(snapshot) = game.query("snapshot", Value::Nil).unwrap() else {
            panic!()
        };
        let unit = snapshot
            .iter()
            .find(|u| u.get("id").and_then(Value::as_str) == Some("eren"))
            .unwrap();
        assert_eq!(
            unit.get("facing").and_then(Value::as_str),
            Some(final_direction)
        );
        assert!(
            unit.get("defense")
                .unwrap()
                .get(wesnoth_engine::terrain::gameplay_type(
                    game.map().raw(target).unwrap(),
                ))
                .and_then(Value::as_i64)
                .is_some()
        );

        for (i, p) in path.iter().enumerate() {
            let actual = animation.position(10.0 + i as f64 * 0.14);
            assert!(actual.distance(hex_center(p.x, p.y)) < 0.001);
        }
        let midpoint = (hex_center(path[0].x, path[0].y) + hex_center(path[1].x, path[1].y)) / 2.0;
        assert!(animation.position(10.07).distance(midpoint) < 0.001);
        assert!(!animation.finished(10.0));
        assert!(animation.finished(100.0));
        assert_eq!(animation.position(100.0), hex_center(target.x, target.y));
    }

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

    #[test]
    fn retarget_cancel_and_revision_change_require_a_new_confirmation() {
        let mut game = game();
        let mut connection = Connection::default();
        let origin = Position { x: 2, y: 4 };
        let first = Position { x: 5, y: 2 };
        let second = Position { x: 3, y: 3 };
        let allowed = reachable(&mut connection, &mut game);
        let mut preview = MovePreview::default();
        assert!(
            preview
                .tap(&mut connection, &mut game, "eren", origin, first, &allowed)
                .unwrap()
                .is_none()
        );
        assert!(
            preview
                .tap(&mut connection, &mut game, "eren", origin, second, &allowed)
                .unwrap()
                .is_none()
        );
        assert_eq!(preview.path().last(), Some(&second));
        preview.clear();
        assert!(
            preview
                .tap(&mut connection, &mut game, "eren", origin, second, &allowed)
                .unwrap()
                .is_none()
        );
        connection.command(&mut game, "end_turn", Value::Nil);
        connection.receive();
        assert!(
            preview
                .tap(&mut connection, &mut game, "eren", origin, second, &allowed)
                .unwrap()
                .is_none(),
            "stale route cannot be confirmed"
        );
    }
}
