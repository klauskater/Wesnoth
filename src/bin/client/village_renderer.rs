use std::collections::BTreeMap;

use crate::{map_renderer::hex_center, map_viewport::MapViewport};
use macroquad::prelude::*;
use wesnoth_engine::{
    engine::{Map, Position},
    terrain::{gameplay_type, visual_codes},
    value::Value,
};

mod art {
    include!(concat!(env!("OUT_DIR"), "/embedded_village_art.rs"));
}

pub struct VillageRenderer {
    buildings: Vec<(Position, Texture2D, Texture2D)>,
    flags: BTreeMap<String, Vec<Texture2D>>,
}

impl VillageRenderer {
    pub fn load(map: &Map, status: &Value) -> Result<Self, String> {
        let mut textures = BTreeMap::new();
        let mut buildings = Vec::new();
        for y in 1..=map.height as i64 {
            for x in 1..=map.width as i64 {
                let position = Position { x, y };
                let code = map.raw(position)?;
                if gameplay_type(code) != "village" {
                    continue;
                }
                let stem = village_stem(code);
                let choices = variants(stem);
                if choices.is_empty() {
                    return Err(format!("missing village art: {stem}"));
                }
                let id = choices[((x * 17 + y * 31) as usize) % choices.len()];
                let night_id = format!("{id}-night");
                let night_id = if asset(&night_id).is_some() {
                    night_id.as_str()
                } else {
                    id
                };
                let mut texture = |key: &str| -> Texture2D {
                    textures
                        .entry(key.to_owned())
                        .or_insert_with(|| {
                            let image = Texture2D::from_file_with_format(
                                asset(key).unwrap(),
                                Some(ImageFormat::Png),
                            );
                            image.set_filter(FilterMode::Nearest);
                            image
                        })
                        .clone()
                };
                buildings.push((position, texture(id), texture(night_id)));
            }
        }
        buildings.sort_by_key(|(p, _, _)| (p.y * 2 - i64::from(p.x % 2 == 0), p.x));
        let mut flags = BTreeMap::new();
        for side in items(status, "side_visuals") {
            let style = string(side, "flag_style");
            let prefix = if style == "default" {
                "flags/flag-".to_owned()
            } else {
                format!("flags/{style}-flag-")
            };
            let mut frames = Vec::new();
            for index in 1..=if style == "ragged" { 6 } else { 4 } {
                let key = format!("{prefix}{index}");
                let bytes = asset(&key).ok_or_else(|| format!("missing flag art: {key}"))?;
                let mut image = Image::from_file_with_format(bytes, Some(ImageFormat::Png))
                    .map_err(|e| e.to_string())?;
                recolor_flag(&mut image, string(side, "color"))?;
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Nearest);
                frames.push(texture);
            }
            flags.insert(string(side, "id").to_owned(), frames);
        }
        Ok(Self { buildings, flags })
    }

    pub fn draw(&self, viewport: &MapViewport, status: &Value, elapsed_ms: u64, tint: Color) {
        let screen = vec2(screen_width(), screen_height());
        let scale = viewport.zoom() / 36.0;
        let night = night_windows(string(status, "time_of_day"));
        for (position, day, dark) in &self.buildings {
            let center = viewport.project(hex_center(position.x, position.y), screen);
            let texture = if night { dark } else { day };
            let size = vec2(texture.width(), texture.height()) * scale;
            draw_texture_ex(
                texture,
                center.x - size.x / 2.0,
                center.y - size.y / 2.0,
                tint,
                DrawTextureParams {
                    dest_size: Some(size),
                    ..Default::default()
                },
            );
            let Some(owner) = village_owner(status, *position) else {
                continue;
            };
            let Some(frames) = self.flags.get(owner) else {
                continue;
            };
            let frame = &frames[flag_frame(elapsed_ms, *position, frames.len())];
            let size = vec2(frame.width(), frame.height()) * scale;
            draw_texture_ex(
                frame,
                center.x - size.x / 2.0,
                center.y - size.y / 2.0,
                tint,
                DrawTextureParams {
                    dest_size: Some(size),
                    ..Default::default()
                },
            );
        }
    }
}

fn asset(id: &str) -> Option<&'static [u8]> {
    art::ALL
        .iter()
        .find(|(key, _)| *key == id)
        .map(|(_, bytes)| *bytes)
}
fn variants(stem: &str) -> Vec<&'static str> {
    let prefix = format!("terrain/village/{stem}");
    art::ALL
        .iter()
        .filter_map(|(id, _)| {
            let suffix = id.strip_prefix(&prefix)?;
            (suffix.is_empty() || suffix.chars().all(|c| c.is_ascii_digit())).then_some(*id)
        })
        .collect()
}
fn village_stem(code: &str) -> &str {
    match visual_codes(code).1 {
        "Vhc" => "human-city",
        "Vhcr" => "human-city-ruin",
        "Vhh" => "human-hills",
        "Vhhr" => "human-hills-ruin",
        "Vc" => "hut",
        "Vct" => "camp",
        "Ve" => "elven",
        "Vl" => "log-cabin",
        _ => "human",
    }
}
fn night_windows(time: &str) -> bool {
    matches!(time, "dusk" | "first_watch" | "second_watch")
}
fn flag_frame(elapsed_ms: u64, position: Position, count: usize) -> usize {
    ((elapsed_ms / 150 + (position.x * 7 + position.y * 13) as u64) % count as u64) as usize
}
fn village_owner(status: &Value, position: Position) -> Option<&str> {
    if status.get("fog") == Some(&Value::Bool(true))
        && !items(status, "visible_cells")
            .iter()
            .any(|p| at(p, position))
    {
        return None;
    }
    items(status, "villages")
        .iter()
        .find(|v| at(v, position))?
        .get("side")?
        .as_str()
        .filter(|id| !id.is_empty())
}
fn at(value: &Value, position: Position) -> bool {
    value.get("x").and_then(Value::as_i64) == Some(position.x)
        && value.get("y").and_then(Value::as_i64) == Some(position.y)
}
fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match value.get(key) {
        Some(Value::List(values)) => values,
        _ => &[],
    }
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

