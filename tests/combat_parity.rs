#[test]
fn scripted_combat_matches_upstream_cases() {
    let lua = mlua::Lua::new();
    let simple_ai: mlua::Table = lua
        .load(include_str!("../scripts/rules/simple_ai.lua"))
        .eval()
        .unwrap();
    let terrain: mlua::Table = lua
        .load(include_str!("../scripts/game/rules/terrain.lua"))
        .eval()
        .unwrap();
    let combat: mlua::Table = lua
        .load(include_str!("../scripts/game/rules/combat.lua"))
        .eval()
        .unwrap();
    let combat_module = combat.clone();
    lua.globals()
        .set(
            "require",
            lua.create_function(move |_, name: String| {
                if name == "rules.simple_ai" {
                    Ok(simple_ai.clone())
                } else if name == "game.rules.terrain" {
                    Ok(terrain.clone())
                } else if name == "game.rules.combat" {
                    Ok(combat_module.clone())
                } else {
                    Err(mlua::Error::runtime(format!("unknown test module: {name}")))
                }
            })
            .unwrap(),
        )
        .unwrap();
    let rules: mlua::Table = lua
        .load(include_str!("../scripts/rules/basic_combat.lua"))
        .eval()
        .unwrap();
    lua.globals().set("rules", rules).unwrap();
    let context = lua.create_table().unwrap();
    context.set("combat", combat).unwrap();
    lua.globals().set("context", context).unwrap();
    lua.load(include_str!("fixtures/combat_parity.lua"))
        .exec()
        .unwrap();
}
