-- NEW:CASTLEWALL / NEW:CASTLEWALL2: shared three-hex corner flags.
-- ASSETS BEGIN
local inventory = [[
castle/aquatic-castle/castle-concave-bl
castle/aquatic-castle/castle-concave-br
castle/aquatic-castle/castle-concave-l
castle/aquatic-castle/castle-concave-r
castle/aquatic-castle/castle-concave-tl
castle/aquatic-castle/castle-concave-tr
castle/aquatic-castle/castle-convex-bl
castle/aquatic-castle/castle-convex-br
castle/aquatic-castle/castle-convex-l
castle/aquatic-castle/castle-convex-r
castle/aquatic-castle/castle-convex-tl
castle/aquatic-castle/castle-convex-tr
castle/aquatic-castle/castle-to-ground-n-ne
castle/aquatic-castle/castle-to-ground-n
castle/aquatic-castle/castle-to-ground-ne
castle/aquatic-castle/castle-to-ground-nw-n-ne
castle/aquatic-castle/castle-to-ground-nw-n
castle/aquatic-castle/castle-to-ground-nw
castle/aquatic-castle/castle2-concave-bl
castle/aquatic-castle/castle2-concave-br
castle/aquatic-castle/castle2-convex-bl
castle/aquatic-castle/castle2-convex-br
castle/aquatic-castle/castle2-convex-r
castle/aquatic-castle/castle2-convex-tl
castle/aquatic-castle/castle2-convex-tr
castle/aquatic-castle/castle3-concave-bl
castle/aquatic-castle/castle3-convex-bl
castle/aquatic-castle/cobbles
castle/aquatic-castle/keep-castle-ccw-bl
castle/aquatic-castle/keep-castle-ccw-br
castle/aquatic-castle/keep-castle-ccw-l
castle/aquatic-castle/keep-castle-ccw-r
castle/aquatic-castle/keep-castle-ccw-tl
castle/aquatic-castle/keep-castle-ccw-tr
castle/aquatic-castle/keep-castle-concave-bl
castle/aquatic-castle/keep-castle-concave-br
castle/aquatic-castle/keep-castle-concave-l
castle/aquatic-castle/keep-castle-concave-r
castle/aquatic-castle/keep-castle-concave-tl
castle/aquatic-castle/keep-castle-concave-tr
castle/aquatic-castle/keep-castle-convex-bl
castle/aquatic-castle/keep-castle-convex-br
castle/aquatic-castle/keep-castle-convex-l
castle/aquatic-castle/keep-castle-convex-r
castle/aquatic-castle/keep-castle-convex-tl
castle/aquatic-castle/keep-castle-convex-tr
castle/aquatic-castle/keep-castle-cw-bl
castle/aquatic-castle/keep-castle-cw-br
castle/aquatic-castle/keep-castle-cw-l
castle/aquatic-castle/keep-castle-cw-r
castle/aquatic-castle/keep-castle-cw-tl
castle/aquatic-castle/keep-castle-cw-tr
castle/aquatic-castle/keep-concave-bl
castle/aquatic-castle/keep-concave-br
castle/aquatic-castle/keep-concave-l
castle/aquatic-castle/keep-concave-r
castle/aquatic-castle/keep-concave-tl
castle/aquatic-castle/keep-concave-tr
castle/aquatic-castle/keep-convex-bl
castle/aquatic-castle/keep-convex-br
castle/aquatic-castle/keep-convex-l
castle/aquatic-castle/keep-convex-r
castle/aquatic-castle/keep-convex-tl
castle/aquatic-castle/keep-convex-tr
castle/aquatic-castle/keep-to-ground-n-ne
castle/aquatic-castle/keep-to-ground-n
castle/aquatic-castle/keep-to-ground-ne-se
castle/aquatic-castle/keep-to-ground-ne
castle/aquatic-castle/keep-to-ground-nw-n-ne-se
castle/aquatic-castle/keep-to-ground-nw-n-ne
castle/aquatic-castle/keep-to-ground-nw-n
castle/aquatic-castle/keep-to-ground-nw
castle/aquatic-castle/keep-to-ground-se
castle/aquatic-castle/keep-to-ground-sw-nw-n-ne
castle/aquatic-castle/keep-to-ground-sw-nw
castle/aquatic-castle/keep-to-ground-sw
castle/aquatic-castle/keep-to-water-n
castle/aquatic-castle/keep-to-water-ne
castle/aquatic-castle/keep-to-water-nw
castle/aquatic-castle/keep2-castle-concave-tr
castle/aquatic-castle/keep2-castle-convex-bl
castle/aquatic-castle/keep2-castle-convex-br
castle/aquatic-castle/keep2-castle-convex-tr
castle/aquatic-castle/keep2-castle-cw-br
castle/aquatic-castle/keep2-concave-bl
castle/aquatic-castle/keep2-convex-br
castle/aquatic-castle/keep2-convex-tl
castle/aquatic-castle/mask-n
castle/aquatic-castle/mask-ne
castle/aquatic-castle/mask-nw
castle/aquatic-castle/mask-se
castle/aquatic-castle/mask-sw
castle/castle-concave-bl
castle/castle-concave-br
castle/castle-concave-l
castle/castle-concave-r
castle/castle-concave-tl
castle/castle-concave-tr
castle/castle-convex-bl
castle/castle-convex-br
castle/castle-convex-l
castle/castle-convex-r
castle/castle-convex-tl
castle/castle-convex-tr
castle/castle-to-ice-n
castle/castle-to-ice-ne
castle/castle-to-ice-nw
castle/castle-to-water-n
castle/castle-to-water-ne
castle/castle-to-water-nw
castle/cobbles-keep
castle/dwarven-castle-concave-bl
castle/dwarven-castle-concave-br
castle/dwarven-castle-concave-l
castle/dwarven-castle-concave-r
castle/dwarven-castle-concave-tl
castle/dwarven-castle-concave-tr
castle/dwarven-castle-convex-bl
castle/dwarven-castle-convex-br
castle/dwarven-castle-convex-l
castle/dwarven-castle-convex-r
castle/dwarven-castle-convex-tl
castle/dwarven-castle-convex-tr
castle/dwarven-castle-floor
castle/dwarven-castle-floor2
castle/dwarven-castle-floor3
castle/dwarven-castle-wall-ccw-bl
castle/dwarven-castle-wall-ccw-br
castle/dwarven-castle-wall-ccw-l
castle/dwarven-castle-wall-ccw-r
castle/dwarven-castle-wall-ccw-tl
castle/dwarven-castle-wall-ccw-tr
castle/dwarven-castle-wall-cw-bl
castle/dwarven-castle-wall-cw-br
castle/dwarven-castle-wall-cw-l
castle/dwarven-castle-wall-cw-r
castle/dwarven-castle-wall-cw-tl
castle/dwarven-castle-wall-cw-tr
castle/dwarven-keep-floor
castle/dwarven-keep
castle/elven-ruin/castle-concave-bl
castle/elven-ruin/castle-concave-br
castle/elven-ruin/castle-concave-l
castle/elven-ruin/castle-concave-r
castle/elven-ruin/castle-concave-tl
castle/elven-ruin/castle-concave-tr
castle/elven-ruin/castle-convex-bl
castle/elven-ruin/castle-convex-br
castle/elven-ruin/castle-convex-l
castle/elven-ruin/castle-convex-r
castle/elven-ruin/castle-convex-tl
castle/elven-ruin/castle-convex-tr
castle/elven-ruin/castle2-concave-bl
castle/elven-ruin/castle2-concave-br
castle/elven-ruin/castle2-concave-tl
castle/elven-ruin/castle2-concave-tr
castle/elven-ruin/castle2-convex-bl
castle/elven-ruin/castle2-convex-br
castle/elven-ruin/castle2-convex-l
castle/elven-ruin/castle2-convex-tr
castle/elven-ruin/castle3-concave-br
castle/elven-ruin/castle3-concave-tl
castle/elven-ruin/grounds
castle/elven-ruin/grounds2
castle/elven-ruin/grounds3
castle/elven-ruin/grounds4
castle/elven-ruin/grounds5
castle/elven-ruin/grounds6
castle/elven-ruin/keep-castle-ccw-bl
castle/elven-ruin/keep-castle-ccw-br
castle/elven-ruin/keep-castle-ccw-l
castle/elven-ruin/keep-castle-ccw-r
castle/elven-ruin/keep-castle-ccw-tl
castle/elven-ruin/keep-castle-ccw-tr
castle/elven-ruin/keep-castle-concave-tr
castle/elven-ruin/keep-castle-convex-bl
castle/elven-ruin/keep-castle-convex-br
castle/elven-ruin/keep-castle-convex-l
castle/elven-ruin/keep-castle-convex-r
castle/elven-ruin/keep-castle-convex-tl
castle/elven-ruin/keep-castle-convex-tr
castle/elven-ruin/keep-castle-cw-bl
castle/elven-ruin/keep-castle-cw-br
castle/elven-ruin/keep-castle-cw-l
castle/elven-ruin/keep-castle-cw-r
castle/elven-ruin/keep-castle-cw-tl
castle/elven-ruin/keep-castle-cw-tr
castle/elven-ruin/keep-castle2-convex-bl
castle/elven-ruin/keep-castle2-convex-br
castle/elven-ruin/keep-castle2-convex-l
castle/elven-ruin/keep-concave-bl
castle/elven-ruin/keep-concave-br
castle/elven-ruin/keep-concave-l
castle/elven-ruin/keep-concave-r
castle/elven-ruin/keep-concave-tl
castle/elven-ruin/keep-concave-tr
castle/elven-ruin/keep-convex-bl
castle/elven-ruin/keep-convex-br
castle/elven-ruin/keep-convex-l
castle/elven-ruin/keep-convex-r
castle/elven-ruin/keep-convex-tl
castle/elven-ruin/keep-convex-tr
castle/elven-ruin/keep
castle/elven-ruin/keep2-concave-br
castle/elven-ruin/keep2-concave-l
castle/elven-ruin/keep2-concave-tl
castle/elven-ruin/keep2-convex-br
castle/elven-ruin/keep2-convex-tl
castle/elven-ruin/keep2
castle/elven-ruin/keep3
castle/elven-ruin/keep4
castle/elven-ruin/keep5
castle/encampment/regular-concave-bl
castle/encampment/regular-concave-br
castle/encampment/regular-concave-l
castle/encampment/regular-concave-r
castle/encampment/regular-concave-tl
castle/encampment/regular-concave-tr
castle/encampment/regular-convex-bl
castle/encampment/regular-convex-br
castle/encampment/regular-convex-l
castle/encampment/regular-convex-r
castle/encampment/regular-convex-tl
castle/encampment/regular-convex-tr
castle/encampment/snow-concave-bl
castle/encampment/snow-concave-br
castle/encampment/snow-concave-l
castle/encampment/snow-concave-r
castle/encampment/snow-concave-tl
castle/encampment/snow-concave-tr
castle/encampment/snow-convex-bl
castle/encampment/snow-convex-br
castle/encampment/snow-convex-l
castle/encampment/snow-convex-r
castle/encampment/snow-convex-tl
castle/encampment/snow-convex-tr
castle/encampment/tall-keep-castle-ccw-bl
castle/encampment/tall-keep-castle-ccw-br
castle/encampment/tall-keep-castle-ccw-l
castle/encampment/tall-keep-castle-ccw-r
castle/encampment/tall-keep-castle-ccw-tl
castle/encampment/tall-keep-castle-ccw-tr
castle/encampment/tall-keep-castle-concave-tr
castle/encampment/tall-keep-castle-convex-bl
castle/encampment/tall-keep-castle-convex-br
castle/encampment/tall-keep-castle-convex-l
castle/encampment/tall-keep-castle-convex-r
castle/encampment/tall-keep-castle-convex-tl
castle/encampment/tall-keep-castle-convex-tr
castle/encampment/tall-keep-castle-cw-bl
castle/encampment/tall-keep-castle-cw-br
castle/encampment/tall-keep-castle-cw-l
castle/encampment/tall-keep-castle-cw-r
castle/encampment/tall-keep-castle-cw-tl
castle/encampment/tall-keep-castle-cw-tr
castle/encampment/tall-keep-concave-bl
castle/encampment/tall-keep-concave-br
castle/encampment/tall-keep-concave-l
castle/encampment/tall-keep-concave-r
castle/encampment/tall-keep-concave-tl
castle/encampment/tall-keep-concave-tr
castle/encampment/tall-keep-convex-bl
castle/encampment/tall-keep-convex-br
castle/encampment/tall-keep-convex-l
castle/encampment/tall-keep-convex-r
castle/encampment/tall-keep-convex-tl
castle/encampment/tall-keep-convex-tr
castle/encampment/tent-snow
castle/encampment/tent-snow2
castle/encampment/tent
castle/encampment/tent2
castle/keep-castle-ccw-bl
castle/keep-castle-ccw-br
castle/keep-castle-ccw-l
castle/keep-castle-ccw-r
castle/keep-castle-ccw-tl
castle/keep-castle-ccw-tr
castle/keep-castle-concave-tr
castle/keep-castle-convex-bl
castle/keep-castle-convex-br
castle/keep-castle-convex-l
castle/keep-castle-convex-r
castle/keep-castle-convex-tl
castle/keep-castle-convex-tr
castle/keep-castle-cw-bl
castle/keep-castle-cw-br
castle/keep-castle-cw-l
castle/keep-castle-cw-r
castle/keep-castle-cw-tl
castle/keep-castle-cw-tr
castle/keep-concave-bl
castle/keep-concave-br
castle/keep-concave-l
castle/keep-concave-r
castle/keep-concave-tl
castle/keep-concave-tr
castle/keep-convex-bl
castle/keep-convex-br
castle/keep-convex-l
castle/keep-convex-r
castle/keep-convex-tl
castle/keep-convex-tr
castle/ruin-concave-bl
castle/ruin-concave-br
castle/ruin-concave-l
castle/ruin-concave-r
castle/ruin-concave-tl
castle/ruin-concave-tr
castle/ruin-convex-bl
castle/ruin-convex-br
castle/ruin-convex-l
castle/ruin-convex-r
castle/ruin-convex-tl
castle/ruin-convex-tr
castle/ruin2-concave-bl
castle/ruin2-concave-br
castle/ruin2-concave-l
castle/ruin2-concave-r
castle/ruin2-concave-tl
castle/ruin2-concave-tr
castle/ruin2-convex-bl
castle/ruin2-convex-br
castle/ruin2-convex-l
castle/ruin2-convex-r
castle/ruin2-convex-tl
castle/ruin2-convex-tr
castle/ruin3-concave-bl
castle/ruin3-concave-br
castle/ruin3-concave-l
castle/ruin3-concave-r
castle/ruin3-concave-tl
castle/ruin3-concave-tr
castle/ruin3-convex-bl
castle/ruin3-convex-br
castle/ruin3-convex-l
castle/ruin3-convex-r
castle/ruin3-convex-tl
castle/ruin3-convex-tr
castle/ruin4-concave-bl
castle/ruin4-concave-br
castle/ruin4-concave-l
castle/ruin4-concave-r
castle/ruin4-concave-tl
castle/ruin4-concave-tr
castle/ruin4-convex-bl
castle/ruin4-convex-br
castle/ruin4-convex-l
castle/ruin4-convex-r
castle/ruin4-convex-tl
castle/ruin4-convex-tr
castle/ruin5-concave-bl
castle/ruin5-concave-br
castle/ruin5-concave-l
castle/ruin5-concave-r
castle/ruin5-concave-tl
castle/ruin5-concave-tr
castle/ruin5-convex-bl
castle/ruin5-convex-br
castle/ruin5-convex-l
castle/ruin5-convex-r
castle/ruin5-convex-tl
castle/ruin5-convex-tr
castle/ruinkeep1-castle-ccw-bl
castle/ruinkeep1-castle-ccw-br
castle/ruinkeep1-castle-ccw-l
castle/ruinkeep1-castle-ccw-r
castle/ruinkeep1-castle-ccw-tl
castle/ruinkeep1-castle-ccw-tr
castle/ruinkeep1-castle-convex-bl
castle/ruinkeep1-castle-convex-br
castle/ruinkeep1-castle-convex-l
castle/ruinkeep1-castle-convex-r
castle/ruinkeep1-castle-convex-tl
castle/ruinkeep1-castle-convex-tr
castle/ruinkeep1-castle-cw-bl
castle/ruinkeep1-castle-cw-br
castle/ruinkeep1-castle-cw-l
castle/ruinkeep1-castle-cw-r
castle/ruinkeep1-castle-cw-tl
castle/ruinkeep1-castle-cw-tr
castle/ruinkeep1-concave-bl
castle/ruinkeep1-concave-br
castle/ruinkeep1-concave-l
castle/ruinkeep1-concave-r
castle/ruinkeep1-concave-tl
castle/ruinkeep1-concave-tr
castle/ruinkeep1-convex-bl
castle/ruinkeep1-convex-br
castle/ruinkeep1-convex-l
castle/ruinkeep1-convex-r
castle/ruinkeep1-convex-tl
castle/ruinkeep1-convex-tr
castle/sunken-keep-coast
castle/sunken-keep-ocean
castle/sunken-ruin-concave-bl
castle/sunken-ruin-concave-br
castle/sunken-ruin-concave-l
castle/sunken-ruin-concave-r
castle/sunken-ruin-concave-tl
castle/sunken-ruin-concave-tr
castle/sunken-ruin-convex-bl
castle/sunken-ruin-convex-br
castle/sunken-ruin-convex-l
castle/sunken-ruin-convex-r
castle/sunken-ruin-convex-tl
castle/sunken-ruin-convex-tr
castle/sunken-ruin-n
castle/sunken-ruin-ne
castle/sunken-ruin-nw
castle/sunken-ruin-s
castle/sunken-ruin-se
castle/sunken-ruin-sw
castle/sunken-ruin
castle/sunken-ruin2-concave-bl
castle/sunken-ruin2-concave-br
castle/sunken-ruin2-concave-l
castle/sunken-ruin2-concave-r
castle/sunken-ruin2-concave-tl
castle/sunken-ruin2-concave-tr
castle/sunken-ruin2-convex-bl
castle/sunken-ruin2-convex-br
castle/sunken-ruin2-convex-l
castle/sunken-ruin2-convex-r
castle/sunken-ruin2-convex-tl
castle/sunken-ruin2-convex-tr
castle/sunken-ruin3-concave-bl
castle/sunken-ruin3-concave-br
castle/sunken-ruin3-concave-l
castle/sunken-ruin3-concave-r
castle/sunken-ruin3-concave-tl
castle/sunken-ruin3-concave-tr
castle/sunken-ruin3-convex-bl
castle/sunken-ruin3-convex-br
castle/sunken-ruin3-convex-l
castle/sunken-ruin3-convex-r
castle/sunken-ruin3-convex-tl
castle/sunken-ruin3-convex-tr
castle/sunken-ruin4-concave-bl
castle/sunken-ruin4-concave-br
castle/sunken-ruin4-concave-l
castle/sunken-ruin4-concave-r
castle/sunken-ruin4-concave-tl
castle/sunken-ruin4-concave-tr
castle/sunken-ruin4-convex-bl
castle/sunken-ruin4-convex-br
castle/sunken-ruin4-convex-l
castle/sunken-ruin4-convex-r
castle/sunken-ruin4-convex-tl
castle/sunken-ruin4-convex-tr
castle/sunken-ruin5-concave-bl
castle/sunken-ruin5-concave-br
castle/sunken-ruin5-concave-l
castle/sunken-ruin5-concave-r
castle/sunken-ruin5-concave-tl
castle/sunken-ruin5-concave-tr
castle/sunken-ruin5-convex-bl
castle/sunken-ruin5-convex-br
castle/sunken-ruin5-convex-l
castle/sunken-ruin5-convex-r
castle/sunken-ruin5-convex-tl
castle/sunken-ruin5-convex-tr
castle/sunken-ruin6-concave-bl
castle/sunken-ruin6-concave-br
castle/sunken-ruin6-concave-l
castle/sunken-ruin6-concave-r
castle/sunken-ruin6-concave-tl
castle/sunken-ruin6-concave-tr
castle/sunken-ruin6-convex-bl
castle/sunken-ruin6-convex-br
castle/sunken-ruin6-convex-l
castle/sunken-ruin6-convex-r
castle/sunken-ruin6-convex-tl
castle/sunken-ruin6-convex-tr
castle/sunken-ruinkeep1-castle-ccw-bl
castle/sunken-ruinkeep1-castle-ccw-br
castle/sunken-ruinkeep1-castle-ccw-l
castle/sunken-ruinkeep1-castle-ccw-r
castle/sunken-ruinkeep1-castle-ccw-tl
castle/sunken-ruinkeep1-castle-ccw-tr
castle/sunken-ruinkeep1-castle-convex-bl
castle/sunken-ruinkeep1-castle-convex-br
castle/sunken-ruinkeep1-castle-convex-l
castle/sunken-ruinkeep1-castle-convex-r
castle/sunken-ruinkeep1-castle-convex-tl
castle/sunken-ruinkeep1-castle-convex-tr
castle/sunken-ruinkeep1-castle-cw-bl
castle/sunken-ruinkeep1-castle-cw-br
castle/sunken-ruinkeep1-castle-cw-l
castle/sunken-ruinkeep1-castle-cw-r
castle/sunken-ruinkeep1-castle-cw-tl
castle/sunken-ruinkeep1-castle-cw-tr
castle/sunken-ruinkeep1-concave-bl
castle/sunken-ruinkeep1-concave-br
castle/sunken-ruinkeep1-concave-l
castle/sunken-ruinkeep1-concave-r
castle/sunken-ruinkeep1-concave-tl
castle/sunken-ruinkeep1-concave-tr
castle/sunken-ruinkeep1-convex-bl
castle/sunken-ruinkeep1-convex-br
castle/sunken-ruinkeep1-convex-l
castle/sunken-ruinkeep1-convex-r
castle/sunken-ruinkeep1-convex-tl
castle/sunken-ruinkeep1-convex-tr
castle/sunkenkeep-castle-ccw-bl
castle/sunkenkeep-castle-ccw-br
castle/sunkenkeep-castle-ccw-l
castle/sunkenkeep-castle-ccw-r
castle/sunkenkeep-castle-ccw-tl
castle/sunkenkeep-castle-ccw-tr
castle/sunkenkeep-castle-concave-tr
castle/sunkenkeep-castle-convex-bl
castle/sunkenkeep-castle-convex-br
castle/sunkenkeep-castle-convex-l
castle/sunkenkeep-castle-convex-r
castle/sunkenkeep-castle-convex-tl
castle/sunkenkeep-castle-convex-tr
castle/sunkenkeep-castle-cw-bl
castle/sunkenkeep-castle-cw-br
castle/sunkenkeep-castle-cw-l
castle/sunkenkeep-castle-cw-r
castle/sunkenkeep-castle-cw-tl
castle/sunkenkeep-castle-cw-tr
castle/sunkenkeep-concave-bl
castle/sunkenkeep-concave-br
castle/sunkenkeep-concave-l
castle/sunkenkeep-concave-r
castle/sunkenkeep-concave-tl
castle/sunkenkeep-concave-tr
castle/sunkenkeep-convex-bl
castle/sunkenkeep-convex-br
castle/sunkenkeep-convex-l
castle/sunkenkeep-convex-r
castle/sunkenkeep-convex-tl
castle/sunkenkeep-convex-tr
flat/dirt
flat/dirt2
flat/dirt3
flat/dirt4
flat/dirt5
flat/dirt6
flat/dirt7
flat/road-icy
flat/road-icy2
flat/road-icy3
flat/road-n-ne-se-s-sw-nw
flat/road-n-ne-se
flat/road-n-ne
flat/road-n
flat/road-ne-se-s
flat/road-ne-se
flat/road-ne
flat/road-nw-n-ne
flat/road-nw-n
flat/road-nw
flat/road-s-sw-nw
flat/road-s-sw
flat/road-s
flat/road-se-s-sw
flat/road-se-s
flat/road-se
flat/road-sw-nw-n
flat/road-sw-nw
flat/road-sw
flat/road
flat/road2
flat/road3
flat/road4
flat/stone-path
flat/stone-path2
]]
-- ASSETS END
local available = {}
for name in inventory:gmatch("%S+") do available[name]=true end
local CORNER={"tr","r","br","bl","l","tl"}
local ORIGIN={{36,108},{36,108},{36,36},{90,72},{90,72},{90,144}}
local function next_corner(c,n) return (c-1+n)%6+1 end
local function neighbors(x,y)
    local up=x%2==0 and -1 or 0
    return {{x,y-1},{x+1,y+up},{x+1,y+up+1},{x,y+1},{x-1,y+up+1},{x-1,y+up}}
