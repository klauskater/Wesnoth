use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
};

use macroquad::prelude::*;
use wesnoth_engine::{
    engine::{
        Position,
        protocol::{self, InteractionKind, UiNode},
    },
    game::Game,
    value::Value,
};

use crate::{
    connection::{Connection, Delivery},
    declarative_ui,
    map_renderer::{MapRenderer, hex_center},
    map_viewport::MapViewport,
    movement::MoveAnimation,
    scene::SpriteRenderer,
    view::{Apply, View},
    village_renderer::VillageRenderer,
    widgets::Ui,
};

mod unit_art {
    include!(concat!(env!("OUT_DIR"), "/embedded_unit_art.rs"));
}

const PANEL_WIDTH: f32 = 340.0;

struct PresentedUnit {
    type_id: String,
}

struct PresentedSelection {
    position: Position,
    reachable: Option<BTreeSet<(i64, i64)>>,
    path: Vec<Position>,
    defense: Option<i64>,
    terrain: (String, String),
    unit: Option<PresentedUnit>,
}

struct PresentedTime {
    ui: UiNode,
    tint: Color,
    night: bool,
}

pub enum Action {
    None,
    Back,
}

pub struct GameScreen {
    battle_art: crate::battle::BattleArt,
    battle: Option<crate::battle::BattleDialog>,
    battle_animation: Option<crate::battle::BattleAnimation>,
    game: Game,
    connection: Connection,
    view: View,
    map: MapRenderer,
    viewport: MapViewport,
    sprites: SpriteRenderer,
    villages: VillageRenderer,
    selected_hex: Option<Position>,
    reachable: Option<BTreeSet<(i64, i64)>>,
    move_path: Vec<Position>,
    move_defense: Option<i64>,
    selected_terrain: Option<(String, String)>,
    selected_unit: Option<PresentedUnit>,
    next_unit: Option<Position>,
    move_animation: Option<MoveAnimation>,
    replay: VecDeque<Value>,
    replay_after: Option<Value>,
    replay_final: Option<(Value, Value)>,
    status: Value,
    units: Value,
    time: PresentedTime,
    ui_assets: declarative_ui::Assets,
    footsteps: Vec<Texture2D>,
    unit_art: BTreeMap<String, Texture2D>,
    recruit_menu: bool,
    error: Option<String>,
}

