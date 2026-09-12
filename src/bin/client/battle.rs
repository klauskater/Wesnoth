use crate::widgets::Ui;
use macroquad::prelude::*;
use std::collections::BTreeMap;
use wesnoth_engine::{game::Game, value::Value};

pub fn string<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
pub fn number(v: &Value, k: &str) -> f64 {
    v.get(k).map_or(0.0, |x| {
        x.as_i64()
            .map(|n| n as f64)
            .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
            .unwrap_or(0.0)
    })
}
fn list(v: &Value) -> &[Value] {
    if let Value::List(xs) = v { xs } else { &[] }
}
pub fn command(a: &str, d: &str, w: &str) -> Value {
    Value::Map(BTreeMap::from([
        ("attacker".into(), Value::String(a.into())),
        ("defender".into(), Value::String(d.into())),
        ("weapon".into(), Value::String(w.into())),
    ]))
}

pub struct BattleDialog {
    pub attacker: String,
    pub defender: String,
    choices: Vec<(String, Value)>,
    selected: usize,
    detail: bool,
    page: usize,
    revision: u64,
    names: [String; 2],
}
pub enum Action {
    None,
    Cancel,
    Attack(Value),
}
impl BattleDialog {
    pub fn open(game: &Game, a: &Value, d: &Value) -> Result<Option<Self>, String> {
        let attacker = string(a, "id");
        let defender = string(d, "id");
        let available = game.query(
            "actions",
            Value::Map(BTreeMap::from([
                ("object".into(), Value::String(attacker.into())),
                ("paths".into(), Value::Bool(false)),
            ])),
        )?;
        if !available
            .get("targets")
            .is_some_and(|v| list(v).iter().any(|x| x.as_str() == Some(defender)))
        {
            return Ok(None);
        }
        let mut choices = Vec::new();
        for weapon in available.get("attacks").map(list).unwrap_or(&[]) {
            if let Some(id) = weapon.as_str() {
                choices.push((
                    id.into(),
                    game.query("preview_attack", command(attacker, defender, id))?,
                ));
            }
        }
        if choices.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            attacker: attacker.into(),
            defender: defender.into(),
            choices,
            selected: 0,
            detail: false,
            page: 0,
            revision: game.revision(),
            names: [
                format!(
                    "{} · ЗД {}/{}",
                    string(a, "name"),
                    number(a, "hitpoints"),
                    number(a, "max_hitpoints")
                ),
                format!(
                    "{} · ЗД {}/{}",
                    string(d, "name"),
                    number(d, "hitpoints"),
                    number(d, "max_hitpoints")
                ),
            ],
        }))
    }
    pub fn draw(&mut self, font: &Font, revision: u64) -> Action {
        let ui = Ui::new();
        let input = ui.input();
        ui.shade(
            Rect::new(0., 0., 1280., 720.),
            Color::from_rgba(0, 0, 0, 195),
        );
        ui.panel(Rect::new(150., 40., 980., 640.));
        ui.label(
            font,
            if self.detail {
                "Подсчёт урона"
            } else {
                "Атаковать врага"
            },
            180.,
            82.,
            28.,
            GOLD,
        );
        for (i, name) in self.names.iter().enumerate() {
            ui.label(font, name, 180. + i as f32 * 475., 122., 22., WHITE);
        }
        if self.detail {
            let preview = &self.choices[self.selected].1;
            for (side, key) in ["attacker_outcomes", "defender_outcomes"]
                .iter()
                .enumerate()
            {
                let x = 180. + side as f32 * 475.;
                if let Some(outcomes) = preview.get(key) {
                    let prefix = if side == 0 { "" } else { "retaliation_" };
                    ui.label(
                        font,
                        &format!(
                            "Урон: {} → {} × {} · попадание {}%",
                            number(preview, &format!("{prefix}base_damage")),
                            number(preview, &format!("{prefix}damage")),
                            number(preview, &format!("{prefix}strikes")),
                            number(preview, &format!("{prefix}chance"))
                        ),
                        x,
                        155.,
                        18.,
                        WHITE,
                    );
                    ui.label(
                        font,
                        &format!(
                            "Время: {:+}% · лидер: {:+}% · сопр.: {}%",
                            number(preview, &format!("{prefix}alignment_modifier")),
                            number(preview, &format!("{prefix}leadership_modifier")),
                            number(preview, &format!("{prefix}resistance_modifier"))
                        ),
                        x,
                        185.,
                        18.,
                        WHITE,
                    );

                    ui.label(
                        font,
                        &format!(
                            "Вероятность гибели: {:.1}%",
                            number(outcomes, "death") * 100.
                        ),
                        x,
                        215.,
                        21.,
                        ORANGE,
                    );
                    ui.label(
                        font,
                        &format!("Без попаданий: {:.1}%", number(outcomes, "unharmed") * 100.),
                        x,
                        247.,
                        21.,
                        GREEN,
                    );
                    ui.label(
                        font,
                        &format!("Среднее ЗД после боя: {:.2}", number(outcomes, "expected")),
                        x,
                        279.,
                        21.,
                        WHITE,
                    );
                    let rows = outcomes.get("distribution").map(list).unwrap_or(&[]);
                    for (index, row) in rows.iter().skip(self.page * 10).take(10).enumerate() {
                        let y = 305. + index as f32 * 27.;
                        let p = number(row, "probability");
                        ui.label(
                            font,
                            &format!("{}", number(row, "hp")),
                            x,
                            y + 19.,
                            18.,
                            WHITE,
                        );
                        ui.shade(
                            Rect::new(x + 50., y, 240., 21.),
                            Color::from_rgba(35, 40, 45, 255),
                        );
                        ui.shade(
                            Rect::new(x + 50., y, 240. * p as f32, 21.),
                            if number(row, "hp") == 0. { RED } else { GREEN },
                        );
                        ui.label(
                            font,
                            &format!("{:.2}%", p * 100.),
                            x + 300.,
                            y + 19.,
                            18.,
                            WHITE,
                        );
                    }
                }
            }
            if ui.button(
                &input,
                font,
                Rect::new(180., 610., 150., 44.),
                "Назад",
                self.page > 0,
            ) {
                self.page = self.page.saturating_sub(1);
            }
            let more = ["attacker_outcomes", "defender_outcomes"].iter().any(|k| {
                preview
                    .get(k)
                    .and_then(|v| v.get("distribution"))
                    .map(list)
                    .is_some_and(|r| r.len() > (self.page + 1) * 10)
            });
            if ui.button(
                &input,
                font,
                Rect::new(350., 610., 150., 44.),
                "Далее",
                more,
            ) {
                self.page += 1;
            }
            if ui.button(
                &input,
                font,
                Rect::new(870., 610., 220., 44.),
                "К выбору оружия",
                true,
            ) {
                self.detail = false;
                self.page = 0;
            }
        } else {
            for (index, (_, p)) in self.choices.iter().enumerate().skip(self.page * 4).take(4) {
                let y = 155. + (index % 4) as f32 * 90.;
                if ui.button(&input, font, Rect::new(180., y, 910., 80.), "", true) {
                    self.selected = index;
                }
                ui.label(
                    font,
                    &format!(
                        "{}{} · {} · {}",
                        if index == self.selected { "▶ " } else { "" },
                        string(p, "weapon_name"),
                        string(p, "damage_type"),
                        string(p, "range")
                    ),
                    195.,
                    y + 27.,
                    22.,
                    if index == self.selected { GOLD } else { WHITE },
                );
                ui.label(
                    font,
                    &format!(
                        "{} × {} · {}%",
                        number(p, "damage"),
                        number(p, "strikes"),
                        number(p, "chance")
                    ),
                    195.,
                    y + 60.,
                    22.,
                    WHITE,
                );
                ui.label(
                    font,
                    &format!(
                        "{}: {} × {} · {}%",
                        string(p, "retaliation_name"),
                        number(p, "retaliation_damage"),
                        number(p, "retaliation_strikes"),
                        number(p, "retaliation_chance")
                    ),
                    640.,
                    y + 60.,
                    20.,
                    WHITE,
                );
            }
            if ui.button(
                &input,
                font,
                Rect::new(180., 530., 130., 42.),
                "Назад",
                self.page > 0,
            ) {
                self.page = self.page.saturating_sub(1);
            }
            if ui.button(
                &input,
                font,
                Rect::new(325., 530., 130., 42.),
                "Далее",
                self.choices.len() > (self.page + 1) * 4,
            ) {
                self.page += 1;
            }
            if ui.button(
                &input,
                font,
                Rect::new(810., 530., 280., 42.),
                "Подсчёт урона",
                true,
            ) {
                self.detail = true;
                self.page = 0;
            }
            if ui.button(
                &input,
                font,
                Rect::new(630., 610., 220., 44.),
                "Атаковать",
                self.revision == revision,
            ) {
                return Action::Attack(command(
                    &self.attacker,
                    &self.defender,
                    &self.choices[self.selected].0,
                ));
            }
            if ui.button(
                &input,
                font,
                Rect::new(870., 610., 220., 44.),
                "Отмена",
                true,
            ) {
                return Action::Cancel;
            }
        }
        Action::None
    }
}

