local combat = {}

local function children(node, name)
    return node.__children and node.__children[name] or {}
end

local function child(node, name)
    return assert(children(node, name)[1], "missing [" .. name .. "]")
end

local function has_special(attack, id)
    for _, special in ipairs(children(attack, "special")) do
        if special.id == id then return true end
    end
    return false
end

local function ability(unit, id)
    for _, candidate in ipairs(children(unit, "ability")) do
        if candidate.id == id then return candidate end
    end
end

local function adjacent_ability(context, unit, id)
    local best
    for _, ally in ipairs(context.objects:all()) do
        local candidate = ally.side == unit.side and ally.id ~= unit.id
            and context.map:are_adjacent(ally.position, unit.position)
            and ability(ally, id)
        if candidate and (not best or (candidate.value or 0) > (best.value or 0)) then
            best = candidate
        end
    end
    return best
end

local function find_attack(unit, id)
    for _, attack in ipairs(children(unit, "attack")) do
        if attack.id == id then
            return attack
        end
    end
    error("unknown attack: " .. tostring(id))
end

local function scenario(context)
    return assert(context.state:get("scenario"), "missing scenario state")
end

local function active_side(context)
    return assert(context.state:get("active_side"), "missing active side")
end

local function assert_playing(context)
    assert(not context.state:get("finished"), "scenario is finished")
end

local function assert_no_pending_advancement(context)
    assert(not context.state:get("pending_advancement"), "advancement choice is required")
end

local function reset_side(context, side)
    for _, object in ipairs(context.objects:all()) do
        if object.side == side and object.hitpoints > 0 then
            context.objects:set(object.id, "movement_points",
                (object.petrified or object.stunned) and 0
                    or object.slowed and math.ceil(object.max_moves / 2)
                    or object.max_moves)
            context.objects:set(object.id, "attacks_left",
                (object.petrified or object.stunned) and 0 or 1)
            context.objects:set(object.id, "resting", true)
        end
    end
end

local function village_at(context, position)
    for _, village in ipairs(context.state:get("villages") or {}) do
        if village.x == position.x and village.y == position.y then return village end
    end
end

local function side_config(context, id)
    for _, side in ipairs(children(scenario(context), "side")) do
        if side.id == id then return side end
    end
    error("unknown side: " .. tostring(id))
end

local function economy(context, side_id)
    local side = side_config(context, side_id)
    local rules = scenario(context)
    local enabled = side.base_income ~= nil or side.village_income ~= nil
        or side.village_gold ~= nil or side.village_support ~= nil
        or rules.base_income ~= nil or rules.village_income ~= nil
        or rules.village_support ~= nil
    if not enabled then
        return {
            enabled = false, base_income = 0, villages_owned = 0,
            village_income = 0, village_support = 0, gross_income = 0,
            upkeep = 0, support = 0, expenses = 0, net_income = 0,
        }
    end
    local owned = 0
    for _, village in ipairs(context.state:get("villages") or {}) do
        if village.side == side_id then owned = owned + 1 end
    end
    local upkeep = 0
    for _, object in ipairs(context.objects:all()) do
        if object.side == side_id and object.is_leader ~= "yes"
            and object.upkeep ~= "free" and object.upkeep ~= "loyal" then
            upkeep = upkeep + (type(object.upkeep) == "number" and object.upkeep
                or object.level or 0)
        end
    end
    local base_income = side.base_income or scenario(context).base_income or 0
    local village_income = side.village_income or side.village_gold
        or scenario(context).village_income or 0
    local village_support = side.village_support
        or scenario(context).village_support or 0
    local support = owned * village_support
    local expenses = math.max(0, upkeep - support)
    local gross = base_income + owned * village_income
    return {
        enabled = true,
        base_income = base_income,
        villages_owned = owned,
        village_income = village_income,
        village_support = village_support,
        gross_income = gross,
        upkeep = upkeep,
        support = support,
        expenses = expenses,
        net_income = gross - expenses,
    }
end

