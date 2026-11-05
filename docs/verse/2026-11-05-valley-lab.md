# Valley Lab: build a valley from checked Aethyr primitives

Status: implementation plan, 2026-11-05. This change adds the plan only.

Build **Valley Lab**, an unlisted world at `/worlds/valley-lab`, where a
visitor can inspect each engine system alone and then watch those same
systems compose into a large, detailed, playable valley. The visual target
is Valley of the Ancient: monumental sandstone, dense rock detail, an open
desert basin, narrow passages, ancient structures, an animated traveller,
and eventually a giant encounter. Build the world and its engine behavior
from scratch in Coax, using shared Aethyr primitives. The local Unreal
project is a reference for scale, composition and system decomposition.

The first useful release is a walkable graybox plus a station holding one
rock. The final destination is the complete valley experience, reached by
small, independently reviewable steps rather than one scene-sized rewrite.

## What the book gives us

Read the [book construction plan](../../aethyr/book/PLAN.md) and the
individual chapters' qualified profiles before implementation. A chapter's
existence does not mean its proposed system is ready to use.

| Foundation | Current basis and work needed for this world |
| --- | --- |
| Time, replay, entities, movement, queries, camera | Chapters 1–10 provide the starting primitives. Use their fixed-step session, typed spaces, stable IDs, collision queries and camera; add valley fixtures, not another controller. |
| Renderer | [Chapter 11](../../aethyr/book/ch011-the-wgsl-renderer.md) has shared frame/pass/resource admission and a qualified Grid teaching profile. Valley needs its own real renderer adoption and evidence. |
| Surface, lighting, animation | Chapters 12–14 have shared pieces but incomplete realm adoption. Connect actual material records, lights and poses to the valley renderer. |
| Budgets and assets | Chapter 16 and [chapter 53](../../aethyr/book/ch053-bring-in-assets.md) provide content hashes, admission and the VPK1 reader/writer. Source import, reported units and other import steps remain unfinished. |
| Large worlds | [Chapter 38](../../aethyr/book/ch038-stream-a-large-world.md) is a design draft: cells, residency and asynchronous streaming must be built. Its existing distance LOD is not virtualized geometry. |
| Terrain and placement | [Chapter 80](../../aethyr/book/ch080-terrain-and-scatter-from-a-seed.md) has terrain and checked keyed scatter; realm rendering, composition and streaming integration remain. |
| Sky, effects, destruction, cinematics | Chapters 65–68 and 71–73, 78 identify the relevant branches. Several are drafts or realm-specific code awaiting shared modules. Qualify only what Valley actually adopts. |

There is no identified shared Nanite-style virtualized geometry system in
the inspected Aethyr sources. It is a new engine workstream. Existing
fragment/vertex GPU paths do not establish the compute, writable storage,
feedback or indirect submission capabilities that workstream may need.

## One world, individually inspectable stations

Use the built-in lab catalogue pattern (`splat-lab` in
`apps/openagents-web/realms.json`): planned ID `valley-lab`, `listed: false`,
direct slug access, absent from public discovery and Grid catalogue lists.
Unlisted means accessible by URL, not private. Test every discovery consumer
and the direct route. Do not change made-realm visibility or access rules.

The entry court has Coax-drawn station controls, keyboard/touch navigation,
a reset, a fixed camera route and a link to the assembled valley. Each
station also has a stable deep link, for example `#geometry` or `#assets`.
Each can load without allocating the complete valley. Switching releases
station-owned resources; shared content retains only its counted references.

Every station offers a small authored fixture, one controlled variable at
a time, reference/optimized views, and useful visual diagnostics. Show the
geometry, material and residency views in the world. Keep detailed test
receipts available in an inspector. Mark unfinished stages explicitly.
The assembled scene calls the exact shared modules the stations exercise.

