-- Declarative game UI. Contract: contracts/target/modules/lua/hud.md
local hud = {}

function hud.build(status)
    return {
        id = "hud",
        kind = "column",
        children = {
            {
                id = "end_turn",
                kind = "button",
                text = "Закончить ход",
                enabled = status.can_end_turn == true,
                action = { action = "end_turn", payload = wesnoth.value.null },
            },
        },
    }
end

return hud
