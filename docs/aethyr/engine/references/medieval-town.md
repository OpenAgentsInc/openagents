# Medieval Town rendering reference and parity plan

Status: reference images inspected and the first local renderer implemented,
2026-10-22. Camera calibration and rendering work continue; no shot has passed
the parity gates. See [implementation status](../implementation.md).

## Reference and scope

The supplied [Fab listing](https://www.fab.com/listings/7564ffa6-ab7b-4f1a-a551-0b7983cedc41)
is Hivemind's **Modular Medieval Town**. The listing credits environment artist
Stanislav Vovchuk and describes UE5 Lumen/Nanite use, day/night lighting, enterable
buildings, sky/cloud tools, light beams, and effects. Those are vendor statements;
they do not establish equivalent features in a converted mesh or browser engine.

All 18 displayed gallery images were downloaded and visually inspected at
1280 × 720, rather than judging the 160 × 90 thumbnails. They comprise **16 scenic
views, one promotional comparison, and one asset overview**. The
[source inventory](medieval-town-gallery.json) records gallery order, full displayed
image URLs, SHA-256 hashes, byte lengths, dimensions, and stable IDs `MT-01`–`MT-18`.
The [shot guide](medieval-town-shots.md) describes every image and its reconstruction.
These IDs identify the acquired images, even if Fab later reorders its gallery.

Only links, metadata, original observations, and plans enter this repository.
Downloaded JPEGs and inspection contact sheets remain in private scratch storage.
Vendor meshes, textures, source maps, and captures remain outside Git under the
existing [asset boundaries](../integration.md#import-and-rights). No purchased
archive was downloaded for this research.

## What the images establish

The target is a densely dressed, naturalistic fantasy town: weathered plaster,
timber, stone, orange roof tiles, vegetation, water, market objects, signs, and
multiple layers of architecture. Large clouds, pale mountain ridges, and a distant
castle are part of the composition. They are not interchangeable with an empty
blue background behind a few houses.

Daylight combines bright warm surfaces with crisp directional shadows and
readable shaded facades. Fine roof, railing, ivy, and leaf silhouettes matter.
Night combines cool blue environmental lighting with localized amber lanterns,
warm openings, dark foreground framing, and depth-dependent atmosphere. The
entrance and fountain day/night views make the difference especially clear.
A global orange or blue filter cannot produce those spatial light relationships.

Use three evidence labels throughout this effort:

- **Observed:** a visible shape, contrast, color relationship, or overlap in a
  specified image.
- **Source-confirmed:** a parameter recovered from the licensed project with its
  export revision, actor/material identity, and units recorded.
- **Reconstruction hypothesis:** our chosen camera, light, shader, or atmospheric
  implementation. Test it against the image and revise it when it fails.

The JPEGs do not uniquely reveal camera intrinsics, world coordinates, sun angles,
physical light intensities, exposure, texture resolution, light count, bake state,
or the exact GI/cloud/post-processing technique. Numerical starting values in a
future recipe must carry the third label unless source-confirmed. A still image
also cannot establish animation quality, frame rate, or temporal stability.

## Starting point in OpenAgents

The supplied listing identifies the purchased reference. Public OpenAgents
metadata names Modular Medieval Town but does not independently bind its export
to this Fab listing GUID. Preserve that distinction when recording asset identity.

| Evidence | Finding and implication |
| --- | --- |
| [Kit refactor notes](https://github.com/OpenAgentsInc/openagents/blob/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793/docs/verse/everglade-medieval-refactor.md) | The historical export contains 285 meshes, 347 textures, and 216 materials. Its documented source town has thousands of placements. Everglade recomposes kit pieces and simplifies materials; it is not an exact import of the gallery scene. |
| [Town Lab README](https://github.com/OpenAgentsInc/openagents/blob/179f88c0f9/apps/haunt-preview/README.md) and [kit metadata](https://github.com/OpenAgentsInc/openagents/blob/179f88c0f9/apps/haunt-preview/src/medieval-kit.json) | At this inspected prototype revision, the Lab exposes eight assets and two editable house recipes, with 18 base-color texture layers at 256 px. This proves a bounded creation experiment, not the whole source town or its rendering. It is not a claim about subsequent live Lab changes. |
| Private metadata inspection, 2026-10-22 | `export/maps/Maps__medieval_town.json` and `Maps__medieval_town_night_scenario.json` contain 8,096 and 8,102 placed records respectively. Historical documentation gives 8,082 under its earlier scope; do not combine these into one asserted count. |
| Private actor/material metadata and [exporter](https://github.com/OpenAgentsInc/openagents/blob/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793/scripts/unreal/medieval_town_ue.py#L426) | Mesh-component placements and selected Blueprint/GroupActor records retain transforms. Cameras, lights, fog, sky, and postprocess actors occur in the class census, but their individual transforms and look settings are not exported. Material records include texture and parameter references, but not the full Unreal master-material expression graph. Re-export those poses/settings or label visual calibration explicitly. |
| [Existing handoff](https://github.com/OpenAgentsInc/openagents/blob/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793/docs/verse/everglade-medieval-handoff.md) | Earlier exported-demo captures were rendered by Verse. They are not Unreal reference renders and cannot establish parity with the Fab gallery. Everglade's scene-bound baked lighting is not the vendor's lighting setup. |

The metadata research used OpenAgents checkout
`83fe8b7f6b3cd437ae9b7af213cf9f04875dd793` plus the named Lab history revision.
Private export presence does not prove freshness or complete asset coverage.
No independently captured Unreal reference render from the purchased project was
inspected. The Fab marketing JPEGs are the visual target. A later bounded metadata
recovery now supplies authored camera and light records from a private project
copy. The earlier normalized map export had removed distinct merged-mesh UAID
identities; recovering those paths restores all 8,096 day placements. Source
camera/gallery correspondence and rendered appearance still require comparison.

## Required rendering capabilities

These are Aethyrforge requirements to qualify, not a claim that reproducing
Lumen or Nanite internally is necessary. Match visible results using owned
interfaces and measured implementations. [Rendering](../rendering.md),
[game systems](../game-systems.md), and [asset compilation](../asset-compilation.md)
own the general contracts.

| Area | Required behavior | First useful reference |
| --- | --- | --- |
| Source/layout fidelity | Preserve instance transforms, UVs, tangents, material slots, detailed roofs, trim, street levels, props, decals, foliage, distant castle, and terrain. Record unsupported source features. | MT-04, MT-07, MT-02 |
| PBR materials | Correct base-color/normal/roughness/metalness/occlusion/emission roles, color spaces, tangent conventions, texture tiling, alpha cutouts, two-sided foliage, and material-instance tints. Retain detail with mipmaps and qualified filtering. | MT-07, MT-03, MT-18 |
| HDR and display | Linear-light shading, declared HDR intermediates, fixed comparison exposure/white balance, a versioned filmic display transform, and restrained bloom. Keep texture detail in bright plaster and dark doors. | MT-07/08 |
| Sun and shadows | Directional lighting with stable cascaded shadows, fine foliage/railing shadows, tuned bias, contact grounding, and a declared penumbra/filter profile. Test near/far and grazing angles. | MT-07, MT-02, MT-06 |
| Sky and environment | Authored sun/moon placement, sky radiance, cloud shapes/coverage, diffuse environment fill, and filtered specular environment response. Sky color and surface illumination agree. | MT-02, MT-11, MT-17 |
| Indirect light | Readable eaves, doorway recesses, and shaded streets, with plausible bounce and occlusion. Test environment/probe/lightmap approaches without painting permanent lighting into base color. AO supplements rather than substitutes for indirect light. | MT-03, MT-05, MT-07 |
| Practical lights | Emissive lantern/window surfaces plus separate local illumination, range/falloff, occlusion, shadow budgets, and clustered selection. Bright glass must not light an entire street uniformly. | MT-08, MT-13, MT-14 |
| Distance atmosphere | Height/distance-dependent aerial perspective, controlled local haze, and correct depth integration. Separate nearby dark silhouettes from pale remote castle/mountains. | MT-17, MT-02, MT-11 |
| Clouds and light shafts | Layered cloud composition first; qualified volumetric clouds or local beam volumes where a simpler model cannot match the image. Bound steps, history, and light interaction. A uniform screen veil fails. | MT-11, MT-12, MT-17 |
| Water and effects | Transparent fountain flow, layered basin water, reflections/highlights, bounded splashes, and foliage/particle motion. Add birds/bats/floating lights as seeded cosmetic effects after the lighting reads correctly. | MT-09/10, MT-03/14 |
| Fine edges and temporal quality | Stable roofs, railings, leaves, pennants, water, and particles under motion. Qualify antialiasing, alpha coverage, mip selection, velocity/history rejection, and transparency interactions. | MT-07, MT-16, MT-18 |

Point-light emission and material emission are separate inputs. Likewise sky
appearance, ambient fill, direct lighting, and fog are separately inspectable.
The reference profile cannot hide missing systems behind a single ambient term,
heavy bloom, extreme sharpening, crushed blacks, or painted screenshot backdrops.

### Fast editing and indirect-light validity

Keep the existing complete-generation contract for geometry, collision,
navigation, and selection. Any probe/lightmap bake declares its exact source,
light/environment/sky, material, geometry, and bake-settings dependency hashes.
A change to any affected input invalidates the bake; stale lighting cannot remain
silently attached to a new scene configuration.

Use a declared dynamic-preview lighting profile while optional indirect work
rebuilds, or hold activation if the selected profile requires that work. The UI
shows the profile/preparation state. A final parity capture requires complete,
current lighting inputs and cannot pass while a fallback is hidden. This gives
routine live editing a fast path without claiming a heavy GI bake is instant.

## Shot families and first proof

| Family | References | What it isolates |
| --- | --- | --- |
| Entrance day/night | MT-07, MT-08 | Material detail, sun/leaf shadows, practical-light falloff, exposure |
| Square day/night | MT-02, MT-11 | Complete composition, castle/sky/depth, large-scale lighting hierarchy |
| Fountain detail day/night | MT-09, MT-10 | Water, foliage, practical/environment balance, near-surface shading |
| Courtyard and street | MT-03, MT-05, MT-06, MT-13, MT-14, MT-15, MT-16 | Architecture density, indirect fill, trees, local-light continuity |
| Facade/skyline | MT-12, MT-17, MT-18 | Silhouette fidelity, camera perspective, distant atmosphere |
| Overview/comparison | MT-04, MT-01 | Asset coverage and explicit paired presentation; not additional cinematic views |

Start with **MT-07 then MT-08**, using a small entrance scene to separate material,
shadow, and night-light problems. Next build **MT-02 and MT-11** in the full square.
The doorway pair proves the shading pipeline; the square pair prevents a close-up
success from being mistaken for a complete town environment. Day/night pairs
may use related layouts without having pixel-identical cameras; match each image
independently before asserting a shared camera.

## Phased effort

| Gate | Work | Exit evidence |
| --- | --- | --- |
| REF-0, recover source and calibrate | Verify purchased scene revision/rights; map asset identities; re-export camera/light/environment transforms and settings, including sky/fog/exposure, plus material and VFX data where available; build the private gallery/camera index. | Source-confirmed fields distinguished from inferred values; missing assets/features listed; color/scale/material calibration fixture renders correctly. |
| REF-1, entrance day/night | Import the entrance faithfully; implement PBR/HDR, environment fill, directional/contact shadows, and warm local lights. Match MT-07/08. | Frozen camera, comparison crops, grayscale/color and material review; no uniform color-filter substitute. |
| REF-2, full square and sky | Recover scene placement, street levels, castle/mountain layers, sky/cloud composition, atmosphere, and indirect-light strategy. Match MT-02/11. | Composition/depth pass, grounded shadows, readable shade, correct warm/cool hierarchy, runtime/memory measurements. |
| REF-3, water and remaining scenic shots | Match MT-03, MT-05/06, MT-09/10, MT-12–18; qualify water, foliage, local haze, fine-edge stability, and bounded cosmetic effects. | All 16 scenic views have recipes and accepted stills plus moving-camera checks; unsupported effects remain explicit. |
| REF-4, catalogue and comparison | Build the asset-overview scene for MT-04 and a paired day/night presentation for MT-01. | Layout/asset coverage report and reproducible comparison; marketing typography is excluded from renderer scoring. |
| REF-5, creation-engine qualification | Walk, change shaders/lights, add/remove a wall, undo, save/reopen, publish, and reload exact reference recipes. | Coherent generations, correct bake invalidation, preserved compatible state, independent frame loop, and declared performance profile pass. |

These gates refine M0–M3 renderer/content work and M7 qualification in
[delivery](../delivery.md). Public engine fixtures use original/redistributable equivalents;
the purchased scene is a separately authorized private comparison workload.

## Reproducible shot recipe

Before implementing a capture tool, specify a versioned recipe with:

- Reference ID, acquired image hash, gallery URL, image framing, and any explicitly
  excluded typography or presentation border.
- Project/source-map revision, compiled generation, asset/material/kit digests,
  engine/compiler builds, and required quality profile.
- Source camera actor when known; position, orientation, projection, explicit
  vertical/horizontal FOV convention, aspect, near/far planes, and optional lens,
  filmback, focus/aperture. Every recovered/inferred field is labeled.
- Named day/night environment, direct and practical lights, sky/cloud/fog settings,
  indirect-light inputs, shadows, exposure, white balance, display transform,
  bloom, antialiasing, texture/LOD, and any permitted quality reductions.
- Frozen simulation tick, particle/water/wind seed, animation time, warm-up and
  temporal-history reset policy. A static match must not depend on an accidental
  previous frame or an unrecorded random state.
- Exact 1280 × 720 output, scale 1, color space, alpha/background, browser/OS/GPU,
  viewport/pixel ratio, capture/readback method, timings, residency, and diagnostics.

Use the same fixed recipe for comparison and a declared adjacent-camera path for
motion checks. Keep editor chrome and adaptive exposure/resolution out of the
reference capture. Debug views include base color, normals, roughness/metalness,
direct/indirect contributions, shadow cascades, depth, fog transmittance, and HDR
luminance before display conversion. These are essential for finding the cause
of a mismatch rather than repeatedly adjusting every setting.

## Acceptance and reporting

1. **Composition:** Match foreground occlusion, roof/castle/window landmarks,
   horizon, street levels, object scale, and empty space. Use normalized screen
   coordinates; a proposed initial landmark tolerance is 2% of image diagonal.
   Confirm the tolerance with reviewers; it is not a measured result.
2. **Light and depth:** Review grayscale alongside color. Check shadow direction,
   footprint and contact, shade readability, bright-wall texture, warm/cool light
   pools, fog depth, and sky placement with the shot's pass/fail criteria.
3. **Surface fidelity:** Compare fixed 1:1 crops of plaster/timber, ivy/railings,
   roofs, stone, metal and water. A matching silhouette with flat base color does
   not pass. Freeze crop masks and output handling before comparing revisions.
4. **Metrics:** Record landmark error, luminance/color statistics on fixed regions,
   image-difference/SSIM where useful, and reviewers' named discrepancies. Set
   thresholds on a calibrated baseline; no single global score proves parity.
   Compare within one declared display transform; JPEG values are not HDR radiance.
5. **Motion and editing:** Inspect camera moves, near/far transitions, foliage,
   water, particles, shadow stability, and temporal artifacts. Edit a light and
   structural wall, then check current bake identity, generation consistency,
   player state and working-shader recovery.
6. **Cost:** Record cold/warm bytes, CPU/GPU pass cost where available, frame
   percentiles, and peak CPU/GPU residency on pinned hardware. Report still quality
   and interactive performance separately under [performance](../performance.md).

Each shot starts **reference inspected**. Later statuses are **recipe ready**,
**scene reconstructed**, **still accepted**, and **interactive qualified**, each
with evidence. MT-01/04 use presentation/coverage gates rather than the scenic
motion claim. Publish a discrepancy list for every attempted match; do not rename
an approximation as parity or claim zero image error against an uninspected image.

The first implementation deliverable is the paired entrance calibration, followed
by the full day/night square with its sky, distant scenery, and lighting.
