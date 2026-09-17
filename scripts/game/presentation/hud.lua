-- Declarative game UI. Contract: contracts/target/modules/lua/hud.md
local hud = {}

local times = {
    dawn = {
        label = "Рассвет", asset = "ui/time/dawn",
        tint = { 230 / 255, 240 / 255, 1, 1 },
        scene_dark = false,
    },
    morning = {
        label = "Утро", asset = "ui/time/morning",
        tint = { 1, 1, 1, 1 },
        scene_dark = false,
    },
    afternoon = {
        label = "День", asset = "ui/time/afternoon",
        tint = { 1, 1, 1, 1 },
        scene_dark = false,
    },
    dusk = {
        label = "Закат", asset = "ui/time/dusk",
        tint = { 1, 235 / 255, 220 / 255, 1 },
        scene_dark = true,
    },
    first_watch = {
        label = "Первая стража", asset = "ui/time/first-watch",
        tint = { 180 / 255 * 0.9, 210 / 255 * 0.9, 242 / 255 * 0.9, 1 }, scene_dark = true,
    },
    second_watch = {
        label = "Вторая стража", asset = "ui/time/second-watch",
        tint = { 180 / 255 * 0.9, 210 / 255 * 0.9, 242 / 255 * 0.9, 1 }, scene_dark = true,
    },
}

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

function hud.summary(status)
    return {
        id = "summary", kind = "text",
        text = string.format("Ход %d · Золото: %d", status.turn, status.gold),
    }
end

function hud.recruit(status, selection)
    if not status.can_end_turn or status.pending_choice or status.pending_advancement
        or not selection or not selection.position
        or #status.recruit_options + #status.recall_options == 0 then return nil end
    local destination
    for _, hex in ipairs(status.recruit_hexes) do
        if hex.x == selection.position.x and hex.y == selection.position.y then
            destination = selection.position
            break
        end
    end
    if not destination then return nil end
    local children = {
        {
            id = "recruit_title", kind = "text", grow = 1,
            text = string.format("Войска · Золото: %d", status.gold),
        },
    }
    for _, option in ipairs(status.recruit_options) do
        children[#children + 1] = {
            id = "recruit_" .. option.id, kind = "button", grow = 2,
            text = string.format("%s · %d зол.", option.name, option.cost),
            enabled = option.cost <= status.gold,
            action = {
                action = "recruit",
                payload = { unit_type = option.id, destination = destination },
            },
        }
    end
    for _, option in ipairs(status.recall_options) do
        children[#children + 1] = {
            id = "recall_" .. option.id, kind = "button", grow = 2,
            text = string.format("Призвать %s · %d зол.", option.name, option.cost),
            enabled = option.cost <= status.gold,
            action = {
                action = "recall",
                payload = { unit = option.id, destination = destination },
            },
        }
    end
    return { id = "recruit", kind = "column", children = children }
end

function hud.unit(object)
    local alignment = {
        lawful = "Порядочный", chaotic = "Хаотичный",
        neutral = "Нейтральный", liminal = "Сумеречный",
    }
    return {
        id = "unit", kind = "column",
        children = {
            { id = "unit_name", kind = "text", text = object.name, grow = 1 },
            { id = "unit_health", kind = "text", grow = 1,
                text = string.format("Здоровье: %d/%d", object.hitpoints, object.max_hitpoints) },
            { id = "unit_moves", kind = "text", grow = 1,
                text = string.format("Ходы: %d/%d · Атак: %d",
                    object.movement_points, object.max_moves, object.attacks_left) },
            { id = "unit_experience", kind = "text", grow = 1,
                text = string.format("Ур. %d · Опыт: %d/%d",
                    object.level, object.experience, object.max_experience) },
            { id = "unit_alignment", kind = "text", grow = 1,
                text = alignment[object.alignment] or object.alignment },
        },
    }
end

function hud.dialog(lines, index)
    local line = assert(lines[index or 1], "dialog page is out of range")
    return {
        id = "dialog", kind = "column",
        children = {
            { id = "dialog_speaker", kind = "text", text = line.speaker, grow = 1 },
            { id = "dialog_text", kind = "text", text = line.text, grow = 6 },
            { id = "dialog_continue", kind = "button", text = "Продолжить", grow = 1,
                action = { action = "advance_dialog", payload = wesnoth.value.null } },
        },
    }
end

function hud.time(status)
    local time = assert(times[status.time_of_day], "unknown time of day")
    local bonus = assert(status.lawful_bonus, "missing lawful bonus")
    local tooltip = "Нейтральное время суток"
    if bonus > 0 then
        tooltip = string.format(
            "Порядочные бойцы: +%d%% · хаотичные: %d%%", bonus, -bonus)
    elseif bonus < 0 then
        tooltip = string.format(
            "Хаотичные бойцы: +%d%% · порядочные: %d%%", -bonus, bonus)
    end
    return {
        schema = "ui",
        map_tint = time.tint,
        scene_dark = time.scene_dark,
        root = {
            id = "time", kind = "column",
            children = {
                { id = "time_label", kind = "text", text = time.label, grow = 1 },
                {
                    id = "time_image", kind = "image", asset = time.asset, grow = 4,
                    children = {
                        { id = "time_tooltip", kind = "tooltip", text = tooltip },
                    },
                },
            },
        },
    }
end

return hud
