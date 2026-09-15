-- Package-owned campaign projection and transitions.
-- Contract: contracts/target/modules/lua/campaign.md
local campaign = {}

function campaign.campaign_state(context)
    local scenario = assert(context.state:get("scenario"), "missing scenario state")
    local side = assert(scenario.campaign_side, "scenario has no campaign_side")
    local units = wesnoth.value.list()
    for _, object in ipairs(context.objects:all()) do
        if object.side == side then units[#units + 1] = object end
    end
    for _, object in ipairs(context.state:get("recall") or {}) do
        units[#units + 1] = object
    end

    local variables = {}
    for key, value in pairs(context.state:all()) do
        if key:sub(1, 9) == "campaign:" then variables[key] = value end
    end
    local percentage = scenario.carryover_percentage or 0
    return {
        units = units,
        gold = (context.state:get("gold:" .. side) or 0) * percentage // 100,
        variables = variables,
    }
end

function campaign.next_scenario(context)
    local scenario = assert(context.state:get("scenario"), "missing scenario state")
    return scenario.next_scenario or wesnoth.value.null
end

campaign.carryover = campaign.campaign_state

function campaign.transition_data(context)
    return {
        next_scenario = campaign.next_scenario(context),
        carryover = campaign.carryover(context),
    }
end

return campaign
