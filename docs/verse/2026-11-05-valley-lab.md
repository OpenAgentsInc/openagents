# Valley Lab: build a valley from checked Aethyr primitives

Status: active implementation roadmap, updated 2026-11-06. The initial
walkable world, procedural asset/indexed baseline, bounded leaves, ownership
hierarchy, parent simplification, screen-error cuts and HTTP page stations now
run locally, including adjacent nonplanar grid parents and a deterministic
six-rock field with shared HTTP pages. The isolated `0 SCANS` station now also
loads two exact Quixel-ID matches from a private local cache, through Coax
download/import, SHA-256 admission and the same bounded GPU path. A dedicated
Canyon Pair view also reconstructs two actual Ancient World instances using
all 1,999,990 source triangles, original 4K maps and recovered world poses.
That section now opens with the shared first-person walker, a metre-grid
inspection floor and conservative source-bounds collision.

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

## Landed: nine original world cells with Echo (2026-11-06)

`N WORLD` opens `/worlds/valley-world?pack=valley-world`, a roughly 250 metre
Ancient World slice: 27,501 original placements, 130 fallback meshes, 90 materials
and 240 texture layers including Echo. The four basin/west/east/north waypoints,
keyboard and touch movement, jump, source-ground reset and material diagnostics
make the composition navigable. The original Valley Echo body rig and clips
remain separate from the Slay performance inspector on `P`.

The pack is private and SHA-256 pinned. Geometry, UVs, indices, placements,
component overrides and resized source pixels have an independent oracle.
The renderer ports the recovered color, normal and S-curve roughness settings;
it does not yet reproduce Unreal's complete lighting/material pipeline or
Nanite source. Collision uses a bounded one-metre sampled top surface with
explicit holes, rather than exact triangles and overhangs. The full source and
verification scope is in the [world slice receipt](../verification/valley-lab-world-2026-11-06/README.md).

## Landed: graybox, one rock and its full-detail baseline

The initial controller/graybox landed in `640adf8deb`. The next slice adds
three actual stations using the same original source asset:

- `#walk`: the basin, character, shared walking/collision/follow camera and
  one rock beside the path. The rock uses its measured AABB as an explicit
  conservative collision proxy; exact triangle collision is still future work.
- `#assets`: a stationary orbit view, original source/licence, measured bounds
  in metres (HUD dimensions in millimetres), vertex/triangle counts, canonical
  byte count and SHA-256. Lit, albedo, normal, triangle-edge and 0–20 m view-depth
  modes inspect separate parts of the asset. The full hex identity is shown.
- `#geometry`: the same mesh with triangle edges and its measured bounds.
  `L` cycles 8, 16 and 32 subdivisions: 768, 3,072 and 12,288 triangles.
  These are independently generated full-detail tessellations, not an LOD
  hierarchy. Positions/normals stay shared; the vertex shader pulls their
  original indices from a separate GPU storage buffer. This uses ordinary
  triangle draws with index pulling, not a new host `drawIndexed` interface.

`1`/`2`/`3` and the touch tabs select stations; `V` cycles inspection views;
`L` changes detail; drag/arrows orbit, wheel zooms, and reset restores the
view. Station changes clear held input and freeze walking while inspecting.
Only one rock identity remains retained after a detail replacement.

The reusable admission primitive is
[`aethyr/render/geometry/indexed.coax`](../../aethyr/render/geometry/indexed.coax):
finite positions, matching near-unit normals, nonempty triangle-aligned
indices, valid index ranges, positive triangle area, provenance and byte
limits. Its authority root mutation gate has zero survivors. The original
source is [`verse/valley/rock.coax`](../../verse/valley/rock.coax), which uses
shared random streams, material admission and lighting functions, plus the
existing content-hash asset registry. Canonical bytes include generator
version, seed, subdivision count, little-endian f32 geometry and u32 indices.
The source and generated art use the repository's Apache-2.0 licence.

| Tessellation | Vertices, including face-seam duplicates | Triangles | Canonical bytes |
| --- | --- | --- | --- |
| 8 | 486 | 768 | 20,904 |
| 16 (default) | 1,734 | 3,072 | 78,504 |
| 32 | 6,534 | 12,288 | 304,296 |

Generate the identical native assets and import-style provenance report with
`coax/web/tool.sh aethyr/tools/valley-rock.coax OUTPUT_DIRECTORY`.
Browser tests compare all three hashes to the native writer and check actual
GPU readback differences between inspection views. Test/build records and
pictures are in the [rock slice receipt](../verification/valley-lab-rock-2026-11-05/README.md).

Milestone 0's playable shell and milestone 1's procedural asset/indexed
baseline are implemented. Their remaining exits include a saved replay/fixed
walkthrough, external source import with unit conversion, broader material/
light fixtures and a qualified shared GPU renderer profile. The current
renderer remains explicitly tracked on the initial scene/HUD path. No GPU
ledger/native-host parity, hero-quality imported art, texture streaming,
cluster hierarchy, error-based LOD or Nanite parity is claimed by this slice.
That slice used a 6 MB arena and 64,000 submitted-vertex budget including
index traffic, with a 65,536-vertex HUD buffer. The field slice below raises
those declared capacities together after measuring its larger submission.

## Landed: bounded leaf clusters