impl GameScreen {
    pub async fn new(mut game: Game) -> Result<Self, String> {
        let mut connection = Connection::default();
        connection.request_snapshot(&mut game);
        let mut view = View::default();
        for delivery in connection.receive() {
            match delivery {
                Delivery::ViewSnapshot(snapshot) => view.apply_snapshot(snapshot)?,
                Delivery::Error(error) => return Err(error.message),
                _ => return Err("unexpected initial connection delivery".into()),
            }
        }
        let map = MapRenderer::from_view(
            view.block("map")
                .ok_or_else(|| "presentation has no map block".to_owned())?,
        )?;
        let (min, max) = map.bounds();
        let width = PANEL_WIDTH * panel_scale(vec2(screen_width(), screen_height()));
        let mut viewport =
            MapViewport::new(min, max, vec2(screen_width() - width, screen_height()));
        viewport.reserve_panel(width);
        let status = view
            .block("status")
            .cloned()
            .ok_or_else(|| "presentation has no status block".to_owned())?;
        let units = view
            .block("objects")
            .cloned()
            .ok_or_else(|| "presentation has no objects block".to_owned())?;
        let time = presented_time(view.block("time"))?;
        let asset_registry = view
            .block("assets")
            .ok_or_else(|| "presentation has no asset block".to_owned())?;
        let ui_assets =
            declarative_ui::Assets::load(asset_registry, Path::new(env!("CARGO_MANIFEST_DIR")))
                .await?;
        let next_unit = presented_navigation(view.block("navigation"))?;
        let scene = view
            .scene_block("scene")?
            .ok_or_else(|| "presentation has no scene block".to_owned())?;
        let sprites = SpriteRenderer::load(
            &scene,
            asset_registry,
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .await?;
        let villages = VillageRenderer::load(&map, &status)?;
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
            connection,
            view,
            map,
            viewport,
            sprites,
            villages,
            selected_hex: None,
            reachable: None,
            move_path: Vec::new(),
            move_defense: None,
            selected_terrain: None,
            selected_unit: None,
            next_unit,
            move_animation: None,
            replay: VecDeque::new(),
            replay_after: None,
            replay_final: None,
            status,
            units,
            time,
            ui_assets,
            footsteps,
            unit_art,
            recruit_menu: false,
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
        self.advance_replay();
        let moving = self.move_animation.is_some()
            || self.battle_animation.is_some()
            || self.replay_final.is_some();
        if self.replay_final.is_some() {
            if let Some(a) = &self.move_animation {
                self.viewport.focus(a.position(get_time()));
            }
        }
        clear_background(Color::from_rgba(12, 17, 22, 255));
        let screen = vec2(screen_width(), screen_height());
        let scale = panel_scale(screen);
        let width = PANEL_WIDTH * scale;
        self.viewport.reserve_panel(width);
        let has_dialog = !moving && self.view.block("dialog").is_some();
        let modal =
            self.battle.is_some() || self.recruit_menu || has_dialog || self.error.is_some();
        if modal || moving {
            self.viewport.cancel_gesture();
        } else if let Some(point) = self.viewport.update(screen) {
            if let Some((_, target)) = self.sprites.hit_at(point) {
                self.tap_hex(target);
            }
        }
        let tint = self.time.tint;
        let elapsed_ms = (get_time() * 1000.0) as u64;
        self.map.draw_base(&self.viewport, tint);
        self.sprites.draw_ground(&self.viewport, elapsed_ms, tint);
        self.sprites.draw_world(&self.viewport, elapsed_ms, tint);
        self.villages.draw(
            &self.viewport,
            &self.status,
            elapsed_ms,
            tint,
            self.time.night,
        );
        self.map.draw_codes(&self.viewport);
        self.draw_units(screen, tint);
        if self.move_animation.is_none() {
            self.map
                .draw_unreachable(&self.viewport, self.reachable.as_ref());
        }
        self.map.draw_grid(&self.viewport, self.selected_hex);
        self.map.draw_route(
            &self.viewport,
            &self.move_path,
            &self.footsteps,
            self.move_defense,
            font,
        );

        let origin = vec2(screen.x - width, 0.0);
        let ui = Ui::anchored(scale, origin);
        let mut input = ui.input();
        input.pressed &= !modal && !moving;
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
        if let (Some(p), Some((label, code))) = (self.selected_hex, &self.selected_terrain) {
            ui.label(
                font,
                &format!("{label} · {}, {}", p.x, p.y),
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
        if let Err(error) = declarative_ui::draw(
            &ui,
            &input,
            font,
            &self.ui_assets,
            &self.time.ui,
            Rect::new(18.0, 310.0, 304.0, 156.0),
            true,
        ) {
            self.error = Some(error);
        }
        ui.shade(Rect::new(18.0, 474.0, 304.0, 1.0), DARKGRAY);
        ui.label(font, "ЮНИТ В ФОКУСЕ", 18.0, 503.0, 20.0, GOLD);
        if let Some(unit) = &self.selected_unit {
            let details_x = if let Some(texture) = self.unit_art.get(&unit.type_id) {
                ui.image(texture, Rect::new(12.0, 550.0, 80.0, 80.0));
                96.0
            } else {
                18.0
            };
            match self.view.ui_block("unit") {
                Ok(Some(node)) => {
                    if let Err(error) = declarative_ui::draw(
                        &ui,
                        &input,
                        font,
                        &self.ui_assets,
                        &node,
                        Rect::new(details_x, 512.0, 322.0 - details_x, 148.0),
                        true,
                    ) {
                        self.error = Some(error);
                    }
                }
                Ok(None) => self.error = Some("selected unit has no UI block".into()),
                Err(error) => self.error = Some(error),
            }
        } else {
            ui.label(font, "На тайле нет юнита", 18.0, 541.0, 23.0, GRAY);
        }

        match self.view.ui_block("summary") {
            Ok(Some(node)) => {
                if let Err(error) = declarative_ui::draw(
                    &ui,
                    &input,
                    font,
                    &self.ui_assets,
                    &node,
                    Rect::new(18.0, 660.0, 304.0, 38.0),
                    true,
                ) {
                    self.error = Some(error);
                }
            }
            Ok(None) => self.error = Some("presentation has no summary UI block".into()),
            Err(error) => self.error = Some(error),
        }
        let ready = !moving;
        let button = |row: f32| Rect::new(16.0, height - 16.0 - 56.0 - row * 64.0, 308.0, 56.0);
        if ui.button(
            &input,
            font,
            button(2.0),
            "Следующий юнит",
            ready && self.next_unit.is_some(),
        ) && let Some(p) = self.next_unit
        {
            self.interact(InteractionKind::CellClick, position_value(p));
            self.viewport.focus(hex_center(p.x, p.y));
        }
        if self.view.block("recruit").is_some()
            && ui.button(&input, font, button(3.0), "Нанять", ready)
        {
            self.recruit_menu = true;
        }
        if ui.button(
            &input,
            font,
            button(1.0),
            "Снять выделение",
            self.selected_hex.is_some(),
        ) {
            self.interact(InteractionKind::Dismiss, Value::String("selection".into()));
        }
        let hud = match self.view.ui_block("hud") {
            Ok(Some(node)) => Some(node),
            Ok(None) => {
                self.error = Some("presentation has no hud UI block".into());
                None
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        };
        if let Some(node) = hud {
            match declarative_ui::draw(
                &ui,
                &input,
                font,
                &self.ui_assets,
                &node,
                button(0.0),
                !moving,
            ) {
                Ok(Some(action)) => self.activate(action),
                Ok(None) => {}
                Err(error) => self.error = Some(error),
            }
        }

        if !moving && (modal || self.recruit_menu || self.error.is_some()) {
            self.draw_modal(font);
        } else if !moving && is_key_pressed(KeyCode::Escape) {
            return Action::Back;
        }
        Action::None
    }

    fn tap_hex(&mut self, target: Position) {
        self.interact(InteractionKind::CellClick, position_value(target));
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

    fn advance_replay(&mut self) {
        if self.move_animation.is_some() || self.battle_animation.is_some() {
            return;
        }
        if let Some(units) = self.replay_after.take() {
            self.units = units;
        }
        if let Some(event) = self.replay.pop_front() {
            self.units = event.get("replay_units").unwrap().clone();
            if let Value::Map(status) = &mut self.status {
                status.insert(
                    "visible_units".into(),
                    event.get("replay_visible").cloned().unwrap_or(Value::Nil),
                );
            }
            self.replay_after = event.get("replay_after").cloned();
            self.move_animation = MoveAnimation::from_event(&event, get_time());
            self.battle_animation = crate::battle::BattleAnimation::new(
                self.units.clone(),
                std::slice::from_ref(&event),
            );
            self.selected_hex = event.get("from").and_then(position).or_else(|| {
                list(&self.units)
                    .iter()
                    .find(|u| string(u, "id") == string(&event, "attacker"))
                    .and_then(|u| u.get("position"))
                    .and_then(position)
            });
            if let Some(p) = self.selected_hex {
                self.viewport.focus(hex_center(p.x, p.y));
            }
            if let Some(a) = &self.battle_animation {
                self.battle_art.prepare(&a.units, &a.strikes);
            }
        } else if let Some((status, units)) = self.replay_final.take() {
            self.status = status;
            self.units = units;
            self.selected_hex = None;
            self.reachable = None;
            self.move_path.clear();
            self.move_defense = None;
            self.selected_terrain = None;
            self.selected_unit = None;
        }
    }

    fn execute(&mut self, action: &str, command: Value) {
        self.connection.command(&mut self.game, action, command);
        self.finish_connected(action);
    }

    fn activate(&mut self, action: wesnoth_engine::engine::protocol::Action) {
        let name = action.action.clone();
        self.connection.interaction(
            &mut self.game,
            InteractionKind::Activate,
            Value::String(name.clone()),
            Value::Map(BTreeMap::from([
                ("action".into(), Value::String(name.clone())),
                ("payload".into(), action.payload),
            ])),
        );
        self.finish_connected(&name);
    }

    fn interact(&mut self, kind: InteractionKind, target: Value) {
        self.connection
            .interaction(&mut self.game, kind, target, Value::Nil);
        self.finish_connected("move");
    }

    fn finish_connected(&mut self, action: &str) {
        match self.receive_connected() {
            Ok(events) => {
                if action == "end_turn" {
                    self.replay = events
                        .iter()
                        .filter(|e| {
                            e.get("replay_units").is_some()
                                && items(e, "replay_visible").iter().any(|id| {
                                    ["object", "attacker", "defender"]
                                        .iter()
                                        .any(|key| e.get(key) == Some(id))
                                })
                        })
                        .cloned()
                        .collect();
                }
                self.battle_animation = if action == "resolve" {
                    crate::battle::BattleAnimation::new(self.units.clone(), &events)
                } else {
                    None
                };
                if let Some(a) = &mut self.battle_animation {
                    self.battle_art.prepare(&a.units, &a.strikes);
                    a.started = get_time();
                }
                self.move_path.clear();
                self.move_defense = None;
                self.selected_terrain = None;
                self.selected_unit = None;
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
                let refresh = (|| {
                    let status = self
                        .view
                        .block("status")
                        .cloned()
                        .ok_or_else(|| "presentation has no status block".to_owned())?;
                    let units = self
                        .view
                        .block("objects")
                        .cloned()
                        .ok_or_else(|| "presentation has no objects block".to_owned())?;
                    let selection = presented_selection(self.view.block("selection"))?;
                    let next_unit = presented_navigation(self.view.block("navigation"))?;
                    let time = presented_time(self.view.block("time"))?;
                    let battle = self
                        .view
                        .block("attack")
                        .map(|attack| {
                            crate::battle::BattleDialog::from_view(
                                attack,
                                &units,
                                self.connection.world_revision(),
                            )
                        })
                        .transpose()?;
                    Ok::<_, String>((status, units, selection, next_unit, time, battle))
                })();
                match refresh {
                    Ok((status, units, mut selection, next_unit, time, battle)) => {
                        self.next_unit = next_unit;
                        self.time = time;
                        if !self.replay.is_empty() {
                            self.replay_final = Some((status, units));
                            self.selected_hex = None;
                            self.reachable = None;
                            self.advance_replay();
                        } else {
                            self.status = status;
                            self.units = units;
                            self.selected_hex = selection.as_ref().map(|view| view.position);
                            self.reachable =
                                selection.as_mut().and_then(|view| view.reachable.take());
                            self.move_path = selection
                                .as_mut()
                                .map(|view| std::mem::take(&mut view.path))
                                .unwrap_or_default();
                            self.move_defense = selection.as_ref().and_then(|view| view.defense);
                            self.selected_terrain = selection
                                .as_mut()
                                .map(|view| std::mem::take(&mut view.terrain));
                            self.selected_unit = selection.and_then(|view| view.unit);
                            self.battle = battle;
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn receive_connected(&mut self) -> Result<Vec<Value>, String> {
        let mut events = Vec::new();
        let mut need_snapshot = false;
        for delivery in self.connection.receive() {
            match delivery {
                Delivery::CommandResult(result) => {
                    if let Some(error) = result.error {
                        return Err(error.message);
                    }
                }
                Delivery::ViewUpdate(update) => {
                    events.extend(update.effects.iter().map(|effect| effect.content.clone()));
                    need_snapshot |= self.view.apply_update(update)? == Apply::NeedSnapshot;
                }
                Delivery::ViewSnapshot(snapshot) => self.view.apply_snapshot(snapshot)?,
                Delivery::Error(error) => return Err(error.message),
            }
        }
        if need_snapshot {
            self.connection.request_snapshot(&mut self.game);
            for delivery in self.connection.receive() {
                match delivery {
                    Delivery::ViewSnapshot(snapshot) => self.view.apply_snapshot(snapshot)?,
                    Delivery::Error(error) => return Err(error.message),
                    _ => return Err("unexpected snapshot delivery".into()),
                }
            }
        }
        Ok(events)
    }

    fn draw_modal(&mut self, font: &Font) {
        if let Some(dialog) = &mut self.battle {
            match dialog.draw(font, self.connection.world_revision()) {
                crate::battle::Action::None => {}
                crate::battle::Action::Cancel => {
                    self.battle = None;
                    self.interact(InteractionKind::Dismiss, Value::String("attack".into()));
                }
                crate::battle::Action::Attack(command) => {
                    self.battle = None;
                    self.interact(InteractionKind::Dismiss, Value::String("attack".into()));
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
        if let Some(dialog) = match self.view.ui_block("dialog") {
            Ok(dialog) => dialog,
            Err(error) => {
                self.error = Some(error);
                None
            }
        } {
            match declarative_ui::draw(
                &ui,
                &input,
                font,
                &self.ui_assets,
                &dialog,
                Rect::new(310.0, 100.0, 660.0, 520.0),
                true,
            ) {
                Ok(Some(action)) => self.activate(action),
                Ok(None) => {}
                Err(error) => self.error = Some(error),
            }
            return;
        }
        if !self.recruit_menu {
            return;
        }
        let recruit = match self.view.ui_block("recruit") {
            Ok(Some(node)) => Some(node),
            Ok(None) => {
                self.recruit_menu = false;
                None
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        };
        if let Some(node) = recruit {
            match declarative_ui::draw(
                &ui,
                &input,
                font,
                &self.ui_assets,
                &node,
                Rect::new(310.0, 100.0, 660.0, 460.0),
                true,
            ) {
                Ok(Some(action)) => {
                    self.recruit_menu = false;
                    self.activate(action);
                }
                Ok(None) => {}
                Err(error) => self.error = Some(error),
            }
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
    }
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
fn position_value(position: Position) -> Value {
    Value::Map(BTreeMap::from([
        ("x".into(), Value::Integer(position.x)),
        ("y".into(), Value::Integer(position.y)),
    ]))
}
fn presented_selection(selection: Option<&Value>) -> Result<Option<PresentedSelection>, String> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    let selected = selection
        .get("position")
        .and_then(position)
        .ok_or("selection position is invalid")?;
    let parse = |key: &str| -> Result<Option<Vec<Position>>, String> {
        let Some(value) = selection.get(key) else {
            return Ok(None);
        };
        let Value::List(values) = value else {
            return Err(format!("selection {key} must be a list"));
        };
        values
            .iter()
            .map(|value| position(value).ok_or_else(|| format!("invalid {key} position")))
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    };
    let reachable = parse("reachable")?.map(|positions| {
        positions
            .into_iter()
            .map(|position| (position.x, position.y))
            .collect()
    });
    let defense = match selection.get("defense") {
        None => None,
        Some(Value::Integer(value)) => Some(*value),
        Some(_) => return Err("selection defense must be an integer".into()),
    };
    let terrain = match selection.get("terrain") {
        Some(Value::Map(terrain)) => match (terrain.get("label"), terrain.get("code")) {
            (Some(Value::String(label)), Some(Value::String(code))) => {
                (label.clone(), code.clone())
            }
            _ => return Err("selection terrain must contain string label and code".into()),
        },
        Some(_) => return Err("selection terrain must be a record".into()),
        None => return Err("selection has no terrain presentation".into()),
    };
    let unit = selection.get("unit").map(presented_unit).transpose()?;
    Ok(Some(PresentedSelection {
        position: selected,
        reachable,
        path: parse("path")?.unwrap_or_default(),
        defense,
        terrain,
        unit,
    }))
}

fn presented_unit(unit: &Value) -> Result<PresentedUnit, String> {
    let Value::Map(unit) = unit else {
        return Err("selection unit must be a record".into());
    };
    Ok(PresentedUnit {
        type_id: unit
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or("selection unit type must be a string")?,
    })
}

fn presented_navigation(navigation: Option<&Value>) -> Result<Option<Position>, String> {
    let Some(navigation) = navigation else {
        return Ok(None);
    };
    navigation
        .get("next_unit")
        .and_then(|unit| unit.get("position"))
        .and_then(position)
        .map(Some)
        .ok_or_else(|| "navigation next_unit position is invalid".into())
}
fn visible(unit: &Value, status: &Value) -> bool {
    items(status, "visible_units")
        .iter()
        .any(|id| id.as_str() == Some(string(unit, "id")))
}
fn presented_time(value: Option<&Value>) -> Result<PresentedTime, String> {
    let value = value.ok_or("presentation has no time block")?;
    let ui = protocol::ui_block(value).map_err(|error| error.message)?;
    let Value::List(tint) = value.get("map_tint").ok_or("time block has no map_tint")? else {
        return Err("time block map_tint must be a list".into());
    };
    if tint.len() != 4 {
        return Err("time block map_tint must contain four channels".into());
    }
    let channels = tint
        .iter()
        .map(|channel| {
            channel
                .as_f64()
                .filter(|channel| channel.is_finite() && (0.0..=1.0).contains(channel))
                .ok_or_else(|| "time block tint channel must be in 0..1".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let night = match value.get("scene_dark") {
        Some(Value::Bool(value)) => *value,
        _ => return Err("time block scene_dark must be a boolean".into()),
    };
    Ok(PresentedTime {
        ui,
        tint: Color::new(
            channels[0] as f32,
            channels[1] as f32,
            channels[2] as f32,
            channels[3] as f32,
        ),
        night,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_actions_follow_real_game_state() {
        let mut game = Game::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/outpost_defense.wml",
        )
        .unwrap();
        let status = game.query("status", Value::Nil).unwrap();
        let units = game.query("snapshot", Value::Nil).unwrap();
        let view = game.view_snapshot("panel-test").unwrap();
        let navigation = view
            .blocks
            .iter()
            .find(|block| block.id == "navigation")
            .map(|block| &block.content);
        let focus = presented_navigation(navigation).unwrap().unwrap();
        assert!(
            list(&units)
                .iter()
                .any(|unit| unit.get("position").and_then(position) == Some(focus))
        );
        let hex = items(&status, "recruit_hexes").first().unwrap();
        let destination = position(hex).unwrap();
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
        assert_eq!(
            integer(&updated, "gold"),
            integer(&status, "gold") - integer(option, "cost")
        );
        let updated_units = game.query("snapshot", Value::Nil).unwrap();
        assert!(
            list(&updated_units)
                .iter()
                .any(|unit| unit.get("position").and_then(position) == Some(destination))
        );
        if !game.start_events().unwrap().is_empty() {
            game.acknowledge_dialog().unwrap();
        }
        game.execute("end_turn", Value::Nil).unwrap();
        let ended = game.query("status", Value::Nil).unwrap();
        assert_eq!(integer(&ended, "turn"), integer(&status, "turn") + 1);
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
