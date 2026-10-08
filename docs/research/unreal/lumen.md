

# Unreal Engine Lumen: architecture, algorithms, hardware support, and practical limitations

**Lumen is Unreal Engine’s real-time global illumination and reflections system. Its central architectural idea is to trace relatively few rays, reuse lighting aggressively, and reconstruct a detailed image from several simplified representations of the scene.** It is not simply “Unreal’s RTX renderer,” and enabling hardware ray tracing does not turn it into a conventional path tracer. Epic provides a separate Path Tracer for that purpose. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-global-illumination-and-reflections-in-unreal-engine)

This report uses **UE 5.8 as the current baseline**, released June 23, 2026. That matters: older explanations miss changes such as **Lumen Lite**, and some older hardware-support statements no longer match Epic’s current platform documentation. [Unreal Engine](https://www.unrealengine.com/news/unreal-engine-5-8-is-now-available)

---

## 1. What Lumen actually computes

Consider sunlight entering a room and hitting a red wall. Direct lighting explains the illuminated wall. **Global illumination explains the red-tinted light that subsequently reaches the floor, ceiling, and other objects.** It also accounts for objects blocking that indirect illumination. Mathematically, the challenge is recursive: the light arriving at one surface depends on light leaving other surfaces, which depends on still other surfaces. [PBR Book](https://www.pbr-book.org/4ed/Light_Transport_I_Surface_Reflection/The_Light_Transport_Equation)

Lumen provides diffuse indirect illumination, indirect specular reflections, and shadowed skylighting. Its material integrations include clear-coat reflections, two-sided foliage lighting, and Single Layer Water. Translucent surfaces and volumetric fog receive lower-quality indirect lighting; high-quality translucent reflections apply to the **frontmost** translucent layer rather than providing unrestricted multilayer glass transport. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-global-illumination-and-reflections-in-unreal-engine)

### Where Lumen fits in Unreal’s renderer

Several UE5 technologies are commonly conflated:

| System | Main responsibility |
|---|---|
| **Lumen** | Indirect lighting and reflections. |
| **Nanite** | Rendering and streaming highly detailed geometry through virtualized geometry clusters. |
| **Virtual Shadow Maps** | High-resolution shadowing from direct lights. |
| **MegaLights** | A stochastic direct-lighting system for many dynamic, shadowed lights. |
| **Temporal Super Resolution, or TSR** | Reconstructing a higher-resolution image from lower-resolution rendering and temporal information. |
| **Path Tracer** | Progressive, higher-fidelity light transport for reference images and final rendering. |

These are complementary systems, not different names for the same feature. In particular, MegaLights handles **direct lighting**, while Lumen handles the indirect illumination and reflections that follow. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/nanite-virtualized-geometry-in-unreal-engine)

---

## 2. The architectural overview

The most useful way to understand Lumen is to separate three questions:

**What did the camera see? Where does a secondary ray intersect geometry? What lighting should be returned at that intersection?**

Lumen does not use the same data structure to answer all three. Its development history explicitly describes the separation of intersection geometry from the cached surface data used for shading. Earlier experiments traced “cards” as geometry; that should not be confused with the role of cards in the shipping Surface Cache architecture. [Krzysztof Narkowicz](https://knarkowicz.wordpress.com/2022/08/18/journey-to-lumen/)

A conceptual—not literal execution-order—pipeline is:

```text
Scene geometry and materials
        │
        ├── Camera rendering → depth, normals, materials, visible scene
        │
        ├── Intersection representation
        │       ├── Software: signed distance fields
        │       └── Hardware: triangle acceleration structures
        │
        └── Surface captures → cached material and lighting data

Indirect-light / reflection query
        │
        ├── Try screen-space tracing
        ├── Resolve remaining rays against the intersection representation
        └── Obtain lighting from caches, or evaluate supported hit lighting

Gather, filter, and integrate lighting
        │
        └── Temporal reconstruction and final image composition
```

The important distinction is that **an accurate geometric hit can still return approximate cached lighting**. Better intersection accuracy and better shading accuracy are separate upgrades. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/hardware-ray-tracing-in-unreal-engine)

