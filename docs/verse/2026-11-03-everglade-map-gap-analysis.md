# Everglade map gap analysis

Status: spec, November 3, 2026. Compares the previous engine's illustrated
Everglade map with the Everglade that runs today at
https://openagents.com/worlds/everglade, and orders the work to close the
gaps.

## Sources

The previous map:

- `verse/everglade/previous/everglade-map.png` and `everglade-map.svg`:
  2400 × 1720 px, titled "Everglade — A City for Building Things Together".
  Positions below are SVG pixels (x east, y south) from the SVG's text
  labels and shapes.
- `verse/everglade/previous/everglade.md`: what the Rust engine built from
  the map. Lines 331 to 345 give the scale and lines 549 to 563 the
  district table.
- `verse/everglade/previous/coast.md`, `water.md`: the coast and the water
  that started from Everglade's stream and ponds.

The current city:

- `verse/everglade/plan.coax`: the layout. Metres, +z north, **+x west**,
  the fountain at the origin (`plan.coax:7-9`).
- `docs/verse/everglade.md`: the plan in words; `docs/verse/everglade-map.svg`
  is drawn from `plan.coax` by `verse/everglade/map.py`.
- `apps/openagents-web/realms/everglade/index.md`: the player-facing page.
- `verse/everglade/river.coax`, `people.coax`, `quests.coax`.
- Headless screenshots of the live city (`#above`, `#commons`), taken
  November 3, 2026 and not kept in git.

## The two cities in one paragraph each

**Previous map.** An open city with no wall, built round a great lawn.
Two ring roads (the inner Commons Loop and an outer ring) circle an
egg-shaped park, the Commons, which holds two ponds. Six roundabouts on
the Commons Loop feed six radial roads out to the map's edges. Seven
named districts fill the band between the rings, about 700 buildings in
all. Past the outer ring, a forest belt rings the whole city, with two
named woods (Walden Woods, Fernhollow), cabins on dashed footpaths, an
orchard, a hill and a meadow. A stream, Glade Run, leaves the Commons and
runs south-west to a fourth pond.

**Now.** A medieval hub inside a square timber palisade with four gates.
A paved commons 32 m across holds the fountain; four 3 m streets lead to
the gates. Sixteen buildings stand inside or just outside the walls, all
of them walk-in. Past a 6 m apron, open woods with six trail-end
clearings run out to a thicket at 101.3 m. A straight river runs the
length of the world 88 m west of the fountain. The four ponds carry the
previous map's names but stand in the woods outside the walls.

## Scale

| | Previous map | Previous build | Now |
| --- | --- | --- | --- |
| Units on the source | Pixels; a scale bar of 240 px labelled "ten minutes on foot" (`everglade-map.svg:13856-13858`) | Metres | Metres |
| Whole world | 2400 × 1720 px sheet | Zone about 510 m across (`previous/everglade.md:331`) | Ground 208 m square, 200 m walkable (`plan.coax:20-23`; `docs/verse/everglade.md:45`) |
| Town | Outer ring road about 1,950 px east to west (x 233 to 2187) | Flat clearing 272 m across inside a tree ring about 300 m across (`previous/everglade.md:331-335`) | Palisade 127 m east to west, 115 m north to south (`plan.coax:41-50`; `docs/verse/everglade.md:46`) |
| Longest street | Main Street, north radial | Main Street about 200 m (`previous/everglade.md:552`) | Inn Street 44 m; Guild Row 52 m (`docs/verse/everglade.md:66, 75`) |
| Walking | Ten minutes on the scale bar | — | 3.5 m/s walk, city half a minute across (`docs/verse/everglade.md:50-53`) |

The map's scale bar does not match its own drawing: at a walk of 1.4 m/s,
ten minutes is about 840 m, which would make the town about 7 km across.
The previous build read the drawing at about 0.14 m per pixel (the 1,950 px
ring road as the 272 m clearing). This analysis uses that reading for
"about" sizes taken off the map, and marks them as derived.

