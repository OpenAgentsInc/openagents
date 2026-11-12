# What Aethyr should learn from Unreal Engine 5.9

Research note, 2026-11-11. Status: analysis only. Nothing here is implemented,
and no issues are filed; the owner decides which proposals become issues.

> **Licence rule: ideas only, never code.** Unreal Engine's source is
> licensed under the Unreal Engine EULA
> (`/Users/christopherdavid/UnrealEngine-5.9/src/LICENSE.md` points to
> <https://www.unrealengine.com/eula>). The EULA lets licensees read and
> modify the engine to make Unreal-based products. It does not let us copy,
> translate or port its code, shaders or data layouts into another engine.
> Aethyr is another engine. So: we read UE 5.9 to learn what a system does
> and why. We write our own Coax from that description, preferably also
> citing Epic's public talks and papers. We never paste, transliterate or
> closely paraphrase a function, a shader, a constant table or a file
> format. Every recommendation below was checked against this rule; see
> [Licensing](#5-licensing).

Sources read for this note (all on `origin/main` unless named):

- `docs/unreal/` (README, build-from-source, mcp-tools, ue6).
- The Aethyr book (`aethyr/book/`, especially chapters 11, 13, 14, 38, 53,
  72, 80, 83, 84, 86–102 and Appendix A), `aethyr/README.md`,
  `aethyr/render/README.md` and the module tree.
- The Verse (`docs/verse/`, especially the Solstice comparison, the lighting
  plan, the action pipeline and texture formats), Everglade (`everglade/`,
  `everglade/book/`), the netcode audits
  (`docs/audits/2026-11-10-everglade-multiplayer-audit.md`,
  `docs/audits/2026-11-11-room-loss-matrix.md`) and `coax/start-here.md`.
- The UE 5.9 source at `/Users/christopherdavid/UnrealEngine-5.9/src`, read
  only with `ls`, `grep` and `sed`. UE paths below are relative to
  `.../src/Engine/`.

The owner's rules this note is held to: no one-off zone code (features go
engine-wide or game-wide); build on existing systems; AAA MMO netcode with no
shortcuts; no shortcut phases; tools in Coax; web-first, with native Mac, iOS
and Android.

---

## 1. Executive summary

Unreal 5.9 is useful to us in two ways. Its gameplay and network frameworks
(Gameplay Ability System prediction keys, the Network Prediction plugin,
Iris, Mass, World Partition) solve problems Everglade has right now. And
its new **Wasm platform** (`Platforms/Wasm/`) is the most detailed public
account of what a AAA engine can and cannot do in a browser today. It is
also a measurement of what we avoid by being web-first.

The headline from the Wasm platform: **UE 5.9 on the web has no Nanite, no
virtual shadow maps and no hardware ray tracing.** Its own config says so
(`Platforms/Wasm/Config/DataDrivenPlatformInfo.ini`: `bSupportsNanite=false`,
`bSupportsUInt64ImageAtomics=false`, `bSupportsTextureAtomics=false`,
`bSupportsRayTracing=false`). Its software Lumen tier is experimental and
off in Preview 1. The browser shipping path is the mobile renderer. Our
renderer should aim at what works on WebGPU today, not at UE's desktop
feature list.

The ten most valuable things to adapt, ranked by value to Everglade and the
Verse now:

| # | Adapt | From | Why now | Verdict |
|---|---|---|---|---|
| 1 | **Prediction keys for every action's side effects:** one key per predicted press; slot, cooldown, resource and cue spends tagged with it; undo on reject; no double cue on catch-up | GAS `GameplayPrediction.h` | Everglade's casts already predict and roll back (audit #13157); melee, spells and resource spends still don't share one model | Adapt the idea |
| 2 | **Quantized, bit-packed replication rows with accumulated priority** and owner/field-of-view boosts | Iris (`Source/Runtime/Net/Iris`), ReplicationGraph | Rows are decimal text and every entity goes every tick (#13165); chapter 92's accumulated priority isn't built | Adapt the idea |
| 3 | **One predicted simulation model** (input, sync and aux state, one tick function) shared by walking, mounts, boats and dash | Network Prediction plugin, Mover | Walk, arrows and casts each have their own prediction path; #12845 plans the shared module | Adapt a simplified version |
| 4 | **Trace channels and a timeline viewer** with stage timers, net events and gameplay state, bound to replay | Unreal Insights, TraceLog, Rewind Debugger, ChaosVD | Chapter 83's stage timers aren't built; the 60 FPS rule and netcode bugs need "where did the time go" per frame | Adapt a simplified version |
| 5 | **Lag compensation for melee and spells**, reusing `aethyr/net/rewind.coax` | Chaos `RewindData.h`, NPP resimulation | Arrows have it (24 ticks, 250 ms); melee and spells still judge in the present (audit table) | Adapt the idea (we're already most of the way) |
| 6 | **HLOD for far cells and data layers for world state** (events, seasons, raids) | World Partition HLOD and data layers | Everglade v2 plants about 100,000 pieces; world events toggle whole sets of content | Adapt a simplified version |
| 7 | **Crowds as structure-of-arrays with LOD and representation tiers**, on the server and the page | Mass Entity, MassLOD, MassRepresentation, MassReplication | Raids put up to 64 raiders, 38 townsfolk and creatures in one square | Adapt a simplified version |
| 8 | **WebGPU delivery lessons:** pipeline prewarm lists, render bundles, no indirect dispatch in hot loops, CPU-ahead pacing, per-GPU texture sets | UE Wasm platform docs and `WebGPURHI` | First visits compile every shader; Chrome's shader cache is 6 MiB for all sites | Adapt the idea |
| 9 | **Motion matching, small and deterministic**, for locomotion starts, stops and turns | PoseSearch, UAF | ECH1 has 24 fixed clips selected by a graph; transitions are the most visible "game feel" gap | Adapt a simplified version |
| 10 | **A derived-data cache keyed by input hashes and tool version**, shared across agents and remote hosts, plus Range-streamed pack chunks | DDC, Zen, IoStore on-demand | Packers (Kokiri, Valley, page packs) rerun per worktree; packs download whole | Adapt a simplified version |

Nanite, Lumen, MegaLights and virtual shadow maps are not in the top ten.
Their web-feasible cores are already on our roadmap (the cluster DAG in
`aethyr/render/geometry/`, cached cascades in `aethyr/render/cascades.coax`,
lighting phases 3–5). Section 2 says what to take from each.

---

## 2. System by system

Each entry: what Unreal does (with UE paths), what Aethyr has (with our
paths), the gap, a verdict and why.

Verdicts: **adapt the idea**, **adapt a simplified version**, **skip**, or
**already better**.

### 2.1 WebGPU: what the browser can and can't do today

This table governs the rendering verdicts below. The "UE 5.9 web" column
comes from `Platforms/Wasm/Config/DataDrivenPlatformInfo.ini` and
`Platforms/Wasm/Docs/{SupportMatrix,Performance}.md`.

| Capability | Core WebGPU today | UE 5.9 web | Consequence for us |
|---|---|---|---|
| 64-bit atomics (buffer or image) | No | `bSupportsUInt64ImageAtomics=false` | No Nanite-style software raster with packed 64-bit depth and ID. A 32-bit `atomicMax` on a storage buffer can pack about 24 bits of depth with 8 bits of ID, enough only for small clusters in one tile |
| Storage-texture atomics | No | `bSupportsTextureAtomics=false` | Visibility buffers and page tables live in storage buffers |
| Mesh or amplification shaders | No | `bSupportsPrimitiveShaders=false` | Cluster culling runs in compute and feeds indirect draws |
| Multi-draw indirect | Not in core | n/a | One `drawIndirect` per bucket, or one instanced draw over a cluster list (what `aethyr/render/geometry/cull.coax` does) |
| Indirect dispatch | Yes, but each call is validated with extra work | `LumenWebDirectDispatch.h`: Lumen issues an upper-bound direct dispatch with early-out instead | Avoid many small indirect dispatches per frame |
| Subgroups | Optional feature | Required for the experimental desktop tier | Write every kernel with a subgroup-free path; subgroups only speed it up |
| Hardware ray tracing | No | `bSupportsRayTracing=false` | Any GI or shadow tracing is software: screen space, distance fields, voxels or heightfields |
| Compressed textures | BC, ASTC or ETC2, each optional | Dual BC/ASTC stages (`Platforms/Wasm/Skills/web-texture-sets`) | We already detect all three (`coax/web/host.js`); ship one set per GPU family |
| Timestamp queries | Optional feature | Used by `WebGPUQueries.cpp` | Stage GPU timers need a fallback when the feature is missing |
| Precompiled shaders or pipelines | No; WGSL only, compiled per visit | Prewarm lists replayed at load | See 2.20 |
| Threads | Workers plus `SharedArrayBuffer`, only on a cross-origin isolated page | wasm64, pthread pool of up to about 76 workers in Chrome | Our room tick already runs in a Worker (chapter 95) |

### 2.2 Nanite (virtualized geometry)

**What it does.** An offline builder cuts each mesh into clusters of at
most 128 triangles (`Shaders/Shared/NaniteDefinitions.h`,
`NANITE_MAX_CLUSTER_TRIANGLES`). It groups neighbours, simplifies each group
with its border locked, and splits the result into new clusters, round
after round, into a DAG. At run time, compute shaders walk the hierarchy
(`Shaders/Private/Nanite/NaniteHierarchyTraversal.ush`,
`NaniteClusterCulling.usf`). They pick the cut whose projected error is
under a pixel and cull in two passes against a depth pyramid
(`NaniteHZBCull.ush`). Small triangles go to a compute software rasterizer
that writes depth and IDs with 64-bit atomics (`NaniteRasterizer.usf`,
`NaniteWritePixel.ush`); big ones go to the hardware rasterizer. Materials
are shaded from the visibility buffer by bins (`NaniteShadeBinning.usf`).
Pages stream on demand (`NaniteStreaming.usf`). The C++ side is
`Source/Runtime/Renderer/Private/Nanite/`.

**What Aethyr has.** A general cluster DAG builder learned from Solstice
(`aethyr/render/geometry/dag.coax`: 128-triangle clusters, groups of up to
four, borders locked, half-edge collapses). Watertight page residency
(`residency.coax`: a page installs only after its parents and leaves only
after its dependents). Projected-error cut selection (`cut.coax`). GPU
cluster culling with a depth pyramid and a retest list (`cull.coax`, run by
`coax/web/cull-lab.js`). Mesh pages (`pages.coax`). Shipping realms use run
culling instead: `render/mesh-cull.coax` and `render/grid-cull.coax`. All of
the DAG work is Valley Lab only.

**The gap.** Our DAG and its rules are `#authority` and checked, which is
stronger than Nanite's guarantees. The gaps are adoption and scale: no
shipped realm draws through the DAG, there's no HTTP Range page streaming,
and there's no visibility buffer.

**Verdict: adapt a simplified version.** Keep the DAG, culling and
residency. Draw with the hardware rasterizer through instanced cluster
lists. Skip the software rasterizer until WebGPU has 64-bit atomics.
Consider a visibility buffer later for material count, not for triangle
count.

**Why.** UE itself turns Nanite off on the web, and the config comment
explains why: without 64-bit image atomics the web never draws Nanite.
Everglade's art is stylized and instanced, so its triangle counts are
moderate. Its limits are draw records and overdraw, which culling solves
without a software rasterizer. One lesson from the same config comment is
worth keeping: with Nanite on, UE's web cook wrote broken fallback meshes
(zeroed tangents, UVs and colours). Every DAG level we ship needs the same
attribute checks as level 0.

### 2.3 Lumen

**What it does.** Software ray tracing against mesh signed distance fields
and a global distance field (`Lumen/LumenMeshSDFCulling.cpp`). Hits are
lit from a surface cache of "mesh cards", small captured views of each
mesh's sides (`LumenMeshCards.cpp`, `LumenSceneCardCapture.cpp`), lit
incrementally (`LumenRadiosity.cpp`). Screen-space probes gather indirect
light (`LumenScreenProbeGather.cpp`), with a world radiance cache for far
light (`LumenRadianceCache.cpp`), ReSTIR resampling (`LumenReSTIRGather.cpp`)
and optional hardware tracing. Everything is temporal.

**What Aethyr has.** No global illumination. A hemisphere ambient with a
split for indirect occlusion and sun visibility (`render/painterly.coax`,
`paint_irradiance`), contact shading from normal and depth
(`render/contact-shade.coax`), height haze and cloud shadows. Lighting
phase 5 plans "indirect visibility and interior transitions"
(`docs/verse/lighting/README.md`).

**The gap.** No bounce light, so interiors and forest floors rely on
authored ambient and regions (`world/environment-region.coax`).

**Verdict: skip Lumen; adapt a simplified version of its pieces.** Take
two ideas. First, a low-frequency irradiance probe grid per cell, baked
offline by Coax tools and blended by the environment regions. Second, a
sky-visibility term per probe. Both are static-world approximations that
fit a painterly style.

**Why.** Even Epic's web Lumen is experimental. It needs subgroups,
clip distances and `texture-formats-tier2`, and the page never picks it
(`Docs/Performance.md`). The useful web lesson is
`LumenWebDirectDispatch.h`: WebGPU validates each indirect dispatch with a
buffer, a bind group and a validation dispatch of its own, which costs more
CPU than the dispatch. Lumen's many small temporal passes don't suit a
phone. Our look is authored, and a baked probe grid gives most of the
bounce for a fraction of the cost.

### 2.4 MegaLights

**What it does.** For many shadowed local lights, it picks a few light
samples per pixel stochastically (`MegaLights/MegaLightsSampling.cpp`,
`r.MegaLights.NumSamplesPerPixel`), traces visibility for each sample (screen
traces first, then world-space traces or hardware ray tracing:
`MegaLightsRayTracing.cpp`), and denoises spatially and temporally
(`MegaLightsDenoising.cpp`), guided by last frame's history.

**What Aethyr has.** A bounded list of local lights (`render/light.coax`),
spell lights (`fx/spell-light.coax`) and the character pass's
`SceneLights` (`render/scene-lights.coax`). Lighting phase 3 plans "shared
local lights and scalable assignment".

**The gap.** No clustered light assignment and no local-light shadows. A
raid with 16 casters can have dozens of spell lights.

**Verdict: adapt a simplified version.** Clustered forward lighting: lights
binned into screen tiles or depth slices by a compute pass, under a hard
per-cluster budget that refuses rather than truncates. Shadow only the few
most important local lights (lighting phase 4 already plans "selected local
shadows").

**Why.** MegaLights depends on temporal denoising and tracing, and we have
neither TAA nor tracing. Clustered lighting is the web-proven standard and
fits our budget rules (`budget.coax`).

### 2.5 Virtual shadow maps

**What it does.** Each light gets a 16k × 16k virtual shadow map in 128 px
pages (`VirtualShadowMaps/VirtualShadowMapArray.h`: `PageSize`,
`Level0DimPagesXY`, `VirtualMaxResolutionXY`). Directional lights use
clipmaps (`VirtualShadowMapClipmap.cpp`). Only the pages visible pixels
need are rendered. Pages are cached across frames and invalidated when
casters move (`VirtualShadowMapCacheManager.cpp`). Rendering many small
pages cheaply depends on Nanite.

**What Aethyr has.** Cached cascades in one depth atlas
(`aethyr/render/cascades.coax`, #13016). A tile redraws only when its
snapped view changes. There's one 2048 map reaching 60 m, with PCF
(`render/light.coax`, `render/shadow-filter.coax`).

**The gap.** No split between static and dynamic casters (a moving
character forces a whole tile redraw), and no page marking.

**Verdict: adapt a simplified version.** Keep cascades. Add VSM's caching
idea: a static-caster layer kept per tile, with dynamic casters drawn over
a copy each frame. Page marking (render only the shadow pages that
on-screen pixels sample) is possible in WebGPU with compute and storage
buffers. Defer it until a profile shows shadow draws dominate.

**Why.** UE's web build can't run VSMs ("they need Nanite, so shadows
render with cascaded shadow maps", `Docs/SupportMatrix.md`). The
static/dynamic split is the part of VSM that pays off without Nanite.

### 2.6 Render Dependency Graph (RDG)

**What it does.** Passes declare the resources they read and write to an
`FRDGBuilder` (`Source/Runtime/RenderCore/Public/RenderGraphBuilder.h`).
The builder culls passes whose outputs nobody reads, computes resource
lifetimes, aliases transient memory and places barriers. It also validates
the graph (`RenderGraphValidation.h`) and traces it
(`RenderGraphTrace.h`, viewed in `Plugins/RenderGraphInsights`).

**What Aethyr has.** Admitted pass operations and resources
(`render/passes.coax`, `render/resources.coax`), atomic composition of the
canonical graph (`render/runtime.coax`), a GPU resource ledger with slot,
generation and epoch (`render/registry.coax`), and device-loss policy
(`render/loss-policy.coax`). Each realm still orchestrates its own passes
(chapter 13, "Still to do").

**The gap.** No pass culling and no transient aliasing. Pass order is
hand-written per realm.

**Verdict: adapt the idea.** Make `render/runtime.coax` own the order:
passes declare reads and writes, Coax derives order, culls dead passes and
aliases transient targets. All of it is checked on the CPU.

**Why.** WebGPU places barriers itself, so half of RDG's job is moot for
us. Ordering, culling and aliasing still save memory on phones and remove
per-realm pass code, which serves the "no one-off zone code" rule.

### 2.7 World Partition, data layers, HLOD and level streaming

**What it does.** In the editor, each actor is one file. At run time a
spatial hash cuts the world into grid cells
(`Source/Runtime/Engine/Public/WorldPartition/WorldPartitionRuntimeSpatialHash.h`,
`RuntimeHashSet/`) that stream around streaming sources
(`WorldPartitionStreamingSource.h`) under a policy
(`WorldPartitionStreamingPolicy.h`). Data layers (`WorldPartition/DataLayer/`)
turn named sets of content on and off at run time. HLOD
(`WorldPartition/HLOD/`) builds merged or instanced proxy meshes that stand
in for unloaded cells, including destructible HLODs (`HLODDestruction.h`).
Level streaming (`Engine/Classes/Engine/LevelStreaming.h`) is the older
per-sublevel path.

**What Aethyr has.** Cells with residency rings, hysteresis, fixed-tick
completion, sleeping bodies, stable IDs and floating origin
(`aethyr/world/cells.coax`). Resource-cost manifests and admission
(`world/resource-*.coax`). A bounded streaming corridor on browser and
native hosts. Environment regions (`world/environment-region.coax`). Realms
as documents with typed edits (`aethyr/forge/doc.coax`, `forge/edit.coax`).
Chapter 38 is still a design draft for a whole large-world realm.

**The gap.** No far-cell proxies (HLOD), and no named runtime layers for
world state. Everglade's world events and seasons are code paths, not data.

**Verdict: adapt a simplified version.**
- *HLOD:* per cell, a baked proxy built from the cluster DAG's coarse levels
  (one draw a cell), shown past the load ring. Destructible-HLOD's idea (a
  proxy hides a part the world broke) maps onto our build sites and
  breakables.
- *Data layers:* a realm document declares named layers (a raid, winter, a
  festival). The room's authority turns them on or off, and replication
  carries the active set. Content is admitted per layer.
- *Streaming sources:* the room has one per seat, the page one per camera.

**Why.** Everglade plants about 100,000 pieces (Everglade book chapter 10).
Far forest is the obvious HLOD win. Data layers make world events
engine-wide, not zone-local, as the owner requires.

### 2.8 Mass Entity (ECS)

**What it does.** Entities live in archetypes, stored as chunks of
fragments in structure-of-arrays layout
(`Source/Runtime/MassEntity/Public/MassArchetypeTypes.h`). Processors run
queries over chunks (`MassProcessor.h`, `MassEntityQuery.h`) in an order the
dependency solver derives from declared reads and writes
(`MassProcessorDependencySolver.h`). Structural changes are deferred to a
command buffer (`MassCommandBuffer.h`), and observers react to them
(`MassObserverProcessor.h`). On top sit `Plugins/Runtime/MassGameplay/`:
LOD (`MassLOD`), representation switching between full actors, instanced
meshes and nothing (`MassRepresentation`), and replication by client
"bubbles" (`MassReplication/Public/MassClientBubbleHandler.h`).

**What Aethyr has.** Entities with slot-and-generation handles, `EntityId`s
for saves and the network, and spawns and despawns applied at the step's
flush (`aethyr/entity.coax`). Structure-of-arrays physics
(`aethyr/sim/rigid.coax`). Bounded actor profiles (`aethyr/actors/`).

**The gap.** No shared archetype storage or query layer for crowds, and no
simulation or representation LOD.

**Verdict: adapt a simplified version.** Keep `entity.coax`'s identity and
deferred flush, which is Mass's command-buffer idea already. Add
fixed-capacity SoA tables per kind (raider, townsperson, creature) with
three LOD tiers on the room: full rules at 30 Hz; coarse movement at
10 Hz; asleep. Add matching representation tiers on the page: skinned;
vertex-animated instances (an idea also in UE's `AnimToTexture` plugin);
impostor. Skip Mass's general runtime archetype composition.

**Why.** A raid is up to 64 raiders, 38 townsfolk and 16 players in one
square (chapter 92). Coax's static types make fixed per-kind tables
simpler and checkable. Dynamic archetypes buy flexibility we don't need.

### 2.9 Iris, NetDriver and ReplicationGraph

**What it does.** The classic path is `UNetDriver` (`Engine/Classes/Engine/NetDriver.h`)
with one `UActorChannel` per actor per connection. Relevancy and priority
are computed per actor per connection, which costs O(actors × connections).
**ReplicationGraph** (`Plugins/Runtime/ReplicationGraph/Source/Public/ReplicationGraph.h`)
replaces that with persistent nodes: a spatial grid, always-relevant lists
and per-connection lists, so most decisions are shared. **Iris**
(`Source/Runtime/Net/Iris/`) goes further:

- Replicated state is copied out of game objects into quantized state
  buffers with dirty tracking.
- NetSerializers do the quantization and bit packing
  (`Iris/Serialization/`: `FloatNetSerializers.h`, `BitPacking.h`).
- Filters decide who may receive what (`Filtering/NetObjectGridFilter.h`,
  `NetObjectConnectionFilter.h`).
- Prioritizers rank what's left (`Prioritization/SphereNetObjectPrioritizer.h`,
  `FieldOfViewNetObjectPrioritizer.h`,
  `SphereWithOwnerBoostNetObjectPrioritizer.h`, `NetObjectCountLimiter.h`).
- Object groups let several objects share a filter (`NetObjectGroupHandle.h`).

**What Aethyr has.** Much of the same architecture, in Coax:
- One row schema for every replicated entity, sent as deltas against the
  last acknowledged tick, with `LIFE` and `WARP` (chapter 91,
  `apps/openagents-web/everglade-wire.coax`).
- Integer field rows with an admitted delta mask (`aethyr/net/row.coax`).
- A replication lifecycle per entity and recipient
  (`net/replication.coax`).
- Per-client byte budgets and backpressure (`net/budget.coax`, #13178).
- A relevance grid (`net/grid.coax`).
- Interest with hysteresis that also gates names, events and the join
  snapshot (`coax/web/room/room-interest.coax`).
- Two transports with five delivery classes (`net/transport.coax`).

**The gap.**
- Rows are decimal text (#13165).
- Every entity goes every tick, with no per-entity rate.
- Priority doesn't accumulate for rows left out (chapter 92 says so).
- No field-of-view or owner boost.

**Verdict: adapt the idea.**
- A schema-driven binary codec: each field declares a range and a
  precision, and `row.coax` quantizes and bit-packs from that.
- Accumulated priority with starvation bounds.
- Prioritizers as small typed functions: distance, field of view, owner and
  party boost, combat target.
- Groups for party subscriptions (chapter 97's non-spatial interest).

**Why.** At 30 KB/s per seat (`WIRE_BUDGET`), text rows waste most of the
budget. Under loss, smaller datagrams also reduce the WebTransport
congestion problems the loss matrix found.

### 2.10 Network Prediction plugin, Mover and Chaos networked physics

**What it does.** The Network Prediction plugin
(`Plugins/Runtime/NetworkPrediction/readme.txt`) separates gameplay from
networking. A simulation defines InputCmd, SyncState and AuxState structs
and one `SimulationTick(input, sync, aux) -> (sync, aux)`. The plugin
handles prediction, correction, rollback and resimulation, so gameplay code
never asks `IsServer()` or `IsResimulating()`. Its buffers are in
`NetworkPredictionBuffer.h` and the per-world driver is
`NetworkPredictionWorldManager.h`. **Mover**
(`Plugins/Experimental/Mover/Source/Mover/Public/`) builds movement on it,
with movement modes, layered moves (`LayeredMove.h`), modifiers and instant
effects. **Chaos networked physics**
(`Engine/Public/Physics/NetworkPhysicsComponent.h`,
`Experimental/Chaos/Public/RewindData.h`) keeps input and state history for
physics bodies and resimulates from a corrected frame.

**What Aethyr has.**
- Server and client share one step (chapter 87).
- The page predicts its own movement, keeps 64 inputs, replays unacknowledged
  ones and eases corrections over 0.12 s (chapter 90).
- Command ordering and earned simulation time (`aethyr/net/command.coax`).
- Server-flown projectiles with lag compensation (`net/projectile.coax`,
  `net/rewind.coax`, `net/shots.coax`).

**The gap.**
- Prediction is written per feature: walking in `everglade5e-face.c` and
  `everglade-net.coax`, casts in `everglade/client/cast.coax`, arrows in their
  own path.
- Mounts, boats and dash have no shared predicted model.
- The drift check still finds too many corrections (#12778).

**Verdict: adapt a simplified version.** Add `aethyr/net/predict.coax` with
the plugin's shape: typed input, sync and aux records per model; a pure
fixed-step tick; a history ring; reconcile, replay and smooth. Walking moves
onto it first, then mounts and dash. Take Mover's layered moves (a dash, a
knockback or a root motion layered over the base mode for a set time) as the
way abilities push bodies. Skip predicted rigid-body physics for now.

**Why.** It's the "no one-off zone code" rule applied to netcode, and it is
what #12845 asks for. Coax's `#authority` checks and our deterministic math
(`aethyr/math.coax`, the same bits natively and in the browser) make
resimulation safer for us than for Unreal. Predicted rigid bodies are
expensive and Everglade doesn't need them yet.

### 2.11 Gameplay Ability System (abilities, effects, attributes, prediction keys)

**What it does.** `Plugins/Runtime/GameplayAbilities/Source/GameplayAbilities/Public/`:
- **Attributes** with base and current values (`AttributeSet.h`).
- **Gameplay effects** (`GameplayEffect.h`), instant, timed, infinite or
  periodic, whose modifiers aggregate (`GameplayEffectAggregator.h`).
- **Tags** that gate abilities.
- **Cues**, cosmetic events kept apart from state (`GameplayCueManager.h`).
- **Abilities** activated through
  `TryActivateAbility -> ServerTryActivateAbility -> ClientActivateAbility(Failed/Succeed)`.

The core of its netcode is the **prediction key** (`GameplayPrediction.h`).
The client makes a key per activation. Every side effect inside the
activation's prediction window (attribute changes, tags, cues, cooldowns)
is tagged with it. The server sends the key back only to the client that
made it. On reject the client rolls back everything tagged with that key.
On catch-up it removes its predicted copies without replaying "on applied"
cues. The header calls these the "Undo", "Redo" and "Completeness"
problems.

**What Aethyr has.**
- The action pipeline, ask, act, present (`aethyr/action/deed.coax`,
  `docs/verse/action-pipeline.md`).
- Predicted casts with rollback by request number, a 1.5 s timeout and a
  late yes (audit #13157).
- Effects as state and events as cues (chapter 94).
- SRD rules as effect programs with conditions as typed bits
  (`aethyr/rules/srd/`: `effect-lifecycle.coax`, `effect-admission.coax`,
  `condition-permission*.coax`).

**The gap.** The request number is a prediction key in all but name, but it
covers only the cast's own start. Spell-slot spends, resources, cooldowns,
predicted conditions on yourself and cues aren't tagged and rolled back as
one set. Duplicate cues on catch-up are handled per feature.

**Verdict: adapt the idea; already better on rules.** Make the key a
`deed.coax` concept: every predicted side effect records its key; rejecting
the key reverts them all; catching up drops the predicted copies and
suppresses their repeat cues. Keep our typed SRD rules over GAS's generic
attribute and tag machinery.

**Why.** It closes the class of bugs the action pipeline was written for
("the bow draws but no arrow fires") for every resource, not just the
cast. GAS's genericity exists to serve Blueprint designers; our rules are
typed, checked SRD code and need less machinery. That is why we're
already better there.

### 2.12 Chaos physics

**What it does.** A PBD rigid solver (`Experimental/Chaos/Public/Chaos/PBDRigidsEvolutionGBF.h`),
geometry collections for fracture, cloth and flesh
(`Plugins/ChaosCloth*`, `Plugins/Dataflow`), an async fixed physics tick,
resim caches (`EvolutionResimCache.h`) and the Chaos Visual Debugger
(`Plugins/ChaosVD`), which records solver frames for playback.

**What Aethyr has.**
- Rigid bodies in f64 under three named integrators (`aethyr/sim/rigid.coax`).
- PBD, SPH, MPM and grid fluids (`aethyr/sim/`).
- Kinematic capsule collision (`collide.coax`).
- A Physics Lab that measures our kernels against pinned Rapier and Jolt
  references (`aethyr/lab/`).
- Book chapters on contact as optimization and differentiable simulation.

**The gap.** Breadth (cloth on characters, fracture at scale) and speed on
large scenes.

**Verdict: already better where it matters; adapt one idea.** Our
determinism, mutation-checked kernels and reference lab beat Chaos's
correctness story. ChaosVD is a recorded-solver debugger, and with our
replay it's nearly free: the trace viewer in 2.17 should show contact and
solver state per step.

**Why.** Everglade gameplay is kinematic plus rules. Large rigid scenes are
a Physics Lab and space-game concern, not a launch one.

### 2.13 Animation: UAF (AnimNext), RigVM, Control Rig, Motion Matching, ML Deformer, MetaHuman

**What it does.**
- **UAF**, formerly AnimNext (`Plugins/Experimental/UAF/`: `UAF`,
  `UAFAnimGraph`, `UAFLayering`, `UAFWarping`, `UAFMass`): Epic's new
  data-oriented animation framework on RigVM (`Plugins/Runtime/RigVM`),
  built from trait stacks and meant to run for crowds through Mass. Mover
  binds to it (`Plugins/Experimental/MoverAnimNext`).
- **Control Rig** (`Plugins/Animation/ControlRig`): procedural rigs and IK
  as RigVM graphs.
- **Motion Matching** (`Plugins/Animation/PoseSearch`): picks the next
  pose from a database by comparing pose and future-trajectory features,
  fed by a trajectory predictor (`Mover/Public/MoverPoseSearchTrajectoryPredictor.h`).
- **ML Deformer** (`Plugins/Animation/MLDeformer`): learns a mesh's
  corrective deformation from a simulation.
- **MetaHuman** (`Plugins/MetaHuman/*`): character, animator and crowd.

**What Aethyr has.**
- A shared pose with per-joint blends and the palette (`aethyr/anim/pose.coax`).
- Layers, masks, retargeting, faces, morphs and glTF/BVH import (`aethyr/anim/`).
- A character runtime with the ECH1 locomotion graph of 24 fixed clips
  (`aethyr/character/pose.coax`).
- Fix layers: planted feet, head look and arm reach
  (`aethyr/character/fix.coax`).
- An Unreal export plugin of our own (`aethyr/tools/unreal-echo-sdk/`) and
  an Unreal source plan reader (`aethyr/anim/unreal-source.coax`).
- MetaHuman support (`docs/verse/metahuman-support.md`).

**The gap.** Locomotion picks clips by state, so starts, stops and
pivots blend generically. There's no trajectory-aware selection.

**Verdict.**
- *Motion matching:* adapt a simplified version. A per-character feature
  table (foot positions and velocities, hip velocity, three future
  trajectory points), searched brute force on the fixed step with a fixed
  tie-break, so replay stays exact.
- *Control Rig and RigVM:* skip the graph VM. Our solvers are Coax
  procedures (`fix.coax`) and agents write code more reliably than graphs.
- *UAF:* already aligned. Its data-oriented, crowd-friendly direction is
  what `aethyr/anim/` already is.
- *ML Deformer:* skip.
- *MetaHuman:* keep the private pipeline; nothing to adapt.

**Why.** Motion matching is the biggest game-feel gain per line of code for
an action MMO. A few thousand frames per character is cheap to search on
the CPU. ML Deformer needs per-asset training, and the owner's MetaHuman
rule forbids training on those assets.

### 2.14 PCG (procedural content generation)

**What it does.** Graphs of nodes over point and spatial data
(`Plugins/PCG/Source/PCG/Public/`). Generation is partitioned on a
hierarchical grid (`Grid/PCGGridDescriptor.h`, `PCGPartitionActor.h`).
Runtime generation is scheduled around streaming sources
(`RuntimeGen/PCGRuntimeGenScheduler.h`), and kernels can run on the GPU
(`Compute/PCGComputeKernel.h`). There are also shape grammars (`Grammar/`).

**What Aethyr has.** Seeded scatter on a jittered grid. Each cell's draws
are keyed by realm seed, layer ID, generator version and grid coordinates,
with keep-outs as data, placed whole or refused (`aethyr/scatter.coax`,
chapter 80). The architecture kit generates buildings and flora from
parameters (`aethyr/arch/`). Staged build sites (`world/build-site.coax`).

**The gap.** Generation runs ahead of time, not around the player at run
time, and there's no hierarchical grid (big things on a coarse grid, small
ones on a fine grid).

**Verdict: already better on determinism; adapt runtime generation.** Our
counter-based keys make every cell reproducible anywhere, which PCG has to
work for. Take two things: generation scheduled by the streaming rings
(2.7), and grid levels per layer.

**Why.** Generating near the player lets the forest grow past 100,000
pieces without shipping the placements. The keys already guarantee server
and client agree.

### 2.15 Niagara

**What it does.** A programmable particle system with CPU and GPU
simulation, data interfaces and emitter graphs (`Plugins/FX/Niagara/`).
Recent versions add **stateless emitters**
(`Niagara/Public/Stateless/`, `NiagaraShader/Private/Stateless/`): each
particle is a pure function of its spawn data and age, with no per-particle
state carried between frames.

**What Aethyr has.** Exactly that model. The puff record, budgets and 42
recipes whose motion is a pure function of seed, index and age
(`aethyr/fx/puff.coax`, `fx/abilities.coax`, chapter 72). GPU drawing
(`fx/gpu-puff.coax`, `fx/gpu-quads.coax`).

**The gap.** Effects that need state (smoke that collides, embers that
settle) have no path.

**Verdict: already better, and Epic moved our way.** If a stateful effect
is ever needed, add a small GPU state buffer per cause under the same
budgets. Skip emitter graphs.

**Why.** Stateless effects replay exactly, cost nothing to network (the
event is the cue) and fit `budget.coax`. That Epic added stateless emitters
confirms the choice.

### 2.16 Derived data cache, Zen, asset registry, cook, pak and IoStore

**What it does.**
- The **DDC** (`Source/Developer/DerivedDataCache/Public/`) stores derived
  data under keys built from input hashes and the build function's version
  (`DerivedDataBuildKey.h`, `DerivedDataBuildFunction.h`,
  `DerivedDataBuildVersion.h`), locally and in shared caches.
- **Zen** (`Source/Developer/Zen`, `Source/Programs/ZenDashboard`) is
  Epic's content-addressed storage server for that cache and for cooked
  output.
- The **asset registry** (`Source/Runtime/AssetRegistry/Public/AssetRegistry/`)
  indexes asset metadata and dependencies without loading the assets.
- The **cook** converts assets per platform.
- **IoStore** containers (`.utoc`/`.ucas`) hold chunks by ID, and
  `Source/Programs/IoStoreOnDemand` streams them over HTTP. On the web, the
  engine reads containers with Range requests and a service worker caches
  whole files (`Platforms/Wasm/Docs/Networking.md`).

**What Aethyr has.**
- Assets identified by the SHA-256 of their bytes, admitted whole or refused
  (`aethyr/assets.coax`).
- Content-addressed shader packages (`render/build.coax`,
  `render/package.coax`).
- VPK1 packs with licence checks (`aethyr/forge/pack.coax`, chapter 53).
- Resource-cost manifests (`world/resource-manifest.coax`).
- `coax/remote/pages.sh` to fetch main's built pages.

**The gap.**
- Derived outputs (Kokiri, Valley and page packs, encoded textures) aren't
  cached by input hash across worktrees or hosts, so agents rebuild them.
- Packs download whole; there's no per-chunk Range streaming.
- There's no queryable dependency index across realms.

**Verdict: adapt a simplified version.**
- A Coax derived-data cache: key = hash(inputs, tool source hash, options),
  value = output bytes, stored on the remote build hosts and read by every
  worktree.
- Chunked packs with a manifest, fetched by Range and admitted per chunk.
- A registry file that lists each asset's dependents.

**Why.** Agents use many short-lived worktrees, so a shared input-keyed
cache saves the most time. Range-chunked packs are what the cell streaming
in 2.7 needs. Our hash-first identity makes all three straightforward.

### 2.17 Unreal Insights and tracing

**What it does.** **TraceLog** (`Source/Runtime/TraceLog/Public/Trace/Trace.h`)
writes low-overhead binary events on named channels. **TraceAnalysis** and
**TraceInsights** (`Source/Developer/TraceAnalysis`, `TraceInsights*`)
show CPU and GPU timing scopes, memory, network packets, render graphs
(`Plugins/RenderGraphInsights`) and asset loading
(`Plugins/IoStoreInsights`). The **Rewind Debugger**
(`Source/Runtime/RewindDebuggerRuntimeInterface`) records gameplay state to
scrub through. Network Prediction has its own trace (`-trace=np`), and
ChaosVD records physics.

**What Aethyr has.**
- Frame statistics and the 60 FPS rule (`aethyr/frame-report.coax`).
- Every realm measured in headless Chrome (`coax/web/realm-perf.coax`) and
  frame-time tooling (`coax/web/frametime.ts`, using `timestamp-query`).
- A netcode overlay (`aethyr/net/stats.coax`, Everglade II `?net`).
- Diagnostic trace capacity (`aethyr/host/diagnostic-trace.coax`).
- Exact replay (`aethyr/replay.coax`).

Chapter 83 lists the gaps: "stage timers in the page's frame, the HUD
overlay that draws them, and the overlay's cost."

**The gap.** No per-frame timeline joining CPU stages, GPU passes, network
events and gameplay state.

**Verdict: adapt a simplified version.** A fixed-size binary trace ring per
page and per room with typed channels (frame, stage, pass, net, rules,
asset, physics), written by Coax and read by a Coax viewer page. Because
replay is exact, a trace plus its replay record *is* a rewind debugger:
scrub to tick N, replay to it, inspect state.

**Why.** Everglade's two current fights, the 60 FPS rule and netcode feel,
both need "what happened in frame N". Our replay gives us the Rewind
Debugger's hard half for free.

### 2.18 Verse (Epic's language), UEFN tools and Scene Graph

**What it does.** Verse is a functional-logic language with failure
contexts (`<decides>`), effect specifiers and transactional rollback. Its
compiler is in the tree (`Source/Runtime/VerseCompiler`,
`Source/Runtime/Solaris/uLangCore`), and C++ code gets rollback through
AutoRTFM (`Source/Runtime/AutoRTFM`). Scene Graph (entities and
components) is not in the 5.9 tree (`docs/unreal/ue6.md`). UE6 removes
Actors, Blueprints and the C++ gameplay framework in favour of Verse,
Scene Graph, MCP and Python.

**What Aethyr has.** Coax: one language for gameplay, rendering (emitted
as WGSL and SPIR-V), tools and tests, with `#effects`, `#authority`,
`#test`/`#forall` and mutation testing. Entities are `entity.coax` and
realms are documents.

**Verdict: already better, and Epic is converging on our shape.** Effect
specifiers match `#effects`. Failure contexts match our "whole or refused"
results. UE6's move to entities and components with an AI-driven tool
surface is where Aethyr already is. One thing to adopt: Verse treats
speculative execution as a language feature. We could make a
`#transacts`-style check that a predicted step writes only to state the
reconcile path can restore. That would extend 2.11's key mechanically.

**Why.** Epic is spending UE6 to reach one language and an agent-first tool
surface. We started there.

### 2.19 The editor and plugin architecture

**What it does.** Plugins are `.uplugin` descriptors with modules and
loading phases (`Source/Runtime/Projects/Public/PluginDescriptor.h`,
`ModuleDescriptor.h`, `IPluginManager.h`). The editor is extended by
subsystems and toolkits. In 5.9, an MCP server
(`Plugins/Experimental/ModelContextProtocol`) and about 50 toolset plugins
(`Plugins/Experimental/Toolsets`) expose editor functions to agents, with
tool search (`docs/unreal/mcp-tools.md`).

**What Aethyr has.**
- Aethyrforge (`aethyr/forge/`) with typed, invertible edits and undo, redo
  and replay.
- `coax/web/tool.sh drive`, which answers from the engine and names targets
  by stable IDs (`tree`, `walk-to`, `use`, `state`).
- `coax vocab` for names.
- Tools in Coax.

**Verdict: already better for agents; adapt one idea.** Epic is bolting an
agent surface onto a GUI editor. Ours is the primary surface. Take the
**toolset registry with search**: one typed catalog of every `tool.sh`
command and `drive` operation, searchable by agents, generated from the
Coax sources so it can't drift.

**Why.** Agents write most of our code. A searchable typed catalog cuts
agent context the way MCP tool search does.

### 2.20 The WebAssembly target

**What it does** (`Platforms/Wasm/`, first committed 2026-10-31; see
`KIT.md`, `Docs/*.md`, `Source/Runtime/WebGPURHI/`):
- **Build:** Emscripten 6.0.10, Dawn's WebGPU port and Tint
  (chromium/8059) patched for speed and smaller WGSL. Shaders are cooked
  to WGSL per tier.
- **Memory and threads:** wasm64 (Memory64) with a 1 GB initial and 4 GB
  maximum heap; the game on a pthread with a pool of Workers (Chrome stops
  at about 76); a cross-origin isolated page is required.
- **Renderer:** two shipping tiers on the *mobile* renderer, forward and
  deferred. The desktop tier with software Lumen is experimental and
  off. No Nanite, no VSM, no hardware ray tracing.
- **Networking:** no raw sockets. UDP datagrams travel as WebSocket messages
  to a relay (`Build/Tools/web/ws_relay.py`) that forwards real UDP. There
  are no listen servers, and HTTP is off by default.
- **Start-up:** Range reads of containers, Brotli, streaming wasm compile, a
  service worker cache and pipeline prewarm lists recorded from play
  (`-WebGPUPipelineRecord`). In one measured Lyra start, 394 shader modules
  (6.4 MB of WGSL) held frames for up to 3.6 s.
- **CPU cost of WebGPU:** render bundles record a pass's draws once and
  replay them with one validated `executeBundles`
  (`WebGPURHI/Private/WebGPURenderBundles.h`). Lumen dispatches directly
  instead of indirectly. The CPU stays at most four frames ahead of the GPU
  (`-WebGPUGpuPacingSlack`).
- **Limits found:** a Development build's code alone is about 913 MiB, and
  iOS Safari closes the tab. A large project's Test build passes 5 GB in
  macOS Safari. iOS draws the 3D scene black.
- **Agent skills shipped with the platform:** `Skills/web-deploy`,
  `web-perf-hillclimb` and `web-texture-sets`.

**What Aethyr has.**
- Web-first Coax compiled to small wasm modules per page or realm, with
  WGSL emitted from the same Coax as the CPU code.
- Rooms reached directly over WebSocket or WebTransport, with no relay
  (`net/transport.coax`).
- An offline room in a Worker (chapter 95).
- Phones as first-class targets (`aethyr/quality.coax`).
- Compressed-texture detection (`coax/web/host.js`) and texture tiers
  (`docs/verse/2026-11-07-texture-formats.md`).
- Native Metal and Vulkan hosts (`render/metal-host.coax`,
  `host/build-vulkan.coax`).

**The gap.** We don't use render bundles. We have no pipeline prewarm
list. Some passes use indirect dispatch (`coax/web/cull-lab.js`).

**Verdict: already better on architecture; adapt the delivery
techniques.** Pipeline prewarm, render bundles for static passes, direct
upper-bound dispatch where indirect validation shows up in profiles,
CPU-ahead pacing and one texture set per GPU family. Skip the UDP relay and
wasm64.

**Why.** UE's web port shows what a native-first engine pays: a 4 GB heap,
a relay for networking, the mobile renderer, and iOS that doesn't work
yet. We pay none of that. Its delivery techniques are engine-independent
browser facts measured on real hardware, and they're worth taking.

### 2.21 Automation and Gauntlet

**What it does.** The automation framework
(`Source/Runtime/AutomationTest`, `Plugins/Tests/CQTest`, functional tests
in maps) runs unit and in-world tests. **Gauntlet**
(`Source/Programs/AutomationTool/Gauntlet/`: `Framework/`, `Unreal/`,
`Platform/`) orchestrates whole sessions: build, deploy to devices, start a
server and N clients, collect logs and performance reports, and judge pass
or fail. `Plugins/NetcodeUnitTest` tests netcode at the packet level.

**What Aethyr has.**
- `#test`, `#forall` and mutation testing with zero survivors on
  `#authority` files.
- Goldens (`coax/web/golden-frames.coax`).
- Engine-level drive (`tool.sh drive`).
- Room proofs (`coax/web/room/room-proof.coax`).
- The latency harness and loss matrix (`everglade2-latency`,
  `coax/web/room/net-matrix.coax`, `net-proxy.coax`).
- Cross-realm checks (`aethyr/tests/cross-realm*.coax`).

**The gap.** Session tests are separate tools with separate reports. Real
phones aren't in the matrix.

**Verdict: already better on unit correctness; adapt Gauntlet's session
shape.** One Coax session runner: a room, N headless pages, a network
profile and a device list (desktop Chrome, a phone profile, a native host),
a scripted scenario, and one JSON report judged by declared bars.

**Why.** Mutation-checked units are rarer in industry than Gauntlet-style
sessions, and we have them. What we lack is the one command that proves
"the room plays like the game" (chapter 100) on every host.

---

## 3. Where Aethyr is already ahead or deliberately different

| Area | Unreal 5.9 | Aethyr | Why ours suits us |
|---|---|---|---|
| Language | C++ engine, Blueprints and Verse for gameplay, HLSL shaders, Python and MCP for tools; UE6 removes Blueprints and Actors to get to fewer | Coax for gameplay, shaders (WGSL and SPIR-V), tools and tests | One language agents learn once, with one checker |
| Web | A native engine ported late: wasm64, 1–4 GB heap, a UDP relay, the mobile renderer, iOS not working | Web-first; small modules; native hosts share the same Coax | Players click a link; phones are a design target |
| Correctness | Unit and functional tests; rules are code reviewed by people | `#authority` files with zero surviving mutants, `#forall`, refusal fixtures (`aethyr/refusals.coax`) | Agent-written code is checked by machine, not by trust |
| Determinism | Not a goal; network prediction resimulates within tolerance | Same bits natively and in the browser (`aethyr/math.coax`), counter-based random streams (`rand.coax`), exact replay with digests (`replay.coax`) | Replay is the debugger, the test oracle and the proof |
| Refusal | Clamp and warn, best effort | Budgets that refuse whole (`budget.coax`, `assets.coax`, every admission) | No hidden limits, as the owner requires |
| Binary size | A Development build's code is about 913 MiB (Wasm docs) | Per-realm wasm modules | First image in seconds on a phone |
| Asset identity | Paths and GUIDs, with derived-data keys beside them | SHA-256 of bytes is the identity | Caching, dedupe and integrity come free |
| Agent tooling | MCP toolsets added to a GUI editor in 5.9 | Agents are the primary authors; `drive` and `tree` answer from the engine | The tools were built for the authors we have |

---

## 4. What not to copy, and why

- **Nanite's software rasterizer.** It needs 64-bit atomics that WebGPU
  doesn't have, and UE's own web build turns Nanite off.
- **Lumen, MegaLights and hardware ray tracing as designed.** They depend on
  temporal accumulation, tracing hardware or subgroups. UE's web tier that
  has software Lumen is experimental and off by default.
- **Virtual shadow maps in full.** They depend on Nanite. Take only the
  caching idea.
- **Blueprints, or any visual scripting or graph VM** (RigVM, Control Rig
  graphs, PCG and Niagara graphs as the authoring surface). Epic is
  removing Blueprints from UE6. Agents write checked code better than
  graphs.
- **The Actor/UObject model, reflection and garbage collection.** UE6
  removes the gameplay framework built on them. Our entities and typed
  records are the shape Epic is moving to.
- **GAS's generic attributes and string tags.** Typed SRD rules are clearer
  and checkable. Take only the prediction keys.
- **The UDP-over-WebSocket relay and wasm64.** Both are workarounds for a
  native-first engine. We already speak to rooms directly, in budget.
- **Mass's dynamic archetype composition.** Fixed per-kind tables are
  simpler and checkable in Coax.
- **ML Deformer**, and any learned asset that would train on MetaHuman
  data. It conflicts with the owner's MetaHuman rule and costs too much.
- **A monolithic GUI editor.** Aethyrforge and `drive` stay the primary
  tools; any GUI is a Coax page over the same edits.
- **Any of UE's code, shaders, constant tables, file formats or sample
  content.** See section 5.

---

## 5. Licensing

**Rule: adapt ideas only. Never copy, translate or closely paraphrase UE
code.**

- The UE source is under the Unreal Engine EULA (`src/LICENSE.md` points to
  <https://www.unrealengine.com/eula>). It permits use and modification to
  build Unreal-based products. It doesn't permit moving engine code into
  another engine, and it limits distributing engine code to other
  licensees. Aethyr is another engine, and this repository is public.
- **What we may do:** read UE source to understand a technique; describe it
  in our own words (as this note does); implement it in Coax from that
  description and from Epic's public talks and papers; cite UE paths so
  readers can verify our claims.
- **What we must not do:** paste UE code or shaders; port a function line
  by line into Coax; copy constant tables, packed bit layouts or file
  formats verbatim; feed UE source to a model as training data; ship Epic
  sample content (Lyra, the Valley of the Ancient sample, Epic's MetaHuman
  assets) in Aethyr realms. `docs/verse/2026-11-05-valley-lab.md` already
  says "Do not copy Epic sample" assets; that rule stands.
- **Third-party code inside the UE tree** (Dawn, Tint, Emscripten) has its
  own permissive licences. If we want any of it, we take it from its
  upstream project under that licence, never from UE's copy.
- **Our own Unreal plugin** (`aethyr/tools/unreal-echo-sdk/`) runs inside
  Unreal to export our own data. That's ordinary EULA use. Its outputs
  must still pass the licence check in `aethyr/forge/pack.coax`.

Each recommendation, checked:

| Recommendation | What we take | Licence check |
|---|---|---|
| Prediction keys (2.11) | The concept of a key tagging predicted side effects, described in a header comment | Concept only; our implementation is in `deed.coax` |
| Quantized rows, prioritizers (2.9) | Quantization and priority as concepts, which are standard netcode practice | No serializer code or bit layouts copied |
| Predicted simulation model (2.10) | The input/sync/aux split and the pure tick, from the plugin's readme | Concept; standard rollback netcode |
| Trace ring and viewer (2.17) | Channels and scopes as concepts | Our own format; nothing from TraceLog |
| Melee and spell lag compensation (2.10, 2.11) | Bounded rewind, already ours (`net/rewind.coax`) | Ours |
| HLOD and data layers (2.7) | Proxy-per-cell and named runtime layers as concepts | Built on our cluster DAG and realm documents |
| SoA crowds with LOD (2.8) | Standard ECS and LOD practice | No Mass code |
| WebGPU delivery (2.20) | Browser facts: render bundles, prewarm, pacing, indirect validation cost | Browser API behaviour, not Epic code |
| Motion matching (2.13) | The published technique (feature vectors and trajectory search) | Our own features and search in Coax |
| Derived-data cache, chunked packs (2.16) | Content-keyed caching and Range streaming, both standard | Our formats, keyed by our SHA-256 |

---

## 6. Ten issue-sized proposals

Prioritized against the current focus, Everglade (the v2 rebuild, its
rooms and its 60 FPS rule). Each is engine-wide or game-wide, never one
zone. Not filed; the owner decides.

### P1. Prediction keys in the action pipeline

Extend `aethyr/action/deed.coax` so every predicted press carries a key.
Every predicted side effect records it: spell slot, resource, cooldown,
global cooldown, a condition on yourself, a cue.

Acceptance:
- A rejected key reverts every side effect it tagged, in one step.
  `#authority`, zero surviving mutants.
- On catch-up, predicted copies are dropped and no cue plays twice. A
  check replays an accepted cast and counts cue plays.
- Everglade II casts, melee and the Longbow all use it; their per-feature
  rollback code is deleted.
- `everglade2-latency` at 120 ms and 2% loss: no visible double cast or
  double sound in 12 trials.

### P2. Binary quantized replication rows (closes #13165)

Fields in `aethyr/net/row.coax` declare a range and a precision. The codec
quantizes and bit-packs from the schema, and the decimal text form is
removed.

Acceptance:
- Round-trip checks for every field kind; out-of-range values refuse.
  `#authority`, zero survivors.
- Bytes per seat per second at the raid workload
  (`docs/verse/2026-11-05-everglade-capacity-workloads.md`) drop by at
  least half against text, as measured by `net/stats.coax`.
- Old pages are refused with a version error, not misread.

### P3. Accumulated priority and prioritizers

In `aethyr/net/budget.coax`, a row left out gains priority each tick until
sent. Typed prioritizers cover distance, field of view, owner and party
boost, and current target.

Acceptance:
- A `#forall` check: no relevant entity waits longer than a declared bound
  under the budget.
- In a 64-raider room test, your target and party members are never more
  than two ticks stale.
- Chapter 92's status line drops its "doesn't yet accumulate" caveat.

### P4. A shared predicted-simulation module

Add `aethyr/net/predict.coax`: typed input, sync and aux records; a pure
fixed-step tick; a history ring; reconcile, replay and smooth. Walking
moves onto it, then dash and mounts. Ability pushes use layered moves.

Acceptance:
- Walking's page-side prediction code in Everglade is replaced, with
  identical replay digests on `aethyr/tests/cross-realm.coax`.
- Dash and one mount predict through it under 120 ms with 2% loss.
- The drift check (#12778) meets its bound.

### P5. Lag compensation for melee and spells

Judge melee hits and targeted spells against bodies as the attacker saw
them, reusing `aethyr/net/rewind.coax`, with the arrows' bounds (250 ms of
age, 400 ms of view).

Acceptance:
- Room checks in `room-proof.coax`: a melee swing that hit on the
  attacker's screen within the bounds lands; one past them is judged in
  the present.
- The audit table's "melee and spells still judge at the present" row
  closes.

### P6. Trace ring and a timeline viewer

A binary trace ring per page and per room with typed channels (frame,
stage, GPU pass, net, rules, asset). A Coax viewer page joins a trace with
its replay record to scrub ticks.

Acceptance:
- Stage timers appear in Everglade's frame, and the overlay costs under
  0.2 ms a frame (chapter 83's "Still to do" closes).
- GPU pass timers use `timestamp-query` where present, with a stated
  fallback.
- A room trace and a page trace from one `everglade2-latency` run line up
  on one timeline.
- Tracing off costs nothing measurable in `realm-perf.coax`.

### P7. WebGPU delivery: pipeline prewarm and render bundles

Record the pipelines a realm creates during a scripted `drive` session,
ship the list, and create them during load. Record static passes into
render bundles.

Acceptance:
- On a cold first visit, Everglade II has no frame over 50 ms from
  pipeline creation after the first image (headless Chrome, recorded).
- CPU time in WebGPU encoding drops by a measured amount on the phone
  profile.
- A missing or stale list degrades to today's behaviour, never to an error.

### P8. Far-cell HLOD from the cluster DAG

For each world cell, bake a proxy from the DAG's coarse levels: one draw,
with materials kept. Show it past the load ring of
`aethyr/world/cells.coax`.

Acceptance:
- Proxy and source agree within a declared screen error at the swap
  distance, checked by golden frames.
- Everglade's far forest draws at least 5× fewer records at the same view.
- Swaps never leave holes, using the same watertight rule as
  `residency.coax`.

### P9. Data layers for world state

A realm document declares named layers (a raid, a season, a festival). The
room turns them on or off. Replication carries the active set, and content
is admitted per layer.

Acceptance:
- Everglade's world events (`docs/verse/2026-11-03-everglade-world-events.md`)
  move from code paths to layers.
- A layer turned on mid-session shows on every seat within one interest
  update.
- A layer's content is refused whole if over budget.

### P10. Crowd tables with simulation and draw LOD

Fixed-capacity SoA tables per crowd kind, with three simulation tiers on
the room (full, coarse, asleep) and three draw tiers on the page (skinned,
vertex-animated instances, impostor).

Acceptance:
- A 64-raider plus 38-townsfolk scene holds the 60 FPS rule on the phone
  profile in `realm-perf.coax`.
- The room tick stays under its p95 budget at that workload.
- Tier changes are invisible within a declared distance band (goldens).

Motion matching (2.13), the derived-data cache and chunked packs (2.16),
clustered lights (2.4), baked probes (2.3) and the session runner (2.21)
are worth doing after these ten. They matter less to Everglade's current
focus.
