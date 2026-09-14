-- Package-owned interpretation of raw map cell codes.
local terrain = {}

function terrain.kind(code)
    local base, overlay = code:match("^%d*%s*([^%^]+)%^?(.*)$")
    if overlay:sub(1, 1) == "B" then return "grassland" end
    if overlay:sub(1, 1) == "V" then return "village" end
    if overlay:sub(1, 1) == "F" then return "forest" end
    if base:sub(1, 1) == "K" then return "keep" end
    if base:sub(1, 1) == "C" then return "castle" end
    if base:sub(1, 2) == "Wo" then return "deep_water" end
    if base:sub(1, 1) == "W" then return "water" end
    if base:sub(1, 1) == "S" then return "swamp_water" end
    if base:sub(1, 1) == "M" then return "mountains" end
    if base:sub(1, 1) == "H" then return "hills" end
    if code == "forest" or code == "hills" or code == "water"
        or code == "castle" or code == "keep" or code == "village" then return code end
    return "grassland"
end

function terrain.install(context)
    if context.map.raw then return context end
    context.map.raw = context.map.get
    context.map.get = function(self, position)
        return terrain.kind(self:raw(position))
    end
    return context
end

return terrain
