# Verse Engine architecture

Status: proposed implementation specification, October 3, 2026. This document
records the owner's direction to build Verse Engine from scratch in Rust, with
owned code and original assets. It specifies future work; it does not claim
that the engine or authoring tools described below already exist.

The first playable milestone is an original dark chamber with an adventurer,
a large monster, robed enemies, cinematic dialogue, and combat. Its distributed
build must require no WoW install, assets, fonts, UI textures, DBC tables,
vmangos SQL, or server. Original content begins alongside the engine foundation;
it does not wait for the entire MMORPG service to be complete.

## Names and product boundary

**Verse Engine** is the game engine: the reusable Rust runtime, renderer,
animation, physics, audio, content pipeline, and authoring tools. **Verse** is
the metaverse built on Verse Engine: its worlds, people, agents, places,
identity, communication, and transactions. The engine can power other games
and worlds without depending on Verse product rules or services.

Existing crate names and compatibility paths remain implementation identifiers.
The `verse` crate currently contains both application and engine facilities;
the module boundaries below describe their planned separation. Engine naming
does not rename the Verse app, metaverse, world IDs, or network identities.

## Scope and meaning of ownership

The project owns the runtime schedule, entity and scene contracts, renderer,
shaders, animation system, character movement, collision queries, navigation,
content compiler, tools, game rules, and world authority. Aeon and WoW are
references for design and behavior. Neither becomes the engine dependency or
an engine fork. The current private WoW chamber remains a separate research
fixture while the original chamber becomes the active product path.

Platform and general-purpose libraries remain explicit dependencies: Rust's
standard library, `wgpu` for portable GPU access, `winit` for desktop surfaces,
math and serialization libraries, and platform audio/device bindings. Using
these does not outsource engine architecture. Versions stay pinned and
reviewable. No new Bevy, Aeon, Rapier, or third-party ECS runtime is assumed by
this specification. A later dependency proposal must identify the behavior it
would delegate and obtain an explicit architecture decision.

Existing OpenAgents-owned GPU and runtime code can be extracted and improved.
Vendored Ruins gameplay remains a parity reference; original gameplay is
reimplemented against the new contracts. Preserve its existing license and
provenance archive. Do not rebrand copied reference code as original work.
Original assets need creator records and source files, not just renamed MPQ
exports or repainted Blizzard textures.

The [delivery roadmap](roadmap.md) inventories existing owned code, adds a
physics plan, and connects the original chamber to #10406 and #10407.

## Aeon study

