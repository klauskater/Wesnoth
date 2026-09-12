local function in_enemy_zoc(context, object, position, objects, allied_with_object)
    for _, enemy in ipairs(objects) do
        if not allied_with_object(enemy.side) and enemy.hitpoints > 0
            and not enemy.petrified and not enemy.stunned
            and (enemy.level or 0) > 0
            and context.map:are_adjacent(enemy.position, position) then
            return true
        end
    end
    return false
end

function combat.legacy_reachable(context, command)
    assert_playing(context)
    if context.state:get("pending_advancement") then return {} end
    local object = context.objects:get(command.object)
    assert(object, "reachable requires an existing object")
    assert(object.hitpoints > 0, "defeated object cannot move")
    assert(command.inspect or object.side == active_side(context),
        "object is not on the active side")
    if object.petrified or object.stunned then return {} end
    local movement_points = command.inspect and object.max_moves or object.movement_points

    -- Fetching objects crosses the Rust/Lua boundary and materializes every
    -- unit. Keep one snapshot for the whole path search instead of doing that
    -- once per visited hex through in_enemy_zoc.
    local objects = context.objects:all()
    local occupied = {}
    for _, other in ipairs(objects) do
        if other.id ~= object.id then
            occupied[position_key(other.position)] = other
        end
    end

    local start_key = position_key(object.position)
    local teams = {}
    local function team(side)
        if teams[side] == nil then
            teams[side] = side_team(context, side) or false
        end
        return teams[side]
    end
    local object_team = team(object.side)
    local function allied_with_object(side)
        return side == object.side or object_team and object_team == team(side)
    end
    local movement_costs = child(object, "movement_costs")
    local ignores_zoc = ability(object, "skirmisher") ~= nil
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
            current.zoc = not ignores_zoc and current_key ~= start_key
                and in_enemy_zoc(context, object, current.position, objects,
                    allied_with_object)
            if current_key ~= start_key and not occupied[current_key] then
                result[#result + 1] = current
            end

            if not current.zoc then
                for _, destination in ipairs(context.map:neighbors(current.position)) do
                    local key = position_key(destination)
                    local terrain = context.map:get(destination)
                    local step_cost = movement_costs[terrain]
                    local passable = step_cost and step_cost <= object.max_moves
                    local cost = passable and current.cost < movement_points
                        and math.min(movement_points, current.cost + step_cost)
                    local blocker = occupied[key]
                    if cost and cost <= movement_points
                        and (not blocker or allied_with_object(blocker.side))
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

    local start_village = village_at(context, object.position)
    if movement_points > 0 and ability(object, "teleport")
        and start_village and start_village.side
        and allied_with_object(start_village.side) then
        for _, village in ipairs(context.state:get("villages") or {}) do
            local position = { x = village.x, y = village.y }
            local key = position_key(position)
            if allied_with_object(village.side) and not occupied[key]
                and key ~= start_key and (best[key] == nil or best[key] > 1) then
                for index = #result, 1, -1 do
                    if position_key(result[index].position) == key then
                        table.remove(result, index)
                    end
                end
                result[#result + 1] = {
                    position = position, cost = 1, path = {position},
                    teleport = true, zoc = false,
                }
                best[key] = 1
            end
        end
    end

    return result
end