| Station | What a visitor sees | Independent acceptance evidence |
| --- | --- | --- |
| **Foundations** `#walk` | Graybox basin, slope, ledge, arch and camera obstruction; run, jump, orbit, reset and replay. | Ground queries match drawn triangles; camera stays outside walls; identical recorded session digests natively and in Wasm across frame schedules. |
| **Assets** `#assets` | One rock, one pillar, a material sphere and a tiny rig; inspect source, metres, bounds, triangles, texture levels and packed bytes. | Known tiny source fixture converts to expected vertices, winding, scale and bounds; deterministic rebuild; corrupt hash, truncated input, bad index, missing licence and one-byte-over-budget cases refuse atomically. |
| **Geometry** `#geometry` | One detailed rock becoming a field of rocks; raw triangles beside cluster/LOD views, screen error, page residency and occlusion. This is “our Nanite part.” | Full-detail reference comparison, hierarchy coverage, conservative bounds, crack fixtures, cut/error bounds, missing-page fallback and measured savings; details below. |
| **Materials** `#materials` | Sandstone, sand, worn stone and metal under fixed light; channels, mips, normals and UV seams separately visible. | CPU numerical fixtures, actual GPU readback for small fixtures, fixed-view goldens and correct texture/memory accounting. Add texture-page residency here only if measured needs justify it. |
| **Light** `#light` | Sunlit arch, shaded interior, moving local light and shadow receiver; direct light, shadow and indirect components separately switchable. | Analytic normal/light fixtures, shadow boundary and bias checks, stable camera movement, GPU captures. Qualify direct light first; add a measured indirect-light method later, with its own reference. |
| **Terrain and scatter** `#terrain` | Basin, cliff kit, path and scatter layers; change seed, density and keep-outs. | Draw/query agreement, no placement in keep-outs, stable derived IDs, split-window/load-order equality, exact native/Wasm scatter digests. |
| **Streaming** `#streaming` | Walk across cells with slowed, failed and reordered loads; residency rings, page requests and retained bytes visible. | Recorded completion events replay exactly; stale callbacks cannot install; cancellation releases bytes; cell seams and collision remain safe; bounded queues and no synchronous loading in a frame. |
| **Character** `#character` | Original traveller on stairs and uneven ground; idle/walk/run/jump and blend/skeleton views. | Tiny hand-worked rig, palette/skin comparisons, clip boundary tests, replayed pose and foot/ground fixtures, body/camera consistency. |
| **Atmosphere and sound** `#atmosphere` | Fixed sky, haze, dust, wind and a spatial stone impact, separately enabled. | Fixed-seed/time pictures, depth-correct fog and particles, finite bounded emission, offline audio samples and spatial attenuation checks; muted browser automation. |
| **Breakage and encounter** `#encounter` | First one breakable pillar, then a small articulated sentinel, then the giant sequence. | Shared rigid/contact checks, stable fragment IDs, bounded rubble/effects, replayed damage and reset; encounter state machine tested without rendering. |

## Build the virtualized geometry system in visible steps

Treat Nanite as the capability reference, not a binary format to copy or a
claim of parity. Build an Aethyr cluster hierarchy and paged triangle path
from ordinary authored meshes. No Unreal runtime, Nanite data or mesh-shader
requirement is assumed. Keep skinned characters on a separately qualified
path initially. Start with opaque static geometry; alpha and deformation
need explicit later profiles.

1. **Full-detail baseline.** Generate a small faceted rock in Coax, then a
   larger original mesh. Draw indexed full detail with fixed cameras and
   retain depth, normal and shaded reference images plus frame costs.
2. **Bounded clusters.** Write a deterministic Coax offline builder with
   declared limits, initially at most 64 vertices and 124 triangles per
   leaf cluster. Check decoded triangles cover the source exactly once,
   indices are valid, and bounds enclose every vertex. Display cluster IDs.
3. **Hierarchy and simplification.** Build parent approximations, conservative
   geometric errors and preserved boundary constraints. Begin with a CPU
   hierarchy traversal and ordinary indexed draws. A selected cut covers
   each source region exactly once: no parent and child drawn together,
   no missing region. Test adjacent unequal LODs, silhouettes and thin
   structures. Quantization error is included in the published bound.
4. **Screen error and visibility.** Select cuts using projected error, with
   conservative near-plane handling and hysteresis. Add frustum culling,
   then a depth pyramid and conservative occlusion. Check against uncullled
   full detail on small exhaustive fixtures; exercise camera cuts,
   disocclusion, clipped bounds and a previously hidden moving camera.
   Invalid or stale occlusion data draws conservatively.
5. **Pages and fallback.** Pack immutable content-hashed geometry pages with
   a small always-resident root. Bound requests, decode workspace, uploads
   and residency. Until all needed child data is ready, retain a complete
   resident ancestor cut. Evict only pages unused by in-flight frames.
   Test exact capacity, slow/corrupt pages, cancellation and device loss.
6. **GPU selection where earned.** First prove a tiny Coax-authored compute
   kernel, buffer writes, synchronization and readback on the intended host.
   Then add GPU traversal/compaction and indirect draws if supported and
   beneficial. Extend shared renderer contracts and SDK effect adapters;
   keep the CPU traversal as oracle and capability fallback. Shader code
   comes from Coax. A software raster path is a separate measured decision,
   not a prerequisite silently hidden inside this milestone.
