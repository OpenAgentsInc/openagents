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

[#10494](https://github.com/OpenAgentsInc/openagents/issues/10494) starts the reusable animation graph contract: bounded serialized nodes, typed boolean/scalar parameters, authored transition priority, one-dimensional blend spaces, and skeleton-sized normal/additive masks. Local TRS evaluation precedes hierarchy composition; inactive branches and zero-weight layers are not sampled. Motion validation is shared with asset admission. Engine tests cover pose math, malformed graphs and skeletons, transition types/order, endpoint selection, and serialization. Immutable admission copies skeletal motion without mesh geometry. Stateful playback preserves interrupted local poses, resets on life/graph/clock or phase-policy changes, and commits pose and marker state only after successful evaluation. A distance-driven phase can pause independently of transition time. Marker ownership follows dominant blend contributions with stable node-order ties; normal masks distribute ownership by mean weight, additive layers retain base ownership, and transition targets own cues while outgoing poses freeze. Tests cover seek/reset, replacements, weighted-source changes, interrupted blends, and atomic refusals. Version-one semantic selectors are stored in model artifacts and participate in asset/motion digests. The original compiler emits graphs for humanoids and Claude; the native renderer consumes immutable admitted graphs. Compatible reloads retain graph admissions; changed motion replaces admission while preserving playback epoch state. The [first native graph proof](../../../bench/verse/2026-10-04/animation-graphs/README.md) records 14 graph-driven actors in every frame, 428 markers, 68 respawned-life events, and no duplicate identities or generation regressions. Its frame budget fails at 17.842 ms work p95. Playback candidates now share immutable pose buffers. The subsequent 100-second native run passes at 15.034 ms work p95 and 15.323 ms delivered p95, with the same full resolution, 4× AA, 21 fireballs, and zero dropped simulation time. The retained graph-driven combat video exercises all ten abilities, shields, hostile casts, cultist respawns, grounded corpses, and adventurer defeat. Graph playback acceptance is complete; IK, root-motion admission, and service multiplayer remain separate work.

[#10496](https://github.com/OpenAgentsInc/openagents/issues/10496) removes singleton resources from the shared combat store. Up to 64 cooperative players retain independent health, mana, regeneration, and projectile-spell cooldowns in one simulation. Explicit caster APIs derive launch origins from admitted actor positions; projectiles and burns retain their caster. Cooperative players do not intercept or receive friendly arrows, homing shots, area damage, or burns. Revival cancels only that caster's effects. Shared-clock tests cover simultaneous casts, resource independence, cover and moving-target collision, revival, bounded admission, corrupt checkpoints, and exact pending-path/projectile replay. `verse-chamber-owned-v15` checkpoints carry the new schema. The existing native `Game` adapter still controls one player: per-player utility/control state, movement and encounter integration, authenticated host sessions, replication/reconciliation, inventory/quests/progression, transactional persistence, and native two-player-plus-spectator acceptance remain required for #10406.

[#10497](https://github.com/OpenAgentsInc/openagents/issues/10497) connects controlled adventurers to one chamber authority. Actor-specific admission fences ownership, life, control epoch, sequence, and ticks. Movement, jumps, cast windups, bow/utility cooldowns, shields, travel phase, and all ten original abilities use per-player state against shared NPCs, collision, spell physics, projectile sweeps, events, and the same world clock. Hostiles choose the nearest living player with stable actor-ID ties, test actual player paths, and resolve that player's shield. One dead player does not end an encounter while another lives. Shared resets and player respawns advance life/control fences. `verse-chamber-owned-v16` checkpoints retain all controlled players and replay shared combat exactly. Native effect extraction includes each player's shield/light; audio uses caster lives. The camera/HUD retains its primary-player focus. Two-player tests cover full-kit use, independent shields, movement and cast interruption, ownership/handoffs, hostile retargeting, death/revival, reset, corrupted checkpoints, and exact replay. This is trusted in-process shared authority; authenticated host admission, replication/prediction, durable inventory/quests/progression, transactional saves, and native two-player-plus-spectator service acceptance remain. Catalog spells beyond the original ten need shared-caster adapters.

[#10498](https://github.com/OpenAgentsInc/openagents/issues/10498) adds explicit chamber instance constructors. Collision, navigation, blockers, bodies, and player/NPC lives use the host-selected instance. Checkpoints and shared combat resets preserve that identity. Tests reject commands and targets from another instance with identical actor IDs, replay nonzero-instance checkpoints, and retain controlled players through reset. Local constructors keep instance zero; authenticated service admission remains required.

[#10499](https://github.com/OpenAgentsInc/openagents/issues/10499) adds the hosted chamber rights boundary. One owned game admits bounded principal grants and connection handles, derives command controllers from player rights, and provides read-only spectator snapshots. Reconnect, disconnect, revocation, respawn, and reset fence stale commands while the host alone advances one clock. Tests cover two players plus spectator, foreign principals/instances, independent shields and resources, lifecycle fences, and bounded enrollment. Authentication adapters must supply verified principals; handles are not bearer credentials. Network authentication/dispatch, durable grants, actor retirement, replication/reconciliation, and full service acceptance remain.

[#10500](https://github.com/OpenAgentsInc/openagents/issues/10500) adds opt-in signed connection verification to the hosted chamber. Enrolled identity keys prove possession through one-use, 30-second Schnorr challenges bound to the server lifetime, instance, connection, deadline, and public key. Gateway dispatch derives principal/session authority from a transport-retained handle; commands contain no authentication principal or controller. OS entropy, monotonic deadlines, bounded pending/authenticated connections, reconnect, and revocation are enforced. Synthetic signed two-player-plus-spectator tests cover replay, expiration, wrong keys/contexts, server replacement, ownership, and spectator refusals. This verifies connection identity in process; the secured network listener, wire replication, durable grants/gameplay, and native service acceptance remain.

[#10501](https://github.com/OpenAgentsInc/openagents/issues/10501) defines bounded version-one JSON opening challenges, authenticated requests, and responses. Snapshot replies bind internal combat IDs to generation-fenced actor lives. Control acknowledgments expose the current life, epoch, host tick, and highest admitted command sequence, including gameplay refusals. Cursor-based event pages report retained history gaps and bounded continuation. Signed JSON tests cover two players plus spectator, foreign commands, replayed envelopes, strict nested decoding, malformed proofs, owned respawn, and cursor gaps. Request correlation IDs do not replace command sequence fences. Secured listener integration, subscribed replication, client prediction/reconciliation, durable gameplay, and native service acceptance remain.

[#10502](https://github.com/OpenAgentsInc/openagents/issues/10502) adds the opt-in TLS-only chamber listener. Socket workers exchange bounded length-prefixed JSON through one host-owned command/tick loop. Socket/queue budgets, handshake/read/write deadlines, per-socket request limits, and bounded catch-up keep IO outside world mutation. Transport statistics retain skipped elapsed time; shutdown drains workers and parks admitted controllers while returning authority. Generated-certificate loopback tests exercise two signed players plus spectator, shared actor lives/outcomes, owned movement, reconnect/disconnect fences, shutdown cleanup, and refusal of plaintext, untrusted certificates, bad signatures, and oversized frames. This is a reusable listener API; executable/deployment configuration, subscribed replication, client prediction/reconciliation, native service integration, and durable gameplay remain.

[#10503](https://github.com/OpenAgentsInc/openagents/issues/10503) adds the verified Rust TLS chamber client. Configured server trust/name and expected instance/version are checked before signing. The client retains acknowledged control fences, derives owned input sequences, validates response context/life bindings/event cursors, and preserves consumed sequences on valid gameplay refusals. Transport/protocol failure or cancellation drops uncertain IO without automatic command replay. Real loopback tests exercise player/spectator clients, ownership, independent shields, reconnect/disconnect, certificate/instance refusal, wrong response correlation, and request cancellation. Snapshot polling is available; streamed replication, tick prediction/reconciliation, native presentation integration, durable gameplay, and full service acceptance remain.

[#10504](https://github.com/OpenAgentsInc/openagents/issues/10504) extends wire version two with shared authoritative presentation. Snapshots include actor appearances/lives, animation selection and phase, health/visibility, and per-player public shield/light/area effects from the same world tick. Clients validate finite values, unique lives, complete bindings, and bounded effects. Primary-player frame health now follows actual resources through death. Tests cover independent Run/Backpedal poses, two shields, light and Web areas, stale/duplicate lives, malformed values/budgets, zero-health death, and real TLS client effect reception. Version-one peers are refused. This supplies render data through polled snapshots; streamed replication, prediction/reconciliation, native client rendering, and durable gameplay remain.

[#10505](https://github.com/OpenAgentsInc/openagents/issues/10505) adds life-aware read-only client interpolation. Wire version three carries a persistent Misty Step stamp derived from admitted cooldown state, including short teleports followed by another cast. The replica buffer atomically validates snapshots, retains two frames and bounded generation history, interpolates compatible position/yaw/animation phase, and keeps shield anchors attached. Life/control changes, teleport stamps, death/visibility, model/animation transitions, and large displacements snap. Combat resources and statuses remain latest authoritative state. Tests cover yaw wrapping, phase/position interpolation, immutable source state, short teleports, corpse/respawn resets, stale controls/lives/ticks, invalid values, generation capacity, and stamp checkpoint retention. Streaming cadence, prediction/reconciliation, native service rendering, and durable gameplay remain.

[#10506](https://github.com/OpenAgentsInc/openagents/issues/10506) adds bounded event delivery cursors. Clients validate contiguous serials, instance/life bindings, ticks, payload bounds, and retention metadata before advancing progress. Duplicate pages emit no repeated effects; explicit gaps report missing serial ranges. Instance-scoped checkpoints retain progress without dialogue or credentials, and the TLS client exposes validated delivery. Tests cover partial/overlapping pages, gaps, reconnect checkpoints, malformed payloads, foreign instances, regressions, and real TLS delivery. Streaming cadence, native effects/audio integration, durable client storage, prediction, and full service acceptance remain.

[#10507](https://github.com/OpenAgentsInc/openagents/issues/10507) adds the Rust remote client worker. Sequential TLS IO runs outside rendering with bounded polling, input/update queues, ordered snapshot/event/outcome delivery, and backpressure. Commands refresh admitted control; shutdown cancels uncertain IO without replay. Event checkpoints accompany delivery for persistence after consumption. Real TLS tests cover player shield commands and replicas, spectator snapshots, context/cadence/queue budgets, consumer loss, and shutdown under backpressure. This is client polling infrastructure; native GPU integration, subscribed server updates, prediction/reconciliation, durable gameplay, and full acceptance remain.

[#10508](https://github.com/OpenAgentsInc/openagents/issues/10508) extends wire version four with hostile telegraph/flight presentation and transient impact flashes. Data carries caster/target lives, positions, timelines, radii, and effect kinds without a damage-execution API. Admission validates identities, finite values, active timelines, unique casters, and budgets. Tests cover extraction/serialization, malformed lives/positions/times/radii/kinds/budgets, and actual TLS spectator reception. Native rendering/lighting adapters, props/blockers, HUD, prediction, persistence, and full service acceptance remain.

[#10509](https://github.com/OpenAgentsInc/openagents/issues/10509) moves native spell instances and dynamic lighting onto shared read-only combat visuals. Local extraction and validated remote state produce the same shield/light/area, projectile, hostile, and impact values. The local app extracts once per frame for both consumers; nearest-light priorities and particle budgets remain. Remote projectile admission also validates caster bindings, unique IDs, finite values, and capacity. Tests cover local/remote equivalence, malformed projectiles, independent player effects, explosion fading, nearest lights, and deterministic particle bounds. Native transport mounting, remote HUD/props/blockers, prediction, persistence, and full service acceptance remain.

[#10510](https://github.com/OpenAgentsInc/openagents/issues/10510) projects admitted remote replicas into native scene frames. Interpolated poses retain lives, animation phases, health, and visibility; bow flights retain positions and stable directions. A validated client-owned camera supplies the view. Ordered committed events drive life-bound dialogue and pending camera handoff, with atomic malformed-batch rejection, duplicate suppression, retention-gap reporting, and respawn/reset fences. Tests cover frame projection, camera admission, event continuity/tick regressions, gaps, stale lives, resets, and real TLS worker-to-frame delivery. Native window/transport mounting, remote HUD/audio/props/blockers, cinematic camera routing, prediction, persistence, and full acceptance remain.

[#10511](https://github.com/OpenAgentsInc/openagents/issues/10511) connects committed remote damage to shared native floating text. Projection preserves actual amounts, incoming/outgoing colors, serial lanes, sampled head positions, and lethal hits, with 1.35-second expiry and exact life fences. Duplicate deliveries produce no extra numbers; respawned lives cannot inherit old text. Local and remote values use the same renderer. Tests cover amounts/colors, duplicate suppression, lethal damage, respawn, expiry, and native vertex output. Remote action/resource HUD, native mounting, audio/props/blockers, prediction, persistence, and full acceptance remain.

[#10512](https://github.com/OpenAgentsInc/openagents/issues/10512) adds wire version five player-owned HUD state. Exact-life extraction supplies health/mana, ten shared-kit readiness/cooldown gates, and cast progress from primary or additional player state. Spectators receive no owned HUD. Client/replica admission matches acknowledged control, resource snapshots, clocks, slots, and cast-target lives. Tests cover independent player mana/shield gates, cast progress, spectator omission, stale lives, malformed resources/slots/timings, foreign control, and real TLS owned HUD reception. Native HUD rendering, catalog shared-caster adapters, mounting, prediction, durable gameplay, and full acceptance remain.

[#10513](https://github.com/OpenAgentsInc/openagents/issues/10513) draws owned remote HUD values through shared native helpers. Local/remote play share the ten-slot icon row, portrait, health/mana bars and numbers, cast progress, and death/respawn button. Rendering validates the owned life against the frame; cinematic hiding and owned pointer tests exclude hidden catalog slots and gate death controls. Tests cover identical row vertices, original icons, finite geometry at two UI sizes, cast/respawn colors, hit regions, and atomic foreign-life refusal. Remote window/transport mounting, target HUD, catalog shared-caster adapters, audio/props/blockers, prediction, persistence, and full acceptance remain.

[#10514](https://github.com/OpenAgentsInc/openagents/issues/10514) adds exact-life remote target selection and shared native target frames. Stable cycling includes live visible hostile poses; friendly, foreign, and stale lives are refused. Death events clear targets before the next snapshot, while dead/hidden poses and new generations clear snapshot-based selection. Invalid snapshot admission preserves the prior target. Native local/remote portrait, name, and health drawing share one helper; dead targets are hidden and stale frame lives are refused. Tests cover cycling, lifecycle/event fences, atomic refusal, and matching target frame vertices at two UI sizes. Native mounting, cinematic camera/audio/props/blockers, catalog adapters, prediction, persistence, and full acceptance remain.

[#10515](https://github.com/OpenAgentsInc/openagents/issues/10515) adds wire version six authoritative prop poses. Live physics box lives, kinds/secured variants, dimensions, centers, and rotations are validated for instance, identity, finite values, unit rotation, and capacity. Replica sampling interpolates compatible transforms, snaps life/shape/displacement changes, drops removed props, refuses retired generations, and clears prop history on an admitted world reset. Native local/remote prop values share one model-transform path without remote physics execution. Tests cover extraction/checkpoints/removal, malformed transforms/budgets, interpolation/lifecycle/reset fences, real TLS reception, and matching native transforms. Remote blockers, mounting, camera/audio, catalog adapters, prediction, persistence, and full acceptance remain.

[#10516](https://github.com/OpenAgentsInc/openagents/issues/10516) adds wire version seven active blocker lives and bounds. Extraction omits corpse blockers and preserves authored table proxy metadata. Admission validates box bounds, identities, capacity, and retirement; replicas use the latest bounds without interpolation. Native local and remote values share model transforms without client collision authority. Tests cover extraction/removal, malformed and duplicate bounds, generation retirement, TLS reception, and native transform/table proxy parity. Native mounting, camera/audio, catalog adapters, prediction, persistence, and full acceptance remain.

[#10517](https://github.com/OpenAgentsInc/openagents/issues/10517) assembles admitted remote native scenes from one replica sample. Frames retain lives and animation phases; shield anchors follow interpolated bodies, and prop/blocker transforms, attachments, particles, and dynamic lighting share existing native helpers. Empty views wait for admission and invalid camera/interpolation/lighting inputs are refused. Tests cover sampled frame/effect alignment and local/native assembly parity. Window/transport mounting, scene/pack admission, camera/audio routing, and full service acceptance remain.

[#10518](https://github.com/OpenAgentsInc/openagents/issues/10518) mounts the authenticated worker and admitted renderer in a native window through `verse_remote`. Explicit configuration supplies TLS trust/name, enrolled signing-key file, instance, and local scene/pack paths. Bounded updates drive nameplates, programmatic dialogue, damage, owned/target HUD, and cinematic-to-follow camera. Classic controls submit shared-kit movement, jump, spells, target cycling, and respawn; movement waits for queue space without accumulating stale commands. Exit stops and joins the worker. Targeted tests cover key mapping, spectator input gates, queue pressure/disconnection, and native entry compilation. No live remote window/video acceptance is claimed. Standalone host configuration, scene/pack identity matching, audio, prediction, and full two-player acceptance remain.

[#10519](https://github.com/OpenAgentsInc/openagents/issues/10519) adds explicit bounded host configuration and the `verse_host` executable. A nonzero instance, local scene/pack, listener, DER TLS identity, and unique public-key primary/player/spectator grants construct one shared combat authority; pack prop collision is admitted before serving. Configuration refuses malformed keys, duplicate grants, multiple primaries, invalid spawns, and capacity overflow. Ctrl+C or Unix SIGTERM drains the service and reports final stats. Tests verify strict configuration and two distinct players plus a spectator over real loopback TLS, along with bounded credential reads and Unix private-key permissions. Durable saves/grants, scene/pack identity matching, deployment, and native two-player video acceptance remain.

[#10520](https://github.com/OpenAgentsInc/openagents/issues/10520) adds wire version eight scene/asset content identity to the signed opening challenge. Configured entry points hash validated scene and compiled-pack values plus bounded, manifest-verified runtime texture bytes before login. The host binds the digest before issuing any challenge; clients refuse differing or missing identity before sending authentication. Tests cover signature binding, immutable host identity, real TLS refusal with no authentication requests, matching admission, relocated asset directories, scene/manifest changes, and altered or missing textures. Portable unconfigured fixture APIs remain explicit. Live native multiplayer/video, remote audio, prediction, durable state, and deployment acceptance remain.

[#10521](https://github.com/OpenAgentsInc/openagents/issues/10521) adds bounded asynchronous recording of submitted remote GPU frames and a programmatic shared-kit controller. The live run exposed and fixed a remote cast adapter bug: host commands require horizontal directions rather than world-space aim points. Five focused adapter/content/recording tests pass. The [45-second native remote video](../../../bench/verse/2026-10-04/remote-combat/combat.mp4), [capture proof](../../../bench/verse/2026-10-04/remote-combat/combat.json), and [run receipt](../../../bench/verse/2026-10-04/remote-combat/run.json) retain 2560 × 1440 native rendering encoded at 1280 × 720/30 FPS, all ten ability command acknowledgements, 51 committed damage events, five dialogue events, and defeat. The encoder reports 1,102 sampled frames and 248 timing duplicates with no queue drops; host dropped time is zero. One primary client connected; the additional player and spectator were enrolled but not connected. This does not establish multi-client, remote audio, restart/retry, respawn interaction, corpse lifecycle, or frame-budget acceptance. Those requirements and the full roadmap remain.

[#10522](https://github.com/OpenAgentsInc/openagents/issues/10522) retains bounded authoritative NPC corpse poses after combat records retire, with death animation, zero health, and hidden nameplates. Wire version nine and replica admission fence stale generations, retired corpse reappearance, and ended-life resurrection. Service tests cover combat retirement, malformed corpses, expiry/respawn, and remote frame retention; native consumer checks compile the shared frame path. A new live corpse video and full multiplayer acceptance remain.

[#10526](https://github.com/OpenAgentsInc/openagents/issues/10526) adds opt-in programmatic remote respawn through the authenticated worker. Each dead owned life receives at most one attempt, with queue fencing and bounded history; capture proof separates attempted lives from admitted life changes. Native tests cover disabled/live/busy/retired-life gates and spectator omission. Live death/respawn and two-player-plus-spectator acceptance remain.

[#10527](https://github.com/OpenAgentsInc/openagents/issues/10527) retains live native two-player-plus-spectator combat acceptance. The run exposed request-budget contention and a cinematic radians/degrees mismatch; native replication now polls at 20 Hz alongside 30 Hz input, camera adapters preserve units, and capture encoders use bounded threads. Three TLS worker tests, nine remote-view tests, and seven native remote tests pass. Three [90-second native recordings](../../../bench/verse/2026-10-04/native-multiplayer/run.json) show both players using all ten abilities, dying, and respawning into new lives; the spectator issues no commands. All clients receive 87 damage events and agree on final player lives/health; corpse poses retain hidden nameplates. Actual 2560 × 1440 rendering is encoded at 1280 × 720/30 FPS with zero capture-queue drops and explicitly counted timing duplicates. Host dropped time is zero. This proves the shared live combat/lifecycle slice; remote audio, gameplay frame/input budgets, prediction, durable saves/rewards, inventory, quests, progression, and full VE-5 acceptance remain.

[#10528](https://github.com/OpenAgentsInc/openagents/issues/10528) adds bounded, content-bound world and enrolled-character recovery. Validated public-key grants restore existing adventurers without duplicate spawns; world lives, resources, pending casts, and corpses survive while controls park and fresh authentication replaces saved sessions. Tests cover two-player/spectator recovery, stale connections/challenges/commands, continued combat, incompatible content/instance/version, malformed ownership, and byte/grant budgets. Atomic host storage and durable mutation acknowledgment are the next dependency; rewards, inventory, quests, progression, and full VE-5 acceptance remain.

[#10529](https://github.com/OpenAgentsInc/openagents/issues/10529) wires recovery into explicit durable host storage. One writer owns bounded, versioned, checksummed snapshots; atomic replacement and file/directory sync precede mutation replies. Bounded reply groups commit at world ticks, and reads wait when mutations are pending. Startup validates saved rights, spawns, and authored content while retaining additional adventurers. Sixty-eight service tests and two host-example tests cover abrupt TLS-host restart, acknowledged mana/pending casts, stale controls, corruption/locking, and withheld replies on storage failure. The [durable-host fixture](../../../bench/verse/2026-10-04/durable-host/run.json) records 180 acknowledged zero-axis input commands for two players plus a spectator in 3.001 seconds, 95 commits, 0.876 seconds of checkpoint work, and zero dropped simulation time. Per-mutation writes had consumed 2.575 seconds; grouped commits reduce contention. This is temporary-store/process-abort evidence, not rendered combat or power-loss acceptance. Deduplicated reward/item/quest transactions, progression, prediction/reconciliation, and full VE-5 acceptance remain.

[#10531](https://github.com/OpenAgentsInc/openagents/issues/10531) adds a host-created character reward ledger to the atomic chamber save. Stable per-character source IDs return original receipts on exact retries and refuse conflicting reuse; experience, item stacks, and quest counters apply together within bounded capacities. Recovery replays validated transactions, rejects duplicate saved receipts and foreign characters/instances, and upgrades version-one saves with an empty ledger. Reset retains transactions. Hosts commit before acknowledging grants; clients cannot submit reward amounts. Authored kill/quest policies, client inventory presentation, item spending/equipment, progression rules, and full VE-5 acceptance remain.

[#10532](https://github.com/OpenAgentsInc/openagents/issues/10532) connects host-authored NPC rewards to cooperative character transactions and wire-version-ten inventory reads. Every enrolled player receives each configured NPC-life reward once; spectators receive none. Saved policy/cursor values fence recovery, generation-bound sources allow respawn/reset rewards, and cooperative batches fail without partial grants. Failed authority ticks retain the prior durable checkpoint. Inventory reads derive ownership from the connection; clients validate life, bounded counts, and nonregressing revisions. The [TLS reward receipt](../../../bench/verse/2026-10-04/combat-rewards/run.json) records 291 world tests and two host-example tests; the fixture casts Magic Missile at a synthetic one-health cultist, reads both players' rewards, aborts/restarts the host, and verifies unchanged inventories. Native inventory UI, spending/equipment, quest completion, progression rules, and full VE-5 acceptance remain.

[#10533](https://github.com/OpenAgentsInc/openagents/issues/10533) adds native remote inventory and quest windows over authenticated character state. The bounded IO worker refreshes inventory at 1 Hz and after life changes, without spectator requests. Read-only presentation validates ownership, control/life fences, and transaction continuity; stale-life counters stay hidden until the matching snapshot arrives. B/I and L toggle windows, Escape closes them first, and paging/panel clicks stay outside world input. The [GPU panel capture and receipt](../../../bench/verse/2026-10-04/character-panels/run.json) retain original ember/parchment geometry at 2700 × 1860, 81 service tests, five native consumer tests, and two panel/GPU checks. The image uses synthetic counters; live OS input acceptance remains. Quest completion, authored catalog expansion, item actions/equipment, progression rules, and full VE-5 acceptance remain.

[#10535](https://github.com/OpenAgentsInc/openagents/issues/10535) adds host-authored auto-active campaign quests, one-time completion rewards, and XP level thresholds. Authenticated claims check owned life, control epoch, and objective completion; stable receipts preserve exact retries across reset and recovery. Version-three saves validate claim transactions against immutable quest definitions, and wire version eleven carries names, progress, rewards, and levels. Native quest panels submit ready claims and refresh authoritative inventory. The [campaign receipt](../../../bench/verse/2026-10-04/campaign/run.json) retains TLS reward/restart and storage-failure checks plus synthetic ready/completed GPU captures. Level thresholds do not scale combat stats. Quest enrollment, abandonment, repeatable quests, quest givers, equipment/item use, catalog expansion, prediction/reconciliation, and full VE-5 acceptance remain.


[#10536](https://github.com/OpenAgentsInc/openagents/issues/10536) adds host-authored recovery consumables and ordered inventory debits. Current-life/current-epoch item use spends one stack and restores owned health/mana atomically; operation IDs return original receipts without duplicate restoration after retry or restart. Version-four saves validate debits against immutable catalog definitions, and wire version twelve carries authored names/effects. Native inventory Use buttons refresh authoritative state. The [item-use receipt](../../../bench/verse/2026-10-04/item-use/run.json) retains storage-failure acknowledgment checks, TLS restart evidence, and synthetic GPU layout. Equipment, visual gear, combat stat progression, wider quests, prediction/reconciliation, and full VE-5 acceptance remain.

[#10537](https://github.com/OpenAgentsInc/openagents/issues/10537) connects owned whole-outfit selection to durable character transactions and replicated presentation. Equip/Unequip preserve stacks, actor identity, resources, and control; exact retries cannot undo later selections. Version-five saves validate ordered ownership against immutable outfit definitions. Wire version thirteen carries equipped render models for all players and spectators. Native inventory emits Equip/Unequip intents; drawing selects the equipped animated rig, including its bow attachments, and refuses missing models or animation states. The [outfit receipt](../../../bench/verse/2026-10-04/outfits/run.json) retains TLS storage-failure/restart and ownership checks, synthetic panel layouts, and animated Universal-model GPU capture. Separate armor/weapon attachments, equipment stats, wider quests, prediction/reconciliation, live window input acceptance, and full VE-5 remain.

[#10538](https://github.com/OpenAgentsInc/openagents/issues/10538) adds portable equipment socket admission and frame resolution from caller-provided skinning palettes. Unique bounded IDs, exact borrowed-model identity, bone count, affine matrices, and derived overflow are checked before returning a frame. Frames retain animated rotation/scale and authored offsets without owning a clock or inventory. Universal compilation adds head/right-palm sockets 5/6; native bows consume the shared point resolver. The [socket receipt](../../../bench/verse/2026-10-04/sockets/run.json) retains engine checks, original geometric hat/wand capture on animated licensed rigs, and body/bow comparison against the prior outfit capture. Owned individual slots, gear stats, shared graph-transition palette scheduling, live combat gear switching, and full VE-3/VE-5 acceptance remain.

[#10539](https://github.com/OpenAgentsInc/openagents/issues/10539) adds owned head/main-hand equipment with authored health/mana bonuses, atomic slot transactions, explicit retry identities, and current-life/control admission. Changes preserve current resources and cooldowns; unequip clamps resources, and respawn/reset retain selected bonuses. Saved chamber version six checks derived maxima against replayed owned selections; wire version fourteen carries inventory actions and visible gear to players and spectators. Native startup admits static gear models and parent sockets, and main-hand gear is hidden during bow poses. The [equipment receipt](../../../bench/verse/2026-10-04/equipment/run.json) retains cooperative TLS/restart/storage-failure checks and synthetic native rendering/panel captures. Damage/armor modifiers, shared final graph-transition palettes, live OS gear switching, quest enrollment/givers, prediction/reconciliation, and full VE-3/VE-5/VE-6 acceptance remain.

[#10540](https://github.com/OpenAgentsInc/openagents/issues/10540) adds portable leaf mounts bound to an exact parent life, render model, and socket, with catalog admission independent of instance order. Native rendering evaluates body animation/grounding first, then resolves gear and each bow from that final palette before bounds, shadows, and color submission; the single-adventurer bow assumption and separate gear pose sampling are removed. The [mount receipt](../../../bench/verse/2026-10-04/mounts/run.json) retains two-rig interrupted transitions, alternating instance order, grounded death poses, generation fencing, and a programmatic four-second video. This is a rendering fixture, not authoritative combat, OS input, or frame-budget acceptance; other VE-3/VE-5/VE-6 requirements remain.

[#10550](https://github.com/OpenAgentsInc/openagents/issues/10550) adds authored campaign prerequisite chains. Host claims require every prerequisite receipt for the same character and instance; recovery validates ledger order, and wire version fifteen exposes availability to the native quest panel. Objective counters retain progress earned before unlocking. Quest giver/enrollment interactions, repeatable quests, progression stat scaling, prediction/reconciliation, and full VE-5 acceptance remain.

[#10554](https://github.com/OpenAgentsInc/openagents/issues/10554) adds NPC giver quests with explicit per-character enrollment. Current lives/control, living actors, four-meter range, and collision sight checks gate acceptance and first turn-in; durable baselines exclude earlier objective progress. Version-seven saves replay acceptance before claims, and wire version sixteen supplies the native Accept/Claim flow. The [enrollment receipt](../../../bench/verse/2026-10-04/quest-enrollment/run.json) retains TLS storage-failure/restart checks and synthetic GPU panel states. Exact retries preserve original receipts under current control. Quest abandonment/repeatability, friendly NPC behavior and dialogue, progression combat scaling, prediction/reconciliation, and full VE-5 acceptance remain.

[#10555](https://github.com/OpenAgentsInc/openagents/issues/10555) fixes remote quest interaction updates at an unchanged reward revision. Only giver life and interaction availability bypass quest ledger equality; counter/claim mutations remain refused. Shared giver metadata must agree, and generation checks retain unavailable lives and observe newer snapshots. The [giver view receipt](../../../bench/verse/2026-10-04/giver-view/run.json) records real TLS movement and View admission at revision zero. Friendly giver scene integration, markers/dialogue, and full roadmap acceptance remain.

[#10557](https://github.com/OpenAgentsInc/openagents/issues/10557) adds authored friendly NPC roles bound to saved simulation factions, hostile AI/target exclusions, protected health, and green native nameplates. The optional entrance variant supplies Warden Liora and two linked giver quests. The [acceptance receipt](../../../bench/verse/2026-10-04/friendly-giver-revised/run.json) records inspected native captures and real TLS enrollment/retry, prerequisite, and spectator checks. Quest markers/dialogue, foreground lighting polish, and the remaining roadmap requirements are still open.

[#10558](https://github.com/OpenAgentsInc/openagents/issues/10558) adds life-bound quest markers, authored giver dialogue, and authenticated native accept/turn-in actions; the [dressed capture receipt](../../../bench/verse/2026-10-04/giver-dressed/run.json) and [TLS quest-chain receipt](../../../bench/verse/2026-10-04/giver-dialogue/tls.json) cover both summoner objectives and retry-safe 175 XP, while foreground lighting, prediction/reconciliation, and broader roadmap acceptance remain.

[#10559](https://github.com/OpenAgentsInc/openagents/issues/10559) adds native owned-movement prediction, exact replicated collision geometry, acknowledgment reconciliation, and capture telemetry. The [first delayed-network run](../../../bench/verse/2026-10-04/prediction-delayed-first/run.json) records two moving players using all ten abilities and a read-only spectator through delayed TLS, with zero host dropped time. Acceptance fails: correction measurements mix intentional teleports with reconciliation, render-submission p95 reaches about 35 ms with three concurrent 1440p clients, and capture queues drop frames. The [traced rerun](../../../bench/verse/2026-10-04/prediction-delayed-traced/run.json) separates intentional teleports and confirms ordinary correction p95 of 2.10 m and 1.72 m; the [coalesced rerun](../../../bench/verse/2026-10-04/prediction-delayed-coalesced/run.json) lowers snapshot correction p95 to about 1.49 m for both players, retains all ten abilities, and records zero capture-queue drops. The [fresh-control run](../../../bench/verse/2026-10-04/prediction-delayed-fresh/run.json) does not establish a snapshot-correction improvement and exposes local retirement correction p95 of 0.21 m, with maxima up to 0.83 m; host dropped time is 0.041 seconds. Superseded local-history retention, movement latency, render budgets, lifecycle fixtures, and full VE-5 acceptance remain.

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
