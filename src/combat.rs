//! Compact probability kernel. Game rules prepare parameters in Lua.
use mlua::{Lua, Table};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct State {
    hp: [i64; 2],
    slow: [bool; 2],
    stopped: bool,
    hit: [bool; 2],
}
struct Weapon {
    chance: f64,
    damage: i64,
    slowed_damage: i64,
    strikes: usize,
    slow: bool,
    heal_percent: i64,
    petrify: bool,
}
impl Weapon {
    fn read(t: Table) -> mlua::Result<Self> {
        Ok(Self {
            chance: t.get::<f64>("chance")?.clamp(0.0, 100.0) / 100.0,
            damage: t.get::<i64>("damage")?.max(0),
            slowed_damage: t.get::<i64>("slowed_damage")?.max(0),
            strikes: t.get::<usize>("strikes")?,
            slow: t.get("slow")?,
            heal_percent: t.get("heal_percent")?,
            petrify: t.get("petrify")?,
        })
    }
}

/// Shared damage/healing arithmetic for actual strikes and forecast branches.
fn impact(source: i64, target: i64, maximum: i64, damage: i64, heal_percent: i64) -> (i64, i64) {
    let inflicted = damage.max(0).min(target);
    (
        (source + inflicted * heal_percent / 100).min(maximum),
        target - inflicted,
    )
}

pub fn install(lua: &Lua, context: &Table) -> mlua::Result<()> {
    let api = lua.create_table()?;
    api.set(
        "impact",
        lua.create_function(
            |_,
             (_self, source, target, maximum, damage, heal_percent): (
                Table,
                i64,
                i64,
                i64,
                i64,
                i64,
            )| { Ok(impact(source, target, maximum, damage, heal_percent)) },
        )?,
    )?;
    api.set(
        "forecast",
        lua.create_function(|lua, (_self, request): (Table, Table)| forecast(lua, request))?,
    )?;
    context.set("combat", api)
}

fn forecast(lua: &Lua, request: Table) -> mlua::Result<Table> {
    let a: Table = request.get("attacker")?;
    let d: Table = request.get("defender")?;
    let initial = State {
        hp: [a.get("hp")?, d.get("hp")?],
        slow: [a.get("slowed")?, d.get("slowed")?],
        stopped: false,
        hit: [false; 2],
    };
    let maximum = [a.get::<i64>("maximum")?, d.get::<i64>("maximum")?];
    let weapons = [Weapon::read(a)?, Weapon::read(d)?];
    let first: bool = request.get("retaliation_first")?;
    let mut states = BTreeMap::from([(initial, 1.0)]);
    for round in 0..weapons[0].strikes.max(weapons[1].strikes) {
        for source in if first { [1, 0] } else { [0, 1] } {
            let weapon = &weapons[source];
            if round >= weapon.strikes {
                continue;
            }
            let target = 1 - source;
            let mut next = BTreeMap::new();
            for (state, probability) in states {
                if state.stopped {
                    *next.entry(state).or_insert(0.0) += probability;
                    continue;
                }
                for (hit, chance) in [(false, 1.0 - weapon.chance), (true, weapon.chance)] {
                    if chance == 0.0 {
                        continue;
                    }
                    let mut after = state;
                    if hit {
                        let damage = if state.slow[source] {
                            weapon.slowed_damage
                        } else {
                            weapon.damage
                        };
                        let (s, t) = impact(
                            state.hp[source],
                            state.hp[target],
                            maximum[source],
                            damage,
                            weapon.heal_percent,
                        );
                        after.hp[source] = s;
                        after.hp[target] = t;
                        after.slow[target] |= weapon.slow;
                        after.hit[target] = true;
                        after.stopped = weapon.petrify || s == 0 || t == 0;
                    }
                    *next.entry(after).or_insert(0.0) += probability * chance;
                }
            }
            states = next;
        }
    }
    let result = lua.create_table()?;
    for (side, name) in [(0, "attacker"), (1, "defender")] {
        let mut distribution = BTreeMap::new();
        let mut expected = 0.0;
        let mut untouched = 0.0;
        for (state, p) in &states {
            *distribution.entry(state.hp[side]).or_insert(0.0) += p;
            expected += state.hp[side] as f64 * p;
            if !state.hit[side] {
                untouched += p;
            }
        }
        let summary = lua.create_table()?;
        summary.set("expected", expected)?;
        summary.set("unharmed", untouched)?;
        summary.set("death", distribution.get(&0).copied().unwrap_or(0.0))?;
        let rows = lua.create_table()?;
        for (index, (hp, p)) in distribution.iter().rev().enumerate() {
            let row = lua.create_table()?;
            row.set("hp", *hp)?;
            row.set("probability", *p)?;
            rows.set(index + 1, row)?;
        }
        summary.set("distribution", rows)?;
        result.set(name, summary)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capped_damage_and_drain_use_actual_damage() {
        assert_eq!(impact(3, 7, 10, 20, 50), (6, 0));
        assert_eq!(impact(9, 7, 10, 6, 50), (10, 1));
        assert_eq!(impact(3, 7, 10, 20, 0), (3, 0));
    }
    #[test]
    fn screenshot_archer_forecast() {
        let lua = Lua::new();
        let context = lua.create_table().unwrap();
        install(&lua, &context).unwrap();
        lua.globals().set("context", context).unwrap();
        lua.load(r#"local r=context.combat:forecast {
            attacker={hp=27,maximum=31,slowed=false,chance=40,damage=7,slowed_damage=4,strikes=3,slow=false,heal_percent=0,petrify=false},
            defender={hp=7,maximum=15,slowed=false,chance=0,damage=0,slowed_damage=0,strikes=0,slow=false,heal_percent=0,petrify=false},retaliation_first=false}
            assert(math.abs(r.defender.death-.784)<1e-12)
            assert(math.abs(r.defender.unharmed-.216)<1e-12)
            assert(r.attacker.unharmed==1 and r.attacker.expected==27)
        "#).exec().unwrap();
    }
}
