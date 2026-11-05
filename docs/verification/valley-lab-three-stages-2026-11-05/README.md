# Valley Lab: parent meshes, screen-error cuts and HTTP pages

Three consecutive stages build on the checked leaf/ownership baseline in
`469366bef4`. The unlisted world exposes PARENT / LOD / PAGES through keys
4 / 5 / 6, touch tabs and `#parents` / `#lod` / `#pages`. The original stations,
controller, source asset identities and shader remain available.

## 1. Boundary-locked parent meshes

`aethyr/render/geometry/parent.coax` supports a bounded strictly convex fan
with one interior apex and a horizontal boundary plane. It finds the apex
from source topology, extracts oriented external edges, validates the full
boundary cycle through strict convexity and preserves every boundary point
and segment. Canonical output is independent of source triangle ordering;
apex numbering and triangle rotations may vary. Capacity is 256 source
vertices and 768 index words. Unsupported/nonmanifold/degenerate patches
leave the published parent unchanged.

The source and simplified surfaces are graphs over the same convex polygon.
Vertical matching bounds both directions of surface distance; the apex
attains the distance to the simplified plane. The local bound is therefore
|apex.y - boundary.y|. Adding child error uses f64 arithmetic and rounds the
f32 result upward, preserving a conservative accumulated bound. This proof
applies to the declared fan class, not arbitrary nonplanar rock meshes.

`verse/valley/parents.coax` supplies adjacent irregular original fans. Each
has seven source vertices/six triangles and six parent vertices/four triangles.
Their shared edge has identical positions and segmentation at mixed detail.
H cycles 12 -> 10 -> 8 triangles. The planar patch has error 0; the bent one
has error 0.75 m. Integration attempts the reducer on the first two actual
rock leaves and records refusal; the original rock stays full detail.

`parent-mutation.log`: 131/131 root mutants killed. Checks cover triangle
reordering, cyclic corner/apex numbering, positive/negative heights, a nonzero
boundary plane, error accumulation/rounding, concavity/collinearity,
nonmanifold/missing-apex topology, malformed views and the exact capacity.
The original mixed-edge fixture and independently evaluated surface samples
are in the realm test graph.

## 2. Conservative screen-error selection and fallback

`aethyr/render/geometry/cut.coax` uses the bound
`focal * error * (1 + radial / near) / (near - error)` for both image axes.
The source AABB is transformed through the actual inspector camera; near
and radial extrema include a 0.1 mm transform margin. Tolerance refers to
physical framebuffer pixels. Unsupported/near-plane cases retain full detail.
T cycles 16 / 64 / 256 pixels; the wheel changes camera distance.

A qualifying resident parent can replace its pinned full-detail children.
A qualifying missing parent requests a page while drawing those children.
The supported two-patch frontier shows source, mixed and coarse choices;
it is not a general simplified rock hierarchy. The old rock hierarchy still
has identity parents and zero simplification error.

`cut-mutation.log`: 24/24 root mutants killed. The subsequent test-only
addition checks independent f64 projections of displaced 3D points against
the bound; no passing mutation gate was repeated on an unchanged body.
Threshold equality, minimal upward rounding, distance changes, near-plane
refusal, invalid inputs and missing residency are also checked.

## 3. Real page transport and bounded CPU residency

`aethyr/render/geometry/pages.coax` owns VGP1 pages: little-endian magic,
vertex/index counts and error, then position f32 words and u32 indices.
Hex is a text envelope for the existing bounded HTTP host interface; it is
not a new published host/protocol contract. The Coax native author writes
`apps/openagents-web/assets/valley/parent-{0,1}.vgp.hex`. The C face only binds
bounded same-origin GET and delivers its existing STATUS/newline inbox.
Request decisions, expected SHA-256, epoch/ticket matching, retries, decoding,
cache ownership, budgets, eviction and fallback are all Coax.

Each page is 136 encoded bytes, 72 decoded position bytes and 48 decoded
index bytes. The two-slot cache budget is 256 or 512 logical resident bytes.
B changes capacity; E flushes residency and advances the epoch. Pending
requests deduplicate and valid arrivals publish atomically. Corrupted, late,
malformed, mismatched and over-budget arrivals preserve held pages. Eviction
uses LRU with deterministic first-slot tie breaking; CPU-copy pins prevent
eviction until the mesh has been copied into the existing frame buffer.
The native tests replay the same arrival record and compare exact ownership
and counters.

