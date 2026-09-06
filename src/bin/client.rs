use std::collections::BTreeSet;
#[cfg(not(target_os = "android"))]
use std::path::PathBuf;

use macroquad::prelude::*;
use wesnoth_engine::{
    engine::{Map, Position},
    game::{DialogLine, Game, GameSnapshot},
    value::Value,
};

const HEX_RADIUS: f32 = 36.0;
const MAP_ORIGIN: Vec2 = vec2(65.0, 75.0);
#[cfg(not(target_os = "android"))]
const PANEL_X: f32 = 800.0;
#[cfg(target_os = "android")]
const ANDROID_PANEL_WIDTH: f32 = 420.0;
#[cfg(target_os = "android")]
const ANDROID_TOP_BAR_HEIGHT: f32 = 84.0;
#[cfg(any(target_os = "android", test))]
const ANDROID_TAP_SLOP: f32 = 24.0;
const SCENARIOS: [&str; 7] = [
    "scenarios/first_battle.wml",
    "scenarios/crossing.wml",
    "scenarios/outpost_defense.wml",
    "scenarios/rooting_out_a_mage.wml",
    "scenarios/the_chase.wml",
    "scenarios/guarded_castle.wml",
    "scenarios/return_to_the_village.wml",
];

#[cfg(target_os = "android")]
const SCENARIO_NAMES: [&str; 7] = [
    "Первая битва",
    "Переправа",
    "Оборона заставы",
    "Искоренение мага",
    "Погоня",
    "Охраняемый замок",
    "Возвращение в деревню",
];

#[cfg(target_os = "android")]
#[derive(Clone, Copy, PartialEq)]
enum AppScreen {
    MainMenu,
    Scenarios,
    Settings,
    Game,
}

#[cfg(target_os = "android")]
struct MissionResult {
    victory: bool,
    next: Option<String>,
}

#[cfg(target_os = "android")]
struct ClientSettings {
    show_grid: bool,
    show_fps: bool,
}

struct DialogView {
    lines: Vec<DialogLine>,
    current: usize,
}

struct ClientSnapshot {
    map: Map,
    objects: Vec<ObjectView>,
    turn: i64,
    active_side: String,
    finished: bool,
    can_end_turn: bool,
    objective: Option<String>,
    #[cfg(target_os = "android")]
    result: Option<String>,
    #[cfg(target_os = "android")]
    turn_limit: i64,
    #[cfg(target_os = "android")]
    carryover_percentage: i64,
    gold: i64,
    recruit_types: Vec<String>,
    #[cfg(target_os = "android")]
    recruit_options: Vec<RecruitOption>,
    #[cfg(target_os = "android")]
    recall_options: Vec<RecruitOption>,
    recall_units: Vec<String>,
    villages: Vec<VillageView>,
    village_income: i64,
    gross_income: i64,
    expenses: i64,
    net_income: i64,
    time_of_day: String,
    lawful_bonus: i64,
    pending_advancement: Option<AdvancementView>,
    fog: bool,
    visible_cells: Vec<Position>,
    shroud: bool,
    revealed_cells: Vec<Position>,
}

struct AdvancementView {
    unit: String,
    options: Vec<String>,
}

struct VillageView {
    position: Position,
    side: Option<String>,
}

struct ObjectView {
    id: String,
    #[cfg(target_os = "android")]
    type_id: String,
    side: String,
    position: Position,
    hitpoints: i64,
    max_hitpoints: i64,
    movement_points: i64,
    max_movement_points: i64,
    attacks_left: i64,
    level: i64,
    experience: i64,
    max_experience: i64,
    poisoned: bool,
    slowed: bool,
    petrified: bool,
    unhealable: bool,
    stunned: bool,
    movement_costs: std::collections::BTreeMap<String, i64>,
    #[cfg(target_os = "android")]
    defense: std::collections::BTreeMap<String, i64>,
    #[cfg(any(target_os = "android", test))]
    attacks: Vec<String>,
}

struct ReachableCell {
    position: Position,
    path: Vec<Position>,
    cost: i64,
    stopped_by_zoc: bool,
}

#[cfg(target_os = "android")]
struct MovePreview {
    destination: Position,
    path: Vec<Position>,
    cost: i64,
    defense: i64,
    stopped_by_zoc: bool,
}

#[cfg(target_os = "android")]
struct CombatForecast {
    chance: f64,
    damage: f64,
    strikes: f64,
    expected_damage: f64,
    kill_probability: f64,
    retaliation_chance: f64,
    retaliation_damage: f64,
    retaliation_strikes: f64,
    expected_retaliation: f64,
    death_probability: f64,
    retaliation_name: String,
}

#[cfg(target_os = "android")]
struct CombatDialog {
    attacker: String,
    defender: String,
    weapon: usize,
    forecasts: Vec<CombatForecast>,
}

#[cfg(target_os = "android")]
struct RecruitOption {
    id: String,
    type_id: String,
    name: String,
    cost: i64,
    max_hitpoints: i64,
    max_moves: i64,
    level: i64,
    alignment: String,
    attacks: Vec<String>,
    hitpoints: i64,
    experience: i64,
    max_experience: i64,
}

#[cfg(target_os = "android")]
#[derive(Clone, Copy)]
struct RecruitMenu {
    destination: Position,
    selected: usize,
    veterans: bool,
}

#[derive(Default)]
struct AvailableActions {
    reachable: Vec<ReachableCell>,
    targets: BTreeSet<String>,
    attacks: Vec<String>,
}

#[cfg(target_os = "android")]
struct AndroidArt {
    terrain: std::collections::BTreeMap<&'static str, Texture2D>,
    water_edges: [Texture2D; 6],
    units: std::collections::BTreeMap<&'static str, Texture2D>,
    sidebar: Texture2D,
    status_icons: [Texture2D; 5],
    time_icons: std::collections::BTreeMap<&'static str, Texture2D>,
}

