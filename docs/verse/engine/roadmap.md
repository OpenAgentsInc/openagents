# Verse Engine delivery roadmap

Status: proposed work, October 3, 2026. This roadmap extends the
[architecture](architecture.md) and the
[#10406 authority plan](https://github.com/OpenAgentsInc/openagents/issues/10406#issuecomment-5973821300).
The [expanded issue plan](https://github.com/OpenAgentsInc/openagents/issues/10406#issuecomment-5974013251)
connects these stages to the service acceptance.
Verse Engine is the reusable engine; Verse is the metaverse built on it.
The original chamber is the first new game fixture, while Lagrange and Physics
Lab remain regression consumers. This roadmap imports no reference-engine code.

[#10432](https://github.com/OpenAgentsInc/openagents/issues/10432) replaces the
native scene's default placeholder actors with the downloaded CC0 Universal
characters, four modular fantasy outfits, and retargeted Standard animations.
The shared engine now evaluates glTF rest transforms and inverse binds. Six
player appearances and mixed outfitted NPCs are available; `--greybox` retains
the procedural fixture. See the [character sources and usage](../../../assets/verse/characters/quaternius/README.md).
This character pipeline uses no Blizzard assets and does not complete world
service authority or general retargeting for unrelated skeletons.

[#10433](https://github.com/OpenAgentsInc/openagents/issues/10433) replaces
mismatched humanoid clips with authored gaits and combat poses, advances gait
clocks from admitted travel, and loads a giant locally licensed Bestiary Puglin
for Claude. Restricted Bestiary source files remain outside the public repo.

## Original scene implementation

[#10435](https://github.com/OpenAgentsInc/openagents/issues/10435) adds portable
generation-safe entity storage, respawn life IDs, and a bounded fixed schedule.
Native player and controller modes now share 30 Hz stepping with a three-step
catch-up limit and recorded dropped time. Chamber arrows carry target life IDs
and cannot damage a later respawn. This starts VE-0; #10437 extends authority
and lifetime fencing, while physics substep integration remains.

[#10436](https://github.com/OpenAgentsInc/openagents/issues/10436) introduces
the headless `verse-world` command boundary. Local human and controller ability
requests share ownership, life, epoch, sequence, tick-window, and finite-input
admission. Explicit control handoffs fence queued commands. #10437 extends this
boundary to movement and owned rules. Service authentication, durable rewards,
and replication remain required before VE-5 acceptance.

[#10437](https://github.com/OpenAgentsInc/openagents/issues/10437) moves the
project-owned chamber encounter, controller, collision/navigation admission,
and utility spells into `verse-world`. Its independently implemented combat
store replaces retained Ruins health, mana, cooldown, projectile, and burn
resolution. The headless dependency tree has no retained vendor, GPU, platform,
or transport code; renderer compatibility modules now re-export owned authority.
Pending arrows, player casts, hostile casts, and statuses are fenced by life or
nonreused combat IDs. Versioned checkpoints retain pending combat and controller
state, and replay tests cover defeat and subsequent respawns. Dialogue, camera
handoff, actual damage, deaths, and respawns have bounded, serialized event IDs.
The owned chamber profile keeps ten abilities, 300,000 boss HP, and 60-second
cultist respawns. This is local authority and checkpoint serialization; it does
not implement transactional saves, multiplayer, rewards, or the VE-2 capsule/
mesh controller. The broader `verse` app still includes the separate Ruins zone.

[#10426](https://github.com/OpenAgentsInc/openagents/issues/10426) implements the
first asset-free procedural scene. The `verse_play` native example generates an
original chamber, skeletal placeholder actors, weapons, particles, and UI
sprites; it uses the bundled licensed Fira Mono font. It retains timed dialogue,
camera handoff, local combat, overhead red health bars, damage numbers, corpses,
and respawns. See the [scene instructions](../../../assets/verse/original/README.md).

[#10427](https://github.com/OpenAgentsInc/openagents/issues/10427) extracts generic
pack, skeletal animation, and cinematic contracts into `crates/verse-engine`.
The renderer and original scene consume that headless crate directly; the WoW
adapter preserves its old API through re-exports.

[#10428](https://github.com/OpenAgentsInc/openagents/issues/10428) adds continuous
axis-aligned box sweep and wall sliding queries to `physics`. The original scene
shares room solids between rendering and collision. Movement and Misty Step use
those queries; a thin obstacle cannot be crossed by a large displacement. This
is primitive box collision, not the full capsule/mesh controller of VE-2.

[#10429](https://github.com/OpenAgentsInc/openagents/issues/10429) uses the same
query for third-person camera clearance against walls, columns, and the floor,
without changing the requested orbit distance.

[#10430](https://github.com/OpenAgentsInc/openagents/issues/10430) adds bounded,
deterministic static box navigation in `physics`. Cultists route around columns,
and shared sweeps constrain their placement and control displacements. This is
a visibility graph for a flat primitive room, not a crowd solver or navmesh.

[#10431](https://github.com/OpenAgentsInc/openagents/issues/10431) applies static
visibility to targeted attacks, delayed bow/cast rechecks, directional damage,
and hostile cast/impact admission. Obstructed cultists navigate toward a firing
position. Full projectile CCD and explosion-radius occlusion remain separate
work; this slice does not replace the retained projectile solver.

This delivers the visual procedural milestone of VE-1/VE-3, not the complete
VE-0–VE-6 roadmap. #10437 supplies owned headless chamber rules; service
persistence, multiplayer, full physics, tools, and platform acceptance remain
implementation work. #10406 and #10407 stay open for their full acceptance.

[#10438](https://github.com/OpenAgentsInc/openagents/issues/10438) adds scoped
ray, capsule overlap, and capsule sweep queries in `physics::queries`. Validated
triangle meshes use a deterministic bounding hierarchy; query results have
instance/layer/usage filters, exact ignored-life matching, stable hit ordering,
bounded output, and traversal counters. Capsule sweeps return time of impact,
contact normals, and initial surface penetration; failure to converge is a
refusal rather than an unreported miss. The chamber compiles the same room
solids into triangle query fixtures. Meshes describe two-sided surfaces, not
automatic filled-volume occupancy. Conservative advancement is independently
implemented; the public method also appears in
[Pan, Zhang, and Manocha's collision-checking study](https://www.roboticsproceedings.org/rss07/p32.pdf).
No planner or reference-engine code is imported.

[#10439](https://github.com/OpenAgentsInc/openagents/issues/10439) adds grounded
upright capsules: wall sliding, slope admission, stairs, gravity, jump and ceiling
rejection, solid-box spawn recovery, checked teleport endpoints, and moving
platform poses. Rigid poses reuse compiled mesh hierarchies. Player and NPC
movement run four 120 Hz substeps per 30 Hz chamber command tick; the clock
records dropped time. Space submits an admitted jump; presentation interpolates
player and NPC positions without changing authority. That milestone uses
`verse-chamber-owned-v2`; checkpoints include movement state and preserve exact
floating-point replay. Native side stairs share rendered and collision geometry.
[#10440](https://github.com/OpenAgentsInc/openagents/issues/10440) replaces box
navigation with a compiled multilayer walkable grid, using capsule clearance,
slope and step admission, and collision-checked connections and shortcuts.
Deterministic routes distinguish unavailable paths from work-budget refusals.
Generation-fenced dynamic blockers update navigation and authoritative collision;
trusted NPC goals still move through the capsule controller. Checkpoints retain
routes and blocker revisions under `verse-chamber-owned-v3`. The native navigation
capture shows a cultist detouring, replanning after blocker removal, and climbing
the side stairs. This independently implemented grid uses public concepts from
[Recast's navigation configuration](https://recastnav.com/structrcConfig.html);
it does not import Recast or implement polygonization or funnel routing.
[#10441](https://github.com/OpenAgentsInc/openagents/issues/10441) completes the
chamber body and projectile integration.
Spell flights now use relative-motion sphere/capsule contact in 120 Hz slices,
including moving targets, earliest wall obstruction, impact-time area positions,
and pending-motion checkpoint replay under `verse-chamber-owned-v4`. Teleports
explicitly skip intermediate space. The trajectory integration records actual
character controller substep positions under `verse-chamber-owned-v5`; projectile
intervals split at every trajectory corner, and checkpoints validate retained
path endpoints. The `verse-chamber-owned-v6` body registry binds player and NPC
kinematic bodies to exact life generations, retains corpse collision for 60 seconds, removes
expired collision/navigation masks, and rebuilds those masks on checkpoint load.
Props cannot overwrite actor collision IDs. The `verse-chamber-owned-v7` profile
binds collision-only prop bodies to blocker geometry; movement, resize, removal,
and generation reuse update both together. Checkpoints reject missing ownership
or mismatched bounds. The `verse-chamber-owned-v8` profile replaces scheduled bow damage with
non-homing arrows in the shared continuous projectile path. Arrows collide with
the first actor or cover, can miss moving targets, expire, and retain exact
in-flight replay. Presentation uses the arrow mesh without spell emission.
The `verse-chamber-owned-v9` profile advances stored hostile flights against
player controller trajectories and cover. Capsule contact can resolve before
nominal area arrival; area impacts sample the player at impact time. Shields
use the shared damage path, and checkpoints validate source and target lives
and stored flight positions.
The `verse-chamber-owned-v10` profile admits live PC and NPC capsule contacts
through shared sweep, overlap, and pose queries. Controllers ignore their own
life; admitted movement updates body poses for subsequent actors. Navigation
plans retain static geometry while controllers resolve living obstructions.
Death replaces the live capsule with corpse collision, and loading rebuilds
live colliders from owned bodies. Defeat cancels pending player casts.
The adventurer now has 200 HP. After defeat, **Respawn** returns human control at the authored spawn with full health and mana, a new player life, and a new command epoch. NPC health and cultist respawn deadlines survive. Native [death](../../../bench/verse/2026-10-03/player-dead.png), [respawn](../../../bench/verse/2026-10-03/player-respawned.png), and [receipt](../../../bench/verse/2026-10-03/player-respawn.json) artifacts record the transition.

The `verse-chamber-owned-v13` profile uses one world clock to consume movement,
combat, and timer time. Combat elapsed time derives from the 120 Hz step count;
scene time uses a retained origin. Fractional frames retain pending input until
a step is available, and checkpoints reject divergent clocks. The retained
[native combat video](../../../bench/verse/2026-10-03/life-bound-combat.mp4) and
[receipt](../../../bench/verse/2026-10-03/life-bound-combat.json) show all ten
abilities, 12 defeated cultists, 54 absorbed damage, player defeat, surviving
Claude, and no dropped physics time. Renderer lighting and overlapping nameplates
remain visual work.

Movement and spell lag measurements on the Apple M5 Max are retained in
[the performance report](../../../bench/verse/2026-10-03/lag-report.json).
Matched frame work falls from 83.2 ms median / 143.5 ms p95 to 9.4 ms / 12.5 ms.
Live frames stay on the GPU, unchanged corpse poses reuse exact grounding, and
posed bounds cull irrelevant shadow cube faces. Development builds optimize the
real-time crates. A 100-second movement/fireball capture completes 23 spells and
respawns cultists at 12.7 ms p95 with no dropped time. Initial hostile casts are
deferred in that isolated stress fixture; the matched battle retains attacks.
Set `VERSE_FRAME_PROFILE` to an NDJSON path when running `verse_play
--combat-demo OUTPUT.mp4` or `--stress-capture OUTPUT.mp4`. Run `verse_play
--check-profile PATH.ndjson` to check retained evidence against the frame budget;
`--stress-demo PATH.ndjson 100` measures the real window with scripted controls.
The retained window run ended early at 86.7 seconds and supports live latency
measurements. The offscreen stress capture provides the full 100-second run.

[#10443](https://github.com/OpenAgentsInc/openagents/issues/10443) replaces original-scene numeric animation selectors with named states and per-model clip bindings. Cinematic cues, world snapshots, humanoid/monster compilers, weapon visibility, corpse grounding, and GPU playback share the contract. Packs declare looping or held poses and transition durations; interrupted blends retain the current local pose. Exact actor lives fence playback and grounding across respawn. Numeric decoding remains in the explicit research compatibility path. `verse-chamber-owned-v13` checkpoints carry the new cue selection format.

The [native named-state battle](../../../bench/verse/2026-10-03/named-animation-combat.mp4) and [receipt](../../../bench/verse/2026-10-03/named-animation-combat.json) record all ten abilities, 200 damage taken, 162 shield absorption, cultist respawns, and surviving Claude. The [respawn proof](../../../bench/verse/2026-10-03/named-animation-respawn/player-respawn.json) records the next player life, movement, and a shield cast. The [performance report](../../../bench/verse/2026-10-03/named-animation-performance.json) retains both runs: the first overlapped verification jobs and exceeded the 50 ms stall bound; its cause is unproven. The isolated repeat passed at 12.7 ms p95 / 19.1 ms maximum. Generic animation graphs, marker/audio events, asset residency, renderer extraction, service authority, and tool/platform acceptance remain.

[#10444](https://github.com/OpenAgentsInc/openagents/issues/10444) moves texture preparation into the optional headless `verse-engine::loading` module. Full pack validation, exact digests, static eight-bit RGBA dimensions, explicit-root file checks, and manifest/encoded/decoded/workspace budgets pass before GPU allocation. The renderer uploads immutable prepared bytes and publishes the admission receipt. The [native receipt](../../../bench/verse/2026-10-03/prepared-pack/player-respawn.json) records 20 textures, 5.8 MB encoded, and 38.1 MB decoded; [pixel comparison evidence](../../../bench/verse/2026-10-03/prepared-pack/comparison.json) confirms unchanged death and respawn images. This is local pack preparation, not typed residency, provenance/distribution admission, asynchronous loading, or atomic GPU reload.

[#10445](https://github.com/OpenAgentsInc/openagents/issues/10445) adds typed model and texture residency handles with nonreused process-local catalog identities. Native frame extraction retains immutable instance data and catalog-bound model references; both shadow and color passes use typed GPU residency. Submission rejects stale catalogs before buffer writes, including empty frames. `verse_play --residency-proof OUTPUT_DIR` checks old-frame rejection after an offscreen renderer rebuild and compares death images byte for byte before continuing the respawn proof. This is not atomic live-surface reload, asynchronous streaming, persistent asset identity, or semantic texture naming.

[#10446](https://github.com/OpenAgentsInc/openagents/issues/10446) adds staged renderer reload on the existing GPU device. Original scenes save a complete runtime manifest; F5 parses, verifies, and uploads a replacement on a worker, then swaps renderer and CPU presentation data between frames. Old-base candidates and resolved frames are rejected. Static collision geometry, required states, and attachment IDs are fenced; compatible motion keeps current playback, and retired resources are disposed off the event thread. The live presenter rebinds its target after commit. `--reload-proof OUTPUT_DIR` and `--reload-window-proof OUTPUT_DIR` retain failure, world-state, pixel, and live-presentation evidence. Renderer asset reload does not admit scene/collision/rules changes or provide device-loss recovery, distribution, general streaming, or an editor. The [offscreen receipt](../../../bench/verse/2026-10-03/reload/reload.json) records unchanged pixels and world checkpoints after digest and GPU validation failures, old-base rejection, and a visible material change. The [live receipt](../../../bench/verse/2026-10-03/reload-window/window-reload.json) and [performance report](../../../bench/verse/2026-10-03/reload-performance.json) retain the 100-second Apple M5 Max run: 22 completed fireballs, cultist respawns, zero dropped time, 9.8 ms p95 CPU frame work, and 10.0 ms p95 / 12.9 ms maximum delivered intervals. The first two short variants synchronously encoded the proof PNG and dropped time; their evidence remains retained. The final probe encodes that image on a worker. These are frame-work and window-interval measurements, not GPU timestamps.

[#10447](https://github.com/OpenAgentsInc/openagents/issues/10447) adds persistent asset inventories alongside runtime residency. Model/texture bindings, content digests and lengths, dependency closure, compiler fingerprints, and declared creator/license evidence pass before original pack GPU upload. Native compilation snapshots pinned CC0 source data and licenses and the admitted local Puglin GLB; source-file locations do not define asset IDs. Capture and redistribution have separate admission receipts, and local-only Bestiary provenance refuses redistribution. Reload pins source declarations and compiler identity; explicit fingerprint refresh preserves IDs and rights after authored edits. The [inventory receipt](../../../bench/verse/2026-10-03/inventory/inventory.json) records relocation, stale content rejection, and the local-only distribution refusal. The [reload receipt](../../../bench/verse/2026-10-03/inventory-reload/reload.json) retains failed replacement and world-state checks. This inventory covers pack models, textures, and source evidence; signed distribution, complete scene/material/collision/audio/UI artifacts, clean-room installation testing, and generic render-world extraction remain. Provenance declarations do not establish legal attestation.

The [inventory window profile](../../../bench/verse/2026-10-03/inventory-window/performance.json) passes a 100-second movement, fireball, and live reload run: 22 casts, no dropped simulation time, 9.7 ms p95 frame work, and 10.3 ms p95 / 19.7 ms maximum delivered frames on Apple M5 Max. These are CPU and window-delivery measurements, not GPU timestamp measurements.


[#10448](https://github.com/OpenAgentsInc/openagents/issues/10448) moves instance presentation and catalog-bound frame extraction into the headless engine. Native drawing consumes the portable contract, preserving borrowed values, life identities, and animation selections. Extraction bounds instance counts and rejects missing models and nonfinite transforms, animation times, or emissions; submission rejects stale catalogs even for empty frames. Camera, lighting, UI extraction, complete render-world scheduling, and other VE-3 requirements remain.

[#10449](https://github.com/OpenAgentsInc/openagents/issues/10449) extracts camera and local-light values, admission, authored flicker, and cube-shadow camera construction into the headless engine. Native uniform packing consumes those contracts with its existing shader layout. Portable tests cover outward cube projections, capacity, nonfinite camera data, and derived intensity/projection overflow. The [native comparison](../../../bench/verse/2026-10-03/portable-lighting/comparison.json) confirms byte-identical death and respawn captures after extraction. UI extraction, generic render-world scheduling, material schemas, and service acceptance remain.

[#10450](https://github.com/OpenAgentsInc/openagents/issues/10450) furnishes the summoning lair with a pinned CC0 Fantasy Props MegaKit subset. Fourteen source models, authored flames and a summoning seal form 56 placements; 12 furniture bounds use the world prop collision path and return on encounter reset. Sixteen base lights combine warm torches/candles with green cauldrons and violet ritual light. The [scene receipt](../../../bench/verse/2026-10-03/summoning-lair/scene.json), [wide capture](../../../bench/verse/2026-10-03/summoning-lair/ritual-wide.png), and [720p combat video](../../../bench/verse/2026-10-03/summoning-lair/combat.mp4) record source admission, composition, all ten abilities, 200 damage taken, 108 shield absorption, 11 defeated cultists, and surviving Claude. The desktop now renders at physical window resolution with four-sample scene anti-aliasing, denser font glyphs, filtered mipmaps, and anisotropic filtering. The [Retina budget](../../../bench/verse/2026-10-03/summoning-lair/performance-hd/budget.json) passes at 3456×2104 over 100 seconds and 23 fireballs: 14.676 ms frame-work p95, 14.974 ms delivered p95, and no dropped simulation time. Static shadow caching, reusable nonempty draw commands, conservative visibility bounds, and zero-contribution light rejection preserve the scene while reducing work. The [resize receipt](../../../bench/verse/2026-10-03/summoning-lair/resize-hd/window-reload.json) verifies viewport changes and live asset reload without changing world authority. The local Bestiary source retains its redistribution restriction. The current material path uses base color; retained normal and ORM maps do not imply those shader features are implemented.

[#10480](https://github.com/OpenAgentsInc/openagents/issues/10480) moves the HUD triangle vertex contract into the headless engine. Borrowed overlays retain catalog identity and reject incomplete triangles, nonfinite positions, invalid atlas coordinates or RGBA values, and uploads above 4 MiB before GPU writes. Native atlas rasterization and layout retain their existing behavior. Semantic UI extraction, atlas asset identity, generic render scheduling, and tool/platform acceptance remain.

[#10481](https://github.com/OpenAgentsInc/openagents/issues/10481) combines camera, lighting, resolved instances, and HUD geometry into a read-only headless `RenderWorld`. Extraction admits all inputs together; submission fences the complete frame against catalog replacement, including empty frames. Native live drawing and capture consume this contract, while existing application entry points remain adapters. GPU resources, world authority, and atlas rasterization stay outside the frame. Material schemas, explicit render graphs, semantic atlas identity, animation graphs, audio, and platform acceptance remain.

[#10482](https://github.com/OpenAgentsInc/openagents/issues/10482) admits animation selections during render-world instance extraction. Catalogs retain each model's declared named states; missing states and negative sample times fail before native GPU writes. Explicit numeric research selections preserve idle/rest fallback. This closes a frame-admission gap, while generic animation graphs and marker/audio events remain.

[#10483](https://github.com/OpenAgentsInc/openagents/issues/10483) completed authored PBR materials. The portable material schema now admits bounded roughness, metallic, normal, occlusion, emissive, opacity, and cutout parameters, with explicit sRGB color/emission and linear data-map semantics. Pack validation and inventories include every material image dependency. Existing surfaces receive default material values. The glTF compiler retains those factors and maps, deduplicates shared image sources, and rejects unsupported UV sets. Per-surface GPU keys now preserve distinct material settings during spatial merging, and immutable bindings retain factors plus sRGB color/emission and linear data-map views. The channel semantics follow the [glTF specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html). The owned WGSL path now evaluates derivative-frame normal maps, GGX/Smith/Schlick material response, ambient occlusion, emission, opacity, and cutout thresholds. Its public mathematical basis is the [Filament material guide](https://google.github.io/filament/Filament.md.html); no reference shader code is imported. The [native captures and coverage](../../../bench/verse/2026-10-04/pbr-materials/README.md) retain actual GPU output. The final [native acceptance run](../../../bench/verse/2026-10-04/pbr-shadow-bindings/README.md) passes at 3456 × 2104 with 4× AA: 16.298 ms work p95, 16.596 ms delivered p95, 23 fireballs, live reload, and zero dropped simulation time. Shadow draws bind only alpha resources; full world PBR channels remain intact.

[#10488](https://github.com/OpenAgentsInc/openagents/issues/10488) adds portable render graph admission with bounded resources/passes, explicit access/dependency declarations, deterministic ordering, and rejection of missing producers, cycles, and unordered write hazards. The immutable chamber plan covers conditional static shadow refresh, cache copy, dynamic shadow faces, world multisample resolve, overlay, and optional readback. Native drawing executes admitted actions before submission and preserves the existing pass bodies and cache keys. The [native acceptance evidence](../../../bench/verse/2026-10-04/render-graph/README.md) passes at 3456 × 2104 with 4× AA: 16.310 ms work p95, 23 fireballs, live reload, and zero dropped simulation time. The graph describes pass ordering and resource access; generic transient allocation, cross-backend graph execution, and GPU timestamp instrumentation remain.

[#10489](https://github.com/OpenAgentsInc/openagents/issues/10489) adds authored animation marker tracks to admitted model packs and inventory digests. Clip identity, duration, ordering, and capacity are validated. Pose playback delivers bounded presentation events across loops and held clips, with actor-life and selection epochs; life/selection changes and seeks reset the baseline. Native frames expose events with model and world position. Authored locomotion foot cues cover walk, run, backpedal, and strafe. The [native event proof](../../../bench/verse/2026-10-04/animation-markers/README.md) retains 385 events, including 37 from respawned lives, with no duplicate identities or generation regressions. The native budget passes at 16.345 ms work p95 with 4× AA and zero dropped time. These cues do not drive gameplay authority; calibrated foot contacts, audio mixing/device output, generic animation graphs, and IK remain.

[#10491](https://github.com/OpenAgentsInc/openagents/issues/10491) adds an owned bounded spatial PCM mixer, exact-life voice cancellation, resampling, original procedural cues, and native device output. Footsteps consume animation markers; spells, impacts, and shields observe local presentation state without changing gameplay authority. The [native audio evidence](../../../bench/verse/2026-10-04/audio/README.md) includes all five cues and error-free callback output. Programmatic proof casts shields without UI input. Grounded shadow casters reuse depth under exact pose, model, life, and light invalidation; cold/cached and rebuilt pixels match. The final 100-second native run passes at 15.444 ms work p95 and 15.773 ms delivered p95 with 4× AA and zero dropped time. Audio routing measures 0.005125 ms p95. Earlier failed profiles remain retained. Generic animation graphs and IK remain separate work.

[#10494](https://github.com/OpenAgentsInc/openagents/issues/10494) starts the reusable animation graph contract: bounded serialized nodes, typed boolean/scalar parameters, authored transition priority, one-dimensional blend spaces, and skeleton-sized normal/additive masks. Local TRS evaluation precedes hierarchy composition; inactive branches and zero-weight layers are not sampled. Motion validation is shared with asset admission. Engine tests cover pose math, malformed graphs and skeletons, transition types/order, endpoint selection, and serialization. Stateful life/epoch playback, interrupted graph transitions, weighted marker policy, native compilation/integration, and native performance acceptance remain in progress.

## Reuse inventory

Review baseline: OpenAgents `843c92dd7e`. The entries below were checked against
current source and retained research; existing measurement numbers are historical,
not new benchmark results. No runtime checks were run for this documentation change.

| Existing owned code | Reuse now | Boundary or remaining gap |
| --- | --- | --- |
| [`physics`](../../../crates/physics/src/lib.rs) | Rigid bodies, forces/torques, shape manifolds, contact friction/restitution, joints, sleep, fixed steps, momentum ledgers, sensors, traces, ropes, modal response, plume forces, thruster allocation. | Keep this crate as the generic physics owner. Primitive overlap/contact and ray queries do not provide a finished character controller, mesh collision, navigation, or general time-of-impact CCD. Current contact detection enumerates collider pairs. |
| [`verse-lagrange`](../../../crates/verse-lagrange/README.md) | 120 Hz integration, versioned `StationState`, tick-stamped command patterns, interpolated presentation, seeded plume events, conservation/replay fixtures. | Keep CR3BP, Coriolis/tidal fields, station keeping, orbital time warp, EVA controls, and construction rules in the zone. Do not apply these to dungeon physics. A serializable station state is not a multiplayer database or admission system. |
| [`Physics Lab`](../physics-lab.md) | Nine live scenarios, parameter knobs, procedural geometry, debug overlays, reset/capture workflow. | Use it as Verse Engine's physics inspection fixture. Retain local-only behavior until an authority adapter is implemented. |
| [`pbr`](../../../crates/verse/src/pbr/mod.rs) | Lit materials, physical exposure conventions, sky/depth passes, shadows, irradiance baking, post-processing. | Separate generic lighting/material/camera machinery from Lagrange-specific Sun/Earth/sky and material tables. Confirm capability/budget fit before sharing between desktop and mobile. |
| [`imported`](../../../crates/verse/src/imported/mod.rs) | Project-owned textured/skinned draw submission, local lights, particles/ribbons, overlays, native video capture. | Generalize data contracts. Original builds cannot depend on `verse-wow`, benilla, M2/WMO IDs, imported font/UI files, or a private asset path. Do not discard proven GPU work merely because its first consumer used imported art. |
| [`runtime`](../../../crates/verse/src/runtime.rs), [`nav`](../../../crates/verse/src/nav.rs), and [`zones/assets`](../../../crates/verse/src/zones/assets.rs) | Shared surface lifecycle, input/camera patterns, asset admission/loading budgets, bounded 2D routing. | Preserve reusable behavior; the plaza footprint planner is not a 3D dungeon navmesh. Player movement/collision belongs behind world admission. |
| [`verse-ruins`](../../../crates/verse-ruins/README.md) | Gameplay behavior, source-parity fixtures, spell/AI observations, failure cases. | Reimplement original game rules against owned contracts. Preserve retained provenance rather than silently relabeling vendored code as new engine code. |

## Physics architecture

`crates/physics` is already the owned physics foundation. The proposed
`verse-engine-physics` boundary in the architecture means an engine-facing
module over that crate and new portable queries, not a second rigid-body solver
or a mandatory crate rename. It owns entity/body/collider lifetime mapping and
query integration; it does not own Lagrange zone rules.

Use a 30 Hz command/rules tick with four 120 Hz physics substeps as the initial
chamber profile. Keep Lagrange's established 120 Hz profile. These rates are
configuration to measure, not universal requirements. Physics runs on authority;
rendering interpolates previous/current poses. Client movement prediction uses
the same admitted solver. Report dropped/capped time; do not advance cooldowns
by a different elapsed time from the simulation that produced damage.

Keep generic physics state in double precision where already implemented. Use
explicit world/local frames and deliberate conversion to GPU float coordinates.
Large-world camera-relative rendering is a separate extension; the current L1
zone's limited extent does not demonstrate a metaverse-wide precision solution.

Add portable APIs for ray, overlap, and shape sweep queries with world/instance,
collision layer, ignored entity/life, maximum distance, and bounded results.
A sweep returns time of impact and contact normal. Distinguish blocking contacts,
triggers, selection queries, and damage volumes. A query must not mutate state.
Static mesh colliders use compiled collision geometry and an acceleration
structure; dynamic colliders retain stable IDs and deterministic ordering.

The character controller needs capsule sweeps, ground detection, gravity/jump,
stairs, slopes, wall sliding, moving-platform motion, spawn depenetration, and
safe teleport endpoints. Navigation consumes compiled walkable geometry and
dynamic blockers but does not replace collision. Projectiles sweep their traveled
segment each substep so thin walls cannot be crossed between endpoints.

Declare corpse behavior: the first chamber uses animated, grounded corpses and
colliders with explicit lifetimes. Full articulated ragdolls require skeletal
joint limits, stable collision filtering, and network/replay support and are
later work. Rope cloth-like motion, modal appendages, and dynamic props can reuse
existing mechanisms where appropriate; fluids, full cloth, and destruction are
independent capabilities, not features inferred from particle rendering.

## Stages and dependency order

| Stage | Work | Depends on | Acceptance |
| --- | --- | --- | --- |
| VE-0: contracts and reuse | Extract IDs, life generations, fixed schedules, commands/events, asset handles, and presentation snapshots; preserve existing physics owner. | Architecture review. | Headless core has no GPU/platform/private-reader dependency. Lagrange and Lab retain their existing behavior through adapters. |
| VE-1: original greybox | Compile a new chamber, new placeholder rigs, UI/font/icons, collision geometry, effects, and a scene timeline. | Asset contracts from VE-0. | Launch with WoW directories absent. Asset dependency/provenance checks pass. Creation of original art proceeds alongside physics and service work. |
| VE-2: physics and movement | Generic query contracts, static mesh acceleration, sweeps, character controller, projectile CCD, navigation, entity/body lifetime synchronization. | VE-0 and VE-1 collision fixtures. | Character stops at thin walls, climbs admitted stairs, slides on walls, handles slopes/teleports, and NPCs route around obstructions. L1 conservation and Lab contact/joint tests remain valid. |
| VE-3: rendering and animation | Generalize owned PBR/imported GPU paths, original skeletons and named state transitions, local shadowed lights, effects, HUD and native capture. | VE-0 and VE-1 packs. | Original dark-room video shows torch and spell lighting, smooth combat transitions, grounded corpses, and actual-damage numbers. No source-art dependency remains. |
| VE-4: owned local game rules | Move all ten abilities, enemy AI/casts, shields/statuses, damage/death, ritual triggers and respawn into the portable authority adapter. | VE-0 and VE-2; VE-3 for visual acceptance. | Human and controller share admission. Claude starts with 300,000 HP; every cultist respawns 60 simulated seconds after its death with a new life generation. Camera/yells are programmatic events. |
| VE-5: service, saves, multiplayer | #10406: authenticated rights, instance ownership, replication, prediction/reconciliation, inventory/items, quest/progression, transactions/checkpoints and replay. | VE-0 contracts; integrate VE-2/VE-4 incrementally. | Two players and a spectator share authoritative combat; rewards survive restart and retries without duplication; reset is instance-scoped; vmangos is absent. |
| VE-6: tools and measured scale | Scene/property/timeline editors, undo/redo, validated reload, profiling, quality tiers, streaming and mobile adapters. | Start pack inspector in VE-1; extend as contracts stabilize. | Edit content without writing game-specific renderer code, recover from failed reload/device loss, and meet measured device budgets. |

VE-1 and VE-3 do not wait for VE-5. #10407's final world acceptance depends on
#10406, while its original-content work starts in VE-1. The first comparable
original-engine video lands after VE-4 using a local authority adapter; #10406
closes only after VE-5's actual service, persistence, and multiplayer checks.

Each implementation slice needs its own claimed issue and targeted tests.
This document does not create or claim those future implementation issues.
#10425 tracks this documentation update. Existing #10406/#10407 retain their
scope and remain open until their implementation acceptance passes.

## Physics research priorities

The retained [Chaos candidate study](../../research/unreal/2026-09-27-chaos-physics-candidates.md)
is a dated candidate list, not evidence that every mechanism landed. Reconcile
each candidate with current source before creating implementation work.

1. Establish stable simulation/input/solve ordering, state checkpoints, and
   divergence traces before network resimulation. Reuse Lagrange's patterns;
   do not assume a whole-world replay guarantee from a local trace.
2. Prioritize overlap/sweep queries and static geometry for the chamber.
   Add collision-pair exemptions, trigger semantics, and generation-safe query
   filtering where the tests require them.
3. Address contact persistence/warm starting and position correction that does
   not inject physical energy when fixtures reveal drift or unstable stacks.
   Treat separation impulses and restitution thresholds explicitly. Preserve
   momentum and record external field/constraint impulses in the ledger.
4. Add a scalable broadphase, manifold persistence/hysteresis, friction anchors,
   or sleep refinements based on measured chamber/prop loads. The current
   small EVA benchmark is not a large multiplayer or dungeon benchmark.
5. Add time-of-impact handling for fast dynamic bodies and articulated joint
   features only when the playable fixtures require them. Projectile sweeps
   are already required at VE-2. GPU rigid-body solving, general fluids, and
   arbitrary cloth are later capability proposals.

Use the public papers and permissive references cited in the candidate study
for our implementations. The [Genesis roadmap](../../physics/2026-09-27-genesis-port-roadmap.md)
records existing owned work and oracle limits. The
[Lagrange realism audit](../../research/unreal/2026-09-27-lagrange-realism-audit.md)
records landed lighting/material/camera work, including explicit rejected and
unadopted features. Preserve these historical statuses rather than converting
all research candidates into approved scope.

## Accessing Unreal for further study

Epic's [source-access instructions](https://www.unrealengine.com/ue-on-github/)
were checked on October 3, 2026. Access uses a verified Epic account and a
personal GitHub account: connect GitHub under Epic's Apps & Accounts, complete
account linking and OAuth authorization, accept the applicable EULA, and accept
the emailed EpicGames organization invitation within seven days. Then the
licensed account can access `EpicGames/UnrealEngine`. Epic's
[download guide](https://dev.epicgames.com/documentation/unreal-engine/downloading-source-code-in-unreal-engine?lang=en-US)
explains branches and source downloads. The Launcher supplies an installed
editor; the GitHub route supplies source for inspection and modification.

Private clone paths already exist at `~/work/UnrealEngine` and
`~/code/UnrealEngine`; existence does not establish current remote access or
source revision. This update reads retained research and public docs only,
not the private Unreal source, and makes no new access/license assertion.

Before future source reading, follow the
[repository's Unreal study policy](../../research/unreal/AGENTS.md), including
its account/data-use condition. Keep any checkout outside tracked trees and
record the exact revision studied. Study a specific engineering gap, write an
independent technique note, find its public basis, then implement our Rust
solution from that note. No Unreal code, transliterations, shaders, Starter
Content, or MetaHuman assets enter Verse Engine. Obtaining access does not make
Unreal an engine or asset dependency of this project.

## Evidence and documentation ownership

The architecture owns stable boundaries; this roadmap owns delivery order;
#10406 owns world-service acceptance. Runtime guides describe what currently
works. Keep Lagrange, Physics Lab, and the Genesis/Unreal research notes linked
as evidence rather than duplicating historical benchmark numbers.

Required checks cover pack provenance, stale handles/life IDs, collision and
query results, conservation/energy, deterministic command replay, character
movement, physics-to-render interpolation, combat events, and restart-safe
rewards. Measure CPU simulation, GPU execution, capture encoding, asset memory,
and replication separately. Use synthetic/original fixtures and temporary
hosts; existing release gates remain reserved for releases.
