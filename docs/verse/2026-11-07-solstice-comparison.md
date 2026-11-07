# Solstice and Aethyr: what to learn

Status: research note, 2026-11-07. Nothing here is implemented yet.

[Solstice](https://github.com/SamG-Coder/solstice) (MIT, read at
`9943ee0c99`, 2026-11-07) is a renderer in Rust and WebGPU by Sam G. It runs
in Chrome and Edge and follows Nanite and Lumen: a cluster LOD DAG built
offline, GPU culling, voxel GI, and, as of this week, streaming on demand.
It is about 26,000 lines across five crates and two apps. It is a renderer
only, with static scenes and no gameplay, physics, network or characters.
This note compares it with Aethyr and lists what we should take from it,
most valuable first. We take the ideas and write our own code in Coax. None
of its code was run here.

## Its headline numbers

The browser cove at 50 Mbit/s and 30 ms latency, on an RTX 5080
(`docs/streaming.md` in their repository):

| | Before streaming | After |
| --- | ---: | ---: |
| Download before the first image | 209 MB | 6.0 MB |
| First image | 36.5 s | 1.4 s |
| Opening view at full detail | at the first image | about 17 s, 61 MB in all |
| Scene data on the GPU | 280 MiB | 195 MiB, fixed |

The cove is 1.22 billion instanced triangles. On a desktop GPU a 60-second
scripted flight averages 3.8 ms a frame, with a p95 of 5.7 ms and a worst
frame of 8.5 ms.

## Where Aethyr stands on the same axes

| Axis | Solstice | Aethyr today |
| --- | --- | --- |
| Geometry LOD | A general DAG: 128-triangle clusters, group, simplify with the boundary locked, regroup every level | Valley Lab only (`aethyr/render/geometry/`): leaf clusters, ownership hierarchy and simplifiers for narrow shape classes (`parent`, `fan`, `star`, `grid-parent`). Shipped realms use a distance cutoff (`apps/openagents-web/everglade-lod.coax`) |
| Cut selection | GPU compute traversal, under 1 px of projected error | CPU cuts (`cut.coax`), Valley Lab only |
| Culling | Frustum, plus two-phase occlusion against a depth pyramid; a compute rasterizer for small clusters | No general frustum culling, no Hi-Z, no indirect draws (Valley Lab steps 4 and 6 are open) |
| Streaming | One file, HTTP Range, 128 KiB pages, parents installed first, LRU over a fixed pool | Whole packs, fetched entire (`coax/web/host.js`). `pages.coax` has 2 slots for one Valley Lab rock. `aethyr/world/cells.coax` exists but no realm uses it |
| GPU memory | Fixed pools allocated at startup, so the budget can't be exceeded | `budget.coax` and `render/frame.coax` refuse a frame that goes over. Registry ceiling of 1 GiB |
| Textures | BC7/BC5 with sRGB; three tiers of arrays, streamed by mip; unpacked on GPUs without BC | JPEG/PNG through `createImageBitmap`, so full RGBA8 on the GPU. No compressed formats |
| Shadows | 4 cascades; a cascade redraws only when its view changes; contact shadows | One 2048 map (1024 on a phone) reaching 60 m, with PCF (`aethyr/render/light.coax`) |
| GI, sky, post | Voxel clipmap GI, Hillaire atmosphere, TAA, histogram exposure | Hemisphere ambient, height haze, authored sky. Bloom and exposure in `render/post.coax`. No GI and no TAA |
| Measurement | Scripted flyover: a frame-time CSV and one image a second; a throttled server for time to first image | `realm-perf.coax` (17 realms × 3 forms, a scripted walk, the 60 FPS rule). No network throttling, no time-to-first-image figure |
| Checks | Watertight-cut tests, a randomized residency test, WGSL validated without a GPU, GPU tests that skip when there is no GPU | `#test`/`#forall`, `#authority` with zero mutants, goldens. This is stronger than theirs, and their ideas should land under our rules |

Aethyr leads where Solstice has nothing: gameplay, physics, characters,
replay, the network, voice, and checks that decide authority. Solstice leads
on scale. It shows a browser can open a billion-triangle scene in 1.4 s, and
it shows how.

## What to take, in order

### 1. Rules that keep a partly loaded scene watertight

This is the most important idea and the cheapest to adopt. It is pure CPU
logic, which suits `#authority` and `coax mutate` well.

- **Draw rule:** draw cluster C when `parent_too_coarse(C) and
  (fine_enough(C) or children_not_installed(C))`. A parent keeps drawing
  until its whole child set has arrived, so a slow network gives coarser
  geometry instead of holes.
