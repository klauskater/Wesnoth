use std::{collections::BTreeMap, path::PathBuf};

use wesnoth_engine::{engine::Position, game::Game, value::Value};

fn game() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/first_battle.wml",
    )
    .unwrap()
}

fn crossing() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/crossing.wml",
    )
    .unwrap()
}

fn outpost() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/outpost_defense.wml",
    )
    .unwrap()
}

fn rooting_out_a_mage() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/rooting_out_a_mage.wml",
    )
    .unwrap()
}

fn the_chase() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/the_chase.wml",
    )
    .unwrap()
}

fn guarded_castle() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/guarded_castle.wml",
    )
    .unwrap()
}

fn return_to_the_village() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/return_to_the_village.wml",
    )
    .unwrap()
}

fn control_test() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/control_test.wml",
    )
    .unwrap()
}

fn movement_test() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/movement_test.wml",
    )
    .unwrap()
}

fn ambush_test() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/ambush_test.wml",
    )
    .unwrap()
}

fn ai_test() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/ai_test.wml",
    )
    .unwrap()
}

fn ai_time_test() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/ai_time_test.wml",
    )
    .unwrap()
}

fn ai_objective_test() -> Game {
    Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/ai_objective_test.wml",
    )
    .unwrap()
}

fn recruit(unit_type: &str) -> Value {
    Value::Map(BTreeMap::from([(
        "unit_type".into(),
        Value::String(unit_type.into()),
    )]))
}

fn recruit_with_leader(unit_type: &str, leader: &str) -> Value {
    Value::Map(BTreeMap::from([
        ("unit_type".into(), Value::String(unit_type.into())),
        ("leader".into(), Value::String(leader.into())),
    ]))
}

fn recruit_at(unit_type: &str, leader: &str, x: i64, y: i64) -> Value {
    let mut command = recruit_with_leader(unit_type, leader)
        .as_map()
        .unwrap()
        .clone();
    command.insert(
        "destination".into(),
        Value::Map(BTreeMap::from([
            ("x".into(), Value::Integer(x)),
            ("y".into(), Value::Integer(y)),
        ])),
    );
    Value::Map(command)
}

fn specials() -> Game {
    let mut game = Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/specials_test.wml",
    )
    .unwrap();
    game.acknowledge_dialog().unwrap();
    game
}

fn abilities() -> Game {
    let mut game = Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/abilities_test.wml",
    )
    .unwrap();
    game.acknowledge_dialog().unwrap();
    game
}

fn states() -> Game {
    let mut game = Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/states_test.wml",
    )
    .unwrap();
    game.acknowledge_dialog().unwrap();
    game
}

fn move_object(id: &str, x: i64, y: i64) -> Value {
    Value::Map(BTreeMap::from([
        ("object".into(), Value::String(id.into())),
        (
            "destination".into(),
            Value::Map(BTreeMap::from([
                ("x".into(), Value::Integer(x)),
                ("y".into(), Value::Integer(y)),
            ])),
        ),
    ]))
}

fn actions_for(id: &str) -> Value {
    Value::Map(BTreeMap::from([(
        "object".into(),
        Value::String(id.into()),
    )]))
}

fn choose(choice: &str, option: &str) -> Value {
    Value::Map(BTreeMap::from([
        ("choice".into(), Value::String(choice.into())),
        ("option".into(), Value::String(option.into())),
    ]))
}

fn attack(weapon: &str) -> Value {
    Value::Map(BTreeMap::from([
        ("attacker".into(), Value::String("alice".into())),
        ("defender".into(), Value::String("bob".into())),
        ("weapon".into(), Value::String(weapon.into())),
    ]))
}

fn attack_units(attacker: &str, defender: &str, weapon: &str) -> Value {
    Value::Map(BTreeMap::from([
        ("attacker".into(), Value::String(attacker.into())),
        ("defender".into(), Value::String(defender.into())),
        ("weapon".into(), Value::String(weapon.into())),
    ]))
}

fn choose_first_advancement(game: &mut Game, events: &[Value]) {
    let Some(required) = events
        .iter()
        .find(|event| event.get("type").and_then(Value::as_str) == Some("advancement_required"))
    else {
        return;
    };
    let unit = required.get("unit").and_then(Value::as_str).unwrap();
    let Some(Value::List(options)) = required.get("options") else {
        panic!("advancement options must be a list")
    };
    let choice = options[0].as_str().unwrap();
    let _ = game.acknowledge_dialog();
    game.execute(
        "advance",
        Value::Map(BTreeMap::from([
            ("unit".into(), Value::String(unit.into())),
            ("choice".into(), Value::String(choice.into())),
        ])),
    )
    .unwrap();
}

fn movement(x: i64, y: i64) -> Value {
    Value::Map(BTreeMap::from([
        ("object".into(), Value::String("alice".into())),
        (
            "destination".into(),
            Value::Map(BTreeMap::from([
                ("x".into(), Value::Integer(x)),
                ("y".into(), Value::Integer(y)),
            ])),
        ),
    ]))
}

#[test]
fn executes_combat_rule_and_rolls_back_errors() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let before = game.snapshot().unwrap();
    assert!(game.execute("resolve", attack("missing")).is_err());
    assert_eq!(game.snapshot().unwrap(), before);
    let events = game.execute("resolve", attack("sword")).unwrap();
    assert!(matches!(events[0].get("strikes"), Some(Value::List(values)) if !values.is_empty()));
}

#[test]
fn save_restores_the_exact_running_game() {
    let mut original = game();
    let save = original.save().unwrap();
    let mut restored = Game::load_save(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        &save,
    )
    .unwrap();

    assert_eq!(
        original.start_events().unwrap(),
        restored.start_events().unwrap()
    );
    assert_eq!(original.snapshot().unwrap(), restored.snapshot().unwrap());
    assert_eq!(
        original.query("status", Value::Nil).unwrap(),
        restored.query("status", Value::Nil).unwrap()
    );
    original.acknowledge_dialog().unwrap();
    restored.acknowledge_dialog().unwrap();
    assert_eq!(
        original.execute("resolve", attack("sword")).unwrap(),
        restored.execute("resolve", attack("sword")).unwrap()
    );
}

