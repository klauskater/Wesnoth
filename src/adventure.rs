//! Описание приключения как корня графа игровых ресурсов.
//!
//! Из одного файла приключения загрузчик узнаёт, какие правила и типы юнитов
//! разрешены, а также какие процесс, карта и диалоги относятся к каждой главе.
//! Карта дальше сама ссылается на допустимые определения тайлов.

use crate::wml;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdventureRules {
    /// Lua-модуль правил всей игры: ходы, перемещение, найм и победа.
    pub game: String,
    /// Lua-модуль расчёта отдельного боя.
    pub combat: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdventureScenario {
    /// WML с самим процессом главы: сторонами, целями, событиями и юнитами.
    pub process: String,
    /// Карта главы. Внутри карты перечислены допустимые определения тайлов.
    pub map: String,
    /// Реплики, на которые ссылается процесс главы.
    pub dialogs: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adventure {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Каталоги задают полный состав доступных приключению типов юнитов.
    pub unit_catalogs: Vec<String>,
    pub rules: AdventureRules,
    pub scenarios: Vec<AdventureScenario>,
}

impl Adventure {
    pub fn parse(source: &str) -> Result<Self, String> {
        let roots = wml::parse(source)?;
        if roots.len() != 1 || roots[0].name != "adventure" {
            return Err("file must contain exactly one [adventure]".to_owned());
        }
        let root = &roots[0];
        let rules = root.child("rules")?;
        let unit_catalogs = root
            .children_named("units")
            .map(|units| units.attribute("source").map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        if unit_catalogs.is_empty() {
            return Err("[adventure] must contain at least one [units]".to_owned());
        }
        let scenarios = root
            .children_named("scenario")
            .map(|scenario| {
                Ok(AdventureScenario {
                    process: scenario.attribute("process")?.to_owned(),
                    map: scenario.attribute("map")?.to_owned(),
                    dialogs: scenario.attribute("dialogs")?.to_owned(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if scenarios.is_empty() {
            return Err("[adventure] must contain at least one [scenario]".to_owned());
        }
        Ok(Self {
            id: root.attribute("id")?.to_owned(),
            name: root.attribute("name")?.to_owned(),
            description: root.attribute("description")?.to_owned(),
            unit_catalogs,
            rules: AdventureRules {
                game: rules.attribute("game")?.to_owned(),
                combat: rules.attribute("combat")?.to_owned(),
            },
            scenarios,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_resource_graph() {
        let adventure = Adventure::parse(
            "[adventure]\nid=demo\nname=Demo\ndescription=Test\n\
             [rules]\ngame=rules.demo\ncombat=rules.combat\n[/rules]\n\
             [units]\nsource=units/demo.wml\n[/units]\n\
             [scenario]\nprocess=scenarios/one.wml\nmap=maps/one.wml\n\
             dialogs=dialogs/one.wml\n[/scenario]\n[/adventure]",
        )
        .unwrap();
        assert_eq!(adventure.id, "demo");
        assert_eq!(adventure.rules.game, "rules.demo");
        assert_eq!(adventure.unit_catalogs, ["units/demo.wml"]);
        assert_eq!(adventure.scenarios[0].map, "maps/one.wml");
    }
}