- **Install and evict:** a page installs only after every page holding its
  clusters' parents, and leaves only after every page that depends on it.
- **Pages are built for it:** a sibling set (the clusters that swap for the
  same parents) is never split across pages. Each page holds sets from one
  build round, and parents come from a later round. So page dependencies point
  only at lower ids and cannot form a cycle. Root sets share pinned pages.
- **Residency without the GPU** (`streaming/residency.rs`, 513 lines):
  - Requests from the GPU are carried up to parents in one downward pass,
    since parents have lower ids. Fetches are sorted coarse first.
  - At most N pages are in flight at once.
  - Installs are capped at about 1 MB a frame.
  - The eviction victim is the least recently used page that has no installed
    dependants, isn't pinned and wasn't wanted in the last 30 frames. If
    nothing qualifies, the load waits and a `starved` counter rises.
  - Their randomized test checks the cut stays valid after every step.

**For us:** generalize `aethyr/render/geometry/pages.coax` from 2 slots to N
slots with this dependency invariant, and turn their randomized test into a
`#forall` over request sequences. `pages.coax` already has content identity,
tickets that fence stale arrivals, and byte accounting, which Solstice lacks.
The invariant is what we're missing. This is Valley Lab step 5 done
properly.

### 2. The GPU says what it wants with one request array

There is one `u32` per page, cleared each frame. Traversal does
`atomicMax(requests[page], priority)` at each leaf that passes the LOD test,
where priority is projected error. An occluded leaf writes `KEEP = 1`, which
means "don't evict, don't load". Shadow and GI views write too, weighted
lower. The array can't overflow and needs no deduplication. It is read back
without blocking, about 8 KB. Texture demand rides on the same array: each
cluster stores a quantized UV density, and the cull pass works out the mip
it wants. That keeps the work out of the resolve pass, which is already at
WebGPU's guaranteed limit of 8 storage buffers.

**For us:** this is the contract between Valley Lab step 6 (GPU selection)
and step 5 (pages). Coax emits the WGSL, and the residency side stays a
checked Coax module.

### 3. Range-addressable packs, with time to first image as the metric

The file layout:

- a header
- a deflated manifest: nodes, cluster records, the page table, materials,
  instances, root pages and texture mips of 128 and below
- pages, each deflated on its own
- raw texture mips

The browser fetches byte ranges and inflates them with
`DecompressionStream`. If a server ignores Range, the page downloads the file
once and serves ranges from memory, so nothing breaks. Each response's
`ETag` is checked, so a scene can't mix two deploys. A test server adds
Range and `--throttle-mbps`.

**For us:** Everglade's pack is 9.0 MB, fetched whole before play
(`docs/verse/everglade.md`), and it will grow. A range-addressable version
of VPK1 fits our model better than theirs. The manifest would carry each
page's SHA-256 `AssetId`, so every range is checked on arrival under the
existing `assets.coax` admission. That is stronger than an ETag. The steps:

