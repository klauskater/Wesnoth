use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use macroquad::prelude::*;
use wesnoth_engine::{engine::Position, game::Game, value::Value};

use crate::{
    map_renderer::{MapRenderer, hex_center, terrain_info, time_tint},
    map_viewport::MapViewport,
    movement::{MoveAnimation, MovePreview},
    sprite_renderer::SpriteRenderer,
    village_renderer::VillageRenderer,
    widgets::Ui,
};

mod unit_art {
    include!(concat!(env!("OUT_DIR"), "/embedded_unit_art.rs"));
}

const PANEL_WIDTH: f32 = 340.0;

pub enum Action {
    None,
    Back,
}

pub struct GameScreen {
    battle_art: crate::battle::BattleArt,
    battle: Option<crate::battle::BattleDialog>,
    battle_animation: Option<crate::battle::BattleAnimation>,
    game: Game,
    map: MapRenderer,
    viewport: MapViewport,
    sprites: SpriteRenderer,
    villages: VillageRenderer,
    selected_hex: Option<Position>,
    reach_key: Option<(Option<Position>, u64)>,
    reachable: Option<BTreeSet<(i64, i64)>>,
    move_preview: MovePreview,
    move_animation: Option<MoveAnimation>,
    status: Value,
    units: Value,
    time_art: Vec<Texture2D>,
    footsteps: Vec<Texture2D>,
    unit_art: BTreeMap<String, Texture2D>,
    recruit_menu: bool,
    recruit_page: usize,
    dialog_line: usize,
    error: Option<String>,
}

