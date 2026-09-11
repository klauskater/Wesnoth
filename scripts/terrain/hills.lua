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
    local hash = type(rule) == "number" and rule or hash_string(rule)
    local c = u32((hash + 127390) ~ 13923787)
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

-- Coordinates copied from the maps in new-mountains.cfg.  Every point,
-- including "*", receives a cropped part of a global image in Wesnoth.
local PATTERNS = {
    range3 = {
        { "*", 2, 0 }, { "*", 1, 0 }, { "*", 3, 0 }, { "*", 0, 1 },
        { "1", 2, 1 }, { "*", 4, 1 }, { "1", 1, 1 }, { "1", 3, 1 },
        { "*", 5, 1 }, { "*", 0, 2 }, { "1", 2, 2 }, { "1", 4, 2 },
        { "*", 6, 2 }, { "*", 1, 2 }, { "1", 3, 2 }, { "1", 5, 2 },
        { "*", 2, 3 }, { "1", 4, 3 }, { "*", 6, 3 }, { "*", 3, 3 },
        { "*", 5, 3 },
    },
    range4 = {
        { "*", 4, 0 }, { "*", 3, 0 }, { "*", 5, 0 }, { "*", 2, 1 },
        { "1", 4, 1 }, { "*", 6, 1 }, { "*", 1, 1 }, { "1", 3, 1 },
        { "1", 5, 1 }, { "*", 0, 2 }, { "1", 2, 2 }, { "1", 4, 2 },
        { "*", 6, 2 }, { "1", 1, 2 }, { "1", 3, 2 }, { "*", 5, 2 },
        { "*", 0, 3 }, { "1", 2, 3 }, { "*", 4, 3 }, { "*", 1, 3 },
        { "*", 3, 3 },
    },
    range1 = {
        { "*", 1, 0 }, { "*", 0, 1 }, { "*", 2, 1 }, { "1", 1, 1 },
        { "*", 3, 1 }, { "*", 0, 2 }, { "1", 2, 2 }, { "*", 4, 2 },
        { "*", 1, 2 }, { "1", 3, 2 }, { "*", 5, 2 }, { "*", 2, 3 },
        { "*", 4, 3 }, { "*", 3, 3 },
    },
    range2 = {
        { "*", 3, 0 }, { "*", 2, 1 }, { "*", 4, 1 }, { "*", 1, 1 },
        { "1", 3, 1 }, { "*", 5, 1 }, { "*", 0, 2 }, { "1", 2, 2 },
        { "*", 4, 2 }, { "1", 1, 2 }, { "*", 3, 2 }, { "*", 0, 3 },
        { "*", 2, 3 },
    },
    block = {
        { "*", 2, 0 }, { "*", 1, 0 }, { "*", 3, 0 }, { "*", 0, 1 },
        { "1", 2, 1 }, { "*", 4, 1 }, { "1", 1, 1 }, { "1", 3, 1 },
        { "*", 0, 2 }, { "1", 2, 2 }, { "*", 4, 2 }, { "*", 1, 2 },
        { "*", 3, 2 }, { "*", 2, 3 },
    },
    peak_range = {
        { "*", 1, 0 }, { "*", 3, 0 }, { "*", 0, 1 }, { "1", 2, 1 },
        { "1", 1, 1 }, { "2", 3, 1 }, { "*", 0, 2 }, { "2", 2, 2 },
        { "2", 1, 2 }, { "*", 3, 2 },
    },
    peak_large = {
        { "*", 1, 0 }, { "*", 0, 1 }, { "*", 2, 1 }, { "1", 1, 1 },
        { "2", 0, 2 }, { "2", 2, 2 }, { "2", 1, 2 },
    },
}

local function legacy_sum(position, delta)
    local parity = position[1] % 2 ~= 0
    local x = position[1] + delta[1]
    local y = position[2] + delta[2]
    if delta[1] > 0 and delta[1] % 2 ~= 0 and parity then
        y = y + 1
    elseif delta[1] < 0 and delta[1] % 2 ~= 0 and not parity then
        y = y - 1
    end
    return { x, y }
end

local function pattern_origin(pattern, x, y)
    local anchor
    for _, point in ipairs(pattern) do
        if point[1] == "1" then
            anchor = point
            break
        end
    end
    return legacy_sum({ x - 1, y - 1 }, { -anchor[2], -anchor[3] })
end

