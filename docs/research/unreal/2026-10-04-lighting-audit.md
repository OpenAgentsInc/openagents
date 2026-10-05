# Verse lighting audit against Unreal Engine

**Status:** Research note, 2026-10-04. No candidate here is approved work yet;
each needs a claimed issue before implementation, and every status is
*candidate* until it says otherwise. Studied under the rules in
[AGENTS.md](AGENTS.md).

**Source studied:** Unreal Engine 5.8.3 (`release` at `396c9f05`, tag
`5.8.3-release`), from the clone at `~/work/UnrealEngine`. The reading covered
`Engine/Source/Runtime/Renderer/Private` (including `Lumen/`, `MegaLights/`,
`VirtualShadowMaps/`, `Shadows/`, `PostProcess/`, and `CompositionLighting/`),
`Engine/Shaders/Private` and `Engine/Shaders/Shared`, and the per-platform
`Engine/Config/*/DataDrivenPlatformInfo.ini` files. Paths that start with
`Renderer/` or `Runtime/` are relative to `Engine/Source/Runtime/`; every
other path is relative to `Engine/`. Paths and console variable names are
pointers only. This
note contains no Unreal code; it describes techniques in our own words and
names the public paper or permissive source to build from.

**Compared against:** `crates/verse` and `crates/verse-engine` at
`61053682b1`.

**Related:** the [Lagrange realism audit](2026-09-27-lagrange-realism-audit.md)
already landed most of the camera and material work this note builds on, and
records its own rejections. The [Verse Engine roadmap](../../verse/engine/roadmap.md)
places lighting under VE-3 (rendering) and quality tiers under VE-6.

## Summary

Unreal's headline lighting systems (Lumen, MegaLights, and virtual shadow
maps) are built for desktop-class GPUs with compute shaders, storage buffers,
and often hardware ray tracing. Unreal itself does not run them on its phone
renderer: in `Config/IOS/DataDrivenPlatformInfo.ini` and
`Config/Android/DataDrivenPlatformInfo.ini`, only the desktop-class
`METAL_SM6_IOS` and `VULKAN_SM5_ANDROID` shader platforms set
`bSupportsLumenGI`; the `ES3_1` mobile platforms do not. Verse's floor is
WebGL2 through wgpu, which has no compute shaders and no storage buffers, so
those systems are rejected outright. Their ideas still help, in reduced form.

The reference profile for Verse is Unreal's **mobile forward renderer**
(`Renderer/Private/MobileShadingRenderer.cpp`): one shadowed directional light
with cascades, a handful of local lights, a sky light stored as spherical
harmonics, reflection cubemaps, precomputed indirect light, exponential height
fog, and a tone mapper that can run in tile memory. Verse already matches or
exceeds that profile in materials, bloom, and exposure. Its gaps are all in
the environment around the surfaces:

1. **No sky-derived ambient or reflections.** Everglade's ambient is two
   constants (sky and ground lux), and glossy surfaces reflect a linear
   probe evaluated in the reflection direction, not a sky.
2. **Shadows stop at a fixed box.** Everglade's one 2048² sun map covers a
   box 80 m across. Anything outside it is fully lit.
3. **No ambient occlusion on textured meshes.** The Lagrange bake exists, but
   Everglade's textured props and foliage shade with no occlusion at all.
4. **Fog is a flat-colored distance ramp** with no height falloff and no glow
   toward the Sun.
5. **Two output transforms.** The physical path ends in Khronos PBR Neutral
   after bloom and exposure; the chamber path tone-maps each fragment with an
   ACES curve fit and has no bloom.

The top five recommendations, in order, are a unified output transform, a
sky-derived ambient and reflection environment, exponential height fog, a
baked occlusion and bounce bake for static zones, and camera-following
cascaded sun shadows. All five run in vertex and fragment shaders or on the
CPU, so they work on WebGL2, phones, and desktop alike. The
[phased plan](#phased-plan) at the end gives sizes and owning crates.

An Everglade sky pass inspired by SkyAtmosphere is in progress in a separate
change by another agent. This note treats it as a dependency and does not
propose a second design for it.

## What Verse renders today

Verse has three draw paths. Which one a zone takes decides what lighting it
gets.

| Path | Used by | Files |
| --- | --- | --- |
| Amber legacy | The plaza's lines and faces, the Ruins zone | `crates/verse/src/shader.wgsl`, `render.rs` |
| Physical (`pbr`) | Lagrange 1 (a `Sky`), the plaza ball and Everglade (a `Neon` stage with a `Key` light), Physics Lab | `crates/verse/src/pbr/{gpu.rs,photo.wgsl,post.wgsl,bake.rs,textured.rs}` |
| Chamber (`imported`) | The original summoning-lair scene with skinned characters and torches | `crates/verse-imported/src/imported/{scene.wgsl,lighting.rs,shadow_cache.rs}`, `crates/verse-engine/src/lighting.rs` |

Feature by feature:

| Feature | Physical path | Chamber path | Amber and Ruins |
| --- | --- | --- | --- |
| Direct light | GGX with height-correlated Smith, multiscatter compensation, disc-light widening, anisotropy, clear coat, sheen, thin film. Sun plus Earth (Lagrange) or key plus rim (stage). | GGX with Smith and Schlick, up to 32 point lights with windowed falloff. | CPU Lambert baked into vertex colors (Ruins); display colors (amber). |
| Shadows | One fitted 2048² sun map with 16-tap PCSS from the disc's angle. On GLES there is no blocker search: a fixed 1 m occluder distance sets the radius. | The first four lights get cube maps, 512² per face in a 24-layer array, 3×3 PCF. Static casters are cached; posed bounds cull unneeded faces. | None. |
| Indirect diffuse | Lagrange: a CPU-baked order-one spherical-harmonic (SH) probe grid of station bounce. Stage: a uniform sky-over-ground probe built from two lux constants. | One flat ambient color. | None. |
| Ambient occlusion | Per-vertex AO baked for Lagrange lit triangles. Textured meshes: none. | Material occlusion maps only. | None. |
| Reflections | The probe grid evaluated in the reflection direction, with specular occlusion. No cubemap. | None beyond direct specular. | None. |
| Sky | Lagrange: Sun, Earth, Moon, catalogue stars, and the Milky Way at infinity. Everglade: the clear color (`[0.4, 0.44, 0.22]`). | Clear color. | Clear color. |
| Fog | Linear ramp to the field color (Everglade 40 m to 170 m). | Exponential distance fog to one color. | Quadratic ramp to the field color. |
| Scene target | `Rg11b10Ufloat` or `Rgba16Float`, 4× MSAA. Without a renderable float format, each draw tone-maps itself and post-processing is skipped. | Tone-mapped in the fragment shader. | sRGB. |
| Exposure | EV100 camera; center-weighted log-average auto exposure with a highlight guard; local exposure. | One multiplier. | None. |
| Tone mapping | Khronos PBR Neutral; a hue-preserving shoulder on neon stages; EDR headroom on Apple displays. | An ACES curve fit, per fragment. | None. |
| Bloom and lens | Six-level energy-conserving bloom, ghosts, vignette, grain, chromatic aberration. | None. | None. |

Two findings stand out beyond the gaps in the summary:

- **The Ruins zone gets none of this.** `zones/ruins.rs` lights its terrain
  on the CPU with one fixed direction and draws it through the amber shader.
  Every lighting item below reaches Ruins only after it moves to the physical
  path.
- **WebGL2 and GLES already lose post-processing on some devices.** When the
  adapter cannot render and blend a float target, the physical path tone-maps
  per draw and skips bloom and exposure. Any new effect must degrade the same
  way, and the plan below keeps every first-phase item inside that envelope.

## Platform envelope

Every recommendation is judged against these constraints:

- **WebGL2 through wgpu** has no compute shaders, no storage buffers or
  storage textures, no ray queries, and cannot sample a multisampled depth
  buffer. Float render targets depend on `EXT_color_buffer_float`. It does
  support 3D textures, 2D texture arrays, cube maps, and comparison samplers.
- **WebGPU** adds compute and storage, but the web build must still fall back
  to WebGL2, so a compute-only effect is a desktop or WebGPU tier feature.
- **Phones** are tile-based. Full-screen passes cost memory bandwidth; MSAA
  resolved in tile memory is cheap; a depth prepass doubles vertex work. iOS
  Metal and Android Vulkan have compute, but Android's GLES fallback (and the
  emulator) does not.
