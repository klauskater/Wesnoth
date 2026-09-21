//! Оболочка графического клиента.
//!
//! Этот файл отвечает только за приложение в целом:
//!
//! - создаёт системное окно и двигает цикл кадров Macroquad;
//! - показывает главное меню и экран выбора приключения;
//! - передаёт выбранное приключение в [`resource_loader::ResourceLoader`];
//! - создаёт и удаляет окно игры при входе в игру и выходе из неё.
//!
//! Здесь намеренно нет выполнения игровых правил и отрисовки игрового мира.
//! После запуска игры границей её жизненного цикла становится
//! [`screens::game::GameScreen`]. Он связывает движок с игровым интерфейсом,
//! а этот файл реагирует только на действия уровня окна, например «вернуться
//! в меню».

// Модули, из которых собран клиент. Они объявлены здесь потому, что этот файл —
// корень бинарного крейта `wesnoth-client`. Само объявление модуля не означает,
// что оболочка приложения отвечает за работу этого модуля.
#[path = "client/battle.rs"]
mod battle;
#[path = "client/connection.rs"]
mod connection;
#[path = "client/game_engine.rs"]
mod game_engine;
#[path = "client/game_ui.rs"]
mod game_ui;
use macroquad::prelude::*;
use wesnoth_engine::adventure::Adventure;

// Экспортируем стандартные подсказки для гибридной графики, чтобы
// Optimus/PowerXpress выбрал дискретную видеокарту. Intel HD 3000 не может
// создать профиль OpenGL Core, который требуется miniquad в Windows.
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
#[used]
pub static NvOptimusEnablement: u32 = 1;

#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
#[used]
pub static AmdPowerXpressRequestHighPerformance: u32 = 1;

#[path = "client/ui.rs"]
mod declarative_ui;
#[path = "client/map_renderer.rs"]
mod map_renderer;
#[path = "client/map_viewport.rs"]
mod map_viewport;
#[path = "client/movement.rs"]
mod movement;
#[path = "client/resource_loader.rs"]
mod resource_loader;
#[path = "client/scene.rs"]
mod scene;
#[path = "client/screens/mod.rs"]
mod screens;
#[path = "client/view.rs"]
mod view;
#[path = "client/village_renderer.rs"]
mod village_renderer;
#[path = "client/widgets.rs"]
mod widgets;

use widgets::{DESIGN_HEIGHT, DESIGN_WIDTH, Ui, draw_background};

const ADVENTURES_DIR: &str = "adventures";

/// Текущее окно приложения верхнего уровня.
///
/// Это не состояние самой игры. Вариант `Game` означает только «сейчас открыто
/// окно игры». Сам игровой мир принадлежит движку, а визуальное состояние —
/// модулю игрового интерфейса.
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
    // Эти ресурсы принадлежат оболочке приложения и нужны меню. Они не относятся
    // к выбранному приключению. Игровые ресурсы приключения позже загрузит
    // `ResourceLoader` и положит в отдельные контексты движка и интерфейса.
    let font = load_ttf_font_from_bytes(include_bytes!("../../assets/fonts/DejaVuSans.ttf"))
        .expect("embedded DejaVu Sans font");
    let background = Texture2D::from_file_with_format(
        include_bytes!("../../assets/wesnoth/ui/titlescreen.png"),
        Some(ImageFormat::Png),
    );
    background.set_filter(FilterMode::Linear);

    // Здесь читается только список доступных приключений. Сценарий ещё не
    // загружается, движок не создаётся, игровой интерфейс не запускается.
    let (adventures, catalog_error) = load_adventures();
    let mut screen = Screen::Main;
    let mut selected = 0usize;
    let mut game_screen: Option<screens::game::GameScreen> = None;
    let mut launch_error: Option<String> = None;

    loop {
        // Оболочка читает ввод для экранов меню. Когда открыт `Screen::Game`,
        // ввод самостоятельно читает и истолковывает игровой интерфейс внутри
        // `GameScreen::draw`.
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
                    // Это единственное место запуска игры в оболочке приложения.
                    // Загрузчик превращает выбранное приключение в два готовых
                    // контекста: один для движка, другой для интерфейса. Окно
                    // получает готовый комплект и запускает обе части. Само окно
                    // не ищет и не читает ресурсы.
                    match resource_loader::ResourceLoader::load(&adventures[selected]) {
                        Ok(resources) => match screens::game::GameScreen::new(resources) {
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
                // Один вызов обрабатывает один кадр игры. Назад в оболочку
                // приложения возвращается только команда навигации между окнами.
                // Игровые события и изменения мира остаются внутри цепочки
                // GameScreen -> GameEngine -> GameUi.
                let action = game_screen
                    .as_mut()
                    .map(|screen| screen.draw())
                    .unwrap_or(screens::game::Action::ExitToMenu);
                if matches!(action, screens::game::Action::ExitToMenu) {
                    screen = Screen::Adventures;
                }
            }
        }

        next_frame().await;
    }
}

#[cfg(not(target_os = "android"))]
fn load_adventures() -> (Vec<Adventure>, Option<String>) {
    // На компьютере каталог берётся из отдельных WML-файлов в каталоге scripts.
    // Эта функция загружает только описания приключений, но не их сценарии.
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
    // На Android во время работы нет дерева файлов проекта, поэтому те же
    // описания читаются из ресурсов, встроенных скриптом build.rs.
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
    // Ошибка в одном описании не должна скрывать остальные приключения. Рабочие
    // записи отправляются на экран выбора, а ошибки чтения и разбора возвращаются
    // рядом с ними, чтобы экран мог сообщить о неполном каталоге.
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
