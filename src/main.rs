use std::path::Path;

use wesnoth_engine::{game::Game, value::Value};

fn main() -> Result<(), String> {
    let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");
    let mut game = Game::load(scripts, "scenarios/first_battle.wml")?;

    println!("{} ({})", game.name, game.id);
    print_dialog(&game, &game.start_dialog)?;

    let mut weapon = "sword";
    loop {
        let (battle, outcome) = game.attack("alice", "bob", weapon)?;
        print_battle(&battle);
        if let Some(outcome) = outcome {
            println!("Сценарий завершён: {}", outcome.result);
            print_dialog(&game, &outcome.dialog)?;
            break;
        }
        // Первый бой показывает ответные удары, затем лук гарантирует отсутствие
        // ответа у орка и доводит демонстрацию до результата сценария.
        weapon = "bow";
    }
    Ok(())
}

fn print_dialog(game: &Game, id: &str) -> Result<(), String> {
    println!("\nДиалог: {id}");
    for line in game.dialog(id)? {
        println!("{}: {}", line.speaker, line.text);
    }
    Ok(())
}

fn print_battle(result: &Value) {
    println!("\nБой:");
    let Some(Value::List(strikes)) = result.get("strikes") else {
        return;
    };
    for strike in strikes {
        let Some(values) = strike.as_map() else {
            continue;
        };
        let hit = matches!(values.get("hit"), Some(Value::Bool(true)));
        println!(
            "{} -> {} [{}]: {} (roll {}, шанс {}%, HP {})",
            values["source"].as_str().unwrap_or("?"),
            values["target"].as_str().unwrap_or("?"),
            values["weapon"].as_str().unwrap_or("?"),
            if hit {
                "попадание"
            } else {
                "промах"
            },
            values["roll"].as_i64().unwrap_or(0),
            values["chance"].as_i64().unwrap_or(0),
            values["target_hitpoints"].as_i64().unwrap_or(0),
        );
    }
}
