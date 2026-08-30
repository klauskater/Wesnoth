local combat = {}

local function find_attack(unit, id)
    for _, attack in ipairs(unit.attacks) do
        if attack.id == id then
            return attack
        end
    end
    error("unknown attack: " .. tostring(id))
end

local function retaliation_for(unit, range)
    for _, attack in ipairs(unit.attacks) do
        if attack.range == range then
            return attack
        end
    end
end

local function strike(context, events, source, target, attack, number)
    local terrain = context.map:get(target.position)
    local defense = assert(target.defense[terrain],
        "unit has no defense value for terrain: " .. terrain)
    local chance = 100 - defense
    local roll = context.random:integer(1, 100)
    local hit = roll <= chance

    if hit then
        target.hitpoints = math.max(0, target.hitpoints - attack.damage)
        context.objects:set(target.id, "hitpoints", target.hitpoints)
    end

    events[#events + 1] = {
        type = "strike",
        number = number,
        source = source.id,
        target = target.id,
        weapon = attack.id,
        chance = chance,
        roll = roll,
        hit = hit,
        damage = hit and attack.damage or 0,
        target_hitpoints = target.hitpoints,
    }
end

function combat.resolve(context, command)
    local attacker = context.objects:get(command.attacker)
    local defender = context.objects:get(command.defender)
    assert(attacker and defender, "attack requires two existing units")
    assert(context.map:are_adjacent(attacker.position, defender.position),
        "attack requires adjacent units")

    local attack = find_attack(attacker, command.weapon)
    local retaliation = retaliation_for(defender, attack.range)
    local events = {}
    local rounds = math.max(attack.strikes, retaliation and retaliation.strikes or 0)

    for round = 1, rounds do
        if round <= attack.strikes and defender.hitpoints > 0 then
            strike(context, events, attacker, defender, attack, round)
        end
        if defender.hitpoints == 0 then break end

        if retaliation and round <= retaliation.strikes and attacker.hitpoints > 0 then
            strike(context, events, defender, attacker, retaliation, round)
        end
        if attacker.hitpoints == 0 then break end
    end

    return {
        type = "battle_resolved",
        attacker = attacker.id,
        defender = defender.id,
        strikes = events,
        defeated = defender.hitpoints == 0 and defender.id
            or attacker.hitpoints == 0 and attacker.id
            or nil,
    }
end

return combat
