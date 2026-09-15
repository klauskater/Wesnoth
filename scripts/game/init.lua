-- Package entry. Contract: contracts/target/modules/lua/entry.md
local bootstrap = require("game.bootstrap")
local campaign = require("game.campaign")
local rules = require("rules.basic_combat")
local terrain = require("game.rules.terrain")
local combat = require("game.rules.combat")
local interaction = require("game.presentation.interaction")
local presenter = require("game.presentation.presenter")

local entry = {}
local function prepare(context)
    terrain.install(context)
    context.combat = combat
    return context
end

function entry.initialize(context, request)
    prepare(context)
    return rules.initialize(context, bootstrap.create_scenario(context, request))
end

function entry.dispatch(context, command)
    prepare(context)
    local action = assert(command.action, "dispatch requires action")
    local pending_dialog = context.state:get("pending_dialog")
    if action == "dismiss_dialog" then
        assert(pending_dialog, "no dialog is waiting for the UI")
        context.state:set("pending_dialog", nil)
        return wesnoth.value.list()
    end
    assert(not pending_dialog, "dialog is waiting for the UI")
    local operation = assert(rules[action], "unknown action: " .. tostring(action))
    local result = operation(context, command.payload)
    local dialog = result and result.dialog
    if not dialog then
        for _, event in ipairs(result or {}) do
            if event.dialog then dialog = event.dialog break end
        end
    end
    if dialog then context.state:set("pending_dialog", dialog) end
    return result
end

function entry.interact(context, request)
    prepare(context)
    local result = interaction.interpret(context, request)
    local query = result.view_context.query
    if query then
        local operation = assert(campaign[query.action] or rules[query.action],
            "unknown query: " .. tostring(query.action))
        result.view_context.query_result = {
            interaction_id = query.id,
            value = operation(context, query.payload),
        }
        result.view_context.query = nil
    end
    return result
end

function entry.present(context, request)
    return presenter.present(prepare(context), request)
end

function entry.validate_restored(context)
    prepare(context)
    bootstrap.validate_restored(context)
    rules.status(context)
    rules.snapshot(context)
    return true
end

return entry