local function place_pattern(pattern, x, y, kind)
    local anchor
    for _, point in ipairs(pattern) do
        if point[1] == "1" then
            anchor = point
            break
        end
    end
    local result = {}
    for _, point in ipairs(pattern) do
        if kind == nil or point[1] == kind then
            -- WML builder columns are staggered down; client columns are
            -- staggered up. Preserve pixel deltas, not raw row deltas.
            local target_x = x + point[2] - anchor[2]
            local row_shift = (point[2] % 2 - anchor[2] % 2
                + (target_x % 2 == 0 and 1 or 0)
                - (x % 2 == 0 and 1 or 0)) // 2
            result[#result + 1] = { target_x, y + point[3] - anchor[3] + row_shift }
        end
    end
    return result
end

local function rule_hash(constraint_count, names)
    local hash = 0
    for _, name in ipairs(names) do
        hash = u32(hash + hash_string(name) * constraint_count)
    end
    return hash == 0 and 105533 or hash
end

local function range_rule_hash(pattern, stem, pieces)
    local names = {}
    for piece = 1, pieces do
        names[#names + 1] = "mountains/" .. stem .. "_" .. piece .. "@V.png"
    end
    return rule_hash(#pattern, names)
end

local function pattern_noise(pattern, x, y, hash)
    local origin = pattern_origin(pattern, x, y)
    return noise(origin[1], origin[2], hash)
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

local function single_footprint(x, y)
    local positions = { { x, y } }
    for _, position in ipairs(neighbors(x, y)) do
        positions[#positions + 1] = position
    end
    return positions
end

local function emit_range(sprites, assets, stem, pieces, x, y, center_x, center_y, bases, footprint)
    local clips = clip_positions(footprint)
    for piece = 1, pieces do
        emit(
            sprites,
            assets,
            stem .. "_" .. piece,
            x,
            y,
            0,
            "world",
            -center_x,
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
    local count = 0
    for direction, neighbor in ipairs(neighbors(x, y)) do
        hard[direction] = hard_edge(cell(map, neighbor[1], neighbor[2]))
        if hard[direction] then
            count = count + 1
        end
    end
    if count == 2 then
        for _, pair in ipairs({ { 1, 2, "n-ne" }, { 6, 1, "nw-n" }, { 5, 6, "sw-nw" } }) do
            if hard[pair[1]] and hard[pair[2]] then
                return "basic-castle-" .. pair[3]
            end
        end
    end
    if count == 1 then
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
end

local function touches_hard_edge(map, x, y)
    for _, neighbor in ipairs(neighbors(x, y)) do
        if hard_edge(cell(map, neighbor[1], neighbor[2])) then
            return true
        end
    end
    return false
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
    local small_marked = {}
    for x = 1, map.width do
        for y = 1, map.height do
            if is_mountain(map, x, y) then
                local name = restricted_asset(map, x, y)
                if name then
                    claimed[key(x, y)] = true
                    emit(sprites, assets, name, x, y, 0, "world", -90, -108, 35,
                        clip_positions(single_footprint(x, y)))
                elseif touches_hard_edge(map, x, y) then
                    small_marked[key(x, y)] = true
                end
            end
        end
    end

    for _, config in ipairs({
        { pattern = PATTERNS.range3, stem = "basic_range3", probability = 18, center_x = 144, center_y = 108,
          bases = { 107, 107, 73, 108, 144 }, hash = range_rule_hash(PATTERNS.range3, "basic_range3", 5) },
        { pattern = PATTERNS.range4, stem = "basic_range4", probability = 26, center_x = 252, center_y = 108,
          bases = { 144, 108, 73, 107, 107 }, hash = range_rule_hash(PATTERNS.range4, "basic_range4", 5) },
    }) do
        for x = 1, map.width do
            for y = 1, map.height do
                local positions = place_pattern(config.pattern, x, y, "1")
                if pattern_noise(config.pattern, x, y, config.hash) % 100 <= config.probability
                    and claim(map, claimed, positions)
                then
                    emit_range(sprites, assets, config.stem, 5, x, y, config.center_x, config.center_y,
                        config.bases, place_pattern(config.pattern, x, y))
                end
            end
        end
    end

    for _, config in ipairs({
        { pattern = PATTERNS.range1, stem = "basic_range1", probability = 20, center_x = 90, center_y = 144,
          bases = { 107, 107, 144 }, hash = range_rule_hash(PATTERNS.range1, "basic_range1", 3) },
        { pattern = PATTERNS.range2, stem = "basic_range2", probability = 20, center_x = 198, center_y = 144,
          bases = { 144, 107, 107 }, hash = range_rule_hash(PATTERNS.range2, "basic_range2", 3) },
    }) do
        for x = 1, map.width do
            for y = 1, map.height do
                local positions = place_pattern(config.pattern, x, y, "1")
                if pattern_noise(config.pattern, x, y, config.hash) % 100 <= config.probability
                    and claim(map, claimed, positions)
                then
                    emit_range(sprites, assets, config.stem, 3, x, y, config.center_x, config.center_y,
                        config.bases, place_pattern(config.pattern, x, y))
                end
            end
        end
    end

    for _, config in ipairs({
        { stem = "basic5", probability = 40, hash = range_rule_hash(PATTERNS.block, "basic5", 3) },
        { stem = "basic6", probability = 30, hash = range_rule_hash(PATTERNS.block, "basic6", 3) },
    }) do
        for x = 1, map.width do
            for y = 1, map.height do
                local positions = place_pattern(PATTERNS.block, x, y, "1")
                if pattern_noise(PATTERNS.block, x, y, config.hash) % 100 <= config.probability
                    and claim(map, claimed, positions)
                then
                    emit_range(sprites, assets, config.stem, 3, x, y, 144, 108,
                        { 107, 107, 107 }, place_pattern(PATTERNS.block, x, y))
                end
            end
        end
    end

    for x = 1, map.width do
        for y = 1, map.height do
            if is_mountain(map, x, y) and not claimed[key(x, y)] then
                local name
                local baseline
                if small_marked[key(x, y)] then
                    name = choose("basic-castle-n", 3, x, y, "mountains/basic-castle-n@V")
                    baseline = 35
                else
                    name = choose("basic", 3, x, y, "mountains/basic@V")
                    baseline = -18
                end
                emit(sprites, assets, name, x, y, 0, "world", -90, -108, baseline,
                    clip_positions(single_footprint(x, y)))
            end
        end
    end

    local peak_claimed = {}
    for x = 1, map.width do
        for y = 1, map.height do
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
                    52,
                    clip_positions(single_footprint(x, y))
                )
            end
        end
    end
    for x = 1, map.width do
        for y = 1, map.height do
            local claimed_positions = place_pattern(PATTERNS.peak_range, x, y, "1")
            local required_positions = place_pattern(PATTERNS.peak_range, x, y, "1")
            for _, position in ipairs(place_pattern(PATTERNS.peak_range, x, y, "2")) do
                required_positions[#required_positions + 1] = position
            end
            local matches = true
            for _, position in ipairs(required_positions) do
                if overlay_code(cell(map, position[1], position[2])) ~= "Xm" then
                    matches = false
                    break
                end
            end
            for _, position in ipairs(claimed_positions) do
                if peak_claimed[key(position[1], position[2])] then
                    matches = false
                end
            end
            local hash = rule_hash(#PATTERNS.peak_range, {
                "mountains/peak_range1_1.png",
                "mountains/peak_range1_2.png",
            })
            if matches and pattern_noise(PATTERNS.peak_range, x, y, hash) % 100 <= 15 then
                for _, position in ipairs(claimed_positions) do
                    peak_claimed[key(position[1], position[2])] = true
                end
                local clips = clip_positions(place_pattern(PATTERNS.peak_range, x, y))
                emit(sprites, assets, "peak_range1_1", x, y, 1, "world", -144, -108, 0, clips)
                emit(sprites, assets, "peak_range1_2", x, y, 2, "world", -144, -108, 0, clips)
            end
        end
    end
    for _, config in ipairs({
        { name = "peak_large1", probability = 25,
          hash = rule_hash(#PATTERNS.peak_large, { "mountains/peak_large1@V.png" }) },
        { name = "peak_large2", probability = 33,
          hash = rule_hash(#PATTERNS.peak_large, { "mountains/peak_large2@V.png" }) },
    }) do
        for x = 1, map.width do
            for y = 1, map.height do
                local required_positions = place_pattern(PATTERNS.peak_large, x, y, "1")
                for _, position in ipairs(place_pattern(PATTERNS.peak_large, x, y, "2")) do
                    required_positions[#required_positions + 1] = position
                end
                local matches = not peak_claimed[key(x, y)]
                for _, position in ipairs(required_positions) do
                    if overlay_code(cell(map, position[1], position[2])) ~= "Xm" then
                        matches = false
                        break
                    end
                end
                if matches
                    and pattern_noise(PATTERNS.peak_large, x, y, config.hash) % 100 <= config.probability
                then
                    peak_claimed[key(x, y)] = true
                    emit(sprites, assets, config.name, x, y, 2, "world", -90, -144, 0,
                        clip_positions(place_pattern(PATTERNS.peak_large, x, y)))
                end
            end
        end
    end
    for x = 1, map.width do
        for y = 1, map.height do
            if overlay_code(cell(map, x, y)) == "Xm" and not peak_claimed[key(x, y)] then
                local name = choose("peak", 5, x, y, "mountains/peak@V")
                -- NEW:OVERLAY centers each variation using its own PNG size.
                local offset_x = name == "peak" and -90 or -72
                local offset_y = name == "peak" and -108 or -72
                emit(sprites, assets, name, x, y, 2, "world", offset_x, offset_y, 52,
                    clip_positions(single_footprint(x, y)))
            end
        end
    end

    return { assets = assets, static = sprites, animated = {} }
end
