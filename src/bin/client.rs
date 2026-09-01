use std::{collections::BTreeSet, path::PathBuf};

use macroquad::prelude::*;
use wesnoth_engine::{
    engine::{Map, Position},
    game::{DialogLine, Game, GameSnapshot},
    value::Value,
};

const HEX_RADIUS: f32 = 38.0;
const MAP_ORIGIN: Vec2 = vec2(65.0, 75.0);
const PANEL_X: f32 = 800.0;
const SCENARIOS: [&str; 3] = [
    "scenarios/first_battle.wml",
    "scenarios/crossing.wml",
    "scenarios/outpost_defense.wml",
];

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
    gold: i64,
    recruit_types: Vec<String>,
    villages: Vec<VillageView>,
    village_income: i64,
    gross_income: i64,
    expenses: i64,
    net_income: i64,
    time_of_day: String,
    lawful_bonus: i64,
    pending_advancement: Option<AdvancementView>,
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
}

struct ReachableCell {
    position: Position,
}

#[derive(Default)]
struct AvailableActions {
    reachable: Vec<ReachableCell>,
    targets: BTreeSet<String>,
    attacks: Vec<String>,
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
    let mut scenario_index = 2;
    let mut game = load_game(scenario_index);
    let mut selected: Option<String> = None;
    let mut target: Option<String> = None;
    let mut weapon = 0usize;
    let mut actions = AvailableActions::default();
    let mut log = Vec::new();
    let mut dialog = None;
    receive_events(
        game.start_events().unwrap_or_default(),
        &mut log,
        &mut dialog,
    );

    loop {
        let requested_scenario = if is_key_pressed(KeyCode::F1) {
            Some(0)
        } else if is_key_pressed(KeyCode::F2) {
            Some(1)
        } else if is_key_pressed(KeyCode::F3) {
            Some(2)
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
        }

        let snapshot = read_snapshot(
            game.snapshot().expect("game snapshot"),
            game.query("status", Value::Nil).expect("game status"),
        )
        .expect("UI snapshot");
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

        if let Some(view) = dialog.as_mut() {
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
        } else if !snapshot.finished && is_mouse_button_pressed(MouseButton::Left) {
            let mouse = Vec2::from(mouse_position());
            if let Some(object) = object_at(&snapshot, mouse) {
                if let Ok(available) = read_actions(&game, &object.id) {
                    selected = Some(object.id.clone());
                    target = None;
                    weapon = 0;
                    actions = available;
                } else if selected.is_some() && actions.targets.contains(&object.id) {
                    target = Some(object.id.clone());
                }
            } else if let (Some(id), Some(destination)) =
                (selected.as_deref(), hex_at(&snapshot, mouse))
            {
                if actions
                    .reachable
                    .iter()
                    .any(|cell| cell.position == destination)
                {
                    let object = id.to_owned();
                    match game.execute("move", object_command(&object, Some(destination))) {
                        Ok(events) => {
                            receive_events(events, &mut log, &mut dialog);
                            actions = read_actions(&game, &object).unwrap_or_default();
                        }
                        Err(error) => log.push(format!("Error: {error}")),
                    }
                    target = None;
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
            if snapshot.can_end_turn && is_key_pressed(KeyCode::E) {
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
                                actions = read_actions(&game, attacker).unwrap_or_default();
                            }
                            Err(error) => log.push(format!("Error: {error}")),
                        }
                    }
                }
            }
        }

        clear_background(Color::from_rgba(28, 31, 34, 255));
        draw_map(
            &font,
            &snapshot,
            selected.as_deref(),
            target.as_deref(),
            &actions.reachable,
        );
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
        text(
            &font,
            "F1 battle | F2 crossing | F3 outpost | Q/W recruit | E end turn | R restart",
            24.0,
            screen_height() - 25.0,
            22.0,
            GRAY,
        );
        if let Some(dialog) = &dialog {
            draw_dialog(&font, dialog);
        }
        next_frame().await;
    }
}

fn load_game(index: usize) -> Game {
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
    Game::load(scripts, SCENARIOS[index]).expect("load scenario")
}

fn text(font: &Font, value: &str, x: f32, y: f32, size: f32, color: Color) {
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

fn hex_center(x: i64, y: i64) -> Vec2 {
    let column = x as f32 - 1.0;
    let row = y as f32 - 1.0;
    vec2(
        MAP_ORIGIN.x + column * HEX_RADIUS * 1.5,
        MAP_ORIGIN.y
            + row * HEX_RADIUS * 3.0_f32.sqrt()
            + (x % 2 == 0) as u8 as f32 * HEX_RADIUS * 3.0_f32.sqrt() / 2.0,
    )
}

fn object_at(snapshot: &ClientSnapshot, point: Vec2) -> Option<&ObjectView> {
    snapshot
        .objects
        .iter()
        .find(|object| hex_center(object.position.x, object.position.y).distance(point) < 34.0)
}

fn hex_at(snapshot: &ClientSnapshot, point: Vec2) -> Option<Position> {
    (1..=snapshot.map.height as i64)
        .flat_map(|y| (1..=snapshot.map.width as i64).map(move |x| Position { x, y }))
        .map(|position| (hex_center(position.x, position.y).distance(point), position))
        .filter(|(distance, _)| *distance < HEX_RADIUS)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, position)| position)
}

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
            let fill = if reachable.iter().any(|cell| cell.position == destination) {
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
    let objects = objects
        .iter()
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
            Ok(ObjectView {
                id,
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
        gold: integer(&status, "gold").unwrap_or(0),
        recruit_types: value_list(&status, "recruit_types")?
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
    })
}

fn read_actions(game: &Game, object: &str) -> Result<AvailableActions, String> {
    let value = game.query("actions", object_command(object, None))?;
    let reachable = value_list(&value, "reachable")?
        .iter()
        .map(|cell| {
            Ok(ReachableCell {
                position: position(cell, "position")
                    .ok_or_else(|| "reachable cell has no position".to_owned())?,
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
    let box_y = screen_height() - 270.0;
    let box_width = screen_width() - 150.0;
    draw_rectangle(
        box_x,
        box_y,
        box_width,
        210.0,
        Color::from_rgba(24, 27, 31, 255),
    );
    draw_rectangle_lines(box_x, box_y, box_width, 210.0, 3.0, GOLD);

    let line = &dialog.lines[dialog.current];
    text(font, &line.speaker, box_x + 24.0, box_y + 42.0, 28.0, GOLD);
    let mut y = box_y + 82.0;
    for line in wrapped_lines(font, &line.text, box_width - 48.0, 25.0) {
        text(font, &line, box_x + 24.0, y, 25.0, WHITE);
        y += 31.0;
    }
    text(
        font,
        &format!(
            "Enter / Space / click    {}/{}",
            dialog.current + 1,
            dialog.lines.len()
        ),
        box_x + 24.0,
        box_y + 188.0,
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
        if !line.is_empty()
            && measure_text(&candidate, Some(font), font_size as u16, 1.0).width > max_width
        {
            lines.push(word.to_owned());
        } else {
            *line = candidate;
        }
    }
    lines
}
