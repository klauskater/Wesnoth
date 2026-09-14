-- Game-owned combat arithmetic and exact outcome enumeration.
local combat = {}

function combat:impact(source, target, maximum, damage, heal_percent,
        heal_constant, minimum_source_hp)
    local inflicted = math.min(math.max(damage, 0), target)
    local healed = source + math.floor(inflicted * heal_percent / 100) + (heal_constant or 0)
    return math.min(maximum, math.max(minimum_source_hp or 0, healed)), target - inflicted
end

local function key(state)
    return table.concat({state.hp[1], state.hp[2], state.slow[1] and 1 or 0,
        state.slow[2] and 1 or 0, state.stopped and 1 or 0,
        state.hit[1] and 1 or 0, state.hit[2] and 1 or 0}, ":")
end

local function add(states, state, probability)
    local id = key(state)
    if states[id] then states[id].probability = states[id].probability + probability
    else state.probability = probability states[id] = state end
end

local function copy(state)
    return { hp = {state.hp[1], state.hp[2]}, slow = {state.slow[1], state.slow[2]},
        stopped = state.stopped, hit = {state.hit[1], state.hit[2]} }
end

function combat:forecast(request)
    local weapons = {request.attacker, request.defender}
    local states = {}
    add(states, { hp = {weapons[1].hp, weapons[2].hp},
        slow = {weapons[1].slowed, weapons[2].slowed}, stopped = false,
        hit = {false, false} }, 1)
    local sequence = request.sequence or {}
    if not request.sequence then
        local maximum = math.max(weapons[1].strikes, weapons[2].strikes)
        for round = 1, maximum do
            local order = request.retaliation_first and {2, 1} or {1, 2}
            for _, side in ipairs(order) do
                if round <= weapons[side].strikes then sequence[#sequence + 1] = side end
            end
        end
    end
    for _, source in ipairs(sequence) do
        assert(source == 1 or source == 2, "combat sequence side must be 1 or 2")
        local weapon, target, next_states = weapons[source], 3 - source, {}
        local chance = math.min(100, math.max(0, weapon.chance)) / 100
        for _, state in pairs(states) do
            if state.stopped then
                add(next_states, copy(state), state.probability)
            else
                if chance < 1 then add(next_states, copy(state), state.probability * (1 - chance)) end
                if chance > 0 then
                    local after = copy(state)
                    local damage = state.slow[source] and weapon.slowed_damage or weapon.damage
                    after.hp[source], after.hp[target] = self:impact(after.hp[source],
                        after.hp[target], weapon.maximum, damage, weapon.heal_percent,
                        weapon.heal_constant, weapon.minimum_source_hp)
                    after.slow[target] = after.slow[target] or weapon.slow
                    after.hit[target] = true
                    after.stopped = weapon.petrify or after.hp[source] == 0 or after.hp[target] == 0
                    add(next_states, after, state.probability * chance)
                end
            end
        end
        states = next_states
    end
    local result = {}
    for side, name in ipairs({"attacker", "defender"}) do
        local distribution, expected, unharmed = {}, 0, 0
        for _, state in pairs(states) do
            distribution[state.hp[side]] = (distribution[state.hp[side]] or 0) + state.probability
            expected = expected + state.hp[side] * state.probability
            if not state.hit[side] then unharmed = unharmed + state.probability end
        end
        local hitpoints = {}
        for hp in pairs(distribution) do hitpoints[#hitpoints + 1] = hp end
        table.sort(hitpoints, function(a, b) return a > b end)
        local rows = {}
        for _, hp in ipairs(hitpoints) do
            rows[#rows + 1] = {hp = hp, probability = distribution[hp]}
        end
        result[name] = { expected = expected, unharmed = unharmed,
            death = distribution[0] or 0, distribution = rows }
    end
    return result
end

return combat