- **wgpu 29** has no portable ray queries on iOS, Android, or the web.

So the default target is everything that runs as vertex and fragment work or
as CPU precomputation. Screen-space effects need a readable single-sample
depth, which today means a depth prepass, and belong in a quality tier.

## Global illumination and reflections

### Lumen — Reject, adapt three ideas

- **Unreal:** `Renderer/Private/Lumen/` (`LumenScene.cpp`,
  `LumenSurfaceCache.cpp`, `LumenRadiosity.cpp`, `LumenScreenProbeGather.cpp`,
  `LumenRadianceCache.cpp`, `LumenIrradianceFieldGather.cpp`,
  `LumenReflections.cpp`, `LumenHardwareRayTracingCommon.cpp`); shaders in
  `Shaders/Private/Lumen/`. Console variables include
  `r.Lumen.ScreenProbeGather.DownsampleFactor` (16),
  `r.Lumen.ScreenProbeGather.TracingOctahedronResolution` (8),
  `r.LumenScene.SurfaceCache.AtlasSize` (4096),
  `r.LumenScene.GlobalSDF.Resolution` (252), and `r.Lumen.HardwareRayTracing`.
- **What it does:** fully dynamic diffuse GI and glossy reflections, with no
  baking.
- **Core idea:** Lumen separates *where light is stored* from *how rays find
  it*. Storage is the **surface cache**: each mesh carries a few oriented
  "cards", rasterized at low resolution into an atlas of material attributes,
  then lit (direct light plus a radiosity pass with its own probes) so a ray
  hit can look up outgoing light without shading the material. Rays are
  traced in a hierarchy that grows coarser with distance: a short **screen
  trace** against the depth buffer, then per-mesh **signed distance fields**
  for mid range, then a **global distance field** clipmap for far range, or a
  hardware ray-tracing path that replaces the distance fields. Rays start
  from **screen probes**, one per 16×16-pixel tile, each tracing an 8×8
  octahedral map that is importance-sampled, filtered between neighbors, and
  integrated per pixel. Long rays are expensive and noisy, so beyond a
  short distance they stop and read a **radiance cache**: world-space probe
  clipmaps around the camera, updated under a per-frame budget. A separate
  irradiance-field gather mode replaces screen probes with a pure world-probe
  grid for lower cost.
- **Cost and platforms:** many compute passes per frame, signed-distance
  generation for every mesh, temporal accumulation that needs history buffers
  and upscaling, and gigabytes of resident data at high settings. Not
  available on Unreal's own ES3.1 phone path. Impossible on WebGL2.
- **Verse fit:** Verse's zones are small, mostly static, and lit by one sun
  or a few torches. The light rarely changes, so most of what Lumen computes
  every frame can be computed once.
- **Verdict: Reject.** Adapt three ideas:
  1. *World-space probes with an update budget* are the radiance cache's
     essence and already exist as `pbr::ProbeGrid`. Rebake them on a worker
     when the light or the static scene changes, as Lagrange does (plan item
     B1).
  2. *Short-range screen-space occlusion on top of coarse probes* is how
     Lumen keeps contact detail; for Verse that is GTAO on the high tier
     (item G1).
  3. *Normalize far reflections by local diffuse light* so that interiors do
     not reflect a bright sky (item B2).
- **Public reference:** Wright, Narkowicz, and Kelly, "Lumen: Real-time
  Global Illumination in Unreal Engine 5" (SIGGRAPH 2022 Advances); Majercik
  et al., "Dynamic Diffuse Global Illumination with Ray-Traced Irradiance
  Fields" (JCGT 2019) for the probe-grid variant.

### Lightmass, lightmaps, and GPU Lightmass — Adapt a simplified version

- **Unreal:** `Source/Programs/UnrealLightmass` and `Plugins/Experimental/GPULightmass`
  bake; `Renderer/Private/LightMapRendering.cpp` and
  `Shaders/Private/LightmapCommon.ush` sample. `Config/BaseLightmass.ini`
  holds the bake settings.
- **What it does:** precomputes indirect light (and optionally static direct
  shadows) for static geometry into lightmap textures over a second, unique
  UV set, plus a probe volume for moving objects.
- **Core idea:** an offline global-illumination solve (photon mapping with a
  final gather in the CPU baker, path tracing in the GPU baker), stored as
  directional lightmaps so normal maps still respond, with sky occlusion baked
  alongside so the sky light darkens under cover.
- **Cost and platforms:** free at run time, one texture fetch per pixel. The
  cost is authoring: every static mesh needs non-overlapping lightmap UVs,
  every placement change needs a rebake, and lightmap atlases add megabytes
  to a pack.
- **Verse fit:** Everglade's meshes come from CC0 kits with one UV set,
  alpha-tested leaf cards, and instanced placement, so lightmap UVs are a poor
  fit. Verse already owns a CPU baker (`pbr/bake.rs`: a BVH, per-vertex AO,
  sun visibility, and one-bounce SH probes) that needs no UVs.
