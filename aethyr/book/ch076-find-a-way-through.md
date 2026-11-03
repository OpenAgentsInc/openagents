# 76. Find a way through

> **Status: partly implemented.** The walking grid is built and shared:
> [aethyr/nav/grid.coax](../../aethyr/nav/grid.coax) holds a grid of any
> size up to 16,384 cells, eight-neighbour steps that never cut a blocked
> corner, and Dijkstra flow fields with a fixed tie-break, bounded by a cost
> and by a number of settled cells, with typed results. Its checks compare
> every cell's cost, the order cells settle in and every next cell with a
> brute-force oracle
> ([aethyr/tests/nav-oracle.coax](../../aethyr/tests/nav-oracle.coax)), and
> [aethyr/tests/nav-deadgears.coax](../../aethyr/tests/nav-deadgears.coax)
> shows that on Dead Gears' deck it gives the same costs, bit for bit, and
> the same next cells as `verse/deadgears/arena.coax`. No realm runs on the
> module yet: Dead Gears keeps its own copy, Everglade's townsfolk keep their
> road graph and Hecate's Cemetery its garden paths
> ([aethyr/terrain.coax](../../aethyr/terrain.coax)). Path graphs, following
> and avoidance, replanning after edits and browser replay are still a
> design. See [Still to do](#still-to-do).

A character that walks to a place needs a route that avoids what it can't
cross, and a step each frame that follows the route without walking into
other characters. This chapter builds the first half for walking grids: a
flow field that gives every cell its walking distance to one goal, so many
characters share one search. The same ground and colliders that
[chapter 8](ch008-ground-and-collision.md) uses for the player decide which
cells a character fits in.

## What this chapter teaches

- How to represent where a character can walk as a grid of cells, and how a
  diagonal step is refused past a blocked corner.
- How to find every cell's shortest way to a goal with a stated cost, and
  why a fixed tie-break makes the result the same on every host.
- How to bound a search by cost and by work, and how to report a target
  with no way to it as a typed result.
- How to check a search against a brute-force oracle, cell by cell.

## The grid

A grid is `nx` by `nz` square cells, `cell` metres each, from its
south-west corner `(x0, z0)`. Cell `c` is column `c % nx` and row `c / nx`,
so cell numbers run west to east and then south to north. The caller owns
the storage: a walkable flag and a cost for each cell, and the search's
heap and slot arrays. A grid has at most `NAV_GRID_MOST` cells.

<!-- source: aethyr/nav/grid.coax grid -->
```jai
NAV_GRID_MOST :: 16384;       // cells a grid may have
NAV_GRID_FAR :: 1000000.0;    // the cost of a cell the search never reached
NAV_GRID_DIAG :: 1.4142135;   // a diagonal step, in cells

NavGrid :: struct { nx: int; nz: int; cell: f32; x0: f32; z0: f32; }

NavFlowStatus :: enum u8 { ok; cut; bad_grid; bad_store; goal_off; goal_blocked; }
// A flow's outcome: whether it was filled (ok), filled until its steps ran out (cut), or
// refused with nothing written; its goal cell, how many cells it settled, and how many times
// it lowered a cell's cost (the search's work).
NavFlow :: struct { status: NavFlowStatus; goal: int = -1; settled: int; lowered: int; }

// The eight neighbours: east, west, north, south, then the diagonals.
NAV_GRID_DX :: [8] int.[1, -1, 0, 0, 1, 1, -1, -1];
NAV_GRID_DZ :: [8] int.[0, 0, 1, -1, 1, -1, 1, -1];
```

A straight step costs one cell and a diagonal `NAV_GRID_DIAG` cells, Dead
Gears' 1.4142135. A diagonal is refused when either cell it passes between
is blocked, so a character's circle never clips a wall's corner:

<!-- source: aethyr/nav/grid.coax step -->
```jai
// The cell one step from c in direction k (0 to 7), or -1 when that leaves the grid, isn't
// walkable, or is a diagonal past a blocked corner.
nav_grid_step :: (g: NavGrid, walk: [] bool, c: int, k: int) -> int {
  x := c % g.nx + NAV_GRID_DX[k]; z := c / g.nx + NAV_GRID_DZ[k];
  if x < 0 || z < 0 || x >= g.nx || z >= g.nz { return -1; }
  to := z * g.nx + x;
  if !walk[to] { return -1; }
  if k >= 4 {
    if !walk[(c / g.nx) * g.nx + x] || !walk[z * g.nx + c % g.nx] { return -1; }
  }
  return to;
}
```

## The flow field

`nav_grid_flow` runs Dijkstra from the goal. A binary heap holds the cells
waiting to settle, ordered by cost and then by cell number, so two cells at
the same cost settle in the same order on every host. A grid, storage or
goal it can't use is refused before anything is written. The search stops
when it settles a cell beyond `max_cost`, leaving cells further away at
`NAV_GRID_FAR`, and after `steps` settled cells it stops and says `cut`.
Every step costs more than nothing, so a settled cell is never lowered
again and needs no mark:

<!-- source: aethyr/nav/grid.coax flow -->
```jai
// Fills cost with every cell's walking distance to `goal`. A cell settled beyond max_cost
// ends the search, so cells further away keep NAV_GRID_FAR; after `steps` settled cells it
// stops and says cut. A bad grid, short storage, or a goal off the grid or not walkable is
// refused before anything is written. heap and slot are the search's own scratch; order,
// when it isn't empty, gets the cells in the order they settled.
nav_grid_flow :: (g: NavGrid, walk: [] bool, cost: [] f32, heap: [] s32, slot: [] s32, order: [] s32,
                  goal: int, max_cost: f32, steps: int) -> NavFlow {
  out: NavFlow;
  if !nav_grid_ok(g) { out.status = NavFlowStatus.bad_grid; return out; }
  cells := nav_grid_cells(g);
  if walk.count < cells || cost.count < cells || heap.count < cells || slot.count < cells { out.status = NavFlowStatus.bad_store; return out; }
  if order.count != 0 && order.count < cells { out.status = NavFlowStatus.bad_store; return out; }
  if goal < 0 || goal >= cells { out.status = NavFlowStatus.goal_off; return out; }
  if !walk[goal] { out.status = NavFlowStatus.goal_blocked; return out; }
  for c: 0..<cells { cost[c] = f32(NAV_GRID_FAR); slot[c] = -1; }
  out.goal = goal;
  cost[goal] = 0;
  size := 1;
  heap[0] = s32(goal);
  // Steps cost more than nothing, so a settled cell is never lowered again: no mark is needed.
  while size > 0 {
    if out.settled >= steps { out.status = NavFlowStatus.cut; return out; }
    c := int(heap[0]);
    size -= 1;
    nav_grid_swap(heap, slot, 0, size);
    nav_grid_down(cost, heap, slot, size);
    if order.count > 0 { order[out.settled] = s32(c); }
    out.settled += 1;
    here := cost[c];
    if here > max_cost { break; }
    for k: 0..<8 {
      to := nav_grid_step(g, walk, c, k);
      if to < 0 { continue; }
      through := here + ifx k < 4 then g.cell else g.cell * f32(NAV_GRID_DIAG);
      if through >= cost[to] { continue; }
      cost[to] = through;
      out.lowered += 1;
      if slot[to] == -1 { heap[size] = s32(to); slot[to] = s32(size); size += 1; }
      nav_grid_up(cost, heap, slot, int(slot[to]));
    }
  }
  return out;
}
```

A character reads the field one cell at a time. It goes to the neighbour
lowest on the field, the first of equals in direction order, so it keeps
no route and can be restarted from anywhere:

<!-- source: aethyr/nav/grid.coax downhill -->
```jai
// The neighbour of c lowest on the field (the first of equals in direction order), or -1 when
// none is lower than c.
nav_grid_downhill :: (g: NavGrid, walk: [] bool, cost: [] f32, c: int) -> int {
  best := -1; low := cost[c];
  for k: 0..<8 {
    to := nav_grid_step(g, walk, c, k);
    if to < 0 { continue; }
    if cost[to] < low { low = cost[to]; best = to; }
  }
  return best;
}
```

A query from a cell is typed. It is `there` at the goal, `step` with the
next cell, or `no_way`. With no way, `nearest` names the reachable cell
nearest to it, by squared cell distance and then by cell number, so a
character can walk as near as it can get:

<!-- source: aethyr/nav/grid.coax way -->
```jai
NavWayStatus :: enum u8 { there; step; no_way; refused; }
// From one cell: there (it is the goal), a step to `next`, or no way, with `nearest` the
// reachable cell nearest it (by squared cell distance, then cell number; -1 when the field
// reached nothing). `cost` is the walking distance from the cell, or from `nearest`.
NavWay :: struct { status: NavWayStatus; next: int = -1; nearest: int = -1; cost: f32; }

nav_grid_way :: (g: NavGrid, walk: [] bool, cost: [] f32, flow: NavFlow, from: int) -> NavWay {
  way: NavWay;
  way.status = NavWayStatus.refused;
  if flow.status != NavFlowStatus.ok || !nav_grid_ok(g) { return way; }
  cells := nav_grid_cells(g);
  if from < 0 || from >= cells || walk.count < cells || cost.count < cells { return way; }
  if cost[from] < f32(NAV_GRID_FAR) {
    way.cost = cost[from];
    if from == flow.goal { way.status = NavWayStatus.there; return way; }
    way.next = nav_grid_downhill(g, walk, cost, from);
    way.status = ifx way.next < 0 then NavWayStatus.no_way else NavWayStatus.step;
    if way.next < 0 { way.nearest = from; }
    return way;
  }
  way.status = NavWayStatus.no_way;
  best: int;
  fx := from % g.nx; fz := from / g.nx;
  for c: 0..<cells {
    if !(cost[c] < f32(NAV_GRID_FAR)) { continue; }
    dx := c % g.nx - fx; dz := c / g.nx - fz;
    d := dx * dx + dz * dz;
    if way.nearest < 0 || d < best { best = d; way.nearest = c; }
  }
  if way.nearest >= 0 { way.cost = cost[way.nearest]; }
  return way;
}
```

## A grid worked by hand

The example's grid is 6 by 5 cells of 1 m, with a wall down column 2 that
leaves a gap in the top row. The goal is cell 0, the south-west corner.

```text
row 4  24 25 26 27 28 29     . . . . . .
row 3  18 19  # 21 22 23     . . # . . .
row 2  12 13  # 15 16 17     . . # . . .
row 1   6  7  #  9 10 11     . . # . . .
row 0   0  1  #  3  4  5     G . # . . .
```

1. The goal settles first at 0. It offers 1 to cells 1 and 6 and 1.4142135
   to cell 7 (cells 1 and 6 are open, so the diagonal is allowed).
2. Cells 1 and 6 both cost 1. Cell 1 has the lower number, so it settles
   first, then cell 6, then cell 7 at 1.4142135. Then come cell 12 at 2,
   cell 13 at 1 + 1.4142135 = 2.4142137 in `f32`, cell 18 at 3 and cell 19
   at 3.4142137. These are the first eight cells the example prints.
3. Column 1 rises to cell 25 at 4.4142137, and the gap (cell 26) costs
   5.4142137. From cell 19 to cell 26 is a diagonal past cell 20, which is
   wall, so it is refused.
4. On the far side, cell 27 costs 6.4142137 and cell 21 under it 7.4142137:
   the diagonal from the gap to cell 21 passes cell 20 and is refused too.
   Cell 22 takes the diagonal from cell 27 at 7.8284273. Going down and
   east by diagonals, cell 5 ends at 11.24264.
5. From cell 5 the lowest neighbour is cell 10 (9.828426), lower than cell
   4 (10.828426) and cell 11 (10.2426405). Going downhill gives 10, 15, 21,
   27, 26, 25, 18, 12, 6 and 0, the way round the wall.

All 26 open cells settle, and the search lowers a cell's cost 25 times.

With `max_cost` 2, cells 0, 1, 6, 7 and 12 settle at 2 or less. The sixth
cell to settle is cell 13, at 2.4142137, which is beyond 2, so the search
stops there. Cell 18 keeps the 3 that cell 12 offered it. Cell 24 was never
offered a cost and stays at `NAV_GRID_FAR`.

Block cells 4 and 11, and cell 5 is walled in: its west and north
neighbours are blocked, and the diagonal to cell 10 passes both. The
reachable cell nearest to it is cell 10, at a squared distance of 2 cells,
nearer than cell 3 at 4.

## The example

[aethyr/examples/flow-round-a-wall.coax](../examples/flow-round-a-wall.coax)
fills the field, prints every cell's cost row by row from the north, the
first eight cells to settle and the way from cell 5. It then cuts a search
short at 10 settled cells, bounds one by a cost of 2 m, walls cell 5 in and
asks for three goals the field refuses: a wall cell and two cells off the
grid.

<!-- source: aethyr/examples/flow-round-a-wall.coax example -->
```jai
#load "../nav/grid.coax";
#import "Basic";

// The chapter's 6 by 5 grid: a wall down column 2 with a gap in the top row. One flow field
// to the south-west corner; the way from the south-east corner, cell by cell; a search cut
// short and one bounded by cost; a walled-in cell; and three goals the field refuses.
main :: () {
  g := NavGrid.{nx = 6, nz = 5, cell = 1, x0 = 0, z0 = 0};
  walk: [30] bool; cost: [30] f32; heap: [30] s32; slot: [30] s32; order: [30] s32;
  for c: 0..<30 { walk[c] = c % 6 != 2 || c / 6 == 4; }
  f := nav_grid_flow(g, walk[..], cost[..], heap[..], slot[..], order[..], 0, 1000, 30);
  print("flow to cell 0: %, % cells settled, % costs lowered\n", f.status, f.settled, f.lowered);
  for r: 0..<5 {
    z := 4 - r;
    print("row %:", z);
    for x: 0..<6 {
      c := z * 6 + x;
      if walk[c] { print(" %", cost[c]); } else { print(" #"); }
    }
    print("\n");
  }
  print("settled first:");
  for i: 0..<8 { print(" %", order[i]); }
  print("\n");
  at := 5;
  print("from cell 5:");
  for s: 0..<12 {
    w := nav_grid_way(g, walk[..], cost[..], f, at);
    if w.status != NavWayStatus.step { print(" (%)\n", w.status); break; }
    print(" %", w.next);
    at = w.next;
  }
  cut := nav_grid_flow(g, walk[..], cost[..], heap[..], slot[..], order[0..<0], 0, 1000, 10);
  print("with 10 steps: %, % settled\n", cut.status, cut.settled);
  near := nav_grid_flow(g, walk[..], cost[..], heap[..], slot[..], order[0..<0], 0, 2, 30);
  print("up to 2 m: %, % settled, cell 18 at %, cell 24 at %\n", near.status, near.settled, cost[18], cost[24]);
  walk[4] = false; walk[11] = false;
  f = nav_grid_flow(g, walk[..], cost[..], heap[..], slot[..], order[0..<0], 0, 1000, 30);
  w := nav_grid_way(g, walk[..], cost[..], f, 5);
  print("cell 5 walled in: %, nearest reachable cell %, % m from the goal\n", w.status, w.nearest, cost[w.nearest]);
  goals := [3] int.[2, 30, -1];
  for k: 0..<3 {
    r := nav_grid_flow(g, walk[..], cost[..], heap[..], slot[..], order[0..<0], goals[k], 1000, 30);
    print("goal %: %, goal cell %\n", goals[k], r.status, r.goal);
  }
}
```

```text
flow to cell 0: ok, 26 cells settled, 25 costs lowered
row 4: 4.0 4.4142137 5.4142137 6.4142137 7.4142137 8.414213
row 3: 3.0 3.4142137 # 7.4142137 7.8284273 8.828427
row 2: 2.0 2.4142137 # 8.414213 8.828427 9.2426405
row 1: 1.0 1.4142135 # 9.414213 9.828426 10.2426405
row 0: 0.0 1.0 # 10.414213 10.828426 11.24264
settled first: 0 1 6 7 12 13 18 19
from cell 5: 10 15 21 27 26 25 18 12 6 0 (there)
with 10 steps: cut, 10 settled
up to 2 m: ok, 6 settled, cell 18 at 3.0, cell 24 at 1000000.0
cell 5 walled in: no_way, nearest reachable cell 10, 9.828426 m from the goal
goal 2: goal_blocked, goal cell -1
goal 30: goal_off, goal cell -1
goal -1: goal_off, goal cell -1
```

The refused goals write nothing and name no goal cell (`-1`). A cut search
is `cut`, and `nav_grid_way` refuses to read a field that isn't `ok`.

## Checks

- `coax/bin/coax test aethyr/nav/grid.coax --library`: 13 of 13. The step
  costs and the corner rule on the chapter's grid. Every goal on that grid
  against the oracle. 90 seeded grids up to 10 by 8 with a third of the
  cells blocked: every cost, every next cell and every nearest reachable
  cell against the oracle. 24 open seeded grids up to 12 by 12: the order
  cells settle in and the number of lowered costs against the oracle. Two
  grids, found by search, where a cell already in the heap is lowered after
  swaps moved it. The cost and step bounds, refusals of a bad grid, short
  storage and an unusable goal (nothing written), the largest grid, a cell
  from a point, and the typed ways.
- `coax/bin/coax mutate aethyr/nav/grid.coax --library --root-only`: 276
  mutants, 276 killed.
- `coax/bin/coax test aethyr/tests/nav-deadgears.coax --library`: 32 of 32,
  with Dead Gears' own checks. On Dead Gears' deck (64 by 76 cells of 0.5 m,
  11 boxes), for six goals, the module's field equals `nav_flow_to`'s in all
  4,864 cells, bit for bit, and `nav_grid_downhill` gives the same next cell
  as `nav_downhill` in every cell.

The oracle in [aethyr/tests/nav-oracle.coax](../../aethyr/tests/nav-oracle.coax)
shares no code with the module. It writes the step rules again and relaxes
every step out of every cell until nothing changes (Bellman-Ford), with no
heap and no order. It then sorts the reachable cells by cost and cell number
for the order a search must settle them in, and replays that order to count
the costs a search lowers.

## Still to do

- **Realms on the module.** Dead Gears' `nav_flow_to` and `nav_downhill` can
  call it with the same results (the identity check above), with its goldens
  unchanged. Everglade's townsfolk and guide, and Hecate's dog, use path
  graphs instead.
- **Path graphs.** Today [aethyr/terrain.coax](../../aethyr/terrain.coax)
  holds them: nodes (x, z pairs) and links (two node numbers each), each
  link a strip `half` metres either side of its line. `costs` is Dijkstra
  over at most `MAX_NODES` (48) nodes, where a metre over grass costs
  `OFF_PATH` (2.2) metres on a path; `verse_route_next` picks the next point
  from where a follower stands, and `verse_route_points` lists the whole
  route for a guide line. Everglade's streets (`EG_NODE_XZ`, `EG_LINK_AB`,
  `EG_PATH_HALF` 1.5 m) and its townsfolk (`verse/everglade/routines.coax`)
  have their own. A shared `aethyr/nav/graph.coax` with typed results and
  the same tie-break is not built.