The next slice builds source-order leaves over the same admitted indexed rock.
[`clusters.coax`](../../aethyr/render/geometry/clusters.coax) owns the reusable
atomic builder: at most 64 local vertices and 124 triangles per leaf, with a
512-leaf physical ceiling. Each leaf carries its source-vertex remap, local
u8 triangle indices, source triangle range and conservative position bounds.
A complete candidate is built privately; malformed input or capacity refusal
leaves the previously published result untouched. This structural partitioner
accepts repeated corners; mesh admission remains the indexed validator's job.

| Tessellation | Leaves | Vertices across leaves | Largest leaf vertices / triangles |
| --- | --- | --- | --- |
| 8 | 9 | 556 | 64 / 97 |
| 16 | 36 | 2,286 | 64 / 90 |
| 32 | 204 | 13,056 | 64 / 62 |

`#clusters` opens the geometry inspector in cluster-ID mode. `C` switches
between decoded leaves and the original reference path; entering from WALK
opens the inspector. `V` cycles six views, including cluster IDs, and can
shade the decoded leaves normally for a reference comparison. Phone VIEW
also reaches cluster IDs. The renderer uploads each leaf's local vertices
and pulls its local indices, so colors identify actual independently remapped
leaves. The HUD reports live leaf/remapped-vertex counts and limits. Source
bytes, hashes, normals and triangle ordering remain unchanged.

This first partitioner is deliberately deterministic and source ordered.
It duplicates boundary vertices and offers no reduction in triangle count;
the high profile duplicates about twice the original vertices. It is a
correctness baseline, not a spatially optimized meshlet builder. It introduces
no hierarchy, simplification, culling, LOD, paging or Nanite parity claim.
The initial renderer adoption remains in place. Checks cover tiny hand-worked
meshes, both exact leaf boundaries, atomic refusal, all three rock profiles,
every decoded source corner, every bound and submitted buffer budgets.
The authority mutation gate kills all 124 mutants. Actual WebGPU checks
compare the decoded lit reference patch with the original path and inspect
cluster-ID pixels on desktop/phone viewports.

Run the native builder/report with
`coax/web/tool.sh aethyr/tools/valley-clusters.coax`.
Evidence is in the [cluster slice receipt](../verification/valley-lab-clusters-2026-11-05/README.md).

## Landed: ownership hierarchy and boundary-preserving fixtures

[`hierarchy.coax`](../../aethyr/render/geometry/hierarchy.coax) builds a bounded
binary ownership tree above the existing leaves. Every parent owns consecutive
children, sums their triangles and unions their bounds. An odd tail is promoted
without duplicating it. Height-limited frontiers partition the leaves exactly
once, including uneven trees. These are identity parents: they retain the
original rock triangles and have zero geometric error. The reusable authority
refuses malformed leaf metadata or insufficient node capacity atomically.

| Rock tessellation | Leaves | Tree nodes | Root height | Triangles at every level |
| --- | --- | --- | --- | --- |
| 8 | 9 | 17 | 4 | 768 |
| 16 | 36 | 71 | 6 | 3,072 |
| 32 | 204 | 407 | 8 | 12,288 |

`#hierarchy` opens the new inspection view. `H` cycles levels; `F` cycles
rock, planar patch and bent patch. Parent-group colors come from actual tree
ownership. Yellow bars show the first frontier group's bounds on the
rock, or the patch's original bounds. The HUD reports level, groups, source
and drawn triangles, and geometric error. Phone controls expose VIEW, LEVEL
and NEXT SOURCE. `L` still changes the rock's source tessellation.

[`fan.coax`](../../aethyr/render/geometry/fan.coax) supplies a deliberately
constrained square-pyramid patch family. Removing only the interior apex
reduces four triangles to two and preserves all four boundary edges. A flat
patch has zero error; the bent patch's 0.75 m apex produces exactly 0.75 m
symmetric surface-distance error after simplification. This is an analytic
result for this graph family, not a sampled estimate or a general mesh-error
metric: vertical matching bounds both directions, and the removed apex
attains the distance to the simplified plane. Tests also cover negative
heights, winding/area and adjacent patches at mixed resolutions.

The authority gate kills all 119 hierarchy mutants; the fixture gate kills
all 10 simplifier mutants. All 451 realm/dependency checks pass. Native reports
preserve the three original rock hashes. Actual hardware WebGPU checks cover
parent colors/counts, both patch resolutions/errors, keyboard and phone touch,
and the existing reference-pixel and controller regressions. Evidence is in
the [hierarchy receipt](../verification/valley-lab-hierarchy-2026-11-05/README.md).
Run `coax/web/tool.sh aethyr/tools/valley-hierarchy.coax` for the native report.

This slice does not simplify the rock, choose levels by screen error, stream
pages or qualify a shared/native renderer. It establishes the ownership and
small boundary/error fixtures required before those steps.

## Landed: three consecutive geometry stages

The next three stages are implemented as independently checked Coax modules
and visible stations. Keys `4`/`5`/`6` and the second row of touch tabs open
PARENT, LOD and PAGES; their direct links are `#parents`, `#lod`, `#pages`.
The original walk, asset, full-detail, cluster and hierarchy views remain.

