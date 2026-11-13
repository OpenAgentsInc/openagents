# One world in cells

This page describes how Everglade's world is held as cells: what is built, how a room and a page
use it, and what is left. Issue:
[#13280](https://github.com/OpenAgentsInc/openagents/issues/13280) (AAA P2.7). Navigation, which
is built for each cell, is on [its own page](navigation.md).

## The model

- **A cell** is 64 m a side, keyed by whole numbers `(cx, cz)`. The engine's controller is
  `metaverse-engine/world/cells.coax`. Each step it gives every cell a level from its distance to the
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
- **Ground** is already terrain tiles (`metaverse-engine/world/terrain-tile.coax`,
  `everglade/world/ground.coax`): the glade's tile and the path's 65 m tiles share a lattice and
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

The page holds the same manifest and steps the same controller
(`everglade/client/cells.coax`), once a frame, on where you stand and where the eye is:

- **Rings.** A cell draws within 272 m of you, which is past the farthest a planted piece shows
  (240 m) with room for the eye behind you. You may stand where a cell draws. A cell is asked for
  at 336 m, a cell's width sooner. So the controller never cuts off something you can see: a
  piece's own dither does the fading, and a cell changing level changes no pixel.
- **Content.** A planting cell (16 m, `everglade/world/plant.coax`) whose middle is in a world
  cell is that cell's, and its pieces and the grass field's blocks draw only while the cell
  draws. Land off the manifest is the zone's backdrop: nobody walks it, and it draws by distance.
- **Loads.** The controller asks for a cell and something answers. The seam is three calls
  (`e2cl_asked`, `e2cl_key`, `e2cl_loaded`); an answer is admitted only for the cell's current
  request at the manifest's digest. The pack still comes whole, so the page answers its own asks
  in the same frame, as a room does. A reader that pages by cell answers through the same calls
  (#13304).
- **Where you stand.** Every place you may stand is on a cell of the manifest. The page counts
  the frames you stood in a cell that did not simulate; with the pack whole it is 0.
- **The origin.** One floating origin follows you by cell (`cell_rebase`). The page keeps it and
  checks it, but nothing on the GPU reads it yet: see "What is left".
- **Readout.** `coaxial_field_probe(180)` to `(193)`: the manifest's cells, resident, drawing,
  loading, frames held, loads that waited a round, rebases, your cell, the origin's cell, the
  most resident, frames off the manifest, loads asked, answers refused.

The glade and the forest path are neighbouring cells of this one world: one page, one room, one
set of terrain tiles, and no hand-over between them.

## Cells and the spatial index

The engine's `World` (`metaverse-engine/core/world.coax`, #13274) has one spatial index
(`metaverse-engine/core/spatial.coax`): a uniform grid over one zone, in the zone's own frame, queried by
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

- `metaverse-engine/world/cells.coax`: the controller, with a step for many players. `coax mutate
  --root-only` leaves no survivor.
- `everglade/world/zone.coax`: the manifest is one the controller takes; the glade and the path
  are neighbours; a player in the glade holds both with no load between them.
- `everglade/server/nav-checks.coax`: a room holds the cells round its players, and builds a
  cell's page on first need, kept only at the manifest's digest.
- `everglade/model/tools/zone-cells.coax --check`: the committed manifest is what the land builds,
  and a creature walks from the spawn to every named point and back.
- `everglade/client/cells.coax`: the page's controller. You walk the world's length and back with
  your cell always simulating and everything in reach drawing, inside the budget; a loader 3 s
  late at a gallop never holds you after your first cell; a wrong, old or failed answer is refused.
- `everglade/client/walk-test.coax`: you walk the path through the real controller, and a route
  walk for each place (the Glade, the Sunlit Path, the Deep Canopy, the Clearing, the Goblin Camp)
  with the cells stepped as the page steps them. Every place you may stand is on the manifest.

## What is left

These are not built. Each is tracked on #13280.

- **Paged packs by cell.** The pack (`.vpk3`, 309 pages) is still fetched whole at the start.
  The page's side is ready (the three calls above). It needs the pack's pages ordered by cell, a
  catalog line from a cell to its byte ranges, and the host's reader fetching those ranges when
  asked (#13304). Everglade's ground is one draw and its planting one record today, so the pack
  tool has to split both by cell first.
- **The origin on the GPU.** Positions are f32 from the world's middle. Everglade is 736 m long,
  where f32 keeps 0.06 mm, so nothing is wrong yet. The page keeps the origin; the shaders and the
  pack's vertices are still in world coordinates. The renderer's instance rows are where it goes
  in (#13303).
- **Holding you at an unloaded cell.** The page counts it and does not stop you. With the pack
  whole it never happens. When cells load over the network, the hold has to be one the room agrees
  with, or prediction and the room part.
- **Far terrain level of detail** beyond the stitched 1.3 m rows.
- **The legacy heightfield** (`metaverse-engine/world/terrain.coax`) still has 12 users.
- **`everglade-parity` and `everglade/features.coax`** stay until every zone is data.
- **Proof on a phone and in a visible Chrome.** A headless pass is not a claim that the walk is
  seamless.