#[cfg(target_os = "android")]
impl AndroidArt {
    fn load() -> Self {
        fn texture(bytes: &'static [u8]) -> Texture2D {
            let texture = Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png));
            texture.set_filter(FilterMode::Nearest);
            texture
        }
        let terrain = [
            (
                "grassland",
                include_bytes!("../../assets/wesnoth/terrain/grassland.png").as_slice(),
            ),
            (
                "forest",
                include_bytes!("../../assets/wesnoth/terrain/forest.png").as_slice(),
            ),
            (
                "hills",
                include_bytes!("../../assets/wesnoth/terrain/hills.png").as_slice(),
            ),
            (
                "water",
                include_bytes!(
                    "../../../Wesnoth-upstream/data/core/images/terrain/water/coast-tile.png"
                )
                .as_slice(),
            ),
            (
                "castle",
                include_bytes!("../../assets/wesnoth/terrain/castle.png").as_slice(),
            ),
            (
                "keep",
                include_bytes!("../../assets/wesnoth/terrain/keep.png").as_slice(),
            ),
            (
                "village",
                include_bytes!("../../assets/wesnoth/terrain/village.png").as_slice(),
            ),
        ]
        .into_iter()
        .map(|(id, bytes)| (id, texture(bytes)))
        .collect();
        let water_edges = [
            include_bytes!(
                "../../../Wesnoth-upstream/data/core/images/terrain/water/coast-tropical-A01-n.png"
            )
            .as_slice(),
            include_bytes!(
                "../../../Wesnoth-upstream/data/core/images/terrain/water/coast-tropical-A01-ne.png"
            )
            .as_slice(),
            include_bytes!(
                "../../../Wesnoth-upstream/data/core/images/terrain/water/coast-tropical-A01-se.png"
            )
            .as_slice(),
            include_bytes!(
                "../../../Wesnoth-upstream/data/core/images/terrain/water/coast-tropical-A01-s.png"
            )
            .as_slice(),
            include_bytes!(
                "../../../Wesnoth-upstream/data/core/images/terrain/water/coast-tropical-A01-sw.png"
            )
            .as_slice(),
            include_bytes!(
                "../../../Wesnoth-upstream/data/core/images/terrain/water/coast-tropical-A01-nw.png"
            )
            .as_slice(),
        ]
        .map(texture);
        let units = [
            (
                "spearman",
                include_bytes!("../../assets/wesnoth/units/spearman.png").as_slice(),
            ),
            (
                "bowman",
                include_bytes!("../../assets/wesnoth/units/bowman.png").as_slice(),
            ),
            (
                "knight",
                include_bytes!("../../assets/wesnoth/units/knight.png").as_slice(),
            ),
            (
                "elvish_fighter",
                include_bytes!("../../assets/wesnoth/units/elvish_fighter.png").as_slice(),
            ),
            (
                "elvish_archer",
                include_bytes!("../../assets/wesnoth/units/elvish_archer.png").as_slice(),
            ),
            (
                "elvish_hero",
                include_bytes!("../../assets/wesnoth/units/elvish_hero.png").as_slice(),
            ),
            (
                "elvish_marksman",
                include_bytes!("../../assets/wesnoth/units/elvish_marksman.png").as_slice(),
            ),
            (
                "elvish_ranger",
                include_bytes!("../../assets/wesnoth/units/elvish_ranger.png").as_slice(),
            ),
            (
                "orcish_grunt",
                include_bytes!("../../assets/wesnoth/units/orcish_grunt.png").as_slice(),
            ),
            (
                "orcish_warrior",
                include_bytes!("../../assets/wesnoth/units/orcish_warrior.png").as_slice(),
            ),
            (
                "orcish_raider",
                include_bytes!("../../assets/wesnoth/units/orcish_raider.png").as_slice(),
            ),
            (
                "red_mage",
                include_bytes!("../../assets/wesnoth/units/red_mage.png").as_slice(),
            ),
            (
                "silver_mage",
                include_bytes!("../../assets/wesnoth/units/silver_mage.png").as_slice(),
            ),
            (
                "dark_sorcerer",
                include_bytes!("../../assets/wesnoth/units/dark_sorcerer.png").as_slice(),
            ),
            (
                "walking_corpse",
                include_bytes!("../../assets/wesnoth/units/walking_corpse.png").as_slice(),
            ),
            (
                "cockatrice",
                include_bytes!("../../assets/wesnoth/units/cockatrice.png").as_slice(),
            ),
        ]
        .into_iter()
        .map(|(id, bytes)| (id, texture(bytes)))
        .collect();
        let sidebar = texture(include_bytes!(
            "../../../Wesnoth-upstream/data/core/images/themes/game/classic/sidebar.png"
        ));
        let status_icons = ["gold", "villages", "units", "upkeep", "income"].map(|name| {
            let bytes = match name {
                "gold" => {
                    include_bytes!("../../../Wesnoth-upstream/data/core/images/themes/gold.png")
                        .as_slice()
                }
                "villages" => {
                    include_bytes!("../../../Wesnoth-upstream/data/core/images/themes/villages.png")
                        .as_slice()
                }
                "units" => {
                    include_bytes!("../../../Wesnoth-upstream/data/core/images/themes/units.png")
                        .as_slice()
                }
                "upkeep" => {
                    include_bytes!("../../../Wesnoth-upstream/data/core/images/themes/upkeep.png")
                        .as_slice()
                }
                _ => include_bytes!("../../../Wesnoth-upstream/data/core/images/themes/income.png")
                    .as_slice(),
            };
            texture(bytes)
        });
        let time_icons = [
            ("dawn", include_bytes!("../../../Wesnoth-upstream/data/core/images/misc/time-schedules/default/schedule-dawn.png").as_slice()),
            ("morning", include_bytes!("../../../Wesnoth-upstream/data/core/images/misc/time-schedules/default/schedule-morning.png").as_slice()),
            ("afternoon", include_bytes!("../../../Wesnoth-upstream/data/core/images/misc/time-schedules/default/schedule-afternoon.png").as_slice()),
            ("dusk", include_bytes!("../../../Wesnoth-upstream/data/core/images/misc/time-schedules/default/schedule-dusk.png").as_slice()),
            ("first_watch", include_bytes!("../../../Wesnoth-upstream/data/core/images/misc/time-schedules/default/schedule-firstwatch.png").as_slice()),
            ("second_watch", include_bytes!("../../../Wesnoth-upstream/data/core/images/misc/time-schedules/default/schedule-secondwatch.png").as_slice()),
        ]
        .into_iter()
        .map(|(id, bytes)| (id, texture(bytes)))
        .collect();
        Self {
            terrain,
            water_edges,
            units,
            sidebar,
            status_icons,
            time_icons,
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Wesnoth debug client".into(),
        window_width: 1200,
        window_height: 800,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let font = load_ttf_font_from_bytes(include_bytes!("../../assets/fonts/DejaVuSans.ttf"))
        .expect("embedded DejaVu Sans font");
    #[cfg(target_os = "android")]
    let art = AndroidArt::load();
    #[cfg(target_os = "android")]
    let mut viewport = AndroidViewport::new();
    #[cfg(target_os = "android")]
    exclude_android_system_gestures();
    #[cfg(target_os = "android")]
    let mut screen = AppScreen::MainMenu;
    #[cfg(target_os = "android")]
    let mut settings = ClientSettings {
        show_grid: true,
        show_fps: false,
    };
    #[cfg(target_os = "android")]
    let mut menu_message = String::new();
    let mut scenario_index = 2;
    let mut game = load_game(scenario_index);
    let mut selected: Option<String> = None;
    let mut target: Option<String> = None;
    let mut weapon = 0usize;
    let mut actions = AvailableActions::default();
    let mut log = Vec::new();
    let mut dialog = None;
    #[cfg(target_os = "android")]
    let mut move_preview: Option<MovePreview> = None;
    #[cfg(target_os = "android")]
    let mut combat_dialog: Option<CombatDialog> = None;
    #[cfg(target_os = "android")]
    let mut recruit_menu: Option<RecruitMenu> = None;
    #[cfg(target_os = "android")]
    let mut confirm_end_turn = false;
    #[cfg(target_os = "android")]
    let mut inspected_hex: Option<Position> = None;
    #[cfg(target_os = "android")]
    let mut panel_scroll = 0.0_f32;
    #[cfg(target_os = "android")]
    let mut panel_drag_y: Option<f32> = None;
    #[cfg(target_os = "android")]
    let mut mission_briefing = false;
    #[cfg(target_os = "android")]
    let mut mission_result: Option<MissionResult> = None;
    receive_events(
        game.start_events().unwrap_or_default(),
        &mut log,
        &mut dialog,
    );

    loop {
        #[cfg(target_os = "android")]
        if screen != AppScreen::Game {
            if let Some(action) = draw_android_menu(&font, screen, &settings, &menu_message) {
                match action {
                    MenuAction::Continue => match load_android_save() {
                        Ok(loaded) => {
                            game = loaded;
                            selected = None;
                            target = None;
                            actions = AvailableActions::default();
                            log.clear();
                            dialog = None;
                            move_preview = None;
                            combat_dialog = None;
                            mission_briefing = false;
                            mission_result = None;
                            screen = AppScreen::Game;
                        }
                        Err(error) => menu_message = error,
                    },
                    MenuAction::NewGame => {
                        screen = AppScreen::Scenarios;
                    }
                    MenuAction::OpenSettings => screen = AppScreen::Settings,
                    MenuAction::Scenario(index) => {
                        scenario_index = index;
                        game = load_game(scenario_index);
                        reset_game_ui(
                            &mut selected,
                            &mut target,
                            &mut weapon,
                            &mut actions,
                            &mut log,
                            &mut dialog,
                            &mut game,
                        );
                        move_preview = None;
                        combat_dialog = None;
                        mission_briefing = true;
                        mission_result = None;
                        screen = AppScreen::Game;
                    }
                    MenuAction::ToggleGrid => settings.show_grid = !settings.show_grid,
                    MenuAction::ToggleFps => settings.show_fps = !settings.show_fps,
                    MenuAction::Back => screen = AppScreen::MainMenu,
                }
            }
            next_frame().await;
            continue;
        }

        let requested_scenario = if is_key_pressed(KeyCode::F1) {
            Some(0)
        } else if is_key_pressed(KeyCode::F2) {
            Some(1)
        } else if is_key_pressed(KeyCode::F3) {
            Some(2)
        } else if is_key_pressed(KeyCode::F4) {
            Some(3)
        } else if is_key_pressed(KeyCode::F5) {
            Some(4)
        } else if is_key_pressed(KeyCode::F6) {
            Some(5)
        } else if is_key_pressed(KeyCode::F7) {
            Some(6)
        } else if is_key_pressed(KeyCode::R) {
            Some(scenario_index)
        } else {
            None
        };
        if let Some(index) = requested_scenario {
            scenario_index = index;
            game = load_game(scenario_index);
            selected = None;
            target = None;
            weapon = 0;
            actions = AvailableActions::default();
            log.clear();
            dialog = None;
            receive_events(
                game.start_events().unwrap_or_default(),
                &mut log,
                &mut dialog,
            );
            #[cfg(target_os = "android")]
            {
                move_preview = None;
                combat_dialog = None;
            }
        }

        let snapshot = read_snapshot(
            game.snapshot().expect("game snapshot"),
            game.query("status", Value::Nil).expect("game status"),
        )
        .expect("UI snapshot");
        #[cfg(target_os = "android")]
        {
            let point = Vec2::from(mouse_position());
            let panel_x = screen_width() - ANDROID_PANEL_WIDTH;
            let scroll_top = ANDROID_TOP_BAR_HEIGHT + 76.0;
            let scroll_bottom = cancel_selection_rect().y - 6.0;
            if is_mouse_button_pressed(MouseButton::Left)
                && point.x >= panel_x
                && point.y >= scroll_top
                && point.y < scroll_bottom
            {
                panel_drag_y = Some(point.y);
            } else if is_mouse_button_down(MouseButton::Left)
                && let Some(previous) = panel_drag_y.as_mut()
            {
                panel_scroll += *previous - point.y;
                *previous = point.y;
            }
            if is_mouse_button_released(MouseButton::Left) {
                panel_drag_y = None;
            }
            panel_scroll = panel_scroll.clamp(
                0.0,
                android_panel_max_scroll(&snapshot, selected.as_deref()),
            );
        }
        if selected
            .as_ref()
            .is_some_and(|id| snapshot.objects.iter().all(|object| object.id != *id))
        {
            selected = None;
            actions = AvailableActions::default();
        }
        if target
            .as_ref()
            .is_some_and(|id| snapshot.objects.iter().all(|object| object.id != *id))
        {
            target = None;
        }
        let confirm = is_key_pressed(KeyCode::Enter)
            || is_key_pressed(KeyCode::Space)
            || is_mouse_button_pressed(MouseButton::Left);
        #[cfg(target_os = "android")]
        let touch = AndroidTouch::read();
        #[cfg(target_os = "android")]
        let touch_start =
            is_mouse_button_pressed(MouseButton::Left).then(|| Vec2::from(mouse_position()));
        #[cfg(target_os = "android")]
        let gesture = if combat_dialog.is_none() && recruit_menu.is_none() && !confirm_end_turn {
            viewport.update()
        } else {
            ViewportGesture::default()
        };
        #[cfg(target_os = "android")]
        let map_press = gesture.tap.map(|point| viewport.screen_to_world(point));
        #[cfg(not(target_os = "android"))]
        let map_press =
            is_mouse_button_pressed(MouseButton::Left).then(|| Vec2::from(mouse_position()));
        #[cfg(not(target_os = "android"))]
        let pointer_consumed = false;
        #[cfg(target_os = "android")]
        let pointer_consumed = touch.consumed;

        #[cfg(target_os = "android")]
        let ui_point = touch_start;
        #[cfg(target_os = "android")]
        let mut mission_overlay_consumed = false;
        #[cfg(target_os = "android")]
        if let Some(point) = ui_point {
            if mission_briefing && mission_start_rect().contains(point) {
                mission_briefing = false;
                mission_overlay_consumed = true;
            } else if mission_result.is_some() {
                mission_overlay_consumed = true;
                let next = mission_result
                    .as_ref()
                    .and_then(|result| result.next.clone());
                if mission_menu_rect().contains(point) {
                    mission_result = None;
                    screen = AppScreen::MainMenu;
                    next_frame().await;
                    continue;
                } else if mission_replay_rect().contains(point) {
                    game = load_game(scenario_index);
                    reset_game_ui(
                        &mut selected,
                        &mut target,
                        &mut weapon,
                        &mut actions,
                        &mut log,
                        &mut dialog,
                        &mut game,
                    );
                    mission_result = None;
                    mission_briefing = true;
                    next_frame().await;
                    continue;
                } else if let Some(path) = next
                    && mission_next_rect().contains(point)
                {
                    match game
                        .campaign_state()
                        .and_then(|state| load_game_with_campaign(&path, &state))
                    {
                        Ok(next_game) => {
                            game = next_game;
                            scenario_index = SCENARIOS
                                .iter()
                                .position(|item| *item == path)
                                .unwrap_or(scenario_index);
                            reset_game_ui(
                                &mut selected,
                                &mut target,
                                &mut weapon,
                                &mut actions,
                                &mut log,
                                &mut dialog,
                                &mut game,
                            );
                            mission_result = None;
                            mission_briefing = true;
                            next_frame().await;
                            continue;
                        }
                        Err(error) => log.push(format!("Transition error: {error}")),
                    }
                }
            }
        }
        #[cfg(target_os = "android")]
        let mut confirmed_end_turn = false;
        #[cfg(target_os = "android")]
        if confirm_end_turn && let Some(point) = ui_point {
            if end_turn_confirm_rect().contains(point) {
                confirm_end_turn = false;
                confirmed_end_turn = true;
            } else if end_turn_cancel_rect().contains(point) {
                confirm_end_turn = false;
            }
        }
        #[cfg(not(target_os = "android"))]
        let confirmed_end_turn = false;
        #[cfg(target_os = "android")]
        let cancel_selection = recruit_menu.is_none()
            && combat_dialog.is_none()
            && !confirm_end_turn
            && ui_point.is_some_and(|point| cancel_selection_rect().contains(point));
        #[cfg(target_os = "android")]
        let end_turn_button = recruit_menu.is_none()
            && combat_dialog.is_none()
            && !confirm_end_turn
            && ui_point.is_some_and(|point| end_turn_rect().contains(point));
        #[cfg(not(target_os = "android"))]
        let end_turn_button = false;
        #[cfg(target_os = "android")]
        if cancel_selection {
            selected = None;
            target = None;
            actions = AvailableActions::default();
            move_preview = None;
            combat_dialog = None;
        }

        #[cfg(target_os = "android")]
        if touch_menu() {
            menu_message = match save_android_game(&game) {
                Ok(()) => "Игра сохранена".into(),
                Err(error) => error,
            };
            screen = AppScreen::MainMenu;
            next_frame().await;
            continue;
        }

        #[cfg(target_os = "android")]
        if combat_dialog.is_some() && is_mouse_button_pressed(MouseButton::Left) {
            let point = Vec2::from(mouse_position());
            if combat_cancel_rect().contains(point) {
                combat_dialog = None;
                target = None;
            } else if combat_confirm_rect().contains(point) {
                let combat = combat_dialog.take().expect("combat dialog");
                if let Some(weapon_id) = actions.attacks.get(combat.weapon) {
                    match game.execute(
                        "resolve",
                        attack_command(&combat.attacker, &combat.defender, weapon_id),
                    ) {
                        Ok(events) => {
                            receive_events(events, &mut log, &mut dialog);
                            actions =
                                read_actions(&game, &combat.attacker, false).unwrap_or_default();
                        }
                        Err(error) => log.push(format!("Error: {error}")),
                    }
                }
                target = None;
                move_preview = None;
            } else if let Some(combat) = combat_dialog.as_mut() {
                for index in 0..combat.forecasts.len() {
                    if combat_weapon_rect(index).contains(point) {
                        combat.weapon = index;
                    }
                }
            }
        }

        #[cfg(target_os = "android")]
        let combat_modal = combat_dialog.is_some()
            || confirm_end_turn
            || mission_briefing
            || mission_result.is_some()
            || mission_overlay_consumed;
        #[cfg(not(target_os = "android"))]
        let combat_modal = false;

        #[cfg(target_os = "android")]
        if let Some(menu) = recruit_menu.as_mut() {
            if let Some(point) = ui_point {
                if recruit_cancel_rect().contains(point) {
                    recruit_menu = None;
                } else if recruit_tab_rect(false).contains(point) {
                    menu.veterans = false;
                    menu.selected = 0;
                } else if recruit_tab_rect(true).contains(point) {
                    menu.veterans = true;
                    menu.selected = 0;
                } else if recruit_confirm_rect().contains(point) {
                    let options = if menu.veterans {
                        &snapshot.recall_options
                    } else {
                        &snapshot.recruit_options
                    };
                    if let Some(option) = options.get(menu.selected) {
                        let key = if menu.veterans { "unit" } else { "unit_type" };
                        let command = Value::Map(std::collections::BTreeMap::from([
                            (key.into(), Value::String(option.id.clone())),
                            ("destination".into(), position_value(menu.destination)),
                        ]));
                        match game
                            .execute(if menu.veterans { "recall" } else { "recruit" }, command)
                        {
                            Ok(events) => receive_events(events, &mut log, &mut dialog),
                            Err(error) => log.push(format!("Error: {error}")),
                        }
                        recruit_menu = None;
                    }
                } else {
                    let option_count = if menu.veterans {
                        snapshot.recall_options.len()
                    } else {
                        snapshot.recruit_options.len()
                    };
                    for index in 0..option_count.min(4) {
                        if recruit_type_rect(index).contains(point) {
                            menu.selected = index;
                            break;
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "android")]
        if recruit_menu.is_none()
            && !combat_modal
            && dialog.is_none()
            && let Some(point) = touch_start
                .filter(|point| !touch_blocked(*point))
                .map(|point| viewport.screen_to_world(point))
            && let Some(object) = object_at(&snapshot, point)
            && let Ok(available) = read_actions(&game, &object.id, false)
        {
            selected = Some(object.id.clone());
            target = None;
            weapon = 0;
            actions = available;
            move_preview = None;
        }

        #[cfg(target_os = "android")]
        if recruit_menu.is_none()
            && (!snapshot.recruit_options.is_empty() || !snapshot.recall_options.is_empty())
            && let Some(point) = gesture
                .long_press
                .map(|point| viewport.screen_to_world(point))
            && let Some(position) = hex_at(&snapshot, point)
        {
            viewport.cancel_gesture();
            recruit_menu = Some(RecruitMenu {
                destination: position,
                selected: 0,
                veterans: snapshot.recruit_options.is_empty()
                    && !snapshot.recall_options.is_empty(),
            });
            inspected_hex = Some(position);
            move_preview = None;
        }

        #[cfg(target_os = "android")]
        let mission_modal =
            mission_briefing || mission_result.is_some() || mission_overlay_consumed;
        #[cfg(not(target_os = "android"))]
        let mission_modal = false;
        if !mission_modal && let Some(view) = dialog.as_mut() {
            if confirm {
                if view.current + 1 < view.lines.len() {
                    view.current += 1;
                } else {
                    dialog = None;
                    if let Err(error) = game.acknowledge_dialog() {
                        log.push(format!("Error: {error}"));
                    }
                }
            }
        } else if !combat_modal
            && !snapshot.finished
            && let Some(mouse) = map_press.filter(|_| !pointer_consumed)
        {
            #[cfg(target_os = "android")]
            {
                inspected_hex = hex_at(&snapshot, mouse);
            }
            if let Some(object) = object_at(&snapshot, mouse) {
                if object.side == snapshot.active_side {
                    if let Ok(available) = read_actions(&game, &object.id, false) {
                        selected = Some(object.id.clone());
                        target = None;
                        weapon = 0;
                        actions = available;
                        #[cfg(target_os = "android")]
                        {
                            move_preview = None;
                            combat_dialog = None;
                        }
                    }
                } else if selected.is_some() && actions.targets.contains(&object.id) {
                    target = Some(object.id.clone());
                    #[cfg(target_os = "android")]
                    if let Some(attacker) = selected.as_deref() {
                        match read_combat_dialog(&game, attacker, &object.id, &actions.attacks) {
                            Ok(preview) => combat_dialog = Some(preview),
                            Err(error) => log.push(format!("Error: {error}")),
                        }
                    }
                } else if let Ok(available) = read_actions(&game, &object.id, true) {
                    selected = Some(object.id.clone());
                    target = None;
                    weapon = 0;
                    actions = available;
                    #[cfg(target_os = "android")]
                    {
                        move_preview = None;
                        combat_dialog = None;
                    }
                }
            } else if let (Some(id), Some(destination)) =
                (selected.as_deref(), hex_at(&snapshot, mouse))
            {
                let controllable = snapshot
                    .objects
                    .iter()
                    .any(|unit| unit.id == id && unit.side == snapshot.active_side);
                if controllable
                    && let Some(cell) = actions
                        .reachable
                        .iter()
                        .find(|cell| cell.position == destination)
                {
                    #[cfg(target_os = "android")]
                    if move_preview
                        .as_ref()
                        .is_none_or(|preview| preview.destination != destination)
                    {
                        let unit = snapshot.objects.iter().find(|unit| unit.id == id);
                        let terrain = snapshot.map.get(destination).unwrap_or("grassland");
                        move_preview = Some(MovePreview {
                            destination,
                            path: cell.path.clone(),
                            cost: cell.cost,
                            defense: unit
                                .and_then(|unit| unit.defense.get(terrain))
                                .copied()
                                .unwrap_or(100),
                            stopped_by_zoc: cell.stopped_by_zoc,
                        });
                        next_frame().await;
                        continue;
                    }
                    let object = id.to_owned();
                    match game.execute("move", object_command(&object, Some(destination))) {
                        Ok(events) => {
                            receive_events(events, &mut log, &mut dialog);
                            actions = read_actions(&game, &object, false).unwrap_or_default();
                        }
                        Err(error) => log.push(format!("Error: {error}")),
                    }
                    target = None;
                    #[cfg(target_os = "android")]
                    {
                        move_preview = None;
                    }
                }
            }
        }

        if dialog.is_none() {
            if let Some(pending) = &snapshot.pending_advancement {
                for (index, key) in [KeyCode::Z, KeyCode::X, KeyCode::C, KeyCode::V]
                    .into_iter()
                    .enumerate()
                {
                    if is_key_pressed(key) {
                        if let Some(choice) = pending.options.get(index) {
                            match game.execute(
                                "advance",
                                Value::Map(std::collections::BTreeMap::from([
                                    ("unit".into(), Value::String(pending.unit.clone())),
                                    ("choice".into(), Value::String(choice.clone())),
                                ])),
                            ) {
                                Ok(events) => receive_events(events, &mut log, &mut dialog),
                                Err(error) => log.push(format!("Error: {error}")),
                            }
                        }
                    }
                }
            }
        }

        if dialog.is_none() && !snapshot.finished && snapshot.pending_advancement.is_none() {
            for (index, key) in [KeyCode::Q, KeyCode::W].into_iter().enumerate() {
                if is_key_pressed(key) {
                    if let Some(unit_type) = snapshot.recruit_types.get(index) {
                        match game.execute(
                            "recruit",
                            Value::Map(std::collections::BTreeMap::from([(
                                "unit_type".into(),
                                Value::String(unit_type.clone()),
                            )])),
                        ) {
                            Ok(events) => receive_events(events, &mut log, &mut dialog),
                            Err(error) => log.push(format!("Error: {error}")),
                        }
                    }
                }
            }
            if is_key_pressed(KeyCode::U) {
                if let Some(unit) = snapshot.recall_units.first() {
                    let destination = (1..=snapshot.map.height as i64)
                        .flat_map(|y| {
                            (1..=snapshot.map.width as i64).map(move |x| Position { x, y })
                        })
                        .find(|position| {
                            snapshot.map.get(*position) == Ok("castle")
                                && snapshot
                                    .objects
                                    .iter()
                                    .all(|object| object.position != *position)
                        });
                    if let Some(destination) = destination {
                        let command = Value::Map(std::collections::BTreeMap::from([
                            ("unit".into(), Value::String(unit.clone())),
                            (
                                "destination".into(),
                                Value::Map(std::collections::BTreeMap::from([
                                    ("x".into(), Value::Integer(destination.x)),
                                    ("y".into(), Value::Integer(destination.y)),
                                ])),
                            ),
                        ]));
                        match game.execute("recall", command) {
                            Ok(events) => receive_events(events, &mut log, &mut dialog),
                            Err(error) => log.push(format!("Error: {error}")),
                        }
                    }
                }
            }
            let end_turn_requested = is_key_pressed(KeyCode::E)
                || cfg!(target_os = "android") && (end_turn_button || confirmed_end_turn);
            #[cfg(target_os = "android")]
            let end_turn_now = if end_turn_requested
                && !confirmed_end_turn
                && snapshot
                    .objects
                    .iter()
                    .any(|unit| unit.side == snapshot.active_side && unit.movement_points > 0)
            {
                confirm_end_turn = true;
                false
            } else {
                end_turn_requested
            };
            #[cfg(not(target_os = "android"))]
            let end_turn_now = end_turn_requested;
            if snapshot.can_end_turn && end_turn_now {
                match game.execute("end_turn", Value::Nil) {
                    Ok(events) => {
                        receive_events(events, &mut log, &mut dialog);
                        selected = None;
                        target = None;
                        actions = AvailableActions::default();
                    }
                    Err(error) => log.push(format!("Error: {error}")),
                }
            }
            if selected.is_some() {
                for index in 0..actions.attacks.len().min(9) {
                    let key = match index {
                        0 => KeyCode::Key1,
                        1 => KeyCode::Key2,
                        2 => KeyCode::Key3,
                        3 => KeyCode::Key4,
                        4 => KeyCode::Key5,
                        5 => KeyCode::Key6,
                        6 => KeyCode::Key7,
                        7 => KeyCode::Key8,
                        _ => KeyCode::Key9,
                    };
                    if is_key_pressed(key) {
                        weapon = index;
                    }
                }
            }

            if is_key_pressed(KeyCode::Enter) {
                if let (Some(attacker), Some(defender)) = (selected.as_ref(), target.as_ref()) {
                    if let Some(weapon_id) = actions.attacks.get(weapon) {
                        match game.execute(
                            "resolve",
                            Value::Map(std::collections::BTreeMap::from([
                                ("attacker".into(), Value::String(attacker.clone())),
                                ("defender".into(), Value::String(defender.clone())),
                                ("weapon".into(), Value::String(weapon_id.clone())),
                            ])),
                        ) {
                            Ok(events) => {
                                receive_events(events, &mut log, &mut dialog);
                                actions = read_actions(&game, attacker, false).unwrap_or_default();
                            }
                            Err(error) => log.push(format!("Error: {error}")),
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "android")]
        if snapshot.finished && dialog.is_none() && mission_result.is_none() {
            let victory = snapshot.result.as_deref() == Some("victory");
            mission_result = Some(MissionResult {
                victory,
                next: victory
                    .then(|| {
                        game.next_scenario()
                            .and_then(scenario_path)
                            .map(str::to_owned)
                    })
                    .flatten(),
            });
        }

        clear_background(Color::from_rgba(28, 31, 34, 255));
        #[cfg(not(target_os = "android"))]
        draw_map(
            &font,
            &snapshot,
            selected.as_deref(),
            target.as_deref(),
            &actions.reachable,
        );
        #[cfg(target_os = "android")]
        draw_map_android(
            &font,
            &art,
            &viewport,
            &snapshot,
            selected.as_deref(),
            target.as_deref(),
            &actions.reachable,
            settings.show_grid,
            move_preview.as_ref(),
            inspected_hex,
        );
        #[cfg(not(target_os = "android"))]
        draw_panel(
            &font,
            &game.name,
            &snapshot,
            selected.as_deref(),
            target.as_deref(),
            weapon,
            &actions.attacks,
            &log,
        );
        #[cfg(not(target_os = "android"))]
        text(
            &font,
            "F1-F3 demos | F4-F7 Two Brothers | Q/W recruit | U recall | E end turn | R restart",
            24.0,
            screen_height() - 25.0,
            22.0,
            GRAY,
        );
        #[cfg(target_os = "android")]
        draw_android_panel(
            &font,
            &art,
            &viewport,
            &game.name,
            &snapshot,
            selected.as_deref(),
            inspected_hex,
            panel_scroll,
        );
        #[cfg(target_os = "android")]
        draw_game_buttons(&font, &snapshot, selected.is_some());
        #[cfg(target_os = "android")]
        if let Some(combat) = &combat_dialog {
            draw_combat_dialog(&font, &art, &snapshot, combat, &actions.attacks);
        }
        #[cfg(target_os = "android")]
        if confirm_end_turn {
            draw_end_turn_confirmation(&font, &snapshot);
        }
        #[cfg(target_os = "android")]
        if let Some(menu) = recruit_menu {
            draw_recruit_menu(
                &font,
                &art,
                &snapshot.recruit_options,
                &snapshot.recall_options,
                menu,
            );
        }
        #[cfg(target_os = "android")]
        if settings.show_fps {
            text(
                &font,
                &format!("{} FPS", get_fps()),
                16.0,
                30.0,
                22.0,
                WHITE,
            );
        }
        if let Some(dialog) = &dialog {
            draw_dialog(&font, dialog);
        }
        #[cfg(target_os = "android")]
        if mission_briefing {
            draw_mission_briefing(&font, &game.name, &snapshot);
        }
        #[cfg(target_os = "android")]
        if let Some(result) = &mission_result {
            draw_mission_result(&font, &game.name, &snapshot, result);
        }
        next_frame().await;
    }
}

#[cfg(target_os = "android")]
fn mission_window_rect() -> Rect {
    let width = (screen_width() - 80.0).min(900.0);
    let height = (screen_height() - 60.0).min(500.0);
    Rect::new(
        (screen_width() - width) / 2.0,
        (screen_height() - height) / 2.0,
        width,
        height,
    )
}

#[cfg(target_os = "android")]
fn mission_start_rect() -> Rect {
    let window = mission_window_rect();
    Rect::new(
        window.x + window.w - 244.0,
        window.y + window.h - 70.0,
        210.0,
        48.0,
    )
}

#[cfg(target_os = "android")]
fn mission_menu_rect() -> Rect {
    let window = mission_window_rect();
    Rect::new(window.x + 28.0, window.y + window.h - 70.0, 220.0, 48.0)
}

#[cfg(target_os = "android")]
fn mission_replay_rect() -> Rect {
    let window = mission_window_rect();
    Rect::new(
        window.x + window.w / 2.0 - 110.0,
        window.y + window.h - 70.0,
        220.0,
        48.0,
    )
}

#[cfg(target_os = "android")]
fn mission_next_rect() -> Rect {
    let window = mission_window_rect();
    Rect::new(
        window.x + window.w - 248.0,
        window.y + window.h - 70.0,
        220.0,
        48.0,
    )
}

#[cfg(target_os = "android")]
fn draw_mission_window(window: Rect) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(0, 0, 0, 190),
    );
    draw_rectangle(
        window.x,
        window.y,
        window.w,
        window.h,
        Color::from_rgba(27, 28, 27, 255),
    );
    draw_rectangle_lines(
        window.x,
        window.y,
        window.w,
        window.h,
        3.0,
        Color::from_rgba(170, 127, 62, 255),
    );
}

#[cfg(target_os = "android")]
fn draw_mission_briefing(font: &Font, scenario_name: &str, snapshot: &ClientSnapshot) {
    let window = mission_window_rect();
    draw_mission_window(window);
    text(
        font,
        "Задание",
        window.x + 32.0,
        window.y + 52.0,
        25.0,
        GOLD,
    );
    text(
        font,
        scenario_name,
        window.x + 32.0,
        window.y + 96.0,
        22.0,
        WHITE,
    );
    text(font, "Цель", window.x + 32.0, window.y + 153.0, 17.0, GOLD);
    let objective = snapshot
        .objective
        .as_deref()
        .unwrap_or("Выполните условия сценария");
    for (index, line) in wrapped_lines(font, objective, window.w - 64.0, 17.0)
        .iter()
        .enumerate()
    {
        text(
            font,
            line,
            window.x + 32.0,
            window.y + 188.0 + index as f32 * 32.0,
            17.0,
            WHITE,
        );
    }
    let player_units = snapshot
        .objects
        .iter()
        .filter(|unit| unit.side == snapshot.active_side)
        .count();
    text(
        font,
        &format!(
            "Начальное золото: {}     Бойцов: {}",
            snapshot.gold, player_units
        ),
        window.x + 32.0,
        window.y + 300.0,
        16.0,
        LIGHTGRAY,
    );
    if snapshot.turn_limit > 0 {
        text(
            font,
            &format!("Лимит ходов: {}", snapshot.turn_limit),
            window.x + 32.0,
            window.y + 337.0,
            16.0,
            LIGHTGRAY,
        );
    }
    dialog_button(font, mission_start_rect(), "Начать", true);
}

#[cfg(target_os = "android")]
fn draw_mission_result(
    font: &Font,
    scenario_name: &str,
    snapshot: &ClientSnapshot,
    result: &MissionResult,
) {
    let window = mission_window_rect();
    draw_mission_window(window);
    text(
        font,
        if result.victory {
            "Победа"
        } else {
            "Поражение"
        },
        window.x + 32.0,
        window.y + 56.0,
        28.0,
        if result.victory { GOLD } else { RED },
    );
    text(
        font,
        scenario_name,
        window.x + 32.0,
        window.y + 98.0,
        19.0,
        WHITE,
    );
    let survivors = snapshot
        .objects
        .iter()
        .filter(|unit| unit.side == snapshot.active_side)
        .count();
    let villages = snapshot
        .villages
        .iter()
        .filter(|village| village.side.as_deref() == Some(snapshot.active_side.as_str()))
        .count();
    let carryover = snapshot.gold.max(0) * snapshot.carryover_percentage / 100;
    let rows = [
        format!("Затрачено ходов: {}", snapshot.turn),
        format!("Осталось бойцов: {}", survivors),
        format!("Захвачено деревень: {}", villages),
        format!("Осталось золота: {}", snapshot.gold),
        format!(
            "Перенос золота: {}%  →  {}",
            snapshot.carryover_percentage, carryover
        ),
    ];
    for (index, row) in rows.iter().enumerate() {
        text(
            font,
            row,
            window.x + 48.0,
            window.y + 155.0 + index as f32 * 43.0,
            17.0,
            if index == 4 { GOLD } else { LIGHTGRAY },
        );
    }
    dialog_button(font, mission_menu_rect(), "Главное меню", false);
    dialog_button(font, mission_replay_rect(), "Переиграть", false);
    if result.victory && result.next.is_some() {
        dialog_button(font, mission_next_rect(), "Следующая миссия", true);
    }
}

fn load_game(index: usize) -> Game {
    #[cfg(target_os = "android")]
    return Game::load_from(SCENARIOS[index], &wesnoth_engine::embedded::read)
        .expect("load embedded scenario");

    #[cfg(not(target_os = "android"))]
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
    #[cfg(not(target_os = "android"))]
    Game::load(scripts, SCENARIOS[index]).expect("load scenario")
}

fn load_game_with_campaign(
    scenario: &str,
    campaign: &wesnoth_engine::game::CampaignState,
) -> Result<Game, String> {
    #[cfg(target_os = "android")]
    return Game::load_with_campaign_from(
        scenario,
        Some(campaign),
        &wesnoth_engine::embedded::read,
    );

    #[cfg(not(target_os = "android"))]
    Game::load_with_campaign(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        scenario,
        Some(campaign),
    )
}

#[cfg(target_os = "android")]
fn reset_game_ui(
    selected: &mut Option<String>,
    target: &mut Option<String>,
    weapon: &mut usize,
    actions: &mut AvailableActions,
    log: &mut Vec<String>,
    dialog: &mut Option<DialogView>,
    game: &mut Game,
) {
    *selected = None;
    *target = None;
    *weapon = 0;
    *actions = AvailableActions::default();
    log.clear();
    *dialog = None;
    receive_events(game.start_events().unwrap_or_default(), log, dialog);
}

#[cfg(target_os = "android")]
#[derive(Clone, Copy)]
enum MenuAction {
    Continue,
    NewGame,
    OpenSettings,
    Scenario(usize),
    ToggleGrid,
    ToggleFps,
    Back,
}

#[cfg(target_os = "android")]
fn draw_android_menu(
    font: &Font,
    screen: AppScreen,
    settings: &ClientSettings,
    message: &str,
) -> Option<MenuAction> {
    clear_background(Color::from_rgba(22, 28, 35, 255));
    let width = screen_width().min(720.0);
    let left = (screen_width() - width) / 2.0;
    let pressed = is_mouse_button_pressed(MouseButton::Left);
    let point = Vec2::from(mouse_position());
    let mut result = None;
    let mut menu_button = |y: f32, label: &str, enabled: bool, action: MenuAction| {
        let rect = Rect::new(left + 35.0, y, width - 70.0, 70.0);
        let hovered = enabled && rect.contains(point);
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if !enabled {
                Color::from_rgba(47, 51, 56, 255)
            } else if hovered {
                Color::from_rgba(93, 119, 145, 255)
            } else {
                Color::from_rgba(52, 76, 99, 255)
            },
        );
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, GRAY);
        let size = 29.0;
        centered_text(font, label, rect, size, if enabled { WHITE } else { GRAY });
        if pressed && hovered {
            result = Some(action);
        }
    };

    text(
        font,
        "WESNOTH",
        left + 45.0,
        75.0,
        48.0,
        Color::from_rgba(224, 190, 95, 255),
    );
    match screen {
        AppScreen::MainMenu => {
            menu_button(
                115.0,
                "Продолжить",
                android_save_path().is_ok_and(|path| path.exists()),
                MenuAction::Continue,
            );
            menu_button(193.0, "Новая игра", true, MenuAction::NewGame);
            menu_button(271.0, "Настройки", true, MenuAction::OpenSettings);
            if !message.is_empty() {
                text(font, message, left + 45.0, 410.0, 21.0, LIGHTGRAY);
            }
        }
        AppScreen::Scenarios => {
            text(font, "Сценарии", left + 45.0, 108.0, 30.0, WHITE);
            for (index, name) in SCENARIO_NAMES.iter().enumerate() {
                menu_button(
                    125.0 + index as f32 * 72.0,
                    name,
                    true,
                    MenuAction::Scenario(index),
                );
            }
            menu_button(screen_height() - 82.0, "Назад", true, MenuAction::Back);
        }
        AppScreen::Settings => {
            text(font, "Настройки", left + 45.0, 108.0, 30.0, WHITE);
            menu_button(
                145.0,
                if settings.show_grid {
                    "Сетка: включена"
                } else {
                    "Сетка: выключена"
                },
                true,
                MenuAction::ToggleGrid,
            );
            menu_button(
                213.0,
                if settings.show_fps {
                    "Счётчик FPS: включён"
                } else {
                    "Счётчик FPS: выключен"
                },
                true,
                MenuAction::ToggleFps,
            );
            menu_button(315.0, "Назад", true, MenuAction::Back);
        }
        AppScreen::Game => {}
    }
    result
}

#[cfg(target_os = "android")]
fn android_save_path() -> Result<std::path::PathBuf, String> {
    use macroquad::miniquad::native::android::{ACTIVITY, attach_jni_env, ndk_sys};
    unsafe {
        let env = attach_jni_env();
        let get_class = (**env)
            .GetObjectClass
            .ok_or("JNI GetObjectClass недоступен")?;
        let get_method = (**env).GetMethodID.ok_or("JNI GetMethodID недоступен")?;
        let call = (**env)
            .CallObjectMethod
            .ok_or("JNI CallObjectMethod недоступен")?;
        let class = get_class(env, ACTIVITY);
        let method = get_method(
            env,
            class,
            c"getFilesDir".as_ptr(),
            c"()Ljava/io/File;".as_ptr(),
        );
        let directory = call(env, ACTIVITY, method);
        if directory.is_null() {
            return Err("Android не предоставил папку сохранений".into());
        }
        let file_class = get_class(env, directory);
        let absolute = get_method(
            env,
            file_class,
            c"getAbsolutePath".as_ptr(),
            c"()Ljava/lang/String;".as_ptr(),
        );
        let java_path = call(env, directory, absolute) as ndk_sys::jstring;
        let chars = (**env)
            .GetStringUTFChars
            .ok_or("JNI GetStringUTFChars недоступен")?(
            env,
            java_path,
            std::ptr::null_mut(),
        );
        let path = std::ffi::CStr::from_ptr(chars)
            .to_string_lossy()
            .into_owned();
        (**env)
            .ReleaseStringUTFChars
            .ok_or("JNI ReleaseStringUTFChars недоступен")?(env, java_path, chars);
        Ok(std::path::PathBuf::from(path).join("autosave.json"))
    }
}

#[cfg(target_os = "android")]
fn exclude_android_system_gestures() {
    use macroquad::miniquad::native::android::{ACTIVITY, attach_jni_env, ndk_sys};
    use macroquad::miniquad::{call_bool_method, call_object_method, call_void_method, new_object};
    unsafe {
        if ndk_sys::android_get_device_api_level() < 29 {
            return;
        }
        let env = attach_jni_env();
        let window = call_object_method!(env, ACTIVITY, "getWindow", "()Landroid/view/Window;");
        let view = call_object_method!(env, window, "getDecorView", "()Landroid/view/View;");
        let exclusions = new_object!(env, "java/util/ArrayList", "()V");
        let rect = new_object!(
            env,
            "android/graphics/Rect",
            "(IIII)V",
            0 as ndk_sys::jint,
            0 as ndk_sys::jint,
            screen_width() as ndk_sys::jint,
            screen_height() as ndk_sys::jint
        );
        call_bool_method!(env, exclusions, "add", "(Ljava/lang/Object;)Z", rect);
        call_void_method!(
            env,
            view,
            "setSystemGestureExclusionRects",
            "(Ljava/util/List;)V",
            exclusions
        );
    }
}

#[cfg(target_os = "android")]
fn save_android_game(game: &Game) -> Result<(), String> {
    let source = game.save()?;
    std::fs::write(android_save_path()?, source)
        .map_err(|error| format!("Не удалось сохранить игру: {error}"))
}

#[cfg(target_os = "android")]
fn load_android_save() -> Result<Game, String> {
    let source = std::fs::read_to_string(android_save_path()?)
        .map_err(|error| format!("Не удалось прочитать сохранение: {error}"))?;
    Game::load_save_from(&source, &wesnoth_engine::embedded::read)
        .map_err(|error| format!("Не удалось загрузить игру: {error}"))
}

fn text(font: &Font, value: &str, x: f32, y: f32, size: f32, color: Color) {
    let size = display_font_size(size);
    draw_text_ex(
        value,
        x,
        y,
        TextParams {
            font: Some(font),
            font_size: size as u16,
            color,
            ..Default::default()
        },
    );
}

fn display_font_size(size: f32) -> f32 {
    #[cfg(target_os = "android")]
    return size * 1.7;
    #[cfg(not(target_os = "android"))]
    return size;
}

fn measure_ui_text(value: &str, font: Option<&Font>, size: f32) -> TextDimensions {
    measure_text(value, font, display_font_size(size) as u16, 1.0)
}

fn centered_text(font: &Font, value: &str, rect: Rect, size: f32, color: Color) {
    let dimensions = measure_ui_text(value, Some(font), size);
    text(
        font,
        value,
        rect.x + (rect.w - dimensions.width) / 2.0,
        rect.y + (rect.h + dimensions.height) / 2.0,
        size,
        color,
    );
}

fn hex_center(x: i64, y: i64) -> Vec2 {
    let column = x as f32 - 1.0;
    let row = y as f32 - 1.0;
    vec2(
        MAP_ORIGIN.x + column * 54.0,
        MAP_ORIGIN.y + row * 72.0 + (x % 2 == 0) as u8 as f32 * 36.0,
    )
}

fn object_at(snapshot: &ClientSnapshot, point: Vec2) -> Option<&ObjectView> {
    let position = hex_at(snapshot, point)?;
    snapshot
        .objects
        .iter()
        .find(|object| object.position == position)
}

fn hex_at(snapshot: &ClientSnapshot, point: Vec2) -> Option<Position> {
    (1..=snapshot.map.height as i64)
        .flat_map(|y| (1..=snapshot.map.width as i64).map(move |x| Position { x, y }))
        .map(|position| (hex_center(position.x, position.y).distance(point), position))
        // Сенсорная цель немного шире видимого гекса: палец закрывает центр клетки.
        .filter(|(distance, _)| *distance < 54.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, position)| position)
}

#[cfg(target_os = "android")]
struct AndroidViewport {
    offset: Vec2,
    scale: f32,
    drag: Option<(Vec2, Vec2, bool, bool, f64, bool)>,
    pinch: Option<(f32, Vec2)>,
}

#[cfg(target_os = "android")]
#[derive(Default)]
struct ViewportGesture {
    tap: Option<Vec2>,
    long_press: Option<Vec2>,
}

#[cfg(target_os = "android")]
impl AndroidViewport {
    fn new() -> Self {
        Self {
            offset: Vec2::ZERO,
            scale: 2.2,
            drag: None,
            pinch: None,
        }
    }

    fn world_to_screen(&self, point: Vec2) -> Vec2 {
        viewport_world_to_screen(point, self.offset, self.scale)
    }

    fn screen_to_world(&self, point: Vec2) -> Vec2 {
        viewport_screen_to_world(point, self.offset, self.scale)
    }

    fn cancel_gesture(&mut self) {
        self.drag = None;
        self.pinch = None;
    }

    fn update(&mut self) -> ViewportGesture {
        let touches = touches();
        let active: Vec<_> = touches
            .iter()
            .filter(|touch| !matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled))
            .collect();
        if active.len() >= 2 {
            let a = active[0].position;
            let b = active[1].position;
            let midpoint = (a + b) / 2.0;
            let distance = a.distance(b).max(1.0);
            if let Some((previous_distance, previous_midpoint)) = self.pinch {
                let anchor = self.screen_to_world(previous_midpoint);
                self.scale = (self.scale * distance / previous_distance).clamp(1.7, 3.6);
                self.offset = midpoint - anchor * self.scale;
            }
            self.pinch = Some((distance, midpoint));
            self.drag = None;
            return ViewportGesture::default();
        }
        self.pinch = None;

        let point = Vec2::from(mouse_position());
        if is_mouse_button_pressed(MouseButton::Left) {
            let blocked = touch_blocked(point);
            self.drag = Some((point, point, false, blocked, get_time(), false));
            if is_mouse_button_released(MouseButton::Left) {
                self.drag = None;
                return ViewportGesture {
                    tap: (!blocked).then_some(point),
                    long_press: None,
                };
            }
            return ViewportGesture::default();
        }
        if is_mouse_button_down(MouseButton::Left) {
            let mut long_press = None;
            if let Some((start, last, moved, blocked, started, fired)) = self.drag.as_mut() {
                if !*blocked {
                    let was_moved = *moved;
                    *moved |= start.distance(point) > ANDROID_TAP_SLOP;
                    if *moved {
                        self.offset += if was_moved {
                            point - *last
                        } else {
                            point - *start
                        };
                    } else if !*fired && get_time() - *started >= 0.55 {
                        *fired = true;
                        long_press = Some(*start);
                    }
                }
                *last = point;
            }
            return ViewportGesture {
                tap: None,
                long_press,
            };
        }
        if let Some((_, _, moved, blocked, _, fired)) = self.drag.take() {
            return ViewportGesture {
                tap: (!moved && !blocked && !fired).then_some(point),
                long_press: None,
            };
        }
        ViewportGesture::default()
    }
}

