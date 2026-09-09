use crate::wml;

#[derive(Clone, Debug, PartialEq)]
pub struct Adventure {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scenarios: Vec<String>,
}

impl Adventure {
    pub fn parse(source: &str) -> Result<Self, String> {
        let roots = wml::parse(source)?;
        if roots.len() != 1 || roots[0].name != "adventure" {
            return Err("file must contain exactly one [adventure]".to_owned());
        }
        let root = &roots[0];
        let scenarios = root
            .children_named("scenario")
            .map(|scenario| scenario.attribute("path").map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        if scenarios.is_empty() {
            return Err("[adventure] must contain at least one [scenario]".to_owned());
        }
        Ok(Self {
            id: root.attribute("id")?.to_owned(),
            name: root.attribute("name")?.to_owned(),
            description: root.attribute("description")?.to_owned(),
            scenarios,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adventure_manifest() {
        let adventure = Adventure::parse(
            "[adventure]\nid=demo\nname=Demo\ndescription=Test\n[scenario]\npath=scenarios/one.wml\n[/scenario]\n[/adventure]",
        )
        .unwrap();
        assert_eq!(adventure.id, "demo");
        assert_eq!(adventure.scenarios, ["scenarios/one.wml"]);
    }
}