#[test]
fn combat_awards_wesnoth_combat_experience() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let events = game.execute("resolve", attack("sword")).unwrap();
    let battle = &events[0];
    assert!(battle.get("defeated").is_none());
    let Value::List(experience) = battle.get("experience").unwrap() else {
        panic!("battle experience must be a list")
    };
    let Some(Value::List(strikes)) = battle.get("strikes") else {
        panic!("battle strikes must be a list")
    };
    assert!(strikes.len() > 1);
    assert_eq!(experience.len(), 2);
    for gained in experience {
        assert_eq!(gained.get("gained"), Some(&Value::Integer(1)));
        assert_eq!(gained.get("total"), Some(&Value::Integer(1)));
    }

    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!("snapshot objects must be a list")
    };
    for gained in experience {
        let unit = gained.get("unit").and_then(Value::as_str).unwrap();
        let object = objects
            .iter()
            .find(|object| object.get("id").and_then(Value::as_str) == Some(unit))
            .unwrap();
        assert_eq!(object.get("experience"), gained.get("total"));
        assert_eq!(object.get("level"), Some(&Value::Integer(1)));
    }
}

#[test]
fn damage_types_apply_target_resistance() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let events = game.execute("resolve", attack("bow")).unwrap();
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    let hits: Vec<_> = strikes
        .iter()
        .filter(|strike| {
            strike.get("source").and_then(Value::as_str) == Some("alice")
                && matches!(strike.get("hit"), Some(Value::Bool(true)))
        })
        .collect();
    assert!(!hits.is_empty());
    for strike in hits {
        assert_eq!(
            strike.get("damage_type").and_then(Value::as_str),
            Some("pierce")
        );
        assert_eq!(strike.get("base_damage"), Some(&Value::Integer(5)));
        assert_eq!(strike.get("resistance"), Some(&Value::Integer(20)));
        assert_eq!(strike.get("chance"), Some(&Value::Integer(60)));
        assert_eq!(strike.get("modified_damage"), Some(&Value::Integer(4)));
        assert_eq!(strike.get("damage"), Some(&Value::Integer(4)));
    }
}

#[test]
fn time_of_day_modifies_chaotic_damage() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let dawn = game.query("status", Value::Nil).unwrap();
    assert_eq!(
        dawn.get("time_of_day").and_then(Value::as_str),
        Some("dawn")
    );
    assert_eq!(dawn.get("lawful_bonus"), Some(&Value::Integer(0)));

    game.execute("end_turn", Value::Nil).unwrap();
    let morning = game.query("status", Value::Nil).unwrap();
    assert_eq!(
        morning.get("time_of_day").and_then(Value::as_str),
        Some("morning")
    );
    assert_eq!(morning.get("lawful_bonus"), Some(&Value::Integer(25)));

    let events = game.execute("end_turn", Value::Nil).unwrap();
    let chaotic_strikes: Vec<_> = events
        .iter()
        .filter(|event| event.get("type").and_then(Value::as_str) == Some("battle_resolved"))
        .flat_map(|battle| match battle.get("strikes") {
            Some(Value::List(strikes)) => strikes.iter(),
            _ => panic!("battle strikes must be a list"),
        })
        .filter(|strike| strike.get("alignment").and_then(Value::as_str) == Some("chaotic"))
        .collect();
    assert!(!chaotic_strikes.is_empty());
    for strike in chaotic_strikes {
        assert_eq!(
            strike.get("time_of_day").and_then(Value::as_str),
            Some("morning")
        );
        assert_eq!(strike.get("alignment_modifier"), Some(&Value::Integer(-25)));
        assert_eq!(strike.get("modified_damage"), Some(&Value::Integer(7)));
    }
}

#[test]
fn magical_marksman_slow_first_strike_drain_berserk_and_poison_are_lua_rules() {
    let mut game = specials();
    let events = game
        .execute("resolve", attack_units("marksman", "magic_target", "bow"))
        .unwrap();
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    assert!(
        strikes
            .iter()
            .all(|strike| strike.get("chance") == Some(&Value::Integer(70)))
    );
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("attack_event")
            && event.get("id").and_then(Value::as_str) == Some("personal_attack_test")
    }));
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("achievement_unlocked")
            && event.get("id").and_then(Value::as_str) == Some("last_blow_test")
    }));

    let mut game = specials();
    let events = game
        .execute("resolve", attack_units("fighter", "slow_target", "sword"))
        .unwrap();
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    assert!(
        strikes
            .iter()
            .any(|strike| strike.get("slowed") == Some(&Value::Bool(true)))
    );
    assert!(
        strikes
            .iter()
            .any(|strike| strike.get("slowed_damage") == Some(&Value::Bool(true)))
    );

    let mut game = specials();
    let preview = game
        .query("preview_attack", attack_units("grunt", "hero", "sword"))
        .unwrap();
    let number = |key: &str| {
        preview
            .get(key)
            .and_then(|value| {
                value
                    .as_i64()
                    .map(|value| value as f64)
                    .or_else(|| value.as_str()?.parse().ok())
            })
            .unwrap()
    };
    let naive_damage = number("damage") * number("strikes") * number("chance") / 100.0;
    assert!(number("death_probability") > 0.0);
    assert!(number("expected_damage") < naive_damage);
    let events = game
        .execute("resolve", attack_units("grunt", "hero", "sword"))
        .unwrap();
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    assert_eq!(
        strikes[0].get("source").and_then(Value::as_str),
        Some("hero")
    );
    assert!(strikes.iter().any(|strike| {
        strike.get("source").and_then(Value::as_str) == Some("hero")
            && strike.get("drained").and_then(Value::as_i64).unwrap_or(0) > 0
    }));

    let mut game = specials();
    let events = game
        .execute(
            "resolve",
            attack_units("warrior", "berserk_target", "sword"),
        )
        .unwrap();
    assert_eq!(events[0].get("berserk"), Some(&Value::Bool(true)));
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    assert!(strikes.len() > 3);
    assert!(
        strikes
            .iter()
            .any(|strike| strike.get("poisoned") == Some(&Value::Bool(true)))
    );
}

