# Building the OpenAgents Metaverse Engine

The OpenAgents Metaverse Engine is the engine the OpenAgents Metaverse's realms run on: the frame loop, spaces and
units, state, input, replay, entities, queries, the camera, the renderer,
physics, streaming, networking, the realm format, and the tools that create
and edit realms. It is written in
[Coax](../../coax/book/README.md); the model engine beside them is
[Psionic](../../psionic/book/README.md). The name and its history are in
[engine-name.md](../../docs/verse/engine-name.md).

This book builds the engine from scratch, starting with the smallest
primitive, one clock and one fixed step. Each chapter adds one working piece
with its own checks and builds only on earlier chapters, until Part VII
rebuilds every realm from them. Part VIII adds sound; it needs only Part I
and can be read at any point after it. Parts IX–XI plan the rest of a game engine:
water, weather, effects, creatures and other world systems; the Metaverse Engine's
generation tools; and what a realm needs to ship. Part XII builds a multiplayer
engine, from one headless server to the full Everglade room. Part XIII builds
the action pipeline that every class and monster uses: ask, act, present. You need the Coax language (its book's
Parts I–III) and, for Part IV onward, Psionic's tensor programs
([Psionic chapters 3–6 and 37–38](../../psionic/book/ch037-a-coax-compute-subset.md)).

The [plan](PLAN.md) gives the order, the prerequisites of each branch and the
Coax features each part waits for. The [contents](SUMMARY.md) list every
chapter.

## Start here

- **What you build.** A real-time engine, one checked piece per chapter,
  and then the realms that run on it.
- **What you need.** The Coax book's Parts I–III. Part IV onward also needs
  Psionic chapters 3–6, 37 and 38.
- **The core path.** To make a playable, shared realm, read Parts I–III
  (chapters 1–17), then chapters 51–55 (a realm as a document, published
  and forked), then Part XII (chapters 86–101, many players) and
  [chapter 102](ch102-ask-act-present.md) (actions). Parts IV and V, physics
  and spacecraft, are an advanced branch. You don't need them to make a
  game.
- **See it used.** [Building Everglade](../../everglade/book/README.md)
  builds one game on this engine, and links back to the chapter each piece
  comes from.
- **Where this book sits.** See [the OpenAgents books](../../docs/books/README.md) for the whole library, the shared
  method and the [glossary](../../docs/books/glossary.md).

## Status

A chapter whose status is *Unimplemented: design draft* is written ahead of
its code. Its first line under the title is a quoted
`**Status: unimplemented — design draft.**` marker. It says what exists today
(the private copies in `verse/` and `apps/openagents-web/` that the chapter
replaces), the method and design, what it builds in `aethyr/`, its acceptance
checks and oracles, and its dependencies. It has no source-linked listings or
examples; sketches in it are illustrative only.

