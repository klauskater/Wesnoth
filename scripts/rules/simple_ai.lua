local function hex_distance(a, b)
    local function axial(position)
        local q = position.x - 1
        local row = position.y - 1
        local r = row - math.floor((q - (q % 2)) / 2)
        return q, r, -q - r
    end
    local aq, ar, as = axial(a)
    local bq, br, bs = axial(b)
    return (math.abs(aq - bq) + math.abs(ar - br) + math.abs(as - bs)) / 2
end

local function simple_ai_turn(rules, context, side_config)
    local side = side_config.id
    local events = {}
    for _, unit in ipairs(context.objects:all()) do
        if unit.side == side and unit.hitpoints > 0
            and not unit.petrified and not unit.stunned then
            local target
            for _, candidate in ipairs(context.objects:all()) do
                if candidate.side ~= side and candidate.hitpoints > 0 and not candidate.petrified
                    and (not target or hex_distance(unit.position, candidate.position)
                        < hex_distance(unit.position, target.position)) then
                    target = candidate
                end
            end
            if target then
                local passive = side_config.passive_leader == "yes" and unit.is_leader == "yes"
                if not passive and not context.map:are_adjacent(unit.position, target.position) then
                    local best
                    for _, cell in ipairs(rules.reachable(context, { object = unit.id })) do
                        if not best or hex_distance(cell.position, target.position)
                            < hex_distance(best.position, target.position) then
                            best = cell
                        end
                    end
                    if best and hex_distance(best.position, target.position)
                        < hex_distance(unit.position, target.position) then
                        local moved = rules.move(context, {
                            object = unit.id,
                            destination = best.position,
                        })
                        if moved.type then
                            events[#events + 1] = moved
                        else
                            for _, event in ipairs(moved) do events[#events + 1] = event end
                        end
                        unit = assert(context.objects:get(unit.id))
                    end
                end
                if context.map:are_adjacent(unit.position, target.position) then
                    local attacks = unit.__children and unit.__children.attack or {}
                    local attack = assert(attacks[1], "AI unit has no attack")
                    local battle = rules.resolve(context, {
                        attacker = unit.id,
                        defender = target.id,
                        weapon = attack.id,
                    })
                    if battle.type then
                        events[#events + 1] = battle
                        if battle.type == "scenario_finished" then return events end
                    else
                        for _, event in ipairs(battle) do
                            events[#events + 1] = event
                            if event.type == "scenario_finished" then return events end
                        end
                    end
                end
            end
        end
    end
    return events
end