#[test]
fn leadership_healing_regeneration_steadfast_and_skirmisher_are_lua_rules() {
    let mut game = abilities();
    let Value::List(cells) = game
        .query(
            "reachable",
            Value::Map(BTreeMap::from([(
                "object".into(),
                Value::String("scout".into()),
            )])),
        )
        .unwrap()
    else {
        panic!("reachable must be a list")
    };
    assert!(!cells.is_empty());
    assert!(
        cells
            .iter()
            .all(|cell| cell.get("zoc") == Some(&Value::Bool(false)))
    );

    let events = game
        .execute(
            "resolve",
            attack_units("soldier", "steadfast_target", "sword"),
        )
        .unwrap();
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    let strike = strikes
        .iter()
        .find(|strike| strike.get("source").and_then(Value::as_str) == Some("soldier"))
        .unwrap();
    assert_eq!(strike.get("leadership_bonus"), Some(&Value::Integer(25)));
    assert_eq!(
        strike.get("effective_resistance"),
        Some(&Value::Integer(40))
    );

    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_healed")
            && event.get("unit").and_then(Value::as_str) == Some("patient")
            && event.get("source").and_then(Value::as_str) == Some("heals")
    }));
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("status_cured")
            && event.get("unit").and_then(Value::as_str) == Some("scout")
    }));
    assert!(!events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_healed")
            && event.get("unit").and_then(Value::as_str) == Some("scout")
    }));

    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_healed")
            && event.get("unit").and_then(Value::as_str) == Some("patient")
            && event.get("amount") == Some(&Value::Integer(5))
    }));
}

#[test]
fn petrified_and_unhealable_states_are_enforced_by_lua() {
    let mut game = states();
    let reachable = game
        .query(
            "reachable",
            Value::Map(BTreeMap::from([(
                "object".into(),
                Value::String("statue".into()),
            )])),
        )
        .unwrap();
    assert_eq!(reachable, Value::Map(BTreeMap::new()));

    let events = game
        .execute("resolve", attack_units("cockatrice", "gaze_target", "gaze"))
        .unwrap();
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    assert!(
        strikes
            .iter()
            .any(|strike| strike.get("petrified") == Some(&Value::Bool(true)))
    );

    let mut stunned_game = states();
    let events = stunned_game
        .execute(
            "resolve",
            attack_units("cockatrice", "stun_target", "shriek"),
        )
        .unwrap();
    let Value::List(strikes) = events[0].get("strikes").unwrap() else {
        panic!("battle strikes must be a list")
    };
    assert!(
        strikes
            .iter()
            .any(|strike| strike.get("stunned") == Some(&Value::Bool(true)))
    );
    let events = stunned_game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("status_expired")
            && event.get("unit").and_then(Value::as_str) == Some("stun_target")
            && event.get("status").and_then(Value::as_str) == Some("stunned")
    }));

    game.execute("end_turn", Value::Nil).unwrap();
    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!("snapshot objects must be a list")
    };
    let target = objects
        .iter()
        .find(|object| object.get("id").and_then(Value::as_str) == Some("gaze_target"))
        .unwrap();
    assert_eq!(target.get("petrified"), Some(&Value::Bool(true)));
    assert_eq!(target.get("movement_points"), Some(&Value::Integer(0)));
    assert_eq!(target.get("attacks_left"), Some(&Value::Integer(0)));

    let unhealable = objects
        .iter()
        .find(|object| object.get("id").and_then(Value::as_str) == Some("unhealable"))
        .unwrap();
    assert_eq!(unhealable.get("hitpoints"), Some(&Value::Integer(10)));
    assert_eq!(unhealable.get("unhealable"), Some(&Value::Bool(true)));
}

#[test]
fn reaching_the_experience_threshold_advances_the_unit() {
    let mut game = Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/advancement_test.wml",
    )
    .unwrap();
    game.acknowledge_dialog().unwrap();
    let events = game.execute("resolve", attack("bow")).unwrap();
    let Value::List(kill_experience) = events[0].get("experience").unwrap() else {
        panic!("battle experience must be a list")
    };
    assert_eq!(kill_experience[0].get("gained"), Some(&Value::Integer(8)));
    let required = events
        .iter()
        .find(|event| event.get("type").and_then(Value::as_str) == Some("advancement_required"))
        .expect("alice must choose an advancement after defeating bob");
    assert!(
        matches!(required.get("options"), Some(Value::List(options)) if options == &vec![
            Value::String("elvish_marksman".into()),
            Value::String("elvish_ranger".into()),
        ])
    );
    let status = game.query("status", Value::Nil).unwrap();
    assert_eq!(status.get("finished"), Some(&Value::Bool(false)));
    let pending = status
        .get("pending_advancement")
        .and_then(Value::as_map)
        .expect("status must describe the pending advancement");
    let Some(Value::List(details)) = pending.get("details") else {
        panic!("pending advancement must contain UI details")
    };
    assert_eq!(details.len(), 2);
    assert_eq!(
        details[0].get("id").and_then(Value::as_str),
        Some("elvish_marksman")
    );
    assert_eq!(details[0].get("hitpoints"), details[0].get("max_hitpoints"));
    assert!(matches!(details[0].get("attacks"), Some(Value::List(attacks)) if !attacks.is_empty()));
    assert!(
        game.execute(
            "advance",
            Value::Map(BTreeMap::from([
                ("unit".into(), Value::String("alice".into())),
                ("choice".into(), Value::String("orcish_grunt".into())),
            ])),
        )
        .is_err()
    );
    let events = game
        .execute(
            "advance",
            Value::Map(BTreeMap::from([
                ("unit".into(), Value::String("alice".into())),
                ("choice".into(), Value::String("elvish_ranger".into())),
            ])),
        )
        .unwrap();
    let advanced = events
        .iter()
        .find(|event| event.get("type").and_then(Value::as_str) == Some("unit_advanced"))
        .unwrap();
    assert_eq!(advanced.get("unit").and_then(Value::as_str), Some("alice"));
    assert_eq!(
        advanced.get("to").and_then(Value::as_str),
        Some("elvish_ranger")
    );
    assert_eq!(advanced.get("level"), Some(&Value::Integer(2)));

    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!("snapshot objects must be a list")
    };
    let alice = objects
        .iter()
        .find(|object| object.get("id").and_then(Value::as_str) == Some("alice"))
        .unwrap();
    assert_eq!(
        alice.get("type").and_then(Value::as_str),
        Some("elvish_ranger")
    );
    assert_eq!(alice.get("level"), Some(&Value::Integer(2)));
    assert_eq!(alice.get("hitpoints"), alice.get("max_hitpoints"));
    assert_eq!(alice.get("experience"), advanced.get("experience"));
    assert!(
        events.iter().any(|event| {
            event.get("type").and_then(Value::as_str) == Some("scenario_finished")
        })
    );
    assert_eq!(
        game.query("status", Value::Nil).unwrap().get("finished"),
        Some(&Value::Bool(true))
    );
    game.acknowledge_dialog().unwrap();
    assert!(
        game.execute(
            "advance",
            Value::Map(BTreeMap::from([
                ("unit".into(), Value::String("alice".into())),
                ("choice".into(), Value::String("elvish_marksman".into())),
            ])),
        )
        .is_err()
    );
}

