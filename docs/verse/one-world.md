# One world in cells

This page describes how Everglade's world is held as cells: what is built, how a room and a page
use it, and what is left. Issue:
[#13280](https://github.com/OpenAgentsInc/openagents/issues/13280) (AAA P2.7). Navigation, which
is built for each cell, is on [its own page](navigation.md).

## The model

- **A cell** is 64 m a side, keyed by whole numbers `(cx, cz)`. The engine's controller is
  `aethyr/world/cells.coax`. Each step it gives every cell a level from its distance to the
  nearest player: absent, loading, resident (within 256 m), simulate (160 m) or draw (96 m), with
  8 m of hysteresis. It asks for loads nearest first, inside a budget of resident cells and loads
  in flight.
- **A zone is a manifest**: the cells with ground, each with the digest of its content.
  Everglade's is `everglade/world/zone-cells.coax`, 21 cells from the glade to the far clearing,
  loaded by `everglade/world/zone.coax`. The glade and the forest path are neighbouring cells of
  that one manifest. A new stretch of the world is more rows, not more code.
- **A cell's content** has a digest. A load is admitted only when its digest is the manifest's
  (`cell_complete`). Today the digest is the navigation page's, and a room keeps a page only at
  that digest.
- **Ground** is already terrain tiles (`aethyr/world/terrain-tile.coax`,
  `verse/everglade2/ground.coax`): the glade's tile and the path's 65 m tiles share a lattice and
  their edges, with a stitched seam where a fine row meets a coarse one.

## The room

The room is the authority (`everglade/server/nav.coax`):

- Each tick it steps the controller on every placed seat (`cell_step_from`). A cell's distance is
  to the nearest seat, so two players far apart each hold their own cells, and an empty room holds
  none.
- A cell has nothing to fetch in a room, so its load is answered at once. Its navigation page is
  built when a creature first needs it, and a host builds ahead in spare ticks: a slice each
  tick, about 1 ms, and under a second a cell.
- A monster steps only while its cell simulates. A monster whose cell goes to sleep while it is
  out after a player is put home, whole.
- Every room of a server holds the same land, so the server keeps one store of pages and one cell
  world. A room keeps its cells' levels and requests (42 numbers for Everglade) and its own
  obstacles.

## The page

The page doesn't hold the manifest yet. It loads the whole pack at the start and draws one world:
the glade and the path are already one page, one room and one set of terrain tiles, with no
hand-over between them. See "What is left" for the page's half of the cell model.

## Cells and the spatial index

The engine's `World` (`aethyr/core/world.coax`, #13274) has one spatial index
(`aethyr/core/spatial.coax`): a uniform grid over one zone, in the zone's own frame, queried by
radius. Cells sit above it:

- A cell is the paging unit. The index is the query structure. World-cell coordinates never go
  into `SpatialGrid`.
- A cell that starts to simulate spawns its creatures into the zone's `World` (`world_spawn`), and
  one that stops despawns them. Today the room keeps its monsters in its own arrays and asks
  `e2nv_awake` before a step. When the room core holds entities in the `World` (#13275, #13276),
  that test becomes the spawn and despawn at the same two moments.
- Static colliders are not `World` entities and are not in the index. Navigation pages are built
  from them for each cell.
- An obstacle that must be found by position (a door, a placed piece) can be a `World` entity with
  a transform and a kind, found with `world_near`. The navigation cells hold its shape by its ID.
- Rebasing the origin by cell is a shift of every slot's `WF_X` and `WF_Z` followed by a settle.
  `cell_rebase` says when.

## Checks

- `aethyr/world/cells.coax`: the controller, with a step for many players. `coax mutate
  --root-only` leaves no survivor.
- `everglade/world/zone.coax`: the manifest is one the controller takes; the glade and the path
  are neighbours; a player in the glade holds both with no load between them.
- `everglade/server/nav-checks.coax`: a room holds the cells round its players, and builds a
  cell's page on first need, kept only at the manifest's digest.
- `everglade/model/tools/zone-cells.coax --check`: the committed manifest is what the land builds,
  and a creature walks from the spawn to every named point and back.
- `everglade/client/walk-test.coax`: you walk the path through the real controller.

## What is left

These are not built. Each is tracked on #13280.

- **The manifest on the page.** The page steps no cell controller yet. It is the same manifest
  and the same `cell_step_rings` call on your position, and it is what the next item needs first.
- **Paged packs by cell on the page.** The pack (`.vpk3`, 309 pages) is fetched whole at the
  start. Fetching a cell's pages when the controller asks needs the pack's pages ordered by cell
  and the host's pack reader to hand them over one cell at a time (#13304).
- **Origin rebase on the page.** Positions are f32 from the world's middle. Everglade is 736 m
  long, where f32 keeps 0.06 mm, so nothing is wrong yet. `cell_rebase` is ready for a larger
  world.
- **Far terrain level of detail** beyond the stitched 1.3 m rows.
- **The legacy heightfield** (`aethyr/world/terrain.coax`) still has 12 users.
- **`everglade-parity` and `everglade/features.coax`** stay until every zone is data.
- **Proof on a phone and in a visible Chrome.** A headless pass is not a claim that the walk is
  seamless.
