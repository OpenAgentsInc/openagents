# Building Aethyr in Coax

Aethyr is the engine the Verse's realms run on: the frame loop, spaces and
units, state, input, replay, entities, queries, the camera, the renderer,
physics, streaming, networking and the realm format. Aethyrforge is its realm
builder and world-creation tools. Both are written in
[Coax](../../coax/book/README.md); the model engine beside them is
[Psionic](../../psionic/book/README.md). The name and its history are in
[engine-name.md](../../docs/verse/engine-name.md).

This book builds the engine from scratch, starting with the smallest
primitive, one clock and one fixed step. Each chapter adds one working piece
with its own checks and builds only on earlier chapters, until Part VII
rebuilds every realm from them. Part VIII adds sound; it needs only Part I
and can be read at any point after it. Parts IX–XI plan the rest of a game engine:
water, weather, effects, creatures and other world systems; Aethyrforge's
generation tools; and what a realm needs to ship. You need the Coax language (its book's
Parts I–III) and, for Part IV onward, Psionic's tensor programs
([Psionic chapters 3–6 and 37–38](../../psionic/book/ch037-a-coax-compute-subset.md)).

The [plan](PLAN.md) gives the order, the prerequisites of each branch and the
Coax features each part waits for. The [contents](SUMMARY.md) list every
chapter.

## Status

A chapter whose status is *Unimplemented: design draft* is written ahead of
its code. Its first line under the title is a quoted
`**Status: unimplemented — design draft.**` marker. It says what exists today
(the private copies in `verse/` and `apps/openagents-web/` that the chapter
replaces), the method and design, what it builds in `aethyr/`, its acceptance
checks and oracles, and its dependencies. It has no source-linked listings or
examples; sketches in it are illustrative only.

