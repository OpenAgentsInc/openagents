# Housewright: gathering materials and building houses (spec)

Status: specification, 2026-11-09. Nothing here is built yet, except the
pieces marked **exists today**. Owner's ask: "a housebuilding talent tree /
profession … where we can let players harvest materials and build basic
houses … what would be logical to start with."

This spec defines:

- a building profession, **Housewright**, and its talent tree;
- the materials players gather, the primitive tools they make, and what
  each tool unlocks;
- five house tiers, from a branch lean-to to a stone cottage;
- how building works in a shared world: plots, stages, help and repair;
- what to build first, and the phases after it.

Housewright is Everglade's own system. The SRD 5.2.1 has no gathering,
professions or house building, so every record this spec adds is marked
`srd: false`, like the house rules in
[`aethyr/rules/srd/house.coax`](../../aethyr/rules/srd/house.coax). Where
the SRD has a rule, it applies as written: tool proficiency, ability checks,
the Utilize and Help actions, crafting time, and the object AC table.

## Contents

1. [What to start with](#1-what-to-start-with)
2. [The idea](#2-the-idea)
3. [What the SRD gives us](#3-what-the-srd-gives-us)
4. [Materials and gathering](#4-materials-and-gathering)
5. [Primitive tools](#5-primitive-tools)
6. [House tiers](#6-house-tiers)
7. [What a house does](#7-what-a-house-does)
8. [Building in a shared world](#8-building-in-a-shared-world)
9. [The Housewright talent tree](#9-the-housewright-talent-tree)
10. [Engine, game and zone](#10-engine-game-and-zone)
11. [Phases](#11-phases)
12. [Open questions for the owner](#12-open-questions-for-the-owner)

## 1. What to start with

Start with the first two tiers: the **branch lean-to** and the **woven
hut**. Build them in Everglade II's city clearing, beside its brook.

| Piece | First release |
| --- | --- |
| Materials | Branches, withies (flexible hazel or willow rods), reeds, fieldstones |
| Tools | Bare hands, then a knapped stone blade, then a stone axe |
| Structures | Branch lean-to (bare hands), woven hut (stone blade and axe) |
| Where | Plots in Everglade II's city clearing (#13093), with reeds at its brook |
| Talents | The first rank of the tree: Gathering, Knapping, Withy Weaving, Reed Thatch |

Why start there:

- **No economy needed.** No metal, no shops and no prices. Every material
  is picked up in the world with bare hands or a stone tool.
- **It uses what Everglade II already has.** A forest for branches and
  poles, a meadow, the clearing for plots, and the brook for reeds.
- **It proves the whole loop.** Gather, make a tool, gather what the tool
  unlocks, build in stages, and see a house stand in a shared world. Every
  later tier is more of the same loop with heavier materials.
- **It's honest to the fantasy.** Someone dropped into the woods builds a
  shelter first, then a hut, then a cottage. The first hour of play matches
  that.

The wattle-and-daub cottage, which needs clay from the brook, is the second
release (section 11).

## 2. The idea

A person alone in a medieval forest builds in the order their tools allow:

```
bare hands       stone blade        stone axe and        Carpenter's        Mason's
                                    digging stick        Tools              Tools
    │                 │                   │                   │                 │
    ▼                 ▼                   ▼                   ▼                 ▼
branch lean-to → woven hut → wattle-and-daub cottage → timber house → stone cottage
```

Each step needs new tools, new materials and new skill. The house itself
is the reward: a place to rest, store things, respawn and, later, run the
Binding Table ([Agentics](agentics-profession-spec.md)).

Three concerns that real builders struggled with become the measures of a
good house:

- **Drainage.** Build on high ground and dig a ditch, or the floor stays damp.
- **Ventilation.** Smoke and moisture must get out.
- **Fire safety.** The hearth sits on stone, away from thatch and timber.

## 3. What the SRD gives us

| Rule | SRD 5.2.1 | Use here |
| --- | --- | --- |
| Tools and tool proficiency | Equipment, "Tools", p. 93 | Add your Proficiency Bonus to a check with the tool. With a skill proficiency in the same check, you also have Advantage. |
| Artisan's Tools | Equipment, pp. 93–94 | Carpenter's, Mason's, Potter's, Weaver's and Woodcarver's Tools gate the later tiers. Confirm each kit's ability and price against the SRD when it's built. |
| Utilize action | Playing the Game, "Actions", p. 10 | Gathering and each building stage is a Utilize action. |
| Help action | Playing the Game, "Actions", p. 10 | Another player helps a build stage. |
| Crafting Nonmagical Items | Equipment, p. 103 | Work comes in 8-hour days that needn't be consecutive, and one helper with the same proficiency divides the time. A house's stages use this rule. |
| Objects: AC and Hit Points | Gameplay Toolbox, "Objects" | Wood AC 15, stone AC 17, iron AC 19. A wall's AC comes from its material. |
| Skilled origin feat | Feats, p. 87 | An SRD route to a building tool proficiency at level 1. |

**Exists today.**

- Tool proficiency and checks: [`check-training.coax`](../../aethyr/rules/srd/check-training.coax) (37 SRD tools as bits) and [`ability-checks.coax`](../../aethyr/rules/srd/ability-checks.coax).
- Help: [`help.coax`](../../aethyr/rules/srd/help.coax).
- A placed house with walls, a doorway and monster pathing: [`aethyr/world/building.coax`](../../aethyr/world/building.coax) and [`everglade/world/home.coax`](../world/home.coax) (`78d99e4c24`).
- Spots you stand at to use something: [`aethyr/world/interact.coax`](../../aethyr/world/interact.coax).
- Long rest: [docs/dnd/long-rest-implementation.md](../../docs/dnd/long-rest-implementation.md).

Equipment crafting is **missing**. The compliance ledger lists
CONTENT-EQUIPMENT as partial.

### Time

A build stage's length is stated in SRD workdays, then converted with the
same world constant the Agentics spec uses
([section 4.4](agentics-profession-spec.md#44-time-the-workday-in-real-time)):

```text
WORKDAY_S = 1200      one SRD workday (8 hours of work) = 20 real minutes
```

The constant moves into a shared crafting module so both professions use
one number. A strict world sets it to 28,800.

## 4. Materials and gathering

| Material | Where in Everglade II | Gathered with | Used for |
| --- | --- | --- | --- |
| Branches | Fallen wood and low boughs in the forest | Bare hands | Lean-to frame and cover, kindling |
| Leafy boughs | Forest edge | Bare hands | Lean-to cover |
| Withies | Hazel and willow thickets at the brook and the clearing's edge | Bare hands; faster with a blade | Woven walls (wattle) |
| Reeds | The brook's banks | Stone blade | Thatch, bedding |
| Fieldstones | The ground near the brook and the forest floor | Bare hands | Foundations, the hearth ring, ditch edging |
| Poles | Young trees | Stone axe | Hut and cottage posts, roof frames |
| Logs | Mature trees | Stone axe (slow), iron axe | Timber frames, beams |
| Clay | The brook's banks | Digging stick | Daub, plaster, floors |
| Earth | Anywhere | Digging stick | Daub, cob, floors, ditches |
| Straw | Meadow grass, cut and dried | Stone blade | Daub reinforcement, thatch, cob |
| Dressed stone | Outcrops and a quarry | Mason's Tools | Stone walls |

### How gathering works

- **A source is a thing in the world with a stable id**, such as
  `thicket-7` or `tree-112`. You walk to it and use it, like the workbench.
  Using it is a Utilize action.
- **The yield takes a check.** Use the ability that fits the work, with the
  matching tool's proficiency:
  - Strength for felling and digging;
  - Dexterity for cutting withies and reeds;
  - Wisdom (Survival) for finding good stone and clay.

  A failure still yields something, just less. Gathering never comes up
  empty. That's a house rule.
- **Sources regrow.** A felled tree leaves a stump that becomes a sapling,
  then a tree again. Reed beds and thickets regrow in a few real hours. The
  room keeps the clock, so regrowth continues while nobody is there.
- **Sources are shared.** Everyone sees the same trees fall and the same
  stumps. A source's yield is per player for a while after someone gathers
  it, so nobody can strip a thicket from someone who just walked up.
- **Materials go in a materials bag.** It holds counts by material, with
  weight from the SRD's carrying rules. Heavy things such as logs are
  dragged one at a time, at reduced speed.

## 5. Primitive tools

Primitive tools are Everglade's own items (`srd: false`). They stand in for
the SRD's artisan's tools at the low tiers, and they wear out.

| Tool | Made from | Made with | Unlocks |
| --- | --- | --- | --- |
| Knapped stone blade | 2 fieldstones (flint where you find it) | Bare hands, a Dexterity check | Cutting reeds, straw and withies faster |
| Cordage | Plant fibre or withies | Stone blade | Lashing posts and frames |
| Stone axe | A shaped stone, a pole and cordage | Stone blade | Cutting poles, slowly felling logs |
| Digging stick | A pole, fire-hardened | Stone blade and a fire | Clay, earth and ditches |
| Wooden mallet | A log section and a pole | Stone axe | Driving posts and ramming floors |

Metal tools come from the SRD: an iron axe, Carpenter's Tools and Mason's
Tools, bought or made by a smith.

## 6. House tiers

Each tier is built in **stages**. A stage needs its materials in your bag,
its tools, and its workdays. A house shows each stage as it's built, so a
half-built hut looks half-built.

| Tier | Structure | Footprint | Tools | Materials | Work | What it gives |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | Branch lean-to | 2 × 3 m | Bare hands | 20 branches, 30 leafy boughs | ¼ day (5 min) | Partial cover; a place to rest |
| 1 | Woven hut | 3 m round | Stone blade and axe | 8 poles, 60 withies, 40 reed bundles, 12 fieldstones | 1 day (20 min) | Out of the rain; a stone fire ring outside; a door flap |
| 2 | Wattle-and-daub cottage | 3.7 × 4.9 m | Stone axe, digging stick, mallet | 40 fieldstones, 6 posts, 120 withies, 30 clay, 20 straw, 20 earth, 120 reed bundles | 3 days (1 h) | Weatherproof and warm; hearth on stone inside; a door; storage |
| 3 | Timber house | 4 × 4 m (the current home) | Carpenter's Tools | 24 timbers, 40 planks, a stone sill, roof thatch or shingles | 5 days (1 h 40 min) | A lockable door; a workbench; room for furniture |
| 4 | Stone cottage | 5 × 6 m | Mason's Tools | 300 dressed stones, lime mortar, roof timbers | 10 days (3 h 20 min) | The best defence and fire safety; a chimney |

Material counts are starting values for playtests, not final numbers.

Each tier's stages follow the same logic: mark and prepare the site, lift
the structure off the wet ground, raise the frame that carries the load,
close the walls, cover the roof, then fit out the inside. A lighter tier
skips the steps it doesn't need. Everglade Experimental shows every tier
frozen at every stage in its housewright showcase
([docs/verse/everglade-experimental.md](../../docs/verse/everglade-experimental.md)).

### The stages of the branch lean-to

1. **Uprights and ridge pole.** Push two forked branches into the ground
   and lay a ridge pole in their forks.
2. **Branches laid.** Lean branches from the ridge pole to the ground
   behind, side by side.
3. **Covered with boughs.** Lay leafy boughs over the branches in two
   layers, the upper over the lower, and lay a bed of boughs underneath.
   A ring of fieldstones beside it holds a fire.

### The stages of the woven hut

1. **Posts set.** Mark a 3 m ring and set poles round it about a metre
   apart, leaving a doorway. Lay a stone fire ring outside.
2. **Walls woven.** Weave withies in and out between the posts, each row
   the other way round, up to head height.
3. **Roof poles lashed.** Lash poles from the wall top to a point above
   the middle.
4. **Thatched.** Lay reed bundles round the roof poles in overlapping
   courses, from the eaves to a knot at the top, and hang a hide flap over
   the doorway.

### The stages of the wattle-and-daub cottage

1. **Site.** Pick high ground in your plot and dig a drainage ditch
   (digging stick).
2. **Foundation.** Stack fieldstones into a raised base, so the frame stays
   off wet ground.
3. **Frame.** Set posts on the foundation and lash or peg a steep roof frame.
4. **Wattle.** Weave withies between the posts.
5. **Daub.** Mix clay, earth, straw and water and press it into both sides
   of the wattle.
6. **Thatch.** Lay reed bundles on the roof frame, overlapping, with a ridge.
7. **Hearth and floor.** Lay a stone hearth away from the walls, and
   compact an earth floor.

### The stages of the timber house

1. **Stone sill laid.** Lay a low wall of fieldstones where the walls will
   stand, so no timber touches the ground.
2. **Frame raised and pegged.** Lay sole plates on the sill, stand posts on
   them, and join posts, wall plates and corner braces with oak pegs.
3. **Roof framed.** Set sawn rafters from the wall plates to a ridge board.
4. **Walls boarded.** Fill the frame with boarded walls, a door frame and
   windows, and lay a plank floor.
5. **Roofed and fitted.** Cover the roof, close its gables, and hang the
   lockable door and the glazing.

### The stages of the stone cottage

1. **Footings laid.** Lay a wide course of fieldstones under the line of
   every wall.
2. **Walls to the sills.** Build the dressed stone walls up to the window
   sills, leaving the doorway open.
3. **Walls and gables raised.** Build up to the eaves round the door and
   the windows, then the two gable ends.
4. **Roof timbers and chimney.** Set the rafters and the ridge, and build a
   stone chimney against the gable by the hearth.
5. **Thatched and the door hung.** Thatch the roof, lay a flagstone floor
   and hearth, and hang the door.

### Biomes

The right materials depend on the land. This spec covers Everglade's
temperate forest, where timber and clay come first. Later realms change the
tiers:

- **Rocky hills:** stone and earth earlier.
- **Treeless plains:** cob or adobe in place of wattle and timber.

## 7. What a house does

A house has five measures, worked out from how it was built:

| Measure | Raised by | Lowered by |
| --- | --- | --- |
| Weather | A steep, overlapped roof; daubed walls | Gaps; a worn roof |
| Warmth | Daub, thatch and a hearth | An open hut; a cold hearth |
| Damp | — | Low ground with no ditch; an earth floor with no foundation |
| Fire risk | — | A hearth near thatch or timber; no smoke hole |
| Defence | Wall material (SRD object AC: wood 15, stone 17); a lockable door | Wattle and thatch |

What the measures do, without adding warnings to the screen:

- **Rest.** A house with good Weather and Warmth lets you take a Long Rest in
  bad weather. A lean-to only shelters you from light weather.
- **Respawn.** Your house is your respawn point.
- **Storage.** A storage chest keeps materials and items, from tier 2 up.
- **The workbench.** From tier 3, the workbench is the Binding Table
  ([Agentics](agentics-profession-spec.md)).
- **Fire.** A house with high fire risk can catch from its own hearth. Fire
  damages the roof first, and you can put it out and repair it.
- **Wear.** Thatch and daub wear over weeks of real time. Repairs take a few
  materials and a fraction of a stage's time.

## 8. Building in a shared world

- **Plots.** Everglade II's city clearing is divided into 16 plots in a 4
  by 4 grid. Each plot is 10 m square, on a 12 m pitch, so a 2 m lane runs
  between each pair of plots, and the whole grid is 46 m across. A 10 m
  plot holds any tier, the stone cottage included, with room round it. The
  grid sits in the middle of the clearing's open meadow, its corners within
  34 m of the centre, clear of the edge, the path on the west and the brook
  on the east. Plots are numbered 1 to 16, row by row from the north-west
  corner. You claim one plot to start, and each plot holds one house at
  first. Only you and the people you invite can build on it. Monsters don't
  spawn inside claimed plots. The engine keeps the plots and their owners
  (`aethyr/world/plots.coax`); the zone says where the grid is.
- **Placing.** You choose a tier and see its outline on the ground, snapped
  to the plot and turned in quarter steps. The outline turns red over a
  stump, a rock, water or a slope too steep for the foundation.
- **Building.** Stand at the build spot and use it. Each stage spends its
  materials and starts its workdays. The stage builds while you're nearby or
  away, because the room keeps the clock.
- **Help.** Another player who uses the Help action at your build spot, and
  has the same tool proficiency, halves the stage's time, as the SRD's
  crafting rule allows one assistant to.
- **Server authority.** The room decides every gather, every stage and every
  plot. The page only shows them.
- **Walls are real.** A finished wall blocks movement and sight, uses the
  engine's colliders, and lets monsters in only through the doorway, as the
  current home does.

## 9. The Housewright talent tree

You become a Housewright by building. Each finished stage gives profession
experience, and each new **rank** gives one talent point. Ranks follow the
tiers:

| Rank | Reached by | Unlocks |
| --- | --- | --- |
| Gatherer | Building a lean-to | Tier 0 and the knapped blade |
| Hut-builder | Building a woven hut | Tier 1, the stone axe and cordage |
| Cottager | Building a wattle-and-daub cottage | Tier 2, the digging stick and mallet |
| Wright | Building a timber house; Carpenter's Tools proficiency | Tier 3 |
| Master Housewright | Building a stone cottage; Mason's Tools proficiency | Tier 4 |

Talent points go into four branches. A talent needs its rank, and some need
an earlier talent in the same branch.

### Timber

| Talent | Rank | Effect |
| --- | --- | --- |
| Gathering | Gatherer | More branches and boughs from each source |
| Felling | Hut-builder | Fell trees with a stone axe in half the time |
| Pegged Joints | Cottager | Frames need no cordage; posts last longer |
| Cruck Frame | Cottager | A curved-timber frame: a taller cottage roof with fewer posts |
| Doorwright | Wright | A hinged, lockable door on any tier from 2 up |
| Shingles | Wright | A wooden shingle roof: less fire risk than thatch |

### Earth and clay

| Talent | Rank | Effect |
| --- | --- | --- |
| Withy Weaving | Gatherer | Weave wattle panels faster and with fewer withies |
| Clay Finder | Hut-builder | Finding a clay bed at the brook always succeeds |
| Good Daub | Cottager | Daub with the right straw and clay mix: higher Weather and Warmth |
| Rammed Floor | Cottager | A compacted clay floor: no Damp from the floor |
| Cob Walls | Wright | Walls of clay, straw and earth without wattle, for treeless land |
| Lime Plaster | Master Housewright | Plastered walls: the best Weather, and they wear slowly |

### Stone

| Talent | Rank | Effect |
| --- | --- | --- |
| Knapping | Gatherer | Make a stone blade that lasts twice as long |
| Dry Stacking | Hut-builder | Stack fieldstones without mortar: hut and cottage foundations |
| Hearthstone | Cottager | A stone hearth inside: Warmth with no added fire risk |
| Raised Foundation | Cottager | A higher base: no Damp from the ground |
| Chimney | Wright | A stone chimney: removes the smoke hole's heat loss |
| Dressed Stone | Master Housewright | Cut stone walls with Mason's Tools |

### Roofing and site

| Talent | Rank | Effect |
| --- | --- | --- |
| Reed Thatch | Gatherer | Bundle reeds into thatch; lean-to and hut roofs |
| Steep Pitch | Hut-builder | Steeper roofs shed rain: higher Weather |
| Drainage | Hut-builder | Dig a ditch around any house: no Damp from water |
| Ridge Cap | Cottager | A capped ridge: thatch wears half as fast |
| Smoke Hole | Cottager | Ventilation under the ridge: lower fire risk inside |
| Firebreak | Wright | Cleared ground and a stone sill: fire can't spread between houses |

There are 24 talents and 5 ranks. A full Master Housewright has most of
the tree, but not all of it, so two Housewrights still build differently.

## 10. Engine, game and zone

Per the owner's rule against one-off zone code:

| Layer | Pieces |
| --- | --- |
| Engine (`aethyr/`) | Gather sources with stable ids, yields, regrowth and per-player cooldowns (`aethyr/world/harvest.coax`). Staged construction with outlines, stages, timers, Help and wear (`aethyr/world/build-site.coax`). Plots and ownership (on [`owners.coax`](../../aethyr/world/owners.coax)). The materials bag. Procedural geometry for branches, wattle and thatch. The existing `building.coax` and `interact.coax`. |
| Game (Everglade) | The materials, tools, tiers, stages, measures and talent tree (`everglade/world/housewright.coax`); the shared workday constant; SRD tool proficiencies and checks. |
| Zone (Everglade II) | Where the plots are, which trees, thickets, reed beds and stones are sources, and where the clay beds are at the brook. Nothing else. |

Wattle, thatch and leafy boughs aren't in the Medieval Village MegaKit. They
should be generated in Coax as procedural meshes, so a half-woven wall can
show exactly the withies woven so far.

## 11. Phases

Each phase lands on `main` with its checks, on the remote build hosts.

1. **The first two tiers.**
   - Branches, boughs, withies, reeds and fieldstones as sources, with regrowth.
   - The materials bag, the stone blade and the stone axe.
   - Plots in the city clearing.
   - The lean-to and the woven hut, built in stages.
   - The Gatherer and Hut-builder ranks and their talents.
   - Tests: a gather-to-hut walk on the room, wall colliders, monster pathing.
2. **The wattle-and-daub cottage.** Clay and earth at the brook, the digging
   stick and mallet, the seven stages, the five measures, rest and respawn at
   home, storage, and the Cottager talents.
3. **The timber house.** Carpenter's Tools from the SRD, logs and planks, the
   existing home as the tier 3 house, and the Wright talents.
4. **The stone cottage.** Mason's Tools, a quarry, lime mortar, the chimney,
   and the Master Housewright talents.
5. **Wear, fire and repair.**

## 12. Open questions for the owner

1. **Name.** Housewright is a real medieval trade name. Other options are
   Hearthwright and plain Builder.
2. **The workday.** Is 20 real minutes per SRD workday right for houses? A
   cottage then takes an hour, and a stone cottage three hours and twenty
   minutes.
3. **Plots.** Decided on 2026-11-09: 16 plots in a 4 by 4 grid, each 10 m
   square on a 12 m pitch, and one house per player at first (section 8).
   The count reads well from the town shot's high camera and leaves room to
   add rings of plots later.
4. **Gathering.** Are sources shared, as written here, or instanced per
   player?
5. **Fire.** Can other players set fire to your house, or only your own
   hearth?
