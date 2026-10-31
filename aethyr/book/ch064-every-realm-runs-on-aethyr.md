# 64. Every realm runs on Aethyr

> **Status: unimplemented — design draft.** Engine-core task: `ec-migrate`
> (#12609); the engine-core agent fills this chapter in as realms move.
> Today every realm keeps some private copy of a loop, a camera, an input
> path or ground logic. Nothing here runs yet.

The book ends where the engine-core milestone does: the Grid, made realms,
Everglade, Dead Gears, Ice Cream Man, Hecate's Cemetery, Fairy Forest and the
space games all run on Aethyr's shared loop, spaces, input, queries and
camera, and their private copies are deleted. A realm is then its content,
its rules and its look, and nothing else.

## What this chapter teaches

- How to migrate a running realm without changing what players see except
  where a fix is visible.
- An inventory of private copies, and a check that keeps them from coming
  back.
- The order: foundations first, then the smallest realm, then the largest.

## The method

Migration is per realm and per subsystem, each step a commit with the realm's
own checks, its golden and the 60 FPS rule passing:

1. Inventory the realm's private loop, camera follow, input path and ground
   logic.
2. Replace each with the Aethyr module (chapters 1, 2, 4, 7 and 10), keeping
   the realm's tuning as data.
3. Record a session before and after; differences are allowed only where a
   bug is fixed, and each is named.
4. Delete the private copy.

The order follows risk: the Grid ([chapter 56](ch056-the-grid.md)), whose
seams are most visible; Hecate's Cemetery and Fairy Forest, which already use
the shared controller and orbit; made realms; Ice Cream Man and Dead Gears,
whose rule steps and entities must keep their traces and checks; Everglade;
then the space games.

A check reads each realm module's load list and its exported symbols and
fails if a realm defines its own frame loop, follow camera, input mapping or
ground query instead of loading Aethyr's.

## What exists today

| Realm | Private copies today |
| --- | --- |
| The Grid | `verse/grid.coax` sticks and frame stepping; per-frame floor projection; grid-unit scale in `newland-walk.coax` |
| Made realms | camera modes and side-view collision in `myrealm.coax`; `droprealm.coax`'s own walk and follow |
| Everglade | forest bodies and edge hold, its own camera smoothing, effect plumbing in `everglade.coax` |
| Dead Gears | `ecs_clock_frame` and the entity system in `verse/deadgears/ecs.coax`; the arena's walking grid |
| Ice Cream Man | the 30-a-second rules loop, input mapping and boom camera in `verse/icecream/` |
| Hecate's Cemetery | controls and flight handover in `villa.coax` and `villa-face.c` |
| Fairy Forest | flight in `fairy-face.c` |
| Space games | 120 Hz substeps in `orbitleague.coax`; lockstep ticks and `spacecam.coax` in Belt 2200 |

Shared modules already in use: [verse/player.coax](../../verse/player.coax),
[camera.coax](../../verse/camera.coax),
[terrain.coax](../../verse/terrain.coax),
[particles.coax](../../verse/particles.coax),
[cinematic.coax](../../verse/cinematic.coax). The task list is
`apps/openagents-web/projects/verse/work/engine-core.json`.

## What it builds

- Each realm on `aethyr/`; the private copies above deleted.
- The no-private-copies check, run with every realm's tests.

## Acceptance checks and oracles

From `ec-migrate`:

- No realm keeps a private loop, camera follow, input path or ground logic.
- Goldens change only where a fix is visible; the 60 FPS checks hold.
- The Grid's floor is one consistent world-space grid from any position.

And from the chapters it depends on: each realm's own tests
(`verse/deadgears/test.sh`, `verse/icecream/test.sh`, the modules'
`#test`s), a recorded session per realm replaying bit-identically natively
and in the browser, and every realm declaring its budgets.

## Dependencies

Every earlier chapter, through the realms of chapters 56–63. Blocked by the
engine-core tasks `ec-loop`, `ec-space`, `ec-state`, `ec-input`,
`ec-replay`, `ec-query`, `ec-entity`, `ec-camera` and `ec-budget`.
