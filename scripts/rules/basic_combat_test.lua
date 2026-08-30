local combat = dofile("scripts/rules/basic_combat.lua")

local units = {
    alice = {
        id = "alice", hitpoints = 29, position = { x = 2, y = 2 },
        defense = { grassland = 40 },
        attacks = {{ id = "sword", range = "melee", damage = 5, strikes = 2 }},
    },
    bob = {
        id = "bob", hitpoints = 38, position = { x = 3, y = 2 },
        defense = { grassland = 40 },
        attacks = {{ id = "sword", range = "melee", damage = 9, strikes = 2 }},
    },
}

local context = { objects = {}, map = {}, random = {} }
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

local result = combat.resolve(context, {
    attacker = "alice", defender = "bob", weapon = "sword",
})

assert(result.type == "battle_resolved")
assert(#result.strikes == 4)
assert(units.alice.hitpoints == 11)
assert(units.bob.hitpoints == 28)
