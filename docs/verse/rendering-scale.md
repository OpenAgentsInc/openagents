# Rendering scale

Everglade's triangle caps came from how the renderer held geometry, not from
what a GPU can draw. This document measures why, studies how Unreal Engine
holds far more, and orders the work that lets Everglade, the Grove, and the
web build carry more geometry. The first two items, GPU instancing and a
compact vertex, are implemented.

## Why the caps existed

Measured on the committed pack before this change (`examples/render_scale.rs`
and the zone tests):

| Quantity | Everglade |
| --- | --- |
| Placements | 7,563, of 927 distinct scene meshes |
| Triangles placed, every level | 2,890,704 |
| Vertices placed | 4,601,771, 1.59 a triangle |
| Bytes a vertex | 40 (`TexturedVertex`: position, normal, and UV in `f32`, color, light) |
| Bytes a triangle | 12 of indices; about 76 with its vertices |
| Uploaded | 208.7 MiB, in 8,144 merged cells |
| Distinct triangles in those meshes | 1,051,795 |
| Draw calls, street view | 815 to 1,322 |
| Triangles drawn, street view | 309,000 to 529,000 |

Three things set the caps:

1. **Everything resident.** Every level of detail of every 8 m cell was
   merged per material and uploaded when the zone loaded, near and far
   levels both.
2. **A copy per placement.** Merging writes a world-space copy of a model
   for every placement. Placements of meshes used at least twice held
   1,123,019 triangles that were 60,866 distinct triangles, eighteen copies
   of each on average. A tree placed 300 times cost 300 trees of memory.
3. **A fat vertex.** 40 bytes, of which 12 were a normal in three floats.

`MERGED_TRIANGLE_BUDGET` (2,900,000) was a memory budget: 209 MiB under the
renderer's 224 MiB bound and the low tier's admission. The frame budgets
(`PLACED_TRIANGLE_BUDGET`, `DRAWN_TRIANGLE_BUDGET`) are throughput budgets,
and a street frame drew well within them. The production crash during
destruction ("Renderer geometry bytes exceed the admitted quality budget")
was the same memory: 209 MiB of world left no room for debris under the low
tier's 256 MiB.

## What Unreal Engine does

Read from the Unreal Engine 5.8.3 source for study only; nothing is copied,
and every idea we adopt is reimplemented in our own code. Paths are under
`Engine/Source/Runtime/Engine/` unless noted.

- **Instanced and hierarchical instanced static meshes**
  (`Private/HierarchicalInstancedStaticMesh.cpp`, `Private/InstancedStaticMesh.cpp`).
  A mesh uploads once; each placement is a per-instance record of about
  48 bytes (a half-float 3×4 transform, an origin, lightmap offsets). HISM
  builds a cluster tree: instances sorted along the longest axis and split
  at the median, leaves of about 8,192 vertices' worth of instances, and
  parents with a branching factor of 16. Each frame it walks the tree,
  culls nodes against the frustum, narrows each node's level of detail from
  its nearest and farthest distance, and emits contiguous runs of instances
  as single draws. Instances can fade out between a start and end cull
  distance. Without a GPU scene, the records ride a per-instance vertex
  stream, which is what OpenGL ES 3.0 and WebGL2 offer.
- **Hierarchical levels of detail** (`Public/WorldPartition/HLOD/HLODLayer.h`,
  `Private/WorldPartition/HLOD/HLODRuntimeSubsystem.cpp`). An HLOD layer
  (instancing, merged mesh, simplified mesh, or approximated mesh) builds
  one proxy per grid cell, 256 m cells loaded to 512 m by default, doubling
  per level. The proxy shows exactly when its source cell is not loaded, so
  streaming, not screen size, swaps them.
- **Level of detail by screen size** (`Private/SceneManagement.cpp`). A
  mesh's screen size is its bounding sphere's projected diameter as a
  fraction of the screen, `2 · max(P[0][0], P[1][1]) / 2 · r / d`; each
  level has a threshold, scaled per platform (`FPerPlatformFloat`) and by
  `r.StaticMeshLODDistanceScale`. Scalability sets `r.ViewDistanceScale`
  from 0.4 to 1.0 and foliage density from 0 to 1.