#[cfg(any(target_os = "android", test))]
fn viewport_world_to_screen(point: Vec2, offset: Vec2, scale: f32) -> Vec2 {
    point * scale + offset
}

#[cfg(any(target_os = "android", test))]
fn viewport_screen_to_world(point: Vec2, offset: Vec2, scale: f32) -> Vec2 {
    (point - offset) / scale
}

#[cfg(target_os = "android")]
struct AndroidTouch {
    consumed: bool,
}

#[cfg(target_os = "android")]
impl AndroidTouch {
    fn read() -> Self {
        let pressed = is_mouse_button_pressed(MouseButton::Left);
        let point = Vec2::from(mouse_position());
        Self {
            consumed: pressed && menu_hotspot().contains(point),
        }
    }
}

#[cfg(target_os = "android")]
fn menu_hotspot() -> Rect {
    Rect::new(8.0, 8.0, 76.0, 68.0)
}

#[cfg(target_os = "android")]
fn touch_menu() -> bool {
    is_mouse_button_pressed(MouseButton::Left)
        && menu_hotspot().contains(Vec2::from(mouse_position()))
}

#[cfg(target_os = "android")]
fn end_turn_rect() -> Rect {
    Rect::new(
        screen_width() - ANDROID_PANEL_WIDTH + 12.0,
        screen_height() - 84.0,
        ANDROID_PANEL_WIDTH - 24.0,
        68.0,
    )
}

