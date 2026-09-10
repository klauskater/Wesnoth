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
    code = code:match("%S+$") or code
    return code:match("^[^^]+") or code
end

local function overlay_code(code)
    return code and (code:match("%^(.*)$") or "") or ""
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

local function choose(stem, variants, x, y, rule)
    local variant = (noise(x - 1, y - 1, rule or stem) // 7919) % variants + 1
    return stem .. (variant == 1 and "" or variant)
end

local function register(assets, name)
    assets[name] = "assets/wesnoth/terrain/decorations/" .. name .. ".png"
    return name
end

local function emit(sprites, assets, name, x, y, order)
    sprites[#sprites + 1] = {
        asset = register(assets, name),
        x = x,
        y = y,
        offset_x = -36,
        offset_y = -36,
        order = order,
        pass = "ground",
    }
end

local function edge_key(ax, ay, bx, by)
    if ax > bx or (ax == bx and ay > by) then
        ax, ay, bx, by = bx, by, ax, ay
    end
    return ax .. "," .. ay .. ":" .. bx .. "," .. by
end

local function spill(map, sprites, assets, claimed, overlay, prefix, order, target)
    for _, tile in ipairs(map.tiles) do
        if tile.terrain == overlay then
            for direction, neighbor in ipairs(neighbors(tile.x, tile.y)) do
                local raw = cell(map, neighbor[1], neighbor[2])
                local key = edge_key(tile.x, tile.y, neighbor[1], neighbor[2])
                if raw and target(raw) and not claimed[key] then
                    local suffix = DIRECTIONS[(direction + 2) % 6 + 1]
                    local available = prefix ~= "farm-veg-spring"
                        or suffix == "n"
                        or suffix == "se"
                        or suffix == "s"
                        or suffix == "nw"
                    if available then
                        claimed[key] = true
                        local name = prefix .. "-" .. suffix
                        if prefix == "flowers-mixed"
                            and noise(tile.x - 1, tile.y - 1, name) % 2 == 1
                        then
                            name = "flowers-mixed2-" .. suffix
                        end
                        emit(sprites, assets, name, neighbor[1], neighbor[2], order)
                    end
                end
            end
        end
    end
end

local function fence_asset(map, tile)
    local connected = {}
    local adjacent = neighbors(tile.x, tile.y)
    for _, direction in ipairs({ 2, 3, 5, 6 }) do
        if overlay_code(cell(map, adjacent[direction][1], adjacent[direction][2])) == "Eff" then
            connected[#connected + 1] = DIRECTIONS[direction]
        end
    end
    if #connected == 0 then
        return "fence-ne-sw-01"
    end
    local suffix = table.concat(connected, "-")
    if suffix == "ne-sw" or suffix == "se-nw" then
        local variant = noise(tile.x - 1, tile.y - 1, "embellishments/fence-" .. suffix) % 2 + 1
        return "fence-" .. suffix .. "-0" .. variant
    end
    return "fence-" .. suffix
end

local function is_remains(map, x, y)
    return overlay_code(cell(map, x, y)) == "Edb"
end

return function(map)
    local assets = {}
    local sprites = {}
    local animated = {}
    local remains = {}

    for _, tile in ipairs(map.tiles) do
        if tile.terrain == "Efm" then
            emit(
                sprites,
                assets,
                choose("flowers-mixed", 4, tile.x, tile.y, "embellishments/flowers-mixed@V"),
                tile.x,
                tile.y,
                -500
            )
        elseif tile.terrain == "Gvs" then
            emit(
                sprites,
                assets,
                choose("farm-veg-spring", 3, tile.x, tile.y),
                tile.x,
                tile.y,
                -81
            )
        elseif tile.terrain == "Es" then
            emit(sprites, assets, choose("stones-small", 10, tile.x, tile.y), tile.x, tile.y, 0)
        elseif tile.terrain == "Em" then
            emit(sprites, assets, choose("mushroom", 7, tile.x, tile.y), tile.x, tile.y, 0)
        elseif tile.terrain == "Eff" then
            emit(sprites, assets, fence_asset(map, tile), tile.x, tile.y, -80)
        elseif tile.terrain == "Edb" then
            remains[#remains + 1] = tile
            local use_scatter = noise(tile.x - 1, tile.y - 1, "misc/detritus/detritusA") % 100 < 33
            local name = use_scatter
                and ("detritusA-" .. (noise(tile.x - 1, tile.y - 1, "detritusA-variant") % 5 + 1))
                or choose("liter", 6, tile.x, tile.y)
            emit(sprites, assets, name, tile.x, tile.y, -201)
        elseif tile.terrain == "Wm" then
            local frames = {}
            for frame = 1, 18 do
                local name = string.format("windmill-%02d", frame)
                frames[#frames + 1] = register(assets, name)
            end
            local frame_ms = noise(tile.x - 1, tile.y - 1, "misc/decorative/windmill") % 100 < 33
                and 30
                or 50
            animated[#animated + 1] = {
                frames = frames,
                frame_ms = frame_ms,
                phase_ms = noise(tile.x - 1, tile.y - 1, "windmill-phase") % (frame_ms * 18),
                x = tile.x,
                y = tile.y,
                offset_x = -36,
                offset_y = -36,
                order = 0,
                pass = "ground",
            }
        end
    end

    local medium = {}
    for _, tile in ipairs(remains) do
        local count = 0
        for _, neighbor in ipairs(neighbors(tile.x, tile.y)) do
            if is_remains(map, neighbor[1], neighbor[2]) then
                count = count + 1
            end
        end
        local key = tile.x .. "," .. tile.y
        medium[key] = count >= 2
        if medium[key] and noise(tile.x - 1, tile.y - 1, "detritus-medium") % 100 < 70 then
            local variant = noise(tile.x - 1, tile.y - 1, "detritusB-variant") % 16 + 1
            emit(sprites, assets, "detritusB-" .. variant, tile.x, tile.y, -200)
        end
    end
    for _, tile in ipairs(remains) do
        local medium_neighbors = 0
        for _, neighbor in ipairs(neighbors(tile.x, tile.y)) do
            if medium[neighbor[1] .. "," .. neighbor[2]] then
                medium_neighbors = medium_neighbors + 1
            end
        end
        if medium_neighbors >= 2
            and noise(tile.x - 1, tile.y - 1, "detritus-big") % 100 < 50
        then
            local variant = noise(tile.x - 1, tile.y - 1, "detritusC-variant") % 7 + 1
            emit(sprites, assets, "detritusC-" .. variant, tile.x, tile.y, -199)
        end
    end

    spill(map, sprites, assets, {}, "Efm", "flowers-mixed", -240, function(raw)
        return starts_with(base_code(raw), "G")
    end)
    spill(map, sprites, assets, {}, "Gvs", "farm-veg-spring", -330, function(raw)
        local base = base_code(raw)
        local overlay = overlay_code(raw)
        return overlay ~= "Gvs"
            and not starts_with(base, "C")
            and not starts_with(base, "K")
            and not starts_with(overlay, "F")
            and not starts_with(base, "M")
            and not starts_with(base, "H")
            and not starts_with(base, "W")
            and not starts_with(base, "Q")
    end)

    return { assets = assets, static = sprites, animated = animated }
end