At that reading the previous town is about 2.1 times the current palisade
east to west and its area about 4.5 times. The current world (208 m) is
smaller than the previous town alone.

## District by district

Each part lists what the previous map draws, then what the current city
has. Building counts on the map are approximate: they count the SVG's
697 building shadows (`<polygon … fill="#3b2c1c" opacity="0.22">`, one per
footprint) by the nearest district label, and terraced row houses count
one per unit.

### The Commons

**Previous map.** Label "THE COMMONS / the great lawn" at (1240, 780). An
egg-shaped park inside the Commons Loop, about 1,300 × 790 px (derived:
about 180 × 110 m). It holds:

- Lantern Pond at (970, 875), the largest water in the city (derived:
  about 50 m across), with the Boathouse on its north bank (1080, 755).
- Reed Pond at (1500, 934), with a jetty.
- The Bandshell at (1240, 916), an arc on the lawn between the ponds.
- Fountain Plaza at (1200, 526): a round plaza (r 26 px) with a basin
  (r 9.1 px) on the park's north edge, where Main Street meets the loop.
- Sculpture Walk at (1580, 704) on the north-east lawn.
- Community Gardens at (775, 1192): a grid of beds in the south-west.
- A web of curving footpaths, open lawns and scattered park trees.

**Now.** The commons is a paved round square 32 m across
(`EG_SQUARE`, `plan.coax:207`: radius 16, level 0) at the city's middle,
inside the Commons Ring, eight nodes 12 m from the fountain
(`EG_NODE_XZ`, `plan.coax:115-122`). It holds the fountain (a 6 m box,
`EG_PROP`, `plan.coax:96`; 6.6 m across as drawn), the quest board
(`plan.coax:97`), the town crier and the errand cat (`EG_SPOT`,
`plan.coax:165`). The live screenshot also shows benches, a brazier, a
signpost and bunting across the square (`street_dress`,
`apps/openagents-web/everglade.coax:3766`).

All four ponds exist but none is on the commons. Lantern Pond is at
(−77, −11), 13 m outside the east wall; Reed Pond at (14, −90) south of
the stables (`EG_POND`, `plan.coax:244-249`). There is no lawn, bandshell,
boathouse, sculpture walk or community garden.

### Main Street

**Previous map.** Label "MAIN STREET / cafés, bookshops, bakers &
grocers" at (1279, 330). Main Street runs north from the north roundabout
(1200, 443) through the Fountain Plaza to the map's top edge. Market Hall
(1064, 256) stands north-west of it in its own square; a green square
lies to its east. About 75 buildings, in terraces along short cross
streets between the two rings. The previous build put the Civic Hall,
twelve shops, the bakehouse and market stalls here, with Market Row
behind (`previous/everglade.md:552-553`).

**Now.** The nearest match is the market, west of the commons on Market
Street: a paved square 26 m across (`EG_SQUARE`, `plan.coax:207`: centre
(34, 0), radius 13), six 3 × 2 m stalls in two rows
(`plan.coax:98-99`), the general store (12 × 10 m, `plan.coax:60`), and
three sellers. There is no market hall and no shop street.

### Stoop Lane

**Previous map.** Label "STOOP LANE / townhouses & little gardens" at
(711, 442), the north-west arc between the rings, about 150 buildings:
the densest district, long terraces on parallel lanes. Hearth Road runs
north-west through it (label at (315, 541)).

**Now.** Six houses, 8 to 10 m a side (`EG_BUILDING` entries 10 to 15,
`plan.coax:62-63`; `docs/verse/everglade.md:86`), spread round the
streets. No terraces, no gardens, no residential district.

### Lantern Quarter

**Previous map.** Label "LANTERN QUARTER / pubs, music & meeting halls"
at (413, 857), the west arc. About 80 buildings, two green squares, and
the round Music Hall at (319, 1037) (r 26 px). Lantern Road runs west out
of it (label at (222, 1147)).

