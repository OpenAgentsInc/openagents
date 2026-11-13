# Spartan Engine audit

Date: 2026-11-07. Subject: [PanosK92/SpartanEngine](https://github.com/PanosK92/SpartanEngine) at
`0baa732` (2026-11-08), read from the reference clone at
`~/work/projects/repos/SpartanEngine` (the last 477 commits, back to 2026-07-24). We read the
source, the shaders, `docs/`, `readme.md`, `plan.md`, the commit history and the engine's own agent
notes (`tools/mcp/spartan_engine/AGENT_MEMORY*.md`). We did not build it.

This audit compares Spartan with Metaverse Engine/Verse on Coax and ranks what Everglade II should adopt.
Paths of the form `source/...` and `data/shaders/...` are in Spartan's repo. All other paths are
in this repo. "Archive line N" means line N of Spartan's
`tools/mcp/spartan_engine/AGENT_MEMORY_ARCHIVE.md`.

## Summary

Spartan is a one-engineer C++20 engine (about 400k lines of C++ and 53k lines of HLSL, not
counting `third_party/`). It ships a 30 km open island with forests, grass and an FFT ocean.
Its renderer is GPU-driven and bindless, with ReSTIR path tracing on top. Most of that
does not port to WebGPU. Its vegetation pipeline does port, and it solves the problems the owner
hit in Everglade II this week:

- Grass and pebbles are placed from world-keyed hashes on a world-snapped grid, so nothing
  re-places as you move.
- LOD tiers fade into each other probabilistically, and noise warps the boundaries between them.
  Each instance keeps its own random rank, so no ring edge shows.
- Distant trees thin out continuously. An instance survives with probability 1/f² and the
  survivors widen by up to 2x. The keep test hashes the world position, so the kept set never
  changes from frame to frame.
- Past the last grass ring, the terrain shader paints the colour of the grass carpet. The far
  ground matches the grass, and the end of the grass doesn't show.
- The shadow cascades are snapped in light space by whole texels, and a far cascade covers 2 km.

Licence: since 2026-09-24 Spartan is under a custom **noncommercial** licence. We may study the
ideas but must not copy current code. See [Licence](#licence).

## Licence

| Version | Licence | What we may do |
| --- | --- | --- |
| `230a887` (2026-09-24) and later | Spartan Engine License 1.0 (`license.md`): copy, modify and distribute for noncommercial purposes only. Commercial use, including "commercial reuse of covered code in another project", needs a written, paid agreement. Attribution is required. No trademark rights. | Read and learn. Do not copy code, shaders or data into OpenAgents, which is commercial. |
| Before `230a887` | MIT (`git show 230a887^:license.md`) | The licence allows copying with the MIT notice. We still don't: everything we write is Coax. |
| `third_party/*`, `bend_sss_cpu.h` (Sony, Apache-2.0), FidelityFX (MIT), XeGTAO (MIT), meshoptimizer (MIT) | Their own licences | Take them from their upstreams if we ever need them, not from Spartan. |

Algorithms and techniques are not covered by copyright. This audit describes them in our own words
so that we can reimplement them in Coax. Name the source in a comment when a Coax function follows
a Spartan technique closely, for example "world-hash thinning as in Spartan's impostor_thin_keep".

## Architecture

| Area | What Spartan does | Files |
| --- | --- | --- |
| Shape | One executable. Subsystems are static classes with `Initialize`/`Tick`/`Shutdown`. Gameplay is components and Lua. No C++ game layer. | `docs/architecture.md`, `source/core/Engine.cpp` |
| Frame | `Engine::Tick`: input, world tick, start physics on a worker, render while physics runs, then join. | `docs/architecture.md` "The frame" |
| RHI | One interface (`RHI_Device`, `RHI_CommandList`, ...) with D3D12, Vulkan and Metal 4 backends, chosen at build time. Pass setters only record state. The pipeline resolves at Draw/Dispatch. | `source/rhi/`, `source/rhi/RHI_Implementation.h` |
| Shaders | One HLSL source, compiled to DXIL, SPIR-V, and Metal through the Metal Shader Converter. CPU and GPU share structs through `shared_buffers.h`. Shaders hot-reload, and a failed reload keeps the last good module. | `data/shaders/shared_buffers.h`, `source/rhi/RHI_Shader.cpp` |
| Render graph | None. `Renderer::ProduceFrame` is a hand-ordered list of passes on three queues (graphics, compute A, compute B) synchronized by timeline semaphores. | `source/rendering/Renderer.cpp:6977` |
| GPU-driven | One global vertex and index buffer. Shaders pull vertices from it. Per-draw data lives in one bindless storage buffer (up to 32,768 draws), and push constants carry an index. Compute culls instances and meshlets (frustum, two-phase Hi-Z, backface cones). One `DrawIndexedIndirectCount` per pass. Mesh shaders when available. | `docs/rendering.md`, `source/rendering/GeometryBuffer.cpp`, `data/shaders/instance_cull.hlsl`, `cull_phase.hlsl` |
| Memory | mimalloc behind a tagged allocator. Overrides global `new`. Frame allocator. VMA on Vulkan. | `source/memory/` |
| Textures | Mip-residency streaming within a budget, with hysteresis and one extra mip of slack. Under pressure it evicts first, otherwise it serves the largest detail deficit. Compression (Compressonator) and mips (FidelityFX SPD) run on the GPU at load. | `source/rhi/RHI_TextureStreaming.cpp:159,286,319` |
| Threads | One thread pool sized to the hardware. `ParallelLoop` runs inline when called from a worker. Entities built off-thread are committed in batches. The main thread owns world, renderer and Lua. | `source/core/ThreadPool.cpp`, `docs/architecture.md` "Threading" |
| World | Entity tree, one component of each type per entity. Worlds are XML. Load runs on a worker, then a "preparation" phase builds terrain and roads and caches them in `generated_cache/`. | `docs/world.md`, `source/world/World.cpp` |
| Physics | PhysX with fixed 5 ms steps. A vehicle sim at 200 Hz with Pacejka tyres. | `source/physics/`, `source/car/` |
| Audio | A mixer over per-component sources. Engine and tyre sounds are synthesized. | `source/audio/`, `source/car/CarEngineSoundSynthesis.cpp` |
| Assets | Assimp import, meshoptimizer LODs and meshlets baked into `.mesh` files, generated caches keyed by content hash. | `source/resource/`, `source/geometry/GeometryProcessing.cpp` |
| Editor | ImGui editor and a game builder. An MCP bridge lets agents drive the engine: screenshots, cvars, viewpoint captures, shader reloads. | `source/editor/`, `source/mcp/`, `tools/mcp/spartan_engine/` |

## Rendering techniques

| Technique | What Spartan does | Files |
| --- | --- | --- |
| GI | ReSTIR path tracing (spatiotemporal reservoirs), NRD denoising, a reference path tracer. Hardware ray queries. No probes, no bake. | `data/shaders/restir_*.hlsl`, `path_tracing.hlsl` |
| Lighting | Clustered deferred shading on a GPU-built logarithmic-Z grid. Physical units. IES profiles. | `light_cluster*.hlsl`, `light.hlsl` |
| Shadows | One 8192² atlas. The sun uses two cascades that follow the camera: a near one (extent set per light) and a far one 2000 m wide, with depth 2 km behind and 8 km ahead. Both snap to whole texels in light space. 32-tap Vogel disk, PCSS with blocker separation converted to metres, near-to-far blend over 0.8 to 0.95 of the near map, edge fade on the far map. Bend-style screen-space contact shadows. RT shadows when enabled. | `source/world/components/Light.cpp:36-41,837-905`, `data/shaders/shadow_mapping.hlsl:12-23,116-141,228-334` |
| Anti-aliasing | Its own TAAU: a single pass, phase-weighted, five-tap Catmull-Rom history, 2.3 KB LDS per 8x8 group, about 0.35 ms. Also DLSS, XeSS, FSR 3.1/4, FSR frame generation and FXAA. | `data/shaders/taau.hlsl:8-10,77` |
| SSAO | XeGTAO. | `data/shaders/ssao.hlsl` |
| Reflections | RT reflections. One function decides how much of the specular lobe RT owns, so the trace, apply and IBL passes stay consistent. | `reflections_*.hlsl`, agent note archive line 93 |
| Sky | Atmosphere LUTs (sky view). Nubis-style volumetric clouds baked into a sky panorama. SH projection of the panorama for ambient light. | `data/shaders/sky/`, `source/rendering/Renderer_Passes_Sky.cpp:50,155` |
| Fog | One view-aligned froxel volume for height and distance fog, sun shafts and underwater shafts, with temporal reprojection. | `data/shaders/fog_froxel.hlsl` |
| Terrain | A heightfield in tiles with meshoptimizer LODs. Detail "pages" with about 1 m spacing where pads and roads reshape a coarse 12.5 m cell. Page edges pin to the coarse edge, so the meshes meet without a step. Baked analysis channels (curvature, flow, occlusion, deposition, wear, insolation, talus). Per pixel, the shader scores every layer rule against those channels, keeps the top few, and blends them by height. Hex tiling (Mikkelsen 2022), biplanar projection on steep slopes, rule thresholds jittered by fbm so boundaries don't follow contour lines. | `source/world/components/Terrain.cpp`, `TerrainDetail.cpp:32-44`, `data/shaders/common_terrain.hlsl:10-85,164,439,580` |
| Placement | Trees, rocks and understory are placed on the CPU per tile. The CPU scorer is an exact twin of the surface shader's layer function, so props never stand on ground the shader painted as something else. | `source/world/components/TerrainProps.cpp:67,143,793-830` |
| Grass | GPU-placed per frame (Ghost of Tsushima style). See [Grass and vegetation](#grass-and-vegetation-in-detail). | `data/shaders/grass_*.hlsl`, `Renderer_Passes_Geometry.cpp:2106,2872` |
| Trees | A procedural tree generator. LODs selected by screen coverage, with hysteresis on the CPU path. Hemi-octahedral impostors as the last LOD. Thinning of distant impostors. | `source/geometry/TreeGenerator.cpp`, `source/world/components/Render.cpp:1108-1206`, `data/shaders/common_culling.hlsl:239-288`, `common_vertex_processing.hlsl:990-1100` |
| Water | FFT ocean (Tessendorf spectrum, multiple IFFT cascades in compute) on a clipmap that follows the camera. | `source/rendering/Renderer_Passes_Ocean.cpp` |
| Culling | Frustum, two-phase Hi-Z occlusion, backface cones per meshlet, a 2 px contribution cull. | `instance_cull.hlsl`, `cull_phase.hlsl` |
| Bindless | Everything: materials, lights, samplers, textures. | `common_resources_bindless.hlsl` |
| Mesh shading | Yes, when available (`r.mesh_shaders`). Otherwise vertex pulling. | `meshlet_mesh*.hlsl` |
| Post | DoF, motion blur, auto exposure, bloom, tonemapping (GT7 by default, also ACES and AgX), CAS, film grain, lens rain. | `Renderer_Passes_Post.cpp` |

### Grass and vegetation in detail

This is the part that matters most for Everglade II.

1. **World-locked placement.** The grid snaps to `floor(camera / cell)`, and every random value
   hashes the cell's integer world coordinates. "The same blade lands at the same world position
   regardless of the camera path. Only ring-boundary cells appear and disappear."
   (`grass_populate.hlsl:71-73`). A second hash goes through a full avalanche mix, because a
   multiply or xor of the first hash puts the scatter on visible parallel rows
   (`grass_patch.hlsl:30-45`).
2. **Overlapping, warped LOD fades.** Each ring fades in and out over a wide band (at least
   10 cells, or 42% of the ring radius). Two noise octaves (34 m and 13 m) warp the distance, and
   each edge warps by its own transition width. A blade survives when its own stable hash falls
   under `fade_in * fade_out * keep`. So a ring seam is a soft blob, never a circle
   (`grass_populate.hlsl:403-505`). The agent notes record that a shared warp amplitude "tore holes
   hundreds of metres deep at the blade-to-card seam" (archive line 123).
3. **Three tiers out to the horizon.** Real blades fill rings 0 and 1 (out to about 90 m, where
   3 cm blades become sub-pixel). Analytic tuft cards fill the last ring, out to 1.5 to 8 km.
   Past that, the terrain shader paints the carpet (archive line 122). The cards are cut out
   analytically (32 tufts per card) with no atlas. A tuft thinner than a pixel widens to one
   pixel, and a dither pays back the extra area (`g_buffer.hlsl:182-227`).
4. **Thinning with coverage held.** Past the thinning distance a card survives with probability
   `(thin/d)²`, and the vertex shader widens it by `d/thin`. Screen coverage stays constant while
   the card count grows only with log(reach): about 0.8 ms of grass at 8 km
   (`grass_populate.hlsl:436-446`, archive line 123). Distant tree impostors use the same scheme:
   cards under 16 px survive with `1/f²` from a world-position hash and widen by `min(f, 2)`
   (`common_vertex_processing.hlsl:1009-1043`). On the island view this cut GPU time from 48 ms to
   22 ms (`AGENT_MEMORY.md:27`).
5. **The far carpet.** `grass_far_cover` uses the same biome, ground, slope and patch rules as the
   populate pass to tint the terrain with the carpet colour. It lays 45% of the carpet colour
   under the blade rings (the ground between tufts reads as thatch, not soil) and 100% past them.
   Once a pocket covers only a few pixels, its outline relaxes to its average coverage so it
   doesn't alias. Beer-Lambert through the blade layer makes grazing views hide more ground
   (`g_buffer.hlsl:229-300`).
6. **Patches.** Domain-warped fbm, pushed through a Gaussian CDF to a uniform value and
   thresholded at `1 - coverage`, decides where grass "took hold". Because the value is uniform,
   coverage means exactly the share of ground covered, and the CPU can raise density honestly to
   compensate. An inverted slot (pebbles) tiles the bare ground with no gap and no overlap
   (`grass_patch.hlsl:56-150`).
7. **Geometry LOD by pixel error.** Near blades switch to a coarse mesh only when the bend error
   (an analytic bound over the wind envelope, at the current and previous frame) is under a pixel
   budget (`grass_populate.hlsl:660-700`). Mesh LODs cap each level's simplification error at about
   1.5 px of a 1080p view at the largest coverage that level is drawn at (`source/geometry/Mesh.h:77`,
   archive line 190).
8. **Tree LOD.** Selection uses screen coverage, never distance: 0.20/0.10/0.048/0.024/0.012 of
   screen height. Changing level needs a 10% margin past the threshold (`Render.cpp:1158-1206`). The
   GPU copy uses the same thresholds (`common_culling.hlsl:239-271`). Impostors take over below
   12% of the screen (`common_culling.hlsl:273-288`). There is no dithered crossfade: TAAU hides
   the swap.
9. **Impostors that light like the mesh.** Each impostor texel stores the source mesh's **UV** and
   a packed normal, depth and coverage, in two layers. The G-buffer samples the real albedo texture
   through that UV and lights the crown normal with the same deferred lighting
   (`common_vertex_processing.hlsl:1057-1100`, `g_buffer.hlsl:495-505`). The colour always matches
   the mesh. One fix was needed: cards lacked crown self-occlusion and looked "pale teal", so
   impostor occlusion is multiplied by 0.4 (archive line 107).

## Performance practice

- **Named viewpoints.** Every change is timed at the same saved views: `lod_ground`,
  `forest_mid`, `island_oblique`, `horizon_low` (`AGENT_MEMORY.md:27`).
- **Fresh against fresh.** Grass and terrain stream differently in long sessions, and auto
  exposure needs about 10 s to settle, so a fresh load is only compared with a fresh load
  (`AGENT_MEMORY.md:40`, archive lines 23, 90, 93).
- **Per-pass GPU time.** A timestamp pair brackets every `BeginPass`/`EndPass`, and GPU time is
  calibrated against the CPU clock so both appear on one timeline. A CSV capture records it
  (`source/profiling/Profiler.h:41,56,82`, `Profiler.cpp:128-175`).
- **Percentile lows.** The overlay reports p99 and p99.9 frame times, because "percentile lows
  describe stutter far better than a min that is dominated by one hitch" (`Profiler.cpp:2307-2321`).
- **Work counters.** One counter per kind of work in each world tick (culls, LOD updates, bounds
  rebuilds, ...), kept per thread and merged after the join with no atomics
  (`source/profiling/WorldWork.h`).
- **Measured non-wins are recorded too**: custom mesh LOD thresholds, VRS (under 1 ms), dropping
  local-light shadow rays. The coarsest terrain LOD saves only about 1.5 ms because pixel shading
  dominates (archive lines 107, 128; `AGENT_MEMORY.md:27`).
- **CPU fixes.** Draw preparation went from 6.2 ms to 2.4 ms. The fix replaced per-draw mutex and
  map lookups with one snapshot per frame and split serial staging from parallel fills (archive
  line 194).

## Code quality

- Comments explain the reason in plain sentences, often with the failure that led to the
  rule. The grass and terrain shaders read like a design log. This is the best part of the
  codebase to learn from.
- Files are very large: `Renderer.cpp` has 7,406 lines, `Terrain.cpp` 5,632 and `Spline.cpp` 4,307.
  Global static singletons are everywhere.
- Experiments ship on master. `grass_populate.hlsl:771-792` carries `// EXPERIMENT` code: the
  banded card-ring walk counts candidates and never calls `populate_cell`, so that path draws
  nothing at this commit.
- Tests are thin. CI runs a start-up smoke test without a GPU, and a GPU smoke suite runs by hand
  (`docs/testing_and_ci.md`). There are no unit tests for the shaders' math. Our `#test` blocks
  on shared CPU/GPU functions cover more than this.
- Commit messages are terse (`misc: ...`, `plan: ...`). The agent notes carry the rationale
  instead.

## Our engine today

| Area | Metaverse Engine/Verse | Where |
| --- | --- | --- |
| Shaders | Coax lowered to WGSL (`coax wgsl`) for the browser, and to SPIR-V and Metal for native hosts. Shared functions run on the CPU in `#test` blocks. | `coax/web/wgsl-module.coax`, `metaverse-engine/gpu/wgsl.coax`, `verse/gpu/spirv.coax`, `verse/gpu/metal-pass.coax` |
| Draws | The CPU picks visible pieces and sorts them into one instanced draw per kit piece and LOD: about 75 draws per frame. No compute pass and no indirect draws in the web pages. | `apps/openagents-web/everglade2.coax:626-670` |
| Grass | Placed by the vertex shader from the cell hash, in four nested tiers fixed to the land (0.5/1/2/4 m cells out to 28/40/52/140 m). Each coarser tuft is one of the four finer tufts it covers. Past 16 m, tufts drop out by rank and the survivors widen. | `everglade/world/field.coax`, `apps/openagents-web/gpu/everglade2.coax:52-74` |
| Trees | Three LODs at hard distances: whole to 36 m, crown cards to 70 m. Past 70 m only items with `q % 3 == 0` remain, scaled 1.3x. Past 160 m only `q % 6 == 0`, scaled 1.5x. | `everglade/world/plant.coax:21-23`, `apps/openagents-web/everglade2.coax:648-663` |
| Understory | Each kit has a hard cutoff distance: pebbles and mushrooms 26 m, flowers 45 m, ferns 80 m, bushes 95 m. | `everglade/world/plant.coax:21-22` |
| Shadows | One map 68 m across, centred on the player and snapped in world xz to 2 m steps. Its y follows the terrain height. Only pieces within 40 m cast, all at LOD 1. The OpenAgents Metaverse Engine has light-space texel-snapped cascades (`metaverse-engine/render/cascades.coax`), but Everglade II doesn't use them. | `apps/openagents-web/everglade2.coax:28,562-564,648-652`, `apps/openagents-web/gpu/everglade2.coax:90-116` |
| AA | No temporal AA. DPR 2. | |
| Quality | A governor steps the resolution and costs down and up with hysteresis. | `metaverse-engine/core/quality.coax` |
| Measurement | `frametime.ts`: per-pass GPU timestamps in headless Chrome. `perf.ts`: p95 must stay at or under 17.5 ms and no frame may exceed 33 ms, at four Everglade v1 places. Everglade II: about 16 ms at the glade and 17 ms at the clearing, at 2400x1400. | `coax/web/frametime.ts`, `coax/web/perf.ts`, `docs/verse/everglade2.md` |

The grass is already in good shape: it is world-locked and nested, and it only thins. The trees,
the understory cutoffs, the grass's outer edge and the shadow map are where the pops come from.

## Lessons to adopt, ranked

Effort: S is under a day, M is a few days, L is a week or more. "WebGPU" says whether the technique
runs on our browser host as it stands.

### 1. Thin distant trees continuously, by a world hash

- **Spartan.** `impostor_thin_factor`, `impostor_thin_scale` and `impostor_thin_keep`
  (`data/shaders/common_vertex_processing.hlsl:1009-1043`). `f` is how many times shorter than
  16 px the tree is on screen. The tree survives when `hash(world position) * f² <= 1`, and the
  survivors widen by `min(f, 2)`. The keep test is stable across frames, views and both cull
  phases.
- **Why.** Our `q % 3` and `q % 6` cuts remove two thirds of the trees at exactly 70 m, and then
  half of the rest at 160 m. The survivors jump from 1.0x to 1.3x to 1.5x. This is the distant-tree
  swap the owner disliked.
- **Map.** In `e2_pieces` (`apps/openagents-web/everglade2.coax:656-663`), replace the modulo test
  and the step `grow` with:
  - `f` as a continuous function of projected height (`dd / (focal * tree height)`);
  - keep the tree when `e2_unit(hash(x, z)) * f² <= 1`;
  - scale by `min(f, 2)`;
  - add a shrink band like `e2_field_grow`, so a dropped tree shrinks over a few metres instead of
    vanishing.

  Put the function in `everglade/world/plant.coax` with a `#test` that keep is monotonic in
  distance and that coverage (count x scale²) stays constant.
- **Effort.** S. **WebGPU.** Yes. It is CPU code today, and it could move to the vertex shader.

### 2. Select tree LOD by screen coverage, with hysteresis and a dithered handover

- **Spartan.** Coverage thresholds 0.20/0.10/0.048/0.024 of screen height, with a 10% hysteresis
  band (`source/world/components/Render.cpp:1158-1206`). The GPU copy uses the same numbers
  (`data/shaders/common_culling.hlsl:239-271`).
- **Why.** Our 36 m and 70 m switches ignore tree size and field of view, and a tree on the line
  flickers between levels. Spartan relies on TAAU to hide its swaps. We have no TAA, so we also need
  a screen-door handover: for about 0.3 s, or over a few metres, draw both levels and discard
  complementary pixels with a hash keyed by instance and pixel.
- **Map.** In `e2_pieces`, compute coverage from the instance's height and `s[0].focal`. Keep each
  piece's last LOD in a small per-item byte array next to `items`. Pass a 0-to-1 fade in the
  instance's spare float, and discard by `fade` against an interleaved-gradient value in the kit
  fragment shader in `apps/openagents-web/gpu/everglade2.coax`. Give the shadow pass the same LOD
  as the view (lesson 5).
- **Effort.** M. **WebGPU.** Yes. Discard works in WGSL. Keep it out of the depth-only pass's hot
  path where possible.

### 3. Paint the grass carpet onto the far ground

- **Spartan.** `grass_far_cover` (`data/shaders/g_buffer.hlsl:229-300`). The terrain uses the same
  gates as placement. It takes 45% of the carpet colour under the rings and 100% past them, and
  patch outlines relax to their mean once they are a few pixels wide.
- **Why.** Our grass ends at 140 m (`everglade/world/field.coax:22`) on ground that is shaded
  separately (gold, litter, noise). In the clearing and along the open path, the grass edge reads as
  a line where the colour and texture change.
- **Map.** In the `ground` branch of `e2_environment_fs` (`apps/openagents-web/gpu/everglade2.coax:125`),
  mix towards the tuft colour that `e2_field_vs` computes. Use the same `e2_field_cover`, `e2_gold`
  and `e2_forest`, weighted by `smoothstep(E2_FIELD_NEAR, E2_FIELD_FAR, d)` up to 1. Move the tuft
  colour into a shared function in `field.coax` so the vertex and ground shaders can't drift apart,
  and test it there.
- **Effort.** S. **WebGPU.** Yes.

### 4. Fade every hard cutoff with a warped, per-instance rank

- **Spartan.** Overlapping fades, each edge warped by its own noise amplitude, and a survival test
  on a stable per-instance hash (`data/shaders/grass_populate.hlsl:403-505`).
- **Why.** Our understory pops in at fixed circles: pebbles and mushrooms at 26 m, flowers at 45 m,
  ferns at 80 m (`everglade/world/plant.coax:21-22`). The grass's last tier fades along a perfect
  circle at 140 m. While you walk, a ring of pieces appears ahead of you.
- **Map.** Turn `E2_KIT_FAR[kit]` into a fade band (say from 0.7 to 1.0 of the reach). Keep a piece
  when `rank(x, z) < fade(d + warp(x, z))`, and shrink it over the last share, as `e2_field_grow`
  does. Use the same warp for the grass's last tier in `e2_field_rel(3, d)`. Add a `#test` that the
  survival curve is monotonic and has no step.
- **Effort.** S. **WebGPU.** Yes.

### 5. Snap the shadow map in light space, and add a far cascade

- **Spartan.** Snaps each cascade's translation to whole texels in light space
  (`source/world/components/Light.cpp:853-858`). A near cascade plus a 2000 m far cascade, blended
  over 0.8 to 0.95 of the near map, with an edge fade on the far map
  (`data/shaders/shadow_mapping.hlsl:290-325`).
- **Why.** We snap the map's centre in world xz to 2 m steps and set its y from the terrain
  (`apps/openagents-web/everglade2.coax:562-564`). Neither is a whole number of texels in light
  space, so every step shifts the shadow texels by a fraction and the edges crawl. Only pieces
  within 40 m cast, so the woods past about 34 m have no shadows at all. Distant forest looks flat,
  and shadows pop on at the edge of the map.
- **Map.** Everglade II should use `metaverse-engine/render/cascades.coax`, which already snaps by texels in
  light space and redraws a tile only when its snapped view changes. Use two tiles on the Mac: a
  near tile of about 40 m and a far tile of about 300 m, which covers the full 300 m tree reach at
  LOD 2 or with cards only. Blend between tiles as Spartan does. Draw casters with the same LOD and
  thinning as the view, or the far tile shows trees that the view has dropped.
- **Effort.** M. **WebGPU.** Yes. The far tile redraws rarely, so its cost is mostly memory.

### 6. Measure Everglade II the way Spartan does

- **Spartan.** Named viewpoints, fresh-against-fresh comparisons, per-pass GPU time on a CPU
  timeline, CSV capture, and p99 and p99.9 lows (`Profiler.cpp:2307-2321`, `AGENT_MEMORY.md:27,40`).
- **Why.** We read Everglade II's frame rate from one counter and from one-off headless runs at two
  places. Every lesson above changes cost, and we need to see each change against a fixed baseline.
- **Map.** Add Everglade II cases to `coax/web/perf.ts`: `?at=0`, `?at=200`, `?at=400`, the
  clearing, looking up, and looking down the path at 0.3 rad. Run them at 2400x1400, DPR 2. Report
  p95, p99 and p99.9 next to `frametime.ts`'s per-pass split. Keep fresh loads only, wait for the
  scene to settle, and record non-wins in the realm's doc.
- **Effort.** S. **WebGPU.** Yes. Timestamp queries already work.

### 7. Light tree cards with the mesh's own material and normals

- **Spartan.** Impostor texels store the source UV and a crown normal. The G-buffer samples the
  real albedo and lights it with the same pipeline (`common_vertex_processing.hlsl:1057-1100`,
  `g_buffer.hlsl:495-505`). Occlusion is multiplied by 0.4 for cards (archive line 107).
- **Why.** At each handover our crown cards must match the whole tree in colour and brightness, or
  the swap shows even with a dither. Spartan found that cards without crown self-occlusion read
  pale.
- **Map.** In `verse/tools/assets/everglade2_pack.coax`, bake each card's normals as a sphere-ish
  crown normal and its AO from the whole tree, not from the flat card. In
  `apps/openagents-web/gpu/everglade2.coax`, run cards through the same `leaves` branch with that
  normal and AO. Compare whole and card at the handover distance in a golden pair.
- **Effort.** M. Full hemi-octahedral impostors would be L. **WebGPU.** Yes.

### 8. Keep one land function for placement and shading

- **Spartan.** The CPU prop scorer is a line-for-line twin of the surface shader's layer function,
  with constants that "must move together" (`source/world/components/TerrainProps.cpp:793-830`).
- **Why.** We already do this, and better: `everglade/world/forest.coax` is one Coax function run
  by both the CPU and the GPU. Keep it that way. Every new placement or colour rule from lessons
  1, 3 and 4 goes into `forest.coax` or `field.coax`, never into the shader alone.
- **Effort.** None. This is a rule to keep.

### 9. Grow grass in patches, not as an even carpet

- **Spartan.** Domain-warped fbm, normalized to a uniform value by a CDF and thresholded at the
  requested coverage, with the density raised to compensate, so meadows grow in pockets that are
  thick in the middle and frayed at the edge (`data/shaders/grass_patch.hlsl:56-150`).
- **Why.** Our cover is a smooth product of the path and woods terms (`field.coax:43-47`). Pockets
  would look grown, and the compensation keeps the tuft budget the same.
- **Map.** Add `e2_field_patch(x, z)` to `field.coax` and multiply it into `e2_field_cover`. Raise
  the near tier's density by `1/coverage`, capped. Lesson 3's carpet must use the same patch term
  (at its mean once pockets are sub-pixel).
- **Effort.** S. **WebGPU.** Yes.

### 10. Cap mesh LOD error in pixels

- **Spartan.** `mesh_lod_max_error` caps each level's simplification error at about 1.5 px at the
  largest coverage that level is drawn at. A level that can't drop 10% of its triangles ends the
  chain (`source/geometry/Mesh.h:77`, archive line 190).
- **Why.** Our kit LODs are hand-built cards. As we add simplified meshes (rocks, bushes, the
  glade's composition), a pixel-error rule ties each level to the coverage it is drawn at and
  prevents silhouette pops.
- **Map.** The pack tool's simplifier (`verse/tools/assets/`) and the coverage thresholds from
  lesson 2.
- **Effort.** M. **WebGPU.** Not applicable: this runs offline.

### 11. Later: GPU-driven population and culling

- **Spartan.** Compute populates the grass with atomics into per-ring sections, writes the indirect
  arguments, and culls each candidate against the frustum and Hi-Z before allocating
  (`grass_populate.hlsl`, `grass_lod_allocation.hlsl`, `instance_cull.hlsl`).
- **Why.** Our vertex shader runs all 465 vertices of every tuft in a visible block, even when the
  tuft is rejected. A compute pass would cull per tuft and draw only survivors, which helps once the
  grass reach or density grows.
- **Map.** Needs compute entries in the WGSL emitter (`coax/web/wgsl-module.coax` lowers vertex and
  fragment entries only), `drawIndexedIndirect` in the host, and storage-buffer atomics. WebGPU has
  no multi-draw-indirect-count and no wave intrinsics without the optional `subgroups` feature, so
  use one indirect draw per tier and a workgroup-shared prefix sum.
- **Effort.** L. **WebGPU.** Yes, with those limits.

### 12. Later: temporal AA

- **Spartan.** A single-pass TAAU at about 0.35 ms (`data/shaders/taau.hlsl`), tuned over many
  commits for foliage, ghosting and speculars (`d2dd151`, `e841ed1`, `4c9b667`).
- **Why.** It would remove grass and leaf shimmer, and it would let hard LOD swaps through as
  Spartan's do. But it needs motion vectors for every instanced, wind-bent vertex, and tuning it
  for thin foliage cost Spartan weeks.
- **Map.** A post pass in `metaverse-engine/render/post.coax`, with the previous frame's transform in every
  vertex stage.
- **Effort.** L. **WebGPU.** Yes. Do lessons 1 to 5 first, because they remove the pops without it.

## What not to copy

| Item | Why not |
| --- | --- |
| Any code, shader or data from Spartan at or after `230a887` | Noncommercial licence. Reimplement the ideas in Coax and name the technique. |
| ReSTIR, path tracing, RT shadows and reflections, NRD | WebGPU has no ray tracing, and the cost doesn't fit 60 FPS at 2400x1400 on a Mac. |
| DLSS, XeSS, FSR, frame generation | Vendor SDKs, native only. FSR frame generation also adds latency. |
| Bindless descriptor arrays, mesh shaders, `DrawIndexedIndirectCount`, wave intrinsics | WebGPU lacks them. Texture arrays and storage buffers, which we use now, are the portable form. |
| An 8192² shadow atlas with a 32-tap Vogel PCSS | Too much memory and fill on a Mac browser. Use 2048 to 4096 tiles and 8 to 16 taps. |
| Three-queue async compute | WebGPU has one queue. |
| Hard LOD swaps hidden by TAA | We have no TAA. Use dithered handovers (lesson 2). |
| Static-singleton subsystems and multi-thousand-line files | Our module and `#test` layout is easier to verify and mutate-test. |
| Lua and visual scripting | Everything we write is Coax. |
| Compressing textures on the GPU at load | Our packs are compressed and range-loaded at build time, which suits a browser download better. |
| `// EXPERIMENT` code on the main branch | `grass_populate.hlsl:771-792` disables a shipped path without saying so. Our tests and goldens exist to prevent this. |

## Suggested order for Everglade II

1. Lessons 1, 3 and 4 in one change, all in `plant.coax`, `field.coax`,
   `apps/openagents-web/everglade2.coax` and `gpu/everglade2.coax`, with `#test` blocks and
   before-and-after captures at the lesson 6 viewpoints.
2. Lesson 5 (cascades), then lesson 2 (coverage LOD and dithered handover).
3. Lessons 7 and 9 for looks. Lessons 10 to 12 when grass reach or density grows past the current
   budget.
