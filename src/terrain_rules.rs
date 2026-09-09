//! Минимальный исполнитель правил графики террейна.
//!
//! Это не полный terrain-движок Battle for Wesnoth. На вход сюда приходит
//! упрощённый WML из `scripts/terrain/graphics.wml`, уже без макросов
//! оригинального препроцессора. `load_rules` разбирает и заранее размножает
//! вращаемые правила, а `compose` прикладывает их ко всем гексам карты.
//!
//! Результат этого модуля — не готовые текстуры, а список `PlacedImage`:
//! «у такого гекса положить изображение с таким логическим именем и слоем».
//! Физические PNG выбираются позднее в Android-клиенте.

use std::collections::BTreeSet;

use crate::engine::{Map, Position};
use crate::wml;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerrainMatch {
    Any,
    Codes {
        include: Vec<String>,
        exclude: Vec<String>,
    },
}

impl TerrainMatch {
    fn accepts(&self, terrain: Option<&str>) -> bool {
        // В оригинале карта окружена служебным off-map террейном. Благодаря
        // этой подстановке правила на краю видят соседа, но реальную карту не
        // приходится расширять дополнительным кольцом гексов.
        let terrain = terrain.unwrap_or("_offmap");
        match self {
            Self::Any => true,
            Self::Codes { include, exclude } => {
                include.iter().any(|pattern| wildcard(pattern, terrain))
                    && !exclude.iter().any(|pattern| wildcard(pattern, terrain))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    /// Смещение проверяемого гекса от якоря в аксиальных координатах (q, r).
    /// При загрузке правила оно заранее поворачивается на нужное число граней.
    pub offset: (i32, i32),
    /// Допустимые и запрещённые WML-коды террейна в этой позиции.
    pub terrain: TerrainMatch,
    /// Флаги предотвращают повторное использование одной грани несколькими
    /// правилами — например, чтобы на конце моста не появились и трап, и пирс.
    pub forbid_flags: Vec<String>,
    pub set_flags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleImage {
    /// Логическое имя, а не путь к PNG. Например `wood-bridge-dock-n`.
    pub path: String,
    /// Чем меньше слой, тем раньше рисуется изображение.
    pub layer: i16,
    /// Вторичный ключ сортировки из оригинального WML.
    pub base: (i16, i16),
    /// Оригинальная точка привязки большого составного изображения.
    /// Поле уже читается и сохраняется, но текущий клиент использует свои
    /// точки привязки в `terrain_anchor`; это важное место для доработки.
    pub center: Option<(i16, i16)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerrainRule {
    pub id: String,
    pub probability: u8,
    pub constraints: Vec<Constraint>,
    pub images: Vec<RuleImage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedImage {
    pub anchor: Position,
    pub path: String,
    pub layer: i16,
    pub base: (i16, i16),
    pub center: Option<(i16, i16)>,
}

pub fn load_rules(source: &str) -> Result<Vec<TerrainRule>, String> {
    let mut rules = Vec::new();
    for node in wml::parse(source)? {
        if node.name != "terrain_rule" {
            return Err(format!("unexpected terrain node [{}]", node.name));
        }
        let id = node.attribute("id")?;
        let probability = node
            .attributes
            .get("probability")
            .map(|value| value.parse())
            .transpose()
            .map_err(|_| format!("invalid probability in {id}"))?
            .unwrap_or(100);
        if probability > 100 {
            return Err(format!("probability exceeds 100 in {id}"));
        }
        // rotations=6 создаёт шесть вариантов одного шаблона. rotation_turns
        // позволяет оставить только нужные повороты: для прямого моста это
        // обычно две противоположные стороны, например N и S.
        let turns = if let Some(value) = node.attributes.get("rotation_turns") {
            csv(value)
                .map(|turn| {
                    turn.parse::<u8>()
                        .map_err(|_| format!("invalid rotation_turns in {id}"))
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let rotations = node
                .attributes
                .get("rotations")
                .map(|value| value.parse::<u8>())
                .transpose()
                .map_err(|_| format!("invalid rotations in {id}"))?
                .unwrap_or(1);
            (0..rotations).collect()
        };
        if turns.is_empty() || turns.iter().any(|turn| *turn > 5) {
            return Err(format!("rotations must select turns 0..=5 in {id}"));
        }
        // Имена направлений подставляются в @R0, @R1 и т. д. Они могут быть
        // стандартными n/ne/se/s/sw/nw либо специальными именами углов стен.
        let rotation_names = node
            .attributes
            .get("rotation_names")
            .map(|value| csv(value).map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_else(|| ["n", "ne", "se", "s", "sw", "nw"].map(str::to_owned).into());
        if rotation_names.len() != 6 {
            return Err(format!("rotation_names must contain six values in {id}"));
        }
        let constraints = node
            .children_named("constraint")
            .map(parse_constraint)
            .collect::<Result<Vec<_>, _>>()?;
        let images = node
            .children_named("image")
            .map(parse_image)
            .collect::<Result<Vec<_>, _>>()?;
        if constraints.is_empty() || images.is_empty() {
            return Err(format!("terrain rule {id} is empty"));
        }
        for turn in turns {
            rules.push(TerrainRule {
                id: format!("{id}:{turn}"),
                probability,
                constraints: constraints
                    .iter()
                    .cloned()
                    .map(|mut constraint| {
                        constraint.offset = rotate(constraint.offset, turn);
                        constraint.forbid_flags = constraint
                            .forbid_flags
                            .iter()
                            .map(|flag| rotated(flag, turn, &rotation_names))
                            .collect();
                        constraint.set_flags = constraint
                            .set_flags
                            .iter()
                            .map(|flag| rotated(flag, turn, &rotation_names))
                            .collect();
                        constraint
                    })
                    .collect(),
                images: images
                    .iter()
                    .cloned()
                    .map(|mut image| {
                        image.path = rotated(&image.path, turn, &rotation_names);
                        image
                    })
                    .collect(),
            });
        }
    }
    Ok(rules)
}

fn parse_constraint(node: &wml::Node) -> Result<Constraint, String> {
    let include = csv(node.attribute("terrain")?).map(str::to_owned).collect();
    let exclude = node
        .attributes
        .get("exclude")
        .map(|value| csv(value).map(str::to_owned).collect())
        .unwrap_or_default();
    Ok(Constraint {
        offset: (integer(node, "q", 0)?, integer(node, "r", 0)?),
        terrain: TerrainMatch::Codes { include, exclude },
        forbid_flags: node
            .attributes
            .get("forbid_flags")
            .map(|value| csv(value).map(str::to_owned).collect())
            .unwrap_or_default(),
        set_flags: node
            .attributes
            .get("set_flags")
            .map(|value| csv(value).map(str::to_owned).collect())
            .unwrap_or_default(),
    })
}

fn parse_image(node: &wml::Node) -> Result<RuleImage, String> {
    Ok(RuleImage {
        path: node.attribute("name")?.into(),
        layer: integer(node, "layer", 0)? as i16,
        base: pair(
            node.attributes
                .get("base")
                .map(String::as_str)
                .unwrap_or("0,0"),
        )?,
        center: node
            .attributes
            .get("center")
            .map(|value| pair(value))
            .transpose()?,
    })
}

fn integer(node: &wml::Node, name: &str, default: i32) -> Result<i32, String> {
    node.attributes
        .get(name)
        .map(|value| value.parse())
        .transpose()
        .map_err(|_| format!("invalid integer {name} in [{}]", node.name))
        .map(|value| value.unwrap_or(default))
}

fn pair(value: &str) -> Result<(i16, i16), String> {
    let (x, y) = value
        .split_once(',')
        .ok_or_else(|| format!("expected x,y, got {value}"))?;
    Ok((
        x.trim()
            .parse()
            .map_err(|_| format!("invalid pair {value}"))?,
        y.trim()
            .parse()
            .map_err(|_| format!("invalid pair {value}"))?,
    ))
}

fn csv(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn rotated(value: &str, turn: u8, names: &[String]) -> String {
    (0..6).fold(value.to_owned(), |value, offset| {
        value.replace(&format!("@R{offset}"), &names[(turn as usize + offset) % 6])
    })
}

/// Применяет нормализованные правила в порядке их записи в WML.
///
/// Для каждого правила перебираются все гексы, каждый гекс считается якорем
/// шаблона. Все `constraint` должны совпасть одновременно. После совпадения
/// выставляются флаги и добавляются изображения. Порядок важен: более частный
/// вариант должен стоять в WML раньше общего и «захватить» нужный флаг.
///
/// `seed` делает вероятность детерминированной: одна карта не меняет декор при
/// каждом кадре. Макросы оригинального Wesnoth здесь не исполняются — нужные
/// правила записываются напрямую в компактном `graphics.wml`.
pub fn compose(map: &Map, rules: &[TerrainRule], seed: u64) -> Vec<PlacedImage> {
    let mut flags = BTreeSet::new();
    let mut images = Vec::new();
    for (rule_index, rule) in rules.iter().enumerate() {
        for y in 1..=map.height as i64 {
            for x in 1..=map.width as i64 {
                let anchor = Position { x, y };
                // offset() переводит аксиальное смещение шаблона в систему
                // карты, где чётные 1-based столбцы подняты на полгекса.
                let matched = rule.constraints.iter().all(|constraint| {
                    let position = offset(anchor, constraint.offset);
                    constraint.terrain.accepts(map.raw(position).ok())
                        && constraint
                            .forbid_flags
                            .iter()
                            .all(|flag| !flags.contains(&(position.x, position.y, flag.clone())))
                });
                if !matched || !roll(seed, rule_index, anchor, rule.probability) {
                    continue;
                }
                for constraint in &rule.constraints {
                    let position = offset(anchor, constraint.offset);
                    for flag in &constraint.set_flags {
                        flags.insert((position.x, position.y, flag.clone()));
                    }
                }
                images.extend(rule.images.iter().map(|image| PlacedImage {
                    anchor,
                    path: image.path.clone(),
                    layer: image.layer,
                    base: image.base,
                    center: image.center,
                }));
            }
        }
    }
    images.sort_by_key(|image| (image.layer, image.anchor.y, image.base.1, image.anchor.x));
    images
}

pub fn rotate(offset: (i32, i32), turns: u8) -> (i32, i32) {
    let (mut q, mut r) = offset;
    for _ in 0..turns % 6 {
        (q, r) = (-r, q + r);
    }
    (q, r)
}

fn offset(anchor: Position, delta: (i32, i32)) -> Position {
    let q = anchor.x - 1 + delta.0 as i64;
    let anchor_r = anchor.y - 1 - ((anchor.x - 1) + ((anchor.x - 1) & 1)) / 2;
    let r = anchor_r + delta.1 as i64;
    Position {
        x: q + 1,
        y: r + (q + (q & 1)) / 2 + 1,
    }
}

fn wildcard(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    let mut rest = value;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let Some(found) = rest.find(part) else {
            return false;
        };
        if index == 0 && !pattern.starts_with('*') && found != 0 {
            return false;
        }
        rest = &rest[found + part.len()..];
    }
    pattern.ends_with('*') || rest.is_empty()
}

fn roll(seed: u64, rule: usize, anchor: Position, probability: u8) -> bool {
    if probability >= 100 {
        return true;
    }
    let mixed = seed
        ^ (rule as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (anchor.x as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
        ^ (anchor.y as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed.wrapping_mul(0x2545_f491_4f6c_dd1d) % 100 < probability as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(path: &str, layer: i16) -> RuleImage {
        RuleImage {
            path: path.into(),
            layer,
            base: (0, 0),
            center: None,
        }
    }

    #[test]
    fn rotates_axial_patterns_through_all_six_directions() {
        let original = (1, 0);
        assert_eq!(rotate(original, 1), (0, 1));
        assert_eq!(rotate(original, 3), (-1, 0));
        assert_eq!(rotate(original, 6), original);
    }

    #[test]
    fn wildcard_constraints_see_the_off_map_border() {
        assert!(
            TerrainMatch::Codes {
                include: vec!["*".into()],
                exclude: vec!["C*".into()],
            }
            .accepts(None)
        );
    }

    #[test]
    fn multi_hex_rules_match_and_flags_prevent_later_images() {
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Ce".into(), "Gg".into()],
        };
        let constraint = |offset, set: &[&str], forbid: &[&str]| Constraint {
            offset,
            terrain: TerrainMatch::Codes {
                include: vec![if offset == (0, 0) { "C*" } else { "G*" }.into()],
                exclude: vec![],
            },
            set_flags: set.iter().map(|value| (*value).into()).collect(),
            forbid_flags: forbid.iter().map(|value| (*value).into()).collect(),
        };
        let first = TerrainRule {
            id: "wall".into(),
            probability: 100,
            constraints: vec![
                constraint((0, 0), &["wall-ne"], &[]),
                constraint((1, -1), &[], &[]),
            ],
            images: vec![image("wall.png", 0)],
        };
        let blocked = TerrainRule {
            id: "later".into(),
            probability: 100,
            constraints: vec![constraint((0, 0), &[], &["wall-ne"])],
            images: vec![image("wrong.png", 1)],
        };
        assert_eq!(
            compose(&map, &[first, blocked], 1)
                .iter()
                .map(|i| i.path.as_str())
                .collect::<Vec<_>>(),
            vec!["wall.png"]
        );
    }

    #[test]
    fn image_order_uses_layer_before_map_position() {
        let map = Map {
            width: 1,
            height: 1,
            cells: vec!["Gg".into()],
        };
        let rule = |id: &str, layer| TerrainRule {
            id: id.into(),
            probability: 100,
            constraints: vec![Constraint {
                offset: (0, 0),
                terrain: TerrainMatch::Any,
                forbid_flags: vec![],
                set_flags: vec![],
            }],
            images: vec![image(id, layer)],
        };
        let images = compose(&map, &[rule("top", 10), rule("bottom", -10)], 1);
        assert_eq!(
            images
                .iter()
                .map(|image| image.path.as_str())
                .collect::<Vec<_>>(),
            vec!["bottom", "top"]
        );
    }

    #[test]
    fn compact_wml_rules_expand_rotations_and_metadata() {
        let rules = load_rules(
            "[terrain_rule]\nid=shore\nrotations=6\nprobability=75\n\
             [constraint]\nq=1\nr=0\nterrain=G*\nexclude=Gd\nset_flags=shore-@R0\n[/constraint]\n\
             [image]\nname=grass/green-@R0.png\nlayer=-400\nbase=54,72\ncenter=90,144\n[/image]\n\
             [/terrain_rule]",
        )
        .unwrap();
        assert_eq!(rules.len(), 6);
        assert_eq!(rules[1].constraints[0].offset, (0, 1));
        assert_eq!(rules[1].constraints[0].set_flags, ["shore-ne"]);
        assert_eq!(rules[1].images[0].path, "grass/green-ne.png");
        assert_eq!(rules[1].images[0].center, Some((90, 144)));
    }

    #[test]
    fn bundled_rules_compose_water_bases_and_boundary() {
        let rules = load_rules(include_str!("../scripts/terrain/graphics.wml")).unwrap();
        let map = Map {
            width: 2,
            height: 1,
            cells: vec!["Wo".into(), "Ww".into()],
        };
        let images = compose(&map, &rules, 1);
        assert!(images.iter().any(|image| image.path == "ocean"));
        assert!(images.iter().any(|image| image.path == "water"));
        assert!(
            images
                .iter()
                .any(|image| image.path.starts_with("transition-ocean-"))
        );
    }
}
