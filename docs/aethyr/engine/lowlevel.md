**My assessment: an AAA-looking MMORPG that renders locally in a browser is now a credible engineering target.** The strongest approach combines WebGPU, GPU-driven rendering, streamed assets, carefully budgeted lighting and animation, and temporal reconstruction. Agents can now help build and optimize that pipeline using actual rendered output and GPU measurements.

The biggest opportunity is **automatically turning rich, editable worlds into representations that fit each player’s device**. That matters especially when agents can generate content faster than developers can manually optimize it.

I researched developments through **October 23, 2026**, focusing on local browser rendering. Performance figures below are the authors’ or vendors’ measurements; I have not independently benchmarked these implementations.

## 1. What has changed recently

These are the developments I would pay closest attention to:

| Development | Why it matters | Current status |
|---|---|---|
| **Unity 6.6 supports WebGPU as a fully supported backend** | Its browser builds can use GPU occlusion culling, GPU Resident Drawing, compute skinning, temporal upscaling, Adaptive Probe Volumes, and VFX Graph. | Announced August 2026; WebGPU remains an explicit opt-in. |
| **Bindless textures have reached public WebGPU experimentation** | Makes large material collections and more flexible GPU-driven shading substantially easier to organize. | October 6, 2026 experiment in Chrome; requires a flag and is not a portable production baseline. |
| **Large Gaussian-splat environments now have practical browser streaming systems** | Photorealistic environments can load progressively and adapt detail to device budgets. | Available implementations include PlayCanvas/SuperSplat and Spark 2.0. |
| **Agents can inspect WebGPU frame captures directly** | Enables an optimization loop grounded in GPU commands, shaders, resources, validation errors, and pass timings. | WebGPU Inspector includes a Claude Code/MCP integration. |
| **Multiplayer generative world models have advanced** | Multiple simultaneous viewpoints and shared state are now serious research topics. | Promising 2026 papers, with substantial compute requirements and unresolved correctness questions. |