**Parent meshes.** [`parent.coax`](../../aethyr/render/geometry/parent.coax)
extracts directed external edges from source triangles, identifies a common
interior apex and retains every boundary vertex and segment. The bounded
supported class is a strictly convex fan with a horizontal boundary plane.
Triangle ordering and apex numbering may vary. Unsupported, degenerate or
nonmanifold input refuses atomically, preserving the full-detail parent.
The local surface distance is the apex's distance to the boundary plane;
parent error adds the child's error, rounding upward in f32. This composes
errors monotonically rather than resetting them at each level.

Two original irregular adjacent patches in
[`verse/valley/parents.coax`](../../verse/valley/parents.coax) each have six
source triangles and four parent triangles. Their shared edge has identical
positions and segmentation in source, mixed and coarse views. `H` cycles
12 / 10 / 8 drawn triangles. The planar parent has zero error and the bent
parent has 0.75 m error. The same reducer is attempted on the first two rock
leaves; their topology is unsupported, so the unchanged rock reference is
retained. This is explicit refusal, not a claim that the rock was simplified.

**Screen-error cuts.** [`cut.coax`](../../aethyr/render/geometry/cut.coax)
bounds perspective displacement with
`focal * error * (1 + radial / near) / (near - error)`.
The inspector derives near depth and radial extent from the source bounds
in the actual camera, with a small conservative transform margin. Tolerance
is in physical framebuffer pixels. Crossing the near plane or invalid bounds
keeps full detail. `T` cycles 16 / 64 / 256 pixels and the wheel changes
camera distance. Selection uses actual parent errors; source tessellation
changes remain separate. A missing requested parent draws pinned full-detail
children, never a hole. The two-patch frontier has full-source, mixed and
coarse choices; the general rock hierarchy still has identity parents.

**Page residency.** [`pages.coax`](../../aethyr/render/geometry/pages.coax)
serializes VGP1 geometry pages, validates bounded decoding and admits only
bytes matching the expected SHA-256 and current request epoch/ticket. The
native Coax author writes the two committed `.vgp.hex` files; the existing
bounded same-origin HTTP effect fetches those real files on demand. The C
face exposes that transport only; Coax owns requests, interpretation, retry
cadence, selection, cache ownership, budgets, eviction and fallback.

Each example page has 136 encoded bytes and 120 decoded geometry bytes.
The two-slot cache allows 256 or 512 logical resident bytes. `B` changes the
budget; `E` flushes and advances the epoch. Pending requests deduplicate;
late, corrupted, malformed or over-budget arrivals refuse. LRU eviction
respects CPU-copy pins and publishes atomically. Pins end after geometry is
copied to the frame's existing mesh buffer. This does not free or page a new
GPU heap. The original source and unpaged parent references remain retained
for isolated comparisons, so cache savings are not total process savings.
Fixed cache capacity, retained references and HTTP inbox are counted in the
existing 6 MB CPU arena; host/browser buffers are outside that declaration.

All 474 realm/dependency checks pass. The new authority gates kill 131 parent,
24 cut and 292 page mutants, with zero survivors. A native SHA-256 oracle
matches both page files. Actual desktop/phone WebGPU checks cover parent
savings, distance/threshold cuts, held-page fallback, stale/corrupt responses,
budget pressure, eviction, and exact unpaged/paged reference pixels. GPU loss
retains the exact state digest, resident pages and reference pixels. Original
rock asset hashes and the controller regression continue to pass. See the
[three-stage receipt](../verification/valley-lab-three-stages-2026-11-05/README.md).

These stages qualify the supported patch family and CPU page cache on the
initial WebGPU scene/HUD path. They do not complete arbitrary rock reduction,
a GPU page registry, occlusion/indirect submission, a large streamed valley,
shared/native renderer qualification or book drafts.

## Landed: first nonplanar parent on the original rock

`7 ROCK` / `#rock-parent` opens a highlighted interior star on the real rock.
`H` switches its six source triangles to four parent triangles. The rest of
that rock keeps its original shared vertex table, indices, normals and material.
The six boundary vertices and segments remain byte-exact. `4 PARENT`, `5 LOD`
and `6 PAGES` retain this dataset after entering ROCK; the direct `#parents`,
`#lod` and `#pages` links still open the earlier two-patch fixtures.
`#rock-lod` and `#rock-pages` open the real-rock versions directly. `L` cycles
all three original profiles, `T` changes the pixel threshold, and `E` flushes
pages. In the rock pager, `B` changes the exact 256-byte resident budget to
255 bytes, demonstrating full-detail fallback when the parent cannot fit.

The new Coax primitive [`star.coax`](../../aethyr/render/geometry/star.coax)
constructs a canonical nonplanar star from an ordered boundary and one interior
apex. Projected strict convexity checks the supported disk. Collapsing the
apex onto the first boundary vertex produces the boundary fan; its
barycentric map gives a conservative two-way surface-distance bound of the
L1 displacement, plus any child error, rounded upward from f64. This loose
bound, rather than a fitted or sampled error, drives the existing perspective
selector. Unsupported inputs refuse atomically. The authority gate kills all
90 root mutants, with no survivors or equivalents.

[`rock-parent.coax`](../../verse/valley/rock-parent.coax) extracts and verifies
one six-triangle star on the +X face of each original rock. This is an
intentionally narrow first real-geometry qualification: **one interior vertex**,
not a reducer for arbitrary cluster groups with several interior vertices.

