# 76. Find a way through

> **Status: unimplemented — design draft.** Today three path systems exist.
> [`verse/terrain.coax`](../../verse/terrain.coax) routes followers along a
> realm's path graph (Hecate's dog, the glowing way to a memorial), Everglade's
> townsfolk walk the town's road graph by shortest length
> (`verse/everglade/routines.coax`), and Dead Gears flows its creatures over a
> 0.5 m walking grid with Dijkstra (`verse/deadgears/arena.coax`). No realm
> shares another's. This chapter plans one navigation module. Nothing in
> `aethyr/` runs yet.

A character that walks to a place needs a route that avoids what it can't
cross, and a step each frame that follows the route without walking into
other characters. This chapter builds both, over the same ground and
colliders that [chapter 8](ch008-ground-and-collision.md) uses for the
player.

## What this chapter teaches

- How to represent where a character can walk: a path graph, a walking grid,
  or both.
- How to find the shortest way with a stated cost, and how to stop when no
  way exists.
- How to follow a route step by step with steering and simple avoidance.
- How to keep a route valid when the world changes, for example after an
  edit or a fallen tree.

## Outline

1. **Path graphs.** A graph is nodes (x, z pairs) and links (two node
   numbers each); each link is a strip `half` metres either side of its line.
   Everglade's streets use `EG_NODE_XZ`, `EG_LINK_AB` and `EG_PATH_HALF`
   (1.5 m). `verse_path_nearest` finds the nearest point on any link.
2. **Cost and the shortest way.** A metre over grass costs `OFF_PATH` (2.2)
   metres on a path, so a follower takes the path only when it is the shorter
   way. Search is Dijkstra over the graph with a fixed tie-break order, so
   the same query gives the same route on every host.
3. **Walking grids and flow fields.** Dead Gears' grid has `NAV_CELL` 0.5 m
   cells, `NAV_NX` × `NAV_NZ` (64 × 76 = 4,864) cells, and an agent
   clearance `NAV_CLEAR` of 0.38 m. A flow field to a goal gives every cell
   its next cell, so many creatures share one search. Dead Gears keeps three:
   to you, to where the support engineer heads, and to the reactor.
4. **Memoryless next steps.** `verse_route_next` picks the next point from
   where a follower stands, with no stored route. This makes followers
   restartable and replay-safe. `verse_route_points` lists the whole route
   for drawing, such as the glowing line in Hecate's Cemetery.
5. **Following and avoidance.** A follower walks at `FOLLOW_WALK` (2.6 m/s),
   runs at `FOLLOW_RUN` (5.2 m/s) beyond `FOLLOW_FAR` (9 m), accelerates at
   `FOLLOW_ACCEL` (14 m/s²) and turns at `FOLLOW_TURN` (9 per second).
   Among scenery (`FOLLOW_BLOCK` boxes) it slides along a face toward the
   nearer way round. The chapter adds separation between characters and a
   rule for two characters meeting in a narrow street.
6. **Changes in the world.** Everglade's guide checks whether an edit, a
   spell's wall or rubble touches the walked strip of a route
   (`verse/everglade/guide-route.coax`). Aethyr generalizes this: an edit
   marks the links and cells it covers, and only routes over them replan.
7. **Unreachable targets.** A search that finds no way returns a typed
   result naming the nearest reachable point. A character that makes no
   progress for a stated time counts as stuck and replans once, then stops.
8. **Other domains.** Swimming at half pace in water, as townsfolk already
   do through `verse/water-wade.coax`, and flying routes that rise over
   roofs, as made-realm owls do. Space navigation stays in
   [chapter 42](ch042-reach-a-moving-target.md).

## What exists today

- [verse/terrain.coax](../../verse/terrain.coax): the path graph,
  `verse_route_next`, `verse_route_points`, `Follower`,
  `verse_follower_step` and `verse_follower_step_among`.
- [verse/everglade/plan.coax](../../verse/everglade/plan.coax): Everglade's
  street and trail graph.
- [verse/everglade/routines.coax](../../verse/everglade/routines.coax):
  daily walks over that graph at `FOLK_WALK` (1.2 m/s).
- [verse/everglade/guide-route.coax](../../verse/everglade/guide-route.coax):
  the route a guard's directions describe, and what blocks it.
- [verse/deadgears/arena.coax](../../verse/deadgears/arena.coax): the
  walking grid and flow fields.
- [apps/openagents-web/villaguide.coax](../../apps/openagents-web/villaguide.coax):
  the way to your tombstone along the garden paths.
- [docs/research/game-engine-systems.md](../../docs/research/game-engine-systems.md),
  section 38: the navigation inventory.

## What it builds

- `aethyr/nav/graph.coax`: path graphs, nearest points and shortest routes.
- `aethyr/nav/grid.coax`: walking grids and shared flow fields.
- `aethyr/nav/follow.coax`: route following, steering and avoidance.
- `aethyr/nav/invalidate.coax`: replanning after edits.

## Acceptance checks and oracles

The chapter is done when:

- Routes on a fixed graph match an independent shortest-path reference for
  every pair of nodes, including ties.
- Dead Gears' flow-field check passes on `aethyr/nav/grid.coax` with the
  same next cells, and its demo checks still pass.
- Over a recorded minute, Hecate's dog never crosses a hedge or plot
  collider.
- An edit off every route replans nothing; an edit on a route replans only
  the routes it touches.
- An unreachable target returns its typed result within a bounded number of
  search steps.
- Navigation replays bit for bit natively and in the browser.

## Dependencies

Chapters 1 (the clock), 6 (entities), 7 (queries), 8 (ground and
collision), 9 (the walking rules characters share) and 52 (edits that
invalidate routes). Coax R14 bounded control for fixed-size search state.