- **World Partition streaming** (`WorldPartitionRuntimeSpatialHash.h`).
  Cells of 128 m load within 256 m of a streaming source, larger cells on
  coarser levels, sorted by distance weighted toward where the source
  faces, at most four loading at once.
- **Distance culling** (`Private/CullDistanceVolume.cpp`). A cull distance
  volume maps an object's size to a maximum draw distance; small things
  stop drawing nearer. Our `textured::DETAIL` test is the same idea.
- **Impostors and billboards.** The engine has no octahedral impostors; the
  SpeedTree importer makes a last level that is a ring of quads, and the
  material collapses the quads that face away from the camera.
- **Nanite** (`Shaders/Shared/NaniteDefinitions.h`). Clusters of 128
  triangles grouped and simplified into a DAG with a monotonic error, then
  a per-cluster screen-space error test on the GPU. Without compute, the
  cheap approximation is chunked levels of detail: per-cell chains chosen
  on the CPU by the same error test.
- **Vertex formats** (`Public/Rendering/StaticMeshVertexBuffer.h`,
  `RenderCore/Public/PackedNormal.h`). Separate streams: a 12-byte position,
  8 bytes of tangent frame in 8-bit components, half-float texture
  coordinates, and an optional color: about 28 to 32 bytes a vertex.
- **Occlusion.** Hardware occlusion queries on bounding boxes, read a frame
  late, and on mobile precomputed visibility cells of 2 m; software
  occlusion is gone from this version.

## The plan in payoff order

| Order | Work | Gain | Status |
| --- | --- | --- | --- |
| 1 | GPU instancing of repeated models | Everglade's upload from 208.7 to 116.6 MiB; the Grove's from 19.3 to 8.9 MiB | Done |
| 2 | Compact vertex, light in a texture | 40 to 28 bytes a vertex, in the numbers above | Done |
| 3 | Instanced building pieces, hidden by record | About 220,000 fewer resident triangles | Planned |
| 4 | Carved buildings as instances with a block mask | Most of the 1,260,000 carved triangles | Planned |
| 5 | 16-bit indices per cell | About 10 MiB | Planned |
| 6 | Distance residency and HLOD proxies per district | Only nearby cells hold near levels | Planned |
| 7 | Tree impostors | Far trees at 2 to 8 triangles | Planned |
| 8 | Screen-size level selection with tier factors | Phones draw less, desktops more | Planned |
| 9 | CPU occlusion by cell | Streets draw what's visible | Planned |

### 1. GPU instancing (done)

`pbr::instanced` reimplements HISM's idea for our backends. A zone places a
model with `TexturedScene::place_instanced`; when at least two such
placements share a scene mesh, the renderer uploads the mesh once, in mesh
space, and draws its placements from a static buffer of 52-byte records (a
3×4 transform and a light offset) through an instance-stepped vertex
stream. OpenGL ES 3.0, WebGL2, WebGPU, Metal, and Vulkan all draw this way,
so one code path serves desktop, phones, and the browser.

- **Clusters.** Records are grouped by mesh, primitive, and winding (a
  mirrored placement gets a reversed index list), then into runs per 8 m
  cell and level, sorted by cell. Each run is culled and given its level
  on the CPU exactly as a merged cell is, with the same hysteresis and
  the same cell box, so a frame draws the same triangles as before.
- **Draws.** Adjacent visible runs of one mesh draw in one call. A street
  frame makes about as many calls as before (780 to 1,260 against 815 to
  1,322) while drawing 2,000 to 3,000 instances.
- **One record buffer a pass.** Each draw starts at its run's first record;
  on OpenGL ES and WebGL2, which have no base instance, wgpu offsets the
  instance stream instead.