At the implementation level, this is a collection of rendering passes and persistent resources, not one giant post-processing shader. Unreal’s Render Dependency Graph manages dependencies, resource lifetimes, barriers, and opportunities for parallel execution. When investigating engine code or GPU captures, think in terms of cooperating passes with shared caches rather than a single “Lumen algorithm.” [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/render-dependency-graph-in-unreal-engine)

---

## 3. How Lumen traces rays

### 3.1 Screen-space tracing comes first

Lumen initially tests rays against the camera-visible scene. This captures detail that may be missing from its other representations. Rays that cannot be resolved this way fall back to software or hardware scene tracing. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-technical-details-in-unreal-engine)

The limitation follows directly from the representation: a camera image does not describe the back of an object or an off-screen room. Screen-space reflections alone therefore cannot provide complete reflected visibility. Lumen’s fallback tracing is what makes the system more than an enhanced screen-space effect. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/screen-space-reflections-in-unreal-engine)

An important diagnostic implication is that a convincing stationary view can conceal a representation problem. Moving the camera may expose it when a previously visible surface must instead be resolved through the fallback scene.

### 3.2 Software ray tracing: signed distance fields

“Software” here describes the tracing method; it does **not** mean that all lighting is being rendered on the CPU.

A **signed distance field**, or SDF, stores the distance to the nearest surface throughout a volume. Values are positive outside an object and negative inside. Unreal generates mesh distance fields offline from mesh geometry and combines them into a camera-centered **Global Distance Field** using nested volumes called clipmaps. Newly exposed or changed regions can be updated without rebuilding the entire field. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/mesh-distance-fields-in-unreal-engine)

The ray-marching technique is often called **sphere tracing**. In an ideal distance field, the next step can be expressed as:

\[
t_{\text{next}} = t + d(\mathbf{o}+t\mathbf{v})
\]

Here, \(\mathbf{o}\) is the ray origin, \(\mathbf{v}\) its direction, and \(d\) the distance to the nearest surface. If the field says the closest surface is two meters away, the ray can skip that empty space instead of advancing by tiny fixed increments.

Real fields are discretized approximations. Resolution affects memory, rounded corners, and whether thin features survive. Two-sided distance-field generation can improve foliage representation, but increases tracing cost. Importantly, generating a new mesh’s distance field is not an arbitrary runtime operation. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/mesh-distance-fields-in-unreal-engine)

**A version-sensitive detail:** older descriptions emphasize tracing individual mesh SDFs nearby and the global field farther away. UE 5.7 deprecated the higher-detail **mesh-SDF tracing path** as an area of continued development, recommending hardware tracing for quality above global-field tracing. This did **not** remove global-field software Lumen. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5-7-release-notes?application_version=5.7)

### 3.3 Hardware ray tracing: triangle acceleration structures

Hardware Lumen traces against triangle geometry organized in a two-level bounding-volume hierarchy:

- **BLAS:** acceleration structures for mesh geometry.
- **TLAS:** the scene’s instances and their transforms.

Static mesh structures can be prepared once, whereas deforming geometry introduces ongoing update work. Skinned characters, hair, procedural meshes, and some terrain operations can therefore make hardware tracing expensive before the lighting rays themselves are even dispatched. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/ray-tracing-performance-guide-in-unreal-engine)

The practical advantage is a richer geometry representation, especially for animated geometry. But “hardware” is not synonymous with “automatically faster.” The cost includes structure construction, scene instances, deformation, traversal, and shading.

For example, a mostly static environment and a crowd of highly detailed animated characters can have very different ray-tracing costs despite producing similarly complex camera images. Shader-driven **World Position Offset** is also not automatically reflected in every ray-tracing representation; enabling its evaluation can require additional geometry updates. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/ray-tracing-performance-guide-in-unreal-engine)

---

## 4. The Surface Cache: Lumen’s central reuse mechanism

After finding an intersection, a renderer still needs to determine what light leaves that surface. Repeatedly executing a complicated material and evaluating its illumination for every incoming ray would waste substantial work.

Lumen therefore maintains cached surface information separately from the geometry used for intersection. Conceptually, the difference is:

> The tracing structure answers **“where did I hit?”**  
> The Surface Cache helps answer **“what light is available there?”**