pub struct BattleAnimation {
    pub units: Value,
    pub strikes: Vec<Value>,
    pub started: f64,
}
impl BattleAnimation {
    pub fn new(units: Value, events: &[Value]) -> Option<Self> {
        let event = events
            .iter()
            .find(|v| string(v, "type") == "battle_resolved")?;
        Some(Self {
            units,
            strikes: event.get("strikes").map(list).unwrap_or(&[]).to_vec(),
            started: get_time(),
        })
    }
    pub fn finished(&self) -> bool {
        get_time() - self.started >= self.strikes.len() as f64 * 0.65 + 0.3
    }
    pub fn current(&self) -> Option<(&Value, f32)> {
        let t = (get_time() - self.started) / 0.65;
        self.strikes.get(t as usize).map(|v| (v, t.fract() as f32))
    }
    pub fn hp(&self, unit: &Value) -> i64 {
        let mut hp = number(unit, "hitpoints") as i64;
        let elapsed = get_time() - self.started;
        for (i, strike) in self.strikes.iter().enumerate() {
            if elapsed < i as f64 * 0.65 + 0.325 {
                break;
            }
            if string(strike, "target") == string(unit, "id") {
                hp = number(strike, "target_hitpoints") as i64;
            }
            if string(strike, "source") == string(unit, "id") {
                hp = number(strike, "source_hitpoints") as i64;
            }
        }
        hp
    }
}

