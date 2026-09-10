local U32 = 0xffffffff
local DIRECTIONS = { "n", "ne", "se", "s", "sw", "nw" }

local function u32(value)
    return value & U32
end

local function hash_string(value)
    local hash = 0
    for index = 1, #value do
        hash = u32(((hash << 9) | (hash >> 23)) ~ string.byte(value, index))
    end
    return hash
end

local function noise(x, y, rule)
    local a = u32((x + 92872973) ~ 918273)
    local b = u32((y + 1672517) ~ 128123)
    local c = u32((hash_string(rule) + 127390) ~ 13923787)
    local mixed = u32(a * b * c + a * b + b * c + a * c + a + b + c)
    return u32(mixed * mixed)
end

local function base_code(code)
    code = (code or ""):match("%S+$") or code or ""
    return code:match("^[^^]+") or code
end

local function overlay_code(code)
    return (code or ""):match("%^(.*)$") or ""
end

local function starts_with(value, prefix)
    return value:sub(1, #prefix) == prefix
end

local function neighbors(x, y)
    local up = x % 2 == 0 and -1 or 0
    return {
        { x, y - 1 },
        { x + 1, y + up },
        { x + 1, y + up + 1 },
        { x, y + 1 },
        { x - 1, y + up + 1 },
        { x - 1, y + up },
    }
end

local function cell(map, x, y)
    return map.cells[y] and map.cells[y][x]
end

local function key(x, y)
    return x .. "," .. y
end

local function register(assets, name)
    assets[name] = "assets/wesnoth/terrain/hills-mountains/" .. name .. ".png"
    return name
end

local function emit(sprites, assets, name, x, y, order, pass, offset_x, offset_y, baseline, clips)
    sprites[#sprites + 1] = {
        asset = register(assets, name),
        x = x,
        y = y,
        offset_x = offset_x or -36,
        offset_y = offset_y or -36,
        baseline = baseline or 0,
        order = order,
        pass = pass or "ground",
        clips = clips,
    }
end

local function choose(stem, variants, x, y, rule)
    local variant = (noise(x - 1, y - 1, rule or stem) // 7919) % variants + 1
    return stem .. (variant == 1 and "" or variant)
end

local function is_mountain(map, x, y)
    return base_code(cell(map, x, y)) == "Mm"
end

local function claim(map, claimed, positions)
    for _, position in ipairs(positions) do
        if claimed[key(position[1], position[2])]
            or not is_mountain(map, position[1], position[2])
        then
            return false
        end
    end
    for _, position in ipairs(positions) do
        claimed[key(position[1], position[2])] = true
    end
    return true
end

local function clip_positions(positions)
    local result = {}
    for _, position in ipairs(positions) do
        result[#result + 1] = { x = position[1], y = position[2] }
    end
    return result
end

local function emit_range(sprites, assets, stem, pieces, x, y, center_y, bases, positions)
    local clips = clip_positions(positions)
    for piece = 1, pieces do
        emit(
            sprites,
            assets,
            stem .. "_" .. piece,
            x,
            y,
            0,
            "world",
            -90,
            -center_y,
            36 + bases[piece] - center_y,
            clips
        )
    end
end

local function hard_edge(raw)
    if not raw then
        return true
    end
    local base = base_code(raw)
    return starts_with(base, "C")
        or starts_with(base, "K")
        or starts_with(base, "X")
        or starts_with(base, "Q")
end

local function restricted_asset(map, x, y)
    local hard = {}
    for direction, neighbor in ipairs(neighbors(x, y)) do
        hard[direction] = hard_edge(cell(map, neighbor[1], neighbor[2]))
    end
    for _, pair in ipairs({ { 1, 2, "n-ne" }, { 6, 1, "nw-n" }, { 5, 6, "sw-nw" } }) do
        if hard[pair[1]] and hard[pair[2]] then
            return "basic-castle-" .. pair[3]
        end
    end
    for direction = 1, 6 do
        if hard[direction] then
            local stem = "basic-castle-" .. DIRECTIONS[direction]
            if direction == 1 then
                return choose(stem, 3, x, y, "mountains/basic-castle-n@V")
            end
            return stem
        end
    end
end

local function transitions(map, sprites, assets, stem, order, source, target, pairs)
    for y = 1, map.height do
        for x = 1, map.width do
            local raw = cell(map, x, y)
            if target(base_code(raw)) then
                local available = {}
                local adjacent = neighbors(x, y)
                for direction, neighbor in ipairs(adjacent) do
                    local adjacent_base = base_code(cell(map, neighbor[1], neighbor[2]))
                    available[direction] = source(adjacent_base)
                end
                for _, pair in ipairs(pairs) do
                    if available[pair[1]] and available[pair[2]] then
                        local name = stem .. "-" .. pair[3]
                        if pair[4] and noise(x - 1, y - 1, name) % 2 == 1 then
                            name = stem .. "2-" .. pair[3]
                        end
                        emit(sprites, assets, name, x, y, order)
                        available[pair[1]] = false
                        available[pair[2]] = false
                    end
                end
                for direction = 1, 6 do
                    if available[direction] then
                        local suffix = DIRECTIONS[direction]
                        local name = stem .. "-" .. suffix
                        if suffix == "s"
                            and noise(x - 1, y - 1, name) % 2 == 1
                            and stem == "regular-to-water"
                        then
                            name = stem .. "2-" .. suffix
                        end
                        emit(sprites, assets, name, x, y, order)
                    end
                end
            end
        end
    end
end

return function(map)
    local assets = {}
    local sprites = {}
    local claimed = {}

    for _, tile in ipairs(map.tiles) do
        if tile.terrain == "hills" or tile.terrain == "Hh" or tile.terrain == "Mm" then
            emit(
                sprites,
                assets,
                choose("regular", 3, tile.x, tile.y, "hills/regular@V"),
                tile.x,
                tile.y,
                -1000
            )
        elseif tile.terrain == "Hd" then
            emit(
                sprites,
                assets,
                choose("desert", 3, tile.x, tile.y, "hills/desert@V"),
                tile.x,
                tile.y,
                -1000
            )
        end
    end

    transitions(map, sprites, assets, "regular", -180, function(base)
        return base == "Hh" or base == "Mm"
    end, function(base)
        return base ~= ""
            and base ~= "Hh"
            and not starts_with(base, "M")
            and base ~= "Ai"
            and not starts_with(base, "W")
            and not starts_with(base, "S")
    end, { { 1, 2, "n-ne" }, { 4, 5, "s-sw" } })
    transitions(map, sprites, assets, "regular-to-water", -482, function(base)
        return base == "Hh" or base == "Mm"
    end, function(base)
        return base == "Ai" or starts_with(base, "W") or starts_with(base, "S")
    end, {
        { 1, 2, "n-ne" },
        { 2, 3, "ne-se" },
        { 6, 1, "nw-n" },
        { 4, 5, "s-sw", true },
        { 3, 4, "se-s", true },
        { 5, 6, "sw-nw" },
    })
    transitions(map, sprites, assets, "desert", -184, function(base)
        return base == "Hd"
    end, function(base)
        return base ~= "" and base ~= "Hd" and not starts_with(base, "Q") and not starts_with(base, "W")
    end, { { 1, 2, "n-ne" }, { 4, 5, "s-sw" } })

    -- Original order: restricted edges, 2x4, 1x3, 2x2, then singles.
    for y = 1, map.height do
        for x = 1, map.width do
            if is_mountain(map, x, y) then
                local name = restricted_asset(map, x, y)
                if name then
                    claimed[key(x, y)] = true
                    emit(sprites, assets, name, x, y, 0, "world", -90, -108, 35, { { x = x, y = y } })
                end
            end
        end
    end

    for _, config in ipairs({
        { direction = 3, side = 2, stem = "basic_range3", probability = 18, center_y = 144,
          bases = { 107, 107, 73, 108, 144 } },
        { direction = 2, side = 3, stem = "basic_range4", probability = 26, center_y = 216,
          bases = { 144, 108, 73, 107, 107 } },
    }) do
        for y = 1, map.height do
            for x = 1, map.width do
                local adjacent = neighbors(x, y)
                local second = adjacent[config.direction]
                local second_adjacent = neighbors(second[1], second[2])
                local third = second_adjacent[config.direction]
                local third_adjacent = neighbors(third[1], third[2])
                local fourth = third_adjacent[config.direction]
                local positions = {
                    { x, y },
                    adjacent[config.side],
                    second,
                    second_adjacent[config.side],
                    third,
                    third_adjacent[config.side],
                    fourth,
                    neighbors(fourth[1], fourth[2])[config.side],
                }
                if noise(x - 1, y - 1, config.stem) % 100 < config.probability
                    and claim(map, claimed, positions)
                then
                    emit_range(sprites, assets, config.stem, 5, x, y, config.center_y, config.bases, positions)
                end
            end
        end
    end

    for _, config in ipairs({
        { direction = 3, stem = "basic_range1", probability = 20, center_y = 144,
          bases = { 107, 107, 144 } },
        { direction = 2, stem = "basic_range2", probability = 20, center_y = 216,
          bases = { 144, 107, 107 } },
    }) do
        for y = 1, map.height do
            for x = 1, map.width do
                local second = neighbors(x, y)[config.direction]
                local third = neighbors(second[1], second[2])[config.direction]
                if noise(x - 1, y - 1, config.stem) % 100 < config.probability
                    and claim(map, claimed, { { x, y }, second, third })
                then
                    emit_range(sprites, assets, config.stem, 3, x, y, config.center_y, config.bases, { { x, y }, second, third })
                end
            end
        end
    end

    for y = 1, map.height do
        for x = 1, map.width do
            local adjacent = neighbors(x, y)
            local east = adjacent[2]
            local southeast = adjacent[3]
            local far = neighbors(east[1], east[2])[3]
            local positions = { { x, y }, east, southeast, far }
            for _, config in ipairs({
                { stem = "basic5", probability = 40 },
                { stem = "basic6", probability = 30 },
            }) do
                if noise(x - 1, y - 1, config.stem) % 100 < config.probability
                    and claim(map, claimed, positions)
                then
                    emit_range(sprites, assets, config.stem, 3, x, y, 144, { 107, 107, 107 }, positions)
                    break
                end
            end
        end
    end

    for y = 1, map.height do
        for x = 1, map.width do
            if is_mountain(map, x, y) and not claimed[key(x, y)] then
                local name = choose("basic", 3, x, y, "mountains/basic@V")
                emit(sprites, assets, name, x, y, 0, "world", -90, -108, -18, { { x = x, y = y } })
            end
        end
    end

    local peak_claimed = {}
    for y = 1, map.height do
        for x = 1, map.width do
            if overlay_code(cell(map, x, y)) == "Xm" then
                emit(
                    sprites,
                    assets,
                    choose("cloud", 3, x, y, "mountains/cloud@V"),
                    x,
                    y,
                    1,
                    "world",
                    -72,
                    -72,
                    0,
                    { { x = x, y = y } }
                )
            end
        end
    end
    for y = 1, map.height do
        for x = 1, map.width do
            if overlay_code(cell(map, x, y)) == "Xm" and not peak_claimed[key(x, y)] then
                local northeast = neighbors(x, y)[2]
                if overlay_code(cell(map, northeast[1], northeast[2])) == "Xm"
                    and not peak_claimed[key(northeast[1], northeast[2])]
                    and noise(x - 1, y - 1, "mountains/peak_range1") % 100 < 15
                then
                    peak_claimed[key(x, y)] = true
                    peak_claimed[key(northeast[1], northeast[2])] = true
                    local clips = { { x = x, y = y }, { x = northeast[1], y = northeast[2] } }
                    emit(sprites, assets, "peak_range1_1", x, y, 1, "world", -90, -144, 0, clips)
                    emit(sprites, assets, "peak_range1_2", x, y, 2, "world", -90, -144, 0, clips)
                end
            end
        end
    end
    for y = 1, map.height do
        for x = 1, map.width do
            if overlay_code(cell(map, x, y)) == "Xm" and not peak_claimed[key(x, y)] then
                local adjacent = neighbors(x, y)
                local large = overlay_code(cell(map, adjacent[3][1], adjacent[3][2])) == "Xm"
                    and overlay_code(cell(map, adjacent[4][1], adjacent[4][2])) == "Xm"
                    and overlay_code(cell(map, adjacent[5][1], adjacent[5][2])) == "Xm"
                local name
                if large and noise(x - 1, y - 1, "mountains/peak_large1") % 100 < 25 then
                    name = "peak_large1"
                elseif large and noise(x - 1, y - 1, "mountains/peak_large2") % 100 < 33 then
                    name = "peak_large2"
                else
                    name = choose("peak", 5, x, y, "mountains/peak@V")
                end
                local clips = { { x = x, y = y } }
                if large then
                    for direction = 3, 5 do
                        clips[#clips + 1] = { x = adjacent[direction][1], y = adjacent[direction][2] }
                    end
                end
                emit(sprites, assets, name, x, y, 2, "world", -90, -144, 0, clips)
            end
        end
    end

    return { assets = assets, static = sprites, animated = {} }
end