Chapters 1–7, 10, 16 and 64 each belong to one Metaverse Engine engine-core task
(GitHub #12609). The task's slug, such as `ec-loop`, is in the chapter's status
block, and the chapter is filled in as that module lands. A completed chapter
drops the marker and shows its code the way the Psionic book does: listings
copied from source between `// book:begin` and `// book:end` markers, and
runnable programs in `aethyr/examples/` with exact transcripts.

The [chapter registry](chapters.json) records each chapter's status (one of
*implemented*, *partly implemented*, *design draft* or *planned*), its
profile, its dependencies and three separate facts: its implemented source
(`source`), the targets with recorded evidence for its profile
(`qualified_targets`), and the code outside the book that uses it today
(`adopted_by`). Dependencies mean the minimum teaching prerequisites;
[the plan](PLAN.md#minimum-prerequisites-and-later-integrations) also records
the later integrations needed for each full goal. The status table below, the contents and each chapter's status
marker must agree with it; `python3 psionic/tests/book.py --book aethyr` fails
when they don't. When an engine-core task lands its chapter, update the
chapter's registry entry, contents label and table row in the same commit.

The registry records 14 implemented chapters, 55 partly implemented chapters
and 33 design drafts. An implemented chapter is
qualified for its named profile; it does not imply that every realm adopted
it or every backend is supported. For example, chapter 8 qualifies native
kinematic contacts, not the rigid solver planned in Part IV. Chapters 65–67
render offline audio; chapter 68 still owns live playback. Existing realm
features and the native shader port can be working while their consolidation
into shared Metaverse Engine teaching modules remains a draft.

## Promoting a draft to a teaching chapter

A design draft or planned chapter becomes a teaching chapter only when it has
all four of the following:

1. **A worked calculation or trace.** A small input whose result the chapter
   works out by hand, step by step, before the program runs.
2. **Runnable source.** The code and an example in `aethyr/examples/`, listed in [examples.json](examples.json), with an exact
   transcript that the book's checks compare in debug and release.
3. **A deliberately broken case and its diagnostic.** An input or an edit
   that the code refuses, with the exact error or refusal shown and its exit
   status checked.
4. **An independent check.** A result computed another way, such as a
   separate oracle, a reference implementation or a hand-derived value, that
   the checks compare with the code's result.

When all four exist, update the chapter's status in the same commit in four
places: the chapter's status marker, its entry in [chapters.json](chapters.json),
its label in the [contents](SUMMARY.md) and its row in the table below. The registry
check fails when they disagree. [Coax chapter 29](../../coax/book/ch29-units-and-frames.md)'s
affine section is an example: a hand calculation, `examples/ch29_frames.coax`,
a refused program with `E0834`, and a `#test` that compares the result with
plain arithmetic.

For a route through the four books, see
[Paths through the books](../../docs/books/paths.md). The
[completion plan](../../docs/books/completion.md) coordinates their shared
milestones; the Metaverse Engine's [completion milestones](PLAN.md#completion-milestones)
separate the playable-realm, Forge and advanced simulation goals.

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
from the chapters that use them: robot control and policies run in the Metaverse Engine's
batched, differentiable worlds. The [move table](tools/move-from-psionic.tsv)
maps every old Psionic number to its new home, and
[the script](tools/move-from-psionic.py) that applied it rewrote the
references in both books and the docs that cite them.

| Psionic chapter | Metaverse Engine chapter |
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
| [1. A frame has a fixed step](ch001-a-frame-has-a-fixed-step.md) | Runnable: native C (`ec-loop`) | One owner of time: a fixed 1/64 s step counted in whole nanoseconds, a frame clamp, at most eight steps a frame with the backlog dropped, and drawing interpolated between the last two steps, so physics is identical at 30, 60, 120 and 144 FPS and a jump at 60 FPS moves smoothly (7.0 mm against 76.2 mm). |
| [2. Spaces and units](ch002-spaces-and-units.md) | Runnable: native f32 spatial boundaries (`ec-space`) | Typed world, local, view, screen and grid points, explicit conversions and metre/grid quantities; six compiler fixtures and nine native avatar-renderer paths check the interpolated world anchor, with four wrong producer controls per path. |
| [3. State starts from its defaults](ch003-state-starts-from-its-defaults.md) | Runnable: native C (`ec-state`) | Every realm's state built from the compiler's own initializers on native and browser hosts, checked byte for byte against native for 16 realm states, with the check proven sharp and a clock that can't freeze from zeroed memory. |
| [4. Input becomes typed actions](ch004-input-becomes-typed-actions.md) | Runnable: typed actions and V2 buffered realm controls, native C and Wasm (`ec-input`) | Keys, touch and pads become one typed `Intent` a frame with one run rule and dead zone; each press is taken by exactly one fixed step, checked against a separate model, and the keyboard and the touch stick drive the same session to the same states. |
| [5. Replay a session exactly](ch005-replay-a-session-exactly.md) | Runnable: versioned accepted-event exchange and bounded guard traces, native C and Wasm (`ec-replay`) | One envelope for every accepted external event (controls, model answers, network updates) with its apply tick, source, request and epoch; a gate that refuses duplicate, stale and unknown events; a record that starts with the constructed state's digest and replays with no remote calls; a digest every step that finds the first divergent step; counter-based random streams; and strict sine, cosine and powers, so a recorded walk gives the same session digest natively and in wasm. |

### II. A world

| Chapter | Status | Result |
| --- | --- | --- |
| [6. Entities are born and die](ch006-entities-are-born-and-die.md) | Runnable: native C (`ec-entity`) | Generation-checked handles for the step and durable `EntityId`s for saves, networking and edits, with spawn and despawn deferred to a flush, so stale handles fail and despawning during iteration is safe. |
| [7. Ask the world where things are](ch007-ask-the-world-where-things-are.md) | Runnable: native C and Wasm semantic query fixture | One query module (ground, raycast, overlap, sweep) over a realm's colliders, giving one meaning of "on the ground" shared by movement, camera, AI, picking and placement, with acceleration that never changes answers. |
| [8. Ground and collision](ch008-ground-and-collision.md) | Runnable: native f32 meshes and capsule contacts | Drawn-triangle ground, bounded heightfield rays and sphere contacts, checked capsule publication and declared realm profiles; a raised-terrain walk, jump and landing. |
| [9. A character moves](ch009-a-character-moves.md) | Runnable: native and Wasm fixed-step walking | The shared controller with declared tuning, compatible C facade and fetching dogs on its actual fixed steps and interpolation; worked run/jump calculations, atomic refusal, replay and independent native/Wasm scene traces. |
| [10. The camera follows](ch010-the-camera-follows.md) | Runnable: native C (`ec-camera`) | One follow camera: a spring that never overshoots, room from the colliders (pulled in at once, backed out by a closed form, the same 4.394547 m after 0.5 s for any step size) and the eye drawn between steps, never outside a wall; one `CameraShot` from every camera (third person, first person, top down, the free orbit and the dive); and a camera that steps after the player, so the player is the same bit for bit with the camera stepped every step, never or twice a step. |

### III. Seeing it

| Chapter | Status | Result |
| --- | --- | --- |
| [11. The WGSL renderer](ch011-the-wgsl-renderer.md) | Runnable: shared renderer; Grid CPU/WebGPU/Vulkan teaching profile | Shared frame, geometry, bindings, pass and resource admission; checked shader emission and bounded quad CPU execution. The moving Grid's recorded frame has strict CPU/GPU parity. Catalogue, host and golden evidence keep their separate scopes; Everglade's complete CPU image and Dead Gears' boundary packets remain unqualified. |
| [12. Materials](ch012-materials.md) | Partly implemented: shared shading functions and material admission; CPU (no realm draws from a material table yet) | The surface shading functions moved into `aethyr/render/material.coax` with every realm's WGSL byte-identical; a typed material record admitted whole or refused at load (missing texture layer, undrawn style); the CPU banded surface equals the fixture's fragment code bit for bit. |
| [13. Light and shadow](ch013-light-and-shadow.md) | Partly implemented: shared light functions, the key by clock, the snapped shadow space and local lights; CPU (realms still build their own light in their shaders) | Ambient, the shadow map's snap and place, contact shadow and haze moved into `aethyr/render/light.coax` with every realm's WGSL byte-identical; the key light by clock equals the realms' uniform block bit for bit; the map's middle moves in whole texels; at most 16 local lights, refused whole past it. |
| [14. Figures and animation](ch014-figures-and-animation.md) | Partly implemented: the shared pose for skinned rigs; CPU (the Grid's cast and the rounded-bone figures still pose their own joints) | One joint's local blend, the root-first chain and the palette rows moved into `aethyr/anim/pose.coax`; `aethyr/anim/skin.coax` plays through them with the Ranger's 180-frame palette digest unchanged; bones stay within their two poses' lengths through every fade; a motion past a joint limit is refused with its line and range. |
| [15. UI and the HUD](ch015-ui-and-the-hud.md) | Partly implemented: native C and browser WebGPU (visual foundations and the UI tree; realms on the tree, the platform mirror and the journey still to do) | Checked HUD geometry, 18 Coax drawing components and a semantic UI tree (`aethyr/ui/tree.coax`): roles, labels and actions, unlabelled controls refused, focus in reading order trapped by dialogs and never lost, and taps and keys giving the same action from the drawn rectangles. |
| [16. Budgets that refuse](ch016-budgets-that-refuse.md) | Partly implemented: native frame budgets; native/Wasm resource-cost companion fixture (`ec-budget`) | Declared frame budgets in every realm; source-derived encoded, decoded, mip and scratch costs; versioned dependency manifests and named admission around existing tickets. Actual large-world allocation/streaming qualification remains open. |

### IV. Physics

| Chapter | Status | Result |
| --- | --- | --- |
| [17. A scene owns the state](ch017-a-scene-owns-the-state.md) | Partly implemented: CPU, f64 (the scene container with two small solvers) | Entities as shape, material and appearance; describe, build, step; solver arrays addressed by handle; a fixed recorded schedule; a published compatibility matrix that refuses unsupported combinations before allocation. |
| [18. Rigid bodies and time steps](ch018-rigid-bodies-and-time-steps.md) | Partly implemented: CPU, f64 (three free-body integrator profiles) | Newton–Euler bodies with quaternion attitude; explicit, semi-implicit and implicit-gyroscopic profiles with measured energy and momentum drift; `aethyr/physics/rigid.coax` as the independent reference. |
| [19. Joints in generalized coordinates](ch019-joints-in-generalized-coordinates.md) | Unimplemented: design draft | Spatial algebra, forward kinematics, RNEA, CRBA and ABA kept as two forward-dynamics routes that must agree; revolute, prismatic, ball, free, fixed and screw joints; Panda values checked against frozen MuJoCo 3.3.1 recordings. |
| [20. Find the pairs that might touch](ch020-find-the-pairs-that-might-touch.md) | Partly implemented: CPU, f64 (three broad-phase methods) | AABBs, sweep and prune, a Morton-code BVH and all-pairs, all returning the same canonically sorted pair set; filters; the method chosen by recorded measurement. |
| [21. Find where they touch](ch021-find-where-they-touch.md) | Partly implemented: CPU, f64 (GJK, EPA, SAT and closed forms) | Support functions, GJK/EPA, MPR, closed forms and SAT, SDF queries, four-point manifolds with stable feature IDs, and time of impact; symmetric under argument swap. |
| [22. Contact as an optimization](ch022-contact-as-an-optimization.md) | Unimplemented: design draft | Contacts, limits and friction as one soft-constrained convex problem (MuJoCo's formulation); Newton with Cholesky and CG; warm start, islands; an independent KKT checker; sequential impulses kept as a second reference. |
| [23. Particles and their neighbors](ch023-particles-and-their-neighbors.md) | Partly implemented: CPU, f64 (particle store and cell-list neighbors) | Spatial hashing, counting sort and stable scatter, fixed-radius neighbors with reported overflow, and shape sampling; the shared substrate for 24–26. |
| [24. Liquids from particles](ch024-liquids-from-particles.md) | Partly implemented: CPU, f64 (weakly compressible SPH) | WCSPH then DFSPH liquids with boundary particles; hydrostatic, dam-break and floating checks; gradients refused. |
| [25. Cloth and rope from constraints](ch025-cloth-and-rope-from-constraints.md) | Partly implemented: CPU, f64 (XPBD distance constraints, two projection profiles) | PBD and XPBD constraints for cloth, rope and soft particles; deterministic graph coloring; catenary and compliance checks; gradients refused until tested. |
| [26. Sand, snow and clay on a grid](ch026-sand-snow-and-clay-on-a-grid.md) | Partly implemented: CPU, f64, 2D (MLS-MPM with APIC) | MLS-MPM with APIC; neo-Hookean, snow, Drucker–Prager sand, clay and muscle; a deterministic block-sorted scatter beside the atomic profile; reverse-mode gradients via 32. |
| [27. Elastic solids on a mesh](ch027-elastic-solids-on-a-mesh.md) | Unimplemented: design draft | Tetrahedral FEM with stable neo-Hookean; explicit (differentiable) and implicit Newton–CG profiles; cantilever and inversion checks. |
| [28. Smoke on a grid](ch028-smoke-on-a-grid.md) | Partly implemented: CPU, f64, 2D (stable fluids on a MAC grid) | Stable Fluids on a MAC grid: semi-Lagrangian advection, pressure projection by CG or multigrid, scalars and vorticity confinement; Taylor–Green decay check. |
| [29. Materials meet](ch029-materials-meet.md) | Unimplemented: design draft | Explicit coupling between solver pairs (rigid–SPH, rigid–MPM, rigid–PBD, rigid–FEM, MPM–SPH) with a zero-sum momentum ledger; unsupported pairs refused. |
| [30. Contact that holds](ch030-contact-that-holds.md) | Unimplemented: design draft | SAP rigid–FEM coupling and later IPC barriers in f64; a held grasp and intersection-free checks; refused on Metal and with gradients. |
| [31. Many worlds in one step](ch031-many-worlds-in-one-step.md) | Unimplemented: design draft | A batch dimension over every solver, per-environment parameters and randomization with provenance, isolated resets, counter-based seeds; batch invariance checked bit for bit; aggregate and per-environment rates reported together. |
| [32. Differentiate a simulation](ch032-differentiate-a-simulation.md) | Unimplemented: design draft | Reverse mode over steps with exact checkpointed recomputation; adjoints of implicit steps reusing the forward factorization; per-solver gradient modes enforced at build and compile time. |
| [33. Sensors read the simulation](ch033-sensors-read-the-simulation.md) | Unimplemented: design draft | Joint, IMU, force/torque, contact, tactile, lidar and depth sensors as read-only stages with rates, delays, noise and R18 stamps. |
| [34. Pictures for policies](ch034-pictures-for-policies.md) | Unimplemented: design draft | A batched observation rasterizer (RGB, depth, segmentation, normals) into device tensors read without copies; the Metaverse stays the presentation renderer; image randomization. |
| [35. Bring in robots and meshes](ch035-bring-in-robots-and-meshes.md) | Unimplemented: design draft | URDF, MJCF and mesh import with inertia and watertightness validation, convex decomposition and tetrahedralization, content-addressed assets and per-field provenance. |
| [36. Record and replay a world](ch036-record-and-replay-a-world.md) | Unimplemented: design draft | Recordings with scene, profile, compiler and seed identities; recomputation replay checked against recorded digests, bisection to the first divergent step and stage. |
| [37. Measure the engine honestly](ch037-measure-the-engine-honestly.md) | Unimplemented: design draft | The compatibility matrix as data, the acceptance suite per backend and profile, fixed benchmark scenes, and claims that always include per-environment rates and compile and startup times. |

### V. Big worlds

| Chapter | Status | Result |
| --- | --- | --- |
| [38. Stream a large world](ch038-stream-a-large-world.md) | Unimplemented: design draft | Existing cell/edge/save primitives support residency rings and a floating origin. The full teaching chapter and actual large-world host qualification remain open; replay is exact for the same accepted completion history. |
| [39. Ephemerides, time scales and frames](ch039-ephemerides-time-scales-and-frames.md) | Partly implemented: CPU, f64 (Horizons tables, light time, aberration, a turning frame) | A pinned Horizons/SPK ephemeris with recorded scale, frame, centre, dataset and coverage; TAI/TT/TDB/UTC through a pinned leap-second table; full state transforms with the ω × r term; geometric, light-time and aberration-corrected states kept apart; `aethyr/physics/orbit.coax`'s mean-element error measured. |
| [40. Propagate with a stated error](ch040-propagate-with-a-stated-error.md) | Unimplemented: design draft | Two-body, patched-conic and restricted n-body propagation on RK4, leapfrog, higher-order compositions and Dormand–Prince 5(4), each reporting error per observable with step count and cost; a Newtonian envelope that refuses, and constant proper acceleration as its own mode. |
| [41. Burns consume mass](ch041-burns-consume-mass.md) | Unimplemented: design draft | Variable-mass flight with separate energy, reaction-mass, coolant and consumable accounts, jet power P = F v_e / 2 as an admission check, off-centre thrust torque and inertia recomputed as tanks drain; Δv against the rocket equation and momentum conserved for ship plus exhaust. |
| [42. Reach a moving target](ch042-reach-a-moving-target.md) | Unimplemented: design draft | Rendezvous as a boundary-value problem: Lambert guesses, the t = 2√(D/a) benchmark, finite-thrust shooting and collocation under crew, reserve and exclusion constraints, and flybys whose energy change follows the encounter; infeasible requests name the binding constraint. |
| [43. Heat has to go somewhere](ch043-heat-has-to-go-somewhere.md) | Unimplemented: design draft | A lumped thermal network with radiators, finite sinks, coolant and failures, and propulsion heat only through declared hypothetical models; radiator equilibrium checked (49.03 GW at 1,000 K and ε = 0.9 needs about 0.961 km²). |
| [44. Inside a turning, accelerating ship](ch044-inside-a-turning-accelerating-ship.md) | Unimplemented: design draft | Interior frames with Coriolis, centrifugal and Euler terms or inertial integration (never both), spin habitats (0.3 g at 500 m is about 0.7325 rpm), foot attachment, compartment leaks, and a crew-load model whose parameters carry provenance. |
| [45. Sensors that see the past](ch045-sensors-that-see-the-past.md) | Unimplemented: design draft | Truth, emitted signal, received measurement and estimated track as separate data: light time (about 499.0 s at 1 AU), round trips, detection, and a track filter whose uncertainty grows; agents get only stamped observations, and a policy that reads truth is refused. |
| [46. Time warp that stops for every event](ch046-time-warp-that-stops-for-every-event.md) | Unimplemented: design draft | Time warp as scheduling: event functions, root finding on dense output and steps split at the earliest event, so warp 1 and 10⁶ give the same event order and final state; a 1,200 km/s projectile cannot pass a target; replay at omniscient and observation levels. |
| [47. Hits through layers](ch047-hits-through-layers.md) | Unimplemented: design draft | Hypervelocity hits as penetration through layers, using tables fitted to ballistic-limit equations that refuse outside their range, over a component graph, so a severed coolant line heats the drive and cuts thrust; large fragments become tracked bodies. |
| [48. One world at three fidelities](ch048-one-world-at-three-fidelities.md) | Unimplemented: design draft | Engineering, game and presentation profiles over one state, typed rule modules with declared footprints, provenance on every tunable, batched rollouts and bounded learned surrogates, the scenario admitted as an environment, and a ten-scenario regression suite at engineering fidelity. |
| [49. Rooms and presence](ch049-rooms-and-presence.md) | Partly implemented: native room and browser client overview | Overview of current code and its remaining scope; follow the linked implementation chapters. |
| [50. Lockstep for small deterministic games](ch050-shared-state-over-the-network.md) | Partly implemented: native relay and browser lockstep overview | Overview of current code and its remaining scope; follow the linked implementation chapters. |

### VI. Making worlds

| Chapter | Status | Result |
| --- | --- | --- |
| [51. A realm is a document](ch051-a-realm-is-a-document.md) | Partly implemented: native C (a made realm's document read, refused whole) | A typed, versioned realm document (setup, catalogue features, wants, placements, tools) built from one catalogue table, laid out deterministically from document and seed, byte-identical natively and in the browser, with every asked thing kept as a feature or a `want` line. |
| [52. Edit a realm in place](ch052-edit-a-realm-in-place.md) | Partly implemented: native C (moving any placed thing; every edit undoable) | Edits as invertible operations over the document with server-decided roles, proposals settled once, a replayable edit log, and the asks ledger's revert rule that leaves later changes alone. |
| [53. Bring in assets](ch053-bring-in-assets.md) | Partly implemented: native C (a VPK1 pack read whole, written again, admitted by content) | One Coax import path for `VPK1` packs, cast and drops, with a licence on every asset, budgets that refuse, unit checks with recorded rescales, admit/upload/scan for uploads, and packs byte-identical to `pack_realm.py`. |
| [54. Publish a realm](ch054-publish-a-realm.md) | Partly implemented: native C (visitors see the revision published) | A published realm as a frozen record of document, module and asset hashes, checked by `realms.ts`, goldens and the 60 FPS rule, with made realms never overwritten by a release. |
| [55. Fork a realm](ch055-fork-a-realm.md) | Partly implemented: native C (a made realm's fork: its parent, its own log) | Forks of any realm as a base version, declared rule switches and their own edit log, with rule-isolation checks, replay from the base, and the original kept byte-identical. |

### VII. Realms

| Chapter | Status | Result |
| --- | --- | --- |
| [56. The Grid](ch056-the-grid.md) | Partly implemented: native C and Wasm (the Grid's session on the engine core) | The Grid rebuilt from the foundation chapters: a world-space floor that looks the same from any position, a smooth jump, feet at the physics height, and live people from the room. |
| [57. Made realms](ch057-made-realms.md) | Unimplemented: design draft | One made-realm player of chapter 51's document, with composition aimed at a first visit, four cameras over one controller, budgets, and dropped-model realms, all on the Metaverse Engine. |
| [58. Everglade](ch058-everglade.md) | Partly implemented: native room and browser worker overview | Overview of current code and its remaining scope; follow the linked implementation chapters. |
| [59. Dead Gears](ch059-dead-gears.md) | Unimplemented: design draft | The ship survival game on the Metaverse Engine's entities and loop (its ECS becomes chapter 6's design), with every ported demo check passing and 200 creatures held to 2 ms a frame. |
| [60. Ice Cream Man](ch060-ice-cream-man.md) | Unimplemented: design draft | The seeded house game on the Metaverse Engine's loop, actions, replay and follow camera, with every recorded Rust trace still replaying fingerprint for fingerprint. |
| [61. Hecate's Cemetery](ch061-hecates-cemetery.md) | Unimplemented: design draft | The moonlit memorial garden rebuilt: the arrival flight handing the camera over smoothly, a dog that follows by the paths, the moon in its real phase, and scanned pet memorials. |
| [62. Fairy Forest](ch062-fairy-forest.md) | Unimplemented: design draft | The bloom-and-roots story on saved state, with flight moved from the face into the shared controller, so a full session replays and flies the same at any frame rate. |
| [63. The space games](ch063-the-space-games.md) | Unimplemented: design draft | Orbit League and Belt 2200 on the Metaverse Engine's physics and spacecraft chapters, with `space.md`'s checks, transfers within their stated error and lockstep games that replay to the same hashes. |
| [64. Every realm runs on the Metaverse Engine](ch064-every-realm-runs-on-aethyr.md) | Partly implemented: native C (every realm's shared modules from its #load closure; one walker in seven realms) (`ec-migrate`) | The `ec-migrate` plan: an inventory of each realm's private copies, a migration order, a check that no private loop, camera, input path or ground logic comes back, and goldens and 60 FPS held throughout. |

### VIII. Hearing it

| Chapter | Status | Result |
| --- | --- | --- |
| [65. Sound from numbers: oscillators and envelopes](ch065-sound-from-numbers.md) | Runnable: native C | Integer oscillators (saw, square, a fitted sine), seeded xorshift noise, ADSR envelopes, decays as a repeated multiply, a one-pole filter, constant-power pan and Padé saturation, with no `libm` and no clock. |
| [66. A sequencer on the beat grid](ch066-a-sequencer-on-the-beat-grid.md) | Runnable: native C | A tempo grid in ticks whose every step is computed from its own tick, so it never drifts; sections snapped to steps; keys and triads from one voicing rule; patterns stepped to exact samples. |
| [67. Scores, mixing and a soundtrack](ch067-scores-mixing-and-a-soundtrack.md) | Runnable: native C | Stereo buses with a Schroeder reverb send, gain automation, a master that normalizes, automates and saturates without clipping, WAV files, a sample hash, and the `promo-epic` score rendered by `aethyr/tools/score.coax`. |
| [68. Sound in the world: playback, space and reaction](ch068-sound-in-the-world.md) | Partly implemented: browser WebGPU (placed one-shot sounds with gain, pan, delay and lowpass in Everglade; the audio-clock worklet and music sections still to do) | Block rendering on the audio clock through a Web Audio worklet and native hosts, sources placed in the world by distance, pan and delay, and music that changes section on the bar line with the game's state, all replaying exactly. |
| [69. Voices in a realm](ch069-voices-in-a-realm.md) | Partly implemented: native C (spoken requests through the replay gate; the forwarder and spatial voices still to do) | Players, NPCs and agents heard from where they stand: one `Speaker`, `Utterance` and `Route` model, a forwarder that picks who hears whom from positions, zones, consent and floors without decoding audio, and voices placed by chapter 68's gain, pan and delay. |

### IX. A living world

| Chapter | Status | Result |
| --- | --- | --- |
| [70. Rivers, ponds and the sea](ch070-rivers-ponds-and-the-sea.md) | Unimplemented: design draft | Water from `verse/water*.coax` (W1–W11) as one module: typed water bodies, a wave surface fixed to the tick, buoyancy and drag, the spectral sea and the underwater view, with the same gameplay surface on every client and the water lab held to 1 ms of GPU on a phone and 3 ms on a desktop. |
| [71. Sky, weather and the time of day](ch071-sky-weather-and-the-time-of-day.md) | Partly implemented: bounded declared day/phase facts; sky/weather renderer remains draft | One day clock (`real`, `cycle` or `fixed:T`) drives the sun, the moon's real phase, palettes, the starfield, a seeded weather schedule and ground wetness, so every visitor and replay sees the same sky. |
| [72. Effects made of particles](ch072-effects-made-of-particles.md) | Partly implemented: browser WebGPU (puffs; stateless emitters remain a design draft) | Event puffs from seventeen recipes, a pure function of seed, index and age, under declared budgets (96 live causes, 2,400 puffs a frame) and drawn on a traced page in 5E Lab; the stateless emitters (embers, dust, fireflies) and fire and smoke are not built yet, kept separate from chapter 23's simulated particles. |
| [73. Things that break](ch073-things-that-break.md) | Unimplemented: design draft | `verse/demolish.coax` on Part IV's bodies and contact: Voronoi fracture into 2–24 chunks, a support graph that topples pieces, budgets for chunks and loose pieces, regrowth, and the destructible-wall integration test. |
| [74. Wheels on the ground](ch074-wheels-on-the-ground.md) | Unimplemented: design draft | Orbit League's raycast vehicle on the Metaverse Engine's queries and rigid bodies: spring-damper suspension, tyres inside the friction circle, steering that fades with speed, game rules declared as rules, and a recorded match that replays to the same digests. |
| [75. Creatures that follow and watch](ch075-creatures-that-follow-and-watch.md) | Partly implemented: bounded compiled actor profile 1, native C and Wasm; legacy realm adapters remain | Creatures sense, decide and act at a fixed step from typed sight and hearing stimuli kept apart from truth; the flock becomes a general follow, the watch's state machines and seeded dice replay, and a remote decision model picks only from a fixed enum. |
| [76. Find a way through](ch076-find-a-way-through.md) | Partly implemented: bounded grid navigation and actor request fences, native C and Wasm; graph and avoidance adapters remain | Walking grids with no corner cutting and Dijkstra flow fields with a fixed tie-break, bounded by cost and settled cells, with typed results naming the nearest reachable cell, checked cell by cell against a brute-force oracle and equal bit for bit to Dead Gears' fields; graph routing, avoidance and remaining realm adapters retain separate work. |
| [77. People who talk back](ch077-people-who-talk-back.md) | Partly implemented: native CPU (Darwin arm64; bounded dialogue and request/reply replay) | Required authored fallback, bounded UTF-8 JSON, request identity fences, tick deadlines and an admitted in-memory answer record; exact native debug/release fixtures replay with no send intentions. Realm/server adoption, durable external-event integration, persistent memory and voice remain. |
| [78. Cinematics and close-ups](ch078-cinematics-and-close-ups.md) | Partly implemented: native CPU camera take; Wasm flight and close-up adoption | Typed flights and close-ups share the quintic blend; Hecate and Garden adopt them. Actual arrival and every-frame skip checks run at 30/60/144 FPS; a native camera take replays five commands and staged captures receive sidecars. Saved realm replay to browser pixels remains. |

### X. Generating worlds

| Chapter | Status | Result |
| --- | --- | --- |
| [79. A piece from a sentence](ch079-a-piece-from-a-sentence.md) | Unimplemented: design draft | One catalogue and one typed selector for the Sandbox's set pieces and the Metaverse Engine's board: the answer is checked against the catalogue on the server and in the game, laid out from a seed as one undo step, and every asked thing is kept. |
| [80. Terrain and scatter from a seed](ch080-terrain-and-scatter-from-a-seed.md) | Partly implemented: native C and Wasm (keyed scatter over the built ground) | Heightfields equal to the drawn surface, levelled paths and flats, jittered scatter with exclusion zones, and one counter-based random stream per feature, so content is the same in any streaming load order. |

### XI. Shipping realms

| Chapter | Status | Result |
| --- | --- | --- |
| [81. Save and come back](ch081-save-and-come-back.md) | Partly implemented: native C (the AESV container and its two slots) | One save layer over today's `AESV` container with a CRC-32 and two slots: only declared fields are saved, older schemas migrate forward, a newer schema is refused, and conflicts found by revision, never by clock time. |
| [82. Change code while it runs](ch082-change-code-while-it-runs.md) | Unimplemented: design draft | Live iteration in four tiers (data, shaders, logic, layout changes) with a save-to-frame target for each; layout fingerprints decide whether to swap, migrate or reset, a broken edit keeps the old module running, and production never loads code. |
| [83. Inspect and profile a running realm](ch083-inspect-and-profile-a-running-realm.md) | Partly implemented: native C and headless Chrome (one 60 FPS rule for every realm; frame reports) | The same frame report, overlay, probes and goldens for every realm, the 60 FPS rule (95th percentile under 17.5 ms, no frame over 33 ms) extended to every listed realm, separate from chapter 37's physics measurements. |
| [84. One realm on every host](ch084-one-realm-on-every-host.md) | Partly implemented: shared lifecycle policy and versioned host overview | Overview of current code and its remaining scope; follow the linked implementation chapters. |
| [85. Everyone can play](ch085-everyone-can-play.md) | Partly implemented: shared settings and browser accessibility | Overview of current code and its remaining scope; follow the linked implementation chapters. |

### XII. Many players: a multiplayer engine

| Chapter | Status | Result |
| --- | --- | --- |
| [86. One server owns the scene](ch086-one-server-owns-the-scene.md) | Partly implemented: native C (oa-room's 30 Hz tick, started at its declared defaults) | oa-room steps every room's world headless at 30 Hz, from every declared default: `--start-check` compares the started server with its declarations and with a server started in `0xa5`-filled memory. |
| [87. Server and client share one step and one world](ch087-server-and-client-share-one-step.md) | Partly implemented: native C and browser Wasm (one movement step, one rules unit and one world document) | The page and the server run one Coax movement step on the input as the wire carried it, one SRD rules unit and one world document key in every setup. |
| [88. A seat joins with a signed ticket](ch088-a-seat-joins-with-a-signed-ticket.md) | Partly implemented: native C (HMAC seat tickets checked at join) | An HMAC-signed seat ticket from the API names the player, room and shard; the server checks it at join and seats the ticket's name, not the hello's. |
| [89. Intents in, acknowledgements out](ch089-intents-in-acknowledgements-out.md) | Partly implemented: native C (numbered inputs, a bounded queue, owed steps and refusals) | Numbered intents into a 16-input queue that drops old and repeated ones, stepped at the rate time passes (`EG_BANK`, `EG_TICK_STEPS`), acknowledged in every state frame and refused with a reason. |
| [90. Predict your own movement](ch090-predict-your-own-movement.md) | Partly implemented: browser Wasm (predict, reconcile and replay; corrections eased on the drawn body) | The page predicts only its own movement, replays its unacknowledged inputs from the server's state, eases the difference out of the drawn body over 0.12 s, and turns the camera toward the controls' motion. |
| [91. One record for every replicated entity](ch091-one-record-for-every-replicated-entity.md) | Partly implemented: native C and browser Wasm (wire 3: one row for every entity, deltas against what the page heard, LIFE and WARP) | One record `s ID BACK MASK ...` for every entity: state at a fixed tick with velocity, deltas against an acknowledged tick, generation numbers and teleports as discontinuities. |
| [92. Priority under a budget](ch092-priority-under-a-budget.md) | Partly implemented: native C (rows by priority within WIRE_BUDGET a tick; a seat behind gets its own row; sockets bounded by bytes and age) | Priority from distance and relevance that accumulates, a per-seat byte budget, your own state and effects always sent, and send queues bounded by age. |
| [93. One interpolation buffer for every remote entity](ch093-one-interpolation-buffer.md) | Partly implemented: browser Wasm (one adaptive buffer for every remote entity; a new LIFE or a WARP starts it again) | Every remote entity drawn from a ring of 8 states a delay behind a ping-estimated server clock, the delay adapting from 80 to 300 ms with lateness and jitter. |
| [94. Effects are state, events are cues](ch094-effects-are-state-events-are-cues.md) | Partly implemented: browser Wasm (lasting abilities as entities; one status state) | Lasting effects as entities with an owner, a cause, `T0` and `DUR`; particles, rings and status lines derived from them; one-shot events as cues that can't change state. |
| [95. Offline is a local server](ch095-offline-is-a-local-server.md) | Partly implemented: browser Wasm (the room's tick in a Web Worker behind ?offline=worker) | The room's simulation built to Wasm and run in a Web Worker as the offline host, so the page is always a client and the offline game's own paths are deleted. |
| [96. Casts and combat under latency](ch096-casts-and-combat-under-latency.md) | Partly implemented: native C and browser Wasm (range and sight at start and release, a half-second command age, one order a tick) | Written rules: range and line of sight at a cast's start and release, a cast waiting at most 0.5 s for the inputs before it with no client clock, a target falling mid-cast, heals before harm in each tick; `room-proof.coax cast` holds at 250 ms with 2% loss. |
| [97. Interest without leaks](ch097-interest-without-leaks.md) | Partly implemented: native C (a 32 m cell grid, 110 m in and 130 m out, blocked and hidden entities withheld from every channel) | Interest from a cell index with separate entry and exit radii, non-spatial subscriptions, and nothing private on the wire: blocked players and unseen invisible entities are never sent. |
| [98. Versions, deploys and reconnect](ch098-versions-deploys-and-reconnect.md) | Partly implemented: native C and browser Wasm (a wire version check, seat generations and a 15 s grace hold; vitals saved by receipt) | A wire version handshake with a reload, deploys that drain a shard, and reconnect with seat generations, a grace hold and stale frames dropped; today's rejoin after a quiet socket. |
| [99. Watch the whole action path](ch099-watch-the-whole-action-path.md) | Partly implemented: native C (the action path's vitals every 10 s: tick by workload, queues, bytes, relevance, acks and refusals) | The action path measured stage by stage: tick percentiles by workload, queue depth and age, bytes by message type, entities per seat, join bursts and input-to-acknowledgement time. |
| [100. Prove the room plays like the game](ch100-prove-the-room-plays-like-the-game.md) | Partly implemented: native C and headless Chrome (the drift check under shaped networks; bots that walk, cast and fight) | The drift check plays one session offline and in a local room at local, 100 and 250 ms with 2% loss and compares what is drawn; bots that fight; the counterexample that motivated this part. |
| [101. The Everglade room](ch101-the-everglade-room.md) | Partly implemented: native C on 6 Cloud Run shards (16 rooms of 16 seats a process; the 60-raider raid) | Everglade's rooms: 16 seats, 16 rooms a process, 6 Cloud Run shards placed by the API's directory, and the Cultist Raid of up to 60 raiders on the server. |

### XIII. Actions and conditions

| Chapter | Status | Result |
| --- | --- | --- |
| [102. Ask, act, present](ch102-ask-act-present.md) | Partly implemented: native C and browser Wasm (ask, answer and show; one bearing from SRD conditions; a feel contract) | Every action of every class and monster asks, waits for the authority's answer and only then presents; one bearing drives the pose, the input lock and the hotbar from a body's conditions; and a check fails when an action lacks an animation, an effect, a sound or a movement. Zones wire the pipeline in and hold no copy of it. Not built: a runnable example and the remaining shared modules (#13079 to #13088). |
| [103. One world: columns, systems and a spatial index](ch103-one-world.md) | Partly implemented: native C (the World on the entity store and the spatial grid; a 2,000-entity session replayed by digest) | A realm's entities live in one `World`: the entity store, component columns keyed by slot, an ordered system list whose declared reads and writes are checked, and a spatial grid. Two runs of a 2,000-entity session through three systems give the same digest at every step. |

[Appendix A](appendix-mmo-gap-analysis.md) compares these chapters with an MMO-capable engine, capability by capability, and links the issues for each gap.