#[cfg(target_os = "android")]
fn cancel_selection_rect() -> Rect {
    let end = end_turn_rect();
    Rect::new(end.x, end.y - 78.0, end.w, 66.0)
}

#[cfg(target_os = "android")]
fn touch_blocked(point: Vec2) -> bool {
    menu_hotspot().contains(point)
        || end_turn_rect().contains(point)
        || cancel_selection_rect().contains(point)
        || point.y <= ANDROID_TOP_BAR_HEIGHT
        || point.x >= screen_width() - ANDROID_PANEL_WIDTH
}

#[cfg(target_os = "android")]
fn draw_map_android(
    font: &Font,
    art: &AndroidArt,
    viewport: &AndroidViewport,
    snapshot: &ClientSnapshot,
    selected: Option<&str>,
    target: Option<&str>,
    reachable: &[ReachableCell],
    show_grid: bool,
    move_preview: Option<&MovePreview>,
    inspected_hex: Option<Position>,
) {
    // Wesnoth terrain is composited in passes: ground first, decorations after it.
    // Drawing both per cell lets the next ground tile erase forests and villages.
    for y in 1..=snapshot.map.height as i64 {
        for x in 1..=snapshot.map.width as i64 {
            let destination = Position { x, y };
            let center = viewport.world_to_screen(hex_center(x, y));
            let radius = HEX_RADIUS * viewport.scale;
            let terrain = snapshot.map.get(destination).unwrap_or("grassland");
            let tint = terrain_tint(snapshot, destination, reachable, selected.is_some());
            draw_wesnoth_hex(center, radius, Color::from_rgba(45, 65, 40, 255));
            let (ground, _) = terrain_layers(terrain);
            if let Some(texture) = art.terrain.get(ground) {
                draw_texture_ex(
                    texture,
                    center.x - radius,
                    center.y - radius,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                        ..Default::default()
                    },
                );
            }
        }
    }
    for y in 1..=snapshot.map.height as i64 {
        for x in 1..=snapshot.map.width as i64 {
            let destination = Position { x, y };
            let center = viewport.world_to_screen(hex_center(x, y));
            let radius = HEX_RADIUS * viewport.scale;
            let terrain = snapshot.map.get(destination).unwrap_or("grassland");
            let tint = terrain_tint(snapshot, destination, reachable, selected.is_some());
            if terrain != "water" {
                for (direction, neighbor) in hex_neighbors(destination).into_iter().enumerate() {
                    if snapshot.map.get(neighbor) == Ok("water") {
                        draw_texture_ex(
                            &art.water_edges[direction],
                            center.x - radius,
                            center.y - radius,
                            tint,
                            DrawTextureParams {
                                dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            if let Some(overlay) = terrain_layers(terrain).1
                && let Some(texture) = art.terrain.get(overlay)
            {
                draw_texture_ex(
                    texture,
                    center.x - radius,
                    center.y - radius,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                        ..Default::default()
                    },
                );
            }
            if show_grid {
                draw_wesnoth_hex_lines(center, radius, 1.5, Color::from_rgba(20, 28, 20, 180));
            }
        }
    }
    for village in &snapshot.villages {
        if let Some(side) = village.side.as_deref() {
            let center =
                viewport.world_to_screen(hex_center(village.position.x, village.position.y));
            let color = if side == "player" { SKYBLUE } else { ORANGE };
            let scale = viewport.scale;
            draw_circle(
                center.x + 21.0 * scale,
                center.y - 20.0 * scale,
                7.0 * scale,
                color,
            );
            draw_circle_lines(
                center.x + 21.0 * scale,
                center.y - 20.0 * scale,
                7.0 * scale,
                2.0,
                WHITE,
            );
        }
    }
    if let Some(preview) = move_preview {
        let mut previous = selected
            .and_then(|id| snapshot.objects.iter().find(|unit| unit.id == id))
            .map(|unit| viewport.world_to_screen(hex_center(unit.position.x, unit.position.y)));
        for position in &preview.path {
            let center = viewport.world_to_screen(hex_center(position.x, position.y));
            if let Some(from) = previous {
                draw_line(
                    from.x,
                    from.y,
                    center.x,
                    center.y,
                    7.0,
                    Color::from_rgba(250, 220, 70, 220),
                );
            }
            draw_circle(center.x, center.y, 6.0, YELLOW);
            previous = Some(center);
        }
        let center =
            viewport.world_to_screen(hex_center(preview.destination.x, preview.destination.y));
        let label = format!(
            "−{} ОД   защита {}%{}",
            preview.cost,
            preview.defense,
            if preview.stopped_by_zoc {
                "   ЗК"
            } else {
                ""
            }
        );
        let width = measure_ui_text(&label, None, 18.0).width + 18.0;
        draw_rectangle(
            center.x - width / 2.0,
            center.y - 58.0,
            width,
            27.0,
            Color::from_rgba(18, 20, 22, 225),
        );
        text(
            font,
            &label,
            center.x - width / 2.0 + 9.0,
            center.y - 39.0,
            18.0,
            WHITE,
        );
    }
    if let Some(position) = inspected_hex {
        let center = viewport.world_to_screen(hex_center(position.x, position.y));
        let radius = HEX_RADIUS * viewport.scale * 0.9;
        draw_wesnoth_hex_lines(center, radius, 4.0, Color::from_rgba(255, 220, 70, 235));
    }
    for object in &snapshot.objects {
        let center = viewport.world_to_screen(hex_center(object.position.x, object.position.y));
        let scale = viewport.scale;
        if target == Some(object.id.as_str()) {
            draw_wesnoth_hex_lines(center, HEX_RADIUS * scale * 0.9, 4.0, RED);
        }
        if let Some(texture) = art.units.get(object.type_id.as_str()) {
            draw_texture_ex(
                texture,
                center.x - 36.0 * scale,
                center.y - 42.0 * scale,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(72.0, 72.0) * scale),
                    ..Default::default()
                },
            );
        }
        draw_unit_status(center, scale, object);
    }
}

#[cfg(target_os = "android")]
fn draw_unit_status(center: Vec2, scale: f32, unit: &ObjectView) {
    let hp = (unit.hitpoints.max(0) as f32 / unit.max_hitpoints.max(1) as f32).clamp(0.0, 1.0);
    let experience =
        (unit.experience.max(0) as f32 / unit.max_experience.max(1) as f32).clamp(0.0, 1.0);
    let bar_top = center.y - 23.0 * scale;
    let bar_height = 34.0 * scale;
    let bar_width = 4.0 * scale;
    let mut bars = vec![(
        0.0,
        hp,
        if hp > 0.66 {
            LIME
        } else if hp > 0.33 {
            YELLOW
        } else {
            RED
        },
    )];
    bars.push((5.0, experience, SKYBLUE));
    for (index, ratio, color) in bars {
        let x = center.x + (17.0 + index) * scale;
        draw_rectangle(
            x,
            bar_top,
            bar_width,
            bar_height,
            Color::from_rgba(20, 20, 20, 180),
        );
        draw_rectangle_lines(x, bar_top, bar_width, bar_height, scale.max(1.0), LIGHTGRAY);
        draw_rectangle(
            x + scale,
            bar_top + (bar_height - 2.0 * scale) * (1.0 - ratio) + scale,
            (bar_width - 2.0 * scale).max(1.0),
            (bar_height - 2.0 * scale) * ratio,
            color,
        );
    }
    let orb = center + vec2(19.0 * scale, -29.0 * scale);
    let color = if unit.movement_points == unit.max_movement_points && unit.attacks_left > 0 {
        LIME
    } else if unit.movement_points > 0 || unit.attacks_left > 0 {
        ORANGE
    } else {
        RED
    };
    draw_circle(orb.x, orb.y, 5.0 * scale, Color::from_rgba(20, 20, 20, 230));
    draw_circle(orb.x, orb.y, 4.0 * scale, color);
    draw_circle(orb.x - 1.3 * scale, orb.y - 1.4 * scale, 1.3 * scale, WHITE);
}

#[cfg(any(target_os = "android", test))]
fn wesnoth_hex_vertices(center: Vec2, radius: f32) -> [Vec2; 6] {
    [
        center + vec2(-radius, 0.0),
        center + vec2(-radius * 0.5, -radius),
        center + vec2(radius * 0.5, -radius),
        center + vec2(radius, 0.0),
        center + vec2(radius * 0.5, radius),
        center + vec2(-radius * 0.5, radius),
    ]
}

#[cfg(target_os = "android")]
fn draw_wesnoth_hex(center: Vec2, radius: f32, color: Color) {
    let vertices = wesnoth_hex_vertices(center, radius);
    for index in 0..6 {
        draw_triangle(center, vertices[index], vertices[(index + 1) % 6], color);
    }
}

#[cfg(target_os = "android")]
fn draw_wesnoth_hex_lines(center: Vec2, radius: f32, thickness: f32, color: Color) {
    let vertices = wesnoth_hex_vertices(center, radius);
    for index in 0..6 {
        let from = vertices[index];
        let to = vertices[(index + 1) % 6];
        draw_line(from.x, from.y, to.x, to.y, thickness, color);
    }
}

#[cfg(any(target_os = "android", test))]
fn terrain_layers(terrain: &str) -> (&str, Option<&str>) {
    match terrain {
        "grassland" => ("grassland", None),
        "water" => ("water", None),
        other => ("grassland", Some(other)),
    }
}

#[cfg(any(target_os = "android", test))]
fn hex_neighbors(position: Position) -> [Position; 6] {
    let diagonal_up = if position.x % 2 == 0 { 0 } else { -1 };
    [
        Position {
            x: position.x,
            y: position.y - 1,
        },
        Position {
            x: position.x + 1,
            y: position.y + diagonal_up,
        },
        Position {
            x: position.x + 1,
            y: position.y + diagonal_up + 1,
        },
        Position {
            x: position.x,
            y: position.y + 1,
        },
        Position {
            x: position.x - 1,
            y: position.y + diagonal_up + 1,
        },
        Position {
            x: position.x - 1,
            y: position.y + diagonal_up,
        },
    ]
}

#[cfg(target_os = "android")]
fn terrain_tint(
    snapshot: &ClientSnapshot,
    position: Position,
    reachable: &[ReachableCell],
    has_selection: bool,
) -> Color {
    if snapshot.shroud && !snapshot.revealed_cells.contains(&position) {
        BLACK
    } else if snapshot.fog && !snapshot.visible_cells.contains(&position) {
        Color::new(0.28, 0.28, 0.28, 1.0)
    } else if reachable.iter().any(|cell| cell.position == position) {
        Color::new(1.0, 1.0, 0.62, 1.0)
    } else if has_selection {
        Color::new(0.22, 0.22, 0.22, 1.0)
    } else {
        WHITE
    }
}

#[cfg(not(target_os = "android"))]
fn draw_map(
    font: &Font,
    snapshot: &ClientSnapshot,
    selected: Option<&str>,
    target: Option<&str>,
    reachable: &[ReachableCell],
) {
    for y in 1..=snapshot.map.height as i64 {
        for x in 1..=snapshot.map.width as i64 {
            let center = hex_center(x, y);
            let destination = Position { x, y };
            let terrain_color = match snapshot.map.get(destination).unwrap_or("unknown") {
                "forest" => Color::from_rgba(37, 92, 48, 255),
                "hills" => Color::from_rgba(132, 105, 66, 255),
                "water" => Color::from_rgba(48, 101, 150, 255),
                "castle" => Color::from_rgba(145, 145, 150, 255),
                "keep" => Color::from_rgba(185, 170, 105, 255),
                "village" => Color::from_rgba(170, 135, 82, 255),
                _ => Color::from_rgba(77, 113, 66, 255),
            };
            let visible = !snapshot.fog || snapshot.visible_cells.contains(&destination);
            let revealed = !snapshot.shroud || snapshot.revealed_cells.contains(&destination);
            let fill = if !revealed {
                Color::from_rgba(7, 9, 11, 255)
            } else if !visible {
                Color::new(
                    terrain_color.r * 0.32,
                    terrain_color.g * 0.32,
                    terrain_color.b * 0.32,
                    1.0,
                )
            } else if reachable.iter().any(|cell| cell.position == destination) {
                Color::new(
                    terrain_color.r * 0.72 + YELLOW.r * 0.28,
                    terrain_color.g * 0.72 + YELLOW.g * 0.28,
                    terrain_color.b * 0.72 + YELLOW.b * 0.28,
                    1.0,
                )
            } else {
                terrain_color
            };
            draw_poly(center.x, center.y, 6, HEX_RADIUS, 0.0, fill);
            draw_poly_lines(
                center.x,
                center.y,
                6,
                HEX_RADIUS,
                0.0,
                2.0,
                Color::from_rgba(33, 50, 30, 255),
            );
        }
    }
    for village in &snapshot.villages {
        let center = hex_center(village.position.x, village.position.y);
        let color = match village.side.as_deref() {
            Some("player") => SKYBLUE,
            Some("enemy") => ORANGE,
            _ => LIGHTGRAY,
        };
        draw_rectangle(center.x - 13.0, center.y - 10.0, 26.0, 22.0, color);
        draw_triangle(
            vec2(center.x - 17.0, center.y - 10.0),
            vec2(center.x + 17.0, center.y - 10.0),
            vec2(center.x, center.y - 25.0),
            color,
        );
    }
    for object in &snapshot.objects {
        let center = hex_center(object.position.x, object.position.y);
        let color = if object.side == "player" {
            SKYBLUE
        } else {
            ORANGE
        };
        if selected == Some(object.id.as_str()) || target == Some(object.id.as_str()) {
            draw_circle(center.x, center.y, 35.0, YELLOW);
        }
        draw_circle(center.x, center.y, 29.0, color);
        text(
            font,
            &object.id[..1].to_uppercase(),
            center.x - 9.0,
            center.y + 9.0,
            30.0,
            BLACK,
        );
        let ratio = object.hitpoints as f32 / object.max_hitpoints as f32;
        draw_rectangle(center.x - 30.0, center.y + 34.0, 60.0, 7.0, DARKGRAY);
        draw_rectangle(center.x - 30.0, center.y + 34.0, 60.0 * ratio, 7.0, LIME);
    }
}

#[cfg(target_os = "android")]
fn draw_android_panel(
    font: &Font,
    art: &AndroidArt,
    viewport: &AndroidViewport,
    scenario_name: &str,
    snapshot: &ClientSnapshot,
    selected: Option<&str>,
    inspected_hex: Option<Position>,
    scroll: f32,
) {
    let x = screen_width() - ANDROID_PANEL_WIDTH;
    let border = Color::from_rgba(116, 91, 54, 255);
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        ANDROID_TOP_BAR_HEIGHT,
        Color::from_rgba(25, 24, 22, 245),
    );
    draw_line(
        0.0,
        ANDROID_TOP_BAR_HEIGHT,
        screen_width(),
        ANDROID_TOP_BAR_HEIGHT,
        3.0,
        border,
    );
    draw_texture_ex(
        &art.sidebar,
        x,
        ANDROID_TOP_BAR_HEIGHT,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(
                ANDROID_PANEL_WIDTH,
                screen_height() - ANDROID_TOP_BAR_HEIGHT,
            )),
            ..Default::default()
        },
    );
    draw_line(x, ANDROID_TOP_BAR_HEIGHT, x, screen_height(), 3.0, border);

    let villages = snapshot
        .villages
        .iter()
        .filter(|village| village.side.as_deref() == Some(snapshot.active_side.as_str()))
        .count();
    let units = snapshot
        .objects
        .iter()
        .filter(|unit| unit.side == snapshot.active_side)
        .count();
    for (index, value) in [
        snapshot.gold.to_string(),
        villages.to_string(),
        units.to_string(),
        snapshot.expenses.to_string(),
        format!("{:+}", snapshot.net_income),
    ]
    .into_iter()
    .enumerate()
    {
        let left = 105.0 + index as f32 * 138.0;
        draw_texture_ex(
            &art.status_icons[index],
            left,
            22.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(if index < 3 { 36.0 } else { 48.0 }, 36.0)),
                ..Default::default()
            },
        );
        text(font, &value, left + 52.0, 55.0, 19.0, WHITE);
    }

    draw_rectangle(8.0, 8.0, 76.0, 68.0, Color::from_rgba(16, 18, 20, 230));
    draw_rectangle_lines(8.0, 8.0, 76.0, 68.0, 2.0, border);
    for y in [26.0, 42.0, 58.0] {
        draw_line(26.0, y, 66.0, y, 4.0, WHITE);
    }

    let top = ANDROID_TOP_BAR_HEIGHT;
    text(font, scenario_name, x + 14.0, top + 29.0, 17.0, GOLD);
    text(
        font,
        &format!("Ход {}   {}", snapshot.turn, snapshot.active_side),
        x + 14.0,
        top + 59.0,
        16.0,
        WHITE,
    );
    draw_line(
        x + 10.0,
        top + 68.0,
        screen_width() - 10.0,
        top + 68.0,
        1.0,
        border,
    );
    let content_top = top + 76.0;
    let content_bottom = cancel_selection_rect().y - 6.0;
    unsafe {
        get_internal_gl().quad_gl.scissor(Some((
            x as i32,
            content_top as i32,
            ANDROID_PANEL_WIDTH as i32,
            (content_bottom - content_top) as i32,
        )));
    }
    let mut y = content_top - scroll;
    let minimap = Rect::new(x + 14.0, y, ANDROID_PANEL_WIDTH - 28.0, 155.0);
    draw_minimap(viewport, snapshot, minimap);
    y += 165.0;

    draw_line(x + 10.0, y, screen_width() - 10.0, y, 1.0, border);
    if let Some(icon) = art.time_icons.get(snapshot.time_of_day.as_str()) {
        draw_texture_ex(
            icon,
            x + 14.0,
            y + 7.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(145.0, 45.0)),
                ..Default::default()
            },
        );
    }
    text(
        font,
        time_name(&snapshot.time_of_day),
        x + 174.0,
        y + 27.0,
        13.0,
        WHITE,
    );
    text(
        font,
        &format!("Законные {:+}%", snapshot.lawful_bonus),
        x + 174.0,
        y + 50.0,
        11.0,
        GRAY,
    );
    y += 62.0;

    draw_line(x + 10.0, y, screen_width() - 10.0, y, 1.0, border);
    if let Some(position) = inspected_hex {
        let terrain = snapshot.map.get(position).unwrap_or("unknown");
        text(
            font,
            &format!("Покрытие: {}", terrain_name(terrain)),
            x + 12.0,
            y + 25.0,
            12.0,
            GOLD,
        );
        text(
            font,
            &format!("Гекс {}, {}", position.x, position.y),
            x + 12.0,
            y + 49.0,
            11.0,
            LIGHTGRAY,
        );
        if let Some(unit) =
            selected.and_then(|id| snapshot.objects.iter().find(|unit| unit.id == id))
        {
            let cost = unit
                .movement_costs
                .get(terrain)
                .map(i64::to_string)
                .unwrap_or_else(|| "—".into());
            let defense = unit
                .defense
                .get(terrain)
                .map(|value| format!("{value}%"))
                .unwrap_or_else(|| "—".into());
            text(
                font,
                &format!("Ход: {cost}   Защита: {defense}"),
                x + 151.0,
                y + 49.0,
                11.0,
                WHITE,
            );
        }
    } else {
        text(font, "Коснитесь гекса", x + 12.0, y + 34.0, 12.0, GRAY);
    }
    y += 64.0;

    if let Some(unit) = selected.and_then(|id| snapshot.objects.iter().find(|unit| unit.id == id)) {
        draw_line(x + 10.0, y, screen_width() - 10.0, y, 1.0, border);
        if let Some(texture) = art.units.get(unit.type_id.as_str()) {
            draw_texture_ex(
                texture,
                x + 12.0,
                y + 8.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(82.0, 82.0)),
                    ..Default::default()
                },
            );
        }
        text(
            font,
            unit_name(&unit.type_id),
            x + 108.0,
            y + 28.0,
            15.0,
            WHITE,
        );
        text(
            font,
            &format!("уровень {}", unit.level),
            x + 108.0,
            y + 54.0,
            12.0,
            GRAY,
        );
        text(
            font,
            &format!(
                "ОЗ {}/{}   ОД {}/{}",
                unit.hitpoints, unit.max_hitpoints, unit.movement_points, unit.max_movement_points
            ),
            x + 108.0,
            y + 79.0,
            11.0,
            LIME,
        );
        text(
            font,
            &format!("Опыт {}/{}", unit.experience, unit.max_experience),
            x + 108.0,
            y + 101.0,
            11.0,
            SKYBLUE,
        );
        y += 114.0;

        draw_line(x + 10.0, y, screen_width() - 10.0, y, 1.0, border);
        text(
            font,
            &format!("Атаки · осталось {}", unit.attacks_left),
            x + 12.0,
            y + 28.0,
            14.0,
            GOLD,
        );
        y += 39.0;
        if unit.attacks.is_empty() {
            text(font, "Нет доступных атак", x + 18.0, y + 25.0, 11.0, GRAY);
        }
        for attack in &unit.attacks {
            draw_rectangle(
                x + 12.0,
                y + 3.0,
                ANDROID_PANEL_WIDTH - 32.0,
                42.0,
                Color::from_rgba(28, 30, 29, 190),
            );
            draw_rectangle_lines(
                x + 12.0,
                y + 3.0,
                ANDROID_PANEL_WIDTH - 32.0,
                42.0,
                1.0,
                border,
            );
            text(font, attack, x + 20.0, y + 29.0, 10.0, WHITE);
            y += 48.0;
        }
    }
    unsafe {
        get_internal_gl().quad_gl.scissor(None);
    }

    let max_scroll = android_panel_max_scroll(snapshot, selected);
    if max_scroll > 0.0 {
        let track_h = content_bottom - content_top;
        let content_h = track_h + max_scroll;
        let thumb_h = (track_h * track_h / content_h).max(34.0);
        let thumb_y = content_top + scroll / max_scroll * (track_h - thumb_h);
        draw_rectangle(
            screen_width() - 7.0,
            content_top,
            5.0,
            track_h,
            Color::from_rgba(20, 20, 20, 180),
        );
        draw_rectangle(screen_width() - 7.0, thumb_y, 5.0, thumb_h, GOLD);
    }
}