mod art {
    include!(concat!(env!("OUT_DIR"), "/embedded_battle_art.rs"));
}
pub struct BattleArt {
    animations: BTreeMap<String, Vec<String>>,
    textures: BTreeMap<String, Texture2D>,
}
impl BattleArt {
    pub fn new() -> Self {
        Self {
            animations: serde_json::from_str(include_str!(
                "../../../assets/wesnoth/battle/animations.json"
            ))
            .expect("valid battle art"),
            textures: BTreeMap::new(),
        }
    }
    pub fn prepare(&mut self, units: &Value, strikes: &[Value]) {
        for strike in strikes {
            if let Some(unit) = list(units)
                .iter()
                .find(|u| string(u, "id") == string(strike, "source"))
            {
                let prefix = format!("{}|{}|", string(unit, "type"), string(strike, "weapon"));
                for (_, frames) in self
                    .animations
                    .range(prefix.clone()..)
                    .take_while(|(k, _)| k.starts_with(&prefix))
                {
                    for frame in frames {
                        if !self.textures.contains_key(frame) {
                            if let Some((_, bytes)) = art::ALL.iter().find(|(p, _)| *p == frame) {
                                self.textures.insert(
                                    frame.clone(),
                                    Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png)),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn frame(
        &self,
        kind: &str,
        weapon: &str,
        facing: &str,
        hit: bool,
        progress: f32,
    ) -> Option<&Texture2D> {
        let key = format!(
            "{kind}|{weapon}|{facing}|{}",
            if hit { "yes" } else { "no" }
        );
        let frames = self.animations.get(&key)?;
        self.textures
            .get(&frames[((progress * frames.len() as f32) as usize).min(frames.len() - 1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weapon_dialog_queries_without_changing_world_or_rng() {
        let mut game = Game::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/first_battle.wml",
        )
        .unwrap();
        game.acknowledge_dialog().unwrap();
        let before = game.save().unwrap();
        let units = game.query("snapshot", Value::Nil).unwrap();
        let a = list(&units)
            .iter()
            .find(|u| string(u, "id") == "alice")
            .unwrap();
        let d = list(&units)
            .iter()
            .find(|u| string(u, "id") == "bob")
            .unwrap();
        let dialog = BattleDialog::open(&game, a, d).unwrap().unwrap();
        assert!(dialog.choices.len() >= 2);
        for (_, preview) in dialog.choices {
            for key in ["attacker_outcomes", "defender_outcomes"] {
                let rows = list(preview.get(key).unwrap().get("distribution").unwrap());
                assert!(
                    (rows.iter().map(|r| number(r, "probability")).sum::<f64>() - 1.).abs() < 1e-10
                );
            }
        }
        assert_eq!(game.save().unwrap(), before);
    }
    #[test]
    fn animation_manifest_has_weapon_and_direction_specific_frames() {
        let animations: BTreeMap<String, Vec<String>> = serde_json::from_str(include_str!(
            "../../../assets/wesnoth/battle/animations.json"
        ))
        .unwrap();
        assert!(animations.contains_key("Knight|sword|ne|yes"));
        assert!(animations.contains_key("Knight|lance|se|no"));
        for frames in animations.values() {
            assert!(!frames.is_empty());
            for frame in frames {
                assert!(art::ALL.iter().any(|(path, _)| *path == frame), "{frame}");
            }
        }
    }
}
