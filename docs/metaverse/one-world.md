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
  request at the manifest's digest. In the browser the pack is paged by cell and the host answers
  as each cell's bytes arrive: see "Paged by cell". A host that holds the pack whole (the desktop
  and phone apps) has the page answer its own asks in the same frame, as a room does.
- **Where you stand.** Every place you may stand is on a cell of the manifest. A step toward a
  cell whose content is not in yet is not taken: the page drops the push from its own input when
  the place 2 m on is in such a cell (`cell-view.coax` `e2cv_holds`, called by `e2_move_in`). The
  hold is on the input, before it is predicted and before it is sent, so the room steps the same
  input and keeps you in the same place; no correction comes of it. You may still turn, and walk
  any way that stays in the cells that are in. The page also counts the frames you stood in a cell
  that did not simulate; with the pack whole both are 0.
- **The origin.** One floating origin follows you by cell (`cell_rebase`). The page keeps it and
  checks it. The GPU does not need it: see "The ground by cell".
- **Readout.** `coaxial_field_probe(180)` to `(193)`: the manifest's cells, resident, drawing,
  loading, frames held, loads that waited a round, rebases, your cell, the origin's cell, the
  most resident, frames off the manifest, loads asked, answers refused.

The glade and the forest path are neighbouring cells of this one world: one page, one room, one
set of terrain tiles, and no hand-over between them.

## Paged by cell

Everglade II's pack in the browser carries the world by cell, and the page fetches a cell when
its controller asks for it.

- **The pack.** `apps/openagents-web/build.sh` runs `everglade/model/tools/cell-pack.coax` on the
  tracked pack (after `rig-pack --into`, before `page-pack`). It moves this tree's planting and
  the ground's record out of the catalog into one more entry, kind 8 (`PACK_CELLS`, "cells"),
  which `page-pack` never makes a root. The entry is a run of blobs
  (`everglade/world/cell-content.coax`): for each cell of the manifest, its own ground, then its
  planting. The catalog gets a `W` line (the entry's licence) and a `Z` line a cell
  (`metaverse-engine/host/pack-cells.coax`): the cell's digest and its run. Today the entry is
  3.3 MB: the ground 1.0 MB in 14 cells' runs, and 68,604 pieces in 21 cells, the largest run
  0.35 MB.
- **Who carries what.** A planting cell (16 m) whose middle is in a world cell is that cell's. A
  backdrop planting cell rides with the manifest's nearest cell when that cell draws from
  everywhere the backdrop's pieces can show (`everglade/world/cell-pack.coax`, checked every 8 m
  of the manifest). The rest stay in the catalog's `P` record and come with the catalog: 28 of
  the 1,242 planting cells, 1,252 pieces.
- **The host.** A page that exports `coaxial_cell_asks_v1` gets its `.vpk3` with
  `loadPaged(pack, {demand: true})`: the roots at the start, in a worker that stays
  (`coax/web/pack-fetch.js`, `pack-worker.js`). Once a frame `host.js` (`pumpCells`) reads the
  cells the controller waits for, asks the worker for each one's ranges, copies them into the
  page and calls `coaxial_cell_loaded_v1`. The sounds' pages follow the first image the same
  way. A cell with no line, or a fetch that fails, is a failed load after half a second, and the
  controller asks again.
- **The page.** `e2_cell_checks` takes a cell's bytes only when they are whole blobs with that
  cell's planting, its first planting cell is this tree's planting bit for bit, and it fits.
  Only then is the controller answered with the line's digest, and only when the controller
  admits the answer is the content taken (`e2_cell_take`): the cell's pieces go into the
  planting where its planting cells' empty ranges are, so every reader of the planting sees a
  cell that is not in as empty. A cell brings its own ground tiles (`e2g_record_take`, see "The
  ground by cell"). A cell whose level falls back to absent gives its pieces and its tiles back
  (`e2_cells_settle`), and is fetched again if you return.
- **The start.** The scene draws nothing until every cell within the draw ring of you is in and
  the ground has settled (`e2_world_ready`), so the woods never arrive in a picture. The host's
  "scene ready" waits for the first draw, as it always has.
- **The glade's baked shade** is baked again when a cell with a tree near the glade's tile
  arrives or leaves, once no load is on its way.
- **A pack that is not by cell** (no `Z` line: the tracked pack the apps bundle, or an older
  build's) is read as before: the woods from `P` lines or planted at start, the ground from `G`
  lines or converted, every cell answered at once. A pack whose planting is another tree's is
  refused cell by cell, and the catalog's own part sends the page back to planting at start.
- **Readout.** `coaxial_field_probe(194)` to `(198)`: paged by cell, cells taken, cells let go,
  the scene may draw, the ground record's status. `coaxial.cellLog()` lists each answer (cell,
  key, milliseconds, bytes, what the page said). In development `?celldelay=MS` holds each
  answer back, and `?cellfree=N` lets the first N through.

What this does not do yet: the pack's meshes and textures still come with the roots. A step toward a cell that is
not in is held (above), but a trunk in a neighbouring cell that is not in yet is not there to
stop you, so on a very slow connection the room may correct a step the page predicted.

## The ground by cell

The ground's tiles come with their cells and draw at a level by distance (#13509).

- **The pack.** The tracked pack holds no tile mesh, only the coarse 4 m ground under the tiles.
  The catalog's `G` lines are the tiles' record, version 2: each live sample's height and
  weights, and its drawn normal (the pack tool's own central difference,
  `everglade/world/ground.coax` `e2g_normal_code`), under a digest. `cell-pack.coax` splits the
  record by cell: a tile goes with the manifest cell its middle is in, or the nearest one
  (`e2cp_tile_carrier`), as that cell's `E2CG` blob, a record of its tiles only. The far tiles'
  coarsest level (26 by 26 samples each) goes into the catalog as `F` lines, so a tile whose cell
  is not in still draws from afar. `everglade/model/tools/ground-cut.coax` took the tiles' mesh
  out of the tracked pack: 79,855 vertices and 463,266 indices, 4.4 MB of the roots.
- **The page.** `e2g_record_take` takes a cell's tiles into the ground set, `e2g_drop_cell` drops
  them when the cell leaves, and `e2g_gpu` writes each tile to a slot of the ground store
  (store 4): its samples' heights and normals, and its cells' solidity at each level. A tile that
  is not in keeps its far slot.
- **The draw.** `e2_ground_draws` (`everglade/client/client.coax`) draws each slot as its own
  non-indexed draw after the environment's, in the scene and shadow passes. The vertex stage
  (`everglade/client/gpu/everglade2.coax` `e2_tile_vs`) pulls each vertex from the store. A
  0.65 m tile draws every sample within 48 m of the eye, every second within 128 m and every
  fourth past that; a 1.3 m tile every sample within 128 m and every second past it. A coarser
  cell draws where any of its own cells is solid.
- **No cracks.** Where two tiles meet at different levels, the finer one's edge vertices take the
  coarser one's line (`terrain_tile_stitched`, `metaverse-engine/world/terrain-tile.coax`, checked
  to level 3). `everglade/client/gpu/ground-tiles-check.coax` runs the real store through the
  shader's own vertex placing at every mix of neighbouring levels and at the levels the page picks
  along the walk.
