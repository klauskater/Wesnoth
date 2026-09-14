local function children(node, name)
    return node.__children and node.__children[name] or {}
end

local function hex_distance(a, b)
    local function axial(p)
        local q, row = p.x - 1, p.y - 1
        local r = row - math.floor((q - (q % 2)) / 2)
        return q, r, -q - r
    end
    local aq, ar, as = axial(a)
    local bq, br, bs = axial(b)
    return (math.abs(aq - bq) + math.abs(ar - br) + math.abs(as - bs)) / 2
end

local function append(events, result)
    if result.type then events[#events + 1] = result
    else for _, event in ipairs(result) do events[#events + 1] = event end end
end

local function is_finished(events)
    for _, event in ipairs(events) do
        if event.type == "scenario_finished" then return true end
    end
    return false
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

local function is_leader(unit)
    return unit.is_leader == "yes" or unit.canrecruit == "yes"
end

local function unit_value(unit)
    return unit.cost or (unit.level or 0) * 18 + (unit.max_hitpoints or 1) * 0.6
end

local function visible_enemies(rules, context, side)
    local result = {}
    for _, unit in ipairs(context.objects:all()) do
        if rules.are_enemies(context, unit.side, side) and unit.hitpoints > 0
            and not unit.petrified and rules.can_see(context, side, unit) then
            result[#result + 1] = unit
        end
    end
    return result
end

local function village_at(context, position)
    for _, village in ipairs(context.state:get("villages") or {}) do
        if village.x == position.x and village.y == position.y then return village end
    end
end

local function adjacent_allies(rules, context, unit, position)
    local count = 0
    for _, ally in ipairs(context.objects:all()) do
        if ally.id ~= unit.id and not rules.are_enemies(context, ally.side, unit.side)
            and context.map:are_adjacent(ally.position, position) then count = count + 1 end
    end
    return count
end

local function nearest_distance(position, units)
    local best = math.huge
    for _, unit in ipairs(units) do
        best = math.min(best, hex_distance(position, unit.position or unit))
    end
    return best
end

local function strategic_targets(context, side)
    local scenario, result = context.state:get("scenario"), {}
    local function add(node)
        for _, location in ipairs(children(node, "location")) do
            result[#result + 1] = location
        end
        if node.when == "unit_reaches" and node.x and node.y then
            result[#result + 1] = {x = node.x, y = node.y}
        end
    end
    for _, objective in ipairs(children(scenario, "objective")) do
        if objective.result == "victory" and objective.side == side then add(objective) end
    end
    for _, current in ipairs(children(scenario, "phase")) do
        if current.id == context.state:get("phase") then
            local unit = current.unit and context.objects:get(current.unit)
            if current.side == side or (unit and unit.side == side) then add(current) end
        end
    end
    return result
end

local function healing_at(rules, context, unit, position)
    local healing, cures = 0, false
    local regeneration = ability(unit, "regenerates")
    if regeneration then healing, cures = regeneration.value or 0, true end
    local village = village_at(context, position)
    if village and village.side and not rules.are_enemies(context, village.side, unit.side) then
        healing = math.max(healing, context.state:get("scenario").village_heal or 0)
        cures = true
    end
    for _, ally in ipairs(context.objects:all()) do
        if ally.id ~= unit.id and not rules.are_enemies(context, ally.side, unit.side)
            and context.map:are_adjacent(ally.position, position) then
            local healer, curer = ability(ally, "heals"), ability(ally, "cures")
            healing = math.max(healing, healer and healer.value or 0,
                curer and curer.value or 0)
            cures = cures or curer ~= nil
        end
    end
    return healing, cures
end

local function threat_at(context, position, enemies)
    local threat = 0
    for _, enemy in ipairs(enemies) do
        local distance = hex_distance(position, enemy.position)
        if distance <= (enemy.max_moves or 0) + 1 then
            local strongest = 0
            for _, attack in ipairs(children(enemy, "attack")) do
                strongest = math.max(strongest, attack.damage * attack.strikes)
            end
            threat = threat + strongest / math.max(1, distance)
        end
    end
    return threat
end

local function attack_score(rules, context, cfg, unit, target, attack, position)
    local preview = rules.preview_attack(context, {
        attacker = unit.id, defender = target.id, weapon = attack.id, position = position,
    })
    local aggression, caution = cfg.aggression or 1, cfg.caution or 1
    if preview.disabled then return -math.huge end
    local score = preview.expected_damage * (3 + aggression)
        - preview.expected_retaliation * (2 + caution)
        + preview.kill_probability * unit_value(target) * 5
        - preview.death_probability * unit_value(unit) * (4 + caution)
        + (1 - target.hitpoints / target.max_hitpoints) * 35
    if is_leader(target) then score = score + 45 end
    local needed = unit.max_experience and unit.max_experience - (unit.experience or 0)
    if needed and needed <= (target.level or 0) * 8 then score = score + 25 end
    if has_special(attack, "poison") and not target.poisoned then score = score + 14 end
    if has_special(attack, "slow") and not target.slowed then score = score + 12 end
    if has_special(attack, "drain") and unit.hitpoints < unit.max_hitpoints then score = score + 8 end
    if is_leader(unit) then score = score - preview.death_probability * 200 end
    local improvement = math.max(0,
        preview.next_alignment_modifier - preview.alignment_modifier)
    score = score - improvement * (cfg.patience or 3)
        * (1 - preview.kill_probability)
    local health = unit.hitpoints / unit.max_hitpoints
    score = score - threat_at(context, position, visible_enemies(rules, context, unit.side))
        * caution * (health < 0.3 and 1.5 or 0.25)
    if health < 0.3 then score = score - 200 end
    return score
end

local function best_attack(rules, context, cfg, committed)
    local enemies, best = visible_enemies(rules, context, cfg.id)
    for _, unit in ipairs(context.objects:all()) do
        if unit.side == cfg.id and unit.hitpoints > 0 and unit.attacks_left > 0
            and not unit.petrified and not unit.stunned and not committed[unit.id] then
            local positions = {{ position = unit.position, cost = 0 }}
            if not (cfg.passive_leader == "yes" and is_leader(unit)) then
                for _, cell in ipairs(rules.reachable(context, { object = unit.id, paths = false })) do
                    positions[#positions + 1] = cell
                end
            end
            for _, cell in ipairs(positions) do
                for _, target in ipairs(enemies) do
                    for _, attack in ipairs(children(unit, "attack")) do
                        if (tonumber(attack.attack_weight) or 1) > 0
                            and rules.weapon_reaches(attack,cell.position,target.position) then
                            local score = attack_score(rules, context, cfg, unit, target,
                                attack, cell.position)
                            if score > -math.huge and (not best or score > best.score) then
                                best = { score = score, unit = unit.id, target = target.id,
                                    weapon = attack.id, position = cell.position,
                                    move = cell.cost > 0 }
                            end
                        end
                    end
                end
            end
        end
    end
    return best
end

local function movement_score(rules, context, cfg, unit, cell, enemies, goals)
    local score = 0
    if #enemies > 0 then
        score = score + (nearest_distance(unit.position, enemies)
            - nearest_distance(cell.position, enemies)) * (8 + (cfg.aggression or 1) * 3)
    end
    if #goals > 0 then
        score = score + (nearest_distance(unit.position, goals)
            - nearest_distance(cell.position, goals)) * (cfg.objective_weight or 14)
    end
    local health = unit.hitpoints / unit.max_hitpoints
    local village = village_at(context, cell.position)
    local owned = village and village.side == cfg.id
    if village and not owned then score = score + 38 end
    if owned and health < 0.7 then score = score + (1 - health) * 90 end
    if unit.poisoned and owned then score = score + 100 end
    local healing, cures = healing_at(rules, context, unit, cell.position)
    score = score + math.min(unit.max_hitpoints - unit.hitpoints, healing) * 3
    if unit.poisoned and cures then score = score + 100 end
    local heals = ability(unit, "heals") or ability(unit, "cures")
    if heals then
        for _, ally in ipairs(context.objects:all()) do
            if ally.id ~= unit.id and not rules.are_enemies(context, ally.side, unit.side)
                and ally.hitpoints < ally.max_hitpoints
                and context.map:are_adjacent(cell.position, ally.position) then
                score = score + math.min(ally.max_hitpoints - ally.hitpoints,
                    heals.value or 0) * 2
            end
        end
    end
    local defense = children(unit, "defense")[1]
    if defense then score = score + (defense[context.map:get(cell.position)] or 0) * 0.15 end
    score = score + adjacent_allies(rules, context, unit, cell.position) * (cfg.grouping or 4)
    score = score - threat_at(context, cell.position, enemies) * (cfg.caution or 1) * 0.35
    if health < 0.3 and not owned and #enemies > 0 then
        score = score + nearest_distance(cell.position, enemies) * 2
    end
    if is_leader(unit) then
        score = score - threat_at(context, cell.position, enemies) * (cfg.leader_caution or 2)
        if context.map:get(unit.position) == "keep"
            and context.state:get("gold:" .. cfg.id) > 0 then score = score - 80 end
    end
    return score
end

local function best_movement(rules, context, cfg, committed)
    local enemies, goals, best = visible_enemies(rules, context, cfg.id),
        strategic_targets(context, cfg.id)
    if #enemies == 0 and #goals == 0 then return end
    for _, unit in ipairs(context.objects:all()) do
        local passive = cfg.passive_leader == "yes" and is_leader(unit)
        if unit.side == cfg.id and unit.hitpoints > 0 and unit.movement_points > 0
            and not unit.petrified and not unit.stunned and not passive
            and not committed[unit.id] then
            for _, cell in ipairs(rules.reachable(context, { object = unit.id, paths = false })) do
                local score = movement_score(rules, context, cfg, unit, cell, enemies, goals)
                if not best or score > best.score then
                    best = { score = score, unit = unit.id, position = cell.position }
                end
            end
        end
    end
    return best and best.score > 0 and best or nil
end

local function recruit_score(template, enemies)
    local score = template.max_hitpoints * 0.5 + template.max_moves * 2 - template.cost * 0.25
    for _, attack in ipairs(children(template, "attack")) do
        local value = attack.damage * attack.strikes
        for _, enemy in ipairs(enemies) do
            local resistance = children(enemy, "resistance")[1]
            value = value + attack.damage * attack.strikes
                * (100 - (resistance and resistance[attack.damage_type] or 0)) / 100
        end
        score = score + value / math.max(1, #enemies)
    end
    return score
end

local function recruit_units(rules, context, cfg, events)
    local available = {}
    for id in string.gmatch(cfg.recruit or "", "[^,%s]+") do available[#available + 1] = id end
    if #available == 0 then return end
    local templates = context.state:get("unit_types")
    local enemies = visible_enemies(rules, context, cfg.id)
    for _ = 1, 20 do
        local gold, best, rating = context.state:get("gold:" .. cfg.id)
        for _, id in ipairs(available) do
            local template = templates[id]
            if template and template.cost <= gold then
                local score = recruit_score(template, enemies)
                if not rating or score > rating then best, rating = id, score end
            end
        end
        if not best then return end
        local ok, recruited = pcall(rules.recruit, context, { unit_type = best })
        if not ok then return end
        append(events, recruited)
    end
end

local function simple_ai_turn(rules, context, cfg, viewer)
    local events, committed = {}, {}
    local function perform(action, command)
        local before = rules.snapshot(context)
        local visible = rules.visible_unit_ids(context, viewer)
        local result = rules[action](context, command)
        local batch = result.type and {result} or result
        for _, event in ipairs(batch) do
            if event.type == "object_moved" or event.type == "battle_resolved" then
                event.replay_visible = visible
                for _, id in ipairs(rules.visible_unit_ids(context, viewer)) do
                    event.replay_visible[#event.replay_visible + 1] = id
                end
                event.replay_units = before
                event.replay_after = rules.snapshot(context)
            end
        end
        return result
    end
    recruit_units(rules, context, cfg, events)
    for _ = 1, 100 do
        local attack = best_attack(rules, context, cfg, committed)
        if attack and attack.score > (cfg.attack_threshold or -20) then
            if attack.move then
                append(events, perform("move",
                    { object = attack.unit, destination = attack.position }))
            end
            if context.objects:get(attack.unit) and context.objects:get(attack.target) then
                append(events, perform("resolve", { attacker = attack.unit,
                    defender = attack.target, weapon = attack.weapon }))
                if is_finished(events) then return events end
            end
        else
            local movement = best_movement(rules, context, cfg, committed)
            if not movement then break end
            append(events, perform("move",
                { object = movement.unit, destination = movement.position }))
            committed[movement.unit] = true
            if is_finished(events) then return events end
        end
    end
    return events
end

return { turn = simple_ai_turn }
