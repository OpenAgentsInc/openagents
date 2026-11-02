# 64. Every realm runs on Aethyr

> **Status: unimplemented — design draft.** Engine-core task: `ec-migrate`
> (#12609). Shared clocks, walking, ground/collision and camera modules are
> already adopted on named paths. This chapter's complete migration,
> per-realm replay corpus and check against private copies returning are
> still to do. Its inventory distinguishes those remaining adapters from
> the shared modules that already run.

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

A planned check reads each realm module's load list and its exported symbols and
fails if a realm defines its own frame loop, follow camera, input mapping or
ground query instead of loading Aethyr's.

## What exists today

| Realm | Shared baseline already in use | Remaining migration |
| --- | --- | --- |
| The Grid | `verse/gridwalk.coax` owns the recorded session, typed input and fixed-step walking; chapter 2 checks native avatar spatial boundaries. | Broader world/entity/render contracts and the complete no-private-copies gate. Do not restore the old face-owned stepping path. |
| Made realms | Declared frame budgets and typed move/undo operations in `myrealm.coax`; shared player paths where adopted. | Camera modes, side-view collision, the drop realm's walk/follow and one document/edit/publication model. |
| Everglade | Shared walking ground/collision and follow camera; debris on `aethyr/loop.coax`; its face uses the player's counted frame time. | Forest-specific spatial/body adapters, edge hold, effect plumbing and a complete recorded realm session. |
| Dead Gears | `ecs_clock_frame` delegates scheduling to `aethyr/loop.coax`; walking uses the shared controller. | The private entity store, rule/input adapters and arena spatial representation. The wrapper's name does not imply a second clock implementation. |
| Ice Cream Man | Declared frame budget and existing rule-trace checks. | `verse/icecream/play.coax` still owns its frame clamp/accumulator, input mapping and the separate boom camera. |
| Hecate's Cemetery | Shared walking/camera and paid-step fetch in the actual villa face. | Flight/boat handover, special controls and the remaining floor-relative adapters. |
| Fairy Forest | Shared walking/camera and counted frame time. | Flight in `fairy-face.c` and its recorded-session integration. |
| Space games | Orbit League's 120 Hz steps use `aethyr/loop.coax`; both games have existing simulation/network checks. | Belt's lockstep schedule, space-camera/world-frame adapters and shared session/profile integration. |

Clock migration already moved these paths onto `aethyr/loop.coax`
([chapter 1](ch001-a-frame-has-a-fixed-step.md), "What it replaced"): Dead
Gears' `ecs_clock_frame`, Orbit League's step cap, `verse/demolish.coax`,
Hold the Stair's solo and co-op clocks, and the 0.1 s clamps in five page
faces. This is evidence of adoption, not completion of `ec-migrate`.
Ice Cream Man's 30-a-second frame accumulator still needs migration; Belt's
network commits and game-time ticks need an explicit session integration.
Keep those scopes distinct from the shared walking and rigid-game clocks.

Ice Cream Man's accumulator is an open conformance gap against the
[one-clock invariant](../../INVARIANTS.md#a-realms-frame-time-is-what-its-clock-counts-and-no-frame-runs-past-8-steps):
`ice_play_frame` in [play.coax](../../verse/icecream/play.coax) still clamps
`frame_dt`, adds it to `acc` and runs its own due-step loop. Migration must
remove that scheduling logic while preserving the rule traces. The invariant
is the required architecture; the remaining private path does not relax it.

The renderer migration also has a working baseline: all catalogue realms
have Coax-generated WGSL and the native Vulkan port's recorded checks
([native-GPU status](../../docs/verse/native-gpu.md#status)). Chapter 11 still
owns common frame/pass/binding admission and the full CPU-reference profile.
Do not count that future consolidation as another shader-source port.
Its first [CPU metadata component](../render/README.md) is now used by
`verse_grid_view` through `verse/grid-render.coax`, before the native matrix
output is written. The browser Grid frame, GPU resource owners and pass
submission remain private adapters. This narrow adoption does not complete
the playable route or `ec-migrate`.

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