#[cfg(target_os = "android")]
fn android_panel_max_scroll(snapshot: &ClientSnapshot, selected: Option<&str>) -> f32 {
    let viewport_height = cancel_selection_rect().y - 6.0 - (ANDROID_TOP_BAR_HEIGHT + 76.0);
    let unit_height = selected
        .and_then(|id| snapshot.objects.iter().find(|unit| unit.id == id))
        .map_or(0.0, |unit| 153.0 + unit.attacks.len() as f32 * 48.0);
    (291.0 + unit_height - viewport_height).max(0.0)
}

#[cfg(target_os = "android")]
fn terrain_name(terrain: &str) -> &str {
    match terrain {
        "grassland" => "Равнина",
        "forest" => "Лес",
        "hills" => "Холмы",
        "water" => "Вода",
        "castle" => "Замок",
        "keep" => "Цитадель",
        "village" => "Деревня",
        other => other,
    }
}

#[cfg(target_os = "android")]
fn draw_minimap(viewport: &AndroidViewport, snapshot: &ClientSnapshot, rect: Rect) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::from_rgba(8, 10, 9, 230),
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        3.0,
        Color::from_rgba(116, 91, 54, 255),
    );
    let world_w = (snapshot.map.width.saturating_sub(1) as f32 * 54.0 + 72.0).max(72.0);
    let world_h = (snapshot.map.height.saturating_sub(1) as f32 * 72.0 + 108.0).max(72.0);
    let scale = ((rect.w - 12.0) / world_w).min((rect.h - 12.0) / world_h);
    let used = vec2(world_w, world_h) * scale;
    let origin = vec2(
        rect.x + (rect.w - used.x) / 2.0,
        rect.y + (rect.h - used.y) / 2.0,
    );
    let to_minimap = |world: Vec2| origin + (world - MAP_ORIGIN + vec2(36.0, 36.0)) * scale;
    for y in 1..=snapshot.map.height as i64 {
        for x in 1..=snapshot.map.width as i64 {
            let position = Position { x, y };
            let center = to_minimap(hex_center(x, y));
            draw_wesnoth_hex(
                center,
                36.0 * scale + 0.5,
                minimap_terrain_color(snapshot.map.get(position).unwrap_or("grassland")),
            );
        }
    }
    for village in &snapshot.villages {
        let center = to_minimap(hex_center(village.position.x, village.position.y));
        draw_circle(center.x, center.y, (4.0 * scale).max(2.0), GOLD);
    }
    for unit in &snapshot.objects {
        let center = to_minimap(hex_center(unit.position.x, unit.position.y));
        let color = if unit.side == snapshot.active_side {
            SKYBLUE
        } else {
            RED
        };
        draw_circle(center.x, center.y, (6.0 * scale).max(3.0), color);
    }
    let world_a = viewport.screen_to_world(vec2(0.0, ANDROID_TOP_BAR_HEIGHT));
    let world_b =
        viewport.screen_to_world(vec2(screen_width() - ANDROID_PANEL_WIDTH, screen_height()));
    let a = to_minimap(world_a);
    let b = to_minimap(world_b);
    let left = a.x.min(b.x).clamp(rect.x, rect.x + rect.w);
    let top = a.y.min(b.y).clamp(rect.y, rect.y + rect.h);
    let right = a.x.max(b.x).clamp(rect.x, rect.x + rect.w);
    let bottom = a.y.max(b.y).clamp(rect.y, rect.y + rect.h);
    draw_rectangle_lines(left, top, right - left, bottom - top, 2.0, WHITE);
}