7. **Scale ladder.** Run one rock, 100, 1,000 and 10,000 instances along the
   same camera routes. Report source and selected triangles, visible
   clusters, requests, page faults, peak bytes, CPU/GPU time and full-detail
   image error. Distinguish instance reuse from genuinely unique geometry.

For a fixed 1280×800 geometry profile, start with a requested one-pixel
projected geometric error. Validate the conservative bound with reference
depth/silhouette fixtures; do not equate geometric error with a one-pixel
shading difference. Freeze image tolerances per fixture before optimizing.
Promote only when the optimized path preserves those checks and improves
measured frame cost or residency on a representative dense scene. A faster
path with holes, unstable LOD or falsely reported residency does not pass.

## Asset strategy: original first, reusable everywhere

The local `Documents/Unreal Projects/ValleyoftheAncient` and
`ValleyoftheAncient2` copies, and the Epic Launcher `AncientGame_5.7` cache,
identify useful reference groups: AncientWorld, Megascans rock/cliff kits,
MASS assemblies, the traveller and AncientBattle. The first project also
contains Medieval_Town and EchoStudy additions; those are not the base
valley target. Offloaded files and the changing second copy mean local
presence is not a complete, pinned source inventory.

Create a read-only inventory of selected reference files before comparing
them. Record hashes and availability; never let a live copy define a
reproducible build. Study composition and dimensions without requiring
Unreal to launch. The shipping path begins with original Coax-generated
rocks, terrain, pillars and a simple original character, then original or
verified permissively licensed high-detail sources. Do not copy Epic sample
assets, extracted meshes, music or characters into the repo or public packs
on the assumption their presence permits non-Unreal redistribution. Any
optional local reference assets stay local and are not a launch dependency.

Build the first importer around a deliberately small documented interchange
profile, such as indexed OBJ with explicit units and a material manifest.
Add a bounded glTF/GLB rig/clip profile when the character station needs it;
unsupported features fail with named diagnostics. Record source hash,
licence/source evidence, coordinate conversion, scale, bounds and output
hash. Coax owns import, simplification, packing and validation. Preserve the
existing VPK1 contract; use small cell packs under its limits or introduce a
new version for paged geometry rather than changing VPK1 in place.

Use shared `aethyr/assets.coax`, `aethyr/forge/pack.coax` and renderer ledger
admission. A pack is admitted whole; progressive residency is composed from
separately admitted packs/pages. Put reusable caches under `~/.openagents/`
with explicit overrides, not an app-private location. Missing optional art
must leave the procedural valley usable and clearly identify the profile.

## Construction sequence and exits

| Milestone | Deliverable and dependency | Exit before expanding |
| --- | --- | --- |
| **0. Lab shell and target** | Unlisted route, station selector, fixed routes/seeds, evidence recorder; procedural graybox and asset inventory. | Direct URL works; discovery excludes it; reset/replay and real frame admission pass. Freeze a shot list: basin overlook, cliff close-up, arch interior, corridor, arena. |
| **1. One honest rock** | Asset station, indexed baseline, material/light fixtures and import report. Depends on 0. | A single original high-detail rock draws through shared admitted resources; malformed inputs refuse; bytes, pixels and timings are recorded. |
| **2. Geometry laboratory** | Cluster builder, hierarchy, error selection, then pages and optional GPU acceleration. Depends on 1. | Each geometry step above passes alone; dense-field improvement is measured against the retained baseline. |
| **3. One valley tile** | Approximately 128 m square: ground, path, arch, cliff wall, scatter and one walker. Depends on 1 and the qualified subset of 2. | Collision matches visible ground; shared materials/light/pose actually drive the frame; no station-only implementation. All five shot types have small fixtures. |
| **4. Stream the valley** | 3×3 tiles, then a roughly 1 km basin with sparse distant terrain and dense local detail. Depends on 3 and paged geometry. | Fast/slow/failure/recovery routes meet bounded memory and frame targets; horizon and cell transitions have no holes; simulation behavior has a stated load boundary. |
| **5. Match the scene language** | Original hero-quality cliff/pillar kit, calibrated sun/sky/shadows, indirect-light experiment, dust, spatial ambience and polished traveller. Depends on 4. | Pinned shot comparisons show scale, silhouette, rock density, contact, atmosphere and character quality separately. Artistically review these; FPS alone is insufficient. |
| **6. Full valley experience** | Traversal route, quiet exploration, passage reveal, arena, sentinel/giant, breakage and cinematic/reset. Depends on 5 and isolated encounter gates. | One continuous recorded walkthrough exercises every system; reset/save/recovery preserve the declared session; all component stations still pass independently. |

