# How SLAB is built: the construction method

This note says how you would build the SLAB hull of
[the world spec](slab-world-spec.md), step by step, with numbers. It turns
steps 2 to 4 of "How it could be built" in
[the feasibility note](slab-image-feasibility.md) into a method that a
simulation can follow. The simulation is `metaverse/slab-build.coax`. This
note is the single source for its sequence: when the two disagree, one of
them has a bug.

SLAB has published no design. Nothing here is Luckey's. Every number below
has one of four tags:

| Tag | Meaning |
| --- | --- |
| **sourced** | From a named source in [Sources](#sources). |
| **assumed** | Our assumption. Change it and the result changes. |
| **decided** | Set by the hull decision in `metaverse/hull.coax` and the spec, not by this note. |
| **computed** | A result of the simulation or of arithmetic on the numbers above. Simulation results are for seed 1 and the default settings unless the text says otherwise. |

Run the checks that hold these numbers:

```
coax/bin/coax test metaverse/slab-build.coax
coax/bin/coax test metaverse/slab-build-view.coax
```

## The sequence in short

1. **Cast.** Six dry casting cells at a yard on the Corpus Christi ship
   channel each build one caisson at a time: bottom slab, slipformed walls,
   top slab, cure and leak test. A caisson takes about 52 days in its cell.
2. **Float out.** Flood the cell and warp the caisson into the yard's
   basin. It floats dry at 10.91 m.
3. **Make up a pair.** Join two caissons end to end in the sheltered basin
   and ballast the pair for the tow: 200 × 50 m, 117,862 t, 11.50 m draft.
4. **Tow.** Two 150 t tugs lead and two 80 t tugs escort the pair down the
   channel, out between the Aransas Pass jetties and 40 nautical miles
   across the shelf. A tow leaves at 06:00 when the forecast shows a window,
   and takes 13 hours.
5. **Moor the first pair.** The first pair is moored at the berth on the
   world's sixteen lines.
6. **Join at sea.** Each later pair stands by until the sea is under 1.5 m,
   is pulled in on fenders, sealed, drained, and made fast on temporary
   bars. Permanent tendons and grout follow. The hull grows outward from
   its centre.
7. **Line the sea walls.** Once an edge caisson is fast, its outer wall is
   thickened from 0.6 m to 1.5 m from inside.
8. **Raise the upper structure.** Over each caisson, the outer wall and the
   columns climb 89.4 m in jump forms, then the girders are cast, then the
   deck slab. The deck ends 143 m over the keel.
9. **Land the city.** Barges bring modules. Two gantries on the deck edge
   lift them in pieces of up to 1,500 t onto each bay whose deck is cast.
10. **Stand down for storms.** At a tropical storm or hurricane warning,
    tows, joins, pours and lifts stop and storm lines are made fast.
11. **Ballast and commission.** Pump the hull down to 25 m and commission
    it.

The default run finishes in 1,510 days (4.1 years) (computed).

## What is built

All of this is **decided** by `metaverse/hull.coax`. The simulation reads
it and has no hull numbers of its own.

| Item | Value |
| --- | --- |
| Hull | 1,000 × 600 m, 120 identical caissons, 10 along the length and 12 across |
| Caisson | 100 × 50 × 50 m; 0.6 m walls, one web each way at mid span, 1.5 m bottom slab, 1.0 m top slab |
| Concrete in a caisson | 22,372 m³: 7,500 bottom, 9,872 walls, 5,000 top |
| Caisson, dry | 55,931 t; floats at 10.91 m with a GM of 2.09 m |
| Caisson, for the tow | 58,931 t with 3,000 t of pressed-up water ballast; 11.50 m; GM 2.48 m |
| Upper structure | To a deck 143 m over the keel: 0.8 m outer wall, 209 columns of 15 m², 2 × 3 m girders, 0.6 m deck slab. 1,008,180 m³ |
| Sea-wall lining | Outer walls from 0.6 m to 1.5 m at the berth. 133,876 m³ |
| Concrete in all | 3,826,744 m³ (9.57 Mt) |
| Topsides and liquids | 2.0 Mt and 0.5 Mt |
| Ballast | 3.31 Mt of sea water |
| Finished | 15.375 Mt at 25 m draft, deck 118 m over the sea |

### The mass ledger against the earlier figures

The feasibility note uses 2.41 million m³ of concrete and 804 days of
placing at 3,000 m³ a day. This hull has 3.83 million m³, so 1,276
placement days at the same rate (computed). The difference is the hull
decision: 120 closed caissons have double walls at every seam (2.68 million
m³ against 2.41), the deck stands 143 m over the keel on a real upper
structure (1.01 million m³), and the sea walls are lined (0.13 million m³).
The simulation keeps both numbers honest: one check places 2.41 million m³
at 3,000 m³ a day in 804 days, and the same check places this hull in
1,275.58.

## Precedents

| Project | What it shows | Figures | Tag |
| --- | --- | --- | --- |
| Fehmarnbelt tunnel | A factory casting very large concrete elements in a line, floated out through a basin | 79 standard elements, each 217 m long and 73,500 t; six production lines at Rødbyhavn | sourced [1] |
| Øresund tunnel | Elements cast ashore, towed and joined under water with rubber gaskets | 20 elements, each 176 m and 55,000 t; Gina gasket and Omega seal at each joint | sourced [2] |
| Troll A | Slipformed concrete at the largest scale, towed deep with a tug fleet | 245,000 m³ of concrete, 656,000 t dry; ten tugs, about 1 knot, seven days | sourced [3] |
| Condeep platforms | Slipform rates for tall concrete walls | 1.5 to 4 m of wall a day | sourced [4] |
| Monaco floating breakwater | One prestressed caisson built in a dry dock and towed in open sea | 352 m, about 163,000 t; built at Algeciras, towed about 900 nautical miles in 12 days in August 2002 | sourced [5] |
| Mega-Float, phase 2 | Units joined afloat into one kilometre-long structure | About 1,000 m, six units joined at sea (steel, welded) | sourced [6] |
| Prelude FLNG | A very large hull towed across an ocean and moored on chains | 488 × 74 m; towed 5,800 km; moored on 16 chains | sourced [7] |
| Hibernia | A concrete base of SLAB's order of mass towed by a fleet | About 600,000 t of base; nine tugs | sourced [8] |

What they do not show: nobody has joined 120 concrete units afloat in the
open sea, and nobody has cast 90 m of structure on top of a floating hull.
Those two steps carry the most risk in this method.

## The unit

Keep the 100 × 50 × 50 m caisson (decided).

- **It fits the channel.** At 11.50 m it has 4.96 m of still water under it
  in a channel dredged to 16.46 m (computed from sourced [9]).
- **It floats upright by itself.** GM is 2.09 m dry and 2.48 m with tow
  ballast (decided, and checked by the simulation with
  `metaverse-engine/physics/afloat.coax`). The ballast is pressed up in
  closed cells. The same 3,000 t left free to run across the cells capsizes
  the caisson: a check shows it.
- **It is within what has been cast.** A caisson is 55,931 t. Fehmarnbelt's
  elements are 73,500 t and Øresund's were 55,000 t (sourced [1], [2]).
- **Slipforming is in a dry cell, not afloat.** Condeep shafts were
  slipformed afloat in a deep fjord. The Texas coast has no deep sheltered
  water, so the walls go up in the dry and the caisson floats out whole, as
  the Monaco caisson did (sourced [5]).

## The yard

The yard is at Ingleside, on the north side of the Corpus Christi ship
channel (assumed site, 27.82° N, 97.20° W). It has dry casting cells behind
a quay, a wet basin beside the channel, and a module quay.

| Step in a cell | Rate or time | Tag |
| --- | --- | --- |
| Make the cell ready, fix bottom steel | 6 days | assumed |
| Pour the bottom slab, 7,500 m³ | 2,500 m³ a day: 3 days | assumed |
| Assemble the slipform | 4 days | assumed |
| Slipform the walls, 47.5 m | 3 m a day: 15.8 days, 624 m³ a day | sourced range [4], our choice in it |
| Falsework and steel for the top slab | 6 days | assumed |
| Pour the top slab, 5,000 m³ | 2,500 m³ a day: 2 days | assumed |
| Cure, prestress, fit out, leak test | 12 days | assumed |
| Flood the cell, warp the caisson out | 1 day | assumed |
| Pump the cell dry | 2 days | assumed |

- **Casting cells:** six (assumed; the panel's slider runs 1 to 8). Each
  cell floor is 13.5 m under the sea (assumed), so a dry caisson has 2.6 m
  under it when the cell is flooded.
- **Concrete supply:** 3,000 m³ a day from all plants, ashore and afloat
  (the feasibility note's rate; the slider runs 1,000 to 6,000). The cells
  have first call. A slipform never waits: walls are served first, then top
  slabs, then bottom slabs. The work at sea gets what is left.
- **Failures:** 3% of slipforms stop for 3 days part way up; 5% of caissons
  fail the leak test and take 6 more days (assumed rates). The default run
  has 8 of each (computed).
- **Wet storage:** floated caissons wait in the basin, 14 m deep (assumed).
  One crew joins caissons into tow pieces there, 6 days a joint (assumed).
- **The first caisson** floats out on day 50 and the last on day 1,152
  (computed).

Six cells at these rates use 2,560 m³ a day on average (computed), which is
why six is the default: more cells wait for concrete, and fewer leave the
plant idle. Fehmarnbelt's factory, at 79 elements of 73,500 t, cast about
5.8 Mt (computed from sourced [1]). SLAB's 9.57 Mt is 1.6 times that.

## The tow

### The route

The route is data (`cs_route_gulf` in `slab-build.coax`), so another site
is another table. Coordinates match `metaverse/slab-where.coax`.

| Reach | Length | Course | Least depth | Channel width | Planned speed | Tag |
| --- | --- | --- | --- | --- | --- | --- |
| Ingleside to Harbor Island, Corpus Christi ship channel | 7.0 nmi | 078° | 16.46 m (54 ft) | 161.5 m (530 ft) | 3 kn | depth and width sourced [9]; the rest assumed |
| The bend into Aransas Pass | 1.6 nmi | 128° | 16.46 m | 161.5 m, 200 m in the bends | 3 kn | assumed |
| The jetty and outer bar channel | 4.0 nmi | 122° | 17.07 m (56 ft) | 161.5 m | 3 kn | depth sourced [9]; the rest assumed |
| The inner shelf | 8.0 nmi | 115° | 20 m | open | 4 kn | assumed |
| The shelf to the berth | 28.0 nmi | 115° | 30 m, falling to 89 m | open | 4 kn | assumed |

The route is 47.6 nautical miles from the basin to a stand-by point
1,800 m short of the berth (computed). Channel bends are drawn at a
3,000 m radius in reaches of 3° or less (assumed). The sea in the first
reach is 15% of the open sea's, and 50% in the bend (assumed). A tide of up
to 0.5 m/s runs along the channel (assumed).

### The fleet and the line

| Item | Value | Tag |
| --- | --- | --- |
| Lead tugs | 150 t bollard pull each | assumed |
| How many | Enough to hold the piece head on in a 3 m sea, 15 m/s of wind and 0.5 m/s of current, plus one; two at least. A pair needs two; a quad needs three | rule assumed; counts computed |
| Escort tugs in the channel | Two of 80 t, on the quarters, to steer and stop | assumed |
| Towline | 76 mm wire, 200 N/m in water, stiffness 270 MN; breaking load twice the tug's bollard pull: 2.94 MN | assumed |
| Line paid out | 250 m in the channel, 300 to 400 m outside | assumed |
| Tugs' return | Light, at 10 knots | assumed |

### When a tow may leave

- At 06:00, so the channel is run in daylight (assumed).
- The channel is closed to other traffic for the transit. The pair is
  3.2 beams wide of channel: less than the 3.6 to 5 beams a concept design
  gives a ship under her own power (assumed), so a real project needs a
  pilotage study and the pilots' agreement.
- The forecast must show a sea under 2.0 m (0.8 of the 2.5 m limit) and
  wind under 12 m/s for 21 hours: 1.5 times the planned passage, in whole
  three-hour steps (assumed, in the form marine warranty practice uses; the
  values are not checked against DNV-ST-N001).
- No storm stand-down is forecast before the tugs are home.
- At most two pieces may be standing by at the berth (assumed).
- 2% of tows are put back a day by a tug breakdown (assumed).

### What the simulation finds

The simulation integrates every tow in 15-second steps: tug thrust, the
towline's catenary, hull and wind drag, wave drift, the tide and the sea.
These are results, not inputs (computed, worst of the 60 tows):

| Result | Value | Limit |
| --- | --- | --- |
| Time for the passage | 13.0 hours | planned 12.9 |
| Steady towline tension | 1.11 MN | |
| With the waves' snatch | 1.65 MN | 2.94 MN breaking load |
| Least water under the keel | 4.24 m | over 0 |
| Least room to a channel edge | 36.1 m | over 0 |
| Least water under the towline's sag | 3.3 m | over 0 |

A tow that grounds, strikes a bank or parts its line stops the run and says
why. With the weather limit ignored, tows sail into storms and at least one
parts its line: a check covers both cases. Weaker tugs change the answer:
three 80 t tugs make the passage, but their lighter-loaded lines sag onto
the channel bed (computed), so they must be shortened.

## The assembly order

The simulation runs each order and checks every intermediate shape for
stability and for hull-girder stress in a design storm of 8 m significant
wave height, 12.4 s, 40 m/s of wind and 1 m/s of current (assumed). The
joints carry 10 MPa of prestress (assumed), so stress over 10 MPa opens a
joint.

| Order | Tows | Joints at sea | Worst stress, head on | Worst stress, sideways | Least room to a bank | Days | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Row by row, single caissons | 120 | 119 | 10.1 MPa | 19.1 MPa | 46.9 m | 1,489 | Fails: a strip 1,000 m long and 50 m wide bends sideways past its prestress |
| Compact, single caissons | 120 | 119 | 9.5 MPa | 1.1 MPa | 46.9 m | 1,495 | Passes, with twice the open-sea joints |
| **Compact, pairs made in the basin** | **60** | **59** | **9.4 MPa** | **3.6 MPa** | **36.1 m** | **1,510** | **Chosen** |
| Compact, quads made in the basin | 30 | 29 | 9.5 MPa | 2.3 MPa | 21.3 m | 1,508 | Passes on stress; a 100 m beam leaves 21 m to the bank and needs a third lead tug |

All figures are computed. "Compact" grows outward from the centre in rings.

Pairs are chosen. They halve the joints made in the open sea, which are
the weather-bound and riskiest step, and they keep 36 m to the channel
edge. Quads halve the joints again and lose the fewest days to weather,
but a 100 m beam fills 62% of the channel. Row by row is ruled out by the
sideways stress.

The head-on figure is close to the limit in every order: 9.4 MPa against
10 MPa. That is the finished 1,000 m length at light draft in the design
storm. The prestress, the design storm and the stress model are all ours,
so read this as "the margin is thin", not as a pass.

## The joint

Each step has a duration and the weather it needs (all assumed).

| Step | Time | Needs |
| --- | --- | --- |
| Moor the first pair on the sixteen lines | 48 h | Sea under 1.5 m |
| Bring a pair alongside on its tugs, pass lines | 3 h | Sea under 1.5 m, wind under 12 m/s |
| Pull in on four fenders a face; shear keys engage | 3 h | Motion between the faces under 0.15 m significant |
| Press the gasket, drain the joint | 6 h | Sea under 1.5 m |
| Stress temporary bars: the pair is now part of the hull | 24 h | Stress in the joint under 1 MPa if the sea rises over 2 m |
| Thread and stress the permanent tendons | 72 h | |
| Grout and test | 36 h | |
| Reseal a joint that fails its test | 36 h | 8% of joints |

- **Draft matching.** The pair arrives at 11.50 m. The hull draws 11.5 m
  early and 15.5 m by the last pair (computed). The pair takes on pressed-up
  ballast to match before it is pulled in. The simulation does not time
  this step or check the pair's stability at the deeper draft.
- **A window.** A join starts only when the forecast shows the first four
  steps (36 hours) under the limits and no stand-down before the tendons
  are in.
- **Abort.** If the sea passes 1.5 m before the bars are stressed, the pair
  backs off and tries again.
- **The seal** follows immersed-tunnel practice: a soft rubber gasket
  pressed by pulling in, then by the sea when the joint is drained
  (sourced [2] for the practice). The sea closes a drained joint with at
  least 33 MN (computed).
- **Two crews** work at once (assumed).

What the simulation finds (computed): no join is aborted in the default
run; the highest sea with a joint open is 1.18 m; the faces move at most
0.06 m against each other; a fender takes at most 0.96 MN of its 4 MN
rating; 4 joints are resealed. A pair waits 5.1 days on average at the
berth before its join starts. With the limit ignored, 143 joins are aborted
and the build finishes 9 days later.

## The moorings

The hull is moored on the world's rig (spec section 4.3,
`metaverse/slab-moor.coax`): sixteen lines, four fanned at each corner,
each a 40 MN bundle laid at 4 MN (decided). In 89 m of water each is a
chain catenary 1,076 m long to an anchor 1,058 m out, heavy enough that
chain still lies on the seabed at breaking load, so no anchor is pulled
upward (computed by that module).

**The sixteen lines hold working weather.** In a 2.5 m sea, 15 m/s of wind
and 0.5 m/s of current on the long side, the finished hull is pushed with
30 MN, moves 10.9 m and loads its worst line to 11.8 MN (computed).

**They do not hold a hurricane on the 143 m wall.** The design storm pushes
the finished hull with 211 MN beam on: 139 MN of wind on the wall, 59 MN of
wave drift and 13 MN of current (computed). Without the upper structure the
same storm pushes with 101 MN. Beam on, a line reaches what it may carry
(40 MN / 1.67) at a push of 66 MN and breaks at 114 MN (computed). The spec
says the lines were not resized when the wall grew (section 4.4); this is
what that costs.

**So the method adds storm lines.** They are more of the same chain, laid
square to the sides as the hull grows: four legs on each long side and two
on each short side (assumed layout). At a storm warning they are made fast,
with as many chains in each leg as the design storm needs for the hull as
it then stands, so that no chain passes 40 MN / 1.67 from ahead, abeam or
the quarter.

| Stage | Hull so far | Design-storm push, beam on | Sixteen lines enough? | Storm chains |
| --- | --- | --- | --- | --- |
| Pairs 1 to 5 | up to 400 × 125 m | 18 to 41 MN | Yes | 0 |
| Pair 6 | 600 × 100 m | 67 MN | No | 8 |
| Pair 20 | 600 × 333 m, wall at 137 m | 115 MN | No | 8 |
| Pair 30 | 600 × 500 m | 121 MN | No | 12 |
| Pair 40 | 1,000 × 400 m | 201 MN | No | 16 |
| Finished | 1,000 × 600 m | 211 MN | No | 20 |

All computed. "Hull so far" is its length by its mean width. The default
run has two storms. The tropical storm of the first September (day 254:
4.8 m, 26.5 m/s) finds a 600 × 200 m hull with 8 storm chains fast. Its
push of 35 MN would not have broken the sixteen lines alone. The hurricane of the fourth summer (day 1,298:
6.8 m, 35.7 m/s) pushes the finished hull with 151 MN. With the 20 storm
chains fast the worst chain carries 16 MN. With the sixteen lines alone a
line breaks and the run stops there: a check covers both cases.

Anchor type is not settled here. The chain is long enough that drag
anchors or piles see no uplift.

Two things the simulation does not do: it does not time the laying of
lines or the moving of a corner's lines as the hull grows, and it lays each
stage's rig about that stage's corners. A real project would lay the
permanent anchors once and lengthen the lines while the hull is small.

## Lining the sea walls

The 40 caissons on the hull's edge are towed with 0.6 m outer walls and
lined to 1.5 m at the berth, from inside (decided). The lining is placed by
pump as each edge caisson is made fast, in seas under 2.5 m (assumed). It
is 133,876 m³ and finishes on day 1,179, two days after the last pair is
fast (computed).

## The upper structure

The upper structure is 1,008,180 m³ and 93 m tall (decided). The simulation
builds it one caisson's bay at a time, in three parts, and gives each bay
an equal share (assumed; the outer wall really belongs to the edge bays).

| Part | A bay's share | How | Rate | Tag |
| --- | --- | --- | --- | --- |
| Outer wall and columns | 4,241 m³, 89.4 m tall | Jump forms, 4 m a lift | 0.8 m of height a day, a lift every five days | assumed |
| Girders, 3 m deep | 1,161 m³ | Cast on falsework between the columns | 20 days at least | assumed |
| Deck slab, 0.6 m | 3,000 m³ | Cast on the girders | 10 days at least | assumed |

- **When it may start.** No upper concrete goes on until the joined hull
  could carry every load it will ever carry and keep a GM of 5 m (assumed
  rule). One pair alone could not: with its full share it would have a GM
  of minus 33 m (computed). The third pair makes a 200 × 150 m hull that
  can (day 83.5). The first upper concrete is placed on day 96.
- **Order.** Bays are served in the order their caissons were joined.
- **Wind at height.** Wind over open sea grows with height as
  (height / 10 m)^0.11 (assumed), so it is 31% stronger at the deck than at
  10 m. A bay's forms stop when the wind at their height passes 20 m/s
  (assumed). Pours stop in seas over 2.5 m.
- **Lifts.** 22 jump-form lifts a bay: 2,640 in all (computed).
- **Stability as it rises.** Once a day the simulation checks the hull's GM
  and the heel the design storm's wind would give it. The least GM of the
  joined hull during the build is 143 m, and the largest heel is 0.0018
  radians (0.1°) (computed). A hull with less than 1 m of GM stops the run.
  The wind is taken on a full wall up to the highest concrete, which is an
  upper bound while only some bays have risen.

The first bay's deck is cast on day 542 and the last on day 1,430
(computed). The first bay takes 445 days and not 142, because the yard has
first call on the concrete and the bays share what is left.

## Topsides

The manifest is built from `metaverse/slab-plan.coax` (`cs_manifest`). Each
record of the plan (a tower, a plant hall, a grove) becomes a module with a
share of the ledger's 2.0 Mt, at its place on the deck. The split between
kinds is assumed: a weight for each cubic metre of each kind of building,
scaled so the total is the ledger's.

| Kind | Records | Mass, kt | Heaviest record, kt | Lifts |
| --- | --- | --- | --- | --- |
| Towers | 34 | 720 | 49 | 480 |
| OTEC plant halls | 4 | 334 | 143 | 223 |
| Colonnades | 8 | 287 | 98 | 191 |
| Industry halls | 4 | 198 | 145 | 132 |
| Groves (soil and planting) | 10 | 141 | 58 | 94 |
| Statues | 6 | 95 | 38 | 64 |
| Data halls | 2 | 91 | 49 | 61 |
| Temples | 5 | 41 | 23 | 27 |
| Civic | 2 | 31 | 16 | 21 |
| Medical | 1 | 30 | 30 | 20 |
| Stacks, plaza, tanks, desalination, containers, cooling, fountain | 23 | 31 | 8 | 21 |
| **All** | **99** | **2,000** | | **1,357** |

All computed from the plan and the assumed weights. A kind's lifts are its
mass over 1,500 t. The run makes 1,357 because a barge's last lift is often
a part load.

- **The largest single lift is 1,500 t** (assumed). Every record is landed
  in lifts of that size or less. The heaviest record, an industry hall, is
  97 lifts.
- **Lift method.** Two gantries on the deck edge lift from a barge lying
  alongside the lee long wall (assumed). The deck is 124 to 130 m over the
  sea while the city goes on, so each lift is a 130 m hoist. A module is
  landed only on a bay whose deck slab is cast. Lifts stop in seas over
  2.0 m or wind over 15 m/s (assumed). A gantry is out of service 5% of
  days (assumed).
- **The dock notch.** The spec's notch (section 3.1) is not cut in the
  drawn hull or in `hull.coax` yet, so this method lifts over the wall.
  When the notch exists, barges should discharge there.
- **Freight.** Three deck barges of 12,000 t (122 × 30.5 m, assumed) load
  for a day at the yard's module quay and cross on a 60 t tug when the
  forecast shows seas under 2.0 m. One barge at a time lies alongside. The
  default run makes 191 barge trips (computed).
- **Liquids.** Once half the topsides are on, tankers fill the tanks at
  25,000 t a day (assumed).

The first module lands on day 544 and the last on day 1,414 (computed).

### Where the freight loads

This method loads modules at Ingleside, beside the casting yard: 47.6
nautical miles from the berth, down the same channel, and
`metaverse/slab-where.coax` and the chart say the same (`WHERE_FREIGHT`).
The earlier timelapse loads freight at the Port of Brownsville. Brownsville is about 113 nautical miles away: 98 from the
Brazos Santiago Pass jetties (near 26.07° N, 97.15° W) to the berth
(computed), plus about 15 up its ship channel (assumed). That more than
doubles a barge's round trip. The simulation's barge passage is a distance
and a weather limit, so a Brownsville run is the same model with a longer
route. It is not built.

## Ballast and commissioning

- The hull is brought from its 0.36 Mt of tow ballast to the ledger's
  3.31 Mt by its own pumps at 150,000 t a day (assumed): 20 days.
- It ends at 15,374,950 t and 24.9999 m, level (computed; the ledger says
  15.375 Mt and 25 m). The city's weight is off centre, and trim ballast
  amounting to 11% of the ballast is carried off centre to level it
  (computed).
- Commissioning takes 60 days (assumed). The feasibility note's step 4
  lists what it must prove.

## Weather

The weather is a function of the seed, the start month and the time
(`cs_weather`), so any day can be asked for in any order.

| Item | Value | Tag |
| --- | --- | --- |
| Monthly mean significant wave height, January to December | 1.30, 1.35, 1.40, 1.35, 1.20, 1.00, 0.90, 0.80, 1.00, 1.20, 1.30, 1.30 m | assumed; NDBC buoy 42020's monthly climatology was not retrieved [10] |
| Spread about the mean | Log-normal, sigma 0.45, with systems of five days, fronts of a day and a half and squalls of ten hours | assumed |
| Wind | From the sea state: Hs = 0.21 U² / g, as in the world's sea | decided (`slab-sim.coax`) |
| Wind direction | From the south-east most days; a norther in the cool months | assumed |
| Shelf current | 0.15 m/s down the coast | assumed |
| Forecast error | Grows with lead time: 10% a day ahead | assumed |

### The hurricane rule

| Item | Value | Tag |
| --- | --- | --- |
| Chance of a first, second and third stand-down in a year | 38%, 10%, 2% | assumed |
| When | Most likely near 1 September, within about ten weeks either side | assumed |
| Share that bring a hurricane's sea to the site | One in four: one hurricane stand-down in about eight years (computed over 200 seeds of 48 years) | assumed; compare 13 to 19 years between hurricanes passing within 50 nautical miles of this coast, sourced [11] |
| A hurricane at the site | Sea 6 to 11 m, wind 33 to 50 m/s | assumed |
| A tropical storm at the site | Sea 3 to 5.5 m, wind 18 to 30 m/s | assumed |
| Stand-down | From 5 days before a hurricane's peak to 3 days after; 4 before and 1.5 after for a tropical storm | assumed |

During a stand-down no tow leaves, no join starts, an open joint is
abandoned, pours and lifts stop, barges stay in port and the storm lines
are made fast. The yard itself stops from two days before the peak to one
day after. Afterwards work resumes where it stopped.

Our hurricane rate is higher than the sourced return period suggests, so
the schedule is on the cautious side. The design storm (8 m, 40 m/s) is not
the worst the rule can produce: across 40 seeds the worst chain load at a
storm's peak is 28.5 MN, over the 24 MN a chain may carry and under its
40 MN breaking load (computed).

## Sites

| Place | Position | Tag |
| --- | --- | --- |
| Casting yard, Ingleside | 27.82° N, 97.20° W | assumed |
| Harbor Island, where the channel bends to the pass | 27.85° N, 97.08° W | computed from the route |
| Aransas Pass jetties | 27.834° N, 97.044° W | assumed, as in `slab-where.coax` |
| The berth | 27.55° N, 96.367° W | decided (`slab-where.coax`, the Texas site) |
| Port of Brownsville | 25.95° N, 97.40° W | assumed; the earlier timelapse's freight port, not used |

The berth is in 89 m of water, 40 nautical miles off the jetties and beyond
Texas's 9 nautical mile state line (decided). It is 47.6 nautical miles
from the yard along the route (computed).

## A tow of the finished hull

The simulation can tow the finished hull to another site
(`CsParams.ocean_nm`). It leaves after the city is on and before the final
ballast. The fleet is sized to hold it in a 5 m sea, 20 m/s of wind and
0.5 m/s of current (assumed, the form of the usual holding case): 30 tugs
of 300 t (computed). The open ocean's sea is taken as 1.6 times the Gulf's
(assumed), and the tow stops making way above 6 m. A 1,900 nautical mile
tow takes 18 days and moves the finish from day 1,510 to day 1,528
(computed). Thirty tugs of that size on one tow has no precedent: Troll A
used ten (sourced [3]).

## What the default run does

Seed 1, six cells, 3,000 m³ a day, a January start, pairs. All computed.

| Day | Event |
| --- | --- |
| 0 | The first cell starts |
| 50 | The first caisson floats out |
| 59 | The first pair leaves; it is moored by day 62 |
| 66 to 76 | The second pair arrives, waits 8.5 days for a calm, and is made fast |
| 96 | First concrete of the upper structure |
| 250 to 256 | Tropical storm stand-down (4.8 m, 26.5 m/s) |
| 542 | The first bay's deck is cast, 143 m over the keel |
| 544 | The first module is landed |
| 1,152 | The last caisson floats out |
| 1,177 | The last pair is fast: the hull is closed |
| 1,179 | The sea walls are lined |
| 1,293 to 1,301 | Hurricane stand-down (6.8 m, 35.7 m/s); 20 storm chains fast |
| 1,414 | The last module is landed |
| 1,430 | The deck slab is whole |
| 1,450 | At 25 m draft |
| 1,510 | Commissioned |

Totals: 1,510.25 days; 1,275.58 placement days; 60 tows; 59 joints at sea;
314 days on which weather held a tow, a join, a pour or a lift; two
stand-downs, one of them a hurricane; 20 reworks; 1,357 module lifts; 2,640
form lifts; 191 barge trips. The concrete plant is busy 89% of the time to
the last pour (1,276 of 1,430 days).

### What moves the finish date

| Change | Days to finish |
| --- | --- |
| Casting cells: 1, 2, 3, 4, 5, 6, 7, 8 | 6,545; 3,429; 2,380; 1,888; 1,584; 1,510; 1,451; 1,440 |
| Concrete a day: 1,000; 2,000; 3,000; 4,000; 5,000; 6,000 m³ | 4,022; 2,102; 1,510; 1,338; 1,321; 1,307 |
| 8 cells and 6,000 m³ a day | 1,065 |
| Start month, January to December | 1,500 to 1,520 |
| Seeds 1 to 40 | 1,490 to 1,531, mean 1,506; seven hurricane stand-downs; no run fails |
| One join crew, not two | 1,518 |

Past six cells or 4,000 m³ a day the schedule stops improving, because the
slipform rate, the climb rate of the upper structure and the freight then
set the pace.

## What the simulation does not do

- Moves inside the yard and basin, a pair's approach from stand-by to its
  joint, and tugs and barges on passage are placed along straight lines at
  the simulation's times. They are not integrated.
- The free surface of the joined hull's ballast is ignored.
- Laying and moving mooring lines takes no time. Anchors are not sized.
- Draft matching before a join is not timed.
- Every bay gets an equal share of the upper structure. The cranes that
  serve the jump forms are not bodies.
- A towline that sags onto the channel bed is reported, not failed.
- Freight loads only at the yard.
- Hurricane counts differ more between blocks of seeds than chance would
  suggest: 8, 28, 14, 15 and 18 in the first 1,510 days of five blocks of
  40 seeds. Seeds 1 to 40 are the quiet block. This is not investigated.
- The remote mutation run of these checks does not finish inside the
  pre-push time limit, so the checks have not been mutation-tested.

## For the drawing

`metaverse/slab-build-view.coax` gives everything in the build at one
moment. A renderer calls three things:

```
cs_run(p, run);                  // once for a seed and settings
cs_view_at(run, day, view);          // any day, with its fraction, in any order
run[0].total                     // the totals for a panel
```

`cs_view_at` fills a `CsvView`:

| Field | What it holds |
| --- | --- |
| `body[0 ..< bodies]` | Every caisson, tow piece, tug, barge and gantry: kind, state, position, yaw, roll and pitch, size, draft, mass, GM, how much is cast and cured, ballast |
| `bay[0 ..< units]`, `unit[i]` | The upper structure over each caisson: how high its wall and columns stand, and the share of its girders and deck slab cast |
| `hull` | The joined hull: extent, draft, heave, roll, pitch, offset on its moorings, GM, the weather's push, wind heel, bending stress, storm chains fast |
| `line[0 ..< lines]` | Towlines, the sixteen mooring lines and the storm legs: both ends, tension, breaking load, sag |
| `rig`, `storm` | The mooring rigs, for `moor_points` in `slab-moor.coax` to draw each line's shape |
| `work[0 ..< works]` | What each crew, form and crane is doing, where, and how far along |
| `wx` | Sea, wind, current and any stand-down |
| `event_last`, `event_next` | Indices into `run[0].ev`, the camera's cues in time order |

The frame is the site's: x east, z north, metres, the finished hull's
centre at the origin, y up from the still sea. The top of
`slab-build-view.coax` documents every kind and state.

The page's timelapse at `?view=build` still draws the earlier closed-form
schedule in `metaverse/slab-construction.coax`. It ends on the same hull.
Drawing this simulation in its place is phases 3 to 6 of issue #13682.

## Sources

Numbers 2 to 9 and 11 were checked by web search on 18 November 2026
against the publishers named, not against a primary document in hand.
Treat them as reported figures.

1. Femern A/S, "Fehmarnbelt tunnel" press fact sheet, January 2025:
   <https://femern.com/media/rz3of21s/2501_factsheet_press_fehmarnbelttunnel_jan25.pdf>.
2. Øresundsbro Konsortiet, the Øresund tunnel: 20 elements of 176 m and
   55,000 t, with Gina gaskets and Omega seals.
3. Equinor (Statoil), Troll A: 245,000 m³ of concrete, 656,000 t, towed
   from Vats by ten tugs in 1995.
4. Norwegian Contractors' Condeep practice, as reported for Gullfaks C and
   Troll A: slipforming at 1.5 to 4 m a day.
5. The Monaco floating breakwater (Port Hercule extension): a 352 m
   caisson built at Algeciras and towed to Monaco in August 2002.
6. Technological Research Association of Mega-Float, phase 2 at Yokosuka:
   an airport model about 1,000 m long made of six units joined at sea.
7. Shell, Prelude FLNG: 488 × 74 m, towed 5,800 km from Geoje in 2017.
8. Hibernia Management and Development Company: the gravity base towed
   from Bull Arm in 1997 by nine tugs.
9. US Army Corps of Engineers and the Port of Corpus Christi, Corpus
   Christi Ship Channel Improvement Project: 54 ft deep and 530 ft wide,
   56 ft in the entrance channel.
10. NOAA National Data Buoy Center, station 42020 (Corpus Christi):
    <https://www.ndbc.noaa.gov/station_page.php?station=42020>. The monthly
    wave climatology was not retrieved, so the monthly figures here are
    assumed.
11. NOAA National Hurricane Center, tropical cyclone climatology: return
    periods of 13 to 19 years for a hurricane passing within 50 nautical
    miles of the Texas Coastal Bend.

DNV-ST-N001 (marine operations and marine warranty) is the standard a real
tow and join would be planned to. Its values were not confirmed for this
note, so the weather limits, forecast factor and holding cases above are
assumed.