impl GameScreen {
    pub async fn new(game: Game) -> Result<Self, String> {
        let map = MapRenderer::new(game.map(), game.map_tiles());
        let (min, max) = map.bounds();
        let width = PANEL_WIDTH * panel_scale(vec2(screen_width(), screen_height()));
        let mut viewport =
            MapViewport::new(min, max, vec2(screen_width() - width, screen_height()));
        viewport.reserve_panel(width);
        let sprites = SpriteRenderer::load(
            game.map(),
            game.terrain_scripts(),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .await?;
        let status = game.query("status", Value::Nil)?;
        let units = game.query("snapshot", Value::Nil)?;
        let villages = VillageRenderer::load(game.map(), &status)?;
        let time_art = [
            include_bytes!("../../../../assets/wesnoth/ui/time-of-day/schedule-dawn.png")
                .as_slice(),
            include_bytes!("../../../../assets/wesnoth/ui/time-of-day/schedule-morning.png")
                .as_slice(),
            include_bytes!("../../../../assets/wesnoth/ui/time-of-day/schedule-afternoon.png")
                .as_slice(),
            include_bytes!("../../../../assets/wesnoth/ui/time-of-day/schedule-dusk.png")
                .as_slice(),
            include_bytes!("../../../../assets/wesnoth/ui/time-of-day/schedule-firstwatch.png")
                .as_slice(),
            include_bytes!("../../../../assets/wesnoth/ui/time-of-day/schedule-secondwatch.png")
                .as_slice(),
        ]
        .into_iter()
        .map(|bytes| Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png)))
        .collect();
        // Load only the types that this scenario can display or recruit.
        let mut unit_art = BTreeMap::new();
        for unit in list(&units)
            .iter()
            .chain(items(&status, "recruit_options"))
            .chain(items(&status, "recall_options"))
        {
            let kind = string(unit, "type");
            if !unit_art.contains_key(kind)
                && let Some((_, bytes)) = unit_art::ALL.iter().find(|(id, _)| *id == kind)
            {
                unit_art.insert(
                    kind.to_owned(),
                    Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png)),
                );
            }
        }
        let footsteps = [
            include_bytes!("../../../../assets/wesnoth/footsteps/footprint-in-n.png").as_slice(),
            include_bytes!("../../../../assets/wesnoth/footsteps/footprint-in-ne.png").as_slice(),
            include_bytes!("../../../../assets/wesnoth/footsteps/footprint-in-se.png").as_slice(),
            include_bytes!("../../../../assets/wesnoth/footsteps/footprint-out-n.png").as_slice(),
            include_bytes!("../../../../assets/wesnoth/footsteps/footprint-out-ne.png").as_slice(),
            include_bytes!("../../../../assets/wesnoth/footsteps/footprint-out-se.png").as_slice(),
        ]
        .into_iter()
        .map(|bytes| Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png)))
        .collect();
        for (id, bytes) in unit_art::ALL {
            if let Some((kind, _)) = id.rsplit_once(':') {
                if unit_art.contains_key(kind) {
                    unit_art.insert(
                        (*id).into(),
                        Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png)),
                    );
                }
            }
        }
        Ok(Self {
            battle_art: crate::battle::BattleArt::new(),
            battle: None,
            battle_animation: None,
            game,
            map,
            viewport,
            sprites,
            villages,
            selected_hex: None,
            reach_key: None,
            reachable: None,
            move_preview: MovePreview::default(),
            move_animation: None,
            status,
            units,
            time_art,
            footsteps,
            unit_art,
            recruit_menu: false,
            recruit_page: 0,
            dialog_line: 0,
            error: None,
        })
    }

    pub fn draw(&mut self, font: &Font) -> Action {
        if self
            .move_animation
            .as_ref()
            .is_some_and(|animation| animation.finished(get_time()))
        {
            self.move_animation = None;
        }
        if self.battle_animation.as_ref().is_some_and(|a| a.finished()) {
            self.battle_animation = None;
        }
        let moving = self.move_animation.is_some() || self.battle_animation.is_some();
        clear_background(Color::from_rgba(12, 17, 22, 255));
        let screen = vec2(screen_width(), screen_height());
        let scale = panel_scale(screen);
        let width = PANEL_WIDTH * scale;
        self.viewport.reserve_panel(width);
        let has_dialog = !moving
            && self
                .game
                .start_events()
                .is_ok_and(|events| !events.is_empty());
        let modal =
            self.battle.is_some() || self.recruit_menu || has_dialog || self.error.is_some();
        if modal || moving {
            self.viewport.cancel_gesture();
        } else if let Some(point) = self.viewport.update(screen) {
            if let Some(target) = self.map.hex_at(point) {
                self.tap_hex(target);
            }
        }
        let reach_key = (self.selected_hex, self.game.revision());
        if self.reach_key != Some(reach_key) {
            self.move_preview.clear();
            self.reach_key = Some(reach_key);
            match movement_range(&self.game, &self.units, &self.status, self.selected_hex) {
                Ok(cells) => self.reachable = cells,
                Err(error) => {
                    self.reachable = None;
                    self.error = Some(error);
                }
            }
        }
        let tint = time_tint(string(&self.status, "time_of_day"));
        let elapsed_ms = (get_time() * 1000.0) as u64;
        self.map.draw_base(&self.viewport, tint);
        self.sprites.draw_ground(&self.viewport, elapsed_ms, tint);
        self.sprites.draw_world(&self.viewport, elapsed_ms, tint);
        self.villages
            .draw(&self.viewport, &self.status, elapsed_ms, tint);
        self.map.draw_codes(&self.viewport);
        self.draw_units(screen, tint);
        if self.move_animation.is_none() {
            self.map
                .draw_unreachable(&self.viewport, self.reachable.as_ref());
        }
        self.map.draw_grid(&self.viewport, self.selected_hex);
        let defense = self.move_preview.path().last().and_then(|p| {
            let terrain = self.game.map().get(*p).ok()?;
            focused_unit(&self.units, &self.status, self.selected_hex)?
                .get("defense")?
                .get(terrain)?
                .as_i64()
        });
        self.map.draw_route(
            &self.viewport,
            self.move_preview.path(),
            &self.footsteps,
            defense,
            font,
        );

        let origin = vec2(screen.x - width, 0.0);
        let ui = Ui::anchored(scale, origin);
        let height = screen.y / scale;
        ui.shade(
            Rect::new(0.0, 0.0, PANEL_WIDTH, height),
            Color::from_rgba(21, 28, 34, 255),
        );
        ui.shade(
            Rect::new(0.0, 0.0, 2.0, height),
            Color::from_rgba(156, 133, 82, 255),
        );
        ui.shade(
            Rect::new(16.0, 16.0, 308.0, 176.0),
            Color::from_rgba(9, 15, 20, 255),
        );
        self.map.draw_minimap(Rect::new(
            origin.x + 24.0 * scale,
            24.0 * scale,
            292.0 * scale,
            160.0 * scale,
        ));
        ui.label(font, "ТАЙЛ В ФОКУСЕ", 18.0, 220.0, 20.0, GOLD);
        let terrain = self.selected_hex.and_then(|p| self.game.map().raw(p).ok());
        if let (Some(p), Some(code)) = (self.selected_hex, terrain) {
            ui.label(
                font,
                &format!("{} · {}, {}", terrain_info(code).0, p.x, p.y),
                18.0,
                248.0,
                23.0,
                WHITE,
            );
            ui.label(font, code, 18.0, 273.0, 19.0, GRAY);
        } else {
            ui.label(font, "Нажмите на гекс", 18.0, 250.0, 23.0, GRAY);
        }
        ui.shade(Rect::new(18.0, 290.0, 304.0, 1.0), DARKGRAY);
        let (time_index, time_name) = time_info(string(&self.status, "time_of_day"));
        ui.label(font, time_name, 18.0, 330.0, 24.0, WHITE);
        let time_texture = &self.time_art[time_index];
        ui.image(
            time_texture,
            Rect::new(
                18.0,
                349.0,
                304.0,
                304.0 * time_texture.height() / time_texture.width(),
            ),
        );
        ui.shade(Rect::new(18.0, 474.0, 304.0, 1.0), DARKGRAY);
        ui.label(font, "ЮНИТ В ФОКУСЕ", 18.0, 503.0, 20.0, GOLD);
        if let Some(unit) = focused_unit(&self.units, &self.status, self.selected_hex) {
            ui.wrapped_label(
                font,
                string(unit, "name"),
                Rect::new(18.0, 512.0, 304.0, 30.0),
                24.0,
                WHITE,
            );
            let details_x = if let Some(texture) = self.unit_art.get(string(unit, "type")) {
                ui.image(texture, Rect::new(12.0, 550.0, 80.0, 80.0));
                96.0
            } else {
                18.0
            };
            for (i, line) in [
                format!(
                    "Здоровье: {}/{}",
                    integer(unit, "hitpoints"),
                    integer(unit, "max_hitpoints")
                ),
                format!(
                    "Ходы: {}/{} · Атак: {}",
                    integer(unit, "movement_points"),
                    integer(unit, "max_movement_points"),
                    integer(unit, "attacks_left")
                ),
                format!(
                    "Ур. {} · Опыт: {}/{}",
                    integer(unit, "level"),
                    integer(unit, "experience"),
                    integer(unit, "max_experience")
                ),
            ]
            .iter()
            .enumerate()
            {
                ui.label(font, line, details_x, 567.0 + i as f32 * 28.0, 20.0, WHITE);
            }
            ui.label(
                font,
                alignment_name(string(unit, "alignment")),
                18.0,
                652.0,
                21.0,
                GRAY,
            );
        } else {
            ui.label(font, "На тайле нет юнита", 18.0, 541.0, 23.0, GRAY);
        }

        ui.label(
            font,
            &format!(
                "Ход {} · Золото: {}",
                integer(&self.status, "turn"),
                integer(&self.status, "gold")
            ),
            18.0,
            686.0,
            21.0,
            GOLD,
        );
        let mut input = ui.input();
        input.pressed &= !modal && !moving;
        let ready = playable(&self.status) && self.move_animation.is_none();
        let button = |row: f32| Rect::new(16.0, height - 16.0 - 56.0 - row * 64.0, 308.0, 56.0);
        if ui.button(
            &input,
            font,
            button(2.0),
            "Следующий юнит",
            ready && next_unit(&self.units, &self.status, self.selected_hex).is_some(),
        ) && let Some(unit) = next_unit(&self.units, &self.status, self.selected_hex)
        {
            self.move_preview.clear();
            self.selected_hex = position(unit.get("position").unwrap_or(&Value::Nil));
            if let Some(p) = self.selected_hex {
                self.viewport.focus(hex_center(p.x, p.y));
            }
        }
        if can_recruit(&self.status, self.selected_hex)
            && ui.button(&input, font, button(3.0), "Нанять", ready)
        {
            self.recruit_menu = true;
            self.recruit_page = 0;
        }
        if ui.button(
            &input,
            font,
            button(1.0),
            "Снять выделение",
            self.selected_hex.is_some(),
        ) {
            self.move_preview.clear();
            self.selected_hex = None;
        }
        if ui.button(&input, font, button(0.0), "Закончить ход", ready) {
            self.execute("end_turn", Value::Nil);
        }

        if modal || self.recruit_menu || self.error.is_some() {
            self.draw_modal(font);
        } else if self.move_animation.is_none() && is_key_pressed(KeyCode::Escape) {
            return Action::Back;
        }
        Action::None
    }

    fn tap_hex(&mut self, target: Position) {
        if playable(&self.status) {
            if let (Some(a), Some(d)) = (
                focused_unit(&self.units, &self.status, self.selected_hex),
                focused_unit(&self.units, &self.status, Some(target)),
            ) {
                if string(a, "id") != string(d, "id")
                    && string(a, "side") == string(&self.status, "active_side")
                {
                    match crate::battle::BattleDialog::open(&self.game, a, d) {
                        Ok(Some(dialog)) => {
                            self.move_preview.clear();
                            self.battle = Some(dialog);
                            return;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.error = Some(error);
                            return;
                        }
                    }
                }
            }
        }

        // Unit selection takes precedence over movement, including shaded cells.
        // A shaded empty tile keeps tile focus but releases the selected unit.
        if self.selected_hex != Some(target)
            && (focused_unit(&self.units, &self.status, Some(target)).is_some()
                || self
                    .reachable
                    .as_ref()
                    .is_some_and(|cells| !cells.contains(&(target.x, target.y))))
        {
            self.move_preview.clear();
            self.selected_hex = Some(target);
            return;
        }
        let unit = focused_unit(&self.units, &self.status, self.selected_hex);
        if let Some(unit) =
            unit.filter(|unit| string(unit, "side") == string(&self.status, "active_side"))
        {
            if !playable(&self.status) {
                return;
            }
            let Some(reachable) = &self.reachable else {
                return;
            };
            let origin = self.selected_hex.unwrap();
            match self
                .move_preview
                .tap(&self.game, string(unit, "id"), origin, target, reachable)
            {
                Ok(Some(command)) => self.execute("move", command),
                Ok(None) => {}
                Err(error) => self.error = Some(error),
            }
        } else {
            self.move_preview.clear();
            self.selected_hex = Some(target);
        }
    }

    fn draw_units(&self, screen: Vec2, tint: Color) {
        let displayed = self
            .battle_animation
            .as_ref()
            .map_or(&self.units, |a| &a.units);
        for unit in list(displayed)
            .iter()
            .filter(|unit| visible(unit, &self.status))
        {
            let Some(p) = unit.get("position").and_then(position) else {
                continue;
            };
            let world = self
                .move_animation
                .as_ref()
                .filter(|animation| animation.unit == string(unit, "id"))
                .map_or_else(
                    || hex_center(p.x, p.y),
                    |animation| animation.position(get_time()),
                );
            if self
                .battle_animation
                .as_ref()
                .is_some_and(|a| a.hp(unit) <= 0)
            {
                continue;
            }
            let mut world = world;
            let mut battle_facing = None;
            if let Some((strike, progress)) =
                self.battle_animation.as_ref().and_then(|a| a.current())
            {
                let id = string(unit, "id");
                let opponent = if id == string(strike, "source") {
                    Some(string(strike, "target"))
                } else if id == string(strike, "target") {
                    Some(string(strike, "source"))
                } else {
                    None
                };
                if let Some(other) = opponent
                    .and_then(|id| list(displayed).iter().find(|u| string(u, "id") == id))
                    .and_then(|u| u.get("position"))
                    .and_then(position)
                {
                    let delta = hex_center(other.x, other.y) - world;
                    battle_facing = Some(crate::movement::direction(delta));
                    if id == string(strike, "source") && string(strike, "range") == "melee" {
                        world += delta * (progress * std::f32::consts::PI).sin() * 0.18;
                    }
                }
            }
            let center = self.viewport.project(world, screen);
            let radius = self.viewport.zoom();
            let color = if string(unit, "side") == string(&self.status, "active_side") {
                SKYBLUE
            } else {
                RED
            };
            draw_circle(center.x, center.y + radius * 0.6, radius * 0.5, color);
            if let Some(animation) = &self.battle_animation {
                let hp = animation.hp(unit);
                let maximum = integer(unit, "max_hitpoints").max(1);
                draw_rectangle(
                    center.x - radius * 0.6,
                    center.y - radius,
                    radius * 1.2,
                    4.,
                    BLACK,
                );
                draw_rectangle(
                    center.x - radius * 0.6,
                    center.y - radius,
                    radius * 1.2 * hp as f32 / maximum as f32,
                    4.,
                    GREEN,
                );
                if let Some((strike, progress)) = animation.current() {
                    if string(strike, "target") == string(unit, "id") && progress >= 0.5 {
                        let label = if strike.get("hit") == Some(&Value::Bool(true)) {
                            format!("-{}", integer(strike, "damage"))
                        } else {
                            "miss".into()
                        };
                        draw_text(
                            &label,
                            center.x,
                            center.y - radius * (1. + progress),
                            24.,
                            if strike.get("hit") == Some(&Value::Bool(true)) {
                                RED
                            } else {
                                WHITE
                            },
                        );
                    }
                }
            }

            let facing = battle_facing
                .or_else(|| {
                    self.move_animation
                        .as_ref()
                        .filter(|a| a.unit == string(unit, "id"))
                        .and_then(|a| a.facing(get_time()))
                })
                .unwrap_or_else(|| string(unit, "facing"));
            let key = format!("{}:{}", string(unit, "type"), facing);
            let attack_frame = self
                .battle_animation
                .as_ref()
                .and_then(|a| a.current())
                .and_then(|(strike, progress)| {
                    if string(strike, "source") == string(unit, "id") {
                        self.battle_art.frame(
                            string(unit, "type"),
                            string(strike, "weapon"),
                            facing,
                            strike.get("hit") == Some(&Value::Bool(true)),
                            progress,
                        )
                    } else {
                        None
                    }
                });
            if let Some(texture) = attack_frame
                .or_else(|| self.unit_art.get(&key))
                .or_else(|| self.unit_art.get(string(unit, "type")))
            {
                draw_texture_ex(
                    texture,
                    center.x - radius,
                    center.y - radius,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                        flip_x: matches!(facing, "nw" | "sw"),
                        ..Default::default()
                    },
                );
            } else {
                draw_circle(center.x, center.y, radius * 0.35, color);
            }
        }
    }

    fn execute(&mut self, action: &str, command: Value) {
        match self.game.execute(action, command) {
            Ok(events) => {
                self.battle_animation = if action == "resolve" {
                    crate::battle::BattleAnimation::new(self.units.clone(), &events)
                } else {
                    None
                };
                if let Some(a) = &mut self.battle_animation {
                    self.battle_art.prepare(&a.units, &a.strikes);
                    a.started = get_time();
                }
                self.move_preview.clear();
                if action == "move" {
                    self.move_animation = events
                        .iter()
                        .find_map(|event| MoveAnimation::from_event(event, get_time()));
                    if let Some(event) = events
                        .iter()
                        .find(|event| string(event, "type") == "object_moved")
                    {
                        self.selected_hex = event.get("to").and_then(position);
                    }
                }
                self.dialog_line = 0;
                match self.game.query("status", Value::Nil).and_then(|status| {
                    self.game
                        .query("snapshot", Value::Nil)
                        .map(|units| (status, units))
                }) {
                    Ok((status, units)) => {
                        self.status = status;
                        self.units = units;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn draw_modal(&mut self, font: &Font) {
        if let Some(dialog) = &mut self.battle {
            match dialog.draw(font, self.game.revision()) {
                crate::battle::Action::None => {}
                crate::battle::Action::Cancel => self.battle = None,
                crate::battle::Action::Attack(command) => {
                    self.battle = None;
                    self.execute("resolve", command);
                }
            }
            return;
        }

        let ui = Ui::new();
        let input = ui.input();
        ui.shade(
            Rect::new(0.0, 0.0, 1280.0, 720.0),
            Color::from_rgba(0, 0, 0, 190),
        );
        ui.panel(Rect::new(280.0, 60.0, 720.0, 600.0));
        if let Some(error) = self.error.clone() {
            ui.label(font, "Действие не выполнено", 310.0, 108.0, 28.0, GOLD);
            ui.wrapped_label(
                font,
                &error,
                Rect::new(310.0, 135.0, 660.0, 390.0),
                22.0,
                WHITE,
            );
            if ui.button(
                &input,
                font,
                Rect::new(720.0, 584.0, 250.0, 52.0),
                "Закрыть",
                true,
            ) {
                self.error = None;
            }
            return;
        }
        if let Ok(events) = self.game.start_events()
            && let Some(event) = events.first()
        {
            let lines = items(event, "lines");
            if let Some(line) = lines.get(self.dialog_line) {
                ui.label(font, string(line, "speaker"), 310.0, 108.0, 28.0, GOLD);
                ui.wrapped_label(
                    font,
                    string(line, "text"),
                    Rect::new(310.0, 135.0, 660.0, 420.0),
                    24.0,
                    WHITE,
                );
            }
            if ui.button(
                &input,
                font,
                Rect::new(720.0, 584.0, 250.0, 52.0),
                "Продолжить",
                true,
            ) {
                self.dialog_line += 1;
                if self.dialog_line >= lines.len() {
                    if let Err(error) = self.game.acknowledge_dialog() {
                        self.error = Some(error);
                    }
                    self.dialog_line = 0;
                }
            }
            return;
        }
        if !self.recruit_menu {
            return;
        }
        ui.label(
            font,
            &format!("Нанять · Золото: {}", integer(&self.status, "gold")),
            310.0,
            108.0,
            28.0,
            GOLD,
        );
        let options = items(&self.status, "recruit_options");
        let mut recruit = None;
        for (index, option) in options
            .iter()
            .enumerate()
            .skip(self.recruit_page * 6)
            .take(6)
        {
            let cost = integer(option, "cost");
            let label = format!("{} · {} зол.", string(option, "name"), cost);
            if ui.button(
                &input,
                font,
                Rect::new(310.0, 138.0 + (index % 6) as f32 * 66.0, 660.0, 56.0),
                &label,
                cost <= integer(&self.status, "gold"),
            ) {
                recruit = Some(string(option, "id").to_owned());
            }
        }
        if ui.button(
            &input,
            font,
            Rect::new(310.0, 584.0, 150.0, 52.0),
            "Назад",
            self.recruit_page > 0,
        ) {
            self.recruit_page -= 1;
        }
        if ui.button(
            &input,
            font,
            Rect::new(478.0, 584.0, 150.0, 52.0),
            "Далее",
            (self.recruit_page + 1) * 6 < options.len(),
        ) {
            self.recruit_page += 1;
        }
        if ui.button(
            &input,
            font,
            Rect::new(720.0, 584.0, 250.0, 52.0),
            "Отмена",
            true,
        ) || is_key_pressed(KeyCode::Escape)
        {
            self.recruit_menu = false;
        }
        if let (Some(kind), Some(p)) = (recruit, self.selected_hex) {
            self.execute(
                "recruit",
                Value::Map(BTreeMap::from([
                    ("unit_type".into(), Value::String(kind)),
                    (
                        "destination".into(),
                        Value::Map(BTreeMap::from([
                            ("x".into(), Value::Integer(p.x)),
                            ("y".into(), Value::Integer(p.y)),
                        ])),
                    ),
                ])),
            );
            if self.error.is_none() {
                self.recruit_menu = false;
            }
        }
    }
}

// Recomputed on focus/state changes, never during camera movement or every frame.
fn movement_range(
    game: &Game,
    units: &Value,
    status: &Value,
    focus: Option<Position>,
) -> Result<Option<BTreeSet<(i64, i64)>>, String> {
    if status.get("finished") == Some(&Value::Bool(true)) {
        return Ok(None);
    }
    let Some(unit) = focused_unit(units, status, focus) else {
        return Ok(None);
    };
    let cells = game.query(
        "reachable",
        Value::Map(BTreeMap::from([
            ("object".into(), Value::String(string(unit, "id").into())),
            ("paths".into(), Value::Bool(false)),
            (
                "inspect".into(),
                Value::Bool(string(unit, "side") != string(status, "active_side")),
            ),
        ])),
    )?;
    let mut positions: BTreeSet<_> = list(&cells)
        .iter()
        .filter_map(|cell| cell.get("position").and_then(position))
        .map(|p| (p.x, p.y))
        .collect();
    if let Some(p) = focus {
        positions.insert((p.x, p.y));
    }
    Ok(Some(positions))
}

fn panel_scale(screen: Vec2) -> f32 {
    (screen.y / 960.0).min(screen.x / 1100.0).max(0.1)
}
fn list(value: &Value) -> &[Value] {
    if let Value::List(values) = value {
        values
    } else {
        &[]
    }
}
fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value.get(key).map(list).unwrap_or(&[])
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}
fn integer(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}
fn position(value: &Value) -> Option<Position> {
    Some(Position {
        x: value.get("x")?.as_i64()?,
        y: value.get("y")?.as_i64()?,
    })
}
fn playable(status: &Value) -> bool {
    status.get("can_end_turn") == Some(&Value::Bool(true))
        && status
            .get("pending_choice")
            .is_none_or(|v| *v == Value::Nil)
        && status
            .get("pending_advancement")
            .is_none_or(|v| *v == Value::Nil)
}
fn can_recruit(status: &Value, focus: Option<Position>) -> bool {
    focus.is_some_and(|p| {
        items(status, "recruit_hexes")
            .iter()
            .any(|hex| position(hex) == Some(p))
    }) && !items(status, "recruit_options").is_empty()
}
fn visible(unit: &Value, status: &Value) -> bool {
    items(status, "visible_units")
        .iter()
        .any(|id| id.as_str() == Some(string(unit, "id")))
}
fn focused_unit<'a>(
    units: &'a Value,
    status: &Value,
    focus: Option<Position>,
) -> Option<&'a Value> {
    focus.and_then(|p| {
        list(units).iter().find(|unit| {
            unit.get("position").and_then(position) == Some(p) && visible(unit, status)
        })
    })
}
fn next_unit<'a>(units: &'a Value, status: &Value, focus: Option<Position>) -> Option<&'a Value> {
    let units = list(units);
    let start = units
        .iter()
        .position(|unit| unit.get("position").and_then(position) == focus)
        .map_or(0, |i| i + 1);
    units
        .iter()
        .cycle()
        .skip(start)
        .take(units.len())
        .find(|unit| {
            string(unit, "side") == string(status, "active_side")
                && visible(unit, status)
                && (integer(unit, "movement_points") > 0 || integer(unit, "attacks_left") > 0)
        })
}
fn time_info(id: &str) -> (usize, &str) {
    match id {
        "morning" => (1, "Утро"),
        "afternoon" => (2, "День"),
        "dusk" => (3, "Закат"),
        "first_watch" => (4, "Первая стража"),
        "second_watch" => (5, "Вторая стража"),
        _ => (0, "Рассвет"),
    }
}
fn alignment_name(id: &str) -> &str {
    match id {
        "lawful" => "Порядочный",
        "chaotic" => "Хаотичный",
        "neutral" => "Нейтральный",
        "liminal" => "Сумеречный",
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_shading_tracks_remaining_moves_and_clears_without_a_unit() {
        let mut game = Game::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/first_battle.wml",
        )
        .unwrap();
        game.acknowledge_dialog().unwrap();
        let status = game.query("status", Value::Nil).unwrap();
        let units = game.query("snapshot", Value::Nil).unwrap();
        let start = Some(Position { x: 2, y: 2 });
        let range = movement_range(&game, &units, &status, start)
            .unwrap()
            .unwrap();
        assert!(range.contains(&(2, 2)));
        assert!(range.len() > 1);
        assert!(
            !range.contains(&(3, 2)),
            "occupied enemy hex is not a movement destination"
        );
        assert!(
            movement_range(&game, &units, &status, None)
                .unwrap()
                .is_none()
        );
        assert!(
            movement_range(&game, &units, &status, Some(Position { x: 1, y: 1 }))
                .unwrap()
                .is_none()
        );
        assert!(
            movement_range(&game, &units, &status, Some(Position { x: 3, y: 2 }))
                .unwrap()
                .is_some(),
            "enemy inspection is supported"
        );

        let command = Value::Map(BTreeMap::from([(
            "object".into(),
            Value::String("alice".into()),
        )]));
        let cells = game.query("reachable", command.clone()).unwrap();
        let destination = list(&cells)
            .iter()
            .find(|cell| cell.get("zoc") == Some(&Value::Bool(true)))
            .unwrap()
            .get("position")
            .unwrap()
            .clone();
        let mut command = command.as_map().unwrap().clone();
        command.insert("destination".into(), destination.clone());
        game.execute("move", Value::Map(command)).unwrap();
        let units = game.query("snapshot", Value::Nil).unwrap();
        let status = game.query("status", Value::Nil).unwrap();
        let focus = position(&destination);
        let range = movement_range(&game, &units, &status, focus)
            .unwrap()
            .unwrap();
        let p = focus.unwrap();
        assert_eq!(
            range,
            BTreeSet::from([(p.x, p.y)]),
            "exhausted unit only keeps its own hex lit"
        );
    }

    #[test]
    fn panel_actions_follow_real_game_state() {
        let mut game = Game::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/outpost_defense.wml",
        )
        .unwrap();
        let status = game.query("status", Value::Nil).unwrap();
        let units = game.query("snapshot", Value::Nil).unwrap();
        let first = next_unit(&units, &status, None).unwrap();
        let focus = first.get("position").and_then(position);
        assert_eq!(focused_unit(&units, &status, focus), Some(first));
        assert!(playable(&status));
        assert!(!can_recruit(&status, None));
        let hex = items(&status, "recruit_hexes").first().unwrap();
        let destination = position(hex).unwrap();
        assert!(can_recruit(&status, Some(destination)));
        let option = items(&status, "recruit_options").first().unwrap();
        if !game.start_events().unwrap().is_empty() {
            game.acknowledge_dialog().unwrap();
        }
        game.execute(
            "recruit",
            Value::Map(BTreeMap::from([
                (
                    "unit_type".into(),
                    Value::String(string(option, "id").into()),
                ),
                ("destination".into(), hex.clone()),
            ])),
        )
        .unwrap();
        let updated = game.query("status", Value::Nil).unwrap();
        assert!(!can_recruit(&updated, Some(destination)));
        assert_eq!(
            integer(&updated, "gold"),
            integer(&status, "gold") - integer(option, "cost")
        );
        let updated_units = game.query("snapshot", Value::Nil).unwrap();
        assert!(focused_unit(&updated_units, &updated, Some(destination)).is_some());
        if !game.start_events().unwrap().is_empty() {
            game.acknowledge_dialog().unwrap();
        }
        game.execute("end_turn", Value::Nil).unwrap();
        let ended = game.query("status", Value::Nil).unwrap();
        assert_eq!(integer(&ended, "turn"), integer(&status, "turn") + 1);
        assert_ne!(
            string(&ended, "time_of_day"),
            string(&status, "time_of_day")
        );
    }

    #[test]
    fn sidebar_leaves_room_for_map_and_all_sections() {
        for screen in [
            vec2(1280.0, 720.0),
            vec2(1920.0, 1080.0),
            vec2(800.0, 600.0),
            vec2(2400.0, 1080.0),
        ] {
            let scale = panel_scale(screen);
            assert!(PANEL_WIDTH * scale < screen.x / 2.0);
            assert!(screen.y / scale - 16.0 - 56.0 - 3.0 * 64.0 > 686.0);
        }
    }
}