#[test]
fn dialog_hands_control_to_ui() {
    let mut game = game();
    assert!(game.execute("resolve", attack("sword")).is_err());
    assert_eq!(game.start_events().unwrap().len(), 1);
    game.acknowledge_dialog().unwrap();
    assert!(game.execute("resolve", attack("sword")).is_ok());
}

#[test]
fn movement_rejects_occupied_destination_and_spends_points() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    assert!(game.execute("move", movement(3, 2)).is_err());
    let events = game.execute("move", movement(1, 1)).unwrap();
    assert_eq!(events[0].get("cost"), Some(&Value::Integer(1)));
    assert_eq!(events[0].get("movement_points"), Some(&Value::Integer(5)));
}

#[test]
fn entering_an_enemy_zone_of_control_ends_movement() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let Value::List(cells) = game
        .query(
            "reachable",
            Value::Map(BTreeMap::from([(
                "object".into(),
                Value::String("alice".into()),
            )])),
        )
        .unwrap()
    else {
        panic!("reachable must be a list")
    };
    let controlled = cells
        .iter()
        .find(|cell| matches!(cell.get("zoc"), Some(Value::Bool(true))))
        .expect("alice must be able to enter an enemy zone of control");
    let destination = controlled.get("position").unwrap().as_map().unwrap();
    let events = game
        .execute(
            "move",
            movement(
                destination["x"].as_i64().unwrap(),
                destination["y"].as_i64().unwrap(),
            ),
        )
        .unwrap();
    assert_eq!(events[0].get("movement_points"), Some(&Value::Integer(0)));
    assert_eq!(events[0].get("stopped_by_zoc"), Some(&Value::Bool(true)));
}

#[test]
fn reachable_cells_respect_points_and_impassable_terrain() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let snapshot = game.snapshot().unwrap();
    let Value::List(cells) = game
        .query(
            "reachable",
            Value::Map(BTreeMap::from([(
                "object".into(),
                Value::String("alice".into()),
            )])),
        )
        .unwrap()
    else {
        panic!("reachable must be a list")
    };
    assert!(cells.iter().all(|cell| {
        let values = cell.get("position").unwrap().as_map().unwrap();
        let position = Position {
            x: values["x"].as_i64().unwrap(),
            y: values["y"].as_i64().unwrap(),
        };
        snapshot.map.get(position).unwrap() != "water"
            && cell.get("cost").and_then(Value::as_i64).unwrap() <= 6
    }));
}

#[test]
fn lua_exposes_available_actions_without_client_side_rule_checks() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let actions = game
        .query(
            "actions",
            Value::Map(BTreeMap::from([(
                "object".into(),
                Value::String("alice".into()),
            )])),
        )
        .unwrap();
    assert!(
        matches!(actions.get("targets"), Some(Value::List(values)) if
        values.contains(&Value::String("bob".into())))
    );
    assert!(matches!(actions.get("attacks"), Some(Value::List(values)) if values.len() == 2));
    assert!(
        game.query(
            "actions",
            Value::Map(BTreeMap::from([(
                "object".into(),
                Value::String("bob".into()),
            )])),
        )
        .is_err()
    );
    let inspected = game
        .query(
            "actions",
            Value::Map(BTreeMap::from([
                ("object".into(), Value::String("bob".into())),
                ("inspect".into(), Value::Bool(true)),
            ])),
        )
        .unwrap();
    assert!(!value_list_for_test(&inspected, "reachable").is_empty());
    assert!(value_list_for_test(&inspected, "targets").is_empty());

    game.execute("resolve", attack("sword")).unwrap();
    let actions = game
        .query(
            "actions",
            Value::Map(BTreeMap::from([(
                "object".into(),
                Value::String("alice".into()),
            )])),
        )
        .unwrap();
    assert!(matches!(actions.get("attacks"), Some(Value::Map(values)) if values.is_empty()));
}

#[test]
fn ending_turn_runs_ai_and_starts_the_next_player_turn() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    game.execute("move", movement(2, 1)).unwrap();
    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("battle_resolved")
            && event.get("attacker").and_then(Value::as_str) == Some("bob")
    }));
    assert!(
        events.iter().all(|event| {
            event.get("type").and_then(Value::as_str) != Some("scenario_finished")
        })
    );
    let status = game.query("status", Value::Nil).unwrap();
    assert_eq!(status.get("turn"), Some(&Value::Integer(2)));
    assert_eq!(
        status.get("active_side"),
        Some(&Value::String("player".into()))
    );
    let snapshot = game.snapshot().unwrap();
    let Value::List(objects) = snapshot.objects else {
        panic!("snapshot objects must be a list")
    };
    let elara = objects
        .iter()
        .find(|object| object.get("id").and_then(Value::as_str) == Some("elara"))
        .unwrap();
    assert_eq!(elara.get("movement_points"), Some(&Value::Integer(6)));
}

#[test]
fn tactical_ai_recruits_retreats_and_selects_its_weapon() {
    let mut game = ai_test();
    game.acknowledge_dialog().unwrap();
    let events = game.execute("end_turn", Value::Nil).unwrap();

    assert!(
        events
            .iter()
            .any(|event| { event.get("type").and_then(Value::as_str) == Some("unit_recruited") })
    );
    let battle = events
        .iter()
        .find(|event| {
            event.get("type").and_then(Value::as_str) == Some("battle_resolved")
                && event.get("attacker").and_then(Value::as_str) == Some("weapon_tester")
        })
        .expect("the healthy archer must take the favorable ranged attack");
    assert!(matches!(battle.get("strikes"), Some(Value::List(strikes))
        if strikes.first().and_then(|strike| strike.get("weapon")).and_then(Value::as_str)
            == Some("bow")));
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("object_moved")
            && event.get("object").and_then(Value::as_str) == Some("wounded")
            && event.get("to").and_then(|position| position.get("x")) == Some(&Value::Integer(5))
            && event.get("to").and_then(|position| position.get("y")) == Some(&Value::Integer(2))
    }));
}

