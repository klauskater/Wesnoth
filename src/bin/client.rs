#[path = "client/battle.rs"]
mod battle;
use macroquad::prelude::*;
use wesnoth_engine::{adventure::Adventure, game::Game};

#[path = "client/map_renderer.rs"]
mod map_renderer;
#[path = "client/map_viewport.rs"]
mod map_viewport;
#[path = "client/movement.rs"]
mod movement;
#[path = "client/screens/mod.rs"]
mod screens;
#[path = "client/sprite_renderer.rs"]
mod sprite_renderer;
#[path = "client/village_renderer.rs"]
mod village_renderer;
#[path = "client/widgets.rs"]
mod widgets;

use widgets::{DESIGN_HEIGHT, DESIGN_WIDTH, Ui, draw_background};

const ADVENTURES_DIR: &str = "adventures";

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    Adventures,
    Game,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Wesnoth".into(),
        window_width: DESIGN_WIDTH as i32,
        window_height: DESIGN_HEIGHT as i32,
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let font = load_ttf_font_from_bytes(include_bytes!("../../assets/fonts/DejaVuSans.ttf"))
        .expect("embedded DejaVu Sans font");
    let background = Texture2D::from_file_with_format(
        include_bytes!("../../assets/wesnoth/ui/titlescreen.png"),
        Some(ImageFormat::Png),
    );
    background.set_filter(FilterMode::Linear);

    let (adventures, catalog_error) = load_adventures();
    let mut screen = Screen::Main;
    let mut selected = 0usize;
    let mut game_screen: Option<screens::game::GameScreen> = None;
    let mut launch_error: Option<String> = None;

    loop {
        let ui = Ui::new();
        let input = ui.input();
        if screen != Screen::Game {
            draw_background(&background);
        }

        match screen {
            Screen::Main => {
                if matches!(
                    screens::main_menu::draw(&ui, &input, &font, !adventures.is_empty()),
                    screens::main_menu::Action::NewGame
                ) {
                    screen = Screen::Adventures;
                }
            }
            Screen::Adventures => match screens::adventures::draw(
                &ui,
                &input,
                &font,
                &adventures,
                launch_error.as_deref().or(catalog_error.as_deref()),
                &mut selected,
            ) {
                screens::adventures::Action::None => {}
                screens::adventures::Action::Back => screen = Screen::Main,
                screens::adventures::Action::Start => {
                    launch_error = None;
                    match load_game(&adventures[selected].scenarios[0]) {
                        Ok(game) => match screens::game::GameScreen::new(game).await {
                            Ok(window) => {
                                game_screen = Some(window);
                                screen = Screen::Game;
                            }
                            Err(error) => launch_error = Some(error),
                        },
                        Err(error) => launch_error = Some(error),
                    }
                }
            },
            Screen::Game => {
                let action = game_screen
                    .as_mut()
                    .map(|screen| screen.draw(&font))
                    .unwrap_or(screens::game::Action::Back);
                if matches!(action, screens::game::Action::Back) {
                    screen = Screen::Adventures;
                }
            }
        }

        next_frame().await;
    }
}

#[cfg(not(target_os = "android"))]
fn load_adventures() -> (Vec<Adventure>, Option<String>) {
    use std::fs;

    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(ADVENTURES_DIR);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) => {
            return (
                Vec::new(),
                Some(format!("{}: {error}", directory.display())),
            );
        }
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "wml"))
        .collect::<Vec<_>>();
    paths.sort();
    load_catalog(paths.iter().map(|path| {
        let name = path.display().to_string();
        (
            name,
            fs::read_to_string(path).map_err(|error| error.to_string()),
        )
    }))
}

#[cfg(target_os = "android")]
fn load_adventures() -> (Vec<Adventure>, Option<String>) {
    let prefix = format!("{ADVENTURES_DIR}/");
    load_catalog(
        wesnoth_engine::embedded::paths(&prefix)
            .filter(|path| path.ends_with(".wml"))
            .map(|path| (path.to_owned(), wesnoth_engine::embedded::read(path))),
    )
}

fn load_catalog(
    sources: impl Iterator<Item = (String, Result<String, String>)>,
) -> (Vec<Adventure>, Option<String>) {
    let mut adventures = Vec::new();
    let mut errors = Vec::new();
    for (path, source) in sources {
        match source.and_then(|source| Adventure::parse(&source)) {
            Ok(adventure) => adventures.push(adventure),
            Err(error) => errors.push(format!("{path}: {error}")),
        }
    }
    adventures.sort_by(|left, right| left.name.cmp(&right.name));
    let error = (!errors.is_empty()).then(|| errors.join("\n"));
    (adventures, error)
}

#[cfg(not(target_os = "android"))]
fn load_game(scenario: &str) -> Result<Game, String> {
    Game::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        scenario,
    )
}

#[cfg(target_os = "android")]
fn load_game(scenario: &str) -> Result<Game, String> {
    Game::load_from(scenario, &wesnoth_engine::embedded::read)
}
