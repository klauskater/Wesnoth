
-- Port of core NEW:WATER_342_180, NEW:GENERIC_CORNER_TRANSITION and NEW:WAVES.
-- Rule priority and image layer are separate: all bases precede shore overlays.
local DIR = { "n", "ne", "se", "s", "sw", "nw" }
local CORNER = { "tr", "r", "br", "bl", "l", "tl" }
local function next_dir(d, n) return (d - 1 + n) % 6 + 1 end
local function neighbors(x, y)
    local up = x % 2 == 0 and -1 or 0
    return {{x,y-1},{x+1,y+up},{x+1,y+up+1},{x,y+1},{x-1,y+up+1},{x-1,y+up}}
end
local function starts(s, p) return s:sub(1, #p) == p end
local function water(c) return starts(c, "W") or c == "water" end
local function sea(c) return starts(c, "Wo") end
local function shallow(c) return starts(c, "Ww") or c == "water" end
local function sand(c) return starts(c, "D") or c == "Hd" or c == "Rrd" end
local function gray(c) return c == "Wog" or c == "Wwg" or c == "Wwrg" end
local function tropical(c) return c == "Wot" or c == "Wwt" or c == "Wwrt" end

local function sand_variant(x,y,stem)
    local hash=0
    local name="sand/"..stem.."@V.png"
    for i=1,#name do hash=(((hash<<9)|(hash>>23)) ~ name:byte(i)) & 0xffffffff end
    local a=((x-1+92872973) ~ 918273) & 0xffffffff
    local b=((y-1+1672517) ~ 128123) & 0xffffffff
    local c=((hash+127390) ~ 13923787) & 0xffffffff
    local mixed=(a*b*c+a*b+b*c+a*c+a+b+c) & 0xffffffff
    return (((mixed*mixed) & 0xffffffff)//7919)%8+1
end

return function(map)
    local assets, static, animated = {}, {}, {}
    local function code(x, y)
        local raw = map.cells[y] and map.cells[y][x] or ""
        return (raw:match("%S+$") or raw):match("^[^^]+") or ""
    end
    local function register(path)
        assets[path] = "assets/wesnoth/terrain/" .. path .. ".png"
        return path
    end
    local function each(fn)
        for x = 1, map.width do
            for y = 1, map.height do fn(x, y, code(x, y), neighbors(x,y)) end
        end
    end
    local function emit(path, x, y, order, masks, opacity, frames, crop, clips, offset)
        local command = { x=x, y=y, order=order, offset_x=offset and offset[1] or -36,
            offset_y=offset and offset[2] or -36, opacity=opacity, crop=crop, clips=clips }
        if masks then
            command.masks = {}
            for _, mask in ipairs(masks) do command.masks[#command.masks+1] = register("masks/" .. mask) end
        end
        if frames then
            command.frames, command.frame_ms = {}, starts(path, "water/waves") and 200 or 125
            for i=1,frames do command.frames[i] = register(path .. string.format("%02d",i)) end
            animated[#animated+1] = command
        else
            command.asset = register(path)
            static[#static+1] = command
        end
    end
    local function crop(x, y)
        local col = (x-1) % 6
        -- Client even columns are shifted UP; compensate in the sheet row.
        return {col*54, ((y-1 - col%2) % 2)*72 + col%2*36, 72,72}
    end
    local function water_image(c) return sea(c) and "water/ocean" or "water/water", sea(c) and 21 or 17 end
    each(function(x,y,c)
        if water(c) then
            local path, frames = water_image(c)
            emit(path,x,y,-999,nil,nil,frames,crop(x,y),{{x=x,y=y}})
            if gray(c) then emit("water/overlay-gray",x,y,-502) end
            if tropical(c) then emit("water/overlay-tropical",x,y,-504) end
            if c == "Wwf" then emit("water/ford",x,y,-519) end
        elseif c == "Ds" or c == "Dd" then
            local variant = sand_variant(x,y,c=="Ds" and "beach" or "desert")
            emit("sand/" .. (c=="Ds" and "beach" or "desert") .. (variant==1 and "" or variant),x,y,-1000)
        end
    end)

    -- NEW:TRANSITION chooses the longest available run, claiming each edge once.
    local function edges(source, target, stem, layer)
        each(function(x,y,c,adj)
            if not target(c) then return end
            local available={}
            for d,p in ipairs(adj) do available[d]=source(code(p[1],p[2])) end
            for _,length in ipairs(stem=="flat/bank-to-ice" and {2,1} or {6,4,3,2,1}) do
                for first=1,6 do
                    if length~=6 or first==1 then
                        local names, matches={},true
                        for n=0,length-1 do
                            local d=next_dir(first,n)
                            matches=matches and available[d]
                            names[#names+1]=DIR[d]
                        end
                        if matches then
                            for n=0,length-1 do available[next_dir(first,n)]=false end
                            emit(stem .. "-" .. table.concat(names,"-"),x,y,layer)
                        end
                    end
                end
            end
        end)
    end
    for _,entry in ipairs({{"Ds","beach"},{"Dd","desert"}}) do
        local source=function(c) return c==entry[1] end
        edges(source,function(c) return starts(c,"R") and c~="Rrd" end,"sand/"..entry[2],-319)
        edges(source,function(c)
            return c~="" and c~=entry[1] and not water(c) and not starts(c,"S")
                and c~="Ai" and not starts(c,"Q") and not starts(c,"R")
        end,"sand/"..entry[2],-510)
    end
    -- Existing hill banks own Mm/Hh edges. Flat banks must not cover beaches.
    edges(function(c)
        return (starts(c,"G") or starts(c,"R") or c=="Uue" or c=="Isa")
            and c~="Rra" and c~="Rrd"
    end,water,"flat/bank-to-ice",-483)

    local flags={}
    local function claim(x,y,channel,mask)
        local key=x..","..y..":"..channel..":"..mask
        if flags[key] then return false end
        flags[key]=true
        return true
    end
    -- Collect masks per target, as the original multi-corner ~BLIT rules do.
    local function corners(source,target,path,layer,opacity,frames,channel)
        local pending={}
        local function add(x,y,suffix)
            if not claim(x,y,channel,suffix) then return end
            local key=x..","..y
            pending[key]=pending[key] or {x=x,y=y,masks={}}
            local list=pending[key].masks
            list[#list+1]="long-"..suffix
        end
        each(function(x,y,c,adj)
            for d,p in ipairs(adj) do
                local nextp=adj[next_dir(d,1)]
                local a,b=code(p[1],p[2]),code(nextp[1],nextp[2])
                local rotation=CORNER[d]
                if target(c) and source(a) and source(b) then
                    add(x,y,"concave-2-"..rotation)
                end
                if source(c) and target(a) and not source(b) then
                    add(p[1],p[2],"convex-"..rotation.."-"..CORNER[next_dir(d,-1)])
                end
                if source(c) and not source(a) and target(b) then
                    add(nextp[1],nextp[2],"convex-"..rotation.."-"..CORNER[next_dir(d,1)])
                end
            end
        end)
        -- Never iterate a hash table to determine sprite order.
        each(function(x,y)
            local group=pending[x..","..y]
            if group then
                emit(path,x,y,layer,group.masks,opacity,frames,frames and crop(x,y) or nil,{{x=x,y=y}})
            end
        end)
    end
    local function bottom(c)
        return c~="" and not sand(c) and not water(c) and not starts(c,"S")
            and not starts(c,"A") and not starts(c,"Qx") and c~="Xv"
            and c~="Chw" and c~="Khw" and c~="Khs" and c~="Rra" and not starts(c,"_")
    end
    corners(bottom,water,"water/bottom",-500,255,nil,"corner")
    corners(function(c) return c=="Wwf" end,function(c) return (water(c) and c~="Wwf") or c=="Sm" end,
        "water/ford",-515,122,nil,"corner")
    corners(sea,function(c) return (water(c) and not sea(c)) or c=="Sm" end,
        "water/ocean",-550,127,21,"water")
    corners(shallow,function(c) return (water(c) and not shallow(c)) or c=="Sm" end,
        "water/water",-551,127,17,"water")
    corners(gray,function(c) return (water(c) and not gray(c)) or c=="Sm" end,
        "water/overlay-gray",-503,51,nil,"corner")
    corners(tropical,function(c) return (water(c) and not tropical(c)) or c=="Sm" end,
        "water/overlay-tropical",-505,41,nil,"corner")

    -- NEW:WAVES: two ordinary corner rules and four sand/water/other rules.
    each(function(x,y,c,adj)
        for d,p in ipairs(adj) do
            local q=adj[next_dir(d,1)]
            local a,b=code(p[1],p[2]),code(q[1],q[2])
            local shape, mask, excluded
            if sand(c) and water(a) and water(b) then
                shape,mask="convex",CORNER[d]
            elseif water(c) and sand(a) and sand(b) then
                shape,mask="concave",CORNER[d]
            elseif sand(c) and water(a) and not sand(b) and not water(b) then
                shape,mask,excluded="convex",CORNER[d],{[0]=true,[next_dir(d,1)]=true}
            elseif water(c) and not sand(a) and not water(a) and sand(b) then
                shape,mask,excluded="concave",CORNER[d].."-"..CORNER[next_dir(d,1)],{[0]=true,[d]=true}
            elseif sand(c) and not sand(a) and not water(a) and water(b) then
                shape,mask,excluded="convex",CORNER[d],{[0]=true,[d]=true}
            elseif water(c) and sand(a) and not sand(b) and not water(b) then
                shape,mask,excluded="concave",CORNER[d].."-"..CORNER[next_dir(d,-1)],{[0]=true,[next_dir(d,1)]=true}
            end
            if shape then
                local clips={}
                if not excluded then clips[1]={x=x,y=y} end
                for i,n in ipairs(adj) do
                    if not excluded or not excluded[i] then clips[#clips+1]={x=n[1],y=n[2]} end
                end
                emit("water/waves-"..shape.."-A",x,y,-499,{"7hex-"..mask},nil,13,nil,clips,{-90,-108})
            end
        end
    end)
    return {assets=assets,static=static,animated=animated}
end
