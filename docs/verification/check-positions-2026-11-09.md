# Positions in scripted checks (2026-11-09)

GitHub #13078. The room drift check failed for days (#13070) because its raid
step stood at the old spawn after the spawn moved to the commons
(e0631669b3). This page lists every place a scripted check stood at, aimed at
or shot from, and says where each one comes from now.

Every scripted check now reads spawn, gates and points of interest from the
zone's map data through one lookup, `aethyr/world/points.coax` (`map_point`).
A tool asks for every point it uses before it starts
(`coax/web/map-points.coax`, `map_need_all`). If a name is missing, the tool
stops with exit 2 and prints the name, the zone and the file to add it to:

```
map point "raid-front" is missing from Everglade (verse/everglade/points.coax)'s map data: name it in that file, never in the check
```

A `#test` asserts `.found` on the points it uses.

## The named points

| Zone | File | Points |
|---|---|---|
| Everglade | `verse/everglade/points.coax` (`eg_point`) | `spawn`, `gate-south`, `gate-north`, `gate-east`, `gate-west`, `guard-post`, `raid-front`, `raid-field`, `cult-camp`, `south-road`, `raid-muster`, `inn-door`, `bridge`, `commons`, `market`, `guilds`, `river`, `deep-wood` |
| Everglade II | `verse/everglade2/points.coax` (`e2_point`) | `spawn`, `guard-post`, `sunlit-path`, `woods`, `deep-canopy`, `tiles-path`, `ground-seam`, `goblin-camp`, `clearing` |
| Everglade Experimental | `verse/experimental-points.coax` (`exp_point`) | `spawn`, `dungeon-door` |

Each zone's spawn is defined in its points file and nowhere else:

- `EG_SPAWN_X` and `EG_SPAWN_Z` moved from `plan.coax`. `plan.coax` loads the
  points file.
- `E2_SPAWN_*` moved from `everglade2-move.coax`.
- `E2_CLEAR_Z` moved from `forest.coax`.
- `E5RD_SPAWN_X` and `E5RD_SPAWN_Z` in `everglade5e-room-raid.coax` were a
  second copy of the spawn. They now read `EG_SPAWN_X` and `EG_SPAWN_Z`.

The other points sit on numbers that the game's own data also holds. A test
next to that data keeps the two the same:

- `plan.coax` checks the spawn, the gates (`EG_GATE`), the guard post (between
  `EG_SPOT` 0 and 1), the bridge (`EG_BRIDGE`) and the places (`EG_PLACE` by
  `EG_PLACE_ID`).
- `everglade-raid-front.coax` checks `RF_GATE_Z`, `RF_HOLD_Z`, `RF_MID_Z` and
  `RF_CAMP_Z`.
- `everglade-doors.coax` checks `door_spot` 0.
- `camp.coax` checks the goblin camp.
- `room-e2-guard.coax` checks `E2G_Z0`.
- `experimental-points.coax` checks the dungeon's `DG_ENTRY` cell.

Everglade II's path points give `z` along the path, so a page's `?at=` is
`-z`. Their `x` is the distance across from the path's middle. The house and
workbench points are not in this change, because another agent is adding them.

## Inventory

The table uses these status values:

- **converted:** the check now reads the place from the map.
- **map already:** the check read the map before this change.
- **kept:** a fixture that is not a zone's place. The row gives the reason.
- **not a place:** a screen pixel, a relative move or a synthetic value.

Coordinates are metres, (x, z), as they stood on 11-09.

### Everglade, the drift check (`coax/web/everglade-drift.coax`, `coax/web/drift-placement.coax`)

| Where | What it held | Status |
|---|---|---|
| `drp_road` | the gravity rehearsal's open-road rectangle, x 0.5 to 2.5, z -58 to -59.5 | converted: 0.5 to 2.5 m across and 2 to 3.5 m out from `gate-south` (`drp_road_at`) |
| `dr_rg_nudge` | the rectangle's middle, x 0.8 to 2.2, z -58.3 to -59.2 | converted: from `gate-south` |
| `dr_gate_keys` | the gate's lane, x 0.8 to 2.2, and the palisade's approach, z -49 and -51 | converted: from `gate-south` |
| `dr_raid_walk` | the walk's end, z -58.3 | converted: 2.3 m past `gate-south` |
| `dr_raid_road` | the raid stand, z -56.5 to -79 | converted: 0.5 m past `gate-south` to 18 m past `raid-front` |
| `dr_rg_patrols` | 7 points of Bram's and Tilda's beats (`routines.coax` `FOLK_VIA`, `EG_SPOT` 0 and 1) | kept: these are character routes, not zone points, and the check holds them on purpose as an independent copy |
| unit tests of the functions above | literal positions | kept: they test the logic against `drp_test_map()`, the map as it stood on 11-09; the runtime uses `dr_map()` |
| `dr_script`, the raid walk's first holds, the forest step | timed key holds from where the page arrives | not a place |
| inputs fence | | the three points files are fenced inputs now |

### Everglade, room checks (`coax/web/room/`)