- Add Range and a bandwidth/latency throttle to `coax serve`. Done in `bc066a9a` (#13000):
  `coax/web/http-range.coax`, per-connection shaping, and first-image baselines in
  `docs/verification/network-first-image-13000.md`.
- Teach `coax/web/host.js` to fetch ranges, falling back to the whole file.
- Make *time to first image at 50 Mbit/s and 30 ms* a number every realm
  reports beside the 60 FPS rule.

Book chapter 38 (stream a large world) and `aethyr/world/cells.coax` cover
the world-cell half of this. The page format covers the asset half.

### 4. Compressed, tiered textures (phones first)

WebGPU has no sparse textures, and every layer of an array shares one size,
so Solstice uses tiers:

| Tier | Edge | Layers | Lifetime |
| --- | ---: | --- | --- |
| Base | 128 | all | in the first download, never evicted |
| Middle | 512 | all | fetched on demand, never evicted |
| Top | largest that gives at least 6 slots | as many as the budget holds | least recently used |

Promotion copies lower mips between arrays with `copy_texture_to_texture`
and fetches only the missing top mips. Their measured format choices:

- **Normals:** BC5 on their own. That is 1.11° mean error, against 1.81° when
  packed into BC7 with roughness.
- **Roughness and metallic:** a second BC5.
- **Colour:** BC7 sRGB, about 42 dB against BC1's 36 dB.
- **Cost:** 3 bytes per texel, down from 8.

**For us:** we upload RGBA8 decoded from JPEG and PNG, so every texture
costs its full size in GPU memory. That matters most on the phone, which is
our main target. Solstice stopped at BC because the good ASTC and ETC2
encoders wrap C++. iPhones expose ASTC, so we should go past them here:

- Write an offline ASTC and BC encoder in Coax as a tool.
  `aethyr/tools/ocarina-texture-core.coax` already decodes both against
  reference runners, which gives us an oracle.
- Choose a format per device through `requiredFeatures`.
- Keep the RGBA8 fallback.

Measure each realm's texture GPU memory first.

### 5. A general cluster DAG builder

The algorithm, repeated until one root set is left:

- Split the mesh into clusters of at most 128 triangles.
- Group neighbouring clusters.
- Simplify each group with its border locked, to about half its triangles.
- Split the group into new clusters.

Grouping changes every level, so a seam locked at one level is simplified at
the next. A group that can't shrink below 85% is left as it is and tried
again in a differently shaped group next level. Their tests are a ready
specification for our checks:

- Every uniform cut is watertight.
- Perspective cuts that mix levels are watertight from any viewpoint.
- The finest cut is exactly the source mesh.
- Error and bounds grow monotonically toward the root.
- Meshes made of seams build and stay watertight.
- Foliage thins with distance but keeps its coverage.

**For us:** Valley Lab has chosen narrow simplifiers on purpose, so far.
This is the largest item and the one that unlocks arbitrary imported assets
(Quaternius, glTF). Do it after items 1 to 3, so streaming and selection have
a real consumer.

### 6. Workarounds where WebGPU lacks a feature

They found these already, so we don't have to:

| Missing in WebGPU | Their workaround |
| --- | --- |
| Mesh shaders | Compute culling and instanced vertex pulling |
| 64-bit atomics for a visibility buffer | Separate 32-bit depth and ID buffers. A rare one-frame wrong ID is hidden by TAA |
| Bindless resources | Shared geometry buffers and texture arrays |
| Hardware ray tracing | Software tracing through a voxel and distance-field volume |
| Work queues that grow themselves | One dispatch per hierarchy level over a shared queue |

Two smaller points:

- **Installs don't touch shaders.** Installing a page rewrites
  `vertex_offset` and `triangle_offset` in records that are already resident,
  so raster and resolve shaders never change.
- **Writes are atomic per frame.** All writes go through `queue.write_buffer`
  between frames, so a frame never sees half a page.

### 7. Cached shadows

Four cascades, and a cascade redraws only when its view changes. That was
the largest single cost on small GPUs. When a page installs, the cascades
and GI are marked stale, rate-limited and taking turns. **For us:** cascades
with redraw-on-change would extend our one 60 m shadow map to large worlds
without paying for it every frame. This comes once worlds outgrow 60 m of
detailed shadow.

### 8. Benchmarks that fly a path, not just walk one

`viewer --flyover path.txt --out DIR` flies a timed path of camera keyframes.
It writes every frame's time and one image a second, then reports the
average, p95, worst frame and frames over 33 ms. It also reports a
before/after table for each change, and states what was *not* measured.
**For us:** `realm-perf.coax` already applies the 60 FPS rule. Add:

- a keyframed camera path, one per realm, that crosses its worst view
- a screenshot each second, so a speed-up can't hide a visual regression
- runs on the throttled server from item 3

## What not to take

- **GitHub Actions CI.** It is against our rules, and our local
  `#test`/mutation gates are stronger.
- **Rust and wgpu.** Coax writes the logic and emits the WGSL.
- **Static-only scenes and GI rebuilt per camera move.** Our worlds move:
  characters, destruction, physics. Any GI we add has to cope with that.
- **Resident cluster records.** They cost 48 bytes per cluster, about
  0.7 GB per billion unique triangles. That's fine at our scale, but we should
  know the ceiling.
- **Voxel GI, for now.** It needs about 150 MB of GPU memory at every quality
  level, which rules out the phones we ship to.

## Proposed sequence

1. **Watertight residency** in `pages.coax`: N slots, dependency invariant,
   `#forall` over random request sequences, zero mutants.
2. **Range and throttle in `coax serve`** (landed, #13000), with time to first image measured
   for Everglade before anything else changes.
3. **Range-addressable packs** with a SHA-256 per page, Range fetches in
   `host.js` and the whole-file fallback.
4. **Valley Lab step 6:** the request array, frustum culling and two-phase
   Hi-Z in Coax-emitted WGSL, and the draw rule in the cull pass.
5. **Texture memory audit, then ASTC/BC** with tiers.
6. **The general DAG builder,** with Solstice's watertight properties as
   `#forall` checks.
7. **Cached cascades** and the **flyover benchmark,** as worlds grow.

Items 1, 2 and 7's flyover are small and independent, and can start now.
