local combat = dofile("scripts/rules/basic_combat.lua")

local units = {
    alice = {
        id = "alice", side = "player", hitpoints = 29, attacks_left = 1,
        movement_points = 6, position = { x = 2, y = 2 },
        __children = {
            defense = {{ grassland = 40 }}, resistance = {{ blade = 0 }},
            attack = {{ id = "sword", range = "melee", damage_type = "blade", damage = 5, strikes = 2 }},
        },
    },
    bob = {
        id = "bob", side = "enemy", hitpoints = 38, attacks_left = 1,
        movement_points = 5, position = { x = 3, y = 2 },
        __children = {
            defense = {{ grassland = 40 }}, resistance = {{ blade = 0 }},
            attack = {{ id = "sword", range = "melee", damage_type = "blade", damage = 9, strikes = 2 }},
        },
    },
}

local state = {
    active_side = "player",
    scenario = { __children = { objective = {} } },
}
local context = { objects = {}, map = {}, random = {}, state = {} }
function context.objects:get(id)
    local source = units[id]
    if not source then return nil end
    local snapshot = {}
    for key, value in pairs(source) do snapshot[key] = value end
    return snapshot
end
function context.objects:set(id, property, value) units[id][property] = value end
function context.map:are_adjacent() return true end
function context.map:get() return "grassland" end
function context.random:integer() return 1 end
function context.state:get(key) return state[key] end
function context.state:set(key, value) state[key] = value end

local result = combat.resolve(context, {
    attacker = "alice", defender = "bob", weapon = "sword",
})

assert(result.type == "battle_resolved")
assert(#result.strikes == 4)
assert(units.alice.hitpoints == 11)
assert(units.bob.hitpoints == 28)