Reference checkout: `~/work/projects/repos/aeon-engine`, cloned from
[`aethelisdev/aeon-engine`](https://github.com/aethelisdev/aeon-engine), pinned to
[`a004404cbf72c740913606407e0156397c737f22`](https://github.com/aethelisdev/aeon-engine/tree/a004404cbf72c740913606407e0156397c737f22).
This is a static source review. Aeon was not built, benchmarked, or executed.
The sibling checkout's instructions are not workspace instructions.

| Source inspected at that revision | Useful lesson | Verse decision |
| --- | --- | --- |
| [`Cargo.toml`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/Cargo.toml) | Separate core, rendering, physics, audio, animation, texture, editor, and launcher modules. | Define ownership and dependencies first; create crates only when the boundary has a consumer. Keep headless authority independent of desktop/editor dependencies. |
| [`ae_core/src/ecs.rs`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/crates/ae_core/src/ecs.rs) | Generational entities, dirty transform propagation, and a hierarchy are useful engine facilities. | Own entity storage and stable ordering. Validate acyclic parenting, stale handles, orphan removal, and transform changes. Sequential ECS iteration alone does not establish replay determinism across revisions. |
| [`ae_core/src/commands.rs`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/crates/ae_core/src/commands.rs) | Defer topology changes until a schedule boundary. | Use typed spawn/despawn/component operations with budgets and stable ordering. Closures over arbitrary mutable ECS state are not a serializable authority or editor transaction format. |
| [`ae_texture/src/asset.rs`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/crates/ae_texture/src/asset.rs) and [`watcher.rs`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/crates/ae_texture/src/watcher.rs) | Generational storage, deduplication, and tracked texture reloads are practical. | Separate logical asset ID, content digest, runtime handle, and source path. Compile changes in the background and swap a validated generation at a frame boundary. Shipping identity cannot be an absolute developer path. |
| [`ae_animation/src/player.rs`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/crates/ae_animation/src/player.rs) | Explicit clip time, speed, looping, and crossfade state. | Share immutable clips; blend local transforms before hierarchy evaluation. Interrupted blends start at the current pose, with death and respawn as explicit life transitions. |
| [`ae_renderer/.../frame/pipeline.rs`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/crates/ae_renderer/src/render/engine/frame/pipeline.rs) | Named shadow, scene, post-processing, and UI passes; sorted transparency and frame instrumentation. | Own a render graph with declared resource access, budgets, and capabilities. CPU `Instant` measurements around submission are CPU timings, not GPU execution timings; use GPU timestamp queries where supported. |
| [`ae_physics/src/world/core.rs`](https://github.com/aethelisdev/aeon-engine/blob/a004404cbf72c740913606407e0156397c737f22/crates/ae_physics/src/world/core.rs) | Map entity lifetimes to physics lifetimes and keep synchronization explicit. Aeon delegates its solver to Rapier. | Implement our character/collision and projectile queries first. Specify any later general rigid-body solver separately; avoid two systems integrating the same body. |
| [`ae_plugin_api`](https://github.com/aethelisdev/aeon-engine/tree/a004404cbf72c740913606407e0156397c737f22/crates/ae_plugin_api) and [`ae_plugin_host`](https://github.com/aethelisdev/aeon-engine/tree/a004404cbf72c740913606407e0156397c737f22/crates/ae_plugin_host) | Lifecycle and compatibility checks matter for replaceable modules. | Start with statically linked Rust modules. Keep renderer, editor, and gameplay types out of a universal shared API. Do not promise a stable Rust dynamic-library ABI from a version string. |
| [`ae_editor`](https://github.com/aethelisdev/aeon-engine/tree/a004404cbf72c740913606407e0156397c737f22/crates/ae_editor) | Picking, gizmos, snapping, and component inspection belong in tools. | Build scene inspection, transactions, undo/redo, and timeline editing on the runtime's own formats. Defer a general visual-programming environment and project launcher. |

Aeon's workspace patches the GPU dependency stack to an `aeongpu` branch. This
project keeps its own pinned upstream GPU stack initially; a fork needs a
specific demonstrated blocker and a maintenance plan. Aeon's source declares
MPL-2.0. This study imports no source files, shaders, assets, or plugin ABI.

## Current project baseline

The existing [`verse` runtime](../../../crates/verse/src/runtime.rs) shares
platform-independent local behavior, but rendering and game-specific adapters
still live together in a broad crate. [`imported`](../../../crates/verse/src/imported/mod.rs)
owns the chamber's geometry submission, materials, shadows, lighting, and UI.
[`verse-wow`](../../../crates/verse-wow/src/lib.rs) owns compatibility snapshots,
imported animation, and cinematic scene descriptions. [`verse-ruins`](../../../crates/verse-ruins/README.md)
wraps retained source combat. These are useful evidence and transition paths;
they are not the final engine boundary.

The local chamber copies player positions into the simulation, uses rectangular
movement bounds, and resolves encounter state in the client. Retained collision
has an unimplemented capsule-versus-OBB path. The existing multiplayer plaza
presence is distinct from authoritative combat. The researched
[#10406 plan](https://github.com/OpenAgentsInc/openagents/issues/10406#issuecomment-5973821300)
remains the world-service workstream. Original content in
[#10407](https://github.com/OpenAgentsInc/openagents/issues/10407) starts early
through a local implementation of the same authority interface, then connects
to the headless service.

## Module boundaries

Names below are proposed; no crates are added by this document.

| Module | Owns | Must not own |
| --- | --- | --- |
| `verse-engine-core` | Typed IDs, generational entities, owned component storage, transforms, schedule, clock, events, command buffers. | GPU, sockets, filesystem discovery, product rules, global state. |
| `verse-engine-assets` | Validated manifests, compiled pack readers, typed asset handles, bounded loading, residency and dependency tracking. | WoW parsing, gameplay mutations, automatic network fetches. |
| `verse-engine-render` | GPU resources, render graph, WGSL, visibility, materials, lights, shadows, effects, debug draws, capture. | Health changes, quest rewards, authority decisions. |
| `verse-engine-animation` | Skeleton/clip contracts, pose evaluation, graphs, blending, attachments, optional IK. | Damage timing or server command admission. |
| `verse-engine-physics` | Engine integration over the existing `physics` crate, spatial indices, swept collision, character integration, queries, and navigation. | A duplicate rigid-body solver, mesh decoding, or visual camera policy. |
| `verse-engine-audio` | Mixing, voices, spatial emitters, occlusion, bounded streaming. | Encounter triggers or arbitrary file access. |
| `verse-world` | Game rules, character/instance ownership, admitted commands, authoritative events and snapshots. | GPU, editor, native platform widgets. |
| `verse-world-host` | Authenticated transport, instance lifecycle, transactional persistence, backups, metrics. | Client camera and draw calls. |
| `verse-tools` | Rust content compiler, validators, inspectors, scene/timeline authoring, pack builds. | A second implementation of runtime rules. |
| `verse` and mobile adapters | Input translation, presentation, surface/audio lifecycle, session clients. | Independent combat or inventory authority. |

The existing `crates/physics` remains the owned generic solver; the proposed
physics module is an integration/query boundary. Reuse Lagrange and Physics Lab
fixtures, keeping orbital fields and EVA rules in `verse-lagrange`. The initial
30 Hz chamber authority uses four 120 Hz physics substeps, subject to measurement.

The dependency direction is platform adapter → engine systems → core, with
world rules consuming portable physics/assets contracts and exporting read-only
presentation snapshots. The headless build must compile without `wgpu`, `winit`,
editor, audio device, benilla, or private asset readers. Common semantic UI
contracts stay in `rust-native`; engine-specific GPU HUD projection belongs in
the engine presentation layer. Mobile glue follows the existing Rust-owned
state boundary.

## Runtime and authority contracts

Use meters, seconds, Y-up, a documented handedness, and one quaternion/matrix
convention. Importers perform coordinate conversion once. Scene placement,
skeleton bind poses, collision, projectiles, and cameras share the convention.
Specify texture color spaces and exposure conventions rather than relying on
format-dependent defaults.

Persistent entity identity is distinct from a generation-checked in-memory
handle and a render instance index. A respawn increments life generation;
old projectiles, statuses, and events cannot attach to the next life. Parent
references cannot cross instances. Structural edits apply at schedule barriers.
Storage can start with owned generational slots and typed component pools;
archetype packing and parallel scheduling require measured benefit.

The authoritative loop starts with a fixed 30 Hz step, independent of render
rate. Its ordered stages are admission → movement/collision → AI/encounter →
casts/projectiles/statuses → damage/death/rewards → respawn → committed events
and snapshots. Bound catch-up after pauses. Inject clock and seeded randomness;
record executable, rules, content digest, tick, input ordering, and RNG state.
Determinism is a tested guarantee within a declared profile, not an assumption
about floating-point behavior on all hardware.

Human input, local agent input, and remote agent input submit the same commands.
Commands contain intent and aim, never trusted position, damage, cost, or reward.
The authority derives cast origin and validates actor ownership, generation,
range, line of sight, resources, cooldowns, and state. Control handoff fences
queued commands with a control epoch. The local adapter and remote host return
the same accepted/rejected results, snapshots, and event shapes.

Camera, animation, sound, particles, nameplates, and floating damage numbers
consume event IDs and server time. They cannot change health. Predicted movement
reconciles to acknowledged inputs; remote actors interpolate snapshots.
Cinematic dialogue is authored timeline data and emitted programmatically.
Late join, seek, replay, and reconnect have explicit cue handling. The engine
exports observation and command APIs for agents; it needs no keyboard/chat-box
automation to stage scenes or fight.

Durable character, item, quest, progression, and instance mutations use atomic
transactions and deduplicated command receipts. Acknowledged rewards survive
restart. Checkpoints include timers, life generations, casts, projectiles,
statuses, encounter phases, spawn state, and RNG. The host owns admission and
persistence; Nostr identity/presence and Tailscale routes do not substitute for
world command authorization. See #10406 for the staged service acceptance.

## Rendering and visual requirements

Extract project-owned primitives into a renderer that accepts a read-only
`RenderWorld`: camera/view, mesh/material handles, instances, skin palettes,
lights, effect emitters, and HUD batches. It has no knowledge of Claude, WoW
spell IDs, or the specific monster rig. A capture path uses the same render
passes as interactive play and returns ungraded engine frames.

Declare pass inputs/outputs and transient resource lifetimes: upload/culling,
shadow maps, opaque/depth, sky, transparent geometry and particles, emissive
lighting, optional bloom, tone mapping, and UI. Define load/store behavior,
formats, alignment, bind-group contracts, hazard validation, and resize/device
loss recovery. Keep bind groups and pipelines persistent; bound shader variants
and retire replaced resources after safe GPU completion.

The original chamber needs:

- Authored PBR materials with albedo, normal, roughness/metallic, emissive, alpha
  mode, and deliberate sRGB/linear interpretation.
- Low ambient illumination, local shadowed torch/vessel lights, and projectiles
  and impacts that throw light on geometry and actors. Bloom enhances emissive
  sources; it does not replace illumination. Preserve dramatic dark areas while
  keeping silhouettes and readable UI.
- Frustum culling, instancing, skin palette batching, material sorting, bounded
  point-light selection, and quality profiles. Clustered lights/LOD/streaming
  follow measured bottlenecks, with portable fallback behavior.
- Authored effects combining sprites, ribbons, trails, ground projections,
  meshes, light envelopes, and sound. Store lifetime curves, deterministic
  emission seeds, blend/depth mode, and particle budgets as data. Emitters do
  not own damage resolution.
- Original HUD frames, spell icons, and typography. Keep the compact ten-ability
  layout, red enemy bars, player/target resources, cooldowns, and outlined damage
  numbers as usability requirements. Replace all imported Friz/Arial files and
  Classic textures with original or explicitly approved redistributable assets.

For animation, compile shared skeletons, inverse bind poses, clips, attachment
sockets, and named semantic states. Resolve locomotion, combat-ready, casting,
hit reaction, death, and respawn without hardcoded M2 animation numbers. Blend
local TRS before hierarchy evaluation, use quaternion interpolation, preserve
interrupted poses, and retain grounded corpses. Gameplay controls root movement;
any root-motion support must reconcile through authority. Events such as footsteps
are visual/audio markers, while spell release is a world event.

## Original content and authoring pipeline

Use editable original source assets and an owned compiled pack. glTF/GLB may be
an interchange input for meshes/rigs/clips, PNG or equivalent for source
textures, and standard audio files for authoring. Runtime data is a versioned,
validated engine format, not direct access to a creator's directory tree.
A Rust compiler resolves references and produces mesh, skeleton, material,
collision, navigation, audio, effect, scene, timeline, and UI artifacts.

Each manifest records logical ID, format/schema revision, digest, byte length,
dependencies, author, source revision, rights/license, compiler revision, and
budget limits. Stable IDs survive source-file moves. Decoder validation rejects
bad indices, hierarchy cycles, nonfinite data, missing required assets, excessive
bones/vertices/textures, and pack path escapes. Compile collision and navigation
from declared authoring geometry; decorative rendering surfaces are not
implicitly solid. Load only an admitted world pack and expose failures clearly.

The first original pack contains a new chamber layout, modular walls/floors and
stairs, light fixtures, a distinct monster, an adventurer rig, robed enemies,
a bow, ten spell icons, original combat effects, HUD/font assets, and ambient/
combat audio. Preserve the scenario's names and programmed lines where desired;
replace the visual designs, topology, UVs, textures, rigs, clips, and sound.
Use rough original meshes first, then improve art within the same contracts.
Do not postpone replacing art until every engine subsystem is optimized.

The authoring tools begin with pack validation, scene inspection, a property
panel, transform gizmos, collider/nav visualization, and a cinematic timeline.
Edits are typed transactions with undo/redo and stable asset references.
Editor preview mounts the actual engine runtime. Hot reload compiles into a new
pack generation, validates it, then swaps atomically; failed compilation keeps
the last valid generation. Authority rules changes require a profile transition
or reset, not an invisible live patch. Shader/material/camera tooling can inspect
GPU output without granting editors arbitrary gameplay authority.

## Ordered delivery and gates

| Milestone | Deliverable | Acceptance evidence |
| --- | --- | --- |
| 1. Core and original greybox | Owned IDs, schedule, scene/pack schema, local command adapter, original room/actors/UI placeholders. | Builds and runs on a machine with no WoW data; stale handles and malformed packs are refused; first original source assets have provenance. |
| 2. Owned rendering and animation | Extracted GPU pipeline, original skinned rigs, named clips, original materials and source lights. | Actual engine captures demonstrate dark-room illumination, blending, grounded deaths, and zero imported textures/fonts/models. |
| 3. Original playable encounter | Owned movement/collision, bow and ten abilities, NPC combat, 300,000-HP boss, per-cultist 60-second respawn, programmatic ritual and camera cut. | Player and controller use the same admitted actions; collisions and effects work; each respawn has a new life generation; native video uses only the original pack. |
| 4. Service and durable gameplay | #10406 authority, multiplayer admission/replication, inventory/quest/progression, persistence. | Two players plus a spectator agree on outcomes; crash/retry/reconnect do not duplicate rewards; no vmangos process or protocol dependency. |
| 5. Authoring workflow | Scene/timeline editing, undo/redo, content compiler, inspection and reload. | Change a room, enemy, ability effect, and cue, rebuild the pack, and play through the same runtime without product code changes. |
| 6. Scale and platform coverage | Streaming zones, profiling, quality tiers, desktop/mobile surface integration. | Measured frame/tick/memory/network budgets on the target machines; platform lifecycle and device-loss checks; larger worlds without special-case chamber logic. |

Milestones 1–3 deliberately make original content usable before milestone 4 is
complete. #10407's existing dependency on #10406 describes final shared-world
acceptance, not a reason to delay content creation. Track implementation work
under those workstreams and create focused engine issues as milestones begin.
This specification's documentation issue is #10423.

A new distribution/capture gate must inspect the resolved asset dependency graph,
not just filenames. Reject private WoW hashes/source provenance, MPQ/M2/WMO/BLP/
DBC inputs, Classic font/UI references, vmangos data, and importer-required paths.
Include a clean-room run with the private installation and research pack absent.
Renaming an asset cannot make it original. Existing historic research videos and
private compatibility tools remain retained and explicitly separate from the
original distribution.

Before GPU optimization, establish counters for frame passes, draw calls,
triangles, skinned actors/bones, particle/light counts, upload bytes, residency,
asset compilation, authority ticks, and replication. Initial desktop goals are
60 FPS at 1280 × 720, 30 Hz authority with p95 tick work below half the step,
and a separately measured mobile quality tier. These are targets to validate,
not current benchmark results. GPU timestamps are optional-capability data;
CPU timing and readback/encode overhead are reported separately.

## Verification and unresolved decisions

Targeted Rust tests and formatting remain the default gate for implementation.
Use synthetic and original fixture packs, temporary state, and local transports
for unit/integration checks. Native capture and multiplayer/restart checks occur
at the relevant milestone; broad release gates are for releases. No GitHub-billed
workflows are introduced.

Resolve before each affected implementation: source-asset authors and art
pipeline, first desktop/mobile GPU capability tiers, compiled pack compression,
collision solver tolerance and stair semantics, navigation representation,
audio codec/mixer boundary, persistence backend, and authenticated direct
transport. A decision records the measured need, ownership boundary, supported
platforms, and failure behavior. General rigid bodies, an arbitrary dynamic
plugin ABI, visual code generation, a general launcher, and world-scale
streaming are subsequent work rather than prerequisites for the first original
playable chamber.