#[cfg(target_os = "android")]
fn minimap_terrain_color(terrain: &str) -> Color {
    match terrain {
        "forest" => Color::from_rgba(25, 92, 39, 255),
        "hills" => Color::from_rgba(112, 91, 57, 255),
        "water" => Color::from_rgba(35, 91, 148, 255),
        "castle" | "keep" => Color::from_rgba(145, 145, 150, 255),
        "village" => Color::from_rgba(170, 135, 82, 255),
        _ => Color::from_rgba(55, 112, 48, 255),
    }
}

#[cfg(target_os = "android")]
fn time_name(id: &str) -> &str {
    match id {
        "dawn" => "Рассвет",
        "morning" => "Утро",
        "afternoon" => "День",
        "dusk" => "Закат",
        "first_watch" => "Ночь",
        "second_watch" => "Ночь",
        other => other,
    }
}

#[cfg(target_os = "android")]
fn draw_game_buttons(font: &Font, snapshot: &ClientSnapshot, has_selection: bool) {
    let cancel = cancel_selection_rect();
    dialog_button(font, cancel, "Снять выбор", false);
    if !has_selection {
        draw_rectangle(
            cancel.x,
            cancel.y,
            cancel.w,
            cancel.h,
            Color::from_rgba(20, 20, 20, 150),
        );
    }
    dialog_button(font, end_turn_rect(), "Конец хода", snapshot.can_end_turn);
}

#[cfg(target_os = "android")]
fn recruit_window_rect() -> Rect {
    let width = screen_width() - ANDROID_PANEL_WIDTH - 32.0;
    let height = (screen_height() - ANDROID_TOP_BAR_HEIGHT - 28.0).min(420.0);
    Rect::new(16.0, screen_height() - height - 14.0, width, height)
}

#[cfg(target_os = "android")]
fn recruit_tab_rect(veterans: bool) -> Rect {
    let window = recruit_window_rect();
    Rect::new(
        window.x + 20.0 + veterans as u8 as f32 * 245.0,
        window.y + 14.0,
        230.0,
        52.0,
    )
}

#[cfg(target_os = "android")]
fn recruit_type_rect(index: usize) -> Rect {
    let window = recruit_window_rect();
    Rect::new(
        window.x + 22.0,
        window.y + 78.0 + index as f32 * 68.0,
        310.0,
        60.0,
    )
}

#[cfg(target_os = "android")]
fn recruit_cancel_rect() -> Rect {
    let window = recruit_window_rect();
    Rect::new(window.x + 24.0, window.y + window.h - 70.0, 190.0, 54.0)
}

#[cfg(target_os = "android")]
fn recruit_confirm_rect() -> Rect {
    let window = recruit_window_rect();
    Rect::new(
        window.x + window.w - 234.0,
        window.y + window.h - 70.0,
        210.0,
        54.0,
    )
}

#[cfg(target_os = "android")]
fn draw_recruit_menu(
    font: &Font,
    art: &AndroidArt,
    recruits: &[RecruitOption],
    veterans: &[RecruitOption],
    menu: RecruitMenu,
) {
    draw_rectangle(
        0.0,
        ANDROID_TOP_BAR_HEIGHT,
        screen_width() - ANDROID_PANEL_WIDTH,
        screen_height() - ANDROID_TOP_BAR_HEIGHT,
        Color::from_rgba(0, 0, 0, 85),
    );
    let window = recruit_window_rect();
    draw_rectangle(
        window.x,
        window.y,
        window.w,
        window.h,
        Color::from_rgba(28, 31, 34, 255),
    );
    draw_rectangle_lines(window.x, window.y, window.w, window.h, 3.0, GOLD);
    dialog_button(font, recruit_tab_rect(false), "Новобранцы", !menu.veterans);
    dialog_button(font, recruit_tab_rect(true), "Ветераны", menu.veterans);
    let options = if menu.veterans { veterans } else { recruits };
    for (index, option) in options.iter().take(4).enumerate() {
        let rect = recruit_type_rect(index);
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if index == menu.selected {
                Color::from_rgba(93, 119, 145, 255)
            } else {
                Color::from_rgba(45, 49, 53, 255)
            },
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            2.0,
            if index == menu.selected { GOLD } else { GRAY },
        );
        if let Some(sprite) = art.units.get(option.type_id.as_str()) {
            draw_texture_ex(
                sprite,
                rect.x + 4.0,
                rect.y + 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(56.0, 56.0)),
                    ..Default::default()
                },
            );
        }
        text(
            font,
            &option.name,
            rect.x + 68.0,
            rect.y + 25.0,
            14.0,
            WHITE,
        );
        text(
            font,
            &format!("{} зол.", option.cost),
            rect.x + 68.0,
            rect.y + 51.0,
            13.0,
            GOLD,
        );
    }
    if let Some(option) = options.get(menu.selected) {
        let x = window.x + 345.0;
        if let Some(sprite) = art.units.get(option.type_id.as_str()) {
            draw_texture_ex(
                sprite,
                x,
                window.y + 84.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(104.0, 104.0)),
                    ..Default::default()
                },
            );
        }
        text(font, &option.name, x + 120.0, window.y + 112.0, 17.0, WHITE);
        text(
            font,
            &format!("Уровень {}   Цена {}", option.level, option.cost),
            x + 120.0,
            window.y + 147.0,
            14.0,
            GOLD,
        );
        text(
            font,
            &format!(
                "Здоровье {}/{}   Ходов {}",
                option.hitpoints, option.max_hitpoints, option.max_moves
            ),
            x,
            window.y + 198.0,
            14.0,
            WHITE,
        );
        text(
            font,
            &format!("Мировоззрение: {}", alignment_name(&option.alignment)),
            x,
            window.y + 226.0,
            13.0,
            LIGHTGRAY,
        );
        if menu.veterans {
            text(
                font,
                &format!("Опыт {}/{}", option.experience, option.max_experience),
                x,
                window.y + 254.0,
                13.0,
                SKYBLUE,
            );
        }
        text(font, "Атаки", x, window.y + 280.0, 14.0, GOLD);
        for (index, attack) in option.attacks.iter().take(2).enumerate() {
            text(
                font,
                attack,
                x + 12.0,
                window.y + 306.0 + index as f32 * 26.0,
                13.0,
                WHITE,
            );
        }
    }
    dialog_button(font, recruit_cancel_rect(), "Отмена", false);
    dialog_button(
        font,
        recruit_confirm_rect(),
        if menu.veterans {
            "Призвать"
        } else {
            "Нанять"
        },
        !options.is_empty(),
    );
}

#[cfg(target_os = "android")]
fn alignment_name(id: &str) -> &str {
    match id {
        "lawful" => "порядочный",
        "chaotic" => "хаотичный",
        "neutral" => "нейтральный",
        other => other,
    }
}

#[cfg(target_os = "android")]
fn end_turn_confirmation_rect() -> Rect {
    let width = (screen_width() - 80.0).min(760.0);
    Rect::new(
        (screen_width() - width) / 2.0,
        (screen_height() - 300.0) / 2.0,
        width,
        300.0,
    )
}

#[cfg(target_os = "android")]
fn end_turn_cancel_rect() -> Rect {
    let window = end_turn_confirmation_rect();
    Rect::new(window.x + 28.0, window.y + window.h - 82.0, 220.0, 58.0)
}

#[cfg(target_os = "android")]
fn end_turn_confirm_rect() -> Rect {
    let window = end_turn_confirmation_rect();
    Rect::new(
        window.x + window.w - 248.0,
        window.y + window.h - 82.0,
        220.0,
        58.0,
    )
}

#[cfg(target_os = "android")]
fn draw_end_turn_confirmation(font: &Font, snapshot: &ClientSnapshot) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(0, 0, 0, 160),
    );
    let window = end_turn_confirmation_rect();
    draw_rectangle(
        window.x,
        window.y,
        window.w,
        window.h,
        Color::from_rgba(28, 31, 34, 255),
    );
    draw_rectangle_lines(window.x, window.y, window.w, window.h, 3.0, GOLD);
    let remaining = snapshot
        .objects
        .iter()
        .filter(|unit| unit.side == snapshot.active_side && unit.movement_points > 0)
        .count();
    text(
        font,
        "Закончить ход?",
        window.x + 28.0,
        window.y + 58.0,
        27.0,
        GOLD,
    );
    text(
        font,
        &format!("У {} юнитов остались очки движения.", remaining),
        window.x + 28.0,
        window.y + 125.0,
        20.0,
        WHITE,
    );
    dialog_button(font, end_turn_cancel_rect(), "Отмена", false);
    dialog_button(font, end_turn_confirm_rect(), "Закончить", true);
}

#[cfg(target_os = "android")]
fn combat_window_rect() -> Rect {
    let width = (screen_width() - 80.0).min(860.0);
    let height = (screen_height() - 50.0).min(560.0);
    Rect::new(
        (screen_width() - width) / 2.0,
        (screen_height() - height) / 2.0,
        width,
        height,
    )
}

#[cfg(target_os = "android")]
fn combat_weapon_rect(index: usize) -> Rect {
    let window = combat_window_rect();
    Rect::new(
        window.x + 18.0,
        window.y + 218.0 + index as f32 * 70.0,
        window.w - 36.0,
        62.0,
    )
}

#[cfg(target_os = "android")]
fn combat_cancel_rect() -> Rect {
    let window = combat_window_rect();
    Rect::new(
        window.x + window.w - 202.0,
        window.y + window.h - 55.0,
        176.0,
        40.0,
    )
}

#[cfg(target_os = "android")]
fn combat_confirm_rect() -> Rect {
    let window = combat_window_rect();
    Rect::new(
        window.x + window.w - 394.0,
        window.y + window.h - 55.0,
        176.0,
        40.0,
    )
}

#[cfg(target_os = "android")]
fn draw_combat_dialog(
    font: &Font,
    art: &AndroidArt,
    snapshot: &ClientSnapshot,
    combat: &CombatDialog,
    attacks: &[String],
) {
    let Some(attacker) = snapshot
        .objects
        .iter()
        .find(|unit| unit.id == combat.attacker)
    else {
        return;
    };
    let Some(defender) = snapshot
        .objects
        .iter()
        .find(|unit| unit.id == combat.defender)
    else {
        return;
    };
    let window = combat_window_rect();
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(0, 0, 0, 165),
    );
    draw_rectangle(
        window.x,
        window.y,
        window.w,
        window.h,
        Color::from_rgba(27, 28, 27, 255),
    );
    draw_rectangle_lines(
        window.x,
        window.y,
        window.w,
        window.h,
        3.0,
        Color::from_rgba(170, 127, 62, 255),
    );
    let unit_card = |unit: &ObjectView, sprite_x: f32, text_x: f32| {
        if let Some(texture) = art.units.get(unit.type_id.as_str()) {
            draw_texture_ex(
                texture,
                sprite_x,
                window.y + 64.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(104.0, 104.0)),
                    ..Default::default()
                },
            );
        }
        text(
            font,
            unit_name(&unit.type_id),
            text_x,
            window.y + 83.0,
            15.0,
            WHITE,
        );
        text(
            font,
            &format!(
                "ур. {}   ОЗ {}/{}",
                unit.level, unit.hitpoints, unit.max_hitpoints
            ),
            text_x,
            window.y + 111.0,
            13.0,
            LIME,
        );
        text(
            font,
            &format!(
                "ОД {}/{}   ОП {}/{}",
                unit.movement_points,
                unit.max_movement_points,
                unit.experience,
                unit.max_experience
            ),
            text_x,
            window.y + 139.0,
            11.0,
            LIGHTGRAY,
        );
    };
    unit_card(attacker, window.x + 28.0, window.x + 142.0);
    unit_card(
        defender,
        window.x + window.w - 132.0,
        window.x + window.w / 2.0 + 28.0,
    );
    text(font, "Атака", window.x + 20.0, window.y + 40.0, 22.0, GOLD);
    draw_line(
        window.x + window.w / 2.0,
        window.y + 58.0,
        window.x + window.w / 2.0,
        window.y + 194.0,
        2.0,
        GRAY,
    );
    text(
        font,
        "АТАКУЮЩИЙ",
        window.x + 142.0,
        window.y + 181.0,
        10.0,
        GOLD,
    );
    text(
        font,
        "ЗАЩИЩАЮЩИЙСЯ",
        window.x + window.w / 2.0 + 28.0,
        window.y + 181.0,
        10.0,
        GOLD,
    );

    for (index, attack) in attacks.iter().take(3).enumerate() {
        let Some(forecast) = combat.forecasts.get(index) else {
            continue;
        };
        let rect = combat_weapon_rect(index);
        let active = index == combat.weapon;
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if active {
                Color::from_rgba(112, 80, 35, 255)
            } else {
                Color::from_rgba(48, 51, 52, 255)
            },
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            2.0,
            if active { GOLD } else { GRAY },
        );
        let split = rect.x + rect.w * 0.54;
        draw_line(split, rect.y + 5.0, split, rect.y + rect.h - 5.0, 1.0, GRAY);
        let description = attacker
            .attacks
            .get(index)
            .map(String::as_str)
            .unwrap_or(attack);
        text(font, description, rect.x + 12.0, rect.y + 23.0, 11.0, WHITE);
        text(
            font,
            &format!(
                "{}×{} · попадание {:.0}% · ожид. −{:.1} ОЗ",
                forecast.damage, forecast.strikes, forecast.chance, forecast.expected_damage
            ),
            rect.x + 12.0,
            rect.y + 49.0,
            10.0,
            LIME,
        );
        text(
            font,
            &format!("Ответ: {}", forecast.retaliation_name),
            split + 12.0,
            rect.y + 23.0,
            11.0,
            WHITE,
        );
        text(
            font,
            &format!(
                "{}×{} · {:.0}% · ожид. −{:.1} ОЗ",
                forecast.retaliation_damage,
                forecast.retaliation_strikes,
                forecast.retaliation_chance,
                forecast.expected_retaliation
            ),
            split + 12.0,
            rect.y + 49.0,
            10.0,
            ORANGE,
        );
        text(
            font,
            &format!(
                "победа {:.0}% / гибель {:.0}%",
                forecast.kill_probability * 100.0,
                forecast.death_probability * 100.0
            ),
            rect.x + rect.w - 218.0,
            rect.y + 23.0,
            9.0,
            GOLD,
        );
    }
    dialog_button(font, combat_cancel_rect(), "Отмена", false);
    dialog_button(font, combat_confirm_rect(), "Атаковать", true);
}

