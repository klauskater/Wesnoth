local DIR = { "n", "ne", "se", "s", "sw", "nw" }
local function opposite(d) return (d + 2) % 6 + 1 end
local function neighbors(x, y)
    local up = x % 2 == 0 and -1 or 0
    return {{x,y-1},{x+1,y+up},{x+1,y+up+1},{x,y+1},{x-1,y+up+1},{x-1,y+up}}
end
local function axis(code)
    if code:find("|", 1, true) then return 1 end
    if code:find("/", 1, true) then return 2 end
    return 3
end
local SHAPES = {
    ["1,4"]="n-s", ["2,5"]="ne-sw", ["3,6"]="se-nw",
    ["1,3"]="n-se", ["2,4"]="ne-s", ["3,5"]="se-sw",
    ["4,6"]="s-nw", ["1,5"]="sw-n", ["2,6"]="nw-ne",
    ["1,3,5"]="n-se-sw", ["2,4,6"]="ne-s-nw",
}

return function(map)
    local assets, sprites, cells, wood = {}, {}, {}, {}
    local function get(x,y)
        local key=x..","..y
        if not cells[key] then
            local raw=map.cells[y] and map.cells[y][x] or ""
            cells[key]={x=x,y=y,base=raw:match("^[^^]+") or "",
                overlay=raw:match("%^(.*)$") or "", connect={},away={}}
        end
        return cells[key]
    end
    local function adjacent(t,d)
        local p=neighbors(t.x,t.y)[d]
        return get(p[1],p[2])
    end
    local function emit(name,t,dx,dy,w,h)
        assets[name]="assets/wesnoth/terrain/bridge/"..name..".png"
        sprites[#sprites+1]={asset=name,x=t.x,y=t.y,
            offset_x=(dx or 0)-(w or 72)/2, offset_y=(dy or 0)-(h or 72)/2,order=0}
    end
    for _,tile in ipairs(map.tiles) do
        local t=get(tile.x,tile.y)
        if t.overlay:sub(1,2)=="Bw" then
            t.axis=axis(t.overlay)
            wood[#wood+1]=t
        end
    end
    -- LAYOUT_TRACKS_F: collinear connections, TRACK_AWAY, logical exits,
    -- then reciprocal TRACK_FINAL flags. Flags also live on neighboring land.
    for _,t in ipairs(wood) do
        for _,d in ipairs({t.axis,t.axis+3}) do
            if adjacent(t,d).axis==t.axis then t.connect[d]=true end
        end
    end
    for _,r in ipairs({
        {1,3,6,1,3},{1,2,2,1,5},{2,1,1,2,4},{2,3,3,2,6},
        {3,2,2,3,5},{3,1,4,3,1},{1,3,3,4,6},{1,2,5,4,2},
        {2,1,4,5,1},{2,3,6,5,3},{3,2,5,6,2},{3,1,1,6,4},
    }) do
        for _,t in ipairs(wood) do
            local exit,blocked=adjacent(t,r[3]),adjacent(t,r[4])
            if t.axis==r[1] and exit.axis==r[2] and not t.connect[r[4]]
                and not exit.away[r[5]] and not blocked.connect[opposite(r[4])]
            then
                t.away[r[4]]=true
                blocked.away[opposite(r[4])]=true
            end
        end
    end
    for _,t in ipairs(wood) do
        for _,d in ipairs({t.axis,t.axis+3}) do
            if not t.away[d] and not adjacent(t,d).away[opposite(d)] then t.connect[d]=true end
        end
    end
    for d=1,6 do
        for _,t in ipairs(wood) do
            if adjacent(t,d).axis and adjacent(t,d).connect[opposite(d)] then t.connect[d]=true end
        end
    end
    for _,t in ipairs(wood) do
        local directions={}
        for d=1,6 do if t.connect[d] then directions[#directions+1]=d end end
        local shape=SHAPES[table.concat(directions,",")]
        if not shape then
            directions={t.axis,t.axis+3}
            shape=SHAPES[table.concat(directions,",")]
        end
        t.drawn={}
        for _,d in ipairs(directions) do t.drawn[d]=true end
        emit("wood-"..shape,t)
    end
    for _,t in ipairs(wood) do
        for d=1,6 do
            local other=adjacent(t,d)
            if t.drawn[d] and not (other.drawn and other.drawn[opposite(d)])
                and other.base~="" and not other.base:match("^[CK]")
            then
                local dock=not other.drawn and (other.base:match("^W")
                    or other.base:match("^Ss") or other.base:match("^Ai"))
                emit((dock and "wood-dock-" or "wood-end-")..DIR[opposite(d)],other)
            end
        end
    end
    -- BRIDGE:STRAIGHTS and BRIDGE:ENDS place one image between two hexes.
    -- Their explicit WML centers are the midpoint, including diagonal spans.
    for _,tile in ipairs(map.tiles) do
        local t=get(tile.x,tile.y)
        if t.overlay:sub(1,3)=="Bsb" then
            local a=axis(t.overlay)
            for _,d in ipairs({a,a+3}) do
                local other=adjacent(t,d)
                local joined=other.overlay==t.overlay
                if not joined or d>3 then
                    local suffix=joined and ({"s-n","sw-ne","se-nw"})[a] or DIR[opposite(d)]
                    local prefix="stonebridge"
                    if not joined then
                        if other.base:match("^[CK]") and (d==1 or d==2 or d==6) then prefix=prefix.."-castle"
                        elseif other.base:match("^[CKX]") and (d==3 or d==4 or d==5) then prefix=prefix.."-short"
                        elseif other.base:match("^[WS]") or other.base:match("^Ql") then prefix=prefix.."-water" end
                    end
                    local w=a==1 and 180 or 126
                    local h=a==1 and 144 or 108
                    if prefix=="stonebridge-castle" and a==1 then w,h=72,72 end
                    if suffix=="se-nw" then h=118 end
                    if suffix=="se" and prefix~="stonebridge-castle" then h=128 end
                    local dx=(other.x-t.x)*54
                    local dy=(other.y-t.y)*72-(other.x%2==0 and 36 or 0)+(t.x%2==0 and 36 or 0)
                    emit(prefix.."-"..suffix,t,dx/2,dy/2,w,h)
                end
            end
        end
    end
    return {assets=assets,static=sprites,animated={}}
end