This separation is one of the key architectural decisions documented by Lumen’s developers. [Krzysztof Narkowicz](https://knarkowicz.wordpress.com/2022/08/18/journey-to-lumen/)

### Cards and coverage

Lumen captures meshes from directions called **Cards**. Static meshes default to a maximum of **12 cards**, adjustable through **Max Lumen Mesh Cards**. In the Surface Cache debug view, **pink indicates missing coverage**; those regions cannot provide correct cached bounced lighting. Current skeletal-mesh support uses six cards around pre-skinned bounds, with limited coverage under substantial deformation. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-technical-details-in-unreal-engine)

The implication for content creation is straightforward: geometry that looks fine from the gameplay camera may still be poorly represented for secondary lighting. Increasing samples does not repair missing cached surface coverage.

### Cached lighting is not baked lighting

The distinction is *when the illumination is evaluated*.

Offline mesh preprocessing can prepare a representation of an object without fixing that object’s lighting forever. At runtime, cached illumination can be refreshed as lights, geometry, and viewpoints change. Thinking of this as “dynamic lightmaps” is an incomplete analogy: it misses the automatic surface representation, feedback, tracing, and temporal updating that make the system work.

It is better understood as a reusable approximation of the scene’s lighting state.

### Hardware tracing does not eliminate the cache

Hardware Lumen offers two important reflection-lighting choices:

| Lighting mode | What happens after an intersection |
|---|---|
| **Surface Cache** | Reuse cached lighting at the hit. |
| **Hit Lighting for Reflections** | Evaluate material and lighting at the hit, including additional shadow rays. |

Even with Hit Lighting, **diffuse indirect illumination visible in the reflection still uses the Surface Cache**. Thus, “hardware ray tracing enabled” and “all lighting evaluated from scratch” are not equivalent. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/hardware-ray-tracing-in-unreal-engine)

---

## 5. How the final gather makes few rays look like many

Finding intersections efficiently is only half the problem. A surface can receive light from an enormous number of directions.

Lumen’s regular high-quality final gather reduces this cost with **screen-space radiance probes**. Instead of independently sampling a dense hemisphere at every pixel, it gathers directional incoming light at fewer locations, filters that information, and integrates it with full-resolution surface data. Filtering occurs in the directional radiance representation, not merely by blurring the final shaded picture. [Unreal Engine](https://www.unrealengine.com/tech-blog/unreal-engine-5-goes-all-in-on-dynamic-global-illumination-with-lumen?lang=en-US)

### Directional information matters

A probe needs more than one RGB brightness value. Light coming from the left must illuminate a left-facing normal differently from a right-facing normal. Retaining directional information allows the renderer to reuse a lighting estimate while still responding to detailed surface orientation.

Lumen also guides sampling toward directions that carried useful illumination previously. Its **importance sampling** spends more effort where rays are likely to matter. A separate **world-space radiance cache** helps resolve distant illumination, especially difficult configurations such as an interior receiving skylight through a small window. [Unreal Engine](https://www.unrealengine.com/tech-blog/unreal-engine-5-goes-all-in-on-dynamic-global-illumination-with-lumen?lang=en-US)

### A concrete sampling example

The console-variable reference exposes controls for screen-probe spacing and octahedral directional resolution. As an illustration, a probe covering a **16 × 16-pixel** region with **8 × 8 directional samples** has:

\[
\frac{64\text{ directional samples}}{256\text{ pixels}}
=0.25\text{ samples per pixel}
\]

That is **not a measurement of total Lumen rays per pixel**. Additional tracing, cache updates, reflections, and reconstruction work remain. It illustrates why sharing directional samples can be dramatically cheaper than launching 64 independent rays from every pixel. Effective settings also depend on scalability and device profiles. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-console-variables-reference)

### Why rough reflections are cheaper than mirrors

A rough surface integrates light over a broad range of directions; a mirror needs a much more precise directional answer. Lumen can reuse gathered lighting for rough specular response, while smoother surfaces require dedicated reflection work. The performance guide gives a default dedicated-tracing roughness threshold of **0.4**, subject to configuration. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-performance-guide-for-unreal-engine)

---

## 6. “Infinite bounces,” temporal accumulation, and responsiveness

### Infinite diffuse bounces does not mean infinite rays per frame

A useful conceptual model for repeated diffuse transport is:

\[
B_{k+1}=E+KB_k
\]

