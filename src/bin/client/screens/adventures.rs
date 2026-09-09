use macroquad::prelude::*;
use wesnoth_engine::adventure::Adventure;

use crate::widgets::{DESIGN_HEIGHT, DESIGN_WIDTH, Input, Ui};

pub enum Action {
    None,
    Back,
    Start,
}

pub fn draw(
    ui: &Ui,
    input: &Input,
    font: &Font,
    adventures: &[Adventure],
    error: Option<&str>,
    selected: &mut usize,
) -> Action {
    ui.shade(
        Rect::new(0.0, 0.0, DESIGN_WIDTH, DESIGN_HEIGHT),
        Color::from_rgba(10, 15, 20, 185),
    );
    ui.label(font, "Приключения", 38.0, 76.0, 36.0, WHITE);

    if adventures.is_empty() {
        ui.panel(Rect::new(38.0, 112.0, 1204.0, 480.0));
        ui.label(font, "Приключения не найдены", 72.0, 172.0, 30.0, GOLD);
        ui.wrapped_label(
            font,
            error.unwrap_or("Папка adventures пуста"),
            Rect::new(72.0, 205.0, 1130.0, 280.0),
            21.0,
            LIGHTGRAY,
        );
    } else {
        *selected = (*selected).min(adventures.len() - 1);
        for (index, adventure) in adventures.iter().enumerate() {
            let rect = Rect::new(38.0, 112.0 + index as f32 * 86.0, 480.0, 76.0);
            if ui.button(input, font, rect, &adventure.name, true) {
                *selected = index;
            }
            if index == *selected {
                ui.outline(rect, 3.0, GOLD);
            }
        }

        let adventure = &adventures[*selected];
        ui.panel(Rect::new(552.0, 112.0, 690.0, 480.0));
        ui.label(font, &adventure.name, 584.0, 174.0, 34.0, WHITE);
        ui.wrapped_label(
            font,
            &adventure.description,
            Rect::new(584.0, 202.0, 626.0, 230.0),
            22.0,
            LIGHTGRAY,
        );
        ui.label(
            font,
            &format!("Сценариев: {}", adventure.scenarios.len()),
            584.0,
            540.0,
            21.0,
            GOLD,
        );
        if let Some(error) = error {
            ui.wrapped_label(
                font,
                error,
                Rect::new(584.0, 555.0, 626.0, 45.0),
                16.0,
                Color::from_rgba(235, 120, 110, 255),
            );
        }
    }

    let start = !adventures.is_empty()
        && ui.button(
            input,
            font,
            Rect::new(552.0, 620.0, 690.0, 68.0),
            "Начать приключение",
            true,
        );
    let back = ui.button(
        input,
        font,
        Rect::new(38.0, 620.0, 300.0, 68.0),
        "Назад",
        true,
    );
    if start {
        Action::Start
    } else if back {
        Action::Back
    } else {
        Action::None
    }
}