The source and unpaged coarse reference remain retained for comparisons.
The cache's fixed physical capacity, source/reference meshes and bounded inbox
are counted in the unchanged 6 MB CPU arena. Logical cache savings are not
whole-process savings; host fetch buffers, browser HTTP cache and GPU memory
are outside that counter. GPU geometry continues through the existing
per-frame mesh/index stores. This is a real bounded HTTP/CPU geometry pager,
not a GPU page heap, feedback buffer, indirect renderer or streamed world.

`pages-mutation.log`: 292/292 root mutants killed, including decoding limits,
float exponent masks, exact frame lengths, every triangle/coordinate,
authentication, ticket identity, byte costs, pins, eviction, atomic refusal
and reset. No new gate uses equivalent annotations.

## End-to-end evidence

- `realm-checks.log`: all 474 checks pass, including the new three-stage
  integration, supported-class refusal on actual rock leaves, 6 MB/frame
  bounds and all prior controller/asset/cluster/hierarchy checks.
  `deep-link-check.log` additionally checks the final station hash publication
  change after that full run; the final browser run checks stable direct links.
- `native-pages.log`: native Coax page producer and hashes.
  `page-sha256-oracle.txt`: independent Python SHA-256 and field decoding
  of the authored files. Both native/Wasm expected identities agree.
- `browser.log`: real muted headless hardware WebGPU, 1280x800 and 390x844.
  Input changes actual source/parent triangle counts and projected-error
  selection. The HTTP oracle holds real fetched responses, verifies pinned
  12-triangle fallback, flushes to fence old arrivals, corrupts responses,
  and checks refusal without holes. Capacity pressure produces 256 resident
  bytes, an eviction and 10 drawn triangles; two slots produce 512 bytes and
  8 triangles. Unpaged and decoded paged GPU reference patches match exactly.
  Existing original/leaf reference-region equality, all three native rock
  hashes, movement/jump/landing/reset, phone input and noindex remain checked.
- The same browser run destroys the GPU device. Recovery preserves the exact
  state digest, resident pages and reference pixels on both viewports. The
  harness requires exactly one known device-loss diagnostic, then no other
  console errors. `browser-loss-first.log` preserves the earlier harness
  failure that treated that expected diagnostic as an unexpected error;
  state/pixel recovery had succeeded. Product behavior did not need a fix.
- PNGs retain actual mixed-parent, screen-error and paging views, with the
  phone's pixel-limit/flush/budget controls. No image editing or mockups.
- `release-build.log`: the 103,547-byte final release Wasm under the existing 160,000-byte cap.
  `artifacts.sha256` identifies source, authored pages, shader and artifact.
  Realm generation, visual-source, whitespace and tools gates pass. The
  shader and its prior passing emission/fixture evidence are unchanged.

Timing rows measure only 30 static paged-parent frames from CPU submission
through GPU queue completion. They are not GPU timestamps or the planned
60-second valley route/FPS qualification. Phone viewports on this Mac are
not qualification of a physical phone. No shared-renderer GPU ledger or
native rendering qualification is added by this slice.

Toolchain: the previously qualified Coax 0.50.0 binary with source fingerprint
`c82201e2cfff7c937fc0a29bf7010ae1ec75e7ab746ad29548bac7c2db61409e`.
The compiler's earlier release bootstrap used COAX_MUTATION=0; all 447 new
geometry/policy root mutants ran separately and were killed as recorded.

```shell
coax/bin/coax test apps/openagents-web/valleylab.coax --library
coax/bin/coax mutate aethyr/render/geometry/parent.coax --library --root-only
coax/bin/coax mutate aethyr/render/geometry/cut.coax --library --root-only
coax/bin/coax mutate aethyr/render/geometry/pages.coax --library --root-only
coax/web/tool.sh aethyr/tools/valley-pages.coax apps/openagents-web/assets/valley
coax/web/tool.sh valley-lab-check http://127.0.0.1:PORT
```

The [single roadmap](../../verse/2026-11-05-valley-lab.md) advances to a
qualified simplified nonplanar rock parent, then a rock field with actual
GPU resource/page-upload ownership. General rock simplification, occlusion,
indirect submission, large-world cell streaming, source import, saved
walkthrough and shared/native renderer qualification remain open. No book
draft or full virtualized-geometry milestone is marked complete.
