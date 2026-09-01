use std::{collections::BTreeMap, path::Path};

use wesnoth_engine::{game::Game, value::Value};

fn main() -> Result<(), String> {
    let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");
    let mut game = Game::load(scripts, "scenarios/first_battle.wml")?;

    println!("{} ({})", game.name, game.id);
    print_events(game.start_events()?);
    game.acknowledge_dialog()?;

    let mut weapon = "sword";
    loop {
        let snapshot = game.snapshot()?;
        if object_exists(&snapshot.objects, "alice") && object_exists(&snapshot.objects, "bob") {
            let events = game.execute(
                "resolve",
                Value::Map(BTreeMap::from([
                    ("attacker".into(), Value::String("alice".into())),
                    ("defender".into(), Value::String("bob".into())),
                    ("weapon".into(), Value::String(weapon.into())),
                ])),
            )?;
            let finished = events.iter().any(|event| {
                event.get("type").and_then(Value::as_str) == Some("scenario_finished")
            });
            print_events(events);
            if finished {
                return Ok(());
            }
        }
        let events = game.execute("end_turn", Value::Nil)?;
        let finished = events
            .iter()
            .any(|event| event.get("type").and_then(Value::as_str) == Some("scenario_finished"));
        print_events(events);
        if finished {
            return Ok(());
        }
        weapon = "bow";
    }
}

fn object_exists(objects: &Value, id: &str) -> bool {
    matches!(objects, Value::List(values) if values.iter().any(|object| {
        object.get("id").and_then(Value::as_str) == Some(id)
    }))
}

fn print_events(events: Vec<Value>) {
    for event in events {
        match event.get("type").and_then(Value::as_str) {
            Some("dialog_requested") => {
                println!("\nDialog: {}", text(&event, "dialog"));
                if let Some(Value::List(lines)) = event.get("lines") {
                    for line in lines {
                        println!("{}: {}", text(line, "speaker"), text(line, "text"));
                    }
                }
            }
            Some("battle_resolved") => {
                println!("\nBattle:");
                if let Some(Value::List(strikes)) = event.get("strikes") {
                    for strike in strikes {
                        println!(
                            "{} -> {} [{}]: {} (HP {})",
                            text(strike, "source"),
                            text(strike, "target"),
                            text(strike, "weapon"),
                            if matches!(strike.get("hit"), Some(Value::Bool(true))) {
                                "hit"
                            } else {
                                "miss"
                            },
                            strike
                                .get("target_hitpoints")
                                .and_then(Value::as_i64)
                                .unwrap_or(0),
                        );
                    }
                }
            }
            Some("object_moved") => println!("Object moved"),
            Some("unit_died") => println!("Unit died: {}", text(&event, "unit")),
            Some("turn_started") | Some("turn_ended") => println!(
                "Turn {}: {} ({})",
                event.get("turn").and_then(Value::as_i64).unwrap_or(0),
                text(&event, "side"),
                text(&event, "type")
            ),
            Some("scenario_finished") => {
                println!("Scenario finished: {}", text(&event, "result"));
            }
            _ => println!("Unknown event: {event:?}"),
        }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("?")
}