- **The origin.** The vertex stage works in world coordinates, in f32. Everglade's land lies within
  1,024 m of the world's origin, where an f32's step is 2^-14 m, 0.061 mm. That is under a pixel
  at any distance the camera can get to the ground, and neighbouring tiles compute shared vertices
  by the same expression, so they agree bit for bit. So the GPU needs no floating origin; the page's
  origin (`cell_rebase`) stays for the room's and the index's sake.
- **Measured** (headless Chrome, 1440x900, dpr 1, at the spawn, ten seconds unthrottled, before
  and after #13509): the page's `.vpk3` went from 32.6 MB to 31.8 MB, and the roots lost the
  tiles' 4.4 MB mesh. The 17 cells the spawn fetches came to 4.22 MB before (each carried the
  whole 0.7 MB record) and 2.36 MB after; a cell's own ground is 0 to 153 KB, 1.04 MB for all
  24 tiles, with 182 KB of far levels in the catalog. GPU time stayed 2.04 ms a frame before and
  2.05 ms after (p50; the scene pass 2.0 ms both). With vsync the page holds 60 FPS, at 6.5 ms of
  GPU a frame. At the spawn the ground is 24 draws: 1 tile at level 0, 12 at level 1, 11 at
  level 2, 9 of them far-only. The page's memory is 64.9 MB of its 64 MiB: the tiles' slots cost
  2.4 MB of state, so a set keeps one normal code a sample and the land at most 26 tiles
  (`E2G_TILES`).
- **Readout.** `coaxial_field_probe(174)` the tile draws, `(175)` to `(177)` the slots at each
  level, `(178)` the far-only slots, `(199)` the far record's status.

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
  late at a gallop never holds you after your first cell; a wrong, old or failed answer is refused;
  a step toward a cell that is not in is held, and one back or off the manifest is not.
- `everglade/world/cell-content.coax`: cells' planting put in one by one in any order is the
  whole planting, and taken out again it is gone; a blob that does not read or is another
  planting's is refused.
- `everglade/world/cell-pack.coax`: every planting cell has a carrier that is in wherever its
  pieces show, or stays in the catalog; every piece is in one blob or the catalog's planting.
- `everglade/model/tools/cell-pack.coax`: a pack takes the cells entry last, each cell's line
  finds its bytes, and the catalog's planting and every cell's together are the whole planting.
- `everglade/client/client.coax` ("the world by cell"): a cell's content is taken for its own
  cell once, and given back when the cell leaves.
- `everglade/world/ground.coax`: the cells' records together publish the whole land's ground; a
  cell that leaves takes its tiles; a damaged record or far record changes nothing; the drawn
  normals are the pack's; every level's corners are filled.
- `everglade/client/gpu/ground-tiles-check.coax`: no level shows a crack (above).
- `everglade/client/walk-test.coax`: you walk the path through the real controller, and a route
  walk for each place (the Glade, the Sunlit Path, the Deep Canopy, the Clearing, the Goblin Camp)
  with the cells stepped as the page steps them. Every place you may stand is on the manifest.

## What is left

These are not built. Each is tracked on #13280.

- **Meshes and textures by cell.** The planting and the ground are paged by cell; the pack's
  meshes and textures are roots (#13303).
- **The native apps** bundle the tracked pack and hold the world whole. Their reader has the same
  three steps (`aethyr_pack_cell_ranges`, `metaverse-engine/host/README.md`).
- **What the hold is not.** Every push the page makes goes through `e2_move_in` and is held: your
  keys and stick, the run after Character Create, a film stage's walk and an agent's walk-to
  (`drive.coax`). A mount's input has to go through it too when riding lands (#13268). A move the
  room makes is not held and cannot be: a blast's throw, or any correction, is the room's own
  step, and the room holds every cell. If one carries you into a cell that is not in, the page
  follows the room and counts the frames (probe 184). A page that could refuse the room's step
  could stall its own character, so the fix for that case is cells that load sooner, not a hold.
- **`everglade-parity` and `everglade/features.coax`** stay until every zone is data.
- **Proof on a phone and in a visible Chrome.** A headless pass is not a claim that the walk is
  seamless.
