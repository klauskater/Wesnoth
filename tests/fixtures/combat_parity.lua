-- Reference: local upstream 1.19.27+dev, actions/attack.cpp and utils/math.hpp.
local function copy(value)
    if type(value) ~= "table" then return value end
    local result = {}
    for k, v in pairs(value) do result[k] = copy(v) end
    return result
end
local units, state
local function weapon(specials)
    return {id="sword", name="Sword", damage=5, strikes=2, range="melee", damage_type="blade",
        __children={special=specials or {}}}
end
local function reset(a, d)
    local function unit(id, side, x, attack)
        return {id=id, type=id, side=side, position={x=x,y=1}, hitpoints=1000,
            max_hitpoints=1000, max_moves=6, movement_points=6, attacks_left=2,
            level=1, experience=0, max_experience=100, alignment="neutral",
            __children={attack={attack or weapon()}, defense={{grassland=40}},
                resistance={{blade=0}}, ability={}}}
    end
    units={a=unit("a","player",1,a), d=unit("d","enemy",2,d)}
    state={turn=1, active_side="player", scenario={__children={side={
        {id="player",team_name="player"}, {id="enemy",team_name="enemy"}}, objective={}}}}
end
context.objects={}
function context.objects:get(id) return copy(units[id]) end
function context.objects:all()
    return {copy(units.a),copy(units.d)}
end
function context.objects:set(id,key,value) units[id][key]=copy(value) end
context.state={}
function context.state:get(key) return copy(state[key]) end
function context.state:set(key,value) state[key]=copy(value) end
context.map={}
function context.map:get() return "grassland" end
function context.map:are_adjacent() return true end
context.random={}
function context.random:integer() return 1 end
local command={attacker="a",defender="d",weapon="sword"}
local function preview() return rules.preview_attack(context,copy(command)) end
local function battle()
    local result=rules.resolve(context,copy(command))
    return result.type and result or result[1]
end

-- Accuracy/parry are applied BEFORE magical/marksman.
reset(weapon({{id="magical"}}),weapon())
units.a.__children.attack[1].accuracy=20
units.d.__children.attack[1].parry=30
assert(preview().chance==70)
reset(weapon({{id="marksman"}}),weapon())
units.d.__children.attack[1].parry=30
assert(preview().chance==60)

-- Upstream half ties round towards base damage; positive attacks have minimum 1.
reset()
state.turn=2 -- morning +25%, 2*1.25 rounds to 2, not 3.
units.a.alignment="lawful"
units.a.__children.attack[1].damage=2
assert(preview().damage==2)
units.a.alignment="neutral"
units.d.__children.resistance[1].blade=100
assert(preview().damage==1)
units.a.__children.attack[1].damage=0
assert(preview().damage==0)

-- Charge multiplies the unrounded damage, then the time/resistance bonus is rounded.
reset(weapon({{id="charge",multiply=2}}))
state.turn=2
units.a.alignment="lawful"
units.a.__children.attack[1].damage=2
assert(preview().damage==5)
assert(preview().retaliation_damage==10)

-- Berserk repeats whole exchanges, preserving unequal strike counts and firststrike.
reset(weapon({{id="berserk",value=3}}),weapon({{kind="firststrike",id="custom-first"}}))
units.a.__children.attack[1].strikes=3
units.d.__children.attack[1].strikes=1
local result=battle()
assert(#result.strikes==12)
for _, i in ipairs({1,5,9}) do assert(result.strikes[i].source=="d") end

-- Swarm may have zero strikes; explicit min/max are interpolated.
reset(weapon({{id="swarm"}}))
units.a.hitpoints=1
assert(preview().strikes==0)
units.a.__children.attack[1].__children.special[1]={id="swarm",swarm_attacks_min=2,swarm_attacks_max=6}
units.a.hitpoints=500
assert(preview().strikes==4)

-- Status immunities affect actual attacks and forecast alike.
reset(weapon({{id="poison"},{id="slow"},{id="drain"},{id="petrify"}}))
units.a.hitpoints=500
for _, flag in ipairs({"unpoisonable","unslowable","undrainable","unpetrifiable"}) do units.d[flag]="yes" end
result=battle()
assert(#result.strikes==4)
assert(not units.d.poisoned and not units.d.slowed and not units.d.petrified)
assert(result.strikes[1].drained==0)
reset()
units.d.invulnerable=true
assert(preview().chance==0)

-- Configured drain and heal_on_hit use actual inflicted damage and cannot kill the source.
reset(weapon({{kind="drains",id="custom-drain",value=100},{kind="heal_on_hit",value=2}}))
units.a.hitpoints=500
result=battle()
assert(result.strikes[1].drained==7)
reset(weapon({{kind="heal_on_hit",value=-1000}}))
units.d.__children.attack[1].strikes=0
units.a.hitpoints=500
result=battle()
assert(result.strikes[1].source_hitpoints==1)

-- Weapon action costs are not hard-coded to all moves and one attack.
reset()
units.a.__children.attack[1].movement_used=2
units.a.__children.attack[1].attacks_used=2
battle()
assert(units.a.movement_points==4 and units.a.attacks_left==0)

-- Generic tags, custom IDs, priority groups and duplicate-id stacking.
reset(weapon({{kind="damage",id="bonus",add=2},{kind="damage",id="bonus",add=3},
    {kind="damage",id="scale",multiply="1.5"}}))
assert(preview().damage==12)
reset(weapon({{kind="chance_to_hit",id="fixed",value=70},
    {kind="chance_to_hit",id="later",add=10,priority=1}}))
assert(preview().chance==80)
reset(weapon(),weapon({{kind="damage",id="guard",multiply="0.5",apply_to="opponent"}}))
assert(preview().damage==3)
reset(weapon({{kind="damage",id="defense-only",multiply=2,active_on="defense"}}))
assert(preview().damage==5)
reset(weapon({{kind="damage",id="attacker-only",multiply=2,apply_to="attacker"}}))
assert(preview().damage==10 and preview().retaliation_damage==5)

-- Context applies to status and strike-count tags too, not just numeric damage.
reset(weapon({{kind="poison",id="defensive-toxin",active_on="defense"}}))
battle()
assert(not units.d.poisoned)
reset(weapon({{kind="attacks",id="extra",add=2}}))
assert(preview().strikes==4)
reset(weapon({{kind="damage",id="conditional",multiply=2,
    __children={filter_opponent={{type="different"}}}}}))
assert(preview().damage==5)
units.a.__children.attack[1].__children.special[1].__children.filter_opponent[1].type="d"
assert(preview().damage==10)

reset(weapon({{kind="damage_type",replacement_type="fire"}}))
units.d.__children.resistance[1].fire=-20
assert(preview().damage==6 and preview().damage_type=="fire")
reset(weapon({{kind="disable",active_on="offense"}}))
assert(preview().disabled)
assert(not pcall(battle))
reset()
units.d.position={x=3,y=1}
assert(preview().disabled)
units.a.__children.attack[1].max_range=2
assert(not preview().disabled and preview().retaliation_strikes==0)

-- The id alone must not supply implicit mechanics when a real WML kind is present.
reset(weapon({{id="magical",kind="chance_to_hit",add=5}}))
assert(preview().chance==65)
-- No retaliation weapon must remain nil, not alias the attacker's weapon.
reset(weapon({{kind="attacks",id="double",multiply=2,apply_to="both"}}))
units.d.__children.attack={}
assert(preview().strikes==4 and preview().retaliation_strikes==0)