- **What stays merged.** The town hides and carves its buildings by
  rewriting their merged indices, so `scene::build_painted` places the
  town's members (`demolition::town::kit_members` and the carved models)
  merged, and everything else instanced. In Everglade 4,858 of 7,563
  placements draw as instances.

### 2. Compact vertex and light texture (done)

A vertex is 28 bytes (`instanced::GpuVertex`): position and texture
coordinate in `f32`, an octahedral normal in two 16-bit components, and the
color. Texture coordinates stay full precision because tiled ground and
walls repeat far outside 0 to 1, where half floats lose texels.

Baked light is per placed vertex, so it can't ride in a shared mesh. It
moves to a light texture, one RGBA8 texel per vertex of
`TexturedScene::merge` in the bake's own order, 2,048 texels wide with
layers of at most 2,048 rows. The vertex shader reads texel
`instance.light + vertex_index`; a merged cell's record has offset zero. A
finished bake rewrites the texture instead of the vertex buffer. The light
costs 4 bytes a placed vertex, so a further copy of a model costs about
6 bytes a triangle instead of about 76.

### 3. Instanced building pieces

The town's kit pieces repeat: 299,434 placed triangles over 77,673
distinct. Hiding a piece could write its record (a zero transform) instead
of its indices. This needs a record-edit slot beside `IndexEdits` and a
change in `demolition::town`.

### 4. Carved buildings as instances

Carved generated buildings hold 1,262,821 merged triangles, unique per
placement only because each house is painted. Moving the paint into the
record (a tint per primitive) and hiding broken blocks with a per-instance
block mask that the vertex shader reads would make them instances too.

### 5. 16-bit indices

The merged cells keep 32-bit indices so the town's edits address them
directly. Per-cell vertex offsets would allow 16-bit indices, about 10 MiB.

### 6. Distance residency and HLOD

Upload a cell's near level only within its switch distance plus a margin,
and its far level only beyond, streaming cells as the eye moves, as World
Partition loads cells by distance. A district beyond the fog's start could
draw as one merged, simplified proxy built in the pack compiler, swapped in
when its cells are not resident. This pays most once item 4 lands, since
carved buildings are most of what remains merged.

### 7. Tree impostors

A far tree as a few camera-facing cards baked from its model, in the
SpeedTree style: a ring of quads with the ones edge-on to the camera
collapsed in the vertex shader. Trees are the largest share of far
triangles.

### 8. Screen-size level selection

Choose levels by projected size rather than a fixed 80 m, with a factor per
quality tier, as Unreal scales `r.ViewDistanceScale`: phones switch nearer
and desktops farther.

### 9. CPU occlusion by cell

Precompute, per cell, which cells its streets can see, from the buildings'
boxes, and skip the rest. This needs no GPU feature, so it works on WebGL2.

## Measurements

Everglade at 1,280 by 800, from `render_scale OUT_DIR 60 everglade` with the
light settled, before (`origin/main` at `1726d56f8d`) and after:

| Quantity | Before | After |
| --- | --- | --- |
| GPU bytes, Everglade | 218,886,088 (208.7 MiB) | 122,259,484 (116.6 MiB) |
| GPU bytes, the Grove | 20,266,412 (19.3 MiB) | 9,375,084 (8.9 MiB) |
| Placed triangles, Everglade | 2,890,704 | 2,890,704 |
| Bytes a vertex | 40 | 28, plus 4 of light texture |

A frame, read back as a capture, in milliseconds over three alternating
rounds of 60 frames each. The machine ran other agents' builds at a load
average near 100, so medians move by 2 ms between rounds; the fastest frame
is the steadier figure. Draws and triangles are the main view's
(`TexturedScene::frame_cost`); after, the renderer drew 1,235 to 1,868
calls a frame over all its passes, shadow cascades included.

