# Plan and prerequisites

The [chapter registry](chapters.json) records each chapter's status, qualified
profile and minimum teaching prerequisites. Chapter numbers give the reading
order; the dependency graph gives the construction order, including independent
branches. A chapter can introduce a small profile before a later chapter adds
imports, coupling or another backend. Those later integrations remain part of
the chapter's stated completion checks. The [contents](SUMMARY.md) give the
reading order and the [introduction](README.md) the result of each chapter.

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
| X. Aethyrforge: generating worlds | 79–80 | A set piece chosen from a typed catalogue by a semantic selector, and terrain and scatter laid out from a seed. |
| XI. Shipping realms | 81–85 | Saves that survive schema changes, code changed while a realm runs, inspection and profiling of a running realm, one realm on every host, and accessibility and languages. |

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

## Completion milestones

The registry currently has 14 implemented chapters (1–11 and 65–67),
ten partly implemented chapters (15, 16, 51, 52, 54–56, 69, 72 and 81), and 61 design drafts. These counts
describe teaching profiles, not a percentage of the engine completed. Existing
realm code can already provide a feature whose shared Aethyr module is a draft.

1. **Complete one playable route.** Chapter 11 qualifies the shared renderer
   and a recorded Grid CPU/WebGPU/Vulkan teaching profile. Build chapters
   12–15 on those Coax-generated shader and host paths, preserving their draw ABI and
   recorded parity. Finish chapter 16's content-hashed asset admission. Take
   the Grid through one shared frame, material, pose and HUD contract; retain
   a native/Wasm session replay, its golden and its frame-time checks.