**Now.** The inn (16 × 14 m, two storeys, `plan.coax:60`;
`docs/verse/everglade.md:68`) is the only gathering hall. No music hall,
pubs or meeting halls.

### Brownstone Row

**Previous map.** Label "BROWNSTONE ROW / homes, stoops & courtyards" at
(836, 1362), the south-west arc. About 120 buildings, mostly long
terraces; a green square on its west; Glade Run crosses its west end
under a footbridge (`previous/everglade.md:337-339`).

**Now.** No equivalent beyond the six houses.

### Knowledge District

**Previous map.** Label "KNOWLEDGE DISTRICT" at (1489, 1392), the south
arc east of Library Way. About 80 buildings, lighter-roofed than the
homes. Landmarks: The Stacks, "public library", at (1642, 1472), a
square building with a round centre; the Old College at (1958, 1222), a
quadrangle round a green; Observatory Hill at (2025, 1513), drawn as
contour rings with a domed observatory (r 13 px) on top. Library Way runs
south out of it (label at (1167, 1488)).

**Now.** Temple Hill north of the commons: the temple (18 × 18 m, 12 m
high, `plan.coax:61`) on a forecourt 18 m across (`EG_SQUARE`,
`plan.coax:207`: radius 9, level 0, so the "hill" is flat). The
observatory (8 × 8 m, 18 m with its dome, `plan.coax:62`) stands north-west
of it on Stargazers' Lane, inside the walls, not on a hill. No library,
no college.

### The Foundry

**Previous map.** Label "THE FOUNDRY / technology & workshops" at
(2117, 967), the east arc. About 33 buildings with blue-grey roofs and the
Server Barn & Fab Yard at (2171, 1126), the largest single footprint east
of the Commons. Foundry Road runs east out of it (label at (2290, 1070)).

**Now.** The smithy (12 × 10 m, brick, `plan.coax:60`) by the West Gate.
No workshop district.

### Creative District

**Previous map.** Label "CREATIVE DISTRICT / studios, ateliers &
workshops" at (1743, 588), the north-east arc. About 85 buildings.
Landmarks: Makers' Hall at (1684, 512), drawn with a yard of beds; the
Boardwalk Cafés at (1894, 728), drawn with a 3 × 2 grid of tables. Studio
Road runs north-east out of it (label at (1970, 599)).

**Now.** Guild Row east of the commons holds the fighters' hall and the
rangers' lodge (14 × 12 m each, `plan.coax:61`), and the mages' tower
(10 × 10 m, 14 m tall, `plan.coax:61`) stands to the north-east. These
fill the same compass sector but serve a different purpose. No studios,
cafés or makers' hall.

### Walden Woods

**Previous map.** Label "WALDEN WOODS / cabins for thinking, writing &
prototyping" at (560, 1580), the south-west corner. Thinking Pond at
(215, 1500), the second-largest water. Three buildings on footpaths: the
Writing Cabin (345, 1416), the Quiet Cabin (130, 1386) and the Prototype
Shed (380, 1606). Glade Run ends in Thinking Pond.

**Now.** The Thinking Pond is at (28, 73), radius 5.5 m, in a 9.5 m
clearing north of the North Gate, outside the walls (`EG_POND`,
`plan.coax:247`; `EG_POND_OPEN`, `plan.coax:250`). No cabins, no named
wood.

### Fernhollow

**Previous map.** Label "FERNHOLLOW" at (2120, 160), the north-east
corner. Fern Pond at (2120, 305) with a footbridge or jetty; the Code
Cabin at (2050, 236) on its shore; the Lookout Hut at (2250, 446) at the
end of a dashed path from Studio Road.

**Now.** The Fern Pond is at (−79, 21), radius 5 m, east of the East Gate
(`plan.coax:248`). No cabin, no lookout.

### Gardens, orchard and meadow