| Original profile | Whole-rock source → parent triangles | Patch vertices | Certified error (m) |
| --- | --- | --- | --- |
| 8 | 768 → 766 | 7 → 6 | 0.708795071 |
| 16 | 3,072 → 3,070 | 7 → 6 | 0.502553105 |
| 32 | 12,288 → 12,286 | 7 → 6 | 0.211739063 |

The standalone patch representation drops from 172 to 136 encoded bytes,
and from 328 to 256 logical encoded-plus-decoded resident bytes. The original
canonical rock assets remain unchanged and retained; these numbers are not
whole-asset download savings. Pages use the existing VGP1 codec, SHA-256
identity, real HTTP transport and two-slot CPU cache. The paged draw copies
the admitted decoded mesh, rather than the locally computed parent.

Native checks cover all three profiles, exact source triangle identity,
boundary words and frame budgets. An independent f64 point-to-triangle oracle
samples both surfaces and checks the analytic bound and pinned hashes.
Desktop/phone WebGPU checks compare source and parent crops, exact
unpaged/paged crops, missing/corrupt page fallback, 255/256-byte admission and
exact state/pixels after device loss. Evidence and limits are in the
[real-rock parent receipt](../verification/valley-lab-rock-parent-2026-11-05/README.md).

## Landed: multi-interior groups, mixed cuts and a deterministic field

1. **Qualified native grid reducer.** `grid-parent.coax` reduces a canonical
   4×4-cell nonplanar grid using four independent interior star collapses:
   25→21 vertices and 32→24 triangles. Its external segmentation is unchanged.
   The maximum sibling bound, with child error included, encloses the union.
   It requires positive orientation in the supplied projection and refuses
   unsupported inputs atomically. All 113 authority root mutants are killed,
   with no survivors/equivalents. This is a bounded structured class, not an
   arbitrary imported topology reducer. Native checks and mutation evidence
   are in `docs/verification/valley-lab-next-three-2026-11-05/`.
2. **Adjacent real groups.** Implemented: `8 GROUPS` / `#groups`,
   `#group-lod` and `#group-pages`. Two neighboring 4×4-cell groups have 64→48
   triangles; `H` selects source/mixed/coarse cuts. Whole-rock triangles are
   768→752, 3,072→3,056, and 12,288→12,272 across the three original profiles.
   The exact shared chain retains five vertices and four segments in every
   mixed cut. Each VGP1 page is 556 bytes; logical encoded-plus-decoded
   residency is 1,096 bytes, versus 1,384 for its source representation.
   `B` selects 2,192 or 1,096 bytes of residency. Native app budgets and an
   independent f64 topology/surface oracle pass for all three profiles.
   Desktop/phone browser checks qualify visible source/mixed/coarse cuts,
   near/far perspective selection, stale/corrupt responses, one-page fallback
   and exact unpaged/paged GPU crops.
3. **Deterministic field.** `9 FIELD` / `#field`, `#field-lod` and
   `#field-pages` show six original rocks on a larger ground court. Seed 12609
   and layout version 1 reproduce all six transforms exactly in native Coax
   and Wasm; the native f32 word record is checked in the browser. Rocks use
   yaw and exact binary scales 1 / ½. Bounds scale with each instance and
   include a 10 µm transform-rounding allowance; the independent oracle checks
   this allowance on both field profiles. Each instance selects its own cut.
   `H` selects 18,432 / 18,384 / 18,336 triangles at profile 16. `L` cycles
   supported field profiles 8 and 16; entering from profile 32 selects 16.
   All six rocks retain **one original source identity and two parent page
   identities**: two HTTP requests and 2,192 resident bytes serve twelve
   coarse group uses. One-page budget pressure leaves six groups full detail.
   `R` restores the field camera and its same seeded record.

The field raised the declared arena to **7 MB**, the submitted-vertex limit
to **80,000**, and the physical HUD buffer to **81,920** vertices together.
The geometry buffers stay at 16,384 vertices / 65,536 indices. Actual paged
field submissions are 69,066 vertices on desktop and 66,834 on phone; the
arena occupies 6,483,512 bytes. Source/mixed/coarse frames and the supported
profiles also pass the native desktop and high-DPR phone budget checks.

The field uses CPU-transformed indexed copies in the existing scene path;
it does not claim hardware instancing, GPU page residency or shared-renderer
resource retirement. Both the single-star fixture and the original controller
remain independently inspectable and pass their regression journeys. This is
an orbitable geometry field, not the completed walkable valley tile. Source
asset hashes remain unchanged. Full native, independent oracle, headless GPU,
HTTP and recovery evidence is in the
[three-phase receipt](../verification/valley-lab-next-three-2026-11-05/README.md).
No book draft is upgraded.

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

The [2026-11-06 source-world cluster receipt](../verification/valley-lab-clusters-2026-11-06/README.md)
implements bounded source-order leaves over all 130 mesh types, an exact-range
binary hierarchy and conservative CPU camera/shadow cuts. Complete four-view
images match the unculled source reference. The measured independent-pass cut
is the default; deeper cluster traversal remains diagnostic. This does not
complete parent simplification, higher-detail input, paged residency, occlusion
or GPU selection below.

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

## Landed: exact Unreal asset inventory and private scan baseline

