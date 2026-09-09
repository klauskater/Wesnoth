use macroquad::prelude::*;

use crate::widgets::{Input, Ui};

pub enum Action {
    None,
    NewGame,
}

pub fn draw(ui: &Ui, input: &Input, font: &Font, has_adventures: bool) -> Action {
    ui.label(font, "WESNOTH", 64.0, 112.0, 62.0, GOLD);
    ui.label(
        font,
        "Битва за королевство",
        68.0,
        150.0,
        23.0,
        Color::from_rgba(230, 221, 190, 255),
    );

    ui.panel(Rect::new(810.0, 62.0, 416.0, 596.0));
    ui.label(font, "Главное меню", 840.0, 117.0, 27.0, GOLD);
    ui.button(
        input,
        font,
        Rect::new(838.0, 154.0, 360.0, 80.0),
        "Продолжить",
        false,
    );
    let new_game = ui.button(
        input,
        font,
        Rect::new(838.0, 244.0, 360.0, 80.0),
        "Новая игра",
        has_adventures,
    );
    ui.button(
        input,
        font,
        Rect::new(838.0, 334.0, 360.0, 80.0),
        "Настройки",
        false,
    );

    if new_game {
        Action::NewGame
    } else {
        Action::None
    }
}