Static visual reconstruction and playable encounter completion are separate
milestone receipts. Multiplayer is a later profile, not a dependency for
the initial local valley. Each milestone is a small series of commits with
working deep links; do not wait for milestone 6 to make the lab useful.

## Implementation boundaries

Planned locations, to refine against existing APIs before adding files:

- `verse/valley/`: original scene data, route, session, stations and encounter.
- `apps/openagents-web/valleylab.coax` and `gpu/valleylab.coax`: thin realm
  composition and Coax shader entry points using shared renderer contracts.
- `aethyr/render/geometry/`: reusable cluster, hierarchy, error, cut,
  visibility and page contracts; `aethyr/world/`: chapter 38's shared cells,
  streaming and origin; shared material/light/pose modules extended in place.
- `aethyr/tools/`: Coax import/build/inspection tools, invoked with
  `coax/web/tool.sh FILE.coax`; small `aethyr/examples/` teaching fixtures.
- `docs/verification/valley-lab/`: source/build hashes, profile definitions,
  test and mutation receipts, routes, captures and performance tables.

New product logic, shader arithmetic, build tools and test selection stay
in Coax. Hosts perform lifecycle, network, file and GPU SDK effects only.
All visible UI is drawn in Coax. New admission/policy files use `#authority;`
with tests written first and zero mutation survivors, without equivalent
markings to excuse failures. Resource ownership must include asynchronous
decode, upload, in-flight use, retirement and replacement generations.

## Evidence and performance contract

For each station, commit the tiny independent oracle/hand-worked fixture,
unit/property checks, real rendered capture, refusal/recovery cases and
bounded workload. Run affected checks only, with every command under 15
minutes; shard mutation runs. Never repeat a passing unchanged gate merely
for confirmation. Record the exact commit and qualified host/profile.

Use fixed seeds, camera paths, viewport, quality and content hashes. CPU
oracles validate selection and arithmetic; headless GPU readbacks validate
actual bindings and passes. Approximate shading gets declared tolerances;
simulation, asset identity and admission retain exact checks. Keep failed
originals. Avoid screenshot comparisons that merely compare a path to itself.

Initial proposed desktop profile: 1280×800, 60 Hz target, p95 frame time at
most 16.7 ms, p99 at most 33.3 ms over a pinned 60-second route after warmup.
Report cold load separately. Start with at most 256 MiB tracked CPU asset/
decode storage and 512 MiB tracked GPU residency, including transient peaks;
also measure process memory because those ledgers do not cover all memory.
Subdivide the shared GPU registry's current 1 GiB ceiling rather than raising
it silently. Freeze per-stage budgets after milestone 1 measurements and
justify any changes. Record machine, backend, queue timing availability,
unique/reused content, resolution and contention. A phone-sized viewport on
this Mac is only a viewport test; qualify a real phone separately with a
declared reduced quality/budget profile.

Test streaming with an identical accepted event record for exact replay.
Across different delivery timings, compare only the declared gameplay
contract: pin collision-critical cells ahead of the walker, or hold the
walker at an explicit safe frontier until ready. Presentation arrivals may
differ; they must not create invisible floors or arbitrary physics changes.
Keep camera interpolation out of authoritative streaming ownership decisions.

WebGPU is the first complete lab profile. Check native Coax logic alongside
Wasm from the start; qualify Vulkan/Metal rendering and lifecycle separately
as the feature set grows. Existing Grid or offscreen receipts do not qualify
Valley and do not authorize changing any desktop default. Preserve current
host contracts and version any new published interface. Exercise device
loss, late old-device completions and exact resource cleanup on the actual
lab path.

Browser checks use headless Chrome with audio muted, through the repository's
`shot` tool or its prescribed flags. Future implementation changes run the
relevant Coax suites, authority mutations, shader emission check and
`coax/web/tool.sh tools-check` before pushing. Update the relevant book
chapter, example inventory and registry only as each teaching profile becomes
real; run the Aethyr book checker when those files change. Do not upgrade a
draft's status on the strength of a roadmap.

## First implementation task

Implement milestone 0 and the procedural half of milestone 1: unlisted
Valley Lab, an empty court, a walkable graybox, a single original rock and
asset/geometry station links, shared resource accounting, one pinned replay,
one headless golden and discovery exclusion checks. Keep full-detail indexed
drawing as the honest baseline. Then build cluster partitioning with its
checks before adding hierarchy selection. This provides something visible
immediately and a concrete baseline for every later “our Nanite” claim.