// Original flag_green palette: pure green 1..255, with 00C800 as its reference.
fn recolor_flag(image: &mut Image, color: &str) -> Result<(), String> {
    let (mid, shadow) = match color {
        "red" | "1" => (0xFF0000, 0),
        "blue" | "2" => (0x2E419B, 15),
        "green" | "3" => (0x62B664, 0),
        "purple" | "4" => (0x93009D, 0),
        "black" | "5" => (0x5A5A5A, 0),
        "brown" | "6" => (0x945027, 0),
        "orange" | "7" => (0xFF7E00, 15),
        "white" | "8" => (0xE1E1E1, 30),
        "teal" | "9" => (0x30CBC0, 0),
        other => (
            u32::from_str_radix(other.trim_start_matches('#'), 16)
                .ok()
                .filter(|v| *v <= 0xFFFFFF)
                .ok_or_else(|| format!("unsupported side color: {other}"))?,
            0,
        ),
    };
    let reference = 200 / 3;
    for pixel in image.bytes.chunks_exact_mut(4) {
        if pixel[0] != 0 || pixel[2] != 0 || pixel[1] == 0 {
            continue;
        }
        let average = (u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2])) / 3;
        let (ratio, end) = if average <= reference {
            (average as f32 / reference as f32, shadow as f32)
        } else {
            ((255 - average) as f32 / (255 - reference) as f32, 255.0)
        };
        for (i, shift) in [16, 8, 0].iter().enumerate() {
            pixel[i] = (ratio * ((mid >> shift) & 255) as f32 + (1.0 - ratio) * end) as u8;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use wesnoth_engine::game::Game;

    #[test]
    fn campaign_villages_and_flags_have_decodable_art() {
        for name in [
            "rooting_out_a_mage",
            "the_chase",
            "guarded_castle",
            "return_to_the_village",
        ] {
            let game = Game::load(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
                &format!("scenarios/{name}.wml"),
            )
            .unwrap();
            for code in &game.map().cells {
                if gameplay_type(code) != "village" {
                    continue;
                }
                assert!(!variants(village_stem(code)).is_empty());
            }
            let status = game.query("status", Value::Nil).unwrap();
            for side in items(&status, "side_visuals") {
                for frame in 1..=if string(side, "flag_style") == "ragged" {
                    6
                } else {
                    4
                } {
                    let key = format!("flags/{}-flag-{frame}", string(side, "flag_style"));
                    let mut image =
                        Image::from_file_with_format(asset(&key).unwrap(), Some(ImageFormat::Png))
                            .unwrap();
                    let original = image.bytes.clone();
                    recolor_flag(&mut image, string(side, "color")).unwrap();
                    assert!(original != image.bytes, "flag must be recolored: {key}");
                    for (before, after) in original.chunks_exact(4).zip(image.bytes.chunks_exact(4))
                    {
                        assert_eq!(before[3], after[3]);
                    }
                }
            }
        }
        for stem in ["human", "hut", "log-cabin", "elven"] {
            for id in variants(stem) {
                let day = Image::from_file_with_format(asset(id).unwrap(), Some(ImageFormat::Png))
                    .unwrap();
                let night = Image::from_file_with_format(
                    asset(&format!("{id}-night")).unwrap(),
                    Some(ImageFormat::Png),
                )
                .unwrap();
                assert_eq!((day.width, day.height), (night.width, night.height));
                assert!(day.bytes != night.bytes, "night variant must differ: {id}");
            }
        }
    }

    #[test]
    fn capture_changes_flag_owner_and_fog_hides_it() {
        let mut game = Game::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/outpost_defense.wml",
        )
        .unwrap();
        game.acknowledge_dialog().unwrap();
        let p = Position { x: 5, y: 2 };
        assert_eq!(
            village_owner(&game.query("status", Value::Nil).unwrap(), p),
            None
        );
        game.execute(
            "move",
            Value::Map(BTreeMap::from([
                ("object".into(), Value::String("eren".into())),
                (
                    "destination".into(),
                    Value::Map(BTreeMap::from([
                        ("x".into(), Value::Integer(5)),
                        ("y".into(), Value::Integer(2)),
                    ])),
                ),
            ])),
        )
        .unwrap();
        let status = game.query("status", Value::Nil).unwrap();
        assert_eq!(village_owner(&status, p), Some("player"));
        let mut fog = status.as_map().unwrap().clone();
        fog.insert("fog".into(), Value::Bool(true));
        fog.insert("visible_cells".into(), Value::List(vec![]));
        assert_eq!(village_owner(&Value::Map(fog), p), None);
        for count in [4, 6] {
            assert_ne!(flag_frame(0, p, count), flag_frame(150, p, count));
            assert_eq!(
                flag_frame(0, p, count),
                flag_frame(count as u64 * 150, p, count)
            );
        }
        assert!(night_windows("dusk") && night_windows("second_watch"));
        assert!(!night_windows("dawn") && !night_windows("morning"));
    }
}
