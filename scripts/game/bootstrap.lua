-- Package-owned scenario construction.
-- Contract: contracts/target/modules/lua/bootstrap.md
local bootstrap = {}

local function children(node, name)
    return node.__children and node.__children[name] or {}
end

local function copy(source)
    local result = {}
    for key, value in pairs(source or {}) do result[key] = value end
    return result
end

function bootstrap.load_catalogs(nodes)
    local catalogs = { unit_types = {}, races = {}, traits = {} }
    local destinations = {
        unit_type = { values = catalogs.unit_types, label = "unit type" },
        race = { values = catalogs.races, label = "race" },
        trait = { values = catalogs.traits, label = "global trait" },
    }
    for _, node in ipairs(nodes) do
        local destination = destinations[node.__tag]
        assert(destination, "unsupported [" .. tostring(node.__tag)
            .. "] in unit type resource")
        local id = assert(node.id, destination.label .. " has no id")
        assert(not destination.values[id], "duplicate " .. destination.label .. " id: " .. id)
        node.__tag = nil
        destination.values[id] = node
    end
    return catalogs
end

local function add_objects(context, scenario, types, carryover)
    local carried = {}
    for _, unit in ipairs(carryover and carryover.units or {}) do
        carried[assert(unit.id, "carried unit has no id")] = unit
    end

    local placed = {}
    for _, unit in ipairs(children(scenario, "unit")) do
        local id = assert(unit.id, "scenario unit has no id")
        assert(not placed[id], "duplicate object id: " .. id)
        local template = assert(types[unit.type], "unknown unit type: " .. tostring(unit.type))
        local veteran = carried[id]
        local object = copy(veteran or template)
        object.id = nil
        for key, value in pairs(template) do
            if object[key] == nil then object[key] = value end
        end
        for key, value in pairs(unit) do
            if not (veteran and key == "type") then object[key] = value end
        end
        object.id = id
        context.objects:add(object)
        placed[id] = true
    end

    local recall = wesnoth.value.list()
    for _, unit in ipairs(carryover and carryover.units or {}) do
        if not placed[unit.id] then recall[#recall + 1] = unit end
    end
    context.state:set("recall", recall)
end

local function discover_villages(context)
    local villages = wesnoth.value.list()
    for _, cell in ipairs(context.map:cells()) do
        if context.map:get(cell.position) == "village" then
            villages[#villages + 1] = cell.position
        end
    end
    context.state:set("villages", villages)
end

function bootstrap.create_scenario(context, request)
    local scenario = assert(request.scenario, "bootstrap requires scenario data")
    local catalogs = bootstrap.load_catalogs(assert(request.catalogs,
        "bootstrap requires catalog data"))
    local carryover = request.carryover

    context.state:set("scenario", scenario)
    context.state:set("unit_types", catalogs.unit_types)
    context.state:set("races", catalogs.races)
    context.state:set("traits", catalogs.traits)
    context.state:set("pending_dialog", assert(scenario.on_start_dialog))
    context.state:set("session:scenario_path", assert(request.scenario_path))
    context.state:set("presentation:scene", assert(request.scene))
    context.state:set("presentation:map", assert(request.map))
    context.state:set("presentation:assets", assert(request.assets))
    context.state:set("presentation:dialogs", assert(request.dialogs))
    for key, value in pairs(carryover and carryover.variables or {}) do
        context.state:set(key, value)
    end
    add_objects(context, scenario, catalogs.unit_types, carryover)
    discover_villages(context)

    if carryover and scenario.campaign_side then
        return {
            campaign_side = scenario.campaign_side,
            campaign_gold = assert(carryover.gold),
        }
    end
    return wesnoth.value.null
end

function bootstrap.validate_restored(context)
    assert(context.state:get("scenario"), "missing scenario state")
    assert(context.state:get("unit_types"), "missing unit types")
    assert(context.state:get("races"), "missing races")
    assert(context.state:get("traits"), "missing traits")
    assert(context.state:get("villages"), "missing villages")
    assert(context.state:get("presentation:map"), "missing map presentation")
    assert(context.state:get("presentation:assets"), "missing asset registry")
    assert(context.state:get("presentation:dialogs"), "missing dialogs")
    return true
end

return bootstrap
