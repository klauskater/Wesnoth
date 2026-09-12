#[test]
fn scripted_combat_matches_upstream_cases() {
    let lua = mlua::Lua::new();
    let rules: mlua::Table = lua
        .load(include_str!("../scripts/rules/basic_combat.lua"))
        .eval()
        .unwrap();
    lua.globals().set("rules", rules).unwrap();
    let context = lua.create_table().unwrap();
    wesnoth_engine::combat::install(&lua, &context).unwrap();
    lua.globals().set("context", context).unwrap();
    lua.load(include_str!("fixtures/combat_parity.lua"))
        .exec()
        .unwrap();
}