#[test]
fn tactical_ai_waits_for_favorable_time_of_day() {
    let mut game = ai_time_test();
    game.acknowledge_dialog().unwrap();
    let dawn = game.execute("end_turn", Value::Nil).unwrap();
    assert!(
        !dawn
            .iter()
            .any(|event| { event.get("type").and_then(Value::as_str) == Some("battle_resolved") })
    );

    let morning = game.execute("end_turn", Value::Nil).unwrap();
    assert!(morning.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("battle_resolved")
            && event.get("attacker").and_then(Value::as_str) == Some("lawful_ai")
    }));
}

#[test]
fn tactical_ai_pursues_its_scenario_objective_without_a_visible_enemy() {
    let mut game = ai_objective_test();
    game.acknowledge_dialog().unwrap();
    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("object_moved")
            && event.get("object").and_then(Value::as_str) == Some("runner")
            && event.get("to").and_then(|position| position.get("x")) == Some(&Value::Integer(6))
            && matches!(
                event.get("to").and_then(|position| position.get("y")),
                Some(Value::Integer(1)) | Some(Value::Integer(2))
            )
    }));
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("scenario_finished")
            && event.get("result").and_then(Value::as_str) == Some("victory")
    }));
}

#[test]
fn scenario_ends_only_after_the_last_unit_of_a_side_dies() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let finished = (0..10)
        .find_map(|_| {
            game.execute("end_turn", Value::Nil)
                .unwrap()
                .into_iter()
                .find(|event| {
                    event.get("type").and_then(Value::as_str) == Some("scenario_finished")
                })
        })
        .expect("AI must eventually defeat the idle player team");
    assert_eq!(
        finished.get("result").and_then(Value::as_str),
        Some("defeat")
    );
    game.acknowledge_dialog().unwrap();
    assert!(game.execute("end_turn", Value::Nil).is_err());
    assert_eq!(
        game.query("status", Value::Nil)
            .unwrap()
            .get("result")
            .and_then(Value::as_str),
        Some("defeat")
    );
    let snapshot = game.snapshot().unwrap();
    let Value::List(objects) = snapshot.objects else {
        panic!("snapshot objects must be a list")
    };
    assert!(
        objects
            .iter()
            .all(|object| object.get("side").and_then(Value::as_str) == Some("enemy"))
    );
}

#[test]
fn defeated_unit_emits_death_and_is_removed_from_the_world() {
    let mut game = specials();
    let events = game
        .execute("resolve", attack_units("marksman", "magic_target", "bow"))
        .unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_died")
            && event.get("unit").and_then(Value::as_str) == Some("magic_target")
    }));
    let snapshot = game.snapshot().unwrap();
    let Value::List(objects) = snapshot.objects else {
        panic!("snapshot objects must be a list")
    };
    assert_eq!(objects.len(), 7);
    assert!(
        objects
            .iter()
            .all(|object| object.get("id").and_then(Value::as_str) != Some("magic_target"))
    );
}

#[test]
fn crossing_reaches_bridge_spawns_ambush_and_advances_after_three_turns() {
    let mut game = crossing();
    game.acknowledge_dialog().unwrap();

    let events = game.execute("move", move_object("kael", 5, 3)).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("phase_changed")
            && event.get("phase").and_then(Value::as_str) == Some("hold_bridge")
    }));
    assert!(game.execute("end_turn", Value::Nil).is_err());
    game.acknowledge_dialog().unwrap();

    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!("snapshot objects must be a list")
    };
    assert!(
        objects
            .iter()
            .any(|object| { object.get("id").and_then(Value::as_str) == Some("raider_north") })
    );
    assert!(
        objects
            .iter()
            .all(|object| { object.get("id").and_then(Value::as_str) != Some("grom") })
    );

    let mut final_events = Vec::new();
    for _ in 0..3 {
        final_events = game.execute("end_turn", Value::Nil).unwrap();
    }
    assert!(final_events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("phase_changed")
            && event.get("phase").and_then(Value::as_str) == Some("defeat_grom")
    }));
    assert!(game.execute("end_turn", Value::Nil).is_err());
    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!("snapshot objects must be a list")
    };
    assert!(
        objects
            .iter()
            .any(|object| { object.get("id").and_then(Value::as_str) == Some("grom") })
    );
}

#[test]
fn crossing_turn_limit_defeats_a_stalling_player() {
    let mut game = crossing();
    game.acknowledge_dialog().unwrap();
    let finished = (0..10)
        .flat_map(|_| game.execute("end_turn", Value::Nil).unwrap())
        .find(|event| event.get("type").and_then(Value::as_str) == Some("scenario_finished"))
        .expect("turn limit must finish the scenario");
    assert_eq!(
        finished.get("result").and_then(Value::as_str),
        Some("defeat")
    );
}

#[test]
fn crossing_defeating_the_ambush_advances_even_off_the_bridge() {
    let mut game = crossing();
    game.acknowledge_dialog().unwrap();
    game.execute("move", move_object("kael", 5, 3)).unwrap();
    game.acknowledge_dialog().unwrap();
    game.execute("move", move_object("kael", 5, 2)).unwrap();
    game.execute("move", move_object("lyra", 5, 3)).unwrap();

    let mut advanced = false;
    for _ in 0..8 {
        for (attacker, defender) in [("kael", "raider_north"), ("lyra", "raider_south")] {
            let snapshot = game.snapshot().unwrap();
            let Value::List(objects) = &snapshot.objects else {
                panic!("snapshot objects must be a list")
            };
            let position = |id: &str| {
                let object = objects
                    .iter()
                    .find(|object| object.get("id").and_then(Value::as_str) == Some(id))?;
                let position = object.get("position")?.as_map()?;
                Some(Position {
                    x: position.get("x")?.as_i64()?,
                    y: position.get("y")?.as_i64()?,
                })
            };
            if let (Some(mut attacker_position), Some(defender_position)) =
                (position(attacker), position(defender))
            {
                if !snapshot
                    .map
                    .are_adjacent(attacker_position, defender_position)
                {
                    let Value::List(cells) =
                        game.query("reachable", actions_for(attacker)).unwrap()
                    else {
                        panic!("reachable must be a list")
                    };
                    if let Some(destination) = cells.iter().find_map(|cell| {
                        let position = cell.get("position")?.as_map()?;
                        let candidate = Position {
                            x: position.get("x")?.as_i64()?,
                            y: position.get("y")?.as_i64()?,
                        };
                        snapshot
                            .map
                            .are_adjacent(candidate, defender_position)
                            .then_some(candidate)
                    }) {
                        game.execute("move", move_object(attacker, destination.x, destination.y))
                            .unwrap();
                        attacker_position = destination;
                    }
                }
                if !snapshot
                    .map
                    .are_adjacent(attacker_position, defender_position)
                {
                    continue;
                }
                let Ok(events) = game.execute("resolve", attack_units(attacker, defender, "bow"))
                else {
                    continue;
                };
                advanced |= events.iter().any(|event| {
                    event.get("type").and_then(Value::as_str) == Some("phase_changed")
                        && event.get("phase").and_then(Value::as_str) == Some("defeat_grom")
                });
                choose_first_advancement(&mut game, &events);
            }
        }
        if advanced {
            break;
        }
        let events = game.execute("end_turn", Value::Nil).unwrap();
        advanced |= events.iter().any(|event| {
            event.get("type").and_then(Value::as_str) == Some("phase_changed")
                && event.get("phase").and_then(Value::as_str) == Some("defeat_grom")
        });
    }

    assert!(advanced, "destroying the ambush must advance the phase");
    let status = game.query("status", Value::Nil).unwrap();
    assert_eq!(
        status.get("phase").and_then(Value::as_str),
        Some("defeat_grom")
    );
}