- **Verdict: Adapt.** Bake per-vertex sky visibility and occlusion for static
  textured meshes, and a bounce probe grid with sky visibility, using the
  existing BVH (item B1). **Reject** UV lightmaps: the unwrap and atlas work
  does not pay for itself in low-poly stylized scenes whose vertex density is
  enough to carry soft occlusion.
- **Public reference:** Jensen, *Realistic Image Synthesis Using Photon
  Mapping* (2001); Ramamoorthi and Hanrahan, "An Efficient Representation for
  Irradiance Environment Maps" (2001); Iwanicki and Sloan, "Precomputed
  Lighting in Call of Duty: Infinite Warfare" (SIGGRAPH 2017).

### Volumetric lightmaps and the indirect lighting cache — Pull in now (already half built)

- **Unreal:** `Renderer/Private/IndirectLightingCache.cpp` (the older
  per-object cache), `Renderer/Private/VisualizeVolumetricLightmap.cpp` and
  the `PRECOMPUTED_IRRADIANCE_VOLUME_LIGHTING` and
  `CACHED_POINT_INDIRECT_LIGHTING` paths in
  `Shaders/Private/MobileBasePassPixelShader.usf`.
- **What it does:** gives moving objects (characters, props) the same baked
  indirect light as the static world.
- **Core idea:** store SH irradiance at points in space. The older cache
  interpolated a few probes per object; the volumetric lightmap stores an
  adaptive brick hierarchy, dense near geometry and sparse in open air, that
  every pixel samples with trilinear filtering. Each probe also stores sky
  visibility, so the sky light dims inside buildings.
- **Cost and platforms:** one 3D texture lookup per pixel; fits WebGL2 and
  phones, and Unreal's mobile path uses it.
- **Verse fit:** `pbr::ProbeGrid` is the uniform-grid form of this with
  order-one SH, and the lit shader already samples it from three 3D textures.
  Everglade feeds it two constants instead of a bake.
- **Verdict: Pull in now** as part of item B1: bake the grid for Everglade,
  add a sky-visibility term per probe so the sky ambient (item A2) is
  occluded under roofs and canopy, and let characters sample the same grid.
  Brick hierarchies are **Pull in later**, when a zone outgrows a uniform grid
  in memory.
- **Public reference:** Sloan, "Stupid Spherical Harmonics Tricks" (GDC
  2008); Iwanicki and Sloan (2017).

### Reflection captures — Adapt a simplified version

- **Unreal:** `Renderer/Private/ReflectionEnvironment.cpp`,
  `ReflectionEnvironmentCapture.cpp`, `MobileReflectionEnvironmentCapture.cpp`;
  `Shaders/Private/ReflectionEnvironmentShared.ush`
  (`ComputeReflectionCaptureMipFromRoughness`).
- **What it does:** placed sphere or box probes capture a cubemap of the
  scene, prefiltered by roughness into mips; glossy pixels sample the nearest
  captures with parallax correction.
- **Core idea:** split-sum prefiltered environment lighting, plus
  *normalization*: the capture's brightness is rescaled by the ratio of the
  pixel's own diffuse indirect light to the capture's average, which hides the
  error of using one capture point for a whole room.
- **Cost and platforms:** offline or load-time capture; one cubemap lookup per
  pixel. On phones it is Unreal's main specular source.
- **Verse fit:** Everglade's glossy surfaces (glass, metal fittings, wet
  stone) reflect nothing. A single sky cube (item A2) fixes outdoor
  reflections. The workshop interior is the only place where a local capture
  would change the picture.
- **Verdict: Adapt.** The sky cube with probe normalization is **Pull in
  now** (items A2 and B2). Box-projected local captures are **Pull in
  later**, after B1, for interiors.
- **Public reference:** Karis, "Real Shading in Unreal Engine 4" (SIGGRAPH
  2013); Lagarde and Zanuttini, "Local Image-Based Lighting with
  Parallax-Corrected Cubemaps" (SIGGRAPH 2012).

### Screen-space reflections — Reject for now

- **Unreal:** `Renderer/Private/ScreenSpaceRayTracing.cpp`,
  `ScreenSpaceReflectionTiles.cpp`, `MobileSSR.cpp`;
  `Shaders/Private/SSRT/SSRTReflections.usf`, `SSRTRayCast.ush`.
- **Core idea:** march the reflected ray through a hierarchical depth buffer
  and read the previous frame's color where it hits, falling back to
  captures where it leaves the screen. The mobile variant works only on the
  forward path and is off by default.
- **Cost and platforms:** needs a resolved depth pyramid, last frame's color,
  and usually temporal denoising. Expensive on phones; awkward with forward
  MSAA.
- **Verdict: Reject** until a zone has water or polished floors as a central
  feature. Stylized low-poly scenes have few mirror-like surfaces, and the sky
  cube covers the rest.
- **Public reference:** McGuire and Mara, "Efficient GPU Screen-Space Ray
  Tracing" (JCGT 2014).

## Direct lighting and shadows

### MegaLights — Reject, keep the light budget

- **Unreal:** `Renderer/Private/MegaLights/` (`MegaLightsSampling.cpp`,
  `MegaLightsRayTracing.cpp`, `MegaLightsDenoising.cpp`);
  `Shaders/Private/MegaLights/`. `r.MegaLights.NumSamplesPerPixel` (4),
  `r.MegaLights.DownsampleMode` (2), `r.MegaLights.ScreenTraces`.
- **What it does:** shadows hundreds of local lights at a fixed cost.
- **Core idea:** instead of shading every light, each downsampled pixel picks
  a few lights at random in proportion to their estimated contribution, traces
  one shadow ray to each (screen trace first, then hardware or distance-field
  rays), and a spatial and temporal denoiser turns the noisy result into
  smooth lighting. Cost follows samples per pixel, not light count.
- **Cost and platforms:** ray tracing, compute, history buffers, and a
  temporal upscaler to hide the noise. Not a phone or WebGL2 technique.
- **Verse fit:** the chamber has 16 base lights plus spell lights, under
  `verse_engine::lighting::MAX_LIGHTS` (32) with four shadowed. That budget
  holds for the planned scenes.
- **Verdict: Reject.** Two cheaper ideas carry over:
  1. *Choose shadowed lights by contribution*, not by list order: rank lights
     by estimated screen-space irradiance each frame and give the top four the
     cube maps (item S3).
  2. If scenes exceed 32 lights, use **clustered forward** light lists, as
     Unreal's mobile base pass does (`ENABLE_CLUSTERED_LIGHTS`). On WebGL2
     the cluster index must live in a texture, not a storage buffer. **Pull in
     later**, with a scene that needs it.
