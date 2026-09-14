-- Visible effect translation. Contract: contracts/target/modules/lua/effects.md
local effects = {}

function effects.translate(events, command_id, viewer)
    local result = wesnoth.value.list()
    for index, event in ipairs(events or {}) do
        result[index] = {
            id = command_id .. ":" .. index .. ":" .. viewer,
            content = event,
        }
    end
    return result
end

return effects
