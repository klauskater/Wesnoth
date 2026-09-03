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

fn recruit(unit_type: &str) -> Value {
    Value::Map(BTreeMap::from([(
        "unit_type".into(),
        Value::String(unit_type.into()),
    )]))
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
fn combat_awards_wesnoth_combat_experience() {
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    let events = game.execute("resolve", attack("sword")).unwrap();
    let battle = &events[0];
    assert!(battle.get("defeated").is_none());
    let Value::List(experience) = battle.get("experience").unwrap() else {
        panic!("battle experience must be a list")
    };
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
    game.acknowledge_dialog().unwrap();
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
    let advanced = &events[0];
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
    assert_eq!(events[0].get("cost"), Some(&Value::Integer(2)));
    assert_eq!(events[0].get("movement_points"), Some(&Value::Integer(4)));
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
    let alice = objects
        .iter()
        .find(|object| object.get("id").and_then(Value::as_str) == Some("alice"))
        .unwrap();
    assert_eq!(alice.get("movement_points"), Some(&Value::Integer(6)));
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
    let mut game = game();
    game.acknowledge_dialog().unwrap();
    game.execute("resolve", attack("sword")).unwrap();
    game.execute("end_turn", Value::Nil).unwrap();
    game.execute("resolve", attack("bow")).unwrap();
    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_died")
            && event.get("unit").and_then(Value::as_str) == Some("alice")
    }));
    assert!(
        events.iter().all(|event| {
            event.get("type").and_then(Value::as_str) != Some("scenario_finished")
        })
    );
    let snapshot = game.snapshot().unwrap();
    let Value::List(objects) = snapshot.objects else {
        panic!("snapshot objects must be a list")
    };
    assert_eq!(objects.len(), 3);
    assert!(
        objects
            .iter()
            .all(|object| object.get("id").and_then(Value::as_str) != Some("alice"))
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
    game.execute("move", move_object("lyra", 5, 4)).unwrap();

    let mut advanced = false;
    for _ in 0..8 {
        for (attacker, defender) in [("kael", "raider_north"), ("lyra", "raider_south")] {
            let Value::List(objects) = game.snapshot().unwrap().objects else {
                panic!("snapshot objects must be a list")
            };
            if objects
                .iter()
                .any(|object| object.get("id").and_then(Value::as_str) == Some(defender))
            {
                let events = game
                    .execute("resolve", attack_units(attacker, defender, "bow"))
                    .unwrap();
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
fn outpost_village_capture_adds_income_and_heals_the_garrison() {
    let mut game = outpost();
    game.acknowledge_dialog().unwrap();
    let recruited = game.execute("recruit", recruit("elvish_fighter")).unwrap();
    let fighter = recruited[0]
        .get("unit")
        .and_then(Value::as_str)
        .unwrap()
        .to_owned();

    let events = game.execute("end_turn", Value::Nil).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("economy_updated")
            && event.get("side").and_then(Value::as_str) == Some("player")
            && event.get("base_income") == Some(&Value::Integer(2))
            && event.get("upkeep") == Some(&Value::Integer(1))
            && event.get("expenses") == Some(&Value::Integer(1))
            && event.get("net_income") == Some(&Value::Integer(1))
    }));
    let events = game.execute("move", move_object(&fighter, 5, 2)).unwrap();
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("village_captured")
            && event.get("side").and_then(Value::as_str) == Some("player")
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
        Some(&Value::Integer(37))
    );
    assert!(events.iter().any(|event| {
        event.get("type").and_then(Value::as_str) == Some("unit_healed")
            && event.get("unit").and_then(Value::as_str) == Some(fighter.as_str())
    }));
}