**Previous map.** The Orchard at (220, 600) west of the outer ring; the
Beekeeper's Hut at (150, 356) in the north-west woods; the Long Meadow at
(1950, 1610) south-east, below Observatory Hill; the Sketch Cabin at
(1960, 1556) beside it. Open meadow clearings lie along the top edge of
the forest belt.

**Now.** None. The clearings in the woods are the six trail ends
(`EG_CLEARING`, `plan.coax:523-525`).

### Roads

**Previous map.**

- The Commons Loop (label at (881, 1279)), the inner ring round the park.
- An outer ring road round the districts.
- Six roundabouts (r 17 px) on the Commons Loop, at (1200, 443) north,
  (1714, 699) north-east, (1928, 1013) east, (1178, 1293) south,
  (547, 1065) south-west and (609, 660) north-west.
- Six named radials: Main Street (north), Studio Road (north-east),
  Foundry Road (east), Library Way (south), Lantern Road (west) and Hearth
  Road (north-west). Main Street, Library Way, Lantern Road and Foundry
  Road run off the map's edge.
- Cross streets between the rings cut every district into blocks.
- Dashed footpaths from the outer ring to the cabins, the orchard, the
  beekeeper, Fernhollow, the lookout and Observatory Hill.

**Now.** Thirty nodes and thirty links (`EG_NODE_XZ`, `EG_LINK_AB`,
`EG_LINK_NAME`, `plan.coax:114-140`): the Commons Ring (eight links),
Inn Street and the Road In south, Market Street and the River Trail west,
Guild Row and the Deep Wood Trail east, the Temple Walk, Stargazers' Lane,
the North Lane and the Ruins and Crypt Trails north, and Mages' Lane.
Every street is 3 m wide (`EG_PATH_HALF`, `plan.coax:30`). There is no
outer ring, no roundabout and no cross street.

### Water

**Previous map.** Four ponds: Lantern and Reed in the Commons, Thinking
in Walden Woods, Fern in Fernhollow. One stream, Glade Run (label at
(700, 1080)), drawn as two blue channels leaving the Commons south-west of
Lantern Pond, crossing the Commons Loop and Brownstone Row, and ending in
Thinking Pond. The previous build's text has Glade Run leave Reed Pond and
run south through the long meadow (`previous/everglade.md:337-339`); the
drawing and the text disagree.

**Now.** The four ponds as carved bowls with still water, 1.6 to 3 m deep
(`EG_POND`, `plan.coax:244-249`), each off a trail outside the walls. One
river, straight along x = 88 (west of the city) from the north edge to
the south, 4 m between banks and 6 m of water, 2.2 m deep, with a weir at
z = −30 and a plunge pool (`plan.coax:314-329`), a bridge
(`EG_BRIDGE`, `plan.coax:111`) and a glade to wade in (`EG_GLADE`,
`plan.coax:528`). Ducks, lily pads and rowboats at three ponds float on
the water system (`verse/everglade/river.coax:25-44`).

### Coast

**Previous map.** None. The map shows no sea, beach or estuary. The coast
was a separate zone (`previous/coast.md`), reached by a trail gate "where
Glade Run leaves Walden Woods" (`previous/coast.md:96-98`), with the
estuary 250 m down the stream (`previous/coast.md:147`).

**Now.** None, and no trail leaves for one.

### Terrain

**Previous map.** Flat paper with one marked rise, Observatory Hill,
drawn with contour rings. The previous build levelled a 272 m clearing
(`previous/everglade.md:331`).

**Now.** A heightfield (`aethyr/terrain.coax`) with gentle hills inside
the walls, flat under every building and square, rising into the forest
from 80 m (`EG_RIM_FROM`, `EG_RIM`, `plan.coax:28-29`). The river's south
end cuts a ravine (`plan.coax:312-313`). No named hill rises; Temple Hill
is level 0.

### The forest

**Previous map.** A forest belt rings the whole sheet, thicker at the
corners, with two named woods (Walden Woods, Fernhollow) and meadow gaps
on the north edge. Trees also stand through the Commons and along the
streets.