Here, \(E\) represents injected lighting and \(K\) represents one stage of transport between surfaces. Repeated substitution gives:

\[
E+KE+K^2E+\cdots
\]

This is an explanatory model, **not literal Lumen implementation pseudocode**. It shows how reusing an evolving illumination estimate can represent successive bounces without tracing an infinitely long path for each pixel. The underlying light-transport equation has exactly this recursive character. [PBR Book](https://www.pbr-book.org/4ed/Light_Transport_I_Surface_Reflection/The_Light_Transport_Equation)

Specular recursion is a separate matter. Lumen’s reflection-bounce count is finite and configurable; additional recursive reflection bounces require supported hardware-tracing Hit Lighting. Do not interpret “infinite diffuse bounces” as unlimited mirrors reflecting mirrors. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-global-illumination-and-reflections-in-unreal-engine)

### Why moving images can look worse than still images

Temporal filtering reuses previous results to suppress noise. Its tradeoff is fundamental: longer histories improve stability but respond more slowly; rejecting history more aggressively improves responsiveness but exposes fresh sampling noise. Lumen’s reflection controls explicitly expose this relationship between history accumulation, ghosting, and flicker. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-console-variables-reference)

This suggests three distinct testing cases:

**A static camera** tests converged quality. **Camera movement** tests reconstruction and newly visible surfaces. **Changing lighting or geometry** tests cache responsiveness. Passing the first does not imply passing the others.

### Lumen filtering and TSR are different stages

Lumen reconstructs lighting. **TSR reconstructs the final higher-resolution image.** Both can use temporal information, but they solve different problems.

For example, rendering at 1920 × 1080 and reconstructing to 3840 × 2160 starts with one quarter as many pixels. That creates more room for expensive lighting work, but it does not remove the need for good temporal data, sufficient input detail, or stable motion handling. TSR is therefore part of the overall quality/performance strategy, not a magic repair for inaccurate lighting. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/temporal-super-resolution-in-unreal-engine)

---

## 7. Nanite and large-world integration

### Nanite is complementary, not a requirement that everything be identical

Nanite’s visible geometry and the geometry intersected by hardware rays are not necessarily the same. By default, ray tracing generally uses a **Nanite fallback mesh**. A mismatch can therefore appear between the detailed camera image and a reflection or shadow.

Increasing fallback fidelity can help, at a cost. Epic also documents experimental native Nanite ray tracing through:

```text
r.RayTracing.Nanite.Mode 1
```

That experimental option should not be mistaken for the universally enabled production path. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/nanite-virtualized-geometry-in-unreal-engine)

The engineering implication is important: a high-detail rasterized object does not prove that every secondary ray encounters equally detailed geometry.

### Large worlds use bounded representations

Lumen does not maintain an equally detailed, fully updated representation of an unlimited world. Software Lumen’s scene coverage is normally around **200 meters**, extendable to **800 meters**. Hardware **Far Field** can extend tracing to a default **1 kilometer**, using World Partition HLOD data; HLOD1 must be built. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-technical-details-in-unreal-engine)

Think of this as a working-set problem: nearby and relevant information receives greater detail and update effort. A rapidly moving camera, distant reflection, and small interior may stress different parts of that working set.

---

## 8. Lumen Lite: the significant UE 5.8 addition

**Lumen Lite is a cheaper lighting configuration, not simply another name for software ray tracing.** The tracing backend and the method used to gather/reconstruct illumination are different architectural choices.