- **Public reference:** Bitterli et al., "Spatiotemporal Reservoir
  Resampling" (SIGGRAPH 2020) for the sampling idea; Olsson, Billeter, and
  Assarsson, "Clustered Deferred and Forward Shading" (HPG 2012).

### Virtual shadow maps — Reject

- **Unreal:** `Renderer/Private/VirtualShadowMaps/`
  (`VirtualShadowMapArray.cpp`, `VirtualShadowMapClipmap.cpp`,
  `VirtualShadowMapCacheManager.cpp`); `Shaders/Shared/VirtualShadowMapDefinitions.h`
  sets 128-texel pages and a 128×128-page level 0, a 16,384² virtual map.
- **Core idea:** a huge virtual shadow map per light, of which only pages that
  visible pixels need are allocated and rendered, arranged as clipmap levels
  around the camera for directional lights. Pages whose casters did not move
  stay cached between frames. Soft shadows come from shadow-map ray tracing
  (`r.Shadow.Virtual.SMRT.*`): a few rays marched through the depth map per
  pixel.
- **Cost and platforms:** GPU page marking, allocation, and culling in compute,
  designed around Nanite's fine-grained geometry. No WebGL2 path.
- **Verdict: Reject.** The caching idea is already in Verse
  (`imported/shadow_cache.rs` caches static casters for cube lights) and
  carries into cascades (item S1).
- **Public reference:** Fernando et al., "Adaptive Shadow Maps" (SIGGRAPH
  2001); Lefohn, Sengupta, and Owens, "Resolution-Matched Shadow Maps" (ACM
  TOG 2007).

### Cascaded shadow maps — Pull in now

- **Unreal:** `Renderer/Private/ShadowSetup.cpp`, `ShadowRendering.cpp`,
  `ShadowSetupMobile.cpp`, `Shadows/CSMSceneExtension.cpp`;
  `Shaders/Private/ShadowProjectionPixelShader.usf`,
  `ShadowPercentageCloserFiltering.ush`. Console variables
  `r.Shadow.CSM.MaxCascades`, `r.Shadow.CSM.TransitionScale`,
  `r.Shadow.CSMSplitPenumbraScale`, `r.Shadow.CSMShadowDistanceFadeoutMultiplier`,
  `r.Shadow.CSMCaching`, `r.Shadow.MaxCSMResolution`.
- **What it does:** shadows from the sun across the whole view distance.
- **Core idea:** split the view frustum into depth ranges on a curve between
  uniform and logarithmic, give each a shadow map, fit each map to a bounding
  sphere of its slice so the projection does not change size as the camera
  turns, and snap its origin to whole texels so edges do not crawl while the
  camera moves. Blend across a band at each split, scale filter width per
  cascade so penumbrae match, fade the last cascade out at the shadow
  distance, and on mobile cull casters per cascade and optionally cache static
  casters.
- **Cost and platforms:** one depth pass per cascade, one or two comparison
  lookups per pixel. Unreal's mobile path ships it. Fits WebGL2 with a 2D
  texture array or an atlas.
- **Verse fit:** Everglade's map is fitted to a fixed box (`shadow_center`,
  `shadow_half` 40 m) at 2048², about 3.9 cm per texel. Inside it shadows are
  sharp; past it the world is unshadowed, and the zone's fog does not close
  until 170 m.
- **Verdict: Pull in now** (item S1): two cascades on phones and WebGL2,
  three on desktop, fitted to bounding spheres with texel snapping, a blend
  band, and a distance fade. The sun is static in Everglade, so the far
  cascade can render only when the camera crosses a texel-snapped cell and
  cache the static-caster depth, as the chamber's cube cache does. Keep PCSS
  for the near cascade on Metal and Vulkan; GLES keeps its fixed-radius
  filter.
- **Public reference:** Zhang et al., "Parallel-Split Shadow Maps" (2006);
  Dimitrov, "Cascaded Shadow Maps" (NVIDIA, 2007); Valient, "Stable Cascaded
  Shadow Maps" (ShaderX6, 2008); Microsoft, "Common Techniques to Improve
  Shadow Depth Maps".

### Distance field shadows and capsule shadows — Reject DF, adapt capsules later

- **Unreal:** `Renderer/Private/DistanceFieldShadowing.cpp`,
  `CapsuleShadowRendering.cpp`; `Shaders/Private/DistanceFieldShadowing.usf`,
  `CapsuleShadowShaders.usf`. Mesh fields come from
  `Runtime/Engine/Private/DistanceFieldAtlas.cpp` (`r.GenerateMeshDistanceFields`).
- **Core idea:** march a shadow ray through each mesh's precomputed signed
  distance field; the nearest miss distance along the ray gives a soft
  penumbra that widens with occluder distance. Capsule shadows apply the same
  idea to a character's skeleton approximated as capsules, to cast soft
  shadows from the sky light and from indirect light, where shadow maps do not
  reach.
- **Cost and platforms:** mesh distance fields need a 3D atlas built per mesh
  and compute culling; mobile support is optional
  (`r.Mobile.AllowDistanceFieldShadows`). Capsule shadows are analytic and
  cheap: a short uniform array of capsules per character.