#[cfg(target_os = "android")]
fn dialog_button(font: &Font, rect: Rect, label: &str, primary: bool) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if primary {
            Color::from_rgba(120, 78, 30, 255)
        } else {
            Color::from_rgba(55, 58, 60, 255)
        },
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        2.0,
        if primary { GOLD } else { GRAY },
    );
    centered_text(font, label, rect, 20.0, WHITE);
}

#[cfg(target_os = "android")]
fn unit_name(type_id: &str) -> &str {
    match type_id {
        "spearman" => "Копейщик",
        "bowman" => "Лучник",
        "knight" => "Рыцарь",
        "elvish_fighter" => "Эльфийский воин",
        "elvish_archer" => "Эльфийский лучник",
        "elvish_hero" => "Эльфийский герой",
        "elvish_marksman" => "Эльфийский стрелок",
        "elvish_ranger" => "Эльфийский следопыт",
        "orcish_grunt" => "Орк-пехотинец",
        "orcish_warrior" => "Орк-воин",
        "orcish_raider" => "Орк-налётчик",
        "red_mage" => "Красный маг",
        "silver_mage" => "Серебряный маг",
        "dark_sorcerer" => "Тёмный колдун",
        "walking_corpse" => "Ходячий труп",
        "cockatrice" => "Василиск",
        other => other,
    }
}

#[cfg(not(target_os = "android"))]
fn draw_panel(
    font: &Font,
    scenario_name: &str,
    snapshot: &ClientSnapshot,
    selected: Option<&str>,
    target: Option<&str>,
    weapon: usize,
    attacks: &[String],
    log: &[String],
) {
    draw_rectangle(
        PANEL_X,
        0.0,
        screen_width() - PANEL_X,
        screen_height(),
        Color::from_rgba(20, 22, 24, 255),
    );
    text(font, scenario_name, PANEL_X + 20.0, 34.0, 25.0, WHITE);
    text(
        font,
        &format!(
            "Turn {}  Side {}  Gold {}{}",
            snapshot.turn,
            snapshot.active_side,
            snapshot.gold,
            if snapshot.finished { "  FINISHED" } else { "" }
        ),
        PANEL_X + 20.0,
        65.0,
        20.0,
        GRAY,
    );
    text(
        font,
        &format!(
            "Time {}  lawful {:+}%",
            snapshot.time_of_day, snapshot.lawful_bonus
        ),
        PANEL_X + 20.0,
        88.0,
        18.0,
        LIGHTGRAY,
    );
    if let Some(objective) = &snapshot.objective {
        text(font, objective, PANEL_X + 20.0, 112.0, 17.0, GOLD);
    }
    let owned_villages = snapshot
        .villages
        .iter()
        .filter(|village| village.side.as_deref() == Some(snapshot.active_side.as_str()))
        .count() as i64;
    text(
        font,
        &format!(
            "Villages {} x{}  Income {:+} (gross {}, upkeep {})",
            owned_villages,
            snapshot.village_income,
            snapshot.net_income,
            snapshot.gross_income,
            snapshot.expenses,
        ),
        PANEL_X + 20.0,
        136.0,
        18.0,
        LIGHTGRAY,
    );
    let mut y = 164.0;
    if let Some(pending) = &snapshot.pending_advancement {
        let choices = pending
            .options
            .iter()
            .zip(["Z", "X", "C", "V"])
            .map(|(option, key)| format!("[{key}] {option}"))
            .collect::<Vec<_>>()
            .join("  ");
        text(
            font,
            &format!("Advance {}: {}", pending.unit, choices),
            PANEL_X + 20.0,
            y,
            18.0,
            YELLOW,
        );
        y += 34.0;
    }
    for object in &snapshot.objects {
        text(
            font,
            &format!(
                "{} L{} HP {}/{} MP {}/{} AP {} XP {}/{}{}{}{}{}{}",
                object.id,
                object.level,
                object.hitpoints,
                object.max_hitpoints,
                object.movement_points,
                object.max_movement_points,
                object.attacks_left,
                object.experience,
                object.max_experience,
                if object.poisoned { " P" } else { "" },
                if object.slowed { " S" } else { "" },
                if object.petrified { " T" } else { "" },
                if object.unhealable { " U" } else { "" },
                if object.stunned { " X" } else { "" },
            ),
            PANEL_X + 20.0,
            y,
            18.0,
            if object.side == "player" {
                SKYBLUE
            } else {
                ORANGE
            },
        );
        y += 30.0;
    }
    y += 20.0;
    text(
        font,
        &format!("Selected: {}", selected.unwrap_or("-")),
        PANEL_X + 20.0,
        y,
        22.0,
        WHITE,
    );
    y += 30.0;
    text(
        font,
        &format!("Target:   {}", target.unwrap_or("-")),
        PANEL_X + 20.0,
        y,
        22.0,
        WHITE,
    );
    y += 42.0;
    if let Some(unit) =
        selected.and_then(|id| snapshot.objects.iter().find(|object| object.id == id))
    {
        text(font, "MOVEMENT", PANEL_X + 20.0, y, 22.0, GRAY);
        y += 27.0;
        let terrains: BTreeSet<_> = snapshot.map.cells.iter().collect();
        for terrain in terrains {
            let cost = unit
                .movement_costs
                .get(terrain)
                .map(i64::to_string)
                .unwrap_or_else(|| "blocked".into());
            text(
                font,
                &format!("{}: {}", terrain, cost),
                PANEL_X + 30.0,
                y,
                18.0,
                LIGHTGRAY,
            );
            y += 22.0;
        }
        y += 12.0;
        text(font, "WEAPON", PANEL_X + 20.0, y, 22.0, GRAY);
        y += 28.0;
        for (index, attack) in attacks.iter().enumerate() {
            text(
                font,
                &format!("{}  {}", index + 1, attack),
                PANEL_X + 30.0,
                y,
                23.0,
                if index == weapon { YELLOW } else { WHITE },
            );
            y += 28.0;
        }
    }
    let mut log_y = screen_height() - 152.0;
    draw_rectangle(
        PANEL_X,
        log_y - 22.0,
        screen_width() - PANEL_X,
        126.0,
        Color::from_rgba(20, 22, 24, 255),
    );
    text(font, "LOG", PANEL_X + 20.0, log_y, 22.0, GRAY);
    for line in log.iter().rev().take(4).rev() {
        log_y += 24.0;
        text(font, line, PANEL_X + 20.0, log_y, 18.0, LIGHTGRAY);
    }
}

fn receive_events(events: Vec<Value>, log: &mut Vec<String>, dialog: &mut Option<DialogView>) {
    for event in events {
        match event.get("type").and_then(Value::as_str) {
            Some("dialog_requested") => {
                let id = string(&event, "dialog").unwrap_or_default();
                let lines = list(&event, "lines")
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|line| {
                        Some(DialogLine {
                            speaker: string(line, "speaker")?,
                            text: string(line, "text")?,
                        })
                    })
                    .collect();
                log.push(format!("Dialog: {id}"));
                *dialog = Some(DialogView { lines, current: 0 });
            }
            Some("battle_resolved") => {
                for strike in list(&event, "strikes").unwrap_or(&[]) {
                    let source = string(strike, "source").unwrap_or_default();
                    let target = string(strike, "target").unwrap_or_default();
                    if matches!(strike.get("hit"), Some(Value::Bool(true))) {
                        log.push(format!(
                            "{} > {} [{} res {}%, ToD {:+}%, lead +{}%]: -{} HP",
                            source,
                            target,
                            string(strike, "damage_type").unwrap_or_default(),
                            integer(strike, "effective_resistance").unwrap_or(0),
                            integer(strike, "alignment_modifier").unwrap_or(0),
                            integer(strike, "leadership_bonus").unwrap_or(0),
                            integer(strike, "damage").unwrap_or(0)
                        ));
                        if matches!(strike.get("poisoned"), Some(Value::Bool(true))) {
                            log.push(format!("{target}: poisoned"));
                        }
                        if matches!(strike.get("slowed"), Some(Value::Bool(true))) {
                            log.push(format!("{target}: slowed"));
                        }
                        if matches!(strike.get("petrified"), Some(Value::Bool(true))) {
                            log.push(format!("{target}: petrified"));
                        }
                        if matches!(strike.get("stunned"), Some(Value::Bool(true))) {
                            log.push(format!("{target}: stunned"));
                        }
                        if integer(strike, "drained").unwrap_or(0) > 0 {
                            log.push(format!(
                                "{source}: drained +{} HP",
                                integer(strike, "drained").unwrap_or(0)
                            ));
                        }
                    } else {
                        log.push(format!("{source} > {target}: miss"));
                    }
                }
                for gained in list(&event, "experience").unwrap_or(&[]) {
                    log.push(format!(
                        "{}: +{} XP ({}/{})",
                        string(gained, "unit").unwrap_or_default(),
                        integer(gained, "gained").unwrap_or(0),
                        integer(gained, "total").unwrap_or(0),
                        integer(gained, "maximum").unwrap_or(0)
                    ));
                }
            }
            Some("object_moved") => {
                let from = position(&event, "from").unwrap_or(Position { x: 0, y: 0 });
                let to = position(&event, "to").unwrap_or(Position { x: 0, y: 0 });
                log.push(format!(
                    "{}: ({}, {}) > ({}, {}), -{} MP, {} left{}",
                    string(&event, "object").unwrap_or_default(),
                    from.x,
                    from.y,
                    to.x,
                    to.y,
                    integer(&event, "cost").unwrap_or(0),
                    integer(&event, "movement_points").unwrap_or(0),
                    if matches!(event.get("stopped_by_zoc"), Some(Value::Bool(true))) {
                        " (ZoC)"
                    } else {
                        ""
                    }
                ));
            }
            Some("unit_died") => log.push(format!(
                "{} died",
                string(&event, "unit").unwrap_or_default()
            )),
            Some("phase_changed") => log.push(format!(
                "Objective: {}",
                string(&event, "phase").unwrap_or_default()
            )),
            Some("unit_recruited") => log.push(format!(
                "Recruited {} for {}, gold {}",
                string(&event, "unit_type").unwrap_or_default(),
                integer(&event, "cost").unwrap_or(0),
                integer(&event, "gold").unwrap_or(0)
            )),
            Some("unit_recalled") => log.push(format!(
                "Recalled {} for {}, gold {}",
                string(&event, "unit").unwrap_or_default(),
                integer(&event, "cost").unwrap_or(0),
                integer(&event, "gold").unwrap_or(0)
            )),
            Some("village_captured") => log.push(format!(
                "{} captured a village",
                string(&event, "side").unwrap_or_default()
            )),
            Some("economy_updated") => log.push(format!(
                "{} economy: {:+} gold (base {}, villages {}, upkeep {}), total {}",
                string(&event, "side").unwrap_or_default(),
                integer(&event, "net_income").unwrap_or(0),
                integer(&event, "base_income").unwrap_or(0),
                integer(&event, "gross_income").unwrap_or(0)
                    - integer(&event, "base_income").unwrap_or(0),
                integer(&event, "expenses").unwrap_or(0),
                integer(&event, "gold").unwrap_or(0),
            )),
            Some("unit_healed") => log.push(format!(
                "{} healed +{} HP ({})",
                string(&event, "unit").unwrap_or_default(),
                integer(&event, "amount").unwrap_or(0),
                string(&event, "source").unwrap_or_else(|| "unknown".into())
            )),
            Some("poison_damage") => log.push(format!(
                "{}: poison -{} HP",
                string(&event, "unit").unwrap_or_default(),
                integer(&event, "damage").unwrap_or(0)
            )),
            Some("status_cured") | Some("status_expired") => log.push(format!(
                "{}: {} cleared",
                string(&event, "unit").unwrap_or_default(),
                string(&event, "status").unwrap_or_default()
            )),
            Some("unit_advanced") => log.push(format!(
                "{} advanced: {} > {} (level {})",
                string(&event, "unit").unwrap_or_default(),
                string(&event, "from").unwrap_or_default(),
                string(&event, "to").unwrap_or_default(),
                integer(&event, "level").unwrap_or(0)
            )),
            Some("advancement_required") => log.push(format!(
                "{}: choose advancement",
                string(&event, "unit").unwrap_or_default()
            )),
            Some("scenario_finished") => {
                let result = string(&event, "result").unwrap_or_default();
                log.push(format!("Finished: {result}"));
            }
            Some("turn_started") => log.push(format!(
                "Turn {}: {}",
                integer(&event, "turn").unwrap_or(0),
                string(&event, "side").unwrap_or_default()
            )),
            Some("turn_ended") => {}
            _ => log.push("Unknown event".into()),
        }
    }
}