UE 5.8’s Lite path uses irradiance fields derived from world-space probes. It interpolates probe illumination with occlusion information, reconstructs lower-resolution results, and reduces other scene costs. Smooth reflections use **screen-space reflections**, while rough specular lighting comes from probe information rather than dedicated Lumen reflection rays. The release notes identify Lite as **Beta**. [Epic Games Developers](https://dev.epicgames.com/documentation/unreal-engine/unreal-engine-5-7-release-notes?lang=en-US\&utm_source=chatgpt.com)

Epic describes it as approximately **twice as fast as Lumen High Quality**, targeting **60 fps on Nintendo Switch 2**, with PC support. That is a lighting-system comparison and target—not a promise that every game doubles its frame rate. [Unreal Engine](https://www.unrealengine.com/news/unreal-engine-5-8-is-now-available)

The tradeoff is meaningful: a scene may retain useful dynamic indirect illumination while losing the off-screen reflection completeness associated with full Lumen reflections.

Also, older advice that **“Medium disables Lumen”** is no longer generally correct: current Medium settings can select the irradiance-field path; Low disables Lumen. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-performance-guide-for-unreal-engine)

---

## 9. Supported platforms and devices

**Compatibility, production support, and acceptable performance are different questions.** A device can expose the required graphics features without delivering the desired scene quality or frame rate.

| Platform or device family | Current position | Important qualification |
|---|---|---|
| **Windows PC, hardware tracing** | Supported on the documented modern GPU families | DX12 and Shader Model 6; Epic lists NVIDIA RTX 2000-series+, AMD RX 6000-series+, and Intel Arc A-series+. |
| **Windows PC, software tracing** | Supported | The technical guide lists GTX 1070-class hardware as a baseline, with current DX12/SM6 requirements. This is not an Epic-quality performance guarantee. |
| **PlayStation 5** | Supported target | Actual quality and frame rate depend on the game’s profile and content. |
| **Xbox Series X / Series S** | Supported targets | Support does not imply identical settings across the two consoles. |
| **Nintendo Switch 2** | Current Lumen Lite target | Epic targets 60 fps with the cheaper path. |
| **Linux / Vulkan** | Hardware ray tracing is documented as experimental | Validate the specific GPU, driver, engine version, and rendering configuration. |
| **macOS, software tracing** | Supported on Apple Silicon M1+ and qualifying Intel/AMD-GPU Macs | Follow the Mac-specific requirements rather than applying the Windows GPU list. |
| **macOS, hardware tracing** | Epic’s current table lists Apple Silicon M2+ as experimental | This is Epic’s compatibility label, not a promise of identical acceleration or performance across chips. |
| **High-end Android** | Experimental | Specific Vulkan-capable profiles; see the renderer caveat below. |
| **iPhone, iPad, Apple TV** | Not supported by the current Lumen mobile documentation | Hardware ray-tracing capability elsewhere in the platform does not establish Lumen support. |
| **Desktop VR / XR** | Experimental, not officially supported for production | PC deferred renderer with DX12; performance is particularly demanding. |
| **Standalone mobile XR** | Not supported | Desktop headset experiments do not establish standalone-headset support. |
| **PS4 / Xbox One** | Not supported | These are outside Lumen’s supported legacy-console targets. |

Sources: Epic’s general hardware requirements, Lumen technical guide, hardware ray-tracing guide, Mac requirements, mobile guide, XR guide, and UE 5.8 announcement. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/hardware-and-software-specifications-for-unreal-engine)

### Android needs special caution

Epic’s detailed Android guide specifies **Vulkan SM5 with the desktop renderer**, including profiles for **Adreno 7xx, Mali G7xx, and Samsung Xclipse 9xx**. These are device-profile families, not guarantees that every associated device supports hardware tracing or runs well. Epic explicitly discourages shipping with this experimental configuration. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/using-lumen-global-illumination-on-mobile-in-unreal-engine)

There is a documentation inconsistency: the general technical page mentions the mobile renderer, while the detailed setup guide requires the desktop renderer. I would treat Android as a **version- and device-specific validation exercise**, not advertise blanket native mobile support. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-technical-details-in-unreal-engine)

For VR, Epic notes that rendering Lumen for both headset views can make target frame rates difficult even on powerful hardware. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/nanite-and-lumen-for-xr-in-unreal-engine)

---

## 10. Performance: what the budgets really mean

Epic’s reference console budgets are approximately:

| Quality level | Lumen GI and reflections budget | Intended console frame-rate target |
|---|---:|---:|
| **High** | **4 ms** | 60 fps |
| **Epic** | **8 ms** | 30 fps |

These refer to **1080p internal rendering**, covering the documented Lumen lighting workload—not the entire frame. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-performance-guide-for-unreal-engine)

A 60 fps frame lasts about **16.67 ms**. Geometry rendering, direct lighting, shadows, animation, effects, upscaling, and other work still need their own budgets.

For an illustrative calculation, suppose a GPU-bound frame costs:

```text
Other work: 12 ms
Lumen:       4 ms
Total:      16 ms → 62.5 fps
```