The local `ValleyoftheAncient2` project yielded 317 Megascans mesh packages,
299 embedded import records and 124 unique original ID candidates. 281 mesh
packages have positive Ancient World actor-package references in the bounded
prefix scan. These are reference lower bounds, not placement counts or a
complete Unreal package parser. Eleven IDs match current Fab source archive
filenames exactly, including CanyonSandstone, several heavily used western
sandstone/ground sets and Quarry Cliff.

Two free Standard source listings were downloaded programmatically and added
as isolated content at `#scans` (`0`): Desert Western Rock Medium 08
(`uk4cdch`, 6,780 vertices / 2,994 triangles) and Quarry Cliff (`ubhvccfda`,
1,949 vertices / 3,099 triangles). Their current low-tier glTF source may
differ from the scene artist's high-detail FBX. `L` selects the other source,
`V` selects diagnostic views, and `R` retries. No paid asset was bought.

Inventory, public catalogue matching, anonymous range downloading, glTF
transforms, JPEG decoding and base-color sampling are Coax. Raw and derived
licensed files remain under the shared private Aethyrforge cache, exposed to
the local preview through an ignored asset directory. Only metadata, hashes,
code and verification records are committed. The sampler bakes color at
vertices; full PBR texture sampling remains a separate task. These scans use
their whole low-tier source mesh, not fabricated grid parents or imported
Nanite data. The arbitrary-mesh reducer remains unsupported.

The live/candidate/transport storage raises the declared frame byte budget
to 9.5 MB together; actual arena is 8,604,520 bytes. Existing physical GPU
buffers and submitted-vertex limits remain adequate. Hash, shape, finite
value, color and index checks precede replacement; missing/corrupt downloads
retain the last valid scan, and replies after leaving do not replace it.
The parser's 171 mutation sites have zero survivors. Independent matrix and
Pillow checks cover every source vertex, color sample and original index.
Desktop/phone browser checks cover both sources, five distinct views, failures,
retry, stale delivery and exact state/pixels after GPU loss.

Full asset inventory, eleven catalogue matches, two download manifests,
commands and receipts:
[exact source assets](../verification/valley-lab-source-assets-2026-11-05/README.md).
Recheck the remaining source IDs during the announced staged free rollout;
a free UEFN reference licence alone does not provide raw source. Add texture
sampling/PBR and a separately qualified arbitrary-mesh baseline before claiming
source-material or scan-LOD parity. First scene placement still needs explicit
transformed collision proxies; the scan inspector has not added them to walking.

## Landed: an exact source section, the Canyon Sandstone pair

The selected section is the two Canyon Sandstone instances in Ancient World's
`L0_X0_Y0_DL44BCB661` HLOD cell, component
`InstancedStaticMeshComponent_7376`. The installed Unreal SDK exports the
original source-model geometry, albedo/roughness maps and exact world poses
through a data-only headless commandlet project. Game modules and sample
plugins are not run. A new isolated inspection view under Valley Lab opens
with **P**, the scan station's touch button or `#section`:
`/worlds/valley-canyon?pack=valley-canyon`. `1` returns to the lab.

Both instances retain all 999,995 source triangles, source normals, UVs and
vertex paint. Exact attribute deduplication produces one shared 504,367-vertex
GPU asset, preserving all 2,999,985 original indexed corner records. The view
uses the original 4096 x 4096 albedo and roughness maps and RGB 1.5 material
override. One common scene origin preserves their original pivots, relative
height, scale and rotation; there is no individual flooring or hand placement.
An independent oracle checks every source corner and every transformed vertex
against the exported SDK receipt; the maximum vertex pose error is below
0.000001 m. Actual desktop/phone GPU checks cover both poses and five material
views, reset and exact device-loss recovery.

The 71 MB source pack remains in the shared private Aethyrforge cache and is
exposed through the existing optional private-asset channel. Public builds do
not require or upload it. Before any pack upload, the new optional pack gate
v1 sends the browser-computed SHA-256 and byte count to a Coax authority module.
A changed source byte is rejected as a whole. Pack-gate and renderer-adoption
mutations have zero survivors. Device recovery now clears packed asset rows
as well as ordinary rows, fixing the old-device buffer bind found on this path.

This source view has a separate declared resource budget for full immutable
geometry, both 4K mipmapped layers and scene/shadow draws. The original lab's
primitive frame/mesh budgets remain independent. It uses ordinary GPU triangle
passes, not Nanite's runtime algorithms or the structured grid reducer.

**Full Unreal image parity remains unqualified.** Exact source geometry,
material maps and transforms are verified; Aethyr studio lighting, shadows
and tone mapping are used for inspection. Matching terrain, surrounding
components, the complete original material graph, sky/Lumen/exposure and an
actual matched Unreal reference render remain separate fidelity work.
No book draft or full-valley completion status is upgraded by this view.

The [source-section record](../verification/valley-lab-canyon-pair-2026-11-05/README.md)
contains exact source identities, original poses, reproducible SDK/Coax tools,
independent all-corner checks, browser observations and screenshots.

## Current implementation: Slay performance (#12962, still open)

