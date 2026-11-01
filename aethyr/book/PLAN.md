# Plan and prerequisites

Proposal, 2026-10-31. The [chapter registry](chapters.json) records which
chapters are written and which are design drafts. The order puts each primitive before anything that uses it, so the
book can be built front to back, but independent branches can start earlier
than their position. The [contents](SUMMARY.md) give the reading order and the
[introduction](README.md) the result of each chapter.

## The order

| Part | Chapters | What the reader has at the end |
| --- | --- | --- |
| I. Foundations | 1–5 | One clock with a fixed step and interpolated drawing; typed spaces and units; state built from declared defaults; typed actions sampled once per step; a session that replays bit for bit. |
| II. A world | 6–10 | Entities with stable IDs and safe lifetimes; one query module; ground and collision that match what is drawn; a walking character; a camera that follows without stepping physics. |
| III. Seeing it | 11–16 | One WGSL renderer with shaders written in Coax and checked against their CPU run; materials; light and shadow; animated figures; UI and the HUD; budgets that refuse. |
| IV. Physics | 17–37 | A physics engine at rough parity with Genesis World 1.4.5: rigid and articulated bodies with contact, five material solvers and their coupling, batched worlds, gradients, sensors, observation pictures, assets, replay and measurement. |
| V. Big worlds | 38–50 | Worlds streamed in cells around a floating origin; spacecraft and the solar system at stated fidelity; rooms, presence and shared state over the network. |
| VI. Making worlds | 51–55 | Aethyrforge: a realm as a typed document, edited in place, with imported assets, published as a frozen record and forked. |
| VII. Realms | 56–64 | The Grid, made realms, Everglade, Dead Gears, Ice Cream Man, Hecate's Cemetery, Fairy Forest and the space games, each rebuilt only from earlier chapters; then every realm on Aethyr with no private copies left. |
| VIII. Hearing it | 65–69 | Integer synthesis, a beat-grid sequencer, mixing with a reverb send, a master that never clips, WAV output and a rendered soundtrack; then real-time playback, sound placed in the world, and music that follows the game's state; then players' and agents' voices placed in the realm. |
| IX. A living world | 70–78 | Water, sky and weather, particle effects, breakable kit pieces, wheeled vehicles, creatures that follow and watch, paths and crowds, characters who talk, and authored camera flights, each moved from `verse/` into one Aethyr module. |
| X. Aethyrforge: generating worlds | 79–81 | A set piece chosen from a typed catalogue by a semantic selector, terrain and scatter laid out from a seed, and a realm made as a reply to another. |
| XI. Shipping realms | 82–86 | Saves that survive schema changes, code changed while a realm runs, inspection and profiling of a running realm, one realm on every host, and accessibility and languages. |

## Engine-core tasks

The engine-core milestone (GitHub #12609, the verse project's `engine-core`
tasks) builds Part I and II's primitives first. Each task owns one chapter.

| Task | Chapter | Module | Blocked by |
| --- | --- | --- | --- |
| `ec-loop` | [1](ch001-a-frame-has-a-fixed-step.md) | `aethyr/loop.coax` | — |
| `ec-space` | [2](ch002-spaces-and-units.md) | `aethyr/space.coax` | — |
| `ec-state` | [3](ch003-state-starts-from-its-defaults.md) | realm state constructors | — |
| `ec-input` | [4](ch004-input-becomes-typed-actions.md) | `aethyr/input.coax` | `ec-loop` |
| `ec-replay` | [5](ch005-replay-a-session-exactly.md) | session records and replay | `ec-loop`, `ec-input`, `ec-state` |
| `ec-entity` | [6](ch006-entities-are-born-and-die.md) | entity IDs and lifetime | `ec-state` |
| `ec-query` | [7](ch007-ask-the-world-where-things-are.md) | `aethyr/query.coax` | `ec-space` |
| `ec-camera` | [10](ch010-the-camera-follows.md) | `aethyr/follow.coax` | `ec-loop`, `ec-space`, `ec-query` |
| `ec-budget` | [16](ch016-budgets-that-refuse.md) | asset identity and budgets | — |
| `ec-migrate` | [64](ch064-every-realm-runs-on-aethyr.md) | every realm on the core | all of the above |

Chapter 8 now owns the admitted ground sampler, mesh contacts and bounded
capsule publication in `aethyr/ground*.coax` and `aethyr/collide.coax`.
Chapter 9 consolidates the walking controller and its declared tuning;
chapter 64 still owns the complete migration of realm adapters. Part III
consolidates the existing Coax shader and host paths in `verse/gpu/`.

## Prerequisites, not a waiting list

