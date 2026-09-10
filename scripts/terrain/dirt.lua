local U32 = 0xffffffff

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

local variants = { "base", "variant2", "variant3", "variant4", "variant5", "variant6", "variant7" }
local variant_rule = hash_string("flat/dirt@V.png")

return function(map)
    local sprites = {}
    for _, tile in ipairs(map.tiles) do
        local x = tile.x - 1
        local y = tile.y - 1
        local choice = (noise(x, y, variant_rule) // 7919) % #variants + 1
        sprites[#sprites + 1] = {
            asset = variants[choice],
            x = tile.x,
            y = tile.y,
            offset_x = -36,
            offset_y = -36,
            order = -1000,
        }
    end

    return {
        assets = {
            base = "assets/wesnoth/terrain/flat/dirt.png",
            variant2 = "assets/wesnoth/terrain/flat/dirt2.png",
            variant3 = "assets/wesnoth/terrain/flat/dirt3.png",
            variant4 = "assets/wesnoth/terrain/flat/dirt4.png",
            variant5 = "assets/wesnoth/terrain/flat/dirt5.png",
            variant6 = "assets/wesnoth/terrain/flat/dirt6.png",
            variant7 = "assets/wesnoth/terrain/flat/dirt7.png",
        },
        static = sprites,
        animated = {},
    }
end