Skin/face/hair follow-up: the full Slay groom replaces the different Valley
strand dataset (118,339 curves /2,301,856 segments, no decimation). Source
highpass/ORM head specular modulation, separate per-light highlights and
single-fiber R/TT/TRT transport are implemented. A native additive/track audit
finds no missing head/neck tracks. The exact187,564,308-byte source binding
is requested by the optional storage-limit v1 host ABI; the lab explicitly
raises its vertex-work budget to50m while keeping its4.6GB aggregate limit.
Full source endpoint/texture checks and the updated comparison are in
[the skin/face/hair record](../verification/valley-lab-slay-2026-11-06/README.md).
The next work remains regional skin roughness/diffusion, rectangular-light
integration, eyes/refraction, hair volume scattering and source camera/frame
matching. This checkpoint is not visual parity or mobile qualification.

The owner's target is the Slay trailer frame at 0:57 (desktop screenshot
2026-11-05 22:44:37): Echo's narrowed eyes and expression, raised open hand,
blue glowing orb, close camera and warm/cool lighting. This extends #12962's
face-only scope with a separate fixed shot recreation; ordinary locomotion
must retain the existing Valley body clips.

1. Land and qualify the current Echo port, then use a fresh worktree.
2. Read Slay through the installed Unreal SDK. Identify the actual source
   sequence/frame, body and facial tracks, camera and orb transforms. Export
   all 20 face clips, target names and sparse position/normal deltas privately.
3. Add bounded face-pack admission and shared Aethyr morph/curve playback.
   Apply morphs before skinning, layer facial bones by name, and test zero
   weights, bad indices, nonfinite deltas and unresolved bindings.
4. Add a fixed source-shot view, preserving the actual hand/body pose and
   expression. Port the orb material, source lighting and lens-flare effects in Coax, comparing fixed
   captures to the supplied reference. Do not substitute neutral Idle or an
   arbitrary sphere and label it an exact match.
5. Add selectable face clips and documented ambient segments over locomotion;
   check blend boundaries, desktop/phone cost and GPU recovery. Record actual
   image differences and outstanding source render features before pushing.

The first implementation is now the parent P destination:
`/worlds/valley-canyon?pack=valley-slay`. Its private 1,023,634,576-byte bundle
adds 181 named morph targets (564,903 sparse position/normal deltas), 20
24-fps facial clips, 125 mapped curves and 16 facial bones to the existing
334-joint/24-body-clip Echo port. Morphs execute before skinning in shared
Aethyr Coax shader code. Face-only layering retains a separate body pose;
zero weights preserve the neutral path. All licensed payloads stay private.

The fixed J view starts paused at TF0290 sequence frame 1125 (body sample
124, face sample 4). It reads the source camera/lens, body pose and animated
`hand_l` orb attachment. The serialized actor transform track is disabled,
so it is not applied over the already authored animation root. P toggles
playback and comma/period step the source clock. F isolates all 20 facial
clips; brackets select, T restarts and B toggles neutral. R returns to the
unchanged walking/collision path. Short ambient source segments run every
eight seconds with 0.15-second fades: TF0130 frames 4–22, TF0145 frames 3–21
and TF0250 frames 19–40, at 24 fps.

The source project disables bloom. The source orb mesh and two texture layers now drive its Coax material
arithmetic and additive colour. The halo/flare remains an interim analytic
effect, and source curl-noise deformation is not yet implemented. Source key, fill and warm wrap directions/colors/intensities feed an
interim direct-light approximation with filtered shadow samples. This does
not reproduce the original area-light integration, skin scattering or room
lighting. Temple scenery, source orb textures/flare,
hair coverage/shading and the remaining material differences still prevent
an exact reproduction of the owner's screenshot. Do not close #12962 on
source-data equality alone. See the [implementation and comparison record](../verification/valley-lab-slay-2026-11-06/README.md).

The handheld source profile (`?pack=valley-slay-handheld`) now includes the
SDK's exact additive camera template and property scales. Its independent
oracle checks all 200 camera frames. The
[camera record](../verification/slay-camera-2026-11-06/README.md) retains the
464-check page result, mutation gates, visible captures and both recovery
observations: a concurrent deadline failure and isolated 9,849 ms recovery.
The 91,959-byte Wasm remains within 96,000 bytes. The existing fixed-camera
private pack stays valid; no licensed payload is committed.

## Subsequent engine work

Qualify **Valley on the shared frame/pass/resource renderer**, using the
current CPU-copy path as the reference. Give real GPU vertex/index/page
allocations explicit ranges and generations, fence upload completions and
retirement, and test late old-device acknowledgements, replacement and exact
cleanup on this actual lab route. The deterministic field and original Canyon Pair now supply
small procedural and full-source repeated-asset workloads; its CPU page cache does not satisfy GPU
ownership exits.

Then compose these field placements into the first walkable valley tile,
connecting their transformed collision proxies to the existing character
controller and preserving collision independently of presentation page
arrivals. Add conservative frustum fixtures before occlusion, indirect
submission and world-cell streaming. Extend the structured grid reducer to
larger connected interior topology only with its own refusal, seam, error and
measured-savings gates; arbitrary imported meshes remain unsupported.

Continue the remaining milestone 0/1 exits alongside that step: saved
walkthrough/replay, bounded source import and shared-renderer qualification.
A working station does not upgrade a book chapter's completion status.

## Earlier stage: walk around the original Canyon Pair