| Branch | Needs | Path |
| --- | --- | --- |
| Foundations | Coax only | Clock 1 → spaces 2 and defaults 3 → actions 4 → replay 5. |
| A world | 1–5 | Entities 6 → queries 7 → ground and collision 8 → movement 9 → camera 10. |
| Rendering | 2, Coax's direct WGSL emission ([Coax chapter 17](../../coax/book/ch17-gpu.md)) | Renderer 11 → materials 12 → light 13 → figures 14 → UI 15; budgets 16 apply to all of them. |
| Physics engine | Psionic chapters 4, 37–38 and 107; 109 and 111 for gradients; Coax R12–R20, then R21–R24 | Scene 17 → rigid bodies, joints and contact 18–22 → batches 31, assets 35, replay 36 (with Psionic chapter 167's control); particle and mesh solvers 23–28 → coupling 29–30 → gradients 32; sensors and observations 33–34; measurement 37. |
| Streaming | 6, 7, 16 and the rigid chapters 18–22 | Cells, rings and a floating origin 38. |
| Spacecraft and the solar system | Psionic chapters 107, 109, 234 and 242; the `verse/` space modules as the starting game layer; Coax R12–R20 forms as they ship | Ephemerides and frames 39 → propagation with stated error 40 → burns 41 → rendezvous 42; heat 43, ship interiors 44 and delayed sensors 45 in any order → event-stopped time warp 46 → impacts 47 → three fidelities, batched rollouts, surrogates and an admitted environment 48. |
| Networking | 5 and 6 | Rooms and presence 49 → shared state 50. |
| Aethyrforge | 3, 5, 6 and 16 | Realm document 51 → editing 52 → assets 53 → publishing 54 → forks 55. |
| Realms | The chapters each realm uses | 56–63 in any order once their chapters exist; 64 last. |
| Sound | Coax only; 1, 2, 5 and 10 for playback in a realm | Oscillators and envelopes 65 → the beat grid 66 → scores and mixing 67 → playback, space and reaction 68 → voices 69 (also 49). |
| A living world | 1–16 and the chapters each system uses (water and vehicles 18–22; effects 16 and 23 for contrast; creatures 7 and 45) | 70–78 in any order once their dependencies settle. |
| Generating worlds | 51–55 | A piece from a sentence 79 → terrain and scatter 80 → replies 81. |
| Shipping realms | 3, 5 and 6 for saves; Coax R26–R27 for live changes; 16 and 37 for profiling; 11 and 15 for hosts and accessibility | 82–86 in any order. |

## Part VIII: sound

Sound comes last in the numbering but needs only Coax: chapters 65–67 build
and check offline synthesis from integers, so a score and its seed render the
same samples on every build and target. The part is numbered after Part VII
so that adding it renumbered no earlier chapter while Parts I–III were being
written. Chapter 68, playback inside a running realm, needs chapter 1's clock,
chapter 2's lens, chapter 5's replay and chapter 10's camera, and can start
once they settle.

## Parts IX–XI: the rest of a game engine

These parts cover the systems in the
[game-engine systems inventory](../../docs/research/game-engine-systems.md)
that the realms already use but that no earlier chapter owns. Each one has a
private copy in `verse/`, `coax/web/` or `apps/openagents-web/` today; each
chapter plans one Aethyr module that replaces it.

- Part IX moves world systems into Aethyr: water, sky and weather, particle
  effects, destruction, vehicles, creature behaviour, navigation, characters
  who talk, and authored cameras.
- Part X adds Aethyrforge's generation tools on top of Part VI's realm
  document: set pieces from a typed catalogue, seeded terrain and scatter,
  and realms made as replies.
- Part XI covers what a realm needs to ship: saves, live code changes,
  inspection and profiling, every host, and accessibility.

They are numbered after Part VIII so that adding them renumbered no earlier
chapter.

## Part IV: a physics engine

A GPU physics engine for realms, robotics and embodied learning, at rough
parity with [Genesis World 1.4.5](../../docs/research/genesis/genesis.md):
rigid and articulated bodies with contact, five material solvers and their
coupling, batched environments, selective gradients, sensors, observation
rendering, assets, replay and published measurement. The
[parity plan](../../psionic/docs/genesis-parity.md) maps each Genesis
capability to a chapter and a Coax phase and states where we deliberately
differ. The solvers are written on Psionic's tensor programs and compute
subset; Psionic's robot chapters (167–171) and world models use the engine.

Rigid work comes first: 17–22, then batching 31, assets 35, replay 36 and
Psionic's control chapter 167, which together can replace MuJoCo behind
Psionic chapter 170. The particle and mesh solvers 24–28 each need 23 (26
also needs 23's sort) and can follow in any order; coupling 29–30 needs them;
gradients 32 need 18–27; 33–34 need the scene and 35; 37 runs throughout. The
engine keeps the game-layer modules in `verse/` (rigid bodies, contacts) as
independent references, not as code to copy, and its Coax prerequisites are
R12–R20 plus the simulation-compute phases R21–R24.

## Part V: big worlds

Chapter 38 streams a world larger than memory and larger than f32 precision.
Chapters 39–48 build one ordinary model of spacecraft and the solar system, so
the same state serves a game, a training environment and an engineering
study. The planning input is the [physics and simulation design
reference](../../docs/research/expanse-physics-and-simulation-design-reference.md);
the chapters keep to generic physics and engineering and check against public
references: JPL [Horizons](https://ssd.jpl.nasa.gov/horizons/manual.html)
and NAIF [SPICE](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html),
NASA's [rocket equation](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/ideal-rocket-equation/),
[thermal control](https://www.nasa.gov/smallsat-institute/sst-soa/thermal-control/),
[trajectory optimization](https://ntrs.nasa.gov/citations/20230006127),
[occupant acceleration](https://www.nasa.gov/wp-content/uploads/2023/12/ochmo-tb-024-acceleration.pdf),
[artificial gravity](https://ntrs.nasa.gov/citations/19740010644) and
[hypervelocity shielding](https://hvit.jsc.nasa.gov/shield-development/).

The starting point already runs in `verse/` ([space primitives](../../docs/verse/space.md)):
f64 rigid bodies, swept contacts, gravity fields including a spinning drum,
thrusters with tanks, two-body Kepler orbits, a J2000 mean-element ephemeris,
spheres of influence and patched conics, Lambert and Hohmann transfers, RK4
finite burns and a floating-origin camera, used by Orbit League and Belt
2200. Those stay the game layer. Part V adds pinned oracles, stated error
budgets, separated truth and observation, batched rollouts, learned
surrogates and admitted environments. Rigid bodies, contact, batching and
replay come from Part IV (18–22, 31, 36) rather than a separate space engine.

Chapter 39 needs only Coax R12 and R13 forms and can start on plain f64
fields; 40 needs 39 and the R15 integrators. 41 builds on 40; 42 needs 40–41
and Psionic chapter 107. 43, 44 and 45 each need 41 (45 also 39) and can run
in any order. 46 collects their event functions and needs R16 replay; 47
needs 41, 43 and 44. 48 needs the rest, plus Psionic chapters 107, 109, 234
and 242. The frames, time scales, integrators, stamps, replay and environment
records are the same ones Psionic chapters 168–170 use, so a robot and a
spacecraft share one foundation. Engineering fidelity means agreement with
the named public references within stated error; it is a study profile, not
flight qualification, and no chapter commands real hardware. Hypothetical
propulsion, fictional rules and crew tolerances stay in declared, inspectable
modules and are never part of the engineering profile.

Chapters 49 and 50 put several people in one realm: presence first, then
shared state by lockstep or server authority, both replayable offline from
their input logs.

## Coax features each part needs

| Feature | Status | Chapters | Until it ships |
| --- | --- | --- | --- |
| Units and coordinate frames ([roadmap R12](../../coax/ROADMAP.md)) | scalar quantities shipped for native C ([quantity profile](../../coax/QUANTITIES.md)); points and frames planned | 2, 8, 18–22, 39–44 | Scalar quantities where native C allows; otherwise plain `f64`/`f32` fields with units stated in comments and one conversion function per space. |
| Typed clocks and multirate ticks (R13) | planned | 1, 4, 33, 39, 45, 46 | Integer step counters and an explicit fixed step. |
| Bounded control (R14) | planned | 9, Psionic 167 | Fixed arrays and no allocation by convention, checked by review. |
| Deterministic schedules and replay (R15–R16) | planned | 5, 20, 22, 23, 36, 46, 50 | Fixed iteration orders, digests per step and recomputation replay under one named profile. |
| Robot-model import (R17) | planned | 19, 35 | A checked URDF subset parsed by library code. |
| Stamped sensors and supervision (R18) | planned | 33, 45 | Stamp fields in plain records. |
| Parameter provenance and rule modules (R20) | planned | 17, 31, 48, 51 | Provenance recorded beside each tunable in data. |
| Logical tensors over layouts, compile budgets, device graphs and zero-copy buffers, kernel derivatives (R21–R24) | planned | 17–37 | An explicit environment dimension, library-level CUDA graph replay, leased DLPack views and hand-written reverse kernels checked against finite differences. |
| Direct WGSL graphics modules | shipped, bounded | 11–15 | — |

Real robot hardware, flight hardware and production multiplayer authority
are separately authorized later work; nothing in this book drives an
actuator.