- **Following and avoidance.** `verse_follower_step_among` walks at
  `FOLLOW_WALK` (2.6 m/s), runs at `FOLLOW_RUN` (5.2 m/s) beyond
  `FOLLOW_FAR` (9 m), accelerates at `FOLLOW_ACCEL` (14 m/s²), turns at
  `FOLLOW_TURN` (9 per second) and slides along `FOLLOW_BLOCK` boxes.
  Separation between characters and a rule for two characters meeting in a
  narrow street are not built.
- **Changes in the world.** Everglade's guide checks whether an edit, a
  spell's wall or rubble touches a route (`verse/everglade/guide-route.coax`).
  Marking the cells and links an edit covers, so only routes over them
  replan ([chapter 52](ch052-edit-a-realm-in-place.md)), is not built.
- **Stuck characters.** A character that makes no progress for a stated time
  replans once and then stops.
- **Replay in the browser.** The module runs natively in debug and release;
  a field built in Wasm and compared with native is not recorded.
- **Other domains.** Swimming at half pace in water, as townsfolk do through
  `verse/water-wade.coax`, and flying routes over roofs, as made-realm owls
  do. Space navigation stays in [chapter 42](ch042-reach-a-moving-target.md).

## What exists today

- [aethyr/nav/grid.coax](../../aethyr/nav/grid.coax): walking grids and flow
  fields, with its oracle and the Dead Gears identity check.