2. **Converge the realms and hosts.** Migrate each realm's remaining input,
   entities, spatial adapters and camera modes, then delete the replaced
   copies. Chapter 64 needs a replay per realm and a check against private
   copies returning. Its inventory records Ice Cream Man's remaining
   `ice_play_frame` clamp/accumulator as an open conformance gap against the
   [one-clock invariant](../../INVARIANTS.md#a-realms-frame-time-is-what-its-clock-counts-and-no-frame-runs-past-8-steps),
   to remove without changing the rule traces. Add chapter 68's live
   synthesized and spatial sound to
   the existing offline audio. Bring saves, host lifecycle and accessibility
   (81, 84 and 85) into this route as their dependencies become available.
   Consolidating current gameplay profiles does not complete the advanced
   realm rebuilds that require later physics or spacecraft chapters.
3. **Make Forge publications reproducible.** Complete the typed document
   (51), one edit log for every operation (52), admitted assets (53), a
   publication that pins document, engine build and asset hashes (54), and
   isolated forks (55). Existing revision publication is the starting point;
   it does not yet freeze the engine build. This route does not require the
   spacecraft fidelity ladder or the full multi-material physics engine.
4. **Qualify a rigid simulation profile.** Build the scene and rigid chain
   (17–22), then rigid batching (31), asset import (35), replay (36), and a
   Psionic control/environment consumer (167–171). Start chapter 37's
   compatibility ledger with the first admitted profile. Add sensors (33)
   and observation images (34) on this bounded profile before extending them
   to other materials.
5. **Extend the qualified profiles.** Add particle and mesh solvers (23–28),
   coupling (29–30), selective gradients (32), and their batch, import and
   observation integrations. Build streaming and the spacecraft fidelity
   ladder (38–48) against pinned oracles and stated error. Consolidate the
   remaining world systems, generation tools and shipping capabilities as
   their prerequisites settle. Genesis parity is a separate, larger result
   than a portable playable realm.

Each milestone retains the chapter-promotion requirements in the
[introduction](README.md#promoting-a-draft-to-a-teaching-chapter). A passing
example or a working product path alone does not complete the chapter's full
declared profile.

## Minimum prerequisites and later integrations

`depends_on` records what a chapter's first teaching profile consumes. It is
not an inventory of every chapter that eventually uses or extends the result.
The following distinctions keep the graph acyclic without dropping the later
acceptance work:

| Chapter | Minimum prerequisite decision | Later integration retained |
| --- | --- | --- |
| 5, replay | 1, 3 and 4 supply a clock, constructed state and actions; 6 is not needed to record a fixed-state realm. | Once entities exist, include chapter 6's `entity_digest` in the realm digest. Chapter 6 can still use chapter 5 to check a store's replay. |
| 7, queries | 2 and 6 provide spaces and durable collider identity; remove 20 and 21 as prerequisites. | Physics broad/narrow phases must agree with these queries on shared shapes. |
| 18, free bodies | 17 supplies the scene; remove the reverse dependency on 19. | Chapter 19 extends the free-body integrator to articulations. |
| 21, narrow phase | 18 and 20 supply poses and pairs. Authored primitive, convex-hull and SDF fixtures do not need an importer or a reverse pass. | Chapter 35 supplies decomposed/baked assets; chapter 32 consumes the differentiable contact path. Both remain required for those extended profiles. |
| 22, contact solve | 19–21 supply mass matrices, pairs and manifolds; remove 32 as a prerequisite. | Chapter 32 differentiates the forward factorization. |
| 24, liquids | 23 supplies particles and neighbours for a fluid with prescribed boundaries; remove 29 and 34 as prerequisites. | Two-way floating-body checks require 29; surface observations require 34. |
| 25, cloth | 21 and 23 supply prescribed collision queries and particles; author the first constraint mesh directly. | 29 supplies two-way coupling and 35 imports meshes. |
| 26, MPM | 23 supplies particle sorting; qualify the forward solver first. | 29 adds coupling and 32 adds gradients; neither is a prerequisite for the forward calculation. |
| 27, FEM | 17 supplies scene arrays; use a directly authored tetrahedral fixture first. | 35 generates/imports meshes, 29–30 add contact and 32 adds gradients. |
| 31, batching | 17–22 suffice for the first rigid batched profile; remove 23–30 as prerequisites. | Every later material/coupler must pass batch invariance before its matrix cell is admitted. |
| 32, gradients | Add the forward solvers of 18–27 as prerequisites, as its chapter already states. | Unsupported solver/coupler modes are refused; a later gradient profile needs its own qualification. |
| 33, sensors | 17–22 suffice for the stated joint, IMU, rigid-contact and primitive-ray checks; remove 23–28 as prerequisites. | Material-field sensors, such as chapter 28's scalar fields, extend the sensor matrix later. |
| 35, import | 17 supplies the scene format and 19 the link/joint model. Chapters 18, 20–34 are not additional direct prerequisites for parsing and validating that data. | Those solvers, batches, gradients, sensors and observation renderers consume admitted assets and need their own integration checks. Chapter 34 retains its dependency on 35 for meshes/textures. |
| 50, network sessions | 1, 4, 5, 6 and 49 supply ticks, actions, replay, identity and transport; remove 18–22 and 36 as prerequisites. | Physics-bearing sessions require those Part IV solvers and world replay; ordinary deterministic game sessions do not. |
| 51, realm document | 3, 6 and 16 suffice for defaults, identity and admission; remove 17 as a prerequisite. | An eventual physics-bearing realm maps the document into chapter 17's scene. |
| 55, forks | 5, 52 and 54 supply replay, edits and a frozen base; remove 48 as a prerequisite. | Chapter 48's fidelity rules share the rule-module idea; spacecraft-profile forks additionally need that profile. |

Chapter 37's ledger and measurement method can start before the solvers. Its
complete matrix and benchmark corpus depend on the admitted profiles of
17–36; an empty ledger is not completed engine measurement. Similarly, a
chapter may refer forward to an integration without requiring it to construct
its first example. Read the integration checks before claiming the full goal.

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
| Aethyrforge | 3, 5, 6 and 16; renderer/UI for the editor; 37's measurement method for publication gates | Realm document 51 → editing 52 → assets 53 → publishing 54 → forks 55. Physics-scene mapping is an extension. |
| Realms | The chapters each realm uses | 56–63 in any order once their chapters exist; 64 last. |
| Sound | Coax only; 1, 2, 5 and 10 for playback in a realm | Oscillators and envelopes 65 → the beat grid 66 → scores and mixing 67 → playback, space and reaction 68 → voices 69 (also 49). |
| A living world | 1–16 and the chapters each system uses (water and vehicles 18–22; effects 16 and 23 for contrast; creatures 7 and 45) | 70–78 in any order once their dependencies settle. |
| Generating worlds | 51–55 | A piece from a sentence 79 → terrain and scatter 80. |
| Shipping realms | 3, 5 and 6 for saves; Coax R26–R27 for live changes; 16 and 37 for profiling; 11 and 15 for hosts and accessibility | 81–85 in any order. |

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

Rigid work comes first: 17–22, then rigid batching 31, assets 35, replay 36 and
Psionic's control chapter 167, which together can replace MuJoCo behind
Psionic chapter 170. Liquids 24, cloth 25 and MPM 26 need chapter 23's
particles (26 also needs its sort). FEM 27 starts from scene arrays and an
authored tetrahedral mesh; grid fluid 28 needs chapter 21's SDFs and sparse
solvers. Coupling 29–30 needs the forward solvers;
gradients 32 need 18–27. Rigid sensors 33 need 17–22; observation rendering
34 adds batching and imported assets. Extend each profile's matrix only after
its integration checks pass; 37 runs throughout. The
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
| Units and coordinate frames ([roadmap R12](../../coax/ROADMAP.md)) | native scalar quantities ([quantity profile](../../coax/QUANTITIES.md)) and an initial native affine point/vector profile ([spatial profile](../../coax/SPATIAL.md)) shipped; full transforms and realizations remain open | 2, 8, 18–22, 39–44 | Use the admitted native profiles; other targets and full state transforms keep explicit library boundaries and stated units. |
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