**Now.** Past a 6 m apron round the palisade (`EG_APRON`,
`plan.coax:518`), everything is woods but the trails (3 m either side,
`EG_TRAIL_FREE`, `plan.coax:519`) and six clearings. You walk among the
trunks; a thicket at 101.3 m (`EG_THICKET`, `plan.coax:27`;
`plant_thicket`, `apps/openagents-web/everglade.coax:786`) ends the world.
No trees stand inside the walls; the live screenshot shows open grass
between the buildings.

### Gates and edges

**Previous map.** No wall and no gate. The city is open; its radial roads
and footpaths run into the forest and off the sheet.

**Now.** A palisade of sixteen boxes, 4.5 m high, with towers of 7 to
7.5 m (`EG_PALISADE`, `plan.coax:40-50`), and four gates: South (6 m),
North, East and West (5 m each) (`EG_GATE`, `plan.coax:52-54`).

## Gap table

Size of the work: **S** a day or less, one file; **M** a few days, the
plan and the realm; **L** a week or more, new buildings or systems;
**XL** a re-plan of the world's extent and every check's budget.

| Feature | Previous map | Now | Gap | Size |
| --- | --- | --- | --- | --- |
| Town extent | Ring road about 1,950 px; built as a 272 m clearing in a 300 m tree ring | Palisade 127 × 115 m; ground 208 m | Town about 2.1× wider, 4.5× the area; the world is smaller than the old town | XL |
| Building count | About 700 footprints | 16 buildings (`plan.coax:58`) | About 680 buildings, most of them terraced houses | XL |
| City form | Open, two concentric rings round a park | Square palisade, grid of four streets | Ring form and its blocks | L |
| Commons | Park about 180 × 110 m (derived): lawn, paths, trees | Paved round square 32 m across | A great lawn round or beside the paved square | L |
| Fountain | Fountain Plaza on the park's north edge | Fountain at the commons' middle | Position differs; both have one | — |
| Lantern Pond | In the Commons, about 50 m across (derived), boathouse | Outside the east wall, radius 6 m | Move into the commons and grow it | M |
| Reed Pond | In the Commons, jetty | South of the stables, radius 5 m | Move into the commons | M |
| Bandshell, Sculpture Walk, Community Gardens | On the Commons | None | Three park landmarks | M |
| Boathouse | Lantern Pond's north bank | None (rowboats exist) | One building | S |
| Main Street and Market Hall | North radial, shops, Market Hall | Market square, six stalls, general store | A shop street and a market hall | L |
| Stoop Lane | About 150 townhouses | None | Residential district | L |
| Lantern Quarter, Music Hall | About 80 buildings, round Music Hall | The inn | Music Hall, pubs, meeting halls | L |
| Brownstone Row | About 120 homes | None | Residential district | L |
| Knowledge District | The Stacks, Old College, about 80 buildings | Temple and observatory | Library and college | L |
| Observatory Hill | Hill with contours, observatory on top | Observatory on flat ground inside the walls | A raised hill; the observatory on it | M |
| The Foundry | Server Barn & Fab Yard, about 33 buildings | Smithy | Workshop district | L |
| Creative District | Makers' Hall, Boardwalk Cafés, about 85 buildings | Guild Row in the same sector | Studio district and cafés | L |
| Ring roads | Commons Loop and outer ring | Commons Ring of radius 12 m | Two rings at town scale | M |
| Roundabouts | Six on the Commons Loop | None | Six junctions | S |
| Radial roads | Six, named | Four streets to four gates | Two more radials; rename to match | M |
| Cross streets | Between the rings, in every district | None | Block grid | M |
| Glade Run | Stream from the Commons to Thinking Pond | Straight river at x = 88, called Glade Run's weir in `plan.coax:321` | A stream through the town; a footbridge on Brownstone Row | L |
| Walden Woods | Thinking Pond, three cabins | Thinking Pond north of the walls, no cabins | Cabins, the pond in the south-west | M |
| Fernhollow | Fern Pond, Code Cabin, Lookout Hut | Fern Pond east of the walls | Two buildings, a lookout | M |
| Orchard, Beekeeper's Hut, Long Meadow, Sketch Cabin | Present | None | Four places | M |
| Footpaths to the woods | Dashed paths to every cabin | Six trails to quest places | Paths to the new places | S |
| Coast | Not on the map (a separate zone) | None | Out of scope for the map; a trail gate later | — |
| Gates | None (open city) | Four palisade gates | The current city adds them | — |

