-- Replace-whole-block presentation. Contract: contracts/target/modules/lua/presenter.md
local rules = require("rules.basic_combat")
local effects = require("game.presentation.effects")
local hud = require("game.presentation.hud")
local presenter = {}

local function same_position(a, b)
    return a and b and a.x == b.x and a.y == b.y
end

local function contains(values, wanted)
    for _, value in ipairs(values or {}) do
        if value == wanted then return true end
    end
    return false
end

local function visible(status, wanted)
    return contains(status.visible_units, wanted)
end

local function terrain_label(code, kind)
    local base, overlay = code:match("^%d*%s*([^%^]+)%^?(.*)$")
    if overlay:sub(1, 1) == "B" then return "Мост" end
    if overlay:sub(1, 1) == "V" or code == "village" then return "Деревня" end
    if overlay:sub(1, 1) == "F" or code == "forest" then return "Лес" end
    if base:sub(1, 1) == "K" or code == "keep" then return "Цитадель" end
    if base:sub(1, 1) == "C" or code == "castle" then return "Замок" end
    if base:sub(1, 2) == "Wo" then return "Глубокая вода" end
    if base:sub(1, 1) == "S" then return "Болото" end
    if base:sub(1, 1) == "D" then return "Песок" end
    if base:sub(1, 1) == "A" then return "Снег" end
    if base:sub(1, 1) == "M" then return "Горы" end
    if base:sub(1, 1) == "R" then return "Дорога" end
    if kind == "water" then return "Мелководье" end
    if kind == "hills" then return "Холмы" end
    return "Равнина"
end

local function navigation_block(context, status, view_context)
    if not status.can_end_turn or status.pending_choice or status.pending_advancement then
        return nil
    end
    local objects = context.objects:all()
    local selected = view_context and view_context.selection
        and view_context.selection.object
    local start = 1
    for index, object in ipairs(objects) do
        if object.id == selected then start = index + 1 break end
    end
    for offset = 0, #objects - 1 do
        local object = objects[(start + offset - 1) % #objects + 1]
        if object.side == status.active_side and visible(status, object.id)
            and (object.movement_points > 0 or object.attacks_left > 0) then
            return { next_unit = { position = object.position } }
        end
    end
end

local function selection_block(context, status, view_context)
    local selection = view_context and view_context.selection
    if not selection then return nil end
    local result = { position = selection.position, object = selection.object }
    if selection.object and not status.finished then
        if not visible(status, selection.object) then return nil end
        local object = context.objects:get(selection.object)
        if not object then return nil end
        result.position = object.position
        result.unit = { type = object.type }
        result.unit_ui = hud.unit(object)
        result.reachable = wesnoth.value.list()
        result.path = wesnoth.value.list()
        result.reachable[#result.reachable + 1] = object.position
        local cells = rules.reachable(context, {
            object = object.id,
            inspect = object.side ~= status.active_side,
            paths = selection.preview ~= nil,
        })
        for _, cell in ipairs(cells) do
            result.reachable[#result.reachable + 1] = cell.position
            if same_position(cell.position, selection.preview) then
                result.path[#result.path + 1] = object.position
                for _, step in ipairs(cell.path or {}) do result.path[#result.path + 1] = step end
                local defense = object.__children and object.__children.defense
                    and object.__children.defense[1]
                result.defense = defense and defense[context.map:get(cell.position)]
            end
        end
    end
    local code = context.map:raw(result.position)
    result.terrain = { code = code, label = terrain_label(code, context.map:get(result.position)) }
    return result
end

local function attack_block(context, status, view_context)
    local attack = view_context and view_context.attack
    if not attack or status.finished or not status.can_end_turn
        or status.pending_choice or status.pending_advancement
        or not visible(status, attack.attacker) or not visible(status, attack.defender) then
        return nil
    end
    local attacker = context.objects:get(attack.attacker)
    local defender = context.objects:get(attack.defender)
    if not attacker or not defender or attacker.side ~= status.active_side
        or not rules.are_enemies(context, attacker.side, defender.side) then return nil end
    local actions = rules.actions(context, { object = attacker.id, paths = false })
    if not contains(actions.targets, defender.id) then return nil end
    local choices = wesnoth.value.list()
    for _, weapon in ipairs(actions.attacks or {}) do
        local preview = rules.preview_attack(context, {
            attacker = attacker.id, defender = defender.id, weapon = weapon,
        })
        if not preview.disabled then
            choices[#choices + 1] = { weapon = weapon, preview = preview }
        end
    end
    if #choices == 0 then return nil end
    return { attacker = attacker.id, defender = defender.id, choices = choices }
end

function presenter.present(context, request)
    local status = rules.status(context)
    local blocks = {
        { id = "status", content = status },
        { id = "objects", content = rules.snapshot(context) },
        { id = "hud", content = { schema = "ui", root = hud.build(status) } },
        { id = "summary", content = { schema = "ui", root = hud.summary(status) } },
        { id = "time", content = hud.time(status) },
        { id = "scene", content = assert(context.state:get("presentation:scene"), "missing scene") },
        { id = "map", content = assert(context.state:get("presentation:map"), "missing map presentation") },
        { id = "assets", content = assert(context.state:get("presentation:assets"), "missing asset registry") },
    }
    local query_result = request.view_context and request.view_context.query_result
    if query_result then
        table.insert(blocks, { id = "query", content = query_result })
    end
    local navigation = navigation_block(context, status, request.view_context)
    if navigation then table.insert(blocks, { id = "navigation", content = navigation }) end
    local selection = selection_block(context, status, request.view_context)
    if selection then
        local unit_ui = selection.unit_ui
        selection.unit_ui = nil
        table.insert(blocks, { id = "selection", content = selection })
        if unit_ui then
            table.insert(blocks, { id = "unit", content = { schema = "ui", root = unit_ui } })
        end
    end
    local recruit = hud.recruit(status, selection)
    if recruit then
        table.insert(blocks, { id = "recruit", content = { schema = "ui", root = recruit } })
    end
    local attack = attack_block(context, status, request.view_context)
    if attack then table.insert(blocks, { id = "attack", content = attack }) end
    local pending_dialog = context.state:get("pending_dialog")
    if pending_dialog then
        local dialogs = assert(context.state:get("presentation:dialogs"), "missing dialogs")
        table.insert(blocks, {
            id = "dialog",
            content = { schema = "ui", root = hud.dialog(
                assert(dialogs[pending_dialog], "unknown dialog: " .. pending_dialog),
                request.view_context and request.view_context.dialog_line or 1) },
        })
    end
    local remove_ids = wesnoth.value.list()
    if not pending_dialog then remove_ids[#remove_ids + 1] = "dialog" end
    if not navigation then remove_ids[#remove_ids + 1] = "navigation" end
    if not selection then remove_ids[#remove_ids + 1] = "selection" end
    if not selection or not selection.unit then remove_ids[#remove_ids + 1] = "unit" end
    if not recruit then remove_ids[#remove_ids + 1] = "recruit" end
    if not attack then remove_ids[#remove_ids + 1] = "attack" end
    return {
        replace_blocks = blocks,
        remove_ids = remove_ids,
        effects = effects.translate(request.events, request.command_id or "snapshot", request.viewer),
    }
end

return presenter