| File | What it held | Status |
|---|---|---|
| `raid-check.coax` | `RC_SQUARE` (0, 16), where the seats muster | converted: `raid-muster` |
| `raid-check.coax` | `RC_GATE` (0, -56), the gap the seats walk out through | converted: `gate-south` |
| `drift-bots.coax` | the bots' road, (0, -128) | converted: `south-road` |
| `drift-bots.coax` | test arguments (-15.53, -56.7) and the wire tests' rows | kept: fixtures that test parsing and a centre argument |
| `siege-sim.coax` | the idle seats "at the players' spawn", (±1, -84), in 2 places | converted: `spawn`; this was the old spawn on the road, so the sim now idles its seats at the real one |
| `door-check.coax` | the inn's door spot (-4.59, -37), inside it (-9.09, -37), the road (0, -44) | converted: `inn-door`, 4.5 m on along its face, and 12 m in from `gate-south` |
| `bridge-check.coax` | the way to the East Gate road: 8 waypoints | converted: each is a named point (`gate-south`, `commons`, `gate-west`) plus metres |
| `bridge-check.coax` | the deck x 84 to 92, the walks to (80, -4), (82, -11.5), (96, -11.5), (80, -11.5), the default aim (88, -11.5) | converted: `bridge` plus metres |
| `background-check.coax` | cast aims (0, -56) in 2 places, (0, -84) | converted: `gate-south`, and 28 m out past it |
| `load.coax` | the input stream's z sweep, -20 to 20 | not a place: synthetic input values |
| `room-hold-control.coax`, `room.coax`, `room-everglade.coax`, `work-budget.coax`, `room-observe.coax` | positions inside the room server's own `#test`s | kept: unit tests of a rule at a stated place, not scripted checks |

### Everglade II

| File | What it held | Status |
|---|---|---|
| `apps/openagents-web/everglade2-walk-test.coax` | the path walk's start, z -2 | converted: 2 m down the path from `spawn` |
| `apps/openagents-web/everglade2-walk-test.coax` | the path walk's end, `E2_CLEAR_Z` | converted: `clearing` (map already, now by name) |
| `apps/openagents-web/everglade2-walk-test.coax` | the glade's rocks (`cg_item` 20 and 21) | map already |
| `apps/openagents-web/everglade2-walk-test.coax` | a trunk by its cell (`e2_trunk(2, 3, -80, ...)`) | kept: a planted tree, from the planting function |
| `apps/openagents-web/everglade2-walk-test.coax` | the pace test's walk, (0, 0) to (0, -100) | not a place: an empty world |
| `coax/web/everglade2-cull-check.coax` | views `golden`, `golden&at=200`, `golden&at=470` | converted: `spawn`, `woods`, `clearing` |
| `coax/web/everglade2-terrain-check.coax` | walks `walk`, `at=120&walk`, `at=360&walk` | converted: `spawn`, `tiles-path`, `ground-seam` |

### Lighting baseline (`coax/web/lighting-baseline.coax`)

| What it held | Status |
|---|---|
| 8 Everglade II loads at `?at=1`, `?at=60`, `?at=300` | converted: `spawn` plus 1 m, `sunlit-path`, `deep-canopy` (`lb_path`; a test holds the 13 addresses to the baseline's) |
| the haze probes' eye, `vec3f(0.0, 1.6, 4.0)`, in the Character Create glade's local frame | kept: a shader argument whose results are in the recorded baseline, not a standing spot |
| pixel probes and plan clicks | not a place: screen coordinates |

### Flyover (`coax/web/flyover.coax`, `coax/web/flyover/everglade.txt`)

| What it held | Status |
|---|---|
| 10 keys, 20 points in world numbers | converted: each point is `@NAME` and metres from it (`gate-south`, `commons`, `market`, `river`, `guilds`, `deep-wood`). A test holds the keys to the frames they flew before. |

`flyover.coax` now accepts `@NAME` before a point. A name that the realm's map
lacks refuses the path.

### Goldens (`coax/web/golden-check.sh`, `goldens.ts`, `golden-pages.ts`, `golden-frames.*`)

| What it held | Status |
|---|---|
| camera spots | none: each page is shot where it puts you, which is its own spawn from the map, or a named hash |
| `fixture-painterly-shafts`: `sun=156&sunup=9.5` | not a place: a sun angle |

### Other

| File | What it held | Status |
|---|---|---|
| `coax/web/perf.ts` | `#market` | map already: an `EG_PLACE_ID` |
| `coax/web/everglade-benchmark.coax` | `#gate` | map already |
| `coax/web/everglade-arrival-session.coax`, `coax/web/tests/character-walk.coax` | clicks and timed keys | not a place |

## Count

- **converted:** 67 positions. The drift check has 5, drift-bots 1,
  raid-check 2, siege-sim 2, door-check 3, bridge-check 14, background-check 3,
  the walk test 2, the cull check 3, the terrain check 3, the lighting baseline
  8, the flyover 20 and the game's second spawn 1.
- **map already:** 4. These are the two glade rocks, `#market` and `#gate`.
- **kept:** 10. These are the 7 patrol points, the trunk, the probe eye and the
  drift-bots centre argument. The unit-test fixtures listed above are kept as
  well.
- **not a place:** the screen points, timed moves and synthetic values listed
  above.
