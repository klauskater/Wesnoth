-- Replace-whole-block presentation. Contract: contracts/target/modules/lua/presenter.md
local rules = require("rules.basic_combat")
local effects = require("game.presentation.effects")
local hud = require("game.presentation.hud")
local presenter = {}

function presenter.present(context, request)
    local status = rules.status(context)
    local blocks = {
        { id = "status", content = status },
        { id = "objects", content = rules.snapshot(context) },
        { id = "hud", content = { schema = "ui", root = hud.build(status) } },
        { id = "scene", content = assert(context.state:get("presentation:scene"), "missing scene") },
    }
    local query_result = request.view_context and request.view_context.query_result
    if query_result then
        table.insert(blocks, { id = "query", content = query_result })
    end
    return {
        replace_blocks = blocks,
        remove_ids = wesnoth.value.list(),
        effects = effects.translate(request.events, request.command_id or "snapshot", request.viewer),
    }
end

return presenter
