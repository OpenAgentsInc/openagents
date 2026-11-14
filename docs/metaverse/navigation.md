# NPC navigation and steering

This page describes how a non-player character (NPC) moves after a target in the OpenAgents Metaverse Engine: a guard, a
monster or a companion. The service lives in the engine, in
[`metaverse-engine/ai/steer.coax`](../../metaverse-engine/ai/steer.coax). Zones wire it in; they don't write their own
chase code. Issue: [#13074](https://github.com/OpenAgentsInc/openagents/issues/13074).

## What the service does

The service gives each NPC five behaviours:

- **Path.** It walks round what stands in the way. When a step is held back, it lays a walking grid
  (a window of up to 40 by 40 cells, 0.5 m each) over the NPC and its goal, searches it with the
  flow field from [`metaverse-engine/world/nav/grid.coax`](../../metaverse-engine/world/nav/grid.coax), and pulls the way taut to
  the furthest corner it can see. That corner is its aim until the next plan.
- **Pursue.** It aims at where the target will be, not where it is: the target's velocity, smoothed
  over the last ticks, times the time to get there (at most `lead` seconds).
- **Circle.** Within melee reach, it moves round the target on a ring inside its reach. It changes
  direction every few seconds, timed by its own seed, and it stands still during its own swing.
- **Avoid bodies.** No step ends inside another body (other NPCs, the players) deeper than it began.
- **Leash.** Past `leash` metres from home, it walks home, unless it's fighting someone who still
  stands within the leash. Home again, it waits until the target comes back within the leash.

## Engine, game and zone

| Layer | What it owns | Where |
| --- | --- | --- |
| Engine | The agent, its tuning, the window, planning, pursuit, circling, body avoidance and the leash | `metaverse-engine/ai/steer.coax` |
| Engine | A window filled from a realm's colliders and terrain (boxes, capsules, hulls, heightfields, steep ground) | `metaverse-engine/ai/steer-colliders.coax` |
| Game | Each kind's tuning: its speed, body, reach, ring and leash | `hostile_tune` in `metaverse/experimental-hostiles.coax` |
| Zone | Where a body may stand, the bodies near it, and the final slide of a step | `e2_may_stand` and `e2m_keep` (Everglade II); `hostile_blocked_pad`, `hostile_walk` and the dungeon's doorway graph (Experimental) |

Coax has no procedure values, so the zone fills the window itself. It does this only when the
service asks.

## Using it

Give each NPC a `NavAgent` (`nav_agent(x, z, seed)`) and each kind a `NavTune`. Then, each tick:

1. Call `nav_think(agent, tune, tx, tz, dt)` with the target's position. It returns `true` when a
   plan is due.
2. If a plan is due and `nav_wants_window(agent)` is `true`, call `nav_window_place`, fill
   `walk[c]` for each cell from `nav_window_spot`, and call `nav_plan`. Otherwise call `nav_direct`.
3. Call `nav_move(agent, tune, busy, dt, bodies, skip)`. `bodies` holds `x, z, radius` for each
   body near it. `busy` is `true` while the NPC swings.
4. Apply the zone's own slide to the returned step, then call `nav_moved(agent, step, x, z)`.

Optional calls:

- `nav_via(agent, x, z)` gives a waypoint from the zone's own map for this tick, such as a dungeon
  doorway.
- `nav_return(agent)` sends the NPC home, for example when the player stands in a safe spot.
- `nav_engage(agent)` rouses it when it's struck.
- `nav_lost(agent)` resets what it knows when its target changes.

In the room, the service runs on the room's fixed tick, so the room stays authoritative. In
Experimental, it runs on the page's frame.

## Who uses it

- Every creature of an Everglade room moves on the navigation cells (`e2mo_move` through
  `e2nv_go_among` in `everglade/server/nav.coax`): the wolves, the goblins, the Ogre and the path
  guard, on his beat and in a fight (#13427). The guard goes round the players, who are bodies to
  him. Each behavior's notice, circling, blows and leash are the creature runtime's
  (`docs/metaverse/creatures.md`).
- The Experimental monsters chase, circle and leash on it (`hostile_steer`). The dungeon's doorway
  graph supplies their waypoints between rooms.

## A long way round, by a graph

The window covers 20 m and the cells' long way 32 m. A wall longer than that (a row of a field 150 m
long) needs a graph: nodes where the walls end (an aisle's two ends, a doorway) and links where
nothing stands. `metaverse_route_seen` in
[`metaverse-engine/world/route-graph.coax`](../../metaverse-engine/world/route-graph.coax) answers the
point to walk toward now and the whole way's length. The realm says, by its own test of what holds a
walker back, which nodes the walker sees straight and which see the goal. Splice Bots' Digger goes round
the solar farm's rows this way (`d_route`, the farm's aisles from `ff_aisles`). The garden paths'
`metaverse_route_next` (`path-graph.coax`) uses the same search over its links.

A walker that isn't steered by the service keeps its body out of others with `nav_apart_at`, the same
rule `nav_move` uses: Splice Bots' robots and Digger push apart this way (`g_apart`).

## Navigation cells

A creature that moves by the world's navigation cells never needs the zone's own stand test. The
data is built from the colliders the players' step uses, one page for each 64 m world cell
([one world](one-world.md)). Issue:
[#13281](https://github.com/OpenAgentsInc/openagents/issues/13281).

| Layer | What it owns | Where |
| --- | --- | --- |
| Engine | A page: 128 by 128 samples, 0.5 m apart, one bit each. Open and closed, a straight walk, the step's last word (`nav_cells_keep`), obstacles (a shut door, a placed piece), what can be reached from a place, a page's digest | `metaverse-engine/world/nav/cells.coax` |
| Engine | A block of a page built from capsules and hulls: a sample opens where a body of the creature's radius stands clear of them all | `metaverse-engine/world/nav/cells-colliders.coax` |
| Engine | A creature's tick on the cells: the long way, the window, the step and the last word | `metaverse-engine/ai/steer-cells.coax` (`nav_cells_go`) |
| Game | Everglade's pages, from `e2_may_stand` and `e2m_world`, the same calls a player's step makes | `everglade/shared/nav.coax` |
| Game | The zone's manifest: each cell, its page's digest and its open samples | `everglade/world/zone-cells.coax`, written by `everglade/model/tools/zone-cells.coax` |
| Room | Which cells a room holds, the page builder, the monsters' steps, a room's own obstacles, the counts | `everglade/server/nav.coax` |

### How a creature goes to a point

Each tick, the realm says where the creature wants to be and how fast. `nav_cells_go` does the
rest:

1. When a plan is due (every half second, when a step was refused, or when an obstacle changed),
   it checks the straight walk on the samples. If that is shut and the point is more than 8 m
   away, it plans the long way: one search of every sample within 32 m that the creature can walk
   to, then a waypoint 8 m along the way to the sample nearest its goal.
2. It lays the service's 20 m window on the samples and plans in it toward the waypoint or the
   point.
3. It takes the service's step, and `nav_cells_keep` has the last word: the step, a slide along
   one axis, or no step. A creature never ends a tick on a closed sample.

Where no way is open, it comes as near as the samples allow and stands there.

In a room, three more rules hold (`everglade/server/nav.coax`):

- A page is built when a creature first needs it: the cells within 32 m of a creature about to
  move. A host also builds ahead in spare ticks, a slice each tick. Until its pages are in, the
  creature stands.
- A creature on closed ground (a piece laid over it) walks straight to the nearest open ground
  within 3 m.
- A monster on its way home with no way there is put home after 20 s.

### Doors and placed pieces

An obstacle is a circle or a turned box laid over the pages by its ID. A room lays one with
`rm_e2_nav_block` and takes it up with `rm_e2_nav_unblock`. The room adds the creature's radius.
Each change raises the cells' revision, so every creature of that room plans again on its next
tick. Obstacles are a room's own: two rooms on one server share pages, not doors.

### At pack time

Run the manifest tool after any change to the land, the planting, a rock's hull, the camp or
`e2_may_stand`:

```sh
coax/web/tool.sh everglade/model/tools/zone-cells.coax --write everglade/world/zone-cells.coax everglade/content/everglade.content
```

It builds every cell's page and refuses the zone when any of these holds:

- A named point (`everglade/world/points.coax`) can't be walked to from the spawn.
- A content point (`everglade/content/everglade.content`) has no reachable ground within its
  radius, or a spawn names no point.
- A creature walked by `nav_cells_go` from the spawn to each named point in turn, and back,
  doesn't arrive or leaves open ground. This is the route walk test's rule, for creatures.

`--check` compares the committed manifest with what the land builds now. A room keeps a page only
at the manifest's digest, so a stale manifest shows as refused pages and creatures that stand
still, never as creatures on the wrong ground.

### What a room counts

The room's vitals report these totals, and the soak fails a run where either of the first two is
above 0 ([soak](../deployment/soak.md)):

| Line | Meaning |
| --- | --- |
| `navigation creature ticks off the cells` | A monster ended a tick off the cells' open ground. Checked for every monster on every tick |
| `navigation creatures found inside a collider` | A creature's middle was inside a trunk, a rock or a prop, or off the ground a player may stand on. One creature is checked each tick, in turn, against the host's own planting. The guard is checked here too |
| `navigation collider looks` | How many of those checks ran |
| `navigation pages kept`, `pages refused` | Pages built and kept at the manifest's digest, and pages that came out at another digest and were dropped |
| `navigation creature ticks slept` | Monster ticks skipped because the monster's cell wasn't simulating |
| `navigation pages built` | Pages the server has built |

## Grid or mesh

**Decision (2026-11-12, #13281):** creatures path on the 0.5 m sample grid, with two searches of
that one grid: the 20 m window and a 64 m search about the creature. No navigation mesh, and no
coarser level.

The measured case is Everglade's zone: 21 cells, 84,770 open samples, of which 84,657 can be
reached from the spawn. The manifest tool walks a creature at 9 m/s from the spawn to each of the
9 named points in turn and back (`zone-cells.coax`, on a Mac under load):

| Design | Result |
| --- | --- |
| The 20 m window alone | Fails. It presses on a wall whose gap is out of the window's sight (`metaverse-engine/ai/steer-cells-checks.coax`) |
| The window, plus a long way planned on 2 m blocks (a block open when any of its 16 samples is) | Fails 2 of 9 walks. Two blocks that both hold open samples aren't always joined, so a waypoint could be out of reach. A creature walked back and forth by z -72 on the path for 482 m without arriving |
| The window, plus a long way searched on the samples within 32 m | Passes 9 of 9: 1,376 m walked for 1,306 m straight. The walk back from the far clearing is 470 m. A tick costs 40 µs on average. The slowest, one with a long-way search, costs 1.4 ms |

Why not a mesh now:

- The sample grid is exact for the colliders it was built from, and a page is 2 KiB. A mesh needs
  a builder, a point-location structure and a funnel, none of which exist here.
- A long-way search is 16,384 cells at most and runs only when the straight walk is shut, at most
  twice a second for each creature.

When to revisit:

- A zone where a creature must find a way round something more than 32 m deep. The search heads
  for the reachable sample nearest the goal, so a deeper pocket can hold it. The pack-time walks
  catch this for the named points.
- Creatures of very different sizes. A page is built for one radius (0.4 m in Everglade).
- A room where long-way searches show in the tick's p99.

## Checks

`coax/bin/coax test metaverse-engine/ai/steer.coax --library` checks the following:

- A pursuer at the target's own run speed closes on a target crossing at full run. Plain seeking at
  the same speed never does.
- A pursuer paths round a wall and through its doorway, and never stands in the wall.
- In melee, it changes its angle round the target over a few seconds and stays within reach.
- Two pursuers after one target never overlap.
- The leash takes it home, where it waits until the target comes back within the leash.

`metaverse-engine/ai/steer-colliders.coax` checks the same doorway with the window filled from colliders.

The navigation cells' checks:

- `metaverse-engine/world/nav/cells.coax`, `cells-colliders.coax` and `metaverse-engine/ai/steer-cells.coax`: pages,
  obstacles, reach, the build from colliders, a walk round a trunk, a gap out of the window's
  sight, and a door that shuts and opens. `coax mutate --root-only` leaves no survivor in any of
  the three.
- `everglade/shared/nav.coax`: a page's open samples are clear of every collider a player's step
  meets.
- `everglade/server/nav-checks.coax` (in the room's suite, `--only "e2 nav"`): a page is built on
  first need and kept only at the manifest's digest, a slice a tick on a host; a wolf chases a
  player round a trunk; a gate across the path sends the pack through its gap and, shut, holds
  them; a piece laid over a wolf sends it out; a monster whose cell sleeps is home and whole.
The guard and the monsters have their own checks in their files.

## Limits

- A window covers at most 20 m. A goal further away gets a window laid toward it, and the zone's
  own map supplies anything longer, such as the dungeon's rooms.
- Filling a window costs one stand test per cell. The service plans only when a step is held back
  or a path is under way, and at most every `replan` seconds.