## What the current city has that the map does not

- The palisade, its four gates and gate towers (`plan.coax:40-54`).
- The road in from the south and the stables outside the gate
  (`plan.coax:62`, `191`).
- The inn, the general store, the smithy, the alchemist, the fighters'
  hall, the rangers' lodge, the mages' tower and the temple
  (`EG_BUILDING_NAME`, `plan.coax:71-76`).
- Sixteen walk-in interiors with doors that open and shut
  (`EG_INTERIOR`, `plan.coax:80-83`).
- Quest places past the walls: the deep wood's waystone, the ruins' three
  pillars, the crypt and the river crossing (`EG_PROP`,
  `plan.coax:100-103`; `EG_PLACE_NAME`, `plan.coax:149-152`).
- The quest board on the commons and the quests (`verse/everglade/quests.coax`).
- A long river you wade, swim and dive in, with a weir, a bridge, ducks,
  lily pads and rowboats (`plan.coax:314-329`;
  `verse/everglade/river.coax`).
- Twenty-two named people and cats who talk and remember you
  (`EG_SPOT`, `plan.coax:162-189`; `verse/everglade/people.coax`).
- Hills inside and round the town (`aethyr/terrain.coax`).
- The 5e spell bar, destruction and the watch
  (`docs/verse/everglade.md:574-799`), and admin edit mode
  (`docs/verse/everglade.md:800-916`).

## Recommended build order