- [aethyr/terrain.coax](../../aethyr/terrain.coax): the path graph,
  `verse_route_next`, `verse_route_points`, `Follower`,
  `verse_follower_step` and `verse_follower_step_among`.
- [verse/deadgears/arena.coax](../../verse/deadgears/arena.coax): Dead
  Gears' own walking grid (`NAV_CELL` 0.5 m, 64 × 76 cells, `NAV_CLEAR`
  0.38 m) and its three flow fields: to you, to where the support engineer
  heads, and to the reactor.
- [verse/everglade/plan.coax](../../verse/everglade/plan.coax),
  [routines.coax](../../verse/everglade/routines.coax) and
  [guide-route.coax](../../verse/everglade/guide-route.coax): Everglade's
  street graph, daily walks at `FOLK_WALK` (1.2 m/s), and the route a
  guard's directions describe.
- [apps/openagents-web/villaguide.coax](../../apps/openagents-web/villaguide.coax):
  the way to your tombstone along the garden paths.
- [docs/research/game-engine-systems.md](../../docs/research/game-engine-systems.md),
  section 38: the navigation inventory.

## Acceptance checks and oracles

The chapter is done when:

- Routes on a fixed graph match an independent shortest-path reference for
  every pair of nodes, including ties. (Done for grids: every cell and every
  goal on the chapter's grid, and every cell on 114 seeded grids.)
- Dead Gears' flow-field check passes on `aethyr/nav/grid.coax` with the
  same next cells, and its demo checks still pass. (The same fields and next
  cells are shown; Dead Gears doesn't call the module yet.)
- Over a recorded minute, Hecate's dog never crosses a hedge or plot
  collider.
- An edit off every route replans nothing; an edit on a route replans only
  the routes it touches.
- An unreachable target returns its typed result within a bounded number of
  search steps. (Done for grids: `no_way` with the nearest reachable cell,
  and `cut` after `steps` settled cells.)
- Navigation replays bit for bit natively and in the browser.

## Dependencies

Chapters 1 (the clock), 6 (entities), 7 (queries), 8 (ground and
collision), 9 (the walking rules characters share) and 52 (edits that
invalidate routes). Coax R14 bounded control for fixed-size search state.