| View | Fastest, before | Fastest, after | Median, before | Median, after | Draws, before | Draws, after | Instances drawn | Triangles, before | Triangles, after |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| spawn | 3.36 | 4.39 | 5.15 | 9.23 | 1,092 | 1,160 | 2,673 | 491,021 | 490,619 |
| market | 5.98 | 4.22 | 8.27 | 7.48 | 815 | 780 | 2,380 | 455,808 | 455,794 |
| lantern | 3.42 | 4.18 | 7.87 | 7.57 | 1,264 | 1,125 | 3,036 | 456,946 | 456,368 |
| brownstone | 3.24 | 5.58 | 6.77 | 8.38 | 846 | 798 | 1,966 | 309,510 | 309,350 |
| foundry | 3.56 | 7.82 | 7.61 | 9.69 | 1,302 | 1,172 | 2,956 | 513,688 | 513,294 |
| observatory | 3.46 | 4.41 | 5.81 | 9.25 | 1,322 | 1,219 | 2,944 | 528,682 | 528,622 |
| south-edge | 3.42 | 4.20 | 7.00 | 6.94 | 1,301 | 1,244 | 2,607 | 490,275 | 490,257 |
| roof-30m | 3.35 | 4.83 | 6.78 | 9.22 | 1,302 | 1,260 | 2,774 | 528,527 | 528,169 |
| air-80m | 3.38 | 4.60 | 8.96 | 8.94 | 908 | 878 | 1,706 | 322,426 | 322,212 |
| air-200m | 2.77 | 3.40 | 5.81 | 4.92 | 0 | 0 | 386 | 0 | 0 |

The fastest frame is about 1 ms slower after, at the same triangles and
fewer draws. The likely costs are the vertex shader's light fetch and
normal transform, and the CPU's run bookkeeping; GPU timestamps would
separate them, and the next renderer change should add them before it
measures.

In headless Chrome (`web_measure.py`, the spawn view, 1,280 by 800), both
builds hold 60 frames a second with WebGL2 and WebGPU. The page's work per
frame, which includes the browser's light bake steps, was 9.7 to 11.4 ms
before and 11.4 to 12.3 ms after on WebGL2, and 11.1 to 13.8 ms before and
6.6 to 12.3 ms after on WebGPU.

Before-and-after captures at ten views from the street to 200 m up, and at
three Grove views, differ by a mean of at most 0.003 of 255 a channel, and
at most 0.002 percent of pixels differ by more than 8: single edge pixels,
where a vertex transformed on the GPU lands a rounding step away from the
one transformed on the CPU. The web captures differ only where the scene
moves between screenshots: chimney smoke, birds, and the idle characters.

## Budgets

| Budget | Before | Now | Why |
| --- | --- | --- | --- |
| Resident GPU bytes, Everglade (`RESIDENT_BYTES_BUDGET`) | none, 209 MiB used | 160 MiB, 117 MiB used | Leaves 160 MiB of the low and medium tiers' 320 MiB, 64 MiB of it the destruction reserve |
| Triangles placed, every level (`MERGED_TRIANGLE_BUDGET`) | 2,900,000 | 4,000,000 | Bounded now by the light bake's CPU merge and the light texture, not GPU memory |
| Triangles placed where they draw (`PLACED_TRIANGLE_BUDGET`) | 1,650,000 | 1,650,000 | Throughput; instancing draws the same triangles |
| Triangles a street frame draws (`DRAWN_TRIANGLE_BUDGET`) | 600,000 | 600,000 | Throughput; raise with items 7 and 8 |
| Renderer bound (`textured::MAX_BYTES`) | 224 MiB of merged geometry | 224 MiB of GPU layout | Unchanged |
| CPU merge for the bake (`textured::MAX_MERGE_BYTES`) | none | 448 MiB | New bound on what the bake builds |

Instancing makes a repeated model nearly free in memory: another hundred
4,000-triangle trees cost about 2.6 MiB of light and records, where merged
they cost 30 MiB. Unique geometry still costs about 63 bytes a triangle, its light included.
The light bake now limits how much a zone places: it lights every placed
vertex, and in a browser it advances 192 vertices a frame. Baking instances
against a per-model occlusion and the probe grid would lift that limit.
