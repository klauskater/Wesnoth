-- Viewer-local input interpretation. Contract: contracts/target/modules/lua/interaction.md
local rules = require("rules.basic_combat")
local interaction = {}

local function same_position(a, b)
    return a and b and a.x == b.x and a.y == b.y
end

local function visible_object_at(context, status, position)
    local visible = {}
    for _, id in ipairs(status.visible_units or {}) do visible[id] = true end
    for _, object in ipairs(context.objects:all()) do
        if visible[object.id] and same_position(object.position, position) then return object end
    end
end

local function contains(values, wanted)
    for _, value in ipairs(values or {}) do
        if value == wanted then return true end
    end
    return false
end

local function cell_click(context, view_context, target, world_revision)
    assert(type(target) == "table" and type(target.x) == "number"
        and type(target.y) == "number", "cell click target must be a position")
    context.map:get(target) -- Bounds validation belongs to the map boundary.
    if context.state:get("pending_dialog") then return nil end
    local status = rules.status(context)
    local selected = view_context.selection
    local object = visible_object_at(context, status, target)
    local selected_object = selected and selected.object
        and context.objects:get(selected.object) or nil

    if object then
        if selected_object and selected_object.side == status.active_side
            and object.id ~= selected_object.id
            and status.can_end_turn and not status.pending_choice
            and not status.pending_advancement
            and rules.are_enemies(context, selected_object.side, object.side)
            and contains(rules.actions(context, {
                object = selected_object.id, paths = false,
            }).targets, object.id) then
            selected.preview = nil
            view_context.attack = {
                attacker = selected_object.id,
                defender = object.id,
                world_revision = world_revision,
            }
            return nil
        end
        view_context.attack = nil
        if selected and selected.object == object.id then
            selected.preview = nil
        else
            view_context.selection = { position = target, object = object.id }
        end
        return nil
    end

    view_context.attack = nil
    if selected_object and selected_object.side == status.active_side
        and status.can_end_turn and not status.pending_choice
        and not status.pending_advancement then
        local reachable = rules.reachable(context, {
            object = selected_object.id, destination = target, paths = true,
        })
        if #reachable > 0 then
            if same_position(selected.preview, target)
                and selected.preview_revision == world_revision then
                view_context.selection = { position = target, object = selected_object.id }
                return { action = "move", payload = {
                    object = selected_object.id, destination = target,
                } }
            end
            selected.preview = target
            selected.preview_revision = world_revision
            return nil
        end
    end

    view_context.selection = { position = target }
end

function interaction.interpret(world, request)
    local context = request.view_context or {}
    local input = request.input
    if input.kind == "hover" then context.hovered = input.target end
    if input.kind == "dismiss" then
        context.modal = nil
        if input.target == "attack" then
            context.attack = nil
        elseif input.target == "selection" then
            context.selection = nil
            context.attack = nil
        else
            context.selection = nil
            context.attack = nil
        end
    end
    if input.payload and input.payload.query then
        context.query = {
            id = input.interaction_id,
            action = input.payload.query,
            payload = input.payload.payload,
        }
    end
    local action = input.payload and input.payload.action
    local command
    if action == "advance_dialog" then
        local dialog = assert(world.state:get("pending_dialog"), "no pending dialog")
        local dialogs = assert(world.state:get("presentation:dialogs"), "missing dialogs")
        local lines = assert(dialogs[dialog], "unknown dialog: " .. dialog)
        local next_line = (context.dialog_line or 1) + 1
        if next_line <= #lines then
            context.dialog_line = next_line
        else
            context.dialog_line = nil
            command = { action = "dismiss_dialog", payload = wesnoth.value.null }
        end
    elseif action then
        command = { action = action, payload = input.payload.payload }
    end
    if input.kind == "cell_click" then
        command = cell_click(world, context, input.target, assert(request.world_revision))
    end
    return { view_context = context, command = command }
end

return interaction
