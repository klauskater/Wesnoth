local combat = {}
local simple_ai = require("rules.simple_ai")
local terrain = require("game.rules.terrain")

local function children(node, name)
    return node.__children and node.__children[name] or {}
end

local function child(node, name)
    return assert(children(node, name)[1], "missing [" .. name .. "]")
end

local function side_team(context, id)
    if context.combat_teams then return context.combat_teams[id] end
    local overridden = context.state:get("team:" .. id)
    if overridden then return overridden end
    local current = assert(context.state:get("scenario"), "missing scenario state")
    for _, side in ipairs(children(current, "side")) do
        if side.id == id then return side.team_name end
    end
end

local function allied(context, first, second)
    if first == second then return true end
    local team = side_team(context, first)
    return team ~= nil and team == side_team(context, second)
end

function combat.are_enemies(context, first, second)
    return not allied(context, first, second)
end

local function enabled(value)
    return value == true or value == "yes" or value == "true" or value == 1
end

local function weapon_special(attack, id)
    if not attack then return nil end
    for _, special in ipairs(children(attack, "special")) do
        local aliases = {drains = "drain", petrifies = "petrify", firststrike = "first_strike"}
        if special.id == id or (aliases[special.kind] or special.kind) == id then return special end
    end
end

local function has_special(attack, id) return weapon_special(attack, id) ~= nil end

local function ability(unit, id)
    for _, candidate in ipairs(children(unit, "ability")) do
        if candidate.id == id then return candidate end
    end
end

local function adjacent_ability(context, unit, id)
    local best
    for _, ally in ipairs(context.objects:all()) do
        local candidate = allied(context, ally.side, unit.side) and ally.id ~= unit.id
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

local function assert_no_pending_choice(context)
    assert(not context.state:get("pending_choice"), "scenario choice is required")
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

local function is_recruiter(object)
    return object.is_leader == "yes" or object.canrecruit == "yes"
end

local function list_contains(list, wanted)
    for id in string.gmatch(list or "", "[^,%s]+") do
        if id == wanted then return true end
    end
    return false
end

