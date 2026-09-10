local U32 = 0xffffffff
local DIRECTIONS = { "n", "ne", "se", "s", "sw", "nw" }

local ROADS = {
    Rd = { stem = "desert-road", variants = 7 },
    Rr = { stem = "road", variants = 4 },
    Rp = { stem = "stone-path", variants = 2 },
}

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

local function noise(x, y, rule_hash)
    local a = u32((x + 92872973) ~ 918273)
    local b = u32((y + 1672517) ~ 128123)
    local c = u32((rule_hash + 127390) ~ 13923787)
    local mixed = u32(a * b * c + a * b + b * c + a * c + a + b + c)
    return u32(mixed * mixed)
end

local function base_code(code)
    code = code:match("%S+$") or code
    return code:match("^[^^]+") or code
end

local function starts_with(value, prefix)
    return value:sub(1, #prefix) == prefix
end

local function is_grass(code)
    return code == "grassland" or code == "Gg" or code == "Gs" or code == "Gd" or code == "Gll"
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

local function edge_key(ax, ay, bx, by)
    if ax > bx or (ax == bx and ay > by) then
        ax, ay, bx, by = bx, by, ax, ay
    end
    return ax .. "," .. ay .. ":" .. bx .. "," .. by
end

local function emit(sprites, assets, name, x, y, order)
    assets[name] = "assets/wesnoth/terrain/flat/" .. name .. ".png"
    sprites[#sprites + 1] = {
        asset = name,
        x = x,
        y = y,
        offset_x = -36,
        offset_y = -36,
        order = order,
        pass = "ground",
    }
end

local function choose_base(config, x, y)
    local variant = (noise(x - 1, y - 1, hash_string("flat/" .. config.stem .. "@V.png")) // 7919)
        % config.variants
        + 1
    return config.stem .. (variant == 1 and "" or variant)
end

local function apply_groups(map, sprites, assets, claimed, rule)
    for y = 1, map.height do
        for x = 1, map.width do
            local target = base_code(cell(map, x, y) or "")
            if rule.target(target) then
                local adjacent = neighbors(x, y)
                local available = {}
                for direction, neighbor in ipairs(adjacent) do
                    local source = base_code(cell(map, neighbor[1], neighbor[2]) or "")
                    local key = edge_key(x, y, neighbor[1], neighbor[2])
                    available[direction] = rule.source(source) and not claimed[rule.channel][key]
                end

                for _, length in ipairs(rule.lengths) do
                    for first = 1, 6 do
                        local suffixes = {}
                        local matches = true
                        for offset = 0, length - 1 do
                            local direction = (first + offset - 1) % 6 + 1
                            if not available[direction] then
                                matches = false
                                break
                            end
                            suffixes[#suffixes + 1] = DIRECTIONS[direction]
                        end
                        if matches then
                            for offset = 0, length - 1 do
                                local direction = (first + offset - 1) % 6 + 1
                                local neighbor = adjacent[direction]
                                available[direction] = false
                                claimed[rule.channel][edge_key(x, y, neighbor[1], neighbor[2])] = true
                            end
                            emit(
                                sprites,
                                assets,
                                rule.stem .. "-" .. table.concat(suffixes, "-"),
                                x,
                                y,
                                rule.order
                            )
                        end
                    end
                end
            end
        end
    end
end

return function(map)
    local assets = {}
    local sprites = {}
    local claimed = { inside = {}, transition = {} }

    for _, tile in ipairs(map.tiles) do
        local config = ROADS[tile.terrain]
        emit(sprites, assets, choose_base(config, tile.x, tile.y), tile.x, tile.y, -1000)
    end

    local function ordinary_target(road)
        return function(code)
            return code ~= ""
                and code ~= road
                and not starts_with(code, "W")
                and code ~= "Ai"
                and not starts_with(code, "Q")
                and not is_grass(code)
        end
    end

    apply_groups(map, sprites, assets, claimed, {
        source = function(code)
            return code == "Rr"
        end,
        target = ordinary_target("Rr"),
        stem = "road",
        order = -320,
        channel = "transition",
        lengths = { 6, 3, 2, 1 },
    })
    apply_groups(map, sprites, assets, claimed, {
        source = function(code)
            return code == "Rp"
        end,
        target = ordinary_target("Rp"),
        stem = "stone-path",
        order = -322,
        channel = "transition",
        lengths = { 6, 3, 2, 1 },
    })
    apply_groups(map, sprites, assets, claimed, {
        source = function(code)
            return code ~= ""
                and code ~= "Rd"
                and not starts_with(code, "Rr")
                and not starts_with(code, "H")
                and not starts_with(code, "M")
                and not starts_with(code, "Q")
                and not starts_with(code, "D")
                and not starts_with(code, "T")
                and not is_grass(code)
        end,
        target = function(code)
            return code == "Rd"
        end,
        stem = "desert-road",
        order = -370,
        channel = "inside",
        lengths = { 6, 4, 3, 2, 1 },
    })
    apply_groups(map, sprites, assets, claimed, {
        source = function(code)
            return code == "Rd"
        end,
        target = function(code)
            return code ~= ""
                and code ~= "Rd"
                and not starts_with(code, "W")
                and code ~= "Ai"
                and not starts_with(code, "Q")
                and not starts_with(code, "D")
                and not starts_with(code, "T")
                and not is_grass(code)
        end,
        stem = "desert-road",
        order = -371,
        channel = "transition",
        lengths = { 6, 4, 3, 2, 1 },
    })

    return { assets = assets, static = sprites, animated = {} }
end