#[test]
fn outpost_recruitment_spends_gold_and_uses_castle_hexes() {
    let mut game = outpost();
    game.acknowledge_dialog().unwrap();
    let status = game.query("status", Value::Nil).unwrap();
    assert_eq!(status.get("gold"), Some(&Value::Integer(45)));
    assert!(matches!(status.get("recruit_types"), Some(Value::List(types)) if types.len() == 2));

    let events = game.execute("recruit", recruit("elvish_fighter")).unwrap();
    assert_eq!(events[0].get("cost"), Some(&Value::Integer(14)));
    assert_eq!(events[0].get("gold"), Some(&Value::Integer(31)));
    let position = events[0].get("position").unwrap().as_map().unwrap();
    assert_eq!(
        game.snapshot()
            .unwrap()
            .map
            .get(Position {
                x: position["x"].as_i64().unwrap(),
                y: position["y"].as_i64().unwrap(),
            })
            .unwrap(),
        "castle"
    );

    game.execute("recruit", recruit("elvish_archer")).unwrap();
    game.execute("recruit", recruit("elvish_fighter")).unwrap();
    assert!(game.execute("recruit", recruit("elvish_fighter")).is_err());
}

#[test]
fn outpost_leader_must_remain_on_the_keep_to_recruit() {
    let mut game = outpost();
    game.acknowledge_dialog().unwrap();
    game.execute("move", move_object("eren", 1, 3)).unwrap();
    assert!(game.execute("recruit", recruit("elvish_fighter")).is_err());
}

#[test]
fn outpost_village_capture_adds_income() {
    let mut game = outpost();
    game.acknowledge_dialog().unwrap();
    game.execute("recruit", recruit("elvish_fighter")).unwrap();
    let events = game.execute("move", move_object("eren", 5, 2)).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("village_captured")
            && event.get("side").and_then(Value::as_str) == Some("player")
    }));
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("object_moved")
            && event.get("movement_points") == Some(&Value::Integer(0))
    }));
    let status = game.query("status", Value::Nil).unwrap();
    assert!(
        matches!(status.get("villages"), Some(Value::List(villages)) if villages.iter().any(|village| {
            village.get("x") == Some(&Value::Integer(5))
                && village.get("y") == Some(&Value::Integer(2))
                && village.get("side").and_then(Value::as_str) == Some("player")
        }))
    );

    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("economy_updated")
            && event.get("side").and_then(Value::as_str) == Some("player")
            && event.get("base_income") == Some(&Value::Integer(2))
            && event.get("villages_owned") == Some(&Value::Integer(1))
            && event.get("gross_income") == Some(&Value::Integer(5))
            && event.get("upkeep") == Some(&Value::Integer(1))
            && event.get("support") == Some(&Value::Integer(1))
            && event.get("expenses") == Some(&Value::Integer(0))
            && event.get("net_income") == Some(&Value::Integer(5))
    }));
    assert_eq!(
        game.query("status", Value::Nil).unwrap().get("gold"),
        Some(&Value::Integer(36))
    );
}

#[test]
fn imported_two_brothers_uses_the_original_first_map_and_easy_settings() {
    let mut game = rooting_out_a_mage();
    assert_eq!(game.id, "01_rooting_out_a_mage");
    assert_eq!(game.dialog(&game.start_dialog).unwrap().len(), 5);
    game.acknowledge_dialog().unwrap();

    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!("snapshot objects must be a list")
    };
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Arvith"))
    );
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Mordak"))
    );

    let snapshot = game.snapshot().unwrap();
    assert_eq!((snapshot.map.width, snapshot.map.height), (40, 32));
    let status = game.query("status", Value::Nil).unwrap();
    assert_eq!(status.get("turn_limit"), Some(&Value::Integer(18)));
    assert!(
        value_list_for_test(&status, "recruit_types")
            .iter()
            .any(|unit| unit.as_str() == Some("Horseman"))
    );
}

#[test]
fn campaign_transition_preserves_the_leader_and_puts_veterans_on_recall() {
    let mut first = rooting_out_a_mage();
    first.acknowledge_dialog().unwrap();
    first.execute("recruit", recruit("Spearman")).unwrap();
    let state = first.campaign_state().unwrap();
    let veteran = state
        .units
        .iter()
        .find(|unit| unit.get("id").and_then(Value::as_str) == Some("player_Spearman_1"))
        .unwrap();
    assert_eq!(
        veteran.get("type").and_then(Value::as_str),
        Some("Spearman")
    );

    let mut second = Game::load_with_campaign(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/the_chase.wml",
        Some(&state),
    )
    .unwrap();
    second.acknowledge_dialog().unwrap();
    let status = second.query("status", Value::Nil).unwrap();
    let destination = value_list_for_test(&status, "recruit_hexes")[0].clone();
    let recalled = second
        .execute(
            "recall",
            Value::Map(BTreeMap::from([
                ("unit".into(), Value::String("player_Spearman_1".into())),
                ("destination".into(), destination),
            ])),
        )
        .unwrap();
    assert_eq!(
        recalled[0].get("type").and_then(Value::as_str),
        Some("unit_recalled")
    );
    let Value::List(objects) = second.snapshot().unwrap().objects else {
        panic!()
    };
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("player_Spearman_1"))
    );
}

