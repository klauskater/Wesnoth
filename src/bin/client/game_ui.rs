//! Game interface runtime.
//!
//! Owns presentation state, local input, rendering, camera state, and all
//! decoded interface resources. It emits semantic game events and never
//! executes game rules.

use macroquad::prelude::*;
use wesnoth_engine::{engine::Position, value::Value};

use crate::{
    connection::Delivery,
    declarative_ui,
    game_engine::GameEvent,
    map_renderer::MapRenderer,
    map_viewport::MapViewport,
    resource_loader::InterfaceContext,
    scene::SpriteRenderer,
    view::{Apply, View},
    widgets::Ui,
};

const PANEL_WIDTH: f32 = 340.0;

pub struct GameUi {
    font: Font,
    view: View,
    assets: declarative_ui::Assets,
    map: MapRenderer,
    viewport: MapViewport,
    scene: SpriteRenderer,
    error: Option<String>,
}

impl GameUi {
    pub fn start(context: InterfaceContext) -> Result<Self, String> {
        let font = load_ttf_font_from_bytes(&context.font)
            .map_err(|error| format!("cannot load game UI font: {error}"))?;
        let mut view = View::default();
        view.apply_snapshot(context.initial_view)?;

        let registry = view
            .block("assets")
            .ok_or("presentation has no asset registry")?;
        let map = MapRenderer::from_view(view.block("map").ok_or("presentation has no map")?)?;
        let scene_items = view
            .scene_block("scene")?
            .ok_or("presentation has no scene")?;
        let assets = declarative_ui::Assets::from_memory(registry, &context.assets)?;
        let scene = SpriteRenderer::from_memory(&scene_items, registry, &context.assets)?;
        let (min, max) = map.bounds();
        let panel = panel_width();
        let mut viewport = MapViewport::new(
            min,
            max,
            vec2((screen_width() - panel).max(1.0), screen_height()),
        );
        viewport.reserve_panel(panel);

        Ok(Self {
            font,
            view,
            assets,
            map,
            viewport,
            scene,
            error: None,
        })
    }

    pub fn frame(&mut self) -> Vec<GameEvent> {
        let font = self.font.clone();
        clear_background(Color::from_rgba(12, 17, 22, 255));
        let mut events = Vec::new();
        let screen = vec2(screen_width(), screen_height());
        let panel = panel_width();
        self.viewport.reserve_panel(panel);

        let modal = self.view.block("dialog").is_some() || self.error.is_some();
        if modal {
            self.viewport.cancel_gesture();
        } else if let Some(point) = self.viewport.update(screen)
            && let Some((_, position)) = self.scene.hit_at(point)
        {
            events.push(GameEvent::CellClick(position));
        }

        let tint = self.map_tint();
        let elapsed_ms = (get_time() * 1000.0) as u64;
        self.map.draw_base(&self.viewport, tint);
        self.scene.draw_ground(&self.viewport, elapsed_ms, tint);
        self.scene.draw_world(&self.viewport, elapsed_ms, tint);
        self.map.draw_grid(&self.viewport, self.selected_position());

        let ui = Ui::anchored(1.0, vec2(screen.x - panel, 0.0));
        let mut input = ui.input();
        input.pressed &= !modal;
        ui.shade(
            Rect::new(0.0, 0.0, panel, screen.y),
            Color::from_rgba(21, 28, 34, 245),
        );

        let content = Rect::new(16.0, 16.0, panel - 32.0, screen.y - 32.0);
        let sections = [
            ("time", Rect::new(content.x, content.y, content.w, 160.0)),
            (
                "unit",
                Rect::new(content.x, content.y + 176.0, content.w, 220.0),
            ),
            (
                "summary",
                Rect::new(content.x, screen.y - 160.0, content.w, 48.0),
            ),
            (
                "hud",
                Rect::new(content.x, screen.y - 96.0, content.w, 64.0),
            ),
        ];
        for (block, rect) in sections {
            self.draw_block(block, &ui, &input, &font, rect, &mut events);
        }

        self.draw_modal(&font, &mut events);
        events
    }

    pub fn apply(&mut self, deliveries: Vec<Delivery>) -> bool {
        let mut need_snapshot = false;
        for delivery in deliveries {
            match delivery {
                Delivery::CommandResult(result) => {
                    if let Some(error) = result.error {
                        self.error = Some(error.message);
                    }
                }
                Delivery::ViewSnapshot(snapshot) => {
                    if let Err(error) = self.view.apply_snapshot(snapshot) {
                        self.error = Some(error);
                    }
                }
                Delivery::ViewUpdate(update) => match self.view.apply_update(update) {
                    Ok(Apply::Applied(_effects)) => {}
                    Ok(Apply::NeedSnapshot) => need_snapshot = true,
                    Err(error) => self.error = Some(error),
                },
                Delivery::Error(error) => self.error = Some(error.message),
            }
        }
        need_snapshot
    }

    fn draw_block(
        &mut self,
        block: &str,
        ui: &Ui,
        input: &crate::widgets::Input,
        font: &Font,
        rect: Rect,
        events: &mut Vec<GameEvent>,
    ) {
        let node = match self.view.ui_block(block) {
            Ok(Some(node)) => node,
            Ok(None) => return,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        match declarative_ui::draw(ui, input, font, &self.assets, &node, rect, true) {
            Ok(Some(action)) => events.push(GameEvent::Activate(action)),
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }

    fn draw_modal(&mut self, font: &Font, events: &mut Vec<GameEvent>) {
        let Some(message) = self.error.clone() else {
            if self.view.block("dialog").is_none() {
                return;
            }
            let ui = Ui::new();
            let input = ui.input();
            ui.shade(
                Rect::new(0.0, 0.0, 1280.0, 720.0),
                Color::from_rgba(0, 0, 0, 190),
            );
            self.draw_block(
                "dialog",
                &ui,
                &input,
                font,
                Rect::new(280.0, 60.0, 720.0, 600.0),
                events,
            );
            return;
        };

        let ui = Ui::new();
        let input = ui.input();
        ui.shade(
            Rect::new(0.0, 0.0, 1280.0, 720.0),
            Color::from_rgba(0, 0, 0, 190),
        );
        ui.panel(Rect::new(280.0, 160.0, 720.0, 400.0));
        ui.wrapped_label(
            font,
            &message,
            Rect::new(320.0, 210.0, 640.0, 230.0),
            22.0,
            WHITE,
        );
        if ui.button(
            &input,
            font,
            Rect::new(710.0, 470.0, 250.0, 52.0),
            "Закрыть",
            true,
        ) {
            self.error = None;
        }
    }

    fn selected_position(&self) -> Option<Position> {
        let position = self.view.block("selection")?.get("position")?;
        Some(Position {
            x: position.get("x")?.as_i64()?,
            y: position.get("y")?.as_i64()?,
        })
    }

    fn map_tint(&self) -> Color {
        let Some(Value::List(channels)) = self.view.block("time").and_then(|v| v.get("map_tint"))
        else {
            return WHITE;
        };
        let [red, green, blue, alpha] = channels.as_slice() else {
            return WHITE;
        };
        let channel = |value: &Value| value.as_f64().unwrap_or(1.0).clamp(0.0, 1.0) as f32;
        Color::new(
            channel(red),
            channel(green),
            channel(blue),
            channel(alpha),
        )
    }
}

fn panel_width() -> f32 {
    PANEL_WIDTH.min((screen_width() * 0.45).max(1.0))
}