end
local function noise(x,y,name)
    local hash=0
    for i=1,#name do hash=(((hash<<9)|(hash>>23)) ~ name:byte(i)) & 0xffffffff end
    local a=((x-1+92872973) ~ 918273) & 0xffffffff
    local b=((y-1+1672517) ~ 128123) & 0xffffffff
    local c=((hash+127390) ~ 13923787) & 0xffffffff
    local mixed=(a*b*c+a*b+b*c+a*c+a+b+c) & 0xffffffff
    return (mixed*mixed) & 0xffffffff
end
local function is_castle(c) return c:match("^[CK]")~=nil end
local function is_open(c) return c:match("^Xu") or c:match("^Xo") end
local function outside(c) return not is_castle(c) and not is_open(c) end
local function court(c) return c:match("^C") or c=="Ke" end
local function keep_outside(c) return not c:match("^K") and not is_open(c) end
local function exact(wanted) return function(c) return c==wanted end end

return function(map)
    local assets,sprites,flags={},{},{}
    local function code(x,y)
        local raw=map.cells[y] and map.cells[y][x] or ""
        local c=(raw:match("%S+$") or raw):match("^[^^]+") or ""
        return c=="castle" and "Ch" or c=="keep" and "Kh" or c
    end
    local function choose(stem,suffix,x,y)
        local options={}
        for i=1,11 do
            local path=stem..(i==1 and "" or i)..suffix
            if available[path] then options[#options+1]=path end
        end
        if #options==0 then return nil end
        return options[(noise(x,y,stem.."@V"..suffix..".png")//7919)%#options+1]
    end
    local function emit(path,x,y,order,pass,ox,oy,baseline,clips)
        if not path then return end
        assets[path]="assets/wesnoth/terrain/"..path..".png"
        sprites[#sprites+1]={asset=path,x=x,y=y,order=order,pass=pass or "ground",
            offset_x=ox or -36,offset_y=oy or -36,baseline=baseline,clips=clips}
    end
    local bases={Ch={"flat/road",-1000},Chr={"flat/stone-path",-1000},
        Kh={"castle/cobbles-keep",-2},Khr={"castle/cobbles-keep",-2},
        Chw={"castle/aquatic-castle/cobbles",-520},
        Ce={"flat/dirt",-1000},Ke={"flat/dirt",-1000},
        Cvr={"castle/elven-ruin/grounds",-1000},Kvr={"castle/elven-ruin/keep",-2},
        Cud={"castle/dwarven-castle-floor",-2},Kud={"castle/dwarven-keep-floor",-2}}
    for _,t in ipairs(map.tiles) do
        local c=code(t.x,t.y)
        local b=bases[c]
        if b then emit(choose(b[1],"",t.x,t.y),t.x,t.y,b[2]) end
        if c=="Ke" or c=="Kud" then
            emit(c=="Ke" and "castle/encampment/tent" or "castle/dwarven-keep",t.x,t.y,0,"world",-36,-36,16)
        end
    end
    -- Core NEW:TRANSITION Ch,Chr,Cha -> other castle floors, including Chw.
    -- Group adjacent edges so the original corner artwork joins seamlessly.
    local directions={"n","ne","se","s","sw","nw"}
    for _,t in ipairs(map.tiles) do
        local c=code(t.x,t.y)
        if (c:match("^C") or c:match("^Ke")) and c~="Ch" and c~="Chr" and c~="Cha" and c~="Ket" then
            local edges={}
            for d,p in ipairs(neighbors(t.x,t.y)) do
                local other=code(p[1],p[2])
                edges[d]=other=="Ch" or other=="Chr" or other=="Cha"
            end
            for _,length in ipairs({6,3,2,1}) do
                for first=1,6 do
                    local suffix,matched={},true
                    for offset=0,length-1 do
                        local d=next_corner(first,offset)
                        if not edges[d] then matched=false end
                        suffix[#suffix+1]=directions[d]
                    end
                    if matched then
                        local path=choose("flat/road","-"..table.concat(suffix,"-"),t.x,t.y)
                        if path then
                            for offset=0,length-1 do edges[next_corner(first,offset)]=false end
                            emit(path,t.x,t.y,-300)
                        end
                    end
                end
            end
        end
    end
    local function key(x,y,c) return x..","..y..":"..c end
    local function rule(source,a,b,stem,shape,prob,channel)
        channel=channel or "castlewall"
        flags[channel]=flags[channel] or {}
        local claimed=flags[channel]
        for c=1,6 do
            for x=0,map.width+1 do for y=0,map.height+1 do
                local adj=neighbors(x,y)
                local p,q=adj[c],adj[next_corner(c,1)]
                if source(code(x,y)) and a(code(p[1],p[2])) and b(code(q[1],q[2])) then
                    local keys={key(x,y,c),key(p[1],p[2],next_corner(c,2)),key(q[1],q[2],next_corner(c,4))}
                    local path=choose(stem,"-"..shape.."-"..CORNER[c],x,y)
                    if path and not claimed[keys[1]] and not claimed[keys[2]] and not claimed[keys[3]]
                        and (not prob or noise(x,y,stem.."@V-"..shape.."-"..CORNER[c]..".png")%100<prob)
                    then
                        for _,k in ipairs(keys) do claimed[k]=true end
                        local origin=ORIGIN[c]
                        emit(path,x,y,0,"world",-origin[1],-origin[2],72-origin[2],
                            {{x=x,y=y},{x=p[1],y=p[2]},{x=q[1],y=q[2]}})
                    end
                end
            end end
        end
    end
    local function supplement(source,a,b,stem,rotations,base_y,target,end_flag)
        flags.ends=flags.ends or {}
        for c=1,6 do
            local suffix=rotations[c]
            if suffix and suffix~="skip" then
                for x=0,map.width+1 do for y=0,map.height+1 do
                    local around=neighbors(x,y)
                    local p,q=around[c],around[next_corner(c,1)]
                    if source(code(x,y)) and a(code(p[1],p[2])) and b(code(q[1],q[2])) then
                        local t=target==2 and p or target==3 and q or {x,y}
                        local k=key(t[1],t[2],(end_flag and "end-" or "")..suffix)
                        local path=choose(stem,"-"..suffix,x,y)
                        if path and not flags.ends[k] then
                            flags.ends[k]=true
                            local origin=ORIGIN[c]
                            emit(path,x,y,0,"world",-origin[1],-origin[2],base_y-origin[2],
                                {{x=x,y=y},{x=p[1],y=p[2]},{x=q[1],y=q[2]}})
                        end
                    end
                end end
            end
        end
    end
    local function wall(source,adj,stem,prob,channel,internal)
        rule(source,adj,adj,stem,"convex",prob,channel)
        rule(adj,source,source,stem,"concave",prob,channel)
        if not internal and channel~="wall" then
            local function join(c)
                return not source(c) and (stem:find("keep",1,true) and c:match("^K") or
                    not stem:find("keep",1,true) and court(c))
            end
            supplement(source,adj,join,stem,{[1]="concave-br",[4]="concave-tl",[6]="concave-r"},107)
            supplement(source,join,adj,stem,{[1]="concave-l",[3]="concave-tr",[6]="concave-bl"},107)
            supplement(source,is_open,adj,stem,{[1]="concave-l",[2]="convex-r",[5]="concave-br",[6]="concave-bl"},107,1,true)
            supplement(source,adj,is_open,stem,{[1]="concave-br",[3]="convex-br",[6]="concave-r"},107,1,true)
            supplement(source,adj,is_open,stem,{[2]="concave-bl",[4]="convex-bl"},26,1,true)
            supplement(adj,is_open,source,stem,{[1]="convex-l"},107,3,true)
            supplement(adj,source,is_open,stem,{[1]="convex-br"},107,2,true)
            supplement(adj,source,is_open,stem,{[2]="convex-bl"},74,2,true)
        end
    end
    local function wall2(source,a,b,stem,prob,channel)
        wall(source,a,stem,prob,channel,true)
        rule(source,a,b,stem,"cw",prob,channel)
        rule(source,b,a,stem,"ccw",prob,channel)
    end
    -- Preserve core terrain-graphics.cfg priority, independently of draw depth.
    wall(exact("Cvr"),outside,"castle/elven-ruin/castle")
    wall2(exact("Kvr"),court,outside,"castle/elven-ruin/keep-castle")
    wall(exact("Kvr"),keep_outside,"castle/elven-ruin/keep")
    wall(exact("Ch"),outside,"castle/castle")
    wall2(exact("Kh"),court,outside,"castle/keep-castle")
    wall(exact("Kh"),keep_outside,"castle/keep")
    wall(exact("Chw"),function(c) return c:match("^W") end,"castle/sunken-ruin")
    wall(function(c) return c=="Chr" or c=="Chw" end,outside,"castle/ruin")
    wall2(exact("Khr"),court,outside,"castle/ruinkeep1-castle",75)
    wall2(exact("Khr"),court,outside,"castle/keep-castle")
    wall(exact("Khr"),keep_outside,"castle/ruinkeep1",75)
    wall(exact("Khr"),keep_outside,"castle/keep")
    wall(function(c) return c=="Ce" or c=="Ke" end,outside,"castle/encampment/regular")
    local function dwarven(c) return c=="Cud" or c=="Kud" end
    wall2(dwarven,function(c) return c:match("^X") end,
        function(c) return not dwarven(c) and not c:match("^X") end,"castle/dwarven-castle-wall")
    wall(dwarven,function(c) return not dwarven(c) and not c:match("^X") end,"castle/dwarven-castle",nil,"wall")
    return {assets=assets,static=sprites,animated={}}
end