The parent lab's **P** key and `#section` open
`/worlds/valley-canyon?pack=valley-canyon`, now in first person by default.
**WASD** moves, **Shift** walks instead of running, **Space** jumps, arrow keys
or dragging the right half looks, and **R** restores the spawn. Touch uses a
left-side drag to move, a right-side drag to look, and the **JUMP** button.
These touch drags are sequential single-pointer controls. **O**, or its top
button, switches between walking and the existing isolated orbit inspection;
**V** and **I** retain material diagnostics and instance isolation. **1** returns
to the primitive stations.

This uses Aethyr's existing `walk.coax` fixed 1/64-second capsule controller,
its declared Valley Lab collision profile and `follow_first_shot` camera.
Source vertices, texture bytes and the two source matrices are unchanged.
The metre-grid floor sits below the pair's conservative bounds. Two explicit
conservative AABB barriers extend from that floor to each source bound's top,
including when an instance is hidden for inspection. These barriers also close
the space below suspended parts of the source assembly. They are inspection
fixtures, not the original scene's terrain or its collision mesh. The view
has no imported Echo avatar, slope-aware animation or vaulting yet. Full
triangle collision and adjoining source terrain remain separate future stages.

Short jump presses are queued through Aethyr's input primitive even when
released before a frame. Blur releases held controls while retaining already queued presses, following
the shared input contract;
reset clears movement and restores the initial feet. Orbit freezes locomotion
and clears held controls.

## Current Echo port: third-person walking and source inspection

The initial source port used `/worlds/valley-canyon?pack=valley-echo`; the current
parent P action opens the Slay extension described above.
The 668,024,432-byte private bundle retains both original Canyon instances,
Echo's source mesh/UVs/normals/tangents, eight skin influences, 334 palette
joints, 24 animation clips (2,696 frames), 32 original texture files and
45,091 groom curves (492,907 rendered segments). Source card geometry is
retained for auditing but excluded from drawing when strands are present.
The independent source-byte oracle checks every vertex/index, bind/rest
transform, animation sample, strand endpoint and bundled texture. Browser
Wasm Idle palette differs from independent double precision by at most
8.35e-7. No licensed source data is committed.

WASD moves, Shift walks, Space performs the lab jump, drag/arrows look,
O freezes in orbit, R resets, C gives a full character inspection and H a
face close-up. Shared touch sticks support concurrent movement and look.
The source flat-ground profile uses 0.42 m capsule radius, 1.92 m height,
1.47/4.27 m/s walk/run speeds, 5 m/s² acceleration, 10 m/s² braking,
ground friction 4, braking factor 2 and zero air control. These are decoded
SDK defaults, not proof of complete original gameplay. Aethyr owns paid
64 Hz steps and conservative lab collisions; Space remains a lab jump,
not the original ability-based vault.

Material slots are now checked against the native SDK manifest (head 11,
skin 6, eyes 9/17, lashes 18). Source albedo overrides, packed roughness,
normal maps, source gray environment radiance and a checked film curve are
ported in Coax. Bounded sequential image upload avoids retaining every
decoded 8K source and resized mip at once. Source inspection supersamples
the strand ribbons within an explicit raster cap.

**Visual parity is unfinished.** Eyes/refraction/occlusion, regional skin
scattering, strand shading/coverage, full animation
graph, source camera stack, cloth and hair simulation remain incomplete.
The native reference's groom also renders incorrectly and cannot qualify
hair parity. The original game's incompatible native modules were not run.
The isolated neutral view remains a control; the Slay extension above adds
source facial performance and a separate fixed shot.

The source velocity, locomotion integration, walking, collision profile,
rig admission and pack identity gates killed 98/25/598/33/165/13 mutants,
respectively, with zero survivors. The independent float32 velocity oracle
checks 1,024 cases; film mapping checks 97 independent values. These results
apply to the unchanged scalar/source paths, not to full Unreal rendering.

## Earlier assessment: the original project's controller and Echo avatar

Inspected the local `ValleyoftheAncient2` project under
`~/Documents/Unreal Projects/`. This assessment comes from its C++ source,
configuration, asset inventory and serialized asset names/references. It does
not execute the sample game or Blueprint graphs. References establish that a
system is present; they do not qualify its runtime behavior or numeric tuning.

