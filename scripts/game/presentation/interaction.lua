-- Viewer-local input interpretation. Contract: contracts/target/modules/lua/interaction.md
local interaction = {}

function interaction.interpret(_, request)
    local context = request.view_context or {}
    local input = request.input
    if input.kind == "hover" then context.hovered = input.target end
    if input.kind == "dismiss" then context.modal = nil end
    if input.payload and input.payload.query then
        context.query = {
            id = input.interaction_id,
            action = input.payload.query,
            payload = input.payload.payload,
        }
    end
    local action = input.payload and input.payload.action
    local command = action and { action = action, payload = input.payload.payload } or nil
    return { view_context = context, command = command }
end

return interaction