#[test]
fn imported_chase_has_the_original_map_and_three_factions() {
    let game = the_chase();
    assert_eq!(game.id, "02_the_chase");
    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!()
    };
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Nil-Galion"))
    );
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Muff_Toras"))
    );
    let snapshot = game.snapshot().unwrap();
    assert_eq!((snapshot.map.width, snapshot.map.height), (21, 55));
    let status = game.query("status", Value::Nil).unwrap();
    assert_eq!(status.get("turn_limit"), Some(&Value::Integer(28)));
}

#[test]
fn chase_discovers_villages_from_map_terrain() {
    let game = the_chase();
    let status = game.query("status", Value::Nil).unwrap();
    let villages = value_list_for_test(&status, "villages");
    assert_eq!(villages.len(), 17);
    assert!(villages.iter().all(|village| village.get("side").is_none()));
}

#[test]
fn chase_uses_upstream_unit_types_and_summons_are_reserved() {
    let game = the_chase();
    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!()
    };
    let unit_type = |id| {
        objects
            .iter()
            .find(|unit| unit.get("id").and_then(Value::as_str) == Some(id))
            .and_then(|unit| unit.get("type"))
            .and_then(Value::as_str)
    };
    assert_eq!(unit_type("Nil-Galion"), Some("Elvish Rider"));
    assert_eq!(unit_type("Muff_Toras"), Some("Dark Adept"));
    assert_eq!(unit_type("summoned_skeleton_1"), None);
}

#[test]
fn guarded_castle_opens_with_the_password_route() {
    let mut game = guarded_castle();
    assert_eq!(
        (
            game.snapshot().unwrap().map.width,
            game.snapshot().unwrap().map.height
        ),
        (39, 33)
    );
    game.acknowledge_dialog().unwrap();
    let events = game.execute("move", move_object("Arvith", 33, 29)).unwrap();
    assert!(
        events
            .iter()
            .any(
                |event| event.get("type").and_then(Value::as_str) == Some("choice_required")
                    && event.get("choice").and_then(Value::as_str) == Some("first_password")
            )
    );
    game.acknowledge_dialog().unwrap();
    let events = game
        .execute("choose", choose("first_password", "Sithrak"))
        .unwrap();
    assert_eq!(events[0].get("correct"), Some(&Value::Bool(true)));
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("phase_changed")
            && event.get("phase").and_then(Value::as_str) == Some("defeat_rotharik")
    }));
    let status = game.query("status", Value::Nil).unwrap();
    assert_eq!(
        status.get("phase").and_then(Value::as_str),
        Some("defeat_rotharik")
    );
}

#[test]
fn guarded_castle_rejects_a_wrong_first_password() {
    let mut wrong = guarded_castle();
    wrong.acknowledge_dialog().unwrap();
    wrong
        .execute("move", move_object("Arvith", 33, 29))
        .unwrap();
    wrong.acknowledge_dialog().unwrap();
    let events = wrong
        .execute("choose", choose("first_password", "Eleben"))
        .unwrap();
    assert_eq!(events[0].get("correct"), Some(&Value::Bool(false)));
    let Value::List(objects) = wrong.snapshot().unwrap().objects else {
        panic!()
    };
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Guard_leader"))
    );
}

#[test]
fn campaign_state_survives_the_whole_campaign() {
    let mut first = rooting_out_a_mage();
    first.acknowledge_dialog().unwrap();
    let first_state = first.campaign_state().unwrap();

    let second = Game::load_with_campaign(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/the_chase.wml",
        Some(&first_state),
    )
    .unwrap();
    let second_state = second.campaign_state().unwrap();
    let third = Game::load_with_campaign(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/guarded_castle.wml",
        Some(&second_state),
    )
    .unwrap();

    let third_state = third.campaign_state().unwrap();
    let fourth = Game::load_with_campaign(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/return_to_the_village.wml",
        Some(&third_state),
    )
    .unwrap();

    let Value::List(objects) = fourth.snapshot().unwrap().objects else {
        panic!()
    };
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Arvith"))
    );
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Baran"))
    );
    let status = fourth.query("status", Value::Nil).unwrap();
    let recall = value_list_for_test(&status, "recall_units");
    assert!(recall.iter().any(|unit| unit.as_str() == Some("Alwyn")));
}

#[test]
fn return_to_the_village_opens_with_both_brothers_and_no_next_chapter() {
    let mut game = return_to_the_village();
    game.acknowledge_dialog().unwrap();
    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!()
    };
    for brother in ["Arvith", "Baran"] {
        assert!(
            objects
                .iter()
                .any(|unit| unit.get("id").and_then(Value::as_str) == Some(brother))
        );
    }
    let status = game.query("status", Value::Nil).unwrap();
    let snapshot = game.snapshot().unwrap();
    assert_eq!((snapshot.map.width, snapshot.map.height), (28, 33));
    assert_eq!(status.get("turn_limit"), Some(&Value::Integer(26)));
    assert_eq!(game.next_scenario(), None);
    assert!(matches!(status.get("fog"), Some(Value::Bool(true))));
    let visible = value_list_for_test(&status, "visible_units");
    assert!(visible.iter().any(|unit| unit.as_str() == Some("Arvith")));
    assert!(!visible.iter().any(|unit| unit.as_str() == Some("Tairach")));
}

#[test]
fn controlling_units_and_villages_can_finish_an_objective() {
    let mut game = control_test();
    game.acknowledge_dialog().unwrap();
    let events = game.execute("move", move_object("scout", 3, 3)).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("scenario_finished")
            && event.get("result").and_then(Value::as_str) == Some("victory")
    }));
}

#[test]
fn a_leader_can_add_private_recruits_to_its_side() {
    let mut game = control_test();
    game.acknowledge_dialog().unwrap();
    let status = game.query("status", Value::Nil).unwrap();
    assert!(
        value_list_for_test(&status, "recruit_types")
            .iter()
            .any(|unit| unit.as_str() == Some("elvish_archer"))
    );

    let events = game
        .execute("recruit", recruit_with_leader("elvish_archer", "anchor"))
        .unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_recruited")
            && event.get("unit_type").and_then(Value::as_str) == Some("elvish_archer")
    }));
}

#[test]
fn recruitment_is_limited_to_the_selected_leaders_castle() {
    let mut game = control_test();
    game.acknowledge_dialog().unwrap();
    assert!(
        game.execute("recruit", recruit_at("elvish_archer", "anchor", 9, 3))
            .is_err()
    );
    let events = game
        .execute("recruit", recruit_at("elvish_archer", "remote", 9, 3))
        .unwrap();
    assert_eq!(
        events[0].get("position").and_then(|value| value.get("x")),
        Some(&Value::Integer(9))
    );
}