fn read_snapshot(snapshot: GameSnapshot, status: Value) -> Result<ClientSnapshot, String> {
    let Value::List(objects) = snapshot.objects else {
        return Err("snapshot objects must be a list".into());
    };
    let visible_units: BTreeSet<_> = value_list(&status, "visible_units")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let objects = objects
        .iter()
        .filter(|object| string(object, "id").is_some_and(|id| visible_units.contains(id.as_str())))
        .map(|object| {
            let id = string(object, "id").ok_or("snapshot object has no id")?;
            let costs = map(object, "movement_costs")?
                .iter()
                .map(|(terrain, cost)| {
                    cost.as_i64()
                        .map(|cost| (terrain.clone(), cost))
                        .ok_or_else(|| "invalid movement cost".to_owned())
                })
                .collect::<Result<_, _>>()?;
            #[cfg(target_os = "android")]
            let defense = map(object, "defense")?
                .iter()
                .map(|(terrain, value)| {
                    value
                        .as_i64()
                        .map(|value| (terrain.clone(), value))
                        .ok_or_else(|| "invalid defense value".to_owned())
                })
                .collect::<Result<_, _>>()?;
            #[cfg(any(target_os = "android", test))]
            let attacks = value_list(object, "attacks")?
                .iter()
                .map(attack_description)
                .collect();
            Ok(ObjectView {
                id,
                #[cfg(target_os = "android")]
                type_id: string(object, "type").unwrap_or_default(),
                side: string(object, "side").ok_or("snapshot object has no side")?,
                position: position(object, "position").ok_or("snapshot object has no position")?,
                hitpoints: integer(object, "hitpoints").ok_or("snapshot object has no HP")?,
                max_hitpoints: integer(object, "max_hitpoints")
                    .ok_or("snapshot object has no max HP")?,
                movement_points: integer(object, "movement_points")
                    .ok_or("snapshot object has no MP")?,
                max_movement_points: integer(object, "max_movement_points")
                    .ok_or("snapshot object has no max MP")?,
                attacks_left: integer(object, "attacks_left")
                    .ok_or("snapshot object has no attacks left")?,
                level: integer(object, "level").ok_or("snapshot object has no level")?,
                experience: integer(object, "experience")
                    .ok_or("snapshot object has no experience")?,
                max_experience: integer(object, "max_experience")
                    .ok_or("snapshot object has no max experience")?,
                poisoned: matches!(object.get("poisoned"), Some(Value::Bool(true))),
                slowed: matches!(object.get("slowed"), Some(Value::Bool(true))),
                petrified: matches!(object.get("petrified"), Some(Value::Bool(true))),
                unhealable: matches!(object.get("unhealable"), Some(Value::Bool(true))),
                stunned: matches!(object.get("stunned"), Some(Value::Bool(true))),
                movement_costs: costs,
                #[cfg(target_os = "android")]
                defense,
                #[cfg(any(target_os = "android", test))]
                attacks,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(ClientSnapshot {
        map: snapshot.map,
        objects,
        turn: integer(&status, "turn").ok_or("status has no turn")?,
        active_side: string(&status, "active_side").ok_or("status has no active side")?,
        finished: matches!(status.get("finished"), Some(Value::Bool(true))),
        can_end_turn: matches!(status.get("can_end_turn"), Some(Value::Bool(true))),
        objective: string(&status, "objective"),
        #[cfg(target_os = "android")]
        result: string(&status, "result"),
        #[cfg(target_os = "android")]
        turn_limit: integer(&status, "turn_limit").unwrap_or(0),
        #[cfg(target_os = "android")]
        carryover_percentage: integer(&status, "carryover_percentage").unwrap_or(0),
        gold: integer(&status, "gold").unwrap_or(0),
        recruit_types: value_list(&status, "recruit_types")?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        #[cfg(target_os = "android")]
        recruit_options: value_list(&status, "recruit_options")?
            .iter()
            .map(|option| {
                let id = string(option, "id").ok_or("recruit option has no id")?;
                Ok(RecruitOption {
                    type_id: string(option, "type").unwrap_or_else(|| id.clone()),
                    id,
                    name: string(option, "name").ok_or("recruit option has no name")?,
                    cost: integer(option, "cost").ok_or("recruit option has no cost")?,
                    max_hitpoints: integer(option, "max_hitpoints")
                        .ok_or("recruit option has no HP")?,
                    max_moves: integer(option, "max_moves").ok_or("recruit option has no moves")?,
                    level: integer(option, "level").unwrap_or(0),
                    alignment: string(option, "alignment").unwrap_or_default(),
                    hitpoints: integer(option, "hitpoints")
                        .or_else(|| integer(option, "max_hitpoints"))
                        .unwrap_or(0),
                    experience: integer(option, "experience").unwrap_or(0),
                    max_experience: integer(option, "max_experience").unwrap_or(0),
                    attacks: value_list(option, "attacks")?
                        .iter()
                        .map(attack_description)
                        .collect(),
                })
            })
            .collect::<Result<_, String>>()?,
        #[cfg(target_os = "android")]
        recall_options: value_list(&status, "recall_options")?
            .iter()
            .map(read_recruit_option)
            .collect::<Result<_, String>>()?,
        recall_units: value_list(&status, "recall_units")?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        villages: value_list(&status, "villages")?
            .iter()
            .map(|village| {
                Ok(VillageView {
                    position: Position {
                        x: integer(village, "x").ok_or("village has no x")?,
                        y: integer(village, "y").ok_or("village has no y")?,
                    },
                    side: string(village, "side"),
                })
            })
            .collect::<Result<_, String>>()?,
        village_income: integer(&status, "income").unwrap_or(0),
        gross_income: integer(&status, "gross_income").unwrap_or(0),
        expenses: integer(&status, "expenses").unwrap_or(0),
        net_income: integer(&status, "net_income").unwrap_or(0),
        time_of_day: string(&status, "time_of_day").ok_or("status has no time of day")?,
        lawful_bonus: integer(&status, "lawful_bonus").unwrap_or(0),
        pending_advancement: match status.get("pending_advancement") {
            Some(Value::Map(pending)) => Some(AdvancementView {
                unit: pending
                    .get("unit")
                    .and_then(Value::as_str)
                    .ok_or("pending advancement has no unit")?
                    .to_owned(),
                options: match pending.get("options") {
                    Some(Value::List(options)) => options
                        .iter()
                        .map(|option| {
                            option
                                .as_str()
                                .map(str::to_owned)
                                .ok_or_else(|| "advancement option is not a string".to_owned())
                        })
                        .collect::<Result<_, _>>()?,
                    _ => return Err("pending advancement has no options".into()),
                },
            }),
            Some(Value::Nil) | None => None,
            _ => return Err("pending advancement is not a map".into()),
        },
        fog: matches!(status.get("fog"), Some(Value::Bool(true))),
        visible_cells: value_list(&status, "visible_cells")?
            .iter()
            .map(|cell| {
                Ok(Position {
                    x: integer(cell, "x").ok_or("visible cell has no x")?,
                    y: integer(cell, "y").ok_or("visible cell has no y")?,
                })
            })
            .collect::<Result<_, String>>()?,
        shroud: matches!(status.get("shroud"), Some(Value::Bool(true))),
        revealed_cells: value_list(&status, "revealed_cells")?
            .iter()
            .map(|cell| {
                Ok(Position {
                    x: integer(cell, "x").ok_or("revealed cell has no x")?,
                    y: integer(cell, "y").ok_or("revealed cell has no y")?,
                })
            })
            .collect::<Result<_, String>>()?,
    })
}

#[cfg(target_os = "android")]
fn read_recruit_option(option: &Value) -> Result<RecruitOption, String> {
    let id = string(option, "id").ok_or("recall option has no id")?;
    Ok(RecruitOption {
        type_id: string(option, "type").unwrap_or_else(|| id.clone()),
        id,
        name: string(option, "name").ok_or("recall option has no name")?,
        cost: integer(option, "cost").unwrap_or(20),
        max_hitpoints: integer(option, "max_hitpoints").ok_or("recall option has no HP")?,
        max_moves: integer(option, "max_moves").ok_or("recall option has no moves")?,
        level: integer(option, "level").unwrap_or(0),
        alignment: string(option, "alignment").unwrap_or_default(),
        hitpoints: integer(option, "hitpoints").unwrap_or(0),
        experience: integer(option, "experience").unwrap_or(0),
        max_experience: integer(option, "max_experience").unwrap_or(0),
        attacks: value_list(option, "attacks")?
            .iter()
            .map(attack_description)
            .collect(),
    })
}

#[cfg(any(target_os = "android", test))]
fn child_values<'a>(value: &'a Value, name: &str) -> &'a [Value] {
    value
        .get("__children")
        .and_then(Value::as_map)
        .and_then(|children| children.get(name))
        .and_then(|values| match values {
            Value::List(values) => Some(values.as_slice()),
            _ => None,
        })
        .unwrap_or(&[])
}

#[cfg(any(target_os = "android", test))]
fn attack_description(attack: &Value) -> String {
    let specials = list(attack, "specials")
        .unwrap_or(child_values(attack, "special"))
        .iter()
        .filter_map(|special| special.as_str().or_else(|| special.get("id")?.as_str()))
        .map(special_name)
        .collect::<Vec<_>>()
        .join(", ");
    let base = format!(
        "{}  {}×{}  {}/{}",
        string(attack, "name")
            .or_else(|| string(attack, "id"))
            .unwrap_or_default(),
        integer(attack, "damage").unwrap_or(0),
        integer(attack, "strikes").unwrap_or(0),
        match string(attack, "range").as_deref() {
            Some("melee") => "ближ",
            Some("ranged") => "дальн",
            _ => "?",
        },
        damage_type_name(string(attack, "damage_type").as_deref().unwrap_or("")),
    );
    if specials.is_empty() {
        base
    } else {
        format!("{base} · {specials}")
    }
}

#[cfg(any(target_os = "android", test))]
fn damage_type_name(value: &str) -> &str {
    match value {
        "blade" => "руб",
        "pierce" => "кол",
        "impact" => "дроб",
        "fire" => "огонь",
        "cold" => "холод",
        "arcane" => "мист",
        other => other,
    }
}

#[cfg(any(target_os = "android", test))]
fn special_name(value: &str) -> &str {
    match value {
        "magical" => "магия",
        "marksman" => "снайпер",
        "first_strike" => "первый удар",
        "drain" => "высасывание",
        "slow" => "замедление",
        "poison" => "яд",
        "berserk" => "берсерк",
        other => other,
    }
}

fn read_actions(game: &Game, object: &str, inspect: bool) -> Result<AvailableActions, String> {
    let mut command = object_command(object, None);
    if inspect && let Value::Map(values) = &mut command {
        values.insert("inspect".into(), Value::Bool(true));
    }
    let value = game.query("actions", command)?;
    let reachable = value_list(&value, "reachable")?
        .iter()
        .map(|cell| {
            Ok(ReachableCell {
                position: position(cell, "position")
                    .ok_or_else(|| "reachable cell has no position".to_owned())?,
                path: value_list(cell, "path")?
                    .iter()
                    .map(|point| {
                        position_from_value(point)
                            .ok_or_else(|| "reachable path has an invalid position".to_owned())
                    })
                    .collect::<Result<_, _>>()?,
                cost: integer(cell, "cost").ok_or("reachable cell has no cost")?,
                stopped_by_zoc: matches!(cell.get("zoc"), Some(Value::Bool(true))),
            })
        })
        .collect::<Result<_, String>>()?;
    let targets = value_list(&value, "targets")?
        .iter()
        .map(|target| {
            target
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "invalid target id".to_owned())
        })
        .collect::<Result<_, _>>()?;
    let attacks = value_list(&value, "attacks")?
        .iter()
        .map(|attack| {
            attack
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "invalid attack id".to_owned())
        })
        .collect::<Result<_, _>>()?;
    Ok(AvailableActions {
        reachable,
        targets,
        attacks,
    })
}

#[cfg(target_os = "android")]
fn read_combat_dialog(
    game: &Game,
    attacker: &str,
    defender: &str,
    attacks: &[String],
) -> Result<CombatDialog, String> {
    let forecasts = attacks
        .iter()
        .map(|weapon| {
            let value = game.query("preview_attack", attack_command(attacker, defender, weapon))?;
            Ok(CombatForecast {
                chance: number(&value, "chance")?,
                damage: number(&value, "damage")?,
                strikes: number(&value, "strikes")?,
                expected_damage: number(&value, "expected_damage")?,
                kill_probability: number(&value, "kill_probability")?,
                retaliation_chance: number(&value, "retaliation_chance")?,
                retaliation_damage: number(&value, "retaliation_damage")?,
                retaliation_strikes: number(&value, "retaliation_strikes")?,
                expected_retaliation: number(&value, "expected_retaliation")?,
                death_probability: number(&value, "death_probability")?,
                retaliation_name: string(&value, "retaliation_name").unwrap_or_else(|| "—".into()),
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(CombatDialog {
        attacker: attacker.into(),
        defender: defender.into(),
        weapon: 0,
        forecasts,
    })
}

#[cfg(target_os = "android")]
fn number(value: &Value, key: &str) -> Result<f64, String> {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_i64()
                .map(|number| number as f64)
                .or_else(|| value.as_str()?.parse().ok())
        })
        .ok_or_else(|| format!("combat preview has no {key}"))
}

fn attack_command(attacker: &str, defender: &str, weapon: &str) -> Value {
    Value::Map(std::collections::BTreeMap::from([
        ("attacker".into(), Value::String(attacker.into())),
        ("defender".into(), Value::String(defender.into())),
        ("weapon".into(), Value::String(weapon.into())),
    ]))
}

fn position_from_value(value: &Value) -> Option<Position> {
    Some(Position {
        x: integer(value, "x")?,
        y: integer(value, "y")?,
    })
}

fn value_list<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    match value.get(key) {
        Some(Value::List(values)) => Ok(values),
        Some(Value::Map(values)) if values.is_empty() => Ok(&[]),
        _ => Err(format!("missing list: {key}")),
    }
}

fn object_command(object: &str, destination: Option<Position>) -> Value {
    let mut values =
        std::collections::BTreeMap::from([("object".into(), Value::String(object.into()))]);
    if let Some(position) = destination {
        values.insert(
            "destination".into(),
            Value::Map(std::collections::BTreeMap::from([
                ("x".into(), Value::Integer(position.x)),
                ("y".into(), Value::Integer(position.y)),
            ])),
        );
    }
    Value::Map(values)
}

#[cfg(target_os = "android")]
fn position_value(position: Position) -> Value {
    Value::Map(std::collections::BTreeMap::from([
        ("x".into(), Value::Integer(position.x)),
        ("y".into(), Value::Integer(position.y)),
    ]))
}

fn scenario_path(id: &str) -> Option<&'static str> {
    match id {
        "02_the_chase" => Some("scenarios/the_chase.wml"),
        "03_guarded_castle" => Some("scenarios/guarded_castle.wml"),
        "04_return_to_the_village" => Some("scenarios/return_to_the_village.wml"),
        _ => None,
    }
}

fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key)?.as_i64()
}

fn list<'a>(value: &'a Value, key: &str) -> Option<&'a [Value]> {
    match value.get(key)? {
        Value::List(values) => Some(values),
        _ => None,
    }
}

fn map<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a std::collections::BTreeMap<String, Value>, String> {
    value
        .get(key)
        .and_then(Value::as_map)
        .ok_or_else(|| format!("missing map: {key}"))
}

fn position(value: &Value, key: &str) -> Option<Position> {
    let values = value.get(key)?.as_map()?;
    Some(Position {
        x: values.get("x")?.as_i64()?,
        y: values.get("y")?.as_i64()?,
    })
}

fn draw_dialog(font: &Font, dialog: &DialogView) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(0, 0, 0, 150),
    );
    let box_x = 75.0;
    let box_width = screen_width() - 150.0;
    let line = &dialog.lines[dialog.current];
    let body_size = 25.0;
    let lines = wrapped_lines(font, &line.text, box_width - 48.0, body_size);
    let line_height = display_font_size(body_size) * 1.22;
    let box_height =
        (145.0 + lines.len() as f32 * line_height).clamp(260.0, screen_height() - 50.0);
    let box_y = screen_height() - box_height - 25.0;
    draw_rectangle(
        box_x,
        box_y,
        box_width,
        box_height,
        Color::from_rgba(24, 27, 31, 255),
    );
    draw_rectangle_lines(box_x, box_y, box_width, box_height, 3.0, GOLD);

    text(font, &line.speaker, box_x + 24.0, box_y + 42.0, 28.0, GOLD);
    let mut y = box_y + 94.0;
    for line in lines {
        text(font, &line, box_x + 24.0, y, body_size, WHITE);
        y += line_height;
    }
    text(
        font,
        &format!(
            "Enter / Space / click    {}/{}",
            dialog.current + 1,
            dialog.lines.len()
        ),
        box_x + 24.0,
        box_y + box_height - 24.0,
        19.0,
        GRAY,
    );
}

fn wrapped_lines(font: &Font, text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let line = lines.last_mut().unwrap();
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && measure_ui_text(&candidate, Some(font), font_size).width > max_width
        {
            lines.push(word.to_owned());
        } else {
            *line = candidate;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_conversion_is_reversible() {
        let world = vec2(137.0, 291.0);
        let offset = vec2(-42.0, 83.0);
        let screen = viewport_world_to_screen(world, offset, 1.7);
        assert!(viewport_screen_to_world(screen, offset, 1.7).distance(world) < 0.001);
    }

    #[test]
    fn terrain_compositor_uses_ground_below_overlays() {
        assert_eq!(terrain_layers("grassland"), ("grassland", None));
        assert_eq!(terrain_layers("water"), ("water", None));
        assert_eq!(terrain_layers("forest"), ("grassland", Some("forest")));
        assert_eq!(terrain_layers("village"), ("grassland", Some("village")));
        let center = Position { x: 2, y: 2 };
        let map = Map {
            width: 4,
            height: 4,
            cells: vec!["grassland".into(); 16],
        };
        assert!(
            hex_neighbors(center)
                .into_iter()
                .all(|cell| map.are_adjacent(center, cell))
        );
    }

    #[test]
    fn wesnoth_hex_edges_meet_without_gaps() {
        let left = wesnoth_hex_vertices(Vec2::ZERO, HEX_RADIUS);
        let lower_right = wesnoth_hex_vertices(vec2(54.0, 36.0), HEX_RADIUS);
        assert_eq!(left[3], lower_right[1]);
        assert_eq!(left[4], lower_right[0]);
    }

    #[test]
    fn attack_description_contains_combat_essentials() {
        let attack = Value::Map(std::collections::BTreeMap::from([
            ("name".into(), Value::String("Лук".into())),
            ("damage".into(), Value::Integer(5)),
            ("strikes".into(), Value::Integer(4)),
            ("range".into(), Value::String("ranged".into())),
            ("damage_type".into(), Value::String("pierce".into())),
            (
                "specials".into(),
                Value::List(vec![Value::String("marksman".into())]),
            ),
        ]));
        assert_eq!(attack_description(&attack), "Лук  5×4  дальн/кол · снайпер");
    }

    #[test]
    fn snapshot_units_keep_attack_details_for_the_panel() {
        let game = Game::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            "scenarios/first_battle.wml",
        )
        .unwrap();
        let status = game.query("status", Value::Nil).unwrap();
        let snapshot = read_snapshot(game.snapshot().unwrap(), status).unwrap();
        let alice = snapshot
            .objects
            .iter()
            .find(|unit| unit.id == "alice")
            .unwrap();
        assert_eq!(alice.attacks.len(), 2);
    }
}
