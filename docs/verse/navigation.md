# NPC navigation and steering

This page describes how a non-player character (NPC) moves after a target in Aethyr: a guard, a
monster or a companion. The service lives in the engine, in
[`aethyr/ai/steer.coax`](../../aethyr/ai/steer.coax). Zones wire it in; they don't write their own
chase code. Issue: [#13074](https://github.com/OpenAgentsInc/openagents/issues/13074).

## What the service does

The service gives each NPC five behaviours:

- **Path.** It walks round what stands in the way. When a step is held back, it lays a walking grid
  (a window of up to 40 by 40 cells, 0.5 m each) over the NPC and its goal, searches it with the
  flow field from [`aethyr/nav/grid.coax`](../../aethyr/nav/grid.coax), and pulls the way taut to
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
| Engine | The agent, its tuning, the window, planning, pursuit, circling, body avoidance and the leash | `aethyr/ai/steer.coax` |
| Engine | A window filled from a realm's colliders and terrain (boxes, capsules, hulls, heightfields, steep ground) | `aethyr/ai/steer-colliders.coax` |
| Game | Each kind's tuning: its speed, body, reach, ring and leash | `E2G_NAV` in `everglade/server/guard.coax`; `hostile_tune` in `verse/experimental-hostiles.coax` |
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

- The Everglade II path guard fights on it in the room (`e2g_fight`). His leash stays his own:
  `E2G_LEASH` from where he was struck, then back to his walk.
- The Experimental monsters chase, circle and leash on it (`hostile_steer`). The dungeon's doorway
  graph supplies their waypoints between rooms.

## Checks

`coax/bin/coax test aethyr/ai/steer.coax --library` checks the following:

- A pursuer at the target's own run speed closes on a target crossing at full run. Plain seeking at
  the same speed never does.
- A pursuer paths round a wall and through its doorway, and never stands in the wall.
- In melee, it changes its angle round the target over a few seconds and stays within reach.
- Two pursuers after one target never overlap.
- The leash takes it home, where it waits until the target comes back within the leash.

`aethyr/ai/steer-colliders.coax` checks the same doorway with the window filled from colliders.
The guard and the monsters have their own checks in their files.

## Limits

- A window covers at most 20 m. A goal further away gets a window laid toward it, and the zone's
  own map supplies anything longer, such as the dungeon's rooms.
- Filling a window costs one stand test per cell. The service plans only when a step is held back
  or a path is under way, and at most every `replan` seconds.
