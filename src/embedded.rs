include!(concat!(env!("OUT_DIR"), "/embedded_scripts.rs"));

pub fn read(path: &str) -> Result<String, String> {
    get(path)
        .map(str::to_owned)
        .ok_or_else(|| format!("embedded resource not found: {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn embedded_package_loads_a_complete_scenario() {
        let game = Game::load_from("scenarios/outpost_defense.wml", &read).unwrap();
        assert_eq!(game.id, "outpost_defense");
    }
}