| System | Evidence in this copy | Implication for Aethyr |
| --- | --- | --- |
| Player character | `Content/AncientContent/Blueprints/Character/BP_EchoCharacter.uasset` references `/Script/ModularGameplayActors.ModularCharacter`, capsule, `CharacterMovementComponent`, `AddMovementInput`, `JumpZVelocity`, `MaxWalkSpeed`, camera, Motion Warping and vaulting. The plugin's `Public/ModularCharacter.h` derives from `ACharacter`. | Use the existing checked Aethyr capsule and camera as the movement foundation. The Blueprint is not an engine-independent controller to drop into Coax. Capsule dimensions, acceleration, jump speed and braking defaults have not yet been decoded. |
| Input | `Config/DefaultInput.ini` selects Enhanced Input. `Input/IM_ThirdPersonControls_InputMapping` and move-forward/right, turn and look-up actions exist. `Source/AncientGame/Input/PlayerControlsComponent.cpp` installs/removes mapping contexts and delegates control setup to Blueprint. | Preserve semantic actions in Coax; translate bindings rather than shipping Unreal input components. |
| Movement tuning and abilities | `Character/MovementAttributeSet.cpp` applies the Gameplay Ability System's `MoveSpeed` attribute to `CharacterMovementComponent::MaxWalkSpeed`. Echo has `Data/DT_EchoAttributeDefaults`, an ability-binding component, `Blueprints/Abilities/GA_Vault`, vaulting trigger/utilities and a `VaultOver` clip/montage. | Movement and animation are coupled to attributes and abilities. Port explicit locomotion states and traversal rules as separately checked systems. Exact defaults and Blueprint transition conditions still need a structured export. |
| Camera | `Source/AncientGame/Camera/` contains a camera-mode stack, spring-arm modes and critically damped location/rotation interpolators. `SpringArm.cpp` sweeps a sphere against the camera channel to pull the camera out of obstacles. Third-person, vaulting-area, catwalk, spline and static-framing Blueprint modes exist. | Our shared follow camera covers the basic movement view. The original's authored camera modes and transitions need their own data and parity checks. |
| Visible avatar | `Characters/Echo/Meshes/Echo.uasset` is about 54 MiB; `Echo_Skeleton.uasset` and `Rig/Physics/PA_Echo` exist. There are 183 files under `Characters/Echo`, including meshes, animation, materials, hair and rigs. Material references include skin/head, eyes/lashes/teeth, clothing, leather, accessories and hair cards. | There is useful source data for a skinned avatar. Export one mesh LOD, skeleton, bind poses, weights, material slots and a small clip set before expanding fidelity. Asset-file size is not a runtime GPU budget or triangle count. |
| Locomotion animation | `Echo_AnimBP`, idle/walk/jog starts and stops, left/right 90/180-degree starts, fall/land, sit/stand, reach and vault clips exist. Serialized animation references include locomotion-state determination, acceleration, falling and root-motion queries. | A single walking loop will not reproduce Echo's responsiveness. Test idle/walk/jog/jump blending and turning first, then starts/stops, root motion and traversal. Graph presence alone does not verify active states. |
| Ground adaptation and secondary motion | `Echo_SlopeWarping_CtrlRig`, twist/helper rigs, `Echo_PostProcess_AnimBP`, hair animation and groom binding assets exist. Physics assets cover skirt, scarf, ponytail, pouch, canister and shoulder pad. Animation metadata names FBIK enable/disable controls. | Separate skinning, foot placement/slope adaptation, corrective deformation, hair and cloth. Begin with baked animation and simpler material/hair representations, then qualify each additional system in isolation. |
| Encounter-specific extensions | `Plugins/GameFeatures/AncientBattle/Content/Characters/Echo/Animations` includes dodge and light-dart charge/release assets and montages. | Keep combat/ability animation separate from the basic walking-avatar milestone. |

The practical next avatar slice is an isolated Echo station: export and verify
one skeletal LOD plus idle, walk, jog and jump clips, show the bind pose and
bone/weight diagnostics, then drive those clips with the same Aethyr walker
used here. Record scale, bone count, influence limits, material cost and clip
samples against the source before claiming equivalence. Add slope warping,
vaulting, secondary motion and encounter abilities only after those checks.
The current Canyon Pair view is first person; it does not contain Echo's mesh
or claim an Unreal controller port.

Slay render source extension:101,672 UV pairs,25,547 sword vertices,
134,418 sword indices and35 bundled textures pass the independent source-byte
oracle. The sword follows the source `prop_01` track through the existing
`groom.prop_01` palette entry135; its mesh inverse bind is the same. Slay's
head instance uses Desat=.3 and Tint=(.921582,1,.871367), unlike Valley's .2
and(.9,1,.949020). Its actual textures replace Valley counterparts in this
private package.35 mipmapped4K RGBA8 layers reserve approximately3.13GB of
GPU texture storage; the lab's explicit aggregate budget is4.6GB. A phone
viewport observation on this Mac does not qualify mobile hardware memory
or performance. A separate mobile/source-detail profile remains necessary.

The orb extension adds559 exact source vertices,2,880 indices and1,114,112
original RGBA texels, including independent checks of the full packed GPU
streams. Its world hand quaternion matches independent double precision over
all200 frames within1.93e-7. [SHS1 v2](../verification/valley-lab-slay-2026-11-06/private-shot-format.md) explicitly versions
that new quaternion metadata and validates its unit length before publication.
Distortion remains linear and normal-mapped; the energy texture decodes sRGB
before bilinear filtering, matching the native SDK texture flags.

The opt-in legacy browser `coaxial_draw_blend_v1` keeps ordinary geometry
opaque, selects straight-alpha additive colour for the orb and source-over
coverage for strand colour. Translucent colour depth-tests without depth
writes; shadow coverage remains shader-owned. The unchanged599558 indexed
module without this export and the parent lab pass on the new host. The
actual orb GPU comparison adds colour to171,054 supersampled pixels and
darkens none; the scene remains ready on return to ordinary walking. Hair
colour uses continuous coverage instead of stochastic opaque colour samples;
its shadow keeps separate stochastic coverage. The new single-fiber R/TT/TRT pass follows after this coverage correction;
deep opacity, multiple scattering and sorted/OIT hair remain unfinished.

The pixel budget bounds the base raster before the explicit3x portrait
supersampling: the1280x800 observation captures3840x2400 physical pixels.
The additive operation currently occurs in the legacy display-mapped target;
full linear-HDR compositing, the source material-time phase,
exact reference frame selection and cinematic background remain open.