Chapters 1–7, 10, 16 and 64 each belong to one Aethyr engine-core task
(GitHub #12609). The task's slug, such as `ec-loop`, is in the chapter's status
block, and the chapter is filled in as that module lands. A completed chapter
drops the marker and shows its code the way the Psionic book does: listings
copied from source between `// book:begin` and `// book:end` markers, and
runnable programs in `aethyr/examples/` with exact transcripts.

Chapters 1–4, 7 and 65–67 are written; every other chapter, through chapter 86, is a design draft.

## Writing style

Write chapters to the
[Google developer documentation style guide](https://developers.google.com/style):

- Use short, plain sentences in active voice and present tense.
- Address the reader as "you".
- Say what the code does and what was measured. Don't use metaphors,
  personification, or literary phrasing.
- Use numbered lists for steps and bulleted lists for related items.
- Use code font for code, file names, and identifiers.

## Checking the book

Commands run from the repository root.

```shell
python3 psionic/tests/book.py --book aethyr
python3 psionic/tests/book.py --book aethyr --coax coax/bin/coax
```

The first checks structure: chapter files and headings, the contents, local
links, status markers, one chapter per engine-core task, source listings
against their files and the example inventory in
[examples.json](examples.json). The second also runs every example in
`aethyr/examples/` in debug and release and compares its output and exit
status. It is the same checker as the Psionic book's; build the compiler once
with `coax/build.sh`.

Coax listings, equations and short illustrative blocks use `jai` fences;
commands use `shell`.

## Where Parts IV and V came from

The physics engine (Part IV) and the spacecraft and solar-system chapters
(Part V, chapters 39–48) were first planned in the Psionic book, as its Parts
XVIII and XX, and moved here on 2026-10-31. The Psionic book now points here
from the chapters that use them: robot control and policies run in Aethyr's
batched, differentiable worlds. The [move table](tools/move-from-psionic.tsv)
maps every old Psionic number to its new home, and
[the script](tools/move-from-psionic.py) that applied it rewrote the
references in both books and the docs that cite them.

| Psionic chapter | Aethyr chapter |
| --- | --- |
| 165–185 (Part XVIII, a physics engine) | 17–37 (Part IV) |
| 193–202 (Part XX, spacecraft and the solar system) | 39–48 (Part V) |
| 186–192 | stay in Psionic as 165–171 |
| 203–281 | stay in Psionic as 172–250 |

The [Genesis parity plan](../../psionic/docs/genesis-parity.md) maps each
capability of Genesis World 1.4.5 to a Part IV chapter.

## Chapters

### I. Foundations

| Chapter | Status | Result |
| --- | --- | --- |
| [1. A frame has a fixed step](ch001-a-frame-has-a-fixed-step.md) | Written (`ec-loop`) | One owner of time: a fixed 1/64 s step counted in whole nanoseconds, a frame clamp, at most eight steps a frame with the backlog dropped, and drawing interpolated between the last two steps, so physics is identical at 30, 60, 120 and 144 FPS and a jump at 60 FPS moves smoothly (7.0 mm against 76.2 mm). |
| [2. Spaces and units](ch002-spaces-and-units.md) | Written (`ec-space`; the per-realm feet check lands with `ec-migrate`) | Typed world, local, view, screen and grid points, crossing spaces only through named functions, with metres and grid units as `quantity` units; mixing them is a compile error, checked by six fixtures. |
| [3. State starts from its defaults](ch003-state-starts-from-its-defaults.md) | Written (`ec-state`) | Every realm's state built from the compiler's own initializers on native and browser hosts, checked byte for byte against native for 16 realm states, with the check proven sharp and a clock that can't freeze from zeroed memory. |
| [4. Input becomes typed actions](ch004-input-becomes-typed-actions.md) | Written (`ec-input`) | Keys, touch and pads become one typed `Intent` a frame with one run rule and dead zone; each press is taken by exactly one fixed step, checked against a separate model, and the keyboard and the touch stick drive the same session to the same states. |
| [5. Replay a session exactly](ch005-replay-a-session-exactly.md) | Unimplemented: design draft (`ec-replay`) | A session record (identities, seed, initial digest, one action sample per step, periodic digests) that replays bit-identically natively and in the browser, with counter-based random streams and bisection to the first divergent step. |

### II. A world

| Chapter | Status | Result |
| --- | --- | --- |
| [6. Entities are born and die](ch006-entities-are-born-and-die.md) | Unimplemented: design draft (`ec-entity`) | Generation-checked handles for the step and durable `EntityId`s for saves, networking and edits, with spawn and despawn deferred to a flush, so stale handles fail and despawning during iteration is safe. |
| [7. Ask the world where things are](ch007-ask-the-world-where-things-are.md) | Unimplemented: design draft (`ec-query`) | One query module (ground, raycast, overlap, sweep) over a realm's colliders, giving one meaning of "on the ground" shared by movement, camera, AI, picking and placement, with acceleration that never changes answers. |
| [8. Ground and collision](ch008-ground-and-collision.md) | Unimplemented: design draft | A heightfield ground exactly equal to the drawn surface, declared colliders, and kinematic capsule resolution with step, slope and pad rules, kept separate from rigid-body contact. |
| [9. A character moves](ch009-a-character-moves.md) | Unimplemented: design draft | The walking controller as an Aethyr module over typed actions and shared queries: steering at a fixed acceleration, a 1.17 m jump, standing, landing, falling and sliding in a fixed order, with no camera or clock of its own. |
| [10. The camera follows](ch010-the-camera-follows.md) | Unimplemented: design draft (`ec-camera`) | Follow, orbit, first-person and dive cameras as `Shot` producers that read the interpolated player state, ease once with frame-rate-independent exponentials, pull in through a read-only sweep and never step physics. |

### III. Seeing it

| Chapter | Status | Result |
| --- | --- | --- |
| [11. The WGSL renderer](ch011-the-wgsl-renderer.md) | Unimplemented: design draft | One renderer for every realm: it owns the pass graph and binding table; shaders are Coax procedures emitted as WGSL and SPIR-V and checked against their CPU run, retiring hand-written WGSL. |
| [12. Materials](ch012-materials.md) | Unimplemented: design draft | One typed material record and style table built from `verse/style.coax`, its presets and `look.coax` outlines; missing texture layers are refused at load, and the painterly goldens stay unchanged. |
| [13. Light and shadow](ch013-light-and-shadow.md) | Unimplemented: design draft | One lighting record per frame: key light from the clock, hemisphere ambient, a texel-snapped shadow map that doesn't shimmer, height fog, tone mapping and bounded local lights. |
| [14. Figures and animation](ch014-figures-and-animation.md) | Unimplemented: design draft | The Grid's cast, skinned rigs and rounded-bone figures share one pose pipeline: fades blend each joint locally, gaits advance by distance, motions that break joint limits are refused, and bone lengths stay constant. |
| [15. UI and the HUD](ch015-ui-and-the-hud.md) | Unimplemented: design draft | Safe-area layout in screen space and a UI tree whose hit rectangles equal its drawn ones; text from a stroke font, and taps become the same typed actions as keys. |
| [16. Budgets that refuse](ch016-budgets-that-refuse.md) | Unimplemented: design draft (`ec-budget`) | Content-hashed asset identity and declared per-realm budgets, taken whole or refused with a visible report, replacing today's silent slot clipping; an over-budget frame fails its test. |

### IV. Physics

| Chapter | Status | Result |
| --- | --- | --- |
| [17. A scene owns the state](ch017-a-scene-owns-the-state.md) | Unimplemented: design draft | Entities as shape, material and appearance; describe, build, step; solver arrays addressed by handle; a fixed recorded schedule; a published compatibility matrix that refuses unsupported combinations before allocation. |
| [18. Rigid bodies and time steps](ch018-rigid-bodies-and-time-steps.md) | Unimplemented: design draft | Newton–Euler bodies with quaternion attitude; explicit, semi-implicit and implicit-gyroscopic profiles with measured energy and momentum drift; `verse/rigid.coax` as the independent reference. |
| [19. Joints in generalized coordinates](ch019-joints-in-generalized-coordinates.md) | Unimplemented: design draft | Spatial algebra, forward kinematics, RNEA, CRBA and ABA kept as two forward-dynamics routes that must agree; revolute, prismatic, ball, free, fixed and screw joints; Panda values checked against frozen MuJoCo 3.3.1 recordings. |
| [20. Find the pairs that might touch](ch020-find-the-pairs-that-might-touch.md) | Unimplemented: design draft | AABBs, sweep and prune, a Morton-code BVH and all-pairs, all returning the same canonically sorted pair set; filters; the method chosen by recorded measurement. |
| [21. Find where they touch](ch021-find-where-they-touch.md) | Unimplemented: design draft | Support functions, GJK/EPA, MPR, closed forms and SAT, SDF queries, four-point manifolds with stable feature IDs, and time of impact; symmetric under argument swap. |
| [22. Contact as an optimization](ch022-contact-as-an-optimization.md) | Unimplemented: design draft | Contacts, limits and friction as one soft-constrained convex problem (MuJoCo's formulation); Newton with Cholesky and CG; warm start, islands; an independent KKT checker; sequential impulses kept as a second reference. |
| [23. Particles and their neighbors](ch023-particles-and-their-neighbors.md) | Unimplemented: design draft | Spatial hashing, counting sort and stable scatter, fixed-radius neighbors with reported overflow, and shape sampling; the shared substrate for 24–26. |
| [24. Liquids from particles](ch024-liquids-from-particles.md) | Unimplemented: design draft | WCSPH then DFSPH liquids with boundary particles; hydrostatic, dam-break and floating checks; gradients refused. |
| [25. Cloth and rope from constraints](ch025-cloth-and-rope-from-constraints.md) | Unimplemented: design draft | PBD and XPBD constraints for cloth, rope and soft particles; deterministic graph coloring; catenary and compliance checks; gradients refused until tested. |
| [26. Sand, snow and clay on a grid](ch026-sand-snow-and-clay-on-a-grid.md) | Unimplemented: design draft | MLS-MPM with APIC; neo-Hookean, snow, Drucker–Prager sand, clay and muscle; a deterministic block-sorted scatter beside the atomic profile; reverse-mode gradients via 32. |
| [27. Elastic solids on a mesh](ch027-elastic-solids-on-a-mesh.md) | Unimplemented: design draft | Tetrahedral FEM with stable neo-Hookean; explicit (differentiable) and implicit Newton–CG profiles; cantilever and inversion checks. |
| [28. Smoke on a grid](ch028-smoke-on-a-grid.md) | Unimplemented: design draft | Stable Fluids on a MAC grid: semi-Lagrangian advection, pressure projection by CG or multigrid, scalars and vorticity confinement; Taylor–Green decay check. |
| [29. Materials meet](ch029-materials-meet.md) | Unimplemented: design draft | Explicit coupling between solver pairs (rigid–SPH, rigid–MPM, rigid–PBD, rigid–FEM, MPM–SPH) with a zero-sum momentum ledger; unsupported pairs refused. |
| [30. Contact that holds](ch030-contact-that-holds.md) | Unimplemented: design draft | SAP rigid–FEM coupling and later IPC barriers in f64; a held grasp and intersection-free checks; refused on Metal and with gradients. |
| [31. Many worlds in one step](ch031-many-worlds-in-one-step.md) | Unimplemented: design draft | A batch dimension over every solver, per-environment parameters and randomization with provenance, isolated resets, counter-based seeds; batch invariance checked bit for bit; aggregate and per-environment rates reported together. |
| [32. Differentiate a simulation](ch032-differentiate-a-simulation.md) | Unimplemented: design draft | Reverse mode over steps with exact checkpointed recomputation; adjoints of implicit steps reusing the forward factorization; per-solver gradient modes enforced at build and compile time. |
| [33. Sensors read the simulation](ch033-sensors-read-the-simulation.md) | Unimplemented: design draft | Joint, IMU, force/torque, contact, tactile, lidar and depth sensors as read-only stages with rates, delays, noise and R18 stamps. |
| [34. Pictures for policies](ch034-pictures-for-policies.md) | Unimplemented: design draft | A batched observation rasterizer (RGB, depth, segmentation, normals) into device tensors read without copies; Verse stays the presentation renderer; image randomization. |
| [35. Bring in robots and meshes](ch035-bring-in-robots-and-meshes.md) | Unimplemented: design draft | URDF, MJCF and mesh import with inertia and watertightness validation, convex decomposition and tetrahedralization, content-addressed assets and per-field provenance. |
| [36. Record and replay a world](ch036-record-and-replay-a-world.md) | Unimplemented: design draft | Recordings with scene, profile, compiler and seed identities; recomputation replay checked against recorded digests, bisection to the first divergent step and stage. |
| [37. Measure the engine honestly](ch037-measure-the-engine-honestly.md) | Unimplemented: design draft | The compatibility matrix as data, the acceptance suite per backend and profile, fixed benchmark scenes, and claims that always include per-environment rates and compile and startup times. |

### V. Big worlds

| Chapter | Status | Result |
| --- | --- | --- |
| [38. Stream a large world](ch038-stream-a-large-world.md) | Unimplemented: design draft | The world in cells with load, simulate and draw rings and hysteresis, a floating origin in f64, and sleeping and waking at cell edges; replays are bit-identical whatever the load order. |
| [39. Ephemerides, time scales and frames](ch039-ephemerides-time-scales-and-frames.md) | Unimplemented: design draft | A pinned Horizons/SPK ephemeris with recorded scale, frame, centre, dataset and coverage; TAI/TT/TDB/UTC through a pinned leap-second table; full state transforms with the ω × r term; geometric, light-time and aberration-corrected states kept apart; `verse/orbit.coax`'s mean-element error measured. |
| [40. Propagate with a stated error](ch040-propagate-with-a-stated-error.md) | Unimplemented: design draft | Two-body, patched-conic and restricted n-body propagation on RK4, leapfrog, higher-order compositions and Dormand–Prince 5(4), each reporting error per observable with step count and cost; a Newtonian envelope that refuses, and constant proper acceleration as its own mode. |
| [41. Burns consume mass](ch041-burns-consume-mass.md) | Unimplemented: design draft | Variable-mass flight with separate energy, reaction-mass, coolant and consumable accounts, jet power P = F v_e / 2 as an admission check, off-centre thrust torque and inertia recomputed as tanks drain; Δv against the rocket equation and momentum conserved for ship plus exhaust. |
| [42. Reach a moving target](ch042-reach-a-moving-target.md) | Unimplemented: design draft | Rendezvous as a boundary-value problem: Lambert guesses, the t = 2√(D/a) benchmark, finite-thrust shooting and collocation under crew, reserve and exclusion constraints, and flybys whose energy change follows the encounter; infeasible requests name the binding constraint. |
| [43. Heat has to go somewhere](ch043-heat-has-to-go-somewhere.md) | Unimplemented: design draft | A lumped thermal network with radiators, finite sinks, coolant and failures, and propulsion heat only through declared hypothetical models; radiator equilibrium checked (49.03 GW at 1,000 K and ε = 0.9 needs about 0.961 km²). |
| [44. Inside a turning, accelerating ship](ch044-inside-a-turning-accelerating-ship.md) | Unimplemented: design draft | Interior frames with Coriolis, centrifugal and Euler terms or inertial integration (never both), spin habitats (0.3 g at 500 m is about 0.7325 rpm), foot attachment, compartment leaks, and a crew-load model whose parameters carry provenance. |
| [45. Sensors that see the past](ch045-sensors-that-see-the-past.md) | Unimplemented: design draft | Truth, emitted signal, received measurement and estimated track as separate data: light time (about 499.0 s at 1 AU), round trips, detection, and a track filter whose uncertainty grows; agents get only stamped observations, and a policy that reads truth is refused. |
| [46. Time warp that stops for every event](ch046-time-warp-that-stops-for-every-event.md) | Unimplemented: design draft | Time warp as scheduling: event functions, root finding on dense output and steps split at the earliest event, so warp 1 and 10⁶ give the same event order and final state; a 1,200 km/s projectile cannot pass a target; replay at omniscient and observation levels. |
| [47. Hits through layers](ch047-hits-through-layers.md) | Unimplemented: design draft | Hypervelocity hits as penetration through layers, using tables fitted to ballistic-limit equations that refuse outside their range, over a component graph, so a severed coolant line heats the drive and cuts thrust; large fragments become tracked bodies. |
| [48. One world at three fidelities](ch048-one-world-at-three-fidelities.md) | Unimplemented: design draft | Engineering, game and presentation profiles over one state, typed rule modules with declared footprints, provenance on every tunable, batched rollouts and bounded learned surrogates, the scenario admitted as an environment, and a ten-scenario regression suite at engineering fidelity. |
| [49. Rooms and presence](ch049-rooms-and-presence.md) | Unimplemented: design draft | Bounded, self-asserted presence frames keyed by stable entity IDs; remote players drawn a fixed delay behind, and only the nearest drawn as bodies, within the one-second two-session latency budget. |
| [50. Shared state over the network](ch050-shared-state-over-the-network.md) | Unimplemented: design draft | Lockstep (orders, commits, hash checks, snapshot repair) and server authority with prediction and rewind on one session layer; every networked session's input log replays offline to the same digest. |

### VI. Making worlds: Aethyrforge

| Chapter | Status | Result |
| --- | --- | --- |
| [51. A realm is a document](ch051-a-realm-is-a-document.md) | Unimplemented: design draft | A typed, versioned realm document (setup, catalogue features, wants, placements, tools) built from one catalogue table, laid out deterministically from document and seed, byte-identical natively and in the browser, with every asked thing kept as a feature or a `want` line. |
| [52. Edit a realm in place](ch052-edit-a-realm-in-place.md) | Unimplemented: design draft | Edits as invertible operations over the document with server-decided roles, proposals settled once, a replayable edit log, and the asks ledger's revert rule that leaves later changes alone. |
| [53. Bring in assets](ch053-bring-in-assets.md) | Unimplemented: design draft | One Coax import path for `VPK1` packs, cast and drops, with a licence on every asset, budgets that refuse, unit checks with recorded rescales, admit/upload/scan for uploads, and packs byte-identical to `pack_realm.py`. |
| [54. Publish a realm](ch054-publish-a-realm.md) | Unimplemented: design draft | A published realm as a frozen record of document, module and asset hashes, checked by `realms.ts`, goldens and the 60 FPS rule, with made realms never overwritten by a release. |
| [55. Fork a realm](ch055-fork-a-realm.md) | Unimplemented: design draft | Forks of any realm as a base version, declared rule switches and their own edit log, with rule-isolation checks, replay from the base, and the original kept byte-identical. |

### VII. Realms

| Chapter | Status | Result |
| --- | --- | --- |
| [56. The Grid](ch056-the-grid.md) | Unimplemented: design draft | The Grid rebuilt from the foundation chapters: a world-space floor that looks the same from any position, a smooth jump, feet at the physics height, and live people from the room. |
| [57. Made realms](ch057-made-realms.md) | Unimplemented: design draft | One made-realm player of chapter 51's document, with composition aimed at a first visit, four cameras over one controller, budgets, and dropped-model realms, all on Aethyr. |
| [58. Everglade](ch058-everglade.md) | Unimplemented: design draft | The forest city rebuilt from Aethyr's chapters: plan, streamed forest, people, the watch, quests, effects, edits and forks, holding `perf.ts`'s 60 FPS cases. |
| [59. Dead Gears](ch059-dead-gears.md) | Unimplemented: design draft | The ship survival game on Aethyr's entities and loop (its ECS becomes chapter 6's design), with every ported demo check passing and 200 creatures held to 2 ms a frame. |
| [60. Ice Cream Man](ch060-ice-cream-man.md) | Unimplemented: design draft | The seeded house game on Aethyr's loop, actions, replay and follow camera, with every recorded Rust trace still replaying fingerprint for fingerprint. |
| [61. Hecate's Cemetery](ch061-hecates-cemetery.md) | Unimplemented: design draft | The moonlit memorial garden rebuilt: the arrival flight handing the camera over smoothly, a dog that follows by the paths, the moon in its real phase, and scanned pet memorials. |
| [62. Fairy Forest](ch062-fairy-forest.md) | Unimplemented: design draft | The bloom-and-roots story on saved state, with flight moved from the face into the shared controller, so a full session replays and flies the same at any frame rate. |
| [63. The space games](ch063-the-space-games.md) | Unimplemented: design draft | Orbit League and Belt 2200 on Aethyr's physics and spacecraft chapters, with `space.md`'s checks, transfers within their stated error and lockstep games that replay to the same hashes. |
| [64. Every realm runs on Aethyr](ch064-every-realm-runs-on-aethyr.md) | Unimplemented: design draft (`ec-migrate`) | The `ec-migrate` plan: an inventory of each realm's private copies, a migration order, a check that no private loop, camera, input path or ground logic comes back, and goldens and 60 FPS held throughout. |

### VIII. Hearing it

| Chapter | Status | Result |
| --- | --- | --- |
| [65. Sound from numbers: oscillators and envelopes](ch065-sound-from-numbers.md) | Written | Integer oscillators (saw, square, a fitted sine), seeded xorshift noise, ADSR envelopes, decays as a repeated multiply, a one-pole filter, constant-power pan and Padé saturation, with no `libm` and no clock. |
| [66. A sequencer on the beat grid](ch066-a-sequencer-on-the-beat-grid.md) | Written | A tempo grid in ticks whose every step is computed from its own tick, so it never drifts; sections snapped to steps; keys and triads from one voicing rule; patterns stepped to exact samples. |
| [67. Scores, mixing and a soundtrack](ch067-scores-mixing-and-a-soundtrack.md) | Written | Stereo buses with a Schroeder reverb send, gain automation, a master that normalizes, automates and saturates without clipping, WAV files, a sample hash, and the `promo-epic` score rendered by `aethyr/tools/score.coax`. |
| [68. Sound in the world: playback, space and reaction](ch068-sound-in-the-world.md) | Unimplemented: design draft | Block rendering on the audio clock through a Web Audio worklet and native hosts, sources placed in the world by distance, pan and delay, and music that changes section on the bar line with the game's state, all replaying exactly. |
| [69. Voices in a realm](ch069-voices-in-a-realm.md) | Unimplemented: design draft | Players, NPCs and agents heard from where they stand: one `Speaker`, `Utterance` and `Route` model, a forwarder that picks who hears whom from positions, zones, consent and floors without decoding audio, and voices placed by chapter 68's gain, pan and delay. |

### IX. A living world

| Chapter | Status | Result |
| --- | --- | --- |
| [70. Rivers, ponds and the sea](ch070-rivers-ponds-and-the-sea.md) | Unimplemented: design draft | Water from `verse/water*.coax` (W1–W11) as one module: typed water bodies, a wave surface fixed to the tick, buoyancy and drag, the spectral sea and the underwater view, with the same gameplay surface on every client and the water lab held to 1 ms of GPU on a phone and 3 ms on a desktop. |
| [71. Sky, weather and the time of day](ch071-sky-weather-and-the-time-of-day.md) | Unimplemented: design draft | One day clock (`real`, `cycle` or `fixed:T`) drives the sun, the moon's real phase, palettes, the starfield, a seeded weather schedule and ground wetness, so every visitor and replay sees the same sky. |
| [72. Effects made of particles](ch072-effects-made-of-particles.md) | Unimplemented: design draft | Stateless visual particles (embers, dust, fireflies) computed from emitter, index and clock, plus fire and smoke, under declared budgets of 24 emitters with 64 particles each, kept separate from chapter 23's simulated particles. |
| [73. Things that break](ch073-things-that-break.md) | Unimplemented: design draft | `verse/demolish.coax` on Part IV's bodies and contact: Voronoi fracture into 2–24 chunks, a support graph that topples pieces, budgets for chunks and loose pieces, regrowth, and the destructible-wall integration test. |
| [74. Wheels on the ground](ch074-wheels-on-the-ground.md) | Unimplemented: design draft | Orbit League's raycast vehicle on Aethyr's queries and rigid bodies: spring-damper suspension, tyres inside the friction circle, steering that fades with speed, game rules declared as rules, and a recorded match that replays to the same digests. |
| [75. Creatures that follow and watch](ch075-creatures-that-follow-and-watch.md) | Unimplemented: design draft | Creatures sense, decide and act at a fixed step from typed sight and hearing stimuli kept apart from truth; the flock becomes a general follow, the watch's state machines and seeded dice replay, and a remote decision model picks only from a fixed enum. |
| [76. Find a way through](ch076-find-a-way-through.md) | Unimplemented: design draft | One navigation module for the three path systems: path graphs with an off-path cost, Dijkstra with fixed tie-breaks, shared flow fields on a 0.5 m grid, avoidance, replanning only where an edit touches, and a typed result for an unreachable target. |
| [77. People who talk back](ch077-people-who-talk-back.md) | Unimplemented: design draft | One conversation path for every realm's characters: bounded requests, per-player memory, scripted fallback lines when the model fails, answers that never change game state, and replays that don't call the model. |
| [78. Cinematics and close-ups](ch078-cinematics-and-close-ups.md) | Unimplemented: design draft | Authored flights, close-ups for pointing at an object, and clips recorded from replays, all as chapter 10 `Shot` producers sharing one blend with a per-frame bound. |

### X. Aethyrforge: generating worlds

| Chapter | Status | Result |
| --- | --- | --- |
| [79. A piece from a sentence](ch079-a-piece-from-a-sentence.md) | Unimplemented: design draft | One catalogue and one typed selector for the Sandbox's set pieces and Aethyrforge's board: the answer is checked against the catalogue on the server and in the game, laid out from a seed as one undo step, and every asked thing is kept. |
| [80. Terrain and scatter from a seed](ch080-terrain-and-scatter-from-a-seed.md) | Unimplemented: design draft | Heightfields equal to the drawn surface, levelled paths and flats, jittered scatter with exclusion zones, and one counter-based random stream per feature, so content is the same in any streaming load order. |
| [81. Reply with a realm](ch081-reply-with-a-realm.md) | Unimplemented: design draft | Everglade's reply as a typed Aethyrforge record (realm, place, kind, words up to 280 characters, seed) that any realm can offer, that replays from its seed for every viewer, and whose link is never fetched. |

### XI. Shipping realms

| Chapter | Status | Result |
| --- | --- | --- |
| [82. Save and come back](ch082-save-and-come-back.md) | Unimplemented: design draft | One save layer over today's `AESV` container with a CRC-32 and two slots: only declared fields are saved, older schemas migrate forward, a newer schema is refused, and the newer of the browser and server copies wins. |
| [83. Change code while it runs](ch083-change-code-while-it-runs.md) | Unimplemented: design draft | Live iteration in four tiers (data, shaders, logic, layout changes) with a save-to-frame target for each; layout fingerprints decide whether to swap, migrate or reset, a broken edit keeps the old module running, and production never loads code. |
| [84. Inspect and profile a running realm](ch084-inspect-and-profile-a-running-realm.md) | Unimplemented: design draft | The same frame report, overlay, probes and goldens for every realm, the 60 FPS rule (95th percentile under 17.5 ms, no frame over 33 ms) extended to every listed realm, separate from chapter 37's physics measurements. |
| [85. One realm on every host](ch085-one-realm-on-every-host.md) | Unimplemented: design draft | One versioned host contract and lifecycle events (resize, blur, hidden, shown, leave) on every host, byte-for-byte parity checks for every realm, and the native-GPU phases G0–G6. |
| [86. Everyone can play](ch086-everyone-can-play.md) | Unimplemented: design draft | Action remapping, a time-scale setting, reduced motion, text scale up to 1.5, captions on sounds that carry information, and stable text IDs checked by pseudolocalization. |