local function leader_can_recruit(side, leader, unit_type)
    return list_contains(side.recruit, unit_type)
        or list_contains(leader.extra_recruit, unit_type)
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
        if object.side == side_id and not is_recruiter(object)
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
            local village_owned = village and village.side
                and allied(context, village.side, side)
            local cure = village_owned or regeneration or curer
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
            local ability_healing = math.max(
                regeneration and regeneration.value or 0, healer and healer.value or 0,
                curer and curer.value or 0)
            local main_healing = math.max(village_owned and healing or 0, ability_healing)
            local turn_healing = object.poisoned and 0 or math.min(
                scenario(context).max_healing or 8, rest_healing + main_healing)
            if not object.unhealable and turn_healing > 0
                and object.hitpoints < object.max_hitpoints then
                local amount = math.min(turn_healing, object.max_hitpoints - object.hitpoints)
                context.objects:set(object.id, "hitpoints", object.hitpoints + amount)
                events[#events + 1] = {
                    type = "unit_healed", unit = object.id, amount = amount,
                    hitpoints = object.hitpoints + amount,
                    source = main_healing == 0 and "rest"
                        or regeneration and main_healing == regeneration.value and "regenerates"
                        or healer and main_healing == healer.value and "heals"
                        or curer and main_healing == curer.value and "cures" or "village",
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
    if context.state:get("pending_advancement") then
        context.state:set("pending_finish", { result = result, dialog = dialog })
        return
    end
    context.state:set("finished", true)
    context.state:set("result", result)
    local achievements = {}
    if result == "victory" then
        for _, achievement in ipairs(children(scenario(context), "achievement")) do
            if achievement.when == "victory"
                and (not achievement.unless_state
                    or not context.state:get(achievement.unless_state)) then
                local key = "campaign:achievement:" .. achievement.id
                if not context.state:get(key) then
                    context.state:set(key, true)
                    achievements[#achievements + 1] = achievement.id
                end
            end
        end
    end
    return {
        type = "scenario_finished",
        result = result,
        dialog = dialog,
        next_scenario = result == "victory" and scenario(context).next_scenario or nil,
        achievements = achievements,
    }
end

local spawn_group, require_choice

local function turn_event(context, turn)
    for _, event in ipairs(children(scenario(context), "turn_event")) do
        local key = "turn_event:" .. (event.id or tostring(event.turn))
        if event.turn == turn and not context.state:get(key)
            and not context.state:get("pending_choice") then
            context.state:set(key, true)
            local spawned = spawn_group(context, event.spawn_group)
            if event.choice then
                local required = require_choice(context, event.choice)
                required.turn = turn
                required.spawned = spawned
                return required
            end
            return { type = "turn_event", turn = turn, id = event.id,
                dialog = event.dialog, spawned = spawned }
        end
    end
end

local function side_defeated(context, side_id)
    local condition = side_config(context, side_id).defeat_condition or "no_units_left"
    if condition == "never" then return false end
    for _, object in ipairs(context.objects:all()) do
        if object.side == side_id
            and (condition ~= "no_leader_left" or is_recruiter(object)) then
            return false
        end
    end
    return true
end

local function enemies_defeated(context, side_id)
    local found = false
    for _, side in ipairs(children(scenario(context), "side")) do
        if not allied(context, side.id, side_id) then
            found = true
            if not side_defeated(context, side.id) then return false end
        end
    end
    return found
end

local function locations_controlled(context, objective)
    local locations = children(objective, "location")
    if #locations == 0 then return false end
    for _, location in ipairs(locations) do
        local village = village_at(context, location)
        local controlled = village and village.side
            and allied(context, village.side, objective.side)
        if not village then
            for _, object in ipairs(context.objects:all()) do
                if object.position.x == location.x and object.position.y == location.y
                    and allied(context, object.side, objective.side) then
                    controlled = true
                    break
                end
            end
        end
        if not controlled then return false end
    end
    return true
end

local function objective_event(context, defeated, killer)
    for _, objective in ipairs(children(scenario(context), "objective")) do
        local matched = false
        if objective.when == "side_defeated" then
            matched = side_defeated(context, objective.side)
        elseif objective.when == "enemies_defeated" then
            matched = enemies_defeated(context, objective.side)
        elseif objective.when == "locations_controlled" then
            matched = locations_controlled(context, objective)
        elseif objective.when == "unit_defeated" then
            matched = defeated == objective.unit
        elseif objective.when == "turn_limit" then
            matched = context.state:get("turn") > objective.turn
        end
        if matched then
            if killer and objective.killer_variable then
                context.state:set("campaign:" .. objective.killer_variable, killer)
            end
            for _, variable in ipairs(children(objective, "campaign_variable")) do
                local value = variable.value
                if variable.options then
                    local options = {}
                    for option in string.gmatch(variable.options, "[^,]+") do
                        options[#options + 1] = option:match("^%s*(.-)%s*$")
                    end
                    value = options[context.random:integer(1, #options)]
                end
                context.state:set("campaign:" .. variable.name, value)
            end
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

local time_of_day

local function visibility(context, side_id)
    local side = side_config(context, side_id)
    if side.fog ~= "yes" and side.shroud ~= "yes" then return nil end
    local radius = side.vision_radius or scenario(context).vision_radius or 2
    local visible, frontier = {}, {}
    for _, unit in ipairs(context.objects:all()) do
        if allied(context, unit.side, side_id) then
            local key = position_key(unit.position)
            if not visible[key] then
                visible[key] = 0
                frontier[#frontier + 1] = { position = unit.position, distance = 0 }
            end
        end
    end
    local index = 1
    while index <= #frontier do
        local current = frontier[index]
        index = index + 1
        if current.distance < radius then
            for _, position in ipairs(context.map:neighbors(current.position)) do
                local key = position_key(position)
                if visible[key] == nil then
                    visible[key] = current.distance + 1
                    frontier[#frontier + 1] = {
                        position = position, distance = current.distance + 1,
                    }
                end
            end
        end
    end
    local cells = {}
    for _, cell in ipairs(frontier) do cells[#cells + 1] = cell.position end
    return visible, cells
end

local function hidden_from(context, side_id, object)
    local terrain = context.map:get(object.position)
    local time = time_of_day(context)
    local hides = ability(object, "ambush") and terrain == "forest"
        or ability(object, "concealment") and terrain == "village"
        or ability(object, "submerge") and terrain == "water"
        or ability(object, "swamp_lurk") and terrain == "water"
        or ability(object, "burrow") and (terrain == "forest" or terrain == "grassland")
            and object.resting
        or ability(object, "nightstalk") and time.lawful_bonus < 0
    local configured = ability(object, "ambush")
    if configured and configured.terrain then hides = configured.terrain == terrain end
    local concealed = object.hidden == true or object.hidden == "yes"
        or hides
    if not concealed then return false end
    for _, viewer in ipairs(context.objects:all()) do
        if allied(context, viewer.side, side_id)
            and context.map:are_adjacent(viewer.position, object.position) then
            return false
        end
    end
    return true
end

local function visible_to(context, side_id, object)
    if allied(context, side_id, object.side) then return true end
    if hidden_from(context, side_id, object) then return false end
    local visible = visibility(context, side_id)
    return visible == nil or visible[position_key(object.position)] ~= nil
end

function combat.visible_unit_ids(context, side)
    local result = {}
    for _, unit in ipairs(context.objects:all()) do
        if visible_to(context, side, unit) then result[#result + 1] = unit.id end
    end
    return result
end

local function sighted_by_side(context, side_id, object)
    local radius = side_config(context, side_id).vision_radius
        or scenario(context).vision_radius or 2
    local frontier, visited = {}, {}
    for _, viewer in ipairs(context.objects:all()) do
        if viewer.side == side_id then
            local key = position_key(viewer.position)
            visited[key] = 0
            frontier[#frontier + 1] = { position = viewer.position, distance = 0 }
        end
    end
    local index = 1
    while index <= #frontier do
        local current = frontier[index]
        index = index + 1
        if current.position.x == object.position.x
            and current.position.y == object.position.y then return true end
        if current.distance < radius then
            for _, position in ipairs(context.map:neighbors(current.position)) do
                local key = position_key(position)
                if visited[key] == nil then
                    visited[key] = current.distance + 1
                    frontier[#frontier + 1] = {
                        position = position, distance = current.distance + 1,
                    }
                end
            end
        end
    end
    return false
end

function combat.can_see(context, side_id, object)
    return visible_to(context, side_id, object)
end

local function update_shroud(context)
    for _, side in ipairs(children(scenario(context), "side")) do
        if side.shroud == "yes" then
            local _, cells = visibility(context, side.id)
            local revealed = context.state:get("revealed:" .. side.id) or {}
            local known = {}
            for _, position in ipairs(revealed) do known[position_key(position)] = true end
            for _, position in ipairs(cells) do
                if not known[position_key(position)] then revealed[#revealed + 1] = position end
            end
            context.state:set("revealed:" .. side.id, revealed)
        end
    end
end

local function recruitment_hexes(context, leader)
    if context.map:get(leader.position) ~= "keep" then return {} end
    local result, frontier = {}, {leader.position}
    local visited = {[position_key(leader.position)] = true}
    while #frontier > 0 do
        local current = table.remove(frontier, 1)
        for _, position in ipairs(context.map:neighbors(current)) do
            local key = position_key(position)
            local terrain = context.map:get(position)
            if not visited[key] and (terrain == "castle" or terrain == "keep") then
                visited[key] = true
                frontier[#frontier + 1] = position
                if terrain == "castle" and not occupied(context, position) then
                    result[#result + 1] = { x = position.x, y = position.y }
                end
            end
        end
    end
    return result
end

local function matching_hex(hexes, wanted)
    if not wanted then return hexes[1] end
    for _, position in ipairs(hexes) do
        if position.x == wanted.x and position.y == wanted.y then return position end
    end
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

spawn_group = function(context, group)
    local spawned = {}
    if group then
        for _, reserve_id in ipairs(context.state:get("reserves") or {}) do
            local object = context.state:get("reserve:" .. reserve_id)
            if object and object.reserve_group == group then
                object.position = safe_spawn_position(context, object)
                object.x = object.position.x
                object.y = object.position.y
                context.objects:add(object)
                context.state:set("reserve:" .. reserve_id, nil)
                spawned[#spawned + 1] = reserve_id
            end
        end
    end
    return spawned
end

local function enter_phase(context, id)
    context.state:set("phase", id)
    context.state:set("phase_started_turn", context.state:get("turn"))
    local current = assert(phase(context), "unknown phase: " .. tostring(id))
    local spawned = spawn_group(context, current.spawn_group)
    update_shroud(context)
    return {
        type = "phase_changed",
        phase = id,
        dialog = current.dialog,
        spawned = spawned,
    }
end

local function scenario_choice(context, id)
    for _, choice in ipairs(children(scenario(context), "choice")) do
        if choice.id == id then return choice end
    end
    error("unknown scenario choice: " .. tostring(id))
end

require_choice = function(context, id)
    local choice = scenario_choice(context, id)
    local options = {}
    for _, option in ipairs(children(choice, "option")) do
        options[#options + 1] = option.id
    end
    local pending = { id = id, options = options }
    context.state:set("pending_choice", pending)
    return { type = "choice_required", choice = id, options = options,
        dialog = choice.dialog }
end

local function location_event(context, object)
    for _, trigger in ipairs(children(scenario(context), "location_event")) do
        local key = "location_event:" .. trigger.id
        if not context.state:get(key) and (not trigger.side or trigger.side == object.side)
            and trigger.x == object.position.x and trigger.y == object.position.y then
            context.state:set(key, true)
            local event = { type = "location_event", id = trigger.id,
                object = object.id, dialog = trigger.dialog }
            if trigger.gold then
                local gold_key = "gold:" .. object.side
                local gold = context.state:get(gold_key) + trigger.gold
                context.state:set(gold_key, gold)
                event.gold = gold
                event.amount = trigger.gold
            end
            if trigger.damage then
                local damage = math.min(trigger.damage, object.hitpoints - 1)
                context.objects:set(object.id, "hitpoints", object.hitpoints - damage)
                event.damage = damage
                event.hitpoints = object.hitpoints - damage
            end
            if trigger.status then
                context.objects:set(object.id, trigger.status, true)
                event.status = trigger.status
            end
            if trigger.achievement then
                context.state:set("campaign:achievement:" .. trigger.achievement, true)
                event.achievement = trigger.achievement
            end
            return event
        end
    end
end

local function attack_event(context, attacker, defender, defeated)
    for _, trigger in ipairs(children(scenario(context), "attack_event")) do
        local key = "attack_event:" .. (trigger.group or trigger.id)
        if not context.state:get(key)
            and (not trigger.attacker or trigger.attacker == attacker.id)
            and (not trigger.defender or trigger.defender == defender.id) then
            context.state:set(key, true)
            local event = { type = "attack_event", id = trigger.id,
                attacker = attacker.id, defender = defender.id,
                dialog = defeated and trigger.kill_dialog or trigger.dialog }
            if trigger.achievement and defeated == defender.id then
                context.state:set("campaign:achievement:" .. trigger.achievement, true)
                event.achievement = trigger.achievement
            end
            return event
        end
    end
end

local function sight_event(context, viewer)
    for _, trigger in ipairs(children(scenario(context), "sight_event")) do
        local key = "sight_event:" .. trigger.id
        if not context.state:get(key)
            and (not trigger.viewer_side or trigger.viewer_side == viewer.side) then
            local target = trigger.target and context.objects:get(trigger.target)
            if not target and trigger.target_side then
                for _, candidate in ipairs(context.objects:all()) do
                    if candidate.side == trigger.target_side
                        and sighted_by_side(context, viewer.side, candidate) then
                        target = candidate
                        break
                    end
                end
            end
            if target and sighted_by_side(context, viewer.side, target) then
                context.state:set(key, true)
                if trigger.set_team_side and trigger.set_team_name then
                    context.state:set("team:" .. trigger.set_team_side, trigger.set_team_name)
                end
                if trigger.ally_side and trigger.ally_team_name then
                    context.state:set("team:" .. trigger.ally_side, trigger.ally_team_name)
                end
                local spawned = spawn_group(context, trigger.spawn_group)
                return { type = "sight_event", id = trigger.id,
                    viewer = viewer.id, target = target.id,
                    dialog = trigger.dialog, spawned = spawned }
            end
        end
    end
end

local function kill_achievement(context, defeated, killer)
    for _, achievement in ipairs(children(scenario(context), "achievement")) do
        if achievement.when == "unit_defeated" and achievement.unit == defeated.id
            and (not achievement.killer or achievement.killer == killer.id) then
            local key = "campaign:achievement:" .. achievement.id
            if not context.state:get(key) then
                context.state:set(key, true)
                return { type = "achievement_unlocked", id = achievement.id }
            end
        end
    end
end

local function advancement_options(object)
    local result = {}
    for id in string.gmatch(object.advances_to or "", "[^,%s]+") do
        if id ~= "null" then result[#result + 1] = id end
    end
    return result
end

local apply_traits

local type_properties = {
    id = true, type = true, name = true, max_hitpoints = true, max_moves = true,
    cost = true, level = true, max_experience = true, advances_to = true,
    alignment = true, race = true, image = true, profile = true, usage = true,
    movement_type = true, num_traits = true, gender = true, zoc = true,
    traits = true, traits_applied = true, __children = true,
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
    advanced.traits = object.traits
    advanced.traits_applied = nil
    apply_traits(context, advanced)
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

time_of_day = function(context)
    return times[(context.state:get("turn") - 1) % #times + 1]
end

local function alignment_modifier(alignment, lawful_bonus)
    if alignment == "lawful" then return lawful_bonus end
    if alignment == "chaotic" then return -lawful_bonus end
    if alignment == "liminal" then return -math.abs(lawful_bonus) end
    return 0
end

local function leadership_bonus(context, source)
    local bonus = 0
    for _, leader in ipairs((context.combat_units or context.objects:all())) do
        if allied(context, leader.side, source.side) and leader.id ~= source.id
            and leader.level > source.level
            and context.map:are_adjacent(leader.position, source.position)
            and ability(leader, "leadership") then
            bonus = math.max(bonus, (leader.level - source.level) * 25)
        end
    end
    return bonus
end

local function illuminated_lawful_bonus(context, source, base)
    local increase = 0
    for _, unit in ipairs((context.combat_units or context.objects:all())) do
        local light = ability(unit, "illuminates")
        if light and (unit.id == source.id
            or context.map:are_adjacent(unit.position, source.position)) then
            increase = math.max(increase, light.value or 25)
        end
    end
    return math.min(25, base + increase)
end

local function opposite_hex(target, source)
    local function axial(position)
        local q = position.x - 1
        return q, position.y - 1 - math.floor((q + q % 2) / 2)
    end
    local tq, tr = axial(target)
    local sq, sr = axial(source)
    local q, r = 2 * tq - sq, 2 * tr - sr
    return { x = q + 1, y = r + math.floor((q + q % 2) / 2) + 1 }
end

local function backstab_active(context, source, target, attack, attacking)
    if not attacking or not has_special(attack, "backstab") then return false end
    local opposite = opposite_hex(target.position, source.position)
    for _, flanker in ipairs((context.combat_units or context.objects:all())) do
        if flanker.position.x == opposite.x and flanker.position.y == opposite.y
            and not allied(context, flanker.side, target.side)
            and not flanker.petrified and not flanker.stunned then
            return true
        end
    end
    return false
end

-- Numeric special attributes are interpreted by kind, not by the display id.
local legacy_specials = {
    magical={kind="chance_to_hit",value=70},
    marksman={kind="chance_to_hit",value=60,cumulative="yes",active_on="offense"},
    charge={kind="damage",multiply=2,apply_to="both",active_on="offense"},
    backstab={kind="damage",multiply=2,active_on="offense"},
    absorb={kind="damage",apply_to="opponent"},
    deflect={kind="chance_to_hit",apply_to="opponent"},
    drain={kind="drains",value=50}, first_strike={kind="firststrike"},
    petrify={kind="petrifies"},
}
local function special_attributes(original)
    local result = {}
    for key, value in pairs(not original.kind and legacy_specials[original.id] or {}) do result[key]=value end
    for key, value in pairs(original) do result[key]=value end
    result.kind = result.kind or result.id
    return result
end

local function listed(value, choices)
    for item in tostring(choices):gmatch("[^,]+") do
        item = item:match("^%s*(.-)%s*$")
        if tostring(value) == item then return true end
        local lo, hi = item:match("^(%-?%d+)%-(%-?%d+)$")
        if lo and tonumber(value) and tonumber(value) >= tonumber(lo) and tonumber(value) <= tonumber(hi) then
            return true
        end
    end
    return false
end

local function weapon_filter(weapon, filter)
    if not weapon then return false end
    local aliases = {name="id",type="damage_type",base_type="damage_type",number="strikes"}
    local result = true
    for key, value in pairs(filter) do
        if key ~= "__children" then
            assert(key ~= "formula", "weapon filter formula requires WFL evaluation")
            if key == "special" or key == "special_id" or key == "special_type" then
                local found=false
                for _, special in ipairs(children(weapon,"special")) do
                    found=found or listed(key == "special_type" and special_attributes(special).kind or special.id,value)
                end
                result=result and found
            else
                result=result and listed(weapon[aliases[key] or key],value)
            end
        end
    end
    for _, f in ipairs(children(filter,"and")) do result=result and weapon_filter(weapon,f) end
    for _, f in ipairs(children(filter,"or")) do result=result or weapon_filter(weapon,f) end
    for _, f in ipairs(children(filter,"not")) do result=result and not weapon_filter(weapon,f) end
    return result
end

local function unit_filter(context, unit, weapon, filter)
    if not unit then return false end
    local result=true
    for key,value in pairs(filter) do
        if key ~= "__children" then
            assert(key ~= "formula" and key ~= "lua_function", "unit filter requires a formula/script evaluator")
            if key == "status" then
                local found=false
                for status in tostring(value):gmatch("[^,%s]+") do found=found or enabled(unit[status]) end
                result=result and found
            elseif key == "canrecruit" then
                result=result and (is_recruiter(unit) == enabled(value))
            elseif key == "x" or key == "y" then
                result=result and listed(unit.position[key],value)
            else
                result=result and listed(unit[key],value)
            end
        end
    end
    for name, filters in pairs(filter.__children or {}) do
        for _, f in ipairs(filters) do
            if name == "filter_weapon" then result=result and weapon_filter(weapon,f)
            elseif name ~= "and" and name ~= "or" and name ~= "not" then
                error("unit filter child is not implemented: " .. name)
            end
        end
    end
    for _, f in ipairs(children(filter,"and")) do result=result and unit_filter(context,unit,weapon,f) end
    for _, f in ipairs(children(filter,"or")) do result=result or unit_filter(context,unit,weapon,f) end
    for _, f in ipairs(children(filter,"not")) do result=result and not unit_filter(context,unit,weapon,f) end
    return result
end

local function matching_specials(context, source, target, weapon, opponent, kind, attacking)
    local result = {}
    local function collect(owner_weapon, own)
        if not owner_weapon then return end
        for _, original in ipairs(children(owner_weapon, "special")) do
            local special = special_attributes(original)
            local owner_attacks = own and attacking or not own and not attacking
            local active = special.active_on or "both"
            local applies = special.apply_to or "self"
            local affects = applies == "both" or (applies == "self" and own)
                or (applies == "opponent" and not own)
                or (applies == "attacker" and attacking) or (applies == "defender" and not attacking)
            if special.kind == kind and affects
                and (active == "both" or active == (owner_attacks and "offense" or "defense"))
                and (special.id ~= "backstab" or backstab_active(context,
                    own and source or target, own and target or source, owner_weapon, owner_attacks)) then
                local matches=true
                local other_weapon=weapon
                if own then other_weapon=opponent end
                for _, entry in ipairs({
                    {"filter_self",own and source or target,owner_weapon},
                    {"filter_opponent",own and target or source,other_weapon},
                    {"filter_attacker",attacking and source or target,attacking and weapon or opponent},
                    {"filter_defender",attacking and target or source,attacking and opponent or weapon},
                }) do
                    -- The standard backstab formula is implemented geometrically above.
                    if not (special.id == "backstab" and entry[1] == "filter_opponent") then
                        for _, filter in ipairs(children(special,entry[1])) do
                            matches=matches and unit_filter(context,entry[2],entry[3],filter)
                        end
                    end
                end
                if matches then result[#result+1] = special end
            end
        end
    end
    collect(weapon, true)
    collect(opponent, false)
    return result
end

local function rounded(value)
    return value < 0 and math.ceil(value - 0.5) or math.floor(value + 0.5)
end

local function composite_value(effects, base, integer)
    local groups, priorities = {}, {}
    for _, effect in ipairs(effects) do
        local priority = tonumber(effect.priority) or 0
        if not groups[priority] then groups[priority]={}; priorities[#priorities+1]=priority end
        groups[priority][#groups[priority]+1]=effect
    end
    table.sort(priorities)
    for index, priority in ipairs(priorities) do
        local set_min, set_max, lower, upper
        local add, sub, mul, div = {}, {}, {}, {}
        for _, effect in ipairs(groups[priority]) do
            local filter = children(effect, "filter_base_value")[1] or {}
            local matches = (not filter.equals or base == filter.equals)
                and (not filter.not_equals or base ~= filter.not_equals)
                and (not filter.less_than or base < filter.less_than)
                and (not filter.greater_than or base > filter.greater_than)
                and (not filter.less_than_equal_to or base <= filter.less_than_equal_to)
                and (not filter.greater_than_equal_to or base >= filter.greater_than_equal_to)
            if matches then
                local id = effect.id or effect.name or ""
                local function number(key)
                    if effect[key] == nil then return nil end
                    return assert(tonumber(effect[key]), "formula evaluation required for special " .. id .. "." .. key)
                end
                local value = number("value")
                if value then
                    if enabled(effect.cumulative) then value=math.max(base,value) end
                    set_min=math.min(set_min or value,value)
                    set_max=math.max(set_max or value,value)
                end
                local lo, hi = number("min_value"), number("max_value")
                if lo then lower=math.max(lower or lo,lo) end
                if hi then upper=math.min(upper or hi,hi) end
                local a,b,m,d = number("add"),number("sub"),number("multiply"),number("divide")
                if a then add[id]=math.max(add[id] or a,a) end
                if b then sub[id]=math.max(sub[id] or b,b) end
                if m then mul[id]=math.max(mul[id] or m,m) end
                if d and d ~= 0 then div[id]=math.max(div[id] or d,d) end
            end
        end
        local value = set_max and (math.max(set_max,0)+math.min(set_min,0)) or base
        for _, n in pairs(add) do value=value+n end
        for _, n in pairs(sub) do value=value-n end
        for _, n in pairs(mul) do value=value*(math.floor(n*100)/100) end
        for _, n in pairs(div) do value=value/(math.floor(n*100)/100) end
        if lower then value=math.max(lower,value) end
        if upper then value=math.min(upper,value) end
        base = (integer or index < #priorities) and rounded(value) or value
    end
    return base
end

-- Same tie-breaking/minimum as upstream utils::round_damage.
local function round_damage(base, bonus, divisor)
    if base == 0 then return 0 end
    local rounding = math.floor(divisor / 2) - ((bonus <= divisor or divisor == 1) and 0 or 1)
    return math.max(1, math.floor((base * bonus + rounding) / divisor))
end

local function effective_strikes(context, unit, target, attack, opponent, attacking)
    local strikes = math.max(0, composite_value(matching_specials(context,unit,target,
        attack,opponent,"attacks",attacking),attack.strikes,true))
    local swarm = matching_specials(context,unit,target,attack,opponent,"swarm",attacking)
    if #swarm == 0 then return strikes end
    local minimum,maximum=0,0
    for _, effect in ipairs(swarm) do
        minimum=math.max(minimum,tonumber(effect.swarm_attacks_min) or 0)
        maximum=math.max(maximum,tonumber(effect.swarm_attacks_max) or strikes)
    end
    return minimum + math.floor((maximum - minimum)
        * math.min(unit.hitpoints, unit.max_hitpoints) / unit.max_hitpoints)
end

function combat.weapon_reaches(weapon, from, to)
    local function axial(p)
        local q=p.x-1
        return q,p.y-1-math.floor((q+q%2)/2)
    end
    local aq,ar=axial(from)
    local bq,br=axial(to)
    local distance=(math.abs(aq-bq)+math.abs(ar-br)+math.abs(aq+ar-bq-br))/2
    return distance >= (weapon.min_range or 1) and distance <= (weapon.max_range or 1)
end

local function disabled_weapon(context, source, target, weapon, opponent, attacking)
    return not combat.weapon_reaches(weapon,source.position,target.position)
        or #matching_specials(context,source,target,weapon,opponent,"disable",attacking) > 0
end

local function attack_stats(context, source, target, attack, attacking, slowed, opponent_weapon)
    local terrain = context.map:get(target.position)
    local defense = assert(child(target, "defense")[terrain],
        "unit has no defense value for terrain: " .. terrain)
    local chance = math.max(0, math.min(100, 100 - defense + (attack.accuracy or 0)
        - (opponent_weapon and opponent_weapon.parry or 0)))
    chance = math.max(0, math.min(100, composite_value(matching_specials(context,
        source,target,attack,opponent_weapon,"chance_to_hit",attacking),chance,true)))
    if enabled(target.invulnerable) then chance = 0 end
    local damage_type = assert(attack.damage_type, "attack has no damage type")
    local replacements, alternatives = {}, {}
    for _, effect in ipairs(matching_specials(context,source,target,attack,opponent_weapon,"damage_type",attacking)) do
        if effect.replacement_type then
            replacements[effect.replacement_type]=(replacements[effect.replacement_type] or 0)+1
        end
        if effect.alternative_type then alternatives[effect.alternative_type]=true end
    end
    local count=0
    for kind, n in pairs(replacements) do
        if n > count or (n == count and kind < damage_type) then damage_type,count=kind,n end
    end
    local resistances=child(target,"resistance")
    local alternative_names={}
    for kind in pairs(alternatives) do alternative_names[#alternative_names+1]=kind end
    table.sort(alternative_names)
    for _, kind in ipairs(alternative_names) do
        if (resistances[kind] or 0) < (resistances[damage_type] or 0) then damage_type=kind end
    end
    local resistance = assert(child(target, "resistance")[damage_type],
        "unit has no resistance for damage type: " .. damage_type)
    local time = time_of_day(context)
    local alignment = attack.alignment or source.alignment or "neutral"
    local lawful_bonus = illuminated_lawful_bonus(context, source, time.lawful_bonus)
    local alignment_bonus = alignment_modifier(alignment, lawful_bonus)
    if source.fearless and alignment_bonus < 0 then alignment_bonus = 0 end
    local leadership = leadership_bonus(context, source)
    local is_slowed = slowed == nil and source.slowed or slowed
    local effective_resistance = resistance
    if attacking and resistance > 0 and ability(target, "steadfast") then
        effective_resistance = math.min(50, resistance * 2)
    end
    local base_damage = composite_value(matching_specials(context,
        source,target,attack,opponent_weapon,"damage",attacking),attack.damage,false)
    local modified_damage = round_damage(base_damage,
        (100 + alignment_bonus + leadership) * (100 - effective_resistance),
        is_slowed and 20000 or 10000)
    return chance, modified_damage, damage_type, resistance, effective_resistance,
        time, alignment, alignment_bonus, leadership
end

local function retaliation_for(context, attacker, defender, attack)
    local best, best_rating
    for _, candidate in ipairs(children(defender, "attack")) do
        if candidate.range == attack.range and (tonumber(candidate.defense_weight) or 1) > 0
            and not disabled_weapon(context,defender,attacker,candidate,attack,false) then
            local chance, damage = attack_stats(context, defender, attacker, candidate, false, nil, attack)
            local strikes = effective_strikes(context,defender,attacker,candidate,attack,false)
            local rating = strikes * damage * chance * (tonumber(candidate.defense_weight) or 1)
            if not best_rating or rating > best_rating then
                best, best_rating = candidate, rating
            end
        end
    end
    return best
end


local function prepare_battle(context, command, attacker, defender)
    attacker = attacker or assert(context.objects:get(command.attacker), "unknown attacker")
    defender = defender or assert(context.objects:get(command.defender), "unknown defender")
    if command.position then attacker.position = command.position end
    context.combat_units = context.objects:all(
        {"side", "level", "position", "petrified", "stunned"}, {"ability"})
    context.combat_teams = {}
    local current = context.state:get("scenario", {}, {"side"})
    for _, side in ipairs(children(current, "side")) do
        context.combat_teams[side.id] = context.state:get("team:" .. side.id) or side.team_name
    end
    local attack = find_attack(attacker, command.weapon)
    local retaliation = retaliation_for(context, attacker, defender, attack)
    local chance, damage = attack_stats(context, attacker, defender, attack, true, nil, retaliation)
    local _, slowed_damage = attack_stats(context, attacker, defender, attack, true, true, retaliation)
    local retaliation_chance, retaliation_damage, slowed_retaliation_damage,
        retaliation_strikes = 0, 0, 0, 0
    if retaliation then
        retaliation_chance, retaliation_damage =
            attack_stats(context, defender, attacker, retaliation, false, nil, attack)
        _, slowed_retaliation_damage =
            attack_stats(context, defender, attacker, retaliation, false, true, attack)
        retaliation_strikes = effective_strikes(context,defender,attacker,retaliation,attack,false)
    end
    local function effects(own, kind)
        if own then return matching_specials(context,attacker,defender,attack,retaliation,kind,true) end
        return matching_specials(context,defender,attacker,retaliation,attack,kind,false)
    end
    local rounds=1
    for _, own in ipairs({true,false}) do
        for _, effect in ipairs(effects(own,"berserk")) do
            rounds=math.max(rounds,tonumber(effect.value) or 30)
        end
    end
    local strikes = effective_strikes(context,attacker,defender,attack,retaliation,true)
    local retaliation_first = retaliation and #effects(false,"firststrike") > 0
        and #effects(true,"firststrike") == 0
    local function participant(unit, target, own, chance, damage, slowed_damage, strikes)
        local drains=effects(own,"drains")
        return { hp = unit.hitpoints, maximum = unit.max_hitpoints,
            slowed = unit.slowed == true, chance = chance, damage = damage,
            slowed_damage = slowed_damage, strikes = strikes,
            slow = #effects(own,"slow") > 0 and not enabled(target.unslowable),
            poison = #effects(own,"poison") > 0 and not enabled(target.unpoisonable),
            heal_percent = #drains > 0 and not enabled(target.undrainable)
                and composite_value(drains,50,true) or 0,
            heal_constant = composite_value(effects(own,"heal_on_hit"),0,true),
            minimum_source_hp = 1,
            petrify = #effects(own,"petrifies") > 0 and not enabled(target.unpetrifiable),
            plague = effects(own,"plague")[1],
        }
    end
    local attack_metadata = {attack_stats(context, attacker, defender, attack, true, nil, retaliation)}
    local defense_metadata = retaliation and {attack_stats(context, defender, attacker, retaliation, false, nil, attack)} or {}
    local model = {
        attacker = participant(attacker, defender, true, chance, damage, slowed_damage, strikes),
        defender = participant(defender, attacker, false, retaliation_chance,
            retaliation_damage, slowed_retaliation_damage, retaliation_strikes),
        retaliation_first = retaliation_first == true,
        berserk = rounds > 1,
        sequence = {},
    }
    for _ = 1, rounds do
        for strike = 1, math.max(strikes, retaliation_strikes) do
            for _, side in ipairs(retaliation_first and {2, 1} or {1, 2}) do
                if strike <= (side == 1 and strikes or retaliation_strikes) then
                    model.sequence[#model.sequence + 1] = side
                end
            end
        end
    end
    model.attacker.metadata = attack_metadata
    model.defender.metadata = defense_metadata
    local time = time_of_day(context)
    local next_time = times[context.state:get("turn") % #times + 1]
    context.combat_units, context.combat_teams = nil, nil
    return model, attack, retaliation, {
        disabled = disabled_weapon(context,attacker,defender,attack,retaliation,true),
        chance = chance,
        damage = damage,
        strikes = strikes,
        base_damage = attack.damage,
        retaliation_base_damage = retaliation and retaliation.damage or 0,
        resistance_modifier = attack_metadata[5],
        leadership_modifier = attack_metadata[9],
        retaliation_alignment_modifier = defense_metadata[8] or 0,
        retaliation_resistance_modifier = defense_metadata[5] or 0,
        retaliation_leadership_modifier = defense_metadata[9] or 0,
        weapon_name = attack.name or attack.id,
        damage_type = attack_metadata[3],
        range = attack.range,
        retaliation_chance = retaliation_chance,
        retaliation_damage = retaliation_damage,
        retaliation_strikes = retaliation_strikes,
        retaliation_name = retaliation and (retaliation.name or retaliation.id) or "—",
        time_of_day = time.id,
        alignment_modifier = attack_metadata[8],
        next_alignment_modifier = alignment_modifier(attacker.alignment,
            next_time.lawful_bonus),
    }
end

function combat.preview_attack(context, command)
    local model, _, _, result = prepare_battle(context, command)
    -- Forecast consumes only prepared scalar inputs and never touches game RNG.
    model.attacker.metadata, model.defender.metadata = nil, nil
    model.attacker.plague, model.defender.plague = nil, nil
    local forecast = context.combat:forecast(model)
    result.attacker_outcomes, result.defender_outcomes = forecast.attacker, forecast.defender
    result.expected_damage = model.defender.hp - forecast.defender.expected
    result.expected_retaliation = model.attacker.hp - forecast.attacker.expected
    result.kill_probability, result.death_probability = forecast.defender.death, forecast.attacker.death
    return result
end

local function strike(context, events, source, target, attack, number, attacking, prepared)
    local _, _, damage_type, resistance, effective_resistance,
        time, alignment, alignment_bonus, leadership = table.unpack(prepared.metadata)
    local chance = prepared.chance
    local modified_damage = source.slowed and prepared.slowed_damage or prepared.damage
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
        local source_after
        source_after, target.hitpoints = context.combat:impact(source.hitpoints, target.hitpoints,
            source.max_hitpoints, modified_damage, prepared.heal_percent, prepared.heal_constant, prepared.minimum_source_hp)
        damage = before - target.hitpoints
        context.objects:set(target.id, "hitpoints", target.hitpoints)
        if prepared.poison and target.hitpoints > 0 and not target.poisoned then
            target.poisoned = true
            context.objects:set(target.id, "poisoned", true)
            applied_poison = true
        end
        if prepared.slow and target.hitpoints > 0 and not target.slowed then
            target.slowed = true
            context.objects:set(target.id, "slowed", true)
            applied_slow = true
        end
        if prepared.petrify and target.hitpoints > 0 and not target.petrified then
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
        drained = source_after - source.hitpoints
        if drained ~= 0 then
            source.hitpoints = source_after
            context.objects:set(source.id, "hitpoints", source.hitpoints)
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
        source_hitpoints = source.hitpoints,
        range = attack.range,
        source_experience = source.experience or 0,
    }
end

local function increased(value, amount, multiplier)
    if amount == nil then return value end
    multiplier = multiplier or 1
    if type(amount) == "number" then return value + amount * multiplier end
    local percent = string.match(amount, "^([+-]?%d+)%%$")
    if percent then return math.floor(value * (100 + tonumber(percent)) / 100) end
    return value + assert(tonumber(amount), "invalid trait increase: " .. tostring(amount))
        * multiplier
end

local function trait_matches_attack(effect, attack)
    return (not effect.range or effect.range == attack.range)
        and (not effect.damage_type or effect.damage_type == attack.damage_type)
end

local function apply_trait_effect(object, effect)
    local multiplier = effect.times == "per level" and object.level or 1
    if effect.apply_to == "hitpoints" then
        object.max_hitpoints = math.max(1,
            increased(object.max_hitpoints, effect.increase_total, multiplier))
    elseif effect.apply_to == "movement" then
        object.max_moves = math.max(0, increased(object.max_moves, effect.increase, multiplier))
    elseif effect.apply_to == "max_experience" then
        object.max_experience = math.max(1,
            increased(object.max_experience, effect.increase, multiplier))
    elseif effect.apply_to == "attack" then
        for _, attack in ipairs(children(object, "attack")) do
            if trait_matches_attack(effect, attack) then
                attack.damage = math.max(0,
                    increased(attack.damage, effect.increase_damage, multiplier))
                attack.strikes = math.max(0,
                    increased(attack.strikes, effect.increase_attacks, multiplier))
            end
        end
    elseif effect.apply_to == "status" and effect.add then
        object[effect.add] = true
    elseif effect.apply_to == "healthy" or effect.apply_to == "fearless"
        or effect.apply_to == "loyal" then
        object[effect.apply_to] = true
    end
end

apply_traits = function(context, object)
    if object.traits_applied then return end
    local definitions = {}
    local mandatory = {}
    local candidates = {}
    local function add(definition, required)
        if not definition or not definition.id then return end
        definitions[definition.id] = definition
        if required then
            mandatory[definition.id] = true
        else
            candidates[definition.id] = true
        end
    end

    local races = context.state:get("races") or {}
    local race = object.race and races[object.race] or nil
    if not race or not enabled(race.ignore_global_traits) then
        for _, definition in pairs(context.state:get("traits") or {}) do add(definition, false) end
    end
    if race then
        for _, definition in ipairs(children(race, "trait")) do
            add(definition, definition.availability == "musthave")
        end
    end
    for _, definition in ipairs(children(object, "trait")) do add(definition, true) end

    local selected = {}
    if object.traits then
        for _, id in ipairs(object.traits) do selected[#selected + 1] = id end
    else
        for id in pairs(mandatory) do selected[#selected + 1] = id end
        table.sort(selected)
        local pool = {}
        for id in pairs(candidates) do
            if not mandatory[id] then pool[#pool + 1] = id end
        end
        table.sort(pool)
        local count = object.num_traits or race and race.num_traits or 0
        for _ = 1, math.min(count, #pool) do
            local index = context.random:integer(1, #pool)
            selected[#selected + 1] = table.remove(pool, index)
        end
    end
    for _, id in ipairs(selected) do
        local definition = definitions[id]
        if definition then
            for _, effect in ipairs(children(definition, "effect")) do
                apply_trait_effect(object, effect)
            end
        end
    end
    object.traits = selected
    object.traits_applied = true
end

function combat.initialize(context, request)
    local occupied = {}
    local reserves = {}
    for _, object in ipairs(context.objects:all()) do
        apply_traits(context, object)
        local position = { x = assert(object.x), y = assert(object.y) }
        context.map:get(position)
        local key = position.x .. "," .. position.y
        if occupied[key] then
            error("objects " .. occupied[key] .. " and " .. object.id
                .. " share a position")
        end
        occupied[key] = object.id
        context.objects:set(object.id, "position", position)
        context.objects:set(object.id, "hitpoints",
            object.initial_hitpoints or object.hitpoints or assert(object.max_hitpoints))
        context.objects:set(object.id, "movement_points", assert(object.max_moves))
        context.objects:set(object.id, "attacks_left", 1)
        context.objects:set(object.id, "experience", object.experience or 0)
        context.objects:set(object.id, "max_hitpoints", object.max_hitpoints)
        context.objects:set(object.id, "max_moves", object.max_moves)
        context.objects:set(object.id, "max_experience", object.max_experience)
        context.objects:set(object.id, "traits", object.traits)
        context.objects:set(object.id, "traits_applied", true)
        for _, status in ipairs({"unpoisonable", "undrainable", "unslowable", "unpetrifiable",
            "unplagueable", "invulnerable", "fearless", "healthy"}) do
            context.objects:set(object.id, status, enabled(object[status]))
        end
        context.objects:set(object.id, "__children", object.__children)
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
    local villages = context.state:get("villages") or {}
    for _, configured in ipairs(children(scenario(context), "village")) do
        assert(context.map:get(configured) == "village", "[village] must reference village terrain")
        local village
        for _, candidate in ipairs(villages) do
            if candidate.x == configured.x and candidate.y == configured.y then
                village = candidate
                break
            end
        end
        assert(village, "[village] is missing from map villages")
        village.side = configured.side
    end
    context.state:set("villages", villages)
    for _, side in ipairs(children(scenario(context), "side")) do
        local gold = side.gold or 0
        if request and request.campaign_side == side.id then
            gold = math.max(gold, assert(request.campaign_gold))
        end
        context.state:set("gold:" .. side.id, gold)
    end
    update_shroud(context)
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
        local attacks = {}
        local traits = object.traits or {}
        for _, attack in ipairs(children(object, "attack")) do
            local specials = {}
            for _, special in ipairs(children(attack, "special")) do
                specials[#specials + 1] = special.id
            end
            attacks[#attacks + 1] = {
                id = attack.id,
                name = attack.name or attack.id,
                damage = attack.damage,
                strikes = attack.strikes,
                range = attack.range,
                damage_type = attack.damage_type,
                specials = specials,
            }
        end
        result[#result + 1] = {
            id = object.id,
            type = object.type,
            name = object.name,
            race = object.race,
            image = object.image,
            facing = object.facing or "se",
            traits = traits,
            side = object.side,
            is_leader = is_recruiter(object),
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
            defense = child(object, "defense"),
            resistances = child(object, "resistance"),
            unpoisonable = enabled(object.unpoisonable),
            undrainable = enabled(object.undrainable),
            unslowable = enabled(object.unslowable),
            unpetrifiable = enabled(object.unpetrifiable),
            unplagueable = enabled(object.unplagueable),
            invulnerable = enabled(object.invulnerable),
            poisoned = object.poisoned == true,
            slowed = object.slowed == true,
            petrified = object.petrified == true,
            unhealable = object.unhealable == true,
            stunned = object.stunned == true,
            attacks = attacks,
        }
    end
    return result
end

local function unit_option(id, unit, cost)
    local attacks = {}
    local traits = {}
    for _, trait in ipairs(children(unit, "trait")) do
        traits[#traits + 1] = trait.id
    end
    for _, attack in ipairs(children(unit, "attack")) do
        local specials = {}
        for _, special in ipairs(children(attack, "special")) do
            specials[#specials + 1] = special.id
        end
        attacks[#attacks + 1] = {
            name = attack.name or attack.id,
            damage = attack.damage,
            strikes = attack.strikes,
            range = attack.range,
            damage_type = attack.damage_type,
            specials = specials,
        }
    end
    return {
        id = id,
        type = unit.type or id,
        name = unit.name or id,
        race = unit.race,
        image = unit.image,
        traits = traits,
        cost = cost or unit.cost,
        hitpoints = unit.hitpoints or unit.max_hitpoints,
        max_hitpoints = unit.max_hitpoints,
        max_moves = unit.max_moves,
        level = unit.level,
        experience = unit.experience,
        max_experience = unit.max_experience,
        alignment = unit.alignment,
        attacks = attacks,
    }
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
    local turn_limit
    for _, objective in ipairs(children(scenario(context), "objective")) do
        if objective.when == "turn_limit" then turn_limit = objective.turn end
    end
    local recruit_types = {}
    local known_recruits = {}
    if side and side.recruit then
        for id in string.gmatch(side.recruit, "[^,%s]+") do
            recruit_types[#recruit_types + 1] = id
            known_recruits[id] = true
        end
    end
    for _, leader in ipairs(context.objects:all()) do
        if leader.side == side.id and is_recruiter(leader) then
            for id in string.gmatch(leader.extra_recruit or "", "[^,%s]+") do
                if not known_recruits[id] then
                    recruit_types[#recruit_types + 1] = id
                    known_recruits[id] = true
                end
            end
        end
    end
    local recruit_options = {}
    local templates = assert(context.state:get("unit_types"), "missing unit types")
    for _, id in ipairs(recruit_types) do
        local unit = assert(templates[id], "unknown unit type")
        recruit_options[#recruit_options + 1] = unit_option(id, unit)
    end
    local recall_units = {}
    local recall_options = {}
    for _, unit in ipairs(context.state:get("recall") or {}) do
        recall_units[#recall_units + 1] = unit.id
        recall_options[#recall_options + 1] = unit_option(
            unit.id, unit, scenario(context).recall_cost or 20)
    end
    local pending_advancement = context.state:get("pending_advancement")
    if pending_advancement then
        pending_advancement.details = {}
        for _, id in ipairs(pending_advancement.options) do
            local unit = assert(templates[id], "unknown advancement type")
            pending_advancement.details[#pending_advancement.details + 1] =
                unit_option(id, unit)
        end
    end
    local recruit_hexes = {}
    for _, leader in ipairs(context.objects:all()) do
        if leader.side == side.id and is_recruiter(leader) then
            for _, position in ipairs(recruitment_hexes(context, leader)) do
                position.leader = leader.id
                recruit_hexes[#recruit_hexes + 1] = position
            end
        end
    end
    local side_visuals = {}
    local colors = { "red", "blue", "green", "purple", "black", "brown", "orange", "white", "teal" }
    for index, configured in ipairs(children(scenario(context), "side")) do
        side_visuals[#side_visuals + 1] = {
            id = configured.id,
            flag_style = configured.flag_style or "default",
            color = configured.color or colors[(index - 1) % #colors + 1],
        }
    end
    local visible, visible_cells = visibility(context, active_side(context))
    local visible_units = {}
    for _, unit in ipairs(context.objects:all()) do
        if visible_to(context, active_side(context), unit) then
            visible_units[#visible_units + 1] = unit.id
        end
    end
    return {
        turn = context.state:get("turn"),
        active_side = active_side(context),
        finished = context.state:get("finished"),
        can_end_turn = can_end_turn,
        phase = context.state:get("phase"),
        objective = current_phase and current_phase.label or scenario(context).objective,
        result = context.state:get("result"),
        turn_limit = turn_limit,
        carryover_percentage = scenario(context).carryover_percentage or 0,
        gold = context.state:get("gold:" .. active_side(context)),
        recruit_types = recruit_types,
        recruit_options = recruit_options,
        recruit_hexes = recruit_hexes,
        recall_units = recall_units,
        recall_options = recall_options,
        side_visuals = side_visuals,
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
        pending_advancement = pending_advancement,
        pending_choice = context.state:get("pending_choice"),
        time_of_day = time.id,
        lawful_bonus = time.lawful_bonus,
        fog = visible ~= nil,
        visible_cells = visible_cells or {},
        visible_units = visible_units,
        shroud = side.shroud == "yes",
        revealed_cells = context.state:get("revealed:" .. active_side(context)) or {},
    }
end

function combat.choose(context, command)
    assert_playing(context)
    assert_no_pending_advancement(context)
    local pending = assert(context.state:get("pending_choice"), "no scenario choice is required")
    assert(pending.id == command.choice, "unexpected scenario choice")
    local choice = scenario_choice(context, pending.id)
    local valid = false
    for _, option in ipairs(children(choice, "option")) do
        if option.id == command.option then valid = true end
    end
    assert(valid, "invalid scenario choice option: " .. tostring(command.option))
    local expected = choice.expected_variable and context.state:get(choice.expected_variable)
        or choice.expected
    local correct = command.option == (expected or choice.fallback)
    context.state:set("pending_choice", nil)
    local removed = {}
    if correct and choice.remove_group then
        for _, object in ipairs(context.objects:all()) do
            if object.event_group == choice.remove_group then
                context.objects:remove(object.id)
                removed[#removed + 1] = object.id
            end
        end
    end
    local events = {{ type = "choice_resolved", choice = choice.id,
        option = command.option, correct = correct, removed = removed,
        dialog = correct and choice.correct_dialog or choice.wrong_dialog }}
    if choice.next then events[#events + 1] = enter_phase(context, choice.next) end
    return events
end

function combat.recruit(context, command)
    assert_playing(context)
    assert_no_pending_advancement(context)
    assert_no_pending_choice(context)
    local side_id = active_side(context)
    local side
    for _, candidate in ipairs(children(scenario(context), "side")) do
        if candidate.id == side_id then side = candidate end
    end
    assert(side, "active side is not configured")

    local leader, destination
    for _, object in ipairs(context.objects:all()) do
        if object.side == side_id and is_recruiter(object)
            and (not command.leader or command.leader == object.id)
            and context.map:get(object.position) == "keep"
            and leader_can_recruit(side, object, command.unit_type) then
            local candidate = matching_hex(recruitment_hexes(context, object), command.destination)
            if candidate then
                leader, destination = object, candidate
                break
            end
        end
    end
    assert(leader and destination,
        "no leader can recruit this unit on the requested castle cell")

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
    apply_traits(context, object)
    object.hitpoints = object.max_hitpoints
    object.movement_points = 0
    object.attacks_left = 0
    object.experience = 0
    context.objects:add(object)
    update_shroud(context)
    context.state:set("gold:" .. side_id, gold - object.cost)
    context.state:set("recruited:" .. side_id, true)
    return {
        type = "unit_recruited",
        unit = object.id,
        unit_type = command.unit_type,
        position = destination,
        cost = object.cost,
        gold = gold - object.cost,
    }
end

function combat.recall(context, command)
    assert_playing(context)
    assert_no_pending_advancement(context)
    assert_no_pending_choice(context)
    local side_id = active_side(context)
    local side = side_config(context, side_id)
    assert(side.controller == "human", "only a human side can recall")
    local destination = assert(command.destination, "recall destination is required")
    local leader
    for _, object in ipairs(context.objects:all()) do
        if object.side == side_id and is_recruiter(object)
            and (not command.leader or command.leader == object.id)
            and matching_hex(recruitment_hexes(context, object), destination) then
            leader = object
            break
        end
    end
    assert(leader, "recall destination is outside the leader's castle")

    local recall = context.state:get("recall") or {}
    local found
    for index, object in ipairs(recall) do
        if object.id == command.unit then
            found = table.remove(recall, index)
            break
        end
    end
    assert(found, "unknown recall unit: " .. tostring(command.unit))
    local cost = scenario(context).recall_cost or 20
    local gold = context.state:get("gold:" .. side_id)
    assert(gold >= cost, "not enough gold")
    found.side = side_id
    found.x, found.y = destination.x, destination.y
    found.position = destination
    found.movement_points = 0
    found.attacks_left = 0
    context.objects:add(found)
    update_shroud(context)
    context.state:set("recall", recall)
    context.state:set("gold:" .. side_id, gold - cost)
    return {
        type = "unit_recalled", unit = found.id, position = destination,
        cost = cost, gold = gold - cost,
    }
end


function combat.advance(context, command)
    assert_playing(context)
    assert_no_pending_choice(context)
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
    local pending_finish = context.state:get("pending_finish")
    if pending_finish then
        context.state:set("pending_finish", nil)
        return { advanced, finished(context, pending_finish.result, pending_finish.dialog) }
    end
    return advanced
end

function combat.resolve(context, command)
    assert_playing(context)
    assert_no_pending_advancement(context)
    assert_no_pending_choice(context)
    local attacker = context.objects:get(command.attacker)
    local defender = context.objects:get(command.defender)
    assert(attacker, "attack requires existing attacker: " .. tostring(command.attacker))
    assert(defender, "attack requires existing defender: " .. tostring(command.defender))
    assert(attacker.side == active_side(context), "attacker is not on the active side")
    assert(not allied(context, attacker.side, defender.side), "cannot attack an allied unit")
    assert(visible_to(context, attacker.side, defender), "cannot attack a hidden unit")
    assert(attacker.hitpoints > 0 and defender.hitpoints > 0, "defeated unit cannot fight")
    assert(not attacker.petrified, "petrified unit cannot attack")
    assert(not attacker.stunned, "stunned unit cannot attack")
    assert(not defender.petrified, "petrified unit cannot be attacked")
    assert(attacker.attacks_left > 0, "unit has no attacks left")
    local model, attack, retaliation, preview = prepare_battle(context, command, attacker, defender)
    assert(not preview.disabled, "weapon is disabled or target is out of range")
    local events = {}
    context.objects:set(attacker.id, "attacks_left", math.max(0, attacker.attacks_left - (attack.attacks_used or 1)))
    context.objects:set(attacker.id, "movement_points", math.max(0, attacker.movement_points - (attack.movement_used or 100000)))
    context.objects:set(attacker.id, "resting", false)
    local strike_counts = {0, 0}
    for _, side in ipairs(model.sequence) do
        if attacker.hitpoints == 0 or defender.hitpoints == 0
            or attacker.petrified or defender.petrified then break end
        strike_counts[side] = strike_counts[side] + 1
        if side == 1 then
            strike(context, events, attacker, defender, attack, strike_counts[side], true, model.attacker)
        else
            strike(context, events, defender, attacker, retaliation, strike_counts[side], false, model.defender)
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
        if ability(killer, "feeding") and defeated.race ~= "undead"
            and defeated.undead_variation ~= "null" then
            killer.max_hitpoints = killer.max_hitpoints + 1
            killer.hitpoints = math.min(killer.max_hitpoints, killer.hitpoints + 1)
            context.objects:set(killer.id, "max_hitpoints", killer.max_hitpoints)
            context.objects:set(killer.id, "hitpoints", killer.hitpoints)
        end
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
        berserk = model.berserk,
    }
    local scripted = attack_event(context, attacker, defender, defeated_id)
    local advancements = {}
    for _, unit in ipairs({attacker, defender}) do
        if unit.hitpoints > 0 then
            local event = advance_unit(context, unit)
            if event then advancements[#advancements + 1] = event end
        end
    end
    if result.defeated then
        local defeated = assert(context.objects:get(result.defeated))
        local killer = defeated.id == defender.id and attacker or defender
        local events = {
            result,
            {
                type = "unit_died",
                unit = defeated.id,
                side = defeated.side,
                position = defeated.position,
            },
        }
        if scripted then events[#events + 1] = scripted end
        context.objects:remove(defeated.id)
        local killer_weapon = defeated.id == defender.id and attack or retaliation
        local plague = (defeated.id == defender.id and model.attacker or model.defender).plague
        if plague and not enabled(defeated.unplagueable) and defeated.race ~= "undead" and defeated.undead_variation ~= "null"
            and context.map:get(defeated.position) ~= "village" then
            local unit_type = plague.type or killer.type
            local template = (context.state:get("unit_types") or {})[unit_type]
            if template then
                local number = context.state:get("next_recruit_id")
                context.state:set("next_recruit_id", number + 1)
                template.id = killer.side .. "_plague_" .. number
                template.type = unit_type
                template.side = killer.side
                template.x, template.y = defeated.position.x, defeated.position.y
                template.position = defeated.position
                apply_traits(context, template)
                template.hitpoints = template.max_hitpoints
                template.movement_points = 0
                template.attacks_left = 0
                template.experience = 0
                template.resting = false
                context.objects:add(template)
                events[#events + 1] = {
                    type = "unit_plagued", unit = template.id, unit_type = unit_type,
                    position = defeated.position, side = killer.side,
                }
            end
        end
        local achievement = kill_achievement(context, defeated, killer)
        if achievement then events[#events + 1] = achievement end
        for _, event in ipairs(advancements) do events[#events + 1] = event end
        local ending = objective_event(context, defeated.id, killer.id)
        local current = phase(context)
        if not ending and not context.state:get("pending_finish") and current then
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
        if scripted then results[#results + 1] = scripted end
        for _, event in ipairs(advancements) do results[#results + 1] = event end
        return results
    end
    if scripted then return { result, scripted } end
    return result
end

function combat.reachable(context, command)
    assert_playing(context)
    if context.state:get("pending_advancement") then return {} end
    local object = context.objects:get(command.object,
        { "side", "position", "hitpoints", "petrified", "stunned", "max_moves", "movement_points" },
        { "movement_costs", "ability" })
    assert(object, "reachable requires an existing object")
    assert(object.hitpoints > 0, "defeated object cannot move")
    assert(command.inspect or object.side == active_side(context), "object is not on the active side")
    if object.petrified or object.stunned then return {} end
    local movement_points = command.inspect and object.max_moves or object.movement_points

    -- Rules prepare compact constraints once. The world/map and the search loop
    -- stay in Rust; neither per-hex callbacks nor complete unit snapshots cross.
    local teams = {}
    for _, side in ipairs(children(context.state:get("scenario", {}, { "side" }), "side")) do
        teams[side.id] = context.state:get("team:" .. side.id) or side.team_name
    end
    local function allied_with_object(side)
        return side == object.side or (teams[object.side] ~= nil and teams[side] == teams[object.side])
    end
    local occupied_keys, blocked, stop_cells, stop_keys = {}, {}, {}, {}
    local ignores_zoc = ability(object, "skirmisher") ~= nil
    for _, other in ipairs(context.objects:all({ "side", "position", "hitpoints", "level", "petrified", "stunned" })) do
        if other.id ~= object.id then
            occupied_keys[position_key(other.position)] = true
            if not allied_with_object(other.side) then
                blocked[#blocked + 1] = other.position
                if not ignores_zoc and other.hitpoints > 0 and not other.petrified
                    and not other.stunned and (other.level or 0) > 0 then
                    for _, position in ipairs(context.map:neighbors(other.position)) do
                        local key = position_key(position)
                        if key ~= position_key(object.position) and not stop_keys[key] then
                            stop_keys[key] = true
                            stop_cells[#stop_cells + 1] = position
                        end
                    end
                end
            end
        end
    end
    local movement_costs, costs = child(object, "movement_costs"), {}
    for _, cell in ipairs(context.map:cells()) do
        local cost = movement_costs[terrain.kind(cell.value)]
        if cost and cost <= object.max_moves then
            costs[#costs + 1] = { position = cell.position, cost = cost }
        else
            blocked[#blocked + 1] = cell.position
        end
    end
    local native = context.map:search {
        origin = object.position, budget = movement_points, costs = costs,
        blocked = blocked, stop_cells = stop_cells,
    }
    local predecessors = {}
    for _, item in ipairs(native.predecessors) do
        predecessors[position_key(item.position)] = item.predecessor
    end
    local result, start_key = {}, position_key(object.position)
    for _, item in ipairs(native.costs) do
        local key = position_key(item.position)
        if key ~= start_key and not occupied_keys[key]
            and (not command.destination or position_key(command.destination) == key) then
            local path
            if command.paths ~= false then
                path = {}
                local cursor = item.position
                while position_key(cursor) ~= start_key do
                    table.insert(path, 1, cursor)
                    cursor = assert(predecessors[position_key(cursor)], "missing predecessor")
                end
            end
            result[#result + 1] = {
                position = item.position, cost = item.cost,
                zoc = stop_keys[key] == true, path = path,
            }
        end
    end

    -- Teleport is a game rule, retaining its existing direct-village semantics.
    if movement_points > 0 and ability(object, "teleport") then
        local villages = context.state:get("villages") or {}
        local start_key = position_key(object.position)
        local start_village
        for _, village in ipairs(villages) do
            if position_key(village) == start_key then start_village = village; break end
        end
        if start_village and start_village.side and allied_with_object(start_village.side) then
            local best = {}
            for _, cell in ipairs(result) do best[position_key(cell.position)] = cell.cost end
            for _, village in ipairs(villages) do
                local position = { x = village.x, y = village.y }
                local key = position_key(position)
                if allied_with_object(village.side) and not occupied_keys[key] and key ~= start_key
                    and (not command.destination or position_key(command.destination) == key)
                    and (best[key] == nil or best[key] > 1) then
                    for index = #result, 1, -1 do
                        if position_key(result[index].position) == key then table.remove(result, index) end
                    end
                    result[#result + 1] = {
                        position = position, cost = 1, teleport = true, zoc = false,
                        path = command.paths ~= false and {position} or nil,
                    }
                end
            end
        end
    end
    return result
end

function combat.end_turn(context)
    assert_playing(context)
    assert_no_pending_advancement(context)
    assert_no_pending_choice(context)
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
            for _, event in ipairs(simple_ai.turn(combat, context, side, current)) do
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
    local scheduled = turn_event(context, turn)
    if scheduled then events[#events + 1] = scheduled end
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
    assert_no_pending_choice(context)
    local object = context.objects:get(command.object)
    local destination
    for _, cell in ipairs(combat.reachable(context, { object = command.object, destination = command.destination, paths = true })) do
        if cell.position.x == command.destination.x
            and cell.position.y == command.destination.y then
            destination = cell
            break
        end
    end
    assert(destination, "move destination is unreachable")

    local from = object.position
    -- Facing follows the final path edge; teleportation preserves orientation.
    if not destination.teleport and #destination.path > 0 then
        local previous = destination.path[#destination.path - 1] or from
        local last = destination.path[#destination.path]
        local dy = (2 * last.y - (last.x % 2 == 0 and 1 or 0))
            - (2 * previous.y - (previous.x % 2 == 0 and 1 or 0))
        local facing = last.x == previous.x and (dy < 0 and "n" or "s")
            or (last.x > previous.x and (dy < 0 and "ne" or "se")
                or (dy < 0 and "nw" or "sw"))
        context.objects:set(object.id, "facing", facing)
    end
    context.objects:set(object.id, "position", command.destination)
    context.objects:set(object.id, "resting", false)
    local movement_points = destination.zoc and 0
        or object.movement_points - destination.cost
    context.objects:set(object.id, "movement_points", movement_points)
    update_shroud(context)
    local sighted = sight_event(context, context.objects:get(object.id))
    local capture
    local villages = context.state:get("villages") or {}
    local village
    for _, candidate in ipairs(villages) do
        if candidate.x == command.destination.x and candidate.y == command.destination.y then
            village = candidate
        end
    end
    if village and village.side ~= object.side then
        movement_points = 0
        context.objects:set(object.id, "movement_points", 0)
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
        teleported = destination.teleport == true,
    }
    local scripted = location_event(context, context.objects:get(object.id))
    local objective_ending = objective_event(context, nil, object.id)
    if objective_ending then
        local events = { event }
        if capture then events[#events + 1] = capture end
        if scripted then events[#events + 1] = scripted end
        if sighted then events[#events + 1] = sighted end
        events[#events + 1] = objective_ending
        return events
    end
    local current = phase(context)
    if current and current.when == "unit_reaches"
        and (not current.unit or current.unit == object.id)
        and (not current.side or current.side == object.side)
        and current.x == command.destination.x and current.y == command.destination.y then
        local events = { event }
        if capture then events[#events + 1] = capture end
        if scripted then events[#events + 1] = scripted end
        if sighted then events[#events + 1] = sighted end
        if current.result then
            events[#events + 1] = finished(context, current.result,
                current.finish_dialog or current.dialog)
        elseif current.choice then
            events[#events + 1] = require_choice(context, current.choice)
        else
            events[#events + 1] = enter_phase(context, current.next)
        end
        return events
    end
    if capture or scripted or sighted then
        local events = { event }
        if capture then events[#events + 1] = capture end
        if scripted then events[#events + 1] = scripted end
        if sighted then events[#events + 1] = sighted end
        return events
    end
    return event
end

function combat.actions(context, command)
    assert_playing(context)
    assert_no_pending_choice(context)
    local object = context.objects:get(command.object)
    assert(object, "actions require an existing object")
    assert(command.inspect or object.side == active_side(context),
        "object is not on the active side")

    local attacks = {}
    local targets = {}
    if object.attacks_left > 0 and not object.petrified and not object.stunned then
        for _, attack in ipairs(children(object, "attack")) do
            attacks[#attacks + 1] = attack.id
        end
        if #attacks > 0 and not command.inspect then
            for _, target in ipairs(context.objects:all()) do
                if not allied(context, target.side, object.side)
                    and visible_to(context, object.side, target)
                    and not target.petrified and target.hitpoints > 0 then
                    for _, weapon in ipairs(children(object,"attack")) do
                        if not disabled_weapon(context,object,target,weapon,nil,true) then
                            targets[#targets + 1] = target.id
                            break
                        end
                    end
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

function combat.move_with_actions(context, command)
    local result = combat.move(context, command)
    local events = result.type and { result } or result
    if not context.state:get("finished") and not context.state:get("pending_choice")
        and context.objects:get(command.object) then
        local available = combat.actions(context, { object = command.object })
        available.type = "available_actions"
        events[#events + 1] = available
    end
    return events
end

return combat