- **Verdict:** mesh distance field shadows **Reject** (cascades are cheaper
  for Verse's scene sizes). Capsule shadows **Pull in later** (item S4): a
  soft occlusion blob under each character from the sky ambient, so figures
  stay grounded where the sun is shadowed, in the workshop and under trees.
- **Public reference:** Wright, "Dynamic Occlusion with Signed Distance
  Fields" (SIGGRAPH 2015 Advances); Iwanicki, "Lighting Technology of The Last
  of Us" (SIGGRAPH 2013) for ellipsoid and capsule occlusion.

### Screen-space contact shadows — Pull in later

- **Unreal:** `Renderer/Private/Shadows/ScreenSpaceShadows.cpp`,
  `Shaders/Private/ScreenSpaceShadows.usf`, `ScreenSpaceShadowRayCast.ush`;
  the Bend Studio implementation is vendored beside it (`bend_sss_cpu.h`).
- **Core idea:** a short ray toward the light through the depth buffer catches
  small occluders the shadow map misses: the gap where a foot meets the floor
  or a cup on a table.
- **Verdict: Pull in later** (item G2), on the high tier, after the depth
  prepass that GTAO needs.
- **Public reference:** Bend Studio, "Screen Space Shadows" (SIGGRAPH 2023;
  sample under Apache-2.0).

## Sky, atmosphere, and fog

### SkyAtmosphere — Adapt a simplified version (in progress elsewhere)

- **Unreal:** `Renderer/Private/SkyAtmosphereRendering.cpp`,
  `SkyPassRendering.cpp`; `Shaders/Private/SkyAtmosphere.usf`,
  `SkyAtmosphereCommon.ush`. Console variables
  `r.SkyAtmosphere.TransmittanceLUT.Width` (256),
  `r.SkyAtmosphere.FastSkyLUT.Width` and `.Height` (192 by 104),
  `r.SkyAtmosphere.AerialPerspectiveLUT.Width` (32) and `.DepthResolution`
  (16 slices over 96 km).
- **Core idea:** a planet-scale atmosphere with Rayleigh, Mie, and ozone
  layers, rendered through small lookup tables: transmittance by height and
  sun angle, a multiple-scattering table that replaces higher scattering
  orders with an isotropic approximation, a sky-view table in
  latitude-longitude around the camera, and a camera-frustum volume of
  in-scattering and transmittance for aerial perspective on distant
  geometry.
- **Cost and platforms:** the tables are tiny and update only when the sun or
  camera altitude changes; Unreal's mobile path renders it. On WebGL2 every
  table can be a fragment pass into a 2D texture (or computed on the CPU for
  a fixed sun); the 3D aerial-perspective volume can be a 2D atlas of slices.
- **Verdict: Adapt.** Another agent is adding an Everglade sky pass in this
  style; this note does not duplicate its design. Two follow-ons depend on
  it: the sky must be callable as a function of direction so the environment
  bake (item A2) can integrate it, and the fog (item A3) should take its
  in-scattering color from the same sky so the horizon and distant geometry
  agree.
- **Public reference:** Hillaire, "A Scalable and Production Ready Sky and
  Atmosphere Rendering Technique" (EGSR 2020), with its MIT-licensed sample
  `sebh/UnrealEngineSkyAtmosphere`; Bruneton and Neyret, "Precomputed
  Atmospheric Scattering" (EGSR 2008, BSD-3 code).

### SkyLight and real-time capture — Pull in now

- **Unreal:** `Renderer/Private/ReflectionEnvironmentRealTimeCapture.cpp`
  (`r.SkyLight.RealTimeReflectionCapture.TimeSlice`),
  `ReflectionEnvironmentDiffuseIrradiance.cpp`;
  `Shaders/Private/SkyLightingDiffuseShared.ush`, `SkyLightingShared.ush`.
  `MobileShadingRenderer.cpp` enables real-time capture on phones too.
- **What it does:** turns the sky into ambient light: diffuse irradiance as
  SH and a roughness-prefiltered cubemap for glossy reflections.
- **Core idea:** render the sky (and optionally distant geometry) into a
  small cubemap, convolve it with a GGX lobe per mip for specular, project it
  onto low-order SH for diffuse, and spread that work over several frames.
  An option treats the lower hemisphere as black or as a ground color.
- **Cost and platforms:** a few hundred thousand texels of work, once per
  sun change. Fits every platform; for a fixed or slowly moving sun it can
  run on the CPU at zone load.
- **Verse fit:** Everglade's `Key::probes` builds a uniform sky-over-ground
  term from two lux constants, and specular ambient reads the linear probe in
  the reflection direction. Neither has the sky's color gradient, the sun's
  aureole, or any horizon.
- **Verdict: Pull in now** (item A2). Integrate the analytic sky over
  directions to get order-two SH (nine coefficients per channel) for diffuse
  and a 6×32² to 6×64² cube with GGX-prefiltered mips for specular, sampled
  through the existing analytic environment BRDF. The probe grid's sky
  visibility (item B1) scales the sky term, so interiors stay dark.
- **Public reference:** Ramamoorthi and Hanrahan (2001); Karis (2013) for the
  split sum; Lagarde and de Rousiers, "Moving Frostbite to PBR" (2014), for
  sky lighting in physical units.

### Exponential height fog — Pull in now

- **Unreal:** `Renderer/Private/FogRendering.cpp`, `MobileFogRendering.cpp`;
  `Shaders/Private/HeightFogCommon.ush`, `HeightFogPixelShader.usf`,
  `MobileFog.usf`.
- **Core idea:** fog density falls off exponentially with height, so the
  optical depth along a view ray has a closed form. Unreal sums two such
  layers, starts fog at a set distance, clamps opacity, adds a
  directional-inscattering lobe that brightens fog toward the sun, and can
  color the fog from the sky's captured light instead of one constant.
- **Cost and platforms:** a few instructions per pixel in the forward pass.
  Fits WebGL2 and phones.
- **Verse fit:** Everglade's fog is a linear ramp by distance to one color;
  the chamber's is exponential by distance only. Neither thins with altitude,
  so a hilltop and a hollow fog the same, and neither glows toward the sun.
- **Verdict: Pull in now** (item A3). One function in the lit, textured, and
  chamber shaders: height-exponential optical depth, start distance, maximum
  opacity, a sun lobe, and the fog color from the sky's horizon (item A2) or
  the zone's air color. Keep the zones' existing fog ranges as the limit, and
  keep the amber plaza's ramp, which is part of its look.
- **Public reference:** Wenzel, "Real-Time Atmospheric Effects in Games
  Revisited" (GDC 2007); Quílez, "Better Fog" (iquilezles.org).

### Volumetric fog and local fog volumes — Reject volumetric, adapt later

- **Unreal:** `Renderer/Private/VolumetricFog.cpp`
  (`r.VolumetricFog.GridPixelSize` 16, `r.VolumetricFog.GridSizeZ` 64,
  `r.VolumetricFog.HistoryWeight` 0.9), `VolumetricFogVoxelization.cpp`;
  `Shaders/Private/VolumetricFog.usf`. Local volumes in
  `Renderer/Private/LocalFogVolumeRendering.cpp` and
  `Shaders/Private/LocalFogVolumes/`. Screen-space light shafts in
  `Renderer/Private/LightShaftRendering.cpp`.
- **Core idea:** a camera-aligned froxel grid (16-pixel cells, 64 depth
  slices) holds participating media; compute passes inject density and
  lighting (with shadows) into each cell, reproject last frame's result to
  hide noise, and integrate front to back so every pixel reads its
  in-scattering. Local fog volumes are analytic spheres of height fog, shaded
  per pixel without a grid.
- **Cost and platforms:** compute-heavy with temporal history; not on
  WebGL2 or Unreal's phone path.
- **Verdict:** volumetric fog **Reject**. Analytic local fog volumes and an
  analytic air-light term for point lights in uniform fog **Pull in later**
  (item A5), for torch glow in the chamber and mist in hollows. Screen-space
  light shafts are a cheap later option for sun through trees.
- **Public reference:** Wronski, "Volumetric Fog" (SIGGRAPH 2014 Advances);
  Hillaire, "Physically Based and Unified Volumetric Rendering in Frostbite"
  (SIGGRAPH 2015); Sun, Ramamoorthi, Narasimhan, and Nayar, "A Practical
  Analytic Single Scattering Model" (SIGGRAPH 2005); Mitchell, "Volumetric
  Light Scattering as a Post-Process" (GPU Gems 3).

### Volumetric clouds — Reject, adapt a painted layer later

- **Unreal:** `Renderer/Private/VolumetricCloudRendering.cpp`
  (`r.VolumetricCloud.ViewRaySampleMaxCount` 768, `r.VolumetricCloud.ShadowMap`),
  `VolumetricRenderTarget.cpp` (`r.VolumetricRenderTarget.Mode`: trace at
  quarter or half resolution and reconstruct over frames);
  `Shaders/Private/VolumetricCloud.usf`.
- **Core idea:** ray-march a cloud layer defined by material noise, with
  light marching toward the sun, multiple-scattering approximations, and
  temporal reconstruction from reduced-resolution traces; a cloud shadow map
  darkens the ground.
- **Verdict: Reject.** For stylized zones, a cloud layer drawn in the sky
  pass from a 2D texture or a few noise octaves, lit by the sun direction,
  plus a scrolling cloud-shadow texture on the ground, gives the read at
  almost no cost (item A6, later).
- **Public reference:** Schneider and Vos, "The Real-Time Volumetric
  Cloudscapes of Horizon Zero Dawn" (SIGGRAPH 2015 Advances); Hillaire,
  "Physically Based Sky, Atmosphere and Cloud Rendering in Frostbite"
  (SIGGRAPH 2016).

## Ambient occlusion

### Distance field ambient occlusion — Reject

- **Unreal:** `Renderer/Private/DistanceFieldAmbientOcclusion.cpp`
  (`r.DistanceFieldAO`, `r.AOQuality`), `DistanceFieldScreenGridLighting.cpp`;
  `Shaders/Private/DistanceFieldAOShared.ush`.
- **Core idea:** cone-trace mesh distance fields from a sparse screen grid to
  get medium-range sky occlusion for movable skies, then upsample.
- **Verdict: Reject** for the same reasons as distance field shadows. With a
  static sky, the baked per-vertex occlusion of item B1 gives the same
  medium-range result for free at run time.

### SSAO and GTAO — Pull in later, on the high tier

- **Unreal:** `Renderer/Private/CompositionLighting/PostProcessAmbientOcclusion.cpp`
  (`r.AmbientOcclusion.Method`), `CompositionLighting.cpp`
  (`r.GTAO.Downsample`), `PostProcess/PostProcessAmbientOcclusionMobile.cpp`
  (`r.Mobile.AmbientOcclusion`, off by default, forward shading only);
  `Shaders/Private/PostProcessAmbientOcclusion.usf`.
- **Core idea:** ground-truth AO searches the depth buffer along a few screen
  directions per pixel for the horizon angles on each side, integrates the
  visible arc of the hemisphere analytically against the cosine, adds a
  thickness heuristic so thin objects do not over-occlude, and denoises
  spatially and temporally. The classic SSAO it replaces samples points in a
  hemisphere.
- **Cost and platforms:** needs a readable depth (and ideally normals), a
  half-resolution trace, and a blur. On phones it is a full-screen pass that
  Unreal leaves off by default. On Verse's forward MSAA path, WebGL2 cannot
  sample the multisampled depth, so it needs a single-sample depth prepass.
- **Verdict: Pull in later** (item G1): a GTAO-lite at half resolution with
  two directions and a bilateral blur, on the desktop and WebGPU tier, applied
  to ambient light only. Baked vertex AO (item B1) is the all-platform answer
  and lands first; GTAO then adds contact detail for moving characters.
- **Public reference:** Jimenez, Wu, Pesce, and Jarabo, "Practical Real-Time
  Strategies for Accurate Indirect Occlusion" (SIGGRAPH 2016); Bavoil and
  Sainz, "Image-Space Horizon-Based Ambient Occlusion" (2008).

## Camera and post-processing

### Auto exposure and eye adaptation — Adapt small parts

- **Unreal:** `Renderer/Private/PostProcess/PostProcessEyeAdaptation.cpp`
  (`r.EyeAdaptation.ExponentialTransitionDistance`),
  `PostProcessHistogram.cpp`; `Shaders/Private/PostProcessHistogram.usf`
  (`HISTOGRAM_SIZE` 64), `PostProcessEyeAdaptation.usf`,
  `EyeAdaptationCommon.ush`.
- **Core idea:** build a 64-bin log-luminance histogram, discard a low and a
  high percentile, average the rest, apply an exposure-compensation curve
  that varies with scene brightness, and move toward the target at different
  speeds when brightening and darkening.
- **Verse fit:** Verse meters a 16×10 grid of the smallest bloom level as a
  center-weighted log average with a highlight guard
  (`pbr/post.wgsl`, `fs_adapt`). That is close to Unreal's basic mode and
  needs no compute, which a histogram does.
- **Verdict: Adapt** (item P3): separate speeds for brightening and
  darkening, and an exposure-compensation curve per zone. Keep the grid meter,
  which runs on WebGL2. Stylized zones such as Everglade can keep a fixed
  EV100.
- **Public reference:** Lagarde and de Rousiers (2014); Reinhard et al.,
  "Photographic Tone Reproduction for Digital Images" (2002).

### Tone mapping, ACES, and the filmic curve — Adapt: one output transform

- **Unreal:** `Renderer/Private/PostProcess/PostProcessTonemap.cpp`,
  `PostProcessCombineLUTs.cpp`, `PostProcessLocalExposure.cpp`;
  `Shaders/Private/PostProcessTonemap.usf`, `TonemapCommon.ush`, and the ACES
  1.3 and 2.0 transforms under `Shaders/Private/ACES/`. Filmic parameters
  (`FilmSlope`, `FilmToe`, `FilmShoulder`, `FilmBlackClip`, `FilmWhiteClip`)
  live in the post-process settings. `r.Mobile.TonemapSubpass` runs the
  tone mapper in tile memory on phones.
- **Core idea:** white balance, color grading, the filmic curve, and the
  display transform are all baked into one 32³ color lookup table each frame,
  so the full-screen tone pass is a single 3D-texture fetch whatever the
  grade. The curve has an adjustable toe and shoulder in the style of the
  ACES reference rendering transform.
- **Verse fit:** the physical path already has Khronos PBR Neutral, a
  hue-preserving shoulder for neon stages, local exposure, and an EDR ceiling.
  The chamber path instead applies an ACES curve fit inside each fragment,
  with no bloom or exposure, so the same torch looks different on the two
  paths.
- **Verdict: Adapt** (item P1). Route the chamber through the physical path's
  float target and output pass, so every zone shares one exposure, bloom, and
  tone curve. Then bake white balance, a per-zone grade, and the curve into a
  small 3D LUT, as Unreal does, so adding a filmic or AgX look later is data,
  not shader code. Keep Neutral as the default: it preserves hue, which the
  amber palette and Everglade's greens depend on. Full ACES output transforms
  are **Reject**: they shift hue in ways stylized palettes fight.
- **Public reference:** Khronos PBR Neutral (Apache-2.0 reference); Academy
  ACES; Hable, "Filmic Tonemapping Operators" (2010); Sobotka, AgX.

### Bloom, lens flares, and FFT bloom — Already have; reject FFT

- **Unreal:** `Renderer/Private/PostProcess/PostProcessBloomSetup.cpp`,
  `PostProcessFFTBloom.cpp`, `PostProcessLensFlares.cpp`,
  `PostProcessDownsample.cpp`; `Shaders/Private/PostProcessBloom.usf`,
  `PostProcessLensFlares.usf`, and the `Bloom/` folder.
- **Core idea:** a six-level Gaussian chain with per-level size and tint, or
  convolution with an arbitrary kernel image in frequency space; lens ghosts
  reuse the bloom chain mirrored about the center.
- **Verdict:** Verse already has energy-conserving mip bloom, ghosts,
  vignette, and grain. The chamber path gains them through item P1. FFT
  bloom **Reject**: desktop-only cost for a look the stylized zones do not
  need.

## Unreal's mobile renderer — the reference profile

- **Unreal:** `Renderer/Private/MobileShadingRenderer.cpp`,
  `MobileBasePass.cpp`, `MobileBasePassRendering.cpp`,
  `MobileDeferredShadingPass.cpp`, `ShadowSetupMobile.cpp`,
  `PostProcess/PostProcessMobile.cpp`;
  `Shaders/Private/MobileBasePassPixelShader.usf`, `MobileLightingCommon.ush`,
  `MobileGGX.ush`, `PostProcessMobile.usf`.
- **What Unreal does on phones:** a forward base pass by default (deferred is
  optional), so MSAA stays cheap in tile memory. One directional light with
  cascaded shadows, with per-cascade caster culling. Local lights through a
  clustered list; shadows for movable spot lights only when enabled. The sky
  light as SH for diffuse and a cubemap for specular, with real-time capture
  available. Reflection captures as the main specular source. Lightmaps and
  the volumetric lightmap for indirect light. Height fog in the base pass.
  Optional SSAO and SSR, both off by default and forward-only. A tone-map
  subpass that never leaves tile memory. Material quality levels that strip
  features per device (`MOBILE_QL_FORCE_FULLY_ROUGH`,
  `MOBILE_QL_FORCE_NONMETAL`, `MOBILE_QL_DISABLE_MATERIAL_NORMAL`).
- **Verse fit:** Verse's forward MSAA design already follows this shape.
  What is missing is the environment (sky light, captures, baked indirect,
  height fog, cascades) and the **quality levels**: Verse has one
  capability probe (`pbr::gpu::Capability`: float format, sample count, GLES)
  but no tier that scales effects.
- **Verdict: Pull in now** as the organizing profile. Adopt explicit quality
  tiers (item T1) that select cascade count, PCSS or fixed PCF, GTAO and
  contact shadows, probe resolution, and material simplifications (fully
  rough, no normal map) for low-end Android and WebGL2.
- **Public reference:** Epic's public mobile rendering documentation; Arm,
  "Mali GPU Best Practices" for tile-based costs.

## Other systems considered

| Unreal system | Pointer | Verdict |
| --- | --- | --- |
| Light functions and the light function atlas | `Renderer/Private/LightFunctionAtlas.cpp` | Pull in later, as a cloud-shadow texture on the sun (item A6). |
| Rect lights and textured area lights | `Renderer/Private/RectLightTextureManager.cpp`, `Shaders/Private/RectLight.ush` | Reject: no rectangular emitters in the planned scenes. Torches are points. |
| Translucency lighting volume | `Renderer/Private/TranslucentLighting.cpp` | Reject: Verse's translucent surfaces (glass, spell effects) shade forward per pixel. |
| Lumen and SSRT screen-space diffuse indirect | `Shaders/Private/SSRT/SSRTDiffuseIndirect.usf` | Reject: compute, temporal history, and noise for little gain over baked probes. |
| Hardware ray tracing (Lumen and MegaLights paths) | `Renderer/Private/Lumen/LumenHardwareRayTracingCommon.cpp` | Reject: no portable ray queries on iOS, Android, or the web. |
| Temporal super resolution | `Renderer/Private/PostProcess/TemporalSuperResolution.cpp` | Reject for lighting work: nothing recommended here needs temporal accumulation. |

## Phased plan

Ranked by visual gain per cost across all platforms. Each phase is visible
and testable alone; captures before and after, at fixed exposure, are the
acceptance evidence, as in the Lagrange audit. Size: **S** under a day,
**M** a few days, **L** a phase of its own. Portable values (cascade fitting,
fog parameters, tier selection, SH math) belong in `crates/verse-engine` so
they are tested headless; GPU passes and shaders belong in `crates/verse`.

### Phase 1 — One output and tiers

| ID | Item | Size | Crate | Verdict |
| --- | --- | --- | --- | --- |
| P1 | Route the chamber path through the float scene target and the shared output pass (bloom, exposure, Neutral), retiring its per-fragment ACES fit; bake white balance and a per-zone grade into a 3D LUT. | M | `verse` (`imported/scene.wgsl`, `pbr/gpu.rs`, `pbr/post.wgsl`); grade values in `verse-engine::lighting` | Pull in now |
| T1 | Quality tiers derived from the capability probe and the platform: cascade count, shadow filter, screen-space effects, probe resolution, material simplifications. | S–M | `verse-engine` (tier contract), `verse` (`pbr/gpu.rs` `Capability`) | Pull in now |

**Accept:** a torch renders the same in the chamber and on a lit stage at
equal exposure; a WebGL2 run selects the low tier and still draws every
phase 2 item.

**Status:** P1 and T1 are implemented under issue #10561. The output pass is
`crates/verse/src/pbr/output.rs`; grades are `verse_engine::lighting::Grade`
and tiers `verse_engine::quality`. The table holds the scene-referred grade
only: Neutral and the hue-preserving shoulder scale by a color's peak, which
a 32³ table interpolates with errors near 10% at the shoulder, so the curve
stays analytic after the table.

### Phase 2 — Sky light and fog

| ID | Item | Size | Crate | Verdict |
| --- | --- | --- | --- | --- |
| A1 | Everglade sky pass in the SkyAtmosphere style. | — | `verse` | In progress in another change; dependency only |
| A2 | Sky-derived ambient: integrate the analytic sky into order-two SH for diffuse and a small GGX-prefiltered cube for specular, on the CPU or in a load-time pass, recomputed only when the sun moves. Replaces `Key::sky`/`ground` and the probe-in-reflection-direction specular. | M | `verse` (new `pbr` environment module, `photo.wgsl`); SH projection in `verse-engine` | Pull in now |
| A3 | Exponential height fog with start distance, opacity cap, a sun in-scattering lobe, and color from the sky's horizon; in the lit, textured, and chamber shaders. | S | `verse` (shaders), parameters in `verse-engine::lighting` and `zones::Atmosphere` | Pull in now |

**Accept:** Everglade's shaded sides take the sky's blue-to-horizon gradient;
glass and metal reflect the sky; fog thins on high ground and glows toward
the sun; the amber plaza is unchanged.

**Status:** A2, A3, and B2 are implemented under issue #10562. The SH
projection, cube prefilter, and fog math are `verse_engine::environment` and
`verse_engine::lighting::HeightFog`; the sky light is
`crates/verse/src/pbr/environment.rs`. The key's `sky` illuminance still sets
the sky light's level on surfaces facing up, so a stylized sky does not
change the stage's exposure balance; the sky sets its color and gradient.

### Phase 3 — Baked occlusion and bounce

| ID | Item | Size | Crate | Verdict |
| --- | --- | --- | --- | --- |
| B1 | Bake static zones with the existing BVH: per-vertex sky visibility and AO for textured meshes (alpha-tested cards as partial occluders), and a probe grid of one sun bounce plus sky visibility. Run on a worker at load, keyed by pack digest and sun, so a cache can skip it. Characters sample the same grid. | M–L | `verse` (`pbr/bake.rs`, `pbr/textured.rs` vertex format) | Pull in now |
| B2 | Normalize the sky cube's specular by the ratio of local probe irradiance to sky irradiance, so covered and interior surfaces stop reflecting open sky. | S | `verse` (`photo.wgsl`) | Pull in now |

**Accept:** the workshop interior and the ground under the tree ring darken
without hand-placed lights; leaf cards do not turn black; a white wall beside
sunlit grass picks up green; bake time on a phone stays within the zone's
load budget.

### Phase 4 — Sun shadows across the view

| ID | Item | Size | Crate | Verdict |
| --- | --- | --- | --- | --- |
| S1 | Camera-following cascades: two on phones and WebGL2, three on desktop; sphere fit, texel snapping, blend band, distance fade, static-caster caching for a fixed sun; PCSS on the near cascade where the blocker search is available. | M | `verse` (`pbr/gpu.rs`, `photo.wgsl`); cascade fitting in `verse-engine::lighting` | Pull in now |
| S2 | Move the Ruins zone onto the physical path, so phases 1–4 reach it. | M | `verse` (`zones/ruins.rs`) | Pull in now |
| S3 | Pick the four shadowed chamber lights by estimated contribution each frame, not list order. | S | `verse-engine::lighting`, `verse` (`imported`) | Pull in now |

**Accept:** a tree 120 m away casts a shadow; walking does not make shadow
edges crawl; cascade seams are not visible in a slow pan.

### Phase 5 — High-tier screen-space detail

| ID | Item | Size | Crate | Verdict |
| --- | --- | --- | --- | --- |
| G0 | Single-sample depth prepass on the high tier. | S–M | `verse` (`pbr/gpu.rs`, render graph declaration in `verse-engine::render_graph`) | Pull in later |
| G1 | GTAO-lite: half resolution, two directions, bilateral blur, ambient term only. | M | `verse` (new post pass) | Pull in later |
| G2 | Screen-space contact shadows for the sun and the nearest shadowed light. | S–M | `verse` | Pull in later |

**Status:** G0–G2 are implemented on the high tier under issue #10565
(`verse_engine::render_graph::PhotoPlan`, `crates/verse/src/pbr/screen.rs`),
with contact shadows for the sun or stage key light only, since the physical
path has no other shadowed light.

### Later, when a scene needs it

| ID | Item | Prerequisite | Verdict |
| --- | --- | --- | --- |
| S4 | Capsule occlusion under characters from the sky ambient. | A2 | Pull in later |
| B3 | Box-projected local reflection captures for interiors. | B1 | Pull in later |
| L1 | Clustered forward light lists (texture-backed for WebGL2). | A chamber or zone that exceeds 32 lights | Pull in later |
| A5 | Analytic local fog volumes and point-light air-light for torch glow. | A3 | Pull in later |
| A6 | Painted or noise cloud layer in the sky pass, with a cloud-shadow texture on the sun. | A1 | Pull in later |
| P3 | Separate brighten and darken exposure speeds and a per-zone compensation curve. | P1 | Adapt |
| — | Screen-space reflections. | A zone with water or mirror floors | Reject for now |

### Order and dependencies

```
P1, T1 ──► every later item
A1 ──► A2 ──► B2, S4
A1 ──► A3
B1 ──► B2, B3
S1 ──► S2 (Ruins gains phases 1–4 once it moves)
T1 ──► G0 ──► G1, G2
```

P1 and T1 come first because every later item needs one output path to judge
and a tier to scale. A2, A3, and B1 give the largest change for Everglade,
and all three run on every platform. S1 matters most for long outdoor views.
The screen-space phase is the only one that some devices skip.

## Rejected

| Unreal feature | Why not |
| --- | --- |
| Lumen GI and reflections, in every tracing mode | Compute, distance fields or ray tracing, and temporal history; Unreal does not run it on its ES3.1 phone path. Verse's lights are static enough to bake (B1). |
| MegaLights | Stochastic ray-traced shadows with temporal denoising. Verse's light counts fit the forward budget. |
| Virtual shadow maps | GPU page management built for Nanite and compute. Cascades with caching (S1) cover Verse's view distances. |
| Mesh distance field shadows and AO | Per-mesh 3D atlases and compute culling; cascades and baked AO do the same job here. |
| Volumetric fog and volumetric clouds | Froxel grids and ray marching with temporal reconstruction; not viable on WebGL2 or phones. |
| UV lightmaps | Needs lightmap UVs on CC0 kit meshes and leaf cards; vertex bakes and probes suffice for low-poly scenes. |
| FFT bloom and full ACES output transforms | Desktop-only cost, and a hue shift that works against the stylized palettes. |
| Hardware ray tracing anywhere | No portable ray queries in wgpu on iOS, Android, or the web. |

## Documentation to update as items land

- `docs/verse/everglade.md`, "Rendering": the sky, ambient, fog, shadows, and
  bake.
- `crates/verse/src/pbr/mod.rs` and `pbr/gpu.rs` module docs: the pass order.
- `docs/verse/engine/roadmap.md`: VE-3 and VE-6 entries for the lighting
  items and quality tiers.
- This note: each candidate's status, issue, and landing commit.