The current plan is a deliberate medieval hub ("keeps its commons,
fountain, observatory and market stalls and drops the workshop and the
Agent Studio", `docs/verse/everglade.md:20-23`). The steps below bring the
map's form and places to it in the medieval kits. They keep the palisade
and the quest places unless the owner decides to open the city. Each step
ends with `coax/bin/coax test --library verse/everglade/plan.coax` and
`python3 verse/everglade/map.py docs/verse/everglade-map.svg`.

1. **Grow the world (XL).** Raise the ground and the walk bound so a
   300 m town fits with woods round it: `EG_CELLS`, `EG_EDGE`, `EG_BOUND`,
   `EG_THICKET`, `EG_RIM_FROM` (`plan.coax:20-29`), the reach grid
   `EG_REACH_N`, `EG_REACH_WEST` (`plan.coax:550-552`) and the `#steps`
   budgets of the checks (`plan.coax:627-893`); move the river's
   `EG_RIVER_X` and `EG_RIVER_COL` (`plan.coax:314-320`) and the quest
   places, clearings and spots out with the edge. Touches `plan.coax`,
   `apps/openagents-web/everglade.coax` (forest planting and
   `plant_thicket`), `verse/everglade/river.coax`, `map.py`,
   `docs/verse/everglade.md` (Scale, Walking times). Check the frame rate on
   a phone before going on.
2. **Re-lay the roads (M).** Replace the four-street grid with the
   Commons Loop, an outer ring, six roundabout nodes and the six named
   radials, and widen the palisade to the outer ring with a gate at each
   radial that leaves town. Touches `EG_PALISADE`, `EG_GATE`,
   `EG_NODE_XZ`, `EG_LINK_AB`, `EG_LINK_NAME` (`plan.coax:40-140`) and the
   gate names; `EG_PLACE*` (`plan.coax:144-158`); the hashes in
   `apps/openagents-web/realms/everglade/index.md`.
3. **Make the Commons a park (L).** Keep a paved Fountain Plaza on its
   north edge; lay lawn, footpaths and park trees inside the loop; move
   Lantern Pond and Reed Pond into it at the map's sizes; add the
   bandshell, the boathouse on Lantern Pond, the Sculpture Walk and the
   community gardens. Touches `EG_SQUARE`, `EG_POND`, `EG_POND_OPEN`,
   `EG_PROP` (`plan.coax:94-111`, `206-252`); the tree planting in
   `apps/openagents-web/everglade.coax` (trees inside the walls for the
   first time); the boat moorings in `verse/everglade/river.coax`.
4. **Bring Glade Run through town (L).** Replace or add to the straight
   river with a stream from the Commons, under the loop and a footbridge
   on Brownstone Row, to Thinking Pond in the south-west. Touches
   `everglade_carve_river` and the river constants (`plan.coax:306-390`),
   `verse/everglade/river.coax` (a curved river body), `EG_BRIDGE`, and the
   water docs (`docs/verse/water.md`).
5. **Place the district landmarks (L).** One building each, as walk-in
   boxes: Market Hall (Main Street), the Music Hall (Lantern Quarter, the
   round footprint needs a round-wall builder in `building()`,
   `apps/openagents-web/everglade.coax:1955`), The Stacks and the Old
   College (Knowledge District), the Server Barn (the Foundry; in the
   medieval kits a forge or workshop hall), Makers' Hall and the Boardwalk
   Cafés (Creative District). Move the existing buildings into their
   districts: the market and store to Main Street, the inn to the Lantern
   Quarter, the smithy to the Foundry. Touches `EG_BUILDING`, `EG_DOOR`,
   `EG_BUILDING_NAME`, `EG_INTERIOR`, `EG_ROOM_ID` (`plan.coax:58-89`),
   `EG_SPOT` (`plan.coax:162-189`), `verse/everglade/people.coax`.
6. **Raise Observatory Hill (M).** A raised flat with a slope, the
   observatory on top, in the south-east past the Knowledge District, with
   the Long Meadow below it. Touches the flats (`EG_SQUARE`,
   `everglade_flats`, `plan.coax:206-226`) and `aethyr/terrain.coax` if
   FLAT needs a raised level with a ramp.
7. **Fill the districts with houses (XL).** Terraces along cross streets
   in Stoop Lane, Brownstone Row and the other arcs. Sixteen boxes become
   hundreds, so this needs a row-house generator (a terrace as one box
   split into units) rather than hand-entered boxes, and instanced
   drawing; most houses stay shut (`EG_INTERIOR` false). Touches
   `plan.coax` (a generated block table and the overlap and reach checks),
   `apps/openagents-web/everglade.coax` (`building()`, house paint),
   `verse/packs/everglade.json` if new kit pieces are needed.
8. **Furnish the woods (M).** Walden Woods with the Writing Cabin, Quiet
   Cabin and Prototype Shed round Thinking Pond; Fernhollow with the Code
   Cabin and Lookout Hut by Fern Pond; the Orchard, the Beekeeper's Hut
   and the Sketch Cabin; footpaths to each. Touches `EG_BUILDING`,
   `EG_CLEARING` (`plan.coax:520-525`), `EG_NODE_XZ` and `EG_LINK_AB`,
   `everglade_in_woods`.
9. **Leave room for the coast (S).** Reserve a trail from Walden Woods
   along Glade Run's outflow to the world's edge for the later coast
   zone (`previous/coast.md:96-111`). Touches `EG_NODE_XZ`, `EG_LINK_AB`.

## Open questions for the owner

- Keep the palisade and gates, or open the city as the map draws it?
- Keep the medieval roles (guilds, temple, crypt) beside the map's
  districts, or rename the districts to medieval trades?
- The target size: the previous build's 272 m clearing, or a smaller
  town that keeps the hub "half a minute across"
  (`docs/verse/everglade.md:50-53`)?