#[test]
fn resting_and_an_allied_village_respect_the_healing_cap() {
    let mut game = control_test();
    game.acknowledge_dialog().unwrap();
    game.execute("end_turn", Value::Nil).unwrap();
    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_healed")
            && event.get("unit").and_then(Value::as_str) == Some("patient")
            && event.get("amount") == Some(&Value::Integer(8))
            && event.get("source").and_then(Value::as_str) == Some("village")
    }));
}

#[test]
fn teleport_moves_between_owned_villages_for_one_point() {
    let mut game = control_test();
    game.acknowledge_dialog().unwrap();
    let actions = game.query("actions", actions_for("teleporter")).unwrap();
    let destination = value_list_for_test(&actions, "reachable")
        .iter()
        .find(|cell| {
            cell.get("position").and_then(|position| position.get("x")) == Some(&Value::Integer(6))
                && cell.get("position").and_then(|position| position.get("y"))
                    == Some(&Value::Integer(4))
        })
        .expect("the remote owned village must be reachable");
    assert_eq!(destination.get("cost"), Some(&Value::Integer(1)));
    assert_eq!(destination.get("teleport"), Some(&Value::Bool(true)));

    let events = game
        .execute("move", move_object("teleporter", 6, 4))
        .unwrap();
    assert_eq!(events[0].get("teleported"), Some(&Value::Bool(true)));
}

#[test]
fn expensive_terrain_consumes_the_last_movement_point() {
    let mut game = movement_test();
    game.acknowledge_dialog().unwrap();
    game.execute("move", move_object("walker", 2, 1)).unwrap();
    let actions = game.query("actions", actions_for("walker")).unwrap();
    let forest = value_list_for_test(&actions, "reachable")
        .iter()
        .find(|cell| {
            cell.get("position").and_then(|position| position.get("x")) == Some(&Value::Integer(1))
                && cell.get("position").and_then(|position| position.get("y"))
                    == Some(&Value::Integer(1))
        })
        .expect("a passable forest must remain reachable with one MP");
    assert_eq!(forest.get("cost"), Some(&Value::Integer(1)));
}

#[test]
fn forest_ambush_hides_until_an_enemy_moves_adjacent() {
    let mut game = ambush_test();
    game.acknowledge_dialog().unwrap();
    let status = game.query("status", Value::Nil).unwrap();
    assert!(
        !value_list_for_test(&status, "visible_units")
            .iter()
            .any(|unit| unit.as_str() == Some("ambusher"))
    );

    let events = game.execute("move", move_object("walker", 3, 3)).unwrap();
    assert_eq!(events[0].get("stopped_by_zoc"), Some(&Value::Bool(true)));
    let status = game.query("status", Value::Nil).unwrap();
    assert!(
        value_list_for_test(&status, "visible_units")
            .iter()
            .any(|unit| unit.as_str() == Some("ambusher"))
    );
    let actions = game.query("actions", actions_for("walker")).unwrap();
    assert!(
        value_list_for_test(&actions, "targets")
            .iter()
            .any(|target| target.as_str() == Some("ambusher"))
    );
}

#[test]
fn loads_generated_core_unit_catalog_across_races() {
    let game = Game::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/core_units_test.wml",
    )
    .unwrap();
    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!("snapshot objects must be a list")
    };
    assert_eq!(objects.len(), 6);
    for (id, race) in [
        ("elf", "elf"),
        ("drake", "drake"),
        ("mermaid", "merman"),
        ("orc", "orc"),
        ("lich", "undead"),
        ("dunefolk", "dunefolk"),
    ] {
        let unit = objects
            .iter()
            .find(|unit| unit.get("id").and_then(Value::as_str) == Some(id))
            .unwrap();
        assert_eq!(unit.get("race").and_then(Value::as_str), Some(race));
        assert!(unit.get("image").and_then(Value::as_str).is_some());
        assert!(matches!(unit.get("attacks"), Some(Value::List(attacks)) if !attacks.is_empty()));
        let Some(Value::List(traits)) = unit.get("traits") else {
            panic!("unit traits must be a list")
        };
        assert_eq!(traits.len(), if id == "lich" { 1 } else { 2 });
    }
    let lich = objects
        .iter()
        .find(|unit| unit.get("id").and_then(Value::as_str) == Some("lich"))
        .unwrap();
    let Some(Value::List(attacks)) = lich.get("attacks") else {
        panic!("lich attacks must be a list")
    };
    assert!(attacks.iter().any(|attack| {
        matches!(attack.get("specials"), Some(Value::List(specials))
            if specials.iter().any(|special| special.as_str() == Some("magical")))
    }));
}

fn value_list_for_test<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match value.get(key) {
        Some(Value::List(values)) => values,
        Some(Value::Map(values)) if values.is_empty() => &[],
        _ => panic!("missing list {key}"),
    }
}

#[test]
fn forecasts_cover_real_outcomes_and_do_not_consume_rng() {
    for (attacker, defender, weapon) in [("alice", "bob", "sword"), ("alice", "bob", "bow")] {
        let mut battle = game();
        battle.acknowledge_dialog().unwrap();
        let before = battle.save().unwrap();
        let request = attack_units(attacker, defender, weapon);
        let forecast = battle.query("preview_attack", request.clone()).unwrap();
        assert_eq!(battle.save().unwrap(), before);
        let events = battle.execute("resolve", request).unwrap();
        let Value::List(strikes) = events[0].get("strikes").unwrap() else {
            panic!()
        };
        for (unit, key) in [
            (attacker, "attacker_outcomes"),
            (defender, "defender_outcomes"),
        ] {
            let final_hp = strikes
                .iter()
                .rev()
                .find_map(|s| {
                    if s.get("source").and_then(Value::as_str) == Some(unit) {
                        s.get("source_hitpoints").and_then(Value::as_i64)
                    } else if s.get("target").and_then(Value::as_str) == Some(unit) {
                        s.get("target_hitpoints").and_then(Value::as_i64)
                    } else {
                        None
                    }
                })
                .unwrap();
            let Value::List(rows) = forecast.get(key).unwrap().get("distribution").unwrap() else {
                panic!()
            };
            assert!(
                rows.iter()
                    .any(|r| r.get("hp").and_then(Value::as_i64) == Some(final_hp))
            );
        }
    }
}
