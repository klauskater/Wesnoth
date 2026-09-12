local U32 = 0xffffffff
local DIRECTIONS = { "n", "ne", "se", "s", "sw", "nw" }

local GRASS = {
    grassland = { code = "Gg", stem = "green", variants = 8, standard = 20 },
    Gg = { code = "Gg", stem = "green", variants = 8, standard = 20 },
    Gs = { code = "Gs", stem = "semi-dry", variants = 6, standard = 25 },
    Gd = { code = "Gd", stem = "dry", variants = 6, standard = 25 },
    Gll = { code = "Gll", stem = "leaf-litter", variants = 6 },
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

local function set(values)
    local result = {}
    for _, value in ipairs(values) do
        result[value] = true
    end
    return result
end

local function edge_key(ax, ay, bx, by)
    if ax > bx or (ax == bx and ay > by) then
        ax, ay, bx, by = bx, by, ax, ay
    end
    return ax .. "," .. ay .. ":" .. bx .. "," .. by
end

local function claim(claimed, channel, ax, ay, bx, by)
    local key = edge_key(ax, ay, bx, by)
    if claimed[channel][key] then
        return false
    end
    claimed[channel][key] = true
    return true
end

local function asset(assets, name)
    assets[name] = "assets/wesnoth/terrain/grass/" .. name .. ".png"
    return name
end

local function emit(sprites, assets, name, x, y, order)
    sprites[#sprites + 1] = {
        asset = asset(assets, name),
        x = x,
        y = y,
        offset_x = -36,
        offset_y = -36,
        order = order,
    }
end

local function base_asset(tile)
    local config = GRASS[tile.terrain]
    local x = tile.x - 1
    local y = tile.y - 1
    local variant
    if config.standard
        and noise(x, y, hash_string("grass/" .. config.stem .. ".png")) % 100 < config.standard
    then
        variant = 1
    else
        variant = (noise(x, y, hash_string("grass/" .. config.stem .. "@V.png")) // 7919)
            % config.variants
            + 1
    end
    return config.stem .. (variant == 1 and "" or variant)
end

local function apply_edge_rule(map, sprites, assets, claimed, rule)
    for _, tile in ipairs(map.tiles) do
        local source = GRASS[tile.terrain].code
        if rule.sources[source] then
            for direction, neighbor in ipairs(neighbors(tile.x, tile.y)) do
                local target = base_code(cell(map, neighbor[1], neighbor[2]) or "")
                if target ~= ""
                    and not starts_with(target, "S")
                    and rule.target(target)
                    and claim(claimed, rule.channel, tile.x, tile.y, neighbor[1], neighbor[2])
                then
                    local suffix = DIRECTIONS[(direction + 2) % 6 + 1]
                    emit(
                        sprites,
                        assets,
                        rule.prefix .. "-" .. suffix,
                        neighbor[1],
                        neighbor[2],
                        rule.order
                    )
                end
            end
        end
    end
end

local function apply_leaf_litter_edges(map, sprites, assets, claimed)
    local function target_matches(code)
        return code ~= ""
            and not starts_with(code, "S")
            and code ~= "Gll"
            and not starts_with(code, "Q")
            and not starts_with(code, "W")
            and code ~= "Ai"
            and not starts_with(code, "C")
            and not starts_with(code, "K")
    end

    for y = 1, map.height do
        for x = 1, map.width do
            local target = base_code(cell(map, x, y) or "")
            if target_matches(target) then
                local available = {}
                local adjacent = neighbors(x, y)
                for direction, neighbor in ipairs(adjacent) do
                    local source = base_code(cell(map, neighbor[1], neighbor[2]) or "")
                    local key = edge_key(x, y, neighbor[1], neighbor[2])
                    available[direction] = source == "Gll" and not claimed.transition[key]
                end

                for length = 3, 1, -1 do
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
                                claim(claimed, "transition", x, y, neighbor[1], neighbor[2])
                            end
                            emit(
                                sprites,
                                assets,
                                "leaf-litter-" .. table.concat(suffixes, "-"),
                                x,
                                y,
                                -270
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
        emit(sprites, assets, base_asset(tile), tile.x, tile.y, -1000)
    end

    local inside_targets = {
        Gs = set({ "Gg", "Gd", "Gll", "Re", "Rb", "Rd", "Rp", "Exos" }),
        Gg = set({ "Gs", "Gd", "Gll", "Re", "Rb", "Rd", "Rp", "Exos" }),
        Gd = set({ "Gg", "Gs", "Gll", "Re", "Rb", "Rd", "Rp", "Exos" }),
        Gll = set({ "Gg", "Gs", "Gd", "Re", "Rb", "Rd", "Rp", "Exos" }),
    }
    for _, entry in ipairs({
        { code = "Gs", stem = "semi-dry-long", order = -250 },
        { code = "Gg", stem = "green-long", order = -251 },
        { code = "Gd", stem = "dry-long", order = -252 },
        { code = "Gll", stem = "leaf-litter-long", order = -253 },
    }) do
        apply_edge_rule(map, sprites, assets, claimed, {
            sources = set({ entry.code }),
            target = function(code)
                return inside_targets[entry.code][code] == true
            end,
            prefix = entry.stem,
            order = entry.order,
            channel = "inside",
        })
    end

    local grass_targets = {
        Gll = set({ "Gg", "Gs", "Gd" }),
        Gd = set({ "Gg", "Gs", "Gll" }),
        Gg = set({ "Gs", "Gd", "Gll" }),
        Gs = set({ "Gg", "Gd", "Gll" }),
    }
    for _, entry in ipairs({
        { code = "Gll", stem = "leaf-litter-long", order = -254 },
        { code = "Gd", stem = "dry-long", order = -255 },
        { code = "Gg", stem = "green-long", order = -256 },
        { code = "Gs", stem = "semi-dry-long", order = -257 },
    }) do
        apply_edge_rule(map, sprites, assets, claimed, {
            sources = set({ entry.code }),
            target = function(code)
                return grass_targets[entry.code][code] == true
            end,
            prefix = entry.stem,
            order = entry.order,
            channel = "transition",
        })
    end

    local function medium_target(code)
        return starts_with(code, "R")
            or starts_with(code, "D")
            or code == "Aa"
            or code == "Ur"
            or code == "Urc"
            or code == "Isa"
    end
    for _, entry in ipairs({
        { code = "Gs", stem = "semi-dry-medium", order = -260 },
        { code = "Gg", stem = "green-medium", order = -261 },
        { code = "Gd", stem = "dry-medium", order = -262 },
    }) do
        apply_edge_rule(map, sprites, assets, claimed, {
            sources = set({ entry.code }),
            target = medium_target,
            prefix = entry.stem,
            order = entry.order,
            channel = "transition",
        })
    end

    apply_leaf_litter_edges(map, sprites, assets, claimed)

    local function green_abrupt_target(code)
        return not starts_with(code, "Gg")
            and not starts_with(code, "Q")
            and code ~= "Mm"
            and code ~= "Ms"
            and code ~= "Hh"
            and not starts_with(code, "C")
            and not starts_with(code, "K")
    end
    local function semi_dry_abrupt_target(code)
        return code ~= "Gs"
            and not starts_with(code, "Q")
            and not starts_with(code, "C")
            and not starts_with(code, "K")
    end
    local function dry_abrupt_target(code)
        return code ~= "Gd"
            and not starts_with(code, "Q")
            and not starts_with(code, "C")
            and not starts_with(code, "K")
    end
    for _, entry in ipairs({
        { code = "Gg", stem = "green-abrupt", order = -271, target = green_abrupt_target },
        { code = "Gs", stem = "semi-dry-abrupt", order = -272, target = semi_dry_abrupt_target },
        { code = "Gd", stem = "dry-abrupt", order = -273, target = dry_abrupt_target },
    }) do
        apply_edge_rule(map, sprites, assets, claimed, {
            sources = set({ entry.code }),
            target = entry.target,
            prefix = entry.stem,
            order = entry.order,
            channel = "transition",
        })
    end

    return { assets = assets, static = sprites, animated = {} }
end
