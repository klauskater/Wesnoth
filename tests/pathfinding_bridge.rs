use std::{collections::BTreeMap, path::Path, time::Instant};
use wesnoth_engine::{game::Game, value::Value};

const PROBE: &str = r#"
function combat.probe_reachable(context, command)
    local stats = { calls = 0, payload_nodes = 0, full_objects = 0,
        map_get = 0, neighbors = 0, are_adjacent = 0, search = 0 }
    local function size(value)
        if type(value) ~= "table" then return 1 end
        local count = 1
        for _, item in pairs(value) do count = count + 1 + size(item) end
        return count
    end
    for _, api in ipairs({"objects", "state", "map"}) do
        for name, callback in pairs(context[api]) do
            context[api][name] = function(self, ...)
                stats.calls = stats.calls + 1
                if api == "map" then
                    local key = name == "get" and "map_get" or name
                    stats[key] = (stats[key] or 0) + 1
                end
                if api == "objects" and ((name == "all" and select(1, ...) == nil)
                    or (name == "get" and select(2, ...) == nil)) then
                    stats.full_objects = stats.full_objects + 1
                end
                stats.payload_nodes = stats.payload_nodes + size({...})
                local result = callback(self, ...)
                stats.payload_nodes = stats.payload_nodes + size(result)
                return result
            end
        end
    end
    local result = (command.legacy and combat.legacy_reachable or combat.reachable)(context, command)
    stats.cells = #result
    stats.path_positions = 0
    for _, cell in ipairs(result) do stats.path_positions = stats.path_positions + #(cell.path or {}) end
    return stats
end
"#;

fn game(name: &str) -> Game {
    Game::load_from(&format!("scenarios/{name}.wml"), &|relative| {
        let mut source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("scripts")
                .join(relative),
        )
        .map_err(|e| e.to_string())?;
        if relative == "rules/basic_combat.lua" {
            source = source.replace(
                "return combat",
                &format!(
                    "{}\n{PROBE}\nreturn combat",
                    include_str!("fixtures/legacy_reachable.lua")
                ),
            );
        }
        Ok(source)
    })
    .unwrap()
}
fn command(id: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("object".into(), Value::String(id.into()))])
}
fn cells(value: &Value) -> &[Value] {
    match value {
        Value::List(items) => items,
        _ => &[],
    }
}
fn number(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap()
}

#[test]
fn native_search_preserves_cells_costs_order_paths_and_special_movement() {
    for scenario in [
        "first_battle",
        "movement_test",
        "abilities_test",
        "states_test",
        "ambush_test",
        "outpost_defense",
        "rooting_out_a_mage",
        "the_chase",
        "guarded_castle",
        "return_to_the_village",
    ] {
        let game = game(scenario);
        let units = game.query("snapshot", Value::Nil).unwrap();
        let status = game.query("status", Value::Nil).unwrap();
        for unit in cells(&units) {
            let id = unit.get("id").and_then(Value::as_str).unwrap();
            let mut args = command(id);
            args.insert(
                "inspect".into(),
                Value::Bool(unit.get("side") != status.get("active_side")),
            );
            let legacy = game
                .query("legacy_reachable", Value::Map(args.clone()))
                .unwrap();
            let native = game.query("reachable", Value::Map(args.clone())).unwrap();
            assert_eq!(native, legacy, "{scenario}: {id}");
            args.insert("paths".into(), Value::Bool(false));
            let compact = game.query("reachable", Value::Map(args.clone())).unwrap();
            assert_eq!(cells(&compact).len(), cells(&legacy).len());
            for (actual, original) in cells(&compact).iter().zip(cells(&legacy)) {
                let mut expected = original.as_map().unwrap().clone();
                expected.remove("path");
                assert_eq!(*actual, Value::Map(expected));
            }
            args.insert("paths".into(), Value::Bool(true));
            for target in cells(&legacy)
                .iter()
                .step_by((cells(&legacy).len() / 4).max(1))
            {
                args.insert(
                    "destination".into(),
                    target.get("position").unwrap().clone(),
                );
                let route = game.query("reachable", Value::Map(args.clone())).unwrap();
                assert_eq!(
                    cells(&route),
                    std::slice::from_ref(target),
                    "single route {scenario}: {id}"
                );
            }
        }
    }
}

#[test]
fn movement_boundary_is_batched_and_exports_no_complete_objects_or_paths() {
    let game = game("rooting_out_a_mage");
    let mut args = command("Arvith");
    args.insert("paths".into(), Value::Bool(false));
    let native = game
        .query("probe_reachable", Value::Map(args.clone()))
        .unwrap();
    args.insert("legacy".into(), Value::Bool(true));
    let legacy = game.query("probe_reachable", Value::Map(args)).unwrap();
    assert_eq!(number(&native, "cells"), number(&legacy, "cells"));
    assert_eq!(number(&native, "search"), 1);
    assert_eq!(number(&native, "full_objects"), 0);
    assert_eq!(number(&native, "path_positions"), 0);
    for key in ["map_get", "neighbors", "are_adjacent"] {
        assert_eq!(number(&native, key), 0, "{key}");
    }
    assert!(number(&native, "calls") <= 12);
    assert!(number(&native, "payload_nodes") < number(&legacy, "payload_nodes"));
    eprintln!("Boundary: legacy={legacy:?}; native compact={native:?}");
}

#[test]
#[ignore = "manual timing: cargo test --test pathfinding_bridge movement_benchmark -- --ignored --nocapture"]
fn movement_benchmark() {
    let game = game("rooting_out_a_mage");
    for (function, compact) in [("legacy_reachable", false), ("reachable", true)] {
        let mut args = command("Arvith");
        args.insert("paths".into(), Value::Bool(!compact));
        for _ in 0..10 {
            std::hint::black_box(game.query(function, Value::Map(args.clone())).unwrap());
        }
        let start = Instant::now();
        for _ in 0..500 {
            std::hint::black_box(game.query(function, Value::Map(args.clone())).unwrap());
        }
        eprintln!("{function}: 500 queries in {:?}", start.elapsed());
    }
}