#[test]
fn adapted_two_brothers_runs_story_milestones_and_turn_limit() {
    let mut game = rooting_out_a_mage();
    assert_eq!(game.id, "01_rooting_out_a_mage");
    assert_eq!(game.dialog(&game.start_dialog).unwrap().len(), 3);
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

    let mut scheduled = None;
    for _ in 1..6 {
        let events = game.execute("end_turn", Value::Nil).unwrap();
        scheduled = scheduled.or_else(|| {
            events
                .into_iter()
                .find(|event| event.get("type").and_then(Value::as_str) == Some("turn_event"))
        });
    }
    let scheduled = scheduled.expect("turn six must fire its story event");
    assert_eq!(
        scheduled.get("id").and_then(Value::as_str),
        Some("baran_ready")
    );
    assert_eq!(scheduled.get("turn"), Some(&Value::Integer(6)));
    game.acknowledge_dialog().unwrap();

    let mut turn_ten = None;
    for _ in 6..10 {
        let events = game.execute("end_turn", Value::Nil).unwrap();
        turn_ten = turn_ten.or_else(|| {
            events
                .into_iter()
                .find(|event| event.get("id").and_then(Value::as_str) == Some("baran_missing"))
        });
    }
    assert!(turn_ten.is_some());
    game.acknowledge_dialog().unwrap();

    let mut ending = None;
    for _ in 10..19 {
        let events = game.execute("end_turn", Value::Nil).unwrap();
        ending = ending.or_else(|| {
            events.into_iter().find(|event| {
                event.get("type").and_then(Value::as_str) == Some("scenario_finished")
            })
        });
    }
    let ending = ending.expect("turn 19 must apply the 18-turn limit");
    assert_eq!(ending.get("result").and_then(Value::as_str), Some("defeat"));
}

#[test]
fn campaign_transition_preserves_the_leader_and_puts_veterans_on_recall() {
    let mut first = rooting_out_a_mage();
    first.acknowledge_dialog().unwrap();
    first.execute("recruit", recruit("spearman")).unwrap();
    let state = first.campaign_state().unwrap();
    let veteran = state
        .units
        .iter()
        .find(|unit| unit.get("id").and_then(Value::as_str) == Some("player_spearman_1"))
        .unwrap();
    assert_eq!(
        veteran.get("type").and_then(Value::as_str),
        Some("spearman")
    );

    let mut second = Game::load_with_campaign(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        "scenarios/the_chase.wml",
        Some(&state),
    )
    .unwrap();
    second.acknowledge_dialog().unwrap();
    let recalled = second
        .execute(
            "recall",
            Value::Map(BTreeMap::from([
                ("unit".into(), Value::String("player_spearman_1".into())),
                (
                    "destination".into(),
                    Value::Map(BTreeMap::from([
                        ("x".into(), Value::Integer(2)),
                        ("y".into(), Value::Integer(9)),
                    ])),
                ),
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
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("player_spearman_1"))
    );
}

#[test]
fn adapted_chase_starts_with_hidden_kidnappers_in_reserve() {
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
        !objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Muff_Toras"))
    );
}

#[test]
fn reaching_the_north_of_the_woods_reveals_the_kidnappers() {
    let mut game = the_chase();
    game.acknowledge_dialog().unwrap();
    let mut revealed = false;
    for _ in 0..5 {
        let actions = game
            .query(
                "actions",
                Value::Map(BTreeMap::from([(
                    "object".into(),
                    Value::String("Arvith".into()),
                )])),
            )
            .unwrap();
        let Value::List(cells) = actions.get("reachable").unwrap() else {
            panic!()
        };
        let destination = cells
            .iter()
            .min_by_key(|cell| {
                let position = cell.get("position").unwrap();
                (position.get("x").and_then(Value::as_i64).unwrap() - 6).abs()
                    + (position.get("y").and_then(Value::as_i64).unwrap() - 2).abs()
            })
            .unwrap()
            .get("position")
            .unwrap()
            .clone();
        let events = game
            .execute(
                "move",
                Value::Map(BTreeMap::from([
                    ("object".into(), Value::String("Arvith".into())),
                    ("destination".into(), destination),
                ])),
            )
            .unwrap();
        if events
            .iter()
            .any(|event| event.get("type").and_then(Value::as_str) == Some("phase_changed"))
        {
            revealed = true;
            break;
        }
        game.execute("end_turn", Value::Nil).unwrap();
    }
    assert!(revealed);
    let Value::List(objects) = game.snapshot().unwrap().objects else {
        panic!()
    };
    assert!(
        objects
            .iter()
            .any(|unit| unit.get("id").and_then(Value::as_str) == Some("Muff_Toras"))
    );
}