local function start_side(context, side)
    local events = {}
    local balance = economy(context, side)
    if balance.enabled then
        local gold = context.state:get("gold:" .. side) + balance.net_income
        context.state:set("gold:" .. side, gold)
        balance.type = "economy_updated"
        balance.side = side
        balance.gold = gold
        events[#events + 1] = balance
    end
    local healing = scenario(context).village_heal or 0
    for _, object in ipairs(context.objects:all()) do
        if object.side == side then
            local village = village_at(context, object.position)
            local regeneration = ability(object, "regenerates")
            local healer = adjacent_ability(context, object, "heals")
            local curer = adjacent_ability(context, object, "cures")
            local cure = village and village.side == side or regeneration or curer
            local poison_protected = cure or healer
            local rest_healing = object.resting and (scenario(context).rest_heal or 2) or 0
            if object.poisoned then
                if cure then
                    context.objects:set(object.id, "poisoned", false)
                    events[#events + 1] = {
                        type = "status_cured", unit = object.id, status = "poisoned",
                    }
                elseif not poison_protected then
                    local damage = math.min(8, object.hitpoints - 1)
                    if damage > 0 then
                        context.objects:set(object.id, "hitpoints", object.hitpoints - damage)
                        object.hitpoints = object.hitpoints - damage
                        events[#events + 1] = {
                            type = "poison_damage", unit = object.id, damage = damage,
                            hitpoints = object.hitpoints,
                        }
                    end
                end
            end
            local ability_healing = object.poisoned and 0 or math.max(
                regeneration and regeneration.value or 0, healer and healer.value or 0,
                curer and curer.value or 0)
            local turn_healing = rest_healing + math.max(
                not object.poisoned and village and village.side == side and healing or 0,
                ability_healing)
            if not object.unhealable and turn_healing > 0
                and object.hitpoints < object.max_hitpoints then
                local amount = math.min(turn_healing, object.max_hitpoints - object.hitpoints)
                context.objects:set(object.id, "hitpoints", object.hitpoints + amount)
                events[#events + 1] = {
                    type = "unit_healed", unit = object.id, amount = amount,
                    hitpoints = object.hitpoints + amount,
                    source = regeneration and "regenerates" or healer and "heals" or "village",
                }
            end
        end
    end
    reset_side(context, side)
    return events
end

local function finish_side(context, side)
    local events = {}
    for _, object in ipairs(context.objects:all()) do
        if object.side == side and object.slowed then
            context.objects:set(object.id, "slowed", false)
            events[#events + 1] = {
                type = "status_expired", unit = object.id, status = "slowed",
            }
        end
        if object.side == side and object.stunned then
            context.objects:set(object.id, "stunned", false)
            events[#events + 1] = {
                type = "status_expired", unit = object.id, status = "stunned",
            }
        end
    end
    return events
end

local function finished(context, result, dialog)
    context.state:set("finished", true)
    return { type = "scenario_finished", result = result, dialog = dialog }
end

local function objective_event(context, defeated)
    for _, objective in ipairs(children(scenario(context), "objective")) do
        local matched = false
        if objective.when == "side_defeated" then
            matched = true
            for _, object in ipairs(context.objects:all()) do
                if object.side == objective.side then
                    matched = false
                    break
                end
            end
        elseif objective.when == "unit_defeated" then
            matched = defeated == objective.unit
        elseif objective.when == "turn_limit" then
            matched = context.state:get("turn") > objective.turn
        end
        if matched then
            return finished(context, objective.result, objective.dialog)
        end
    end
end

local function phase(context)
    local id = context.state:get("phase")
    for _, candidate in ipairs(children(scenario(context), "phase")) do
        if candidate.id == id then return candidate end
    end
end

local function occupied(context, position)
    for _, object in ipairs(context.objects:all()) do
        if object.position.x == position.x and object.position.y == position.y then
            return true
        end
    end
    return false
end

local function position_key(position)
    return position.x .. "," .. position.y
end

local function safe_spawn_position(context, object)
    if not occupied(context, object.position) then return object.position end
    for _, position in ipairs(context.map:neighbors(object.position)) do
        local costs = child(object, "movement_costs")
        if costs[context.map:get(position)] and not occupied(context, position) then
            return position
        end
    end
    error("no free spawn position for " .. object.id)
end

local function enter_phase(context, id)
    context.state:set("phase", id)
    context.state:set("phase_started_turn", context.state:get("turn"))
    local current = assert(phase(context), "unknown phase: " .. tostring(id))
    local spawned = {}
    if current.spawn_group then
        for _, reserve_id in ipairs(context.state:get("reserves") or {}) do
            local object = context.state:get("reserve:" .. reserve_id)
            if object and object.reserve_group == current.spawn_group then
                object.position = safe_spawn_position(context, object)
                object.x = object.position.x
                object.y = object.position.y
                context.objects:add(object)
                context.state:set("reserve:" .. reserve_id, nil)
                spawned[#spawned + 1] = reserve_id
            end
        end
    end
    return {
        type = "phase_changed",
        phase = id,
        dialog = current.dialog,
        spawned = spawned,
    }
end

local function side_defeated(context, side)
    for _, object in ipairs(context.objects:all()) do
        if object.side == side then return false end
    end
    return true
end

local function advancement_options(object)
    local result = {}
    for id in string.gmatch(object.advances_to or "", "[^,%s]+") do
        result[#result + 1] = id
    end
    return result
end

local type_properties = {
    id = true, type = true, name = true, max_hitpoints = true, max_moves = true,
    cost = true, level = true, max_experience = true, advances_to = true,
    alignment = true, __children = true,
}

local function apply_advancement(context, object, choice)
    local templates = assert(context.state:get("unit_types"), "missing unit types")
    local advanced = assert(templates[choice], "unknown advancement type: " .. tostring(choice))
    for key, value in pairs(object) do
        if not type_properties[key] then advanced[key] = value end
    end
    local from = object.type
    local remaining = object.experience - object.max_experience
    advanced.id = object.id
    advanced.type = choice
    advanced.side = object.side
    advanced.x = object.position.x
    advanced.y = object.position.y
    advanced.position = object.position
    advanced.experience = remaining
    advanced.hitpoints = advanced.max_hitpoints
    advanced.movement_points = 0
    advanced.attacks_left = 0
    advanced.is_leader = object.is_leader
    advanced.poisoned = false
    advanced.slowed = false
    advanced.petrified = false
    advanced.stunned = false
    advanced.resting = false
    context.objects:remove(object.id)
    context.objects:add(advanced)
    return {
        type = "unit_advanced",
        unit = object.id,
        from = from,
        to = advanced.type,
        level = advanced.level,
        experience = remaining,
        maximum = advanced.max_experience,
        hitpoints = advanced.hitpoints,
    }
end


local function advance_unit(context, object)
    if object.experience < object.max_experience then return end
    local options = advancement_options(object)
    if #options == 0 then return end
    if #options > 1 and side_config(context, object.side).controller == "human" then
        if context.state:get("pending_advancement") then return end
        local pending = { unit = object.id, options = options }
        context.state:set("pending_advancement", pending)
        context.objects:set(object.id, "movement_points", 0)
        context.objects:set(object.id, "attacks_left", 0)
        return {
            type = "advancement_required",
            unit = object.id,
            from = object.type,
            options = options,
        }
    end
    return apply_advancement(context, object, options[1])
end

local times = {
    { id = "dawn", lawful_bonus = 0 },
    { id = "morning", lawful_bonus = 25 },
    { id = "afternoon", lawful_bonus = 25 },
    { id = "dusk", lawful_bonus = 0 },
    { id = "first_watch", lawful_bonus = -25 },
    { id = "second_watch", lawful_bonus = -25 },
}

local function time_of_day(context)
    return times[(context.state:get("turn") - 1) % #times + 1]
end

local function alignment_modifier(alignment, lawful_bonus)
    if alignment == "lawful" then return lawful_bonus end
    if alignment == "chaotic" then return -lawful_bonus end
    return 0
end

local function leadership_bonus(context, source)
    local bonus = 0
    for _, leader in ipairs(context.objects:all()) do
        if leader.side == source.side and leader.id ~= source.id
            and leader.level > source.level
            and context.map:are_adjacent(leader.position, source.position)
            and ability(leader, "leadership") then
            bonus = math.max(bonus, (leader.level - source.level) * 25)
        end
    end
    return bonus
end

local function attack_stats(context, source, target, attack, attacking)
    local terrain = context.map:get(target.position)
    local defense = assert(child(target, "defense")[terrain],
        "unit has no defense value for terrain: " .. terrain)
    local chance = 100 - defense
    if has_special(attack, "magical") then
        chance = 70
    elseif attacking and has_special(attack, "marksman") then
        chance = math.max(chance, 60)
    end
    local damage_type = assert(attack.damage_type, "attack has no damage type")
    local resistance = assert(child(target, "resistance")[damage_type],
        "unit has no resistance for damage type: " .. damage_type)
    local time = time_of_day(context)
    local alignment = source.alignment or "neutral"
    local alignment_bonus = alignment_modifier(alignment, time.lawful_bonus)
    local leadership = leadership_bonus(context, source)
    local slow_modifier = source.slowed and 50 or 100
    local effective_resistance = resistance
    if attacking and resistance > 0 and ability(target, "steadfast") then
        effective_resistance = math.min(50, resistance * 2)
    end
    local modified_damage = math.max(0,
        math.floor(attack.damage * (100 + alignment_bonus + leadership)
            * (100 - effective_resistance)
            * slow_modifier / 1000000 + 0.5))
    return chance, modified_damage, damage_type, resistance, effective_resistance,
        time, alignment, alignment_bonus, leadership
end

local function retaliation_for(context, attacker, defender, attack)
    local best, best_rating
    for _, candidate in ipairs(children(defender, "attack")) do
        if candidate.range == attack.range and (candidate.defense_weight or 1) > 0 then
            local chance, damage = attack_stats(context, defender, attacker, candidate, false)
            local rating = candidate.strikes * damage * chance * (candidate.defense_weight or 1)
            if not best_rating or rating > best_rating then
                best, best_rating = candidate, rating
            end
        end
    end
    return best
end

local function strike(context, events, source, target, attack, number, attacking)
    local chance, modified_damage, damage_type, resistance, effective_resistance,
        time, alignment, alignment_bonus, leadership =
        attack_stats(context, source, target, attack, attacking)
    local roll = context.random:integer(1, 100)
    local hit = roll <= chance
    local damage = 0
    local applied_poison = false
    local applied_slow = false
    local applied_petrify = false
    local applied_stun = false
    local drained = 0

    if hit then
        local before = target.hitpoints
        target.hitpoints = math.max(0, target.hitpoints - modified_damage)
        damage = before - target.hitpoints
        context.objects:set(target.id, "hitpoints", target.hitpoints)
        if has_special(attack, "poison") and not target.poisoned then
            target.poisoned = true
            context.objects:set(target.id, "poisoned", true)
            applied_poison = true
        end
        if has_special(attack, "slow") and not target.slowed then
            target.slowed = true
            context.objects:set(target.id, "slowed", true)
            applied_slow = true
        end
        if has_special(attack, "petrify") and not target.petrified then
            target.petrified = true
            context.objects:set(target.id, "petrified", true)
            context.objects:set(target.id, "movement_points", 0)
            context.objects:set(target.id, "attacks_left", 0)
            applied_petrify = true
        end
        if has_special(attack, "stun") and not target.stunned then
            target.stunned = true
            context.objects:set(target.id, "stunned", true)
            context.objects:set(target.id, "movement_points", 0)
            context.objects:set(target.id, "attacks_left", 0)
            applied_stun = true
        end
        if has_special(attack, "drain") and source.hitpoints < source.max_hitpoints then
            drained = math.min(math.floor(damage / 2), source.max_hitpoints - source.hitpoints)
            if drained > 0 then
                source.hitpoints = source.hitpoints + drained
                context.objects:set(source.id, "hitpoints", source.hitpoints)
            end
        end
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
        damage = damage,
        base_damage = attack.damage,
        damage_type = damage_type,
        resistance = resistance,
        effective_resistance = effective_resistance,
        modified_damage = modified_damage,
        alignment = alignment,
        alignment_modifier = alignment_bonus,
        leadership_bonus = leadership,
        time_of_day = time.id,
        slowed_damage = source.slowed == true,
        poisoned = applied_poison,
        slowed = applied_slow,
        petrified = applied_petrify,
        stunned = applied_stun,
        drained = drained,
        target_hitpoints = target.hitpoints,
        source_experience = source.experience or 0,
    }
end

function combat.initialize(context)
    local occupied = {}
    local reserves = {}
    for _, object in ipairs(context.objects:all()) do
        local position = { x = assert(object.x), y = assert(object.y) }
        context.map:get(position)
        local key = position.x .. "," .. position.y
        if occupied[key] then
            error("objects " .. occupied[key] .. " and " .. object.id
                .. " share a position")
        end
        occupied[key] = object.id
        context.objects:set(object.id, "position", position)
        context.objects:set(object.id, "hitpoints", object.initial_hitpoints or assert(object.max_hitpoints))
        context.objects:set(object.id, "movement_points", assert(object.max_moves))
        context.objects:set(object.id, "attacks_left", 1)
        context.objects:set(object.id, "experience", object.experience or 0)
        context.objects:set(object.id, "resting", false)
        context.objects:set(object.id, "poisoned", object.poisoned == true or object.poisoned == "yes")
        context.objects:set(object.id, "slowed", object.slowed == true or object.slowed == "yes")
        context.objects:set(object.id, "petrified",
            object.petrified == true or object.petrified == "yes")
        context.objects:set(object.id, "unhealable",
            object.unhealable == true or object.unhealable == "yes")
        context.objects:set(object.id, "stunned",
            object.stunned == true or object.stunned == "yes")
        assert(object.level and object.max_experience, "unit type has no experience fields")
        assert(object.alignment, "unit type has no alignment")
        if object.reserve_group then
            context.state:set("reserve:" .. object.id, context.objects:get(object.id))
            reserves[#reserves + 1] = object.id
            context.objects:remove(object.id)
        end
    end
    context.state:set("reserves", reserves)
    context.state:set("turn", 1)
    context.state:set("active_side", assert(children(scenario(context), "side")[1]).id)
    context.state:set("finished", false)
    context.state:set("next_recruit_id", 1)
    local villages = {}
    for _, village in ipairs(children(scenario(context), "village")) do
        assert(context.map:get(village) == "village", "[village] must reference village terrain")
        villages[#villages + 1] = { x = village.x, y = village.y, side = village.side }
    end
    context.state:set("villages", villages)
    for _, side in ipairs(children(scenario(context), "side")) do
        context.state:set("gold:" .. side.id, side.gold or 0)
    end
    local first_phase = children(scenario(context), "phase")[1]
    if first_phase then
        context.state:set("phase", first_phase.id)
        context.state:set("phase_started_turn", 1)
    end
    return { type = "initialized" }
end

function combat.snapshot(context)
    local result = {}
    for _, object in ipairs(context.objects:all()) do
        result[#result + 1] = {
            id = object.id,
            type = object.type,
            name = object.name,
            side = object.side,
            position = object.position,
            hitpoints = object.hitpoints,
            max_hitpoints = object.max_hitpoints,
            movement_points = object.movement_points,
            max_movement_points = object.max_moves,
            attacks_left = object.attacks_left,
            level = object.level,
            experience = object.experience,
            max_experience = object.max_experience,
            alignment = object.alignment,
            movement_costs = child(object, "movement_costs"),
            resistances = child(object, "resistance"),
            poisoned = object.poisoned == true,
            slowed = object.slowed == true,
            petrified = object.petrified == true,
            unhealable = object.unhealable == true,
            stunned = object.stunned == true,
        }
    end
    return result
end

function combat.status(context)
    local can_end_turn = false
    if not context.state:get("finished") then
        for _, side in ipairs(children(scenario(context), "side")) do
            if side.id == active_side(context) and side.controller == "human" then
                can_end_turn = true
            end
        end
    end
    local current_phase = phase(context)
    local time = time_of_day(context)
    local side = side_config(context, active_side(context))
    local balance = economy(context, active_side(context))
    local recruit_types = {}
    if side and side.recruit then
        for id in string.gmatch(side.recruit, "[^,%s]+") do recruit_types[#recruit_types + 1] = id end
    end
    return {
        turn = context.state:get("turn"),
        active_side = active_side(context),
        finished = context.state:get("finished"),
        can_end_turn = can_end_turn,
        phase = context.state:get("phase"),
        objective = current_phase and current_phase.label or scenario(context).objective,
        gold = context.state:get("gold:" .. active_side(context)),
        recruit_types = recruit_types,
        villages = context.state:get("villages") or {},
        income = balance.village_income,
        base_income = balance.base_income,
        villages_owned = balance.villages_owned,
        village_support = balance.village_support,
        upkeep = balance.upkeep,
        support = balance.support,
        expenses = balance.expenses,
        gross_income = balance.gross_income,
        net_income = balance.net_income,
        pending_advancement = context.state:get("pending_advancement"),
        time_of_day = time.id,
        lawful_bonus = time.lawful_bonus,
    }
end

function combat.recruit(context, command)
    assert_playing(context)
    assert_no_pending_advancement(context)
    local side_id = active_side(context)
    local side
    for _, candidate in ipairs(children(scenario(context), "side")) do
        if candidate.id == side_id then side = candidate end
    end
    assert(side and side.controller == "human", "only a human side can recruit")

    local allowed = false
    for id in string.gmatch(side.recruit or "", "[^,%s]+") do
        if id == command.unit_type then allowed = true end
    end
    assert(allowed, "unit type is not recruitable: " .. tostring(command.unit_type))

    local leader
    for _, object in ipairs(context.objects:all()) do
        if object.side == side_id and object.is_leader == "yes" then leader = object end
    end
    assert(leader, "side has no leader")
    assert(context.map:get(leader.position) == "keep", "leader must stand on a keep")

    local destination
    local frontier = {leader.position}
    local visited = {[position_key(leader.position)] = true}
    while #frontier > 0 and not destination do
        local current = table.remove(frontier, 1)
        for _, position in ipairs(context.map:neighbors(current)) do
            local key = position_key(position)
            local terrain = context.map:get(position)
            if not visited[key] and (terrain == "castle" or terrain == "keep") then
                visited[key] = true
                frontier[#frontier + 1] = position
                if terrain == "castle" and not occupied(context, position) then
                    destination = { x = position.x, y = position.y }
                    break
                end
            end
        end
    end
    assert(destination, "no free castle cell")

    local templates = assert(context.state:get("unit_types"), "missing unit types")
    local object = assert(templates[command.unit_type], "unknown unit type")
    local gold = context.state:get("gold:" .. side_id)
    assert(gold >= object.cost, "not enough gold")
    local number = context.state:get("next_recruit_id")
    context.state:set("next_recruit_id", number + 1)
    object.id = side_id .. "_" .. command.unit_type .. "_" .. number
    object.type = command.unit_type
    object.side = side_id
    object.x = destination.x
    object.y = destination.y
    object.position = destination
    object.hitpoints = object.max_hitpoints
    object.movement_points = 0
    object.attacks_left = 0
    object.experience = 0
    context.objects:add(object)
    context.state:set("gold:" .. side_id, gold - object.cost)
    return {
        type = "unit_recruited",
        unit = object.id,
        unit_type = command.unit_type,
        position = destination,
        cost = object.cost,
        gold = gold - object.cost,
    }
end


function combat.advance(context, command)
    local pending = assert(context.state:get("pending_advancement"),
        "no advancement choice is pending")
    assert(command.unit == pending.unit, "wrong unit for advancement choice")
    local allowed = false
    for _, option in ipairs(pending.options) do
        if option == command.choice then allowed = true end
    end
    assert(allowed, "invalid advancement choice: " .. tostring(command.choice))
    local object = assert(context.objects:get(pending.unit), "advancing unit no longer exists")
    context.state:set("pending_advancement", nil)
    local advanced = apply_advancement(context, object, command.choice)
    for _, candidate in ipairs(context.objects:all()) do
        local next_event = advance_unit(context, candidate)
        if next_event then return { advanced, next_event } end
    end
    return advanced
end

function combat.resolve(context, command)
    assert_playing(context)
    assert_no_pending_advancement(context)
    local attacker = context.objects:get(command.attacker)
    local defender = context.objects:get(command.defender)
    assert(attacker and defender, "attack requires two existing units")
    assert(attacker.side == active_side(context), "attacker is not on the active side")
    assert(attacker.side ~= defender.side, "cannot attack an allied unit")
    assert(attacker.hitpoints > 0 and defender.hitpoints > 0, "defeated unit cannot fight")
    assert(not attacker.petrified, "petrified unit cannot attack")
    assert(not attacker.stunned, "stunned unit cannot attack")
    assert(not defender.petrified, "petrified unit cannot be attacked")
    assert(attacker.attacks_left > 0, "unit has no attacks left")
    assert(context.map:are_adjacent(attacker.position, defender.position),
        "attack requires adjacent units")

    local attack = find_attack(attacker, command.weapon)
    local retaliation = retaliation_for(context, attacker, defender, attack)
    local events = {}
    context.objects:set(attacker.id, "attacks_left", attacker.attacks_left - 1)
    context.objects:set(attacker.id, "movement_points", 0)
    context.objects:set(attacker.id, "resting", false)
    local berserk = has_special(attack, "berserk")
        or (retaliation and has_special(retaliation, "berserk"))
    local rounds = berserk and 30
        or math.max(attack.strikes, retaliation and retaliation.strikes or 0)
    local attacker_strikes = berserk and rounds or attack.strikes
    local defender_strikes = berserk and rounds or (retaliation and retaliation.strikes or 0)
    local retaliation_first = retaliation and has_special(retaliation, "first_strike")
        and not has_special(attack, "first_strike")

    for round = 1, rounds do
        if retaliation_first then
            if retaliation and round <= defender_strikes and attacker.hitpoints > 0 then
                strike(context, events, defender, attacker, retaliation, round, false)
            end
            if attacker.hitpoints == 0 or attacker.petrified then break end
            if round <= attacker_strikes and defender.hitpoints > 0 then
                strike(context, events, attacker, defender, attack, round, true)
            end
            if defender.hitpoints == 0 then break end
            if defender.petrified then break end
        else
            if round <= attacker_strikes and defender.hitpoints > 0 then
                strike(context, events, attacker, defender, attack, round, true)
            end
            if defender.hitpoints == 0 then break end
            if defender.petrified then break end
            if retaliation and round <= defender_strikes and attacker.hitpoints > 0 then
                strike(context, events, defender, attacker, retaliation, round, false)
            end
            if attacker.hitpoints == 0 then break end
            if attacker.petrified then break end
        end
    end

    local defeated_id = defender.hitpoints == 0 and defender.id
        or attacker.hitpoints == 0 and attacker.id
        or nil
    local experience = {}
    if defeated_id then
        local defeated = defeated_id == defender.id and defender or attacker
        local killer = defeated_id == defender.id and attacker or defender
        local kill_experience = scenario(context).kill_experience or 8
        local gained = defeated.level > 0 and kill_experience * defeated.level
            or math.floor(kill_experience / 2)
        killer.experience = (killer.experience or 0) + gained
        experience[killer.id] = gained
        context.objects:set(killer.id, "experience", killer.experience)
    else
        local combat_experience = scenario(context).combat_experience or 1
        for _, pair in ipairs({{attacker, defender}, {defender, attacker}}) do
            local unit, opponent = pair[1], pair[2]
            local gained = combat_experience * (opponent.level or 0)
            if gained > 0 then
                unit.experience = (unit.experience or 0) + gained
                experience[unit.id] = gained
                context.objects:set(unit.id, "experience", unit.experience)
            end
        end
    end
    local experience_events = {}
    for _, unit in ipairs({attacker, defender}) do
        if experience[unit.id] then
            experience_events[#experience_events + 1] = {
                unit = unit.id,
                gained = experience[unit.id],
                total = unit.experience,
                maximum = unit.max_experience,
            }
        end
    end
    local result = {
        type = "battle_resolved",
        attacker = attacker.id,
        defender = defender.id,
        strikes = events,
        defeated = defeated_id,
        experience = experience_events,
        berserk = berserk,
    }
    local advancements = {}
    for _, unit in ipairs({attacker, defender}) do
        if unit.hitpoints > 0 then
            local event = advance_unit(context, unit)
            if event then advancements[#advancements + 1] = event end
        end
    end
    if result.defeated then
        local defeated = assert(context.objects:get(result.defeated))
        local events = {
            result,
            {
                type = "unit_died",
                unit = defeated.id,
                side = defeated.side,
                position = defeated.position,
            },
        }
        context.objects:remove(defeated.id)
        for _, event in ipairs(advancements) do events[#events + 1] = event end
        local ending = objective_event(context, defeated.id)
        local current = phase(context)
        if not ending and current then
            if current.when == "unit_defeated" and current.unit == defeated.id then
                if current.result then
                    ending = finished(context, current.result, current.finish_dialog or current.dialog)
                elseif current.next then
                    ending = enter_phase(context, current.next)
                end
            elseif current.advance_when_side_defeated
                and side_defeated(context, current.advance_when_side_defeated) then
                ending = enter_phase(context, current.next)
            end
        end
        if ending then events[#events + 1] = ending end
        return events
    end
    if #advancements > 0 then
        local results = { result }
        for _, event in ipairs(advancements) do results[#results + 1] = event end
        return results
    end
    return result
end

local function in_enemy_zoc(context, object, position)
    for _, enemy in ipairs(context.objects:all()) do
        if enemy.side ~= object.side and enemy.hitpoints > 0
            and not enemy.petrified and not enemy.stunned
            and (enemy.level or 0) > 0
            and context.map:are_adjacent(enemy.position, position) then
            return true
        end
    end
    return false
end

function combat.reachable(context, command)
    assert_playing(context)
    if context.state:get("pending_advancement") then return {} end
    local object = context.objects:get(command.object)
    assert(object, "reachable requires an existing object")
    assert(object.hitpoints > 0, "defeated object cannot move")
    assert(object.side == active_side(context), "object is not on the active side")
    if object.petrified or object.stunned then return {} end

    local occupied = {}
    for _, other in ipairs(context.objects:all()) do
        if other.id ~= object.id then
            occupied[position_key(other.position)] = other
        end
    end

    local start_key = position_key(object.position)
    local best = { [start_key] = 0 }
    local finalized = {}
    local frontier = {{ position = object.position, cost = 0, path = {} }}
    local result = {}

    while #frontier > 0 do
        local best_index = 1
        for index = 2, #frontier do
            if frontier[index].cost < frontier[best_index].cost then
                best_index = index
            end
        end
        local current = table.remove(frontier, best_index)
        local current_key = position_key(current.position)
        if not finalized[current_key] then
            finalized[current_key] = true
            current.zoc = not ability(object, "skirmisher") and current_key ~= start_key
                and in_enemy_zoc(context, object, current.position)
            if current_key ~= start_key and not occupied[current_key] then
                result[#result + 1] = current
            end

            if not current.zoc then
                for _, destination in ipairs(context.map:neighbors(current.position)) do
                    local key = position_key(destination)
                    local terrain = context.map:get(destination)
                    local step_cost = child(object, "movement_costs")[terrain]
                    local cost = step_cost and current.cost + step_cost
                    local blocker = occupied[key]
                    if cost and cost <= object.movement_points
                        and (not blocker or blocker.side == object.side)
                        and not finalized[key] and (best[key] == nil or cost < best[key]) then
                        local path = {}
                        for index, position in ipairs(current.path) do
                            path[index] = position
                        end
                        path[#path + 1] = destination
                        best[key] = cost
                        frontier[#frontier + 1] = {
                            position = destination,
                            cost = cost,
                            path = path,
                        }
                    end
                end
            end
        end
    end

    return result
end


function combat.end_turn(context)
    assert_playing(context)
    assert_no_pending_advancement(context)
    local sides = children(scenario(context), "side")
    local current = active_side(context)
    local current_index
    for index, side in ipairs(sides) do
        if side.id == current then current_index = index end
    end
    assert(current_index, "active side is not defined by the scenario")
    assert(sides[current_index].controller == "human", "only a human side can end its turn")

    local events = {{ type = "turn_ended", turn = context.state:get("turn"), side = current }}
    for _, event in ipairs(finish_side(context, current)) do events[#events + 1] = event end
    for offset = 1, #sides - 1 do
        local side = sides[(current_index - 1 + offset) % #sides + 1]
        context.state:set("active_side", side.id)
        events[#events + 1] = { type = "turn_started", turn = context.state:get("turn"), side = side.id }
        for _, event in ipairs(start_side(context, side.id)) do events[#events + 1] = event end
        if side.controller == "ai" then
            for _, event in ipairs(simple_ai_turn(combat, context, side.id)) do
                events[#events + 1] = event
                if event.type == "scenario_finished" then return events end
            end
            for _, event in ipairs(finish_side(context, side.id)) do events[#events + 1] = event end
        end
    end

    local turn = context.state:get("turn") + 1
    context.state:set("turn", turn)
    context.state:set("active_side", current)
    events[#events + 1] = { type = "turn_started", turn = turn, side = current }
    for _, event in ipairs(start_side(context, current)) do events[#events + 1] = event end
    local ending = objective_event(context)
    if ending then
        events[#events + 1] = ending
        return events
    end
    local current_phase = phase(context)
    if current_phase and current_phase.when == "turn_reached" then
        local holder = current_phase.unit and context.objects:get(current_phase.unit)
        local holding = not current_phase.unit or (holder
            and holder.position.x == current_phase.x and holder.position.y == current_phase.y)
        if not holding then
            context.state:set("phase_started_turn", turn)
        elseif turn >= context.state:get("phase_started_turn") + current_phase.turns then
            events[#events + 1] = enter_phase(context, current_phase.next)
        end
    end
    return events
end

function combat.move(context, command)
    assert_no_pending_advancement(context)
    local object = context.objects:get(command.object)
    local destination
    for _, cell in ipairs(combat.reachable(context, command)) do
        if cell.position.x == command.destination.x
            and cell.position.y == command.destination.y then
            destination = cell
            break
        end
    end
    assert(destination, "move destination is unreachable")

    local from = object.position
    context.objects:set(object.id, "position", command.destination)
    context.objects:set(object.id, "resting", false)
    local movement_points = destination.zoc and 0
        or object.movement_points - destination.cost
    context.objects:set(object.id, "movement_points", movement_points)
    local capture
    local villages = context.state:get("villages") or {}
    local village
    for _, candidate in ipairs(villages) do
        if candidate.x == command.destination.x and candidate.y == command.destination.y then
            village = candidate
        end
    end
    if village and village.side ~= object.side then
        village.side = object.side
        context.state:set("villages", villages)
        capture = {
            type = "village_captured",
            object = object.id,
            side = object.side,
            position = command.destination,
        }
    end
    local event = {
        type = "object_moved",
        object = object.id,
        from = from,
        to = command.destination,
        path = destination.path,
        cost = destination.cost,
        movement_points = movement_points,
        stopped_by_zoc = destination.zoc,
    }
    local current = phase(context)
    if current and current.when == "unit_reaches" and current.unit == object.id
        and current.x == command.destination.x and current.y == command.destination.y then
        local events = { event }
        if capture then events[#events + 1] = capture end
        events[#events + 1] = enter_phase(context, current.next)
        return events
    end
    if capture then return { event, capture } end
    return event
end

function combat.actions(context, command)
    assert_playing(context)
    local object = context.objects:get(command.object)
    assert(object, "actions require an existing object")
    assert(object.side == active_side(context), "object is not on the active side")

    local attacks = {}
    local targets = {}
    if object.attacks_left > 0 and not object.petrified and not object.stunned then
        for _, attack in ipairs(children(object, "attack")) do
            attacks[#attacks + 1] = attack.id
        end
        if #attacks > 0 then
            for _, target in ipairs(context.objects:all()) do
                if target.side ~= object.side
                    and context.map:are_adjacent(object.position, target.position) then
                    targets[#targets + 1] = target.id
                end
            end
        end
    end
    return {
        reachable = combat.reachable(context, command),
        targets = targets,
        attacks = attacks,
    }
end

return combat