Sources: Unity’s official announcement, the WebGPU bindless experiment, the renderer releases, and the Inspector’s repository. [Unity Discussions](https://discussions.unity.com/t/webgpu-out-of-experimental-in-unity-6-6/1734694?utm_source=chatgpt.com)

### Browser support is broad enough to take seriously

Safari 26 enables WebGPU on macOS, iOS/iPadOS, and visionOS. Chromium supports major desktop platforms and substantial Android coverage. Firefox supports Windows and Apple Silicon Macs, while some Linux, Android, and other configurations still have gaps. **Check the actual GPU adapter and required features at runtime.** Browser-family support does not guarantee that every device supports your rendering path. [GitHub](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status?utm_source=chatgpt.com)

Also keep four categories distinct:

- Core, broadly available WebGPU.
- Optional WebGPU features.
- Browser experiments behind flags.
- Features available through native Vulkan, Metal, or DirectX.

Hardware ray tracing and mesh shaders remain outside the portable WebGPU baseline; their GPUWeb feature requests are still open. Compute shaders nevertheless allow software implementations of many related algorithms. [GitHub](https://github.com/gpuweb/gpuweb/issues/535?utm_source=chatgpt.com)

---

## 2. The rendering techniques I would prioritize

### A. GPU-driven rendering: the foundation for dense worlds

**Topics:** GPU culling, indirect drawing, instancing, meshlets, hierarchical depth buffers, visibility buffers, material batching.

The central idea is to let the GPU decide which objects and pieces of objects deserve rendering. The CPU supplies scene data and updates; compute passes determine visibility and prepare drawing work.

For an MMORPG, this becomes valuable when a city contains thousands of buildings, props, decorations, characters, and effects. CPU work associated with visiting and submitting every individual object can become expensive even when much of the world is hidden.

A useful progression is:

1. Batch repeated geometry and materials.
2. Cull whole instances outside the camera.
3. Use a depth hierarchy to reject obscured objects.
4. Split large meshes into small clusters—**meshlets**—for finer visibility and detail selection.
5. Generate drawing work without repeatedly reading visibility results back to the CPU.

The foundational [GPU-Driven Rendering Pipelines](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf) presentation remains highly relevant. Its principles explain much of the architecture behind newer virtual-geometry systems. [advances.realtimerendering.com](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf?utm_source=chatgpt.com)

**Visibility buffers** are another worthwhile branch: render compact identifiers for visible triangles, then resolve materials and shading afterward. This can reduce some intermediate-buffer costs, but requires careful handling of material access and texture derivatives. October’s bindless-texture experiment is particularly relevant to this direction because shaders can select textures from a resource table. [Microsoft Learn](https://learn.microsoft.com/en-us/samples/microsoft/xbox-gdk-samples/visibilitybuffer/?utm_source=chatgpt.com)

**My priority:** establish efficient batching, visibility, and submission before attempting a complete Nanite-style renderer.

### B. Virtual geometry: useful, with important limits

**Topics:** hierarchical cluster LOD, screen-space error, geometry streaming, microtriangle rasterization, mesh simplification.

Nanite’s broad architectural lesson is that **the amount of geometry processed should track visible detail**. An asset can contain far more detail than a particular frame needs.

There is an inspectable browser implementation: [Scthe’s Nanite WebGPU](https://github.com/Scthe/nanite-webgpu). It implements cluster hierarchies, instance and meshlet culling, hardware/software rasterization, and impostors.

Its caveats are educational:

- Its enormous triangle headlines describe source scenes, including repeated instances.
- Its software-rasterization path makes precision and shading compromises because WebGPU lacks the 64-bit atomics used by the demonstrated native approach.
- It is a research implementation, with limitations in occlusion handling and materials. [GitHub](https://github.com/Scthe/nanite-webgpu?utm_source=chatgpt.com)

For implementation, study [meshoptimizer](https://meshoptimizer.org/). It provides simplification, meshlet construction, compression, and supporting algorithms for hierarchical LOD pipelines. Its documentation also makes clear that preprocessing is only part of a complete renderer. [meshoptimizer.org](https://meshoptimizer.org/v1.html?utm_source=chatgpt.com)

For an MMO, I would initially apply virtual geometry to **dense static architecture, rocks, and large environmental assets**. Characters, foliage, transparency, shadows, and material evaluation require their own budgets.

### C. Stream multiple representations of the world

**Topics:** HLOD, spatial chunking, texture mip streaming, predictive loading, content-addressed assets, device-adaptive representations.

This is probably the most consequential architectural topic for your use case.

A browser world needs to balance several independent costs:

- Bytes transferred before the player can move.
- CPU memory while decoding and constructing assets.
- GPU memory after upload.
- Per-frame rendering work.
- Simulation and networking work associated with those objects.

**Roblox’s SLIM is an excellent reference**, even though Roblox itself is not evidence of browser deployment. SLIM builds simplified composite representations in the cloud and selects among them at runtime. Its example car goes from 112 meshes and 24 textures to a representation using one mesh and four textures. Its resource controller adjusts streaming according to CPU, GPU, memory, and bandwidth conditions. [Roblox](https://about.roblox.com/newsroom/2025/12/introducing-roblox-slim-scalable-lightweight-interactive-models?utm_source=chatgpt.com)

The architectural implication is powerful:

> Preserve a rich world description for editing and gameplay, then compile different visual representations for different distances and devices.

A nearby market stall may retain individual objects and interactions. A distant market district may become a composite mesh with simplified materials. Both can refer back to the same world records.

For agent-created content, this gives you a scalable acceptance process: generated objects enter the world through an asset compiler that produces appropriate representations.

### D. Lighting: combine reliable techniques with selected research

**Topics:** PBR, clustered lighting, shadow caching, light probes, lightmaps, screen-space GI, radiance caches, stochastic light sampling.

For an initial browser MMO renderer, I would investigate this combination:

| Lighting layer | Initial approach |
|---|---|
| Sun and major directional lighting | Dynamic directional light with bounded shadow coverage |
| Static architecture’s indirect lighting | Baked lightmaps or probe volumes |
| Moving characters | Probe-based indirect lighting plus dynamic direct lights |
| Local contact and depth | Ambient occlusion and contact-shadow techniques |
| Reflections | Environment probes, with selective screen-space reflections |
| Spell lights and dense local lighting | Clustered or tiled light assignment, with explicit budgets |
| Atmosphere | Scalable fog, sky, and selected volumetric effects |

These building blocks are available in current browser engines; Three.js also has a WebGPU screen-space GI example and implementation. Screen-space effects should be evaluated during camera motion because their information is limited to what the current view contains. [PlayCanvas Developer Site](https://developer.playcanvas.com/user-manual/graphics/?utm_source=chatgpt.com)

Three research directions deserve particular attention:

**1. Stochastic Tile-Based Lighting in HypeHype — SIGGRAPH 2025**

This targets unpredictable user-generated scenes across devices, including budget phones. It uses reservoir sampling and shared tile-level work to handle many lights while controlling bandwidth and computation.

That is directly relevant to an MMO where players can concentrate torches, spells, and glowing effects in one area. The broader lesson is to design lighting around bounded work and coherent memory access. [advances.realtimerendering.com](https://advances.realtimerendering.com/s2025/content/s2025_stb_lighting_v1.1_notes.pdf?utm_source=chatgpt.com)

**2. Split Radiance Cascades — July 2026, revised October 11**

This develops radiance cascades into a sparse, world-space 3D diffuse-GI approach. It addresses the storage cost that restricted many earlier implementations to 2D or screen space.

It is a strong research candidate. I would treat a production browser implementation as a separate project requiring measurement, particularly for ray traversal, memory consumption, and mobile performance. [arxiv.org](https://arxiv.org/abs/2607.20384?utm_source=chatgpt.com)

**3. Neural Dynamic GI — CVPR 2026**

This compresses precomputed lighting across changing illumination conditions using feature maps, lightweight neural networks, and virtual texturing.

**My inference:** this could be especially useful for an MMO city with static architecture and a changing sun. Baking multiple lighting states offline and decoding a compact representation may offer a better practical tradeoff than computing all indirect lighting dynamically. Arbitrary destruction would require additional treatment. [arxiv.org](https://arxiv.org/abs/2604.12625?utm_source=chatgpt.com)

### E. Temporal reconstruction: a major performance tool

**Topics:** TAA, temporal upscaling, motion vectors, disocclusion, reactive masks, dynamic resolution.

Rendering at 1280×720 and reconstructing to 1920×1080 begins with approximately **44% as many rendered pixels**. That can free substantial GPU time when shading and post-processing dominate. The upscaler adds its own cost, and CPU-bound scenes receive much less benefit.

There is a practical WebGPU project to study: [pmndrs/upscaler](https://github.com/pmndrs/upscaler). Its spatial shaders derive from AMD’s FSR work; its temporal path follows FSR 2/3 architecture with its own implementation and adaptations. It integrates with Three.js and exposes interactive examples. [GitHub](https://github.com/pmndrs/upscaler?utm_source=chatgpt.com)

An MMO makes temporal reconstruction demanding:

- Characters move independently of the camera.
- Grass and hair contain thin moving detail.
- Spell trails and particles are translucent.
- Teleports and rapid turns expose previously hidden surfaces.
- Nameplates and interface text need to stay readable.

I would implement motion vectors and history management early, render the interface at output resolution, and judge quality using moving sequences.

**Temporal upscaling, frame generation, and generative visual enhancement solve different problems.** Keep their performance and quality measurements separate.

### F. Crowd rendering and animation budgets

**Topics:** instanced skinning, bone textures, animation LOD, vertex-animation textures, significance scheduling, material consolidation.

A beautiful empty landscape tells you relatively little about capital-city performance.

For characters, detail has several independent dimensions:

| Dimension | What can scale |
|---|---|
| Geometry | Triangle count and accessory detail |
| Skeleton | Bone count and deformation complexity |
| Animation | Update frequency, blend complexity, procedural corrections |
| Materials | Material count, texture resolution, shader complexity |
| Shadows | Participation, resolution, update frequency |
| Simulation | Local prediction, collision detail, AI update frequency |
| Networking | Relevance and update frequency |

Instanced skinning and baked animation data are established ways to reduce crowd-rendering overhead. Epic’s Animation Budget Allocator is also a useful reference for allocating animation work according to significance and a fixed time budget. [NVIDIA Developer](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-2-animated-crowd-rendering?utm_source=chatgpt.com)

My implementation preference would be:

- High-quality animation and shading for the player and nearby important characters.
- Simplified rigs and less frequent animation evaluation farther away.
- Aggressive consolidation of accessories and materials.
- Separate budgets for character shadows and combat effects.

An army of identical animated instances is a useful benchmark, but customized player avatars introduce much more variation.

### G. Gaussian splatting: a valuable environmental layer

**Topics:** 3DGS, hierarchical splat LOD, GPU sorting, compression, streaming, relighting, hybrid mesh/splat scenes.

The most important recent advances concern **how splats are stored, streamed, sorted, and budgeted**.

PlayCanvas’s June 2026 renderer moves splat culling, projection, and radix sorting onto the GPU. Its streamed format supplies progressively refined chunks. The published museum example contains 24 million Gaussians at its highest detail level, while clients load and render a device-appropriate subset. [PlayCanvas Blog](https://blog.playcanvas.com/new-in-supersplat-webgpu-and-streaming-bring-huge-performance-wins/?utm_source=chatgpt.com)

World Labs’ [Spark 2.0](https://www.worldlabs.ai/blog/spark-2.0) also provides hierarchical streaming and LOD. A useful distinction: Spark’s documented renderer uses **WebGL2**, demonstrating that representation and streaming improvements can matter as much as changing graphics APIs. [World Labs](https://www.worldlabs.ai/blog/spark-2.0?utm_source=chatgpt.com)

For an MMORPG, my suggested division is:

| Content | Representation I would investigate first |
|---|---|
| Scanned scenery, vistas, relatively static interiors | Splats or a hybrid |
| Avatars, equipment, articulated creatures | Meshes |
| Doors, destructible objects, interactable props | Meshes with explicit gameplay state |
| Collision and navigation | Dedicated simplified geometry |
| Generated environments | Evaluate both mesh and splat outputs |

Dynamic and relightable splats are advancing. The practical question is whether a particular implementation supports your required lighting, animation, collision, editing, and memory behavior.

The browser-specific paper to read is **WebSplatter**. Its authors report 1.2–4.5× speedups against their selected web viewers through a WebGPU-adapted sorting and rendering pipeline. Those are viewer results, rather than whole-game performance measurements. [arxiv.org](https://arxiv.org/abs/2602.03207?utm_source=chatgpt.com)

### H. Texture compression and neural materials

**Topics:** KTX2, Basis Universal, BC/ASTC/ETC formats, virtual texturing, neural texture compression.

For a practical baseline, study **KTX2/Basis Universal**, which distributes compressed texture data and transcodes it into formats the device can use efficiently. This addresses both delivery size and GPU memory. [GitHub](https://github.com/KhronosGroup/3D-Formats-Guidelines/blob/main/KTXDeveloperGuide.md?utm_source=chatgpt.com)

The more experimental direction is **neural texture compression**: jointly encode a material’s maps and reconstruct their values through a small network. NVIDIA’s *Random-Access Neural Compression of Material Textures* is the foundational paper. There is now a browser experiment, [three-ntc](https://github.com/bhouston/three-ntc), using GPU decoding through Three.js TSL. [research.nvidia.com](https://research.nvidia.com/labs/rtr/neural_texture_compression/?utm_source=chatgpt.com)

My evaluation would compare three costs: download size, resident memory, and additional shader computation. A smaller asset can still produce a slower frame. For mobile, decoding once during loading may sometimes be preferable to repeated decoding during shading.

---

## 3. What agents make newly practical

The most useful change is that agents can operate a **generate → execute → inspect → measure → revise** loop with increasingly specialized graphics tools.

### Direct inspection of browser rendering

[WebGPU Inspector’s agent integration](https://github.com/brendan-duncan/webgpu_inspector/blob/main/claude-plugin/README.md) is one of the most immediately relevant projects I found.

Its documented tools include:

- Capturing frames and measuring GPU pass times.
- Reading WGSL shaders and resolved draw state.
- Inspecting buffers, textures, and resource descriptors.
- Reporting validation errors.
- Examining render-graph dependencies and suspected bottlenecks.

That gives an agent much better evidence for an optimization than source code and a screenshot alone. The tool still needs to be used with a controlled benchmark and image-quality checks. [GitHub](https://github.com/brendan-duncan/webgpu_inspector/blob/main/claude-plugin/README.md?utm_source=chatgpt.com)

There is complementary evidence from Apple’s **WWDC26 game-porting demonstration**: an agent uses GPU capture/debugging tools while bringing a DirectX engine to Metal, investigating rendering errors and fixing them. It is a native-platform demonstration, but the workflow transfers naturally to an observable WebGPU engine. [Apple Developer](https://developer.apple.com/videos/play/wwdc2026/357/?utm_source=chatgpt.com)

### World and asset construction is becoming more structured

| Work | What it contributes | Practical interpretation |
|---|---|---|
| **Code2Games — October 2026** | Shared scene/gameplay representation, Blender-world adaptation to UE5, compilation/runtime/play-test feedback | Study how it preserves correspondence between visual objects and gameplay. Its benchmark contains ten fixed prompts. |
| **WorldClaw — August 2026** | Agents plan regions, terrain, assets, materials, and spatial relationships, then refine rendered results | Useful architecture for generating large editable environments. |
| **TRELLIS.2 — December 2025** | Image-to-3D generation with geometry and PBR material attributes | A building block for offline prop creation; subsequent optimization remains necessary. |
| **“Create Interactive 3D Assets in Seconds!” — SIGGRAPH 2026** | Live image-to-textured, rigged, animated asset pipeline | Evidence that generation stages can be integrated into an interactive creation workflow. |

These are documented in the respective papers, project pages, and official conference program. [arxiv.org](https://arxiv.org/abs/2610.05033?utm_source=chatgpt.com)

The next step I would build around them is an **asset acceptance pipeline**. Every generated asset should produce usable outputs: render mesh, materials, LODs, collision proxy, animation compatibility information, and measured resource costs.

### Automated performance search

Agents can also explore shader and kernel alternatives systematically. Cursor and NVIDIA reported a **1.38× geometric-mean speedup across 235 CUDA-kernel tasks** using a multi-agent process. That supports the value of measured search; it does not imply a 38% improvement in browser-game FPS. [Cursor](https://cursor.com/blog/multi-agent-kernels?utm_source=chatgpt.com)

For graphics, the optimization objective should combine:

- Performance on several representative devices.
- Visual error across multiple scenes and camera paths.
- Memory and download costs.
- Correctness under difficult cases.
- Stability over sustained play.

An agent optimizing one screenshot on one GPU can easily choose an unsuitable compromise.

### Build, GPU-test, deploy agents

Wonder Interactive’s **SimplyStream/Strata** describes a pipeline that compiles Unreal projects to WebAssembly/WebGPU, launches them in Chrome on cloud GPUs, examines captured output, and deploys passing builds.

Its claims include sophisticated Unreal features and large-world streaming. I would evaluate it as an emerging third-party route; the graphics claims remain vendor-reported in this research. The documented build/test/deploy loop itself is directly relevant to an agent-assisted production system. [simplystream.com](https://simplystream.com/?utm_source=chatgpt.com)

---

## 4. Neural rendering and generative world models: the frontier to watch

There are several distinct research directions here.

### Neural enhancement of an ordinary game renderer

NVIDIA’s **DLSS 5** adds a neural stage that modifies lighting and material appearance, with developer controls and masking. Its October 2026 update reports reduced model overhead. This is part of the current native AAA frontier, alongside RTX Mega Geometry 2.0 and neural texture/shader work. [NVIDIA Technical Blog](https://developer.nvidia.com/blog/whats-new-for-game-developers-dlss-5-with-3d-guided-neural-rendering-nvidia-ace-updates-and-new-rtx-kit-capabilities/?utm_source=chatgpt.com)

The debate is substantive: **how much visual interpretation should the model perform, and how does the developer preserve intentional appearance?** For an MMO, identity, equipment detail, silhouettes, and combat readability are particularly sensitive.

These native SDK capabilities are not automatically available through portable WebGPU. Treat them as research references and possible future directions.

### Generated explicit 3D worlds

World Labs’ public **World API** generates explorable 3D environments from text, images, panoramas, and video. This is directly relevant to producing content that a browser renderer can consume.

Its newer **Atlas** model, announced September 2026, adds further reconstruction and generation capabilities, but the announcement describes early access and future product integration. Do not assume every Atlas capability is already in the generally available API. [World Labs](https://www.worldlabs.ai/blog/announcing-the-world-api?utm_source=chatgpt.com)

### Models that generate multiplayer game views

This area has moved quickly enough that a blanket dismissal would be outdated.

- **MIRA, July 2026:** generates four-player Rocket League matches at a reported 20 FPS using a single NVIDIA B200.
- **WorldCast, October 8:** distributes view generation across clients that exchange player and scene state.
- **PAWA, October 14:** studies panoramic representations for multiplayer worlds, including events outside a player’s current view. [arxiv.org](https://arxiv.org/abs/2607.05352?utm_source=chatgpt.com)

WorldCast’s detailed evaluation is especially informative. It uses **one H200 per player**, around **27 GiB per client**, and reports timings excluding synchronization waits. Its hour-long tests restart position estimates and clear scene state at round boundaries. These are substantial research advances, with deployment conditions far removed from local browser rendering on ordinary hardware. [arxiv.org](https://arxiv.org/html/2610.12412v1?utm_source=chatgpt.com)

**My interpretation:** persistent multiplayer generation deserves a research track. For a product built now, explicit world state and ordinary rendering give you much clearer control over inventory, collisions, combat rules, persistence, and performance.

---

## 5. Demos and implementations worth opening

These are useful because they expose particular capabilities you can inspect and compare.

| Demo/project | What to examine |
|---|---|
| [Unity Fantasy Kingdom and Spaceship VFX](https://discussions.unity.com/t/webgpu-out-of-experimental-in-unity-6-6/1734694) | A mainstream engine’s browser rendering, temporal upscaling, and compute effects |
| [Nanite WebGPU](https://scthe.github.io/nanite-webgpu/) | Cluster LOD, culling, impostors, and software-rasterization tradeoffs |
| [SuperSplat Bavarian Railway Museum](https://superspl.at/s?id=71a8c8aa) | Progressive loading and refinement of a large scanned environment |
| [Spark 2.0 examples](https://www.worldlabs.ai/blog/spark-2.0) | Streamed splat LOD and web integration |
| [pmndrs upscaler examples](https://pmndrs.github.io/upscaler/) | Motion, thin detail, transparency, reactive masks, and reconstruction quality |
| [Three.js WebGPU SSGI](https://threejs.org/examples/webgpu_postprocessing_ssgi.html) | What screen-space indirect lighting contributes and where it breaks down |
| [three-ntc](https://three-ntc.ben3d.ca/) | Neural material decoding in a browser |
| [Code2Games](https://aigeeksgroup.github.io/Code2Games/) | Agent-generated worlds linked to executable gameplay |

The linked project documentation establishes the intended capabilities; these should be treated as evaluation starting points rather than interchangeable whole-game benchmarks. [Unity Discussions](https://discussions.unity.com/t/webgpu-out-of-experimental-in-unity-6-6/1734694?utm_source=chatgpt.com)

---

## 6. Papers and talks I would put on your reading list

### Highest relevance to the browser MMORPG architecture

| Reading | Date | Why read it |
|---|---|---|
| [GPU-Driven Rendering Pipelines](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf) | 2015 foundation | Understand GPU visibility and submission architecture. |
| [SLIM: Scaling User-Generated 3D Worlds on Roblox](https://advances.realtimerendering.com/s2026/index.html) | SIGGRAPH 2026 | Automatically compile heterogeneous worlds into device-adaptive representations. |
| [Stochastic Tile-Based Lighting in HypeHype](https://advances.realtimerendering.com/s2025/content/s2025_stb_lighting_v1.1_notes.pdf) | SIGGRAPH 2025 | Handle unpredictable light counts within mobile constraints. |
| [WebSplatter](https://arxiv.org/abs/2602.03207) | February 2026 | Adapt sorting and splat rendering to browser GPU constraints. |
| [Neural Dynamic GI](https://arxiv.org/abs/2604.12625) | April 2026 | Compress precomputed lighting across changing conditions. |
| [Split Radiance Cascades](https://arxiv.org/abs/2607.20384) | July–October 2026 | Sparse world-space diffuse GI. |
| [Six Ways to Draw Vangers with WebGPU](https://arxiv.org/abs/2608.17390) | August 2026 | Compare editable-terrain representations under a shared implementation and measurement framework. |
| [Random-Access Neural Compression of Material Textures](https://research.nvidia.com/labs/rtr/neural_texture_compression/) | SIGGRAPH 2023 foundation | Understand neural materials’ storage/compute tradeoff. |

The terrain paper is particularly valuable methodologically: representations that looked similar from an overhead camera behaved differently at eye-level horizons, and the fastest tested representation carried meaningful memory costs. [arxiv.org](https://arxiv.org/abs/2608.17390?utm_source=chatgpt.com)

### Additional directions worth tracking

The [SIGGRAPH 2026 Advances in Real-Time Rendering course](https://advances.realtimerendering.com/s2026/index.html) also includes:

- **ORCA:** a radiance cache designed without temporal history.
- **Variable Rate Ray Tracing:** allocating a bounded ray budget where it matters.
- **Smolder:** integrated volumetric effects.
- **Adaptive Tessellation and Subdivision:** compute-based, screen-adaptive geometry refinement.

These supply ideas that can transfer into browser renderers even when the original implementation targets native hardware. [advances.realtimerendering.com](https://advances.realtimerendering.com/s2026/index.html?utm_source=chatgpt.com)

For neural/browser integration, add [Visionary](https://arxiv.org/abs/2512.08478), which combines WebGPU splat rendering with per-frame neural processing. It is useful for studying dynamic neural representations, rather than assuming every learned representation must be a static scan. [arxiv.org](https://arxiv.org/abs/2512.08478?utm_source=chatgpt.com)

For agents and world models, prioritize **Code2Games, WorldClaw, WorldCast, and MIRA**, discussed above.

---

## 7. The debates that should shape your decisions

### “WebGPU will make the existing renderer faster”

Sometimes. The larger gains often require reorganizing the renderer around compute, batching, and reduced CPU work.

A Unity WebGPU engineer explicitly reports cases where equivalent workloads favor either API. Bind-group management and other implementation details matter. **Benchmark the rendering architecture and workload, not the API name.** [Unity Discussions](https://discussions.unity.com/t/webgpu-out-of-experimental-in-unity-6-6/1734694?utm_source=chatgpt.com)

### “More geometry means better AAA graphics”

Geometry is one contributor. Lighting, materials, animation, atmosphere, composition, and temporal stability also determine perceived quality.

For an MMO, I would judge additions by their visual benefit per millisecond and per downloaded megabyte. A sophisticated geometry system can leave the dominant cost—shadows, transparency, animation, or materials—untouched.

### “Splats should replace meshes”

The useful choice is often per content category. Splats can reproduce environmental appearance efficiently; meshes provide a particularly direct structure for articulated objects, material editing, and gameplay interaction.

A hybrid world also needs to solve transitions and consistency: characters should appear grounded in the scenery, lighting should agree, and collision should match visible surfaces.

### “Agents make custom engine development cheap”

Agents make implementation and experimentation more accessible. They also increase the value of **good measurement and explicit constraints**.

A renderer still needs reproducible scenes, trustworthy performance data, visual comparisons, and target-device testing. Otherwise, agents can produce a large amount of plausible code without establishing that the game is faster or more correct.

### “A browser experience starts instantly”

Code, textures, meshes, animations, and sounds still need to arrive. The engineering target is **short time to meaningful interaction**, followed by progressive refinement.

This makes first-load payload, decoding, shader preparation, cache behavior, and streaming transitions part of the graphics architecture.

---

## 8. Which engine direction looks most sensible?

| Direction | Why I would consider it | What I would validate |
|---|---|---|
| **Unity 6.6 WebGPU** | Strong baseline for quickly evaluating a complete game pipeline with modern rendering features | Loading behavior, memory, target browsers, and crowded gameplay |
| **Babylon.js** | Browser-focused engine with current features including clustered lighting, frame graphs, volumetrics, and animation tools | The particular feature combination’s cost on your device tiers |
| **PlayCanvas** | Browser workflows, editor integration, and particularly strong splat tooling | Its documentation still labels WebGPU Beta; test the chosen backend and fallback |
| **Three.js + custom systems** | Flexible rendering foundation and accessible experiments | You own more streaming, world, animation, and game architecture |
| **Rust/wgpu or Bevy** | Attractive when preserving Rust code and controlling architecture matter | Browser-backend capabilities separately from native features |
| **Unreal through SimplyStream** | Potentially preserves an existing Unreal content workflow | Vendor-specific compatibility, performance, operational dependence, and actual project behavior |

Sources for the current engine capabilities and qualifications: [Web-Based 3D At Its Best](https://babylonjs.com/?utm_source=chatgpt.com)

One particularly important Rust caveat: **Bevy’s current meshlet renderer requires 64-bit texture atomics and documents Vulkan/Metal support. Its native virtual-geometry feature is not a browser capability you can assume will carry over.** [Rust](https://docs.rs/bevy/latest/bevy/pbr/experimental/meshlet/struct.MeshletPlugin.html?utm_source=chatgpt.com)

For the surrounding runtime, investigate worker-based decoding/simulation, careful memory ownership, and minimal synchronization between CPU and GPU. Shared-memory Wasm threading also affects hosting: Emscripten documents the required cross-origin isolation headers. WebTransport provides useful stream/datagram primitives, while replication relevance and authoritative simulation remain application responsibilities. [Emscripten 6.0.14-git (dev) documentation](https://emscripten.org/docs/porting/pthreads.html?utm_source=chatgpt.com)

---

## 9. What I would investigate first for an agent-built MMORPG

I would organize the work around a **representative, demanding playable district**.

It should contain:

- A visually polished street and an interior.
- Foliage and overlapping transparent effects.
- A crowd of independently animated, differently equipped characters.
- Several concurrent combat effects.
- Movement across an asset-streaming boundary.
- A fast camera turn and a teleport.
- A long enough session to reveal memory growth and thermal throttling.

Then establish three explicit device tiers: desktop with a discrete GPU, integrated-GPU laptop, and mobile. Assign each a resolution, frame-rate target, memory budget, and loading target. A 60 FPS target gives roughly **16.7 milliseconds per frame**; the CPU and GPU measurements should each be examined with their actual overlap and scheduling.

Measure:

| Measurement | What it tells you |
|---|---|
| Time to first controllable frame | Whether browser delivery feels immediate |
| Initial and streamed bytes | Whether content density fits network conditions |
| CPU and GPU frame-time distributions | Whether occasional stalls undermine the average |
| Peak CPU/GPU memory | Whether loading transitions exceed the steady-state budget |
| Per-pass GPU timings | Where optimization will actually help |
| Moving image comparisons | Whether reconstruction and LOD remain visually stable |
| Crowd/effect scaling curves | Where the MMO workload stops fitting |
| Sustained-session behavior | Whether performance survives extended play |

My implementation order would be:

1. **Establish high-quality materials, lighting, and animation in one district.**
2. **Build streaming, compression, and representation LOD into the asset pipeline.**
3. **Add GPU-driven visibility and effective crowd budgeting.**
4. **Integrate temporal reconstruction and scalable effects.**
5. **Give agents frame captures, performance reports, and reproducible tests.**
6. **Evaluate virtual geometry, neural materials, and splat layers where measurements show a clear benefit.**

The product opportunity I find most compelling is an **agent-operated world compiler**: creators author rich places and characters, and the system repeatedly turns them into efficient, validated, versioned browser assets. A good renderer makes those assets look excellent; the compiler and feedback loop make an expanding multiplayer world practical to maintain.
