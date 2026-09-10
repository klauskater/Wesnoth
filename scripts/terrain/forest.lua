local U32 = 0xffffffff

local FORESTS = {
    forest = {
        normal = { stem = "mixed-summer", variants = 2, small_variants = 1 },
    },
    Fp = {
        normal = { stem = "pine", variants = 4, small_variants = 2 },
        sparse = { stem = "pine-sparse", variants = 4, small_variants = 1 },
    },
    Fds = {
        normal = { stem = "deciduous-summer", variants = 4, small_variants = 1 },
        sparse = { stem = "deciduous-summer-sparse", variants = 3, small_variants = 1 },
    },
    Fdw = {
        normal = { stem = "deciduous-winter", variants = 2, small_variants = 4 },
        sparse = { stem = "deciduous-winter-sparse", variants = 3, small_variants = 2 },
    },
    Fms = {
        normal = { stem = "mixed-summer", variants = 2, small_variants = 1 },
        sparse = { stem = "mixed-summer-sparse", variants = 2, small_variants = 1 },
    },
    Fmw = {
        normal = { stem = "mixed-winter", variants = 2, small_variants = 1 },
        sparse = { stem = "mixed-winter-sparse", variants = 2, small_variants = 1 },
    },
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

local function overlay_code(code)
    return code:match("%^(.*)$") or ""
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

local function hard_neighbor(code)
    if not code then
        return true
    end
    local base = base_code(code)
    local overlay = overlay_code(code)
    return starts_with(base, "C")
        or starts_with(base, "K")
        or starts_with(base, "X")
        or starts_with(base, "Q")
        or starts_with(base, "W")
        or base == "Ai"
        or starts_with(base, "M")
        or starts_with(overlay, "Qh")
        or starts_with(overlay, "V")
        or starts_with(overlay, "B")
end

local function needs_small(map, x, y)
    for _, neighbor in ipairs(neighbors(x, y)) do
        if hard_neighbor(cell(map, neighbor[1], neighbor[2])) then
            return true
        end
    end
    return false
end

local function choose(stem, variants, x, y)
    local variant = (noise(x - 1, y - 1, hash_string("forest/" .. stem .. "@V.png")) // 7919)
        % variants
        + 1
    return stem .. (variant == 1 and "" or variant)
end

return function(map)
    local assets = {}
    local sprites = {}

    for _, tile in ipairs(map.tiles) do
        local name
        local offset_x
        local offset_y
        local baseline
        if tile.terrain == "Fet" then
            name = choose("great-tree", 3, tile.x, tile.y)
            offset_x = -36
            offset_y = -86
            baseline = 16
        else
            local family = FORESTS[tile.terrain]
            local base = base_code(tile.code)
            local config = family.normal
            if (starts_with(base, "H") or starts_with(base, "M")) and family.sparse then
                config = family.sparse
            end
            local small = needs_small(map, tile.x, tile.y)
            local stem = config.stem .. (small and "-small" or "")
            local variants = small and config.small_variants or config.variants
            name = choose(stem, variants, tile.x, tile.y)
            offset_x = -72
            offset_y = -72
            baseline = 17
        end

        assets[name] = "assets/wesnoth/terrain/forest/" .. name .. ".png"
        sprites[#sprites + 1] = {
            asset = name,
            x = tile.x,
            y = tile.y,
            offset_x = offset_x,
            offset_y = offset_y,
            baseline = baseline,
            order = 0,
            pass = "world",
        }
    end

    return { assets = assets, static = sprites, animated = {} }
end