Halving Lumen to 2 ms produces a 14 ms frame, or roughly **71.4 fps**, not 125 fps. Real asynchronous overlap can make the relationship more complicated.

### Profile the correct work

Epic groups the major Lumen work into **Scene Lighting**, **Screen Probe Gather**, and **Reflections**. Use `Stat GPU` and `ProfileGPU`; hardware scene-update costs also deserve separate inspection. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-performance-guide-for-unreal-engine)

Do not sum overlapping GPU passes as though they all execute serially. Unreal’s render graph supports asynchronous scheduling, and the frame’s critical path is what ultimately limits throughput. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/render-dependency-graph-in-unreal-engine)

Also distinguish lighting-system costs. A scene with many problematic lights may be constrained by direct-light sampling or shadow work rather than indirect illumination. MegaLights itself has a fixed-sampling tradeoff: increasing local lighting complexity can reduce quality at the same sampling budget. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/megalights-in-unreal-engine)

There is no universal “Lumen requires exactly X GB of VRAM” figure. Epic’s general development recommendation is **32 GB system RAM and 8 GB or more graphics memory**; that is a workstation guideline, not a complete runtime memory budget for every Lumen game. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/hardware-and-software-specifications-for-unreal-engine)

---

## 11. Enabling and validating a project

The basic project choices are:

```text
Dynamic Global Illumination Method: Lumen
Reflection Method: Lumen
```

For software tracing, enable **Generate Mesh Distance Fields**. For hardware tracing, enable **Support Hardware Ray Tracing**, its required **Compute Skin Cache**, and **Use Hardware Ray Tracing when available**. Reflection Hit Lighting is a separate quality decision. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-global-illumination-and-reflections-in-unreal-engine)

Lumen is not compatible with Unreal’s **Forward Shading** path. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-technical-details-in-unreal-engine)

When Lumen GI is active, precomputed static lighting contributions are disabled. Static-light mobility is therefore not an interchangeable substitute for dynamic lighting in a Lumen setup. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-global-illumination-and-reflections-in-unreal-engine)

### A practical validation sequence

My recommended order is to test **representation first, sampling second, performance third**.

Start with a deliberately small scene containing an interior, a bright opening, a reflective surface, a moving character, and a switchable light. Compare the normal camera view with the relevant geometry and lighting debug views. Then move the camera and change the lighting.

This isolates several failure categories:

| Observed problem | First question to investigate |
|---|---|
| Reflection changes drastically when the camera turns | Was screen-visible information concealing a fallback-representation problem? |
| Geometry is present but reflected lighting looks wrong | Is the intersection accurate while the cached shading is inadequate? |
| Thin features leak or disappear | Does the tracing representation actually contain those features? |
| A stationary image looks good but movement does not | Is reconstruction relying on history that motion invalidates? |
| Increasing quality barely helps | Is the underlying data missing rather than merely undersampled? |
| Hardware tracing becomes expensive around characters | Is deformation/acceleration-structure maintenance the dominant cost? |

These are diagnostic hypotheses, not one-setting fixes. They follow from the separation between tracing, cached shading, reconstruction, and geometry maintenance described above.

For visual validation, Unreal’s **Path Tracer** is useful as a separate reference renderer. Compare equivalent geometry, materials, lighting, and exposure, while recognizing that Lumen intentionally makes real-time approximations rather than promising the same result at a fraction of the cost. [Epic Games Developers](https://dev.epicgames.com/documentation/en-us/unreal-engine/path-tracer-in-unreal-engine)

---

## 12. Overall assessment

**Lumen’s main achievement is making dynamic indirect lighting practical by reusing information across surfaces, directions, pixels, and frames.** Hardware ray tracing improves what it can intersect; it does not replace the entire caching and reconstruction architecture.

For planning purposes, I would treat it as three related decisions:

**Choose the intersection backend** according to geometry requirements and platform capability. **Choose the lighting and reflection quality** according to what the scene must depict. **Choose the update and reconstruction budget** according to motion, responsiveness, and frame-rate requirements.

The most important engineering lesson is this:

> **When Lumen looks wrong, first determine whether the problem is missing geometry, missing surface information, insufficient samples, or stale history. Those are different problems, and increasing a generic quality setting will not solve all of them.**

