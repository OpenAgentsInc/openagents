# Coax Metaverse Engine lighting implementation plan

Date: 2026-11-08. Source baseline: `7fab57cd75`. Revised 2026-11-08 (owner
direction: the Character Create glade is the look).
Status: Phase 1 qualified for the browser (#13067; receipt
`docs/verification/aethyr-lighting-p1-2026-11-08/`). Done: step 0, the glade lifted into the shared
painterly default (#13060), step 1, the baseline (#13061), step 2, the records (#13062), step 3, the
regions (#13063), step 4, day and weather (#13064), step 5, Everglade II's environment set and compatibility profiles (#13065), step 6, the realms
routed through the resolved environment (#13066), and step 7, the ordinary default, the light views
and the exit (#13067). Primitives: done. Everglade adoption: done. Browser qualification: done.
Native qualification: not qualified (no native consumer reads the resolved record yet). Phase 2 has
not started.

Build one shared lighting system for Coax the OpenAgents Metaverse Engine, with Everglade as its first
complete production consumer. It must support authored environmental character,
readable interiors, moving characters, many local lights, stable shadows, layered
atmosphere, water and effects, and controlled final color. Shared code that the
playable realm does not consume is an unfinished phase.

## The reference look: Character Create's glade

The owner's direction (2026-11-08): the Character Create glade is the most
beautiful thing we render, and its look extends through **every painterly
realm** as the **default** lighting. A painterly realm that authors nothing gets
the glade; authored profiles are deltas from it, never a fresh start.

"Everglade" in this plan means **Everglade II** (`/worlds/everglade2`,
`everglade/client/`, `everglade/world/`), which is becoming
Everglade. Version one (`apps/openagents-web/gpu/everglade-v1/gpu.coax`) is frozen: it gets the
painterly default through a compatibility adapter at the end, not new work.

What the glade is today, and what becomes the painterly default:

| Glade element | Where it lives now | Default it becomes |
| --- | --- | --- |
| Linear HDR, decode once, encode once | `everglade/scene/gpu/character-glade-light.coax` (`cg_linear`, `cg_srgb`, `cg_decode`, `cg_encode`) | The one transfer contract for painterly realms (Phase 2) |
| Key light: 4500 K sun under 6500 K white balance, x2.15, fixed direction | `cg_sun_colour`, `cg_sun_dir` | The painterly noon key in the default profile |
| Warm ground / cool sky hemisphere ambient | `cg_hemisphere`, `cg_irradiance` (indirect occlusion and sun visibility on separate terms) | Default upper/lower ambient and the direct/indirect split |
| Height air with a clear 30 m near field, forward-scatter haze to 65 m | `cg_air_depth`, `cg_phase` (g 0.62), `cg_haze_window`, `cg_atmosphere` | Default analytic atmosphere (Phase 6 starts here) |
| Cyan daylight sky, drifting cloud shadows (sun x0.45 in shade) | `everglade/scene/gpu/character-glade.coax` (`cg_sky`, `cg_cloud`) | Default sky and cloud-shadow layer |
| Film response at exposure 1.18, saturation lift 1.16 | `cg_film`, `cg_output` over `metaverse-engine/render/shade/filmic.coax` | Default painterly presentation (Phase 9's identity grade) |
| Octahedral normals with room for an indirect-light weight; normal/depth contact shading | `cg_normal_pack`, the glade's contact pass | Default contact/AO path (Phase 5) |
| Painterly bands, rim, dab, cool shadow band | `metaverse/style-preset.coax` (`painterly_key`, `shadow_cool` 1.5) | Kept as the painterly material response; the glade defaults sit under it |

Since #13060 these functions and numbers live in the Metaverse Engine: the numbers once, as
`metaverse-engine/world/environment.coax`'s `ENV_GLADE_*` (`env_glade_profile` is built from them), their
evaluation in `metaverse-engine/render/shade/painterly.coax`, the transfer helpers in `metaverse-engine/render/transfer.coax`,
the normal packing and contact taps in `metaverse-engine/render/shade/contact-shade.coax`, and the exposed film
response in `metaverse-engine/render/shade/filmic.coax`. The `cg_*` names in the app files are thin delegates, and
`metaverse_style_preset(STYLE_PAINTERLY, ...)` names the glade profile as its default environment.

Since #13065, `metaverse_style_compat_profile` turns a preset's four clock keys into a profile:
painterly's noon knot is the glade's (it resolves bit-equal to `env_glade_profile()`), and its
dawn, dusk and night are deltas on the glade; lowpoly keeps its own. Everglade II's places are in
`everglade/world/environment.coax` (`e2_environment_set()`): the open glade as the map default,
a subtle sunlit-path delta and a deeper, cooler canopy delta, as prisms along the path. Town,
interior and river deltas wait until Everglade II has those places. Receipts:
`docs/verification/aethyr-lighting-p1-5-2026-11-08/`.

Everglade II already reads part of this (`everglade/client/gpu/everglade2.coax`
calls `cg_decode`, `cg_hemisphere`, `cg_sun_dir`, `cg_sky`, `cg_output` and
more), and Experimental/fivelab loads it too. It is still private: the numbers
are constants in an app file, not a shared profile a realm can select or vary.

Rules for the default:

- **Character Create is the identity test.** After every phase, Character
  Create's glade must render the same as before, within a declared image and
  probe bound. If it changes, it's a regression, unless the owner approved
  the new look.
- **One default, many consumers.** The glade's values live once in a shared
  painterly default profile. Character Create, Everglade II, Experimental,
  `oneshot-glade`, made realms (`myrealm`, the Metaverse Engine's painterly default) and
  the v1 Everglade adapter all read it. No copy of the numbers in a realm.
- **Other times and places derive from it.** Dusk, night, deep canopy, interiors
  and river mist are authored as bounded deltas on the glade profile in the same
  style. They keep its film response, saturation and painterly bands, so the
  whole world still reads as the glade.
- **Non-painterly styles** (lowpoly, flat, anime, photoreal) keep their own
  presets. They may adopt shared mechanics (lights, shadows, fog), but they do
  not inherit the glade's colours.

## Starting point

The existing system supplies useful parts that should be composed and migrated:

| Existing owner | Current capability | Boundary to close |
| --- | --- | --- |
| `metaverse-engine/render/shade/light.coax` | Key direction, two-color hemisphere ambient, snapped shadow space, haze and a bounded 16-light CPU list | Realms still construct lighting and shadow space; the CPU list is not the live realm light path |
| `metaverse/style-preset.coax` | Four time keys, sun/moon choice, style uniforms and clock modes | No general spatial environment profile resolver or irregular time tracks |
| `metaverse-engine/render/shade/sky.coax`, `vegetation.coax`, `shadow-filter.coax`, `post.coax` | Shared sky/cloud, vegetation, PCF offsets, tone, bloom and shaft arithmetic | Pass ownership and several material/color contracts remain consumer-specific |
| `apps/openagents-web/gpu/everglade-v1/gpu.coax` | Real geometry shadows, baked sky visibility, lamp/spell lighting, room warmth, weather, water and post | One key shadow map; at most 16 lamp entries evaluated directly; lighting remains embedded in the realm shader |
| `metaverse-engine/render/shade/scene-lights.coax`, `character.coax` | Detailed characters use world lighting, shadows and post; up to eight selected point lights | Character and world light admission/evaluation are different paths |
| Character Create's private glade renderer | Explicit linear HDR, separated direct/indirect terms, height atmosphere and normal/depth contact shading | Private implementation and qualification, not a shared realm capability |
| Shared render package, graph and resource ledger | Typed resources, bindings, passes, ownership and recovery contracts | New lighting resources and passes must enter these contracts explicitly |

Existing water includes dedicated reflection, absorption, scattering, foam and
underwater behavior. Existing character shaders include specialized skin, eyes
and hair. Preserve those capabilities; a new common record must not flatten all
materials into one diffuse shader. The separate C++ engine is reference code,
not evidence that a capability is implemented or qualified in Coax.

## Ownership and contracts

The following are proposed module boundaries. Extend an existing owner where
appropriate rather than introducing parallel implementations.

| Proposed shared owner | Responsibility |
| --- | --- |
| `metaverse-engine/world/environment.coax` | Typed profiles, time tracks, regions, conditions, deterministic selection and blending |
| `metaverse-engine/world/day.coax`, `weather.coax` | Adapters over simulation time and existing weather arithmetic; no second simulation clock |
| `metaverse-engine/render/shade/environment.coax` | Resolved environment records and CPU/GPU packing; spatial samples and camera presentation inputs |
| `metaverse-engine/render/shade/material.coax` | Surface inputs, authored illumination/occlusion semantics and material response |
| `metaverse-engine/render/shade/scene-lights.coax` | Shared scene light records and adapters, evolving the existing character contract |
| `metaverse-engine/render/light-grid.coax`, `shadow.coax`, `occlusion.coax` | Light selection, shadow planning and indirect visibility |
| `metaverse-engine/render/fog.coax`, `fog-volume.coax` | Analytic atmosphere, volume injection, scattering, integration and history |
| Existing `sky.coax`, `post.coax` and shared water hooks | Sky appearance, grading/presentation and consistent water integration |
| `everglade/world/`, and v1's `everglade/legacy/v1/` | Authored profiles, placements and gameplay light/effect sources; consumes shared evaluation |

Coax owns product arithmetic, scheduling decisions, admission and all new tools.
Hosts execute platform GPU operations and report measured capabilities. Required
compiler operations, storage textures and shader stages must be demonstrated
through Coax emission before a phase depends on them. No handwritten shader or
host-language lighting implementation may fill a compiler gap.

Preserve the published draw ABI, immutable package identities, resource ledger,
generation/epoch ownership and bounded frame admission. New bindings, layouts or
asset records receive explicit versions and consumers. Protocol changes, if any
are necessary, start in the schema and specification with a new version. Rendering
adoption must not change gameplay weather, room authority, combat or simulation.

### Common numerical rules

- World positions use engine coordinates and metres. Clock inputs come from the
  simulation tick and an explicit epoch/mode; deterministic captures pin them.
  A multiplayer world never silently uses a visitor's local wall clock.
- Scene radiance and lighting colors are linear. Texture formats declare their
  transfer function; palette inputs decode once. Alpha, masks, normals and depth
  are data. HDR intermediates remain linear through lighting and composition.
- Base color, authored illumination, ambient occlusion, emission and direct
  visibility are distinct inputs. Emission alone never creates an illuminating
  light. Occlusion primarily attenuates indirect terms, with explicit exceptions
  documented for individual materials.
- Environment evaluation separates sun/moon direction, direct color, upper/
  horizon/lower ambient, sky, fog, water and presentation. A sun-colored sky does
  not imply every one of those channels shares the same value.
- Resource declarations fix capacities and costs. Malformed or required missing
  data refuses atomically. Capacity pressure follows a declared deterministic
  selection rule with telemetry; it never silently truncates an upload.
- GPU history belongs to a camera, environment/package identity and device
  epoch. Replacement, cuts and invalid input cannot inherit stale history.

One frame owns a resolved environment context, not one universal value for every
point. Surfaces and characters can occupy different regions in the same view.
Camera grading/sky selection and spatial surface/fog evaluation have explicit,
separate rules. Store bounded region candidates rather than recomputing an
unbounded world search per pixel.

## Phase 1 — The glade as the shared painterly default profile

**Outcome:** the glade's lighting lives in one shared painterly default profile.
Character Create, Everglade II and Experimental read it through a shared
environment resolver for key, ambient, sky, haze, cloud shadow and presentation.
Character Create looks the same. This is the first delivery.

0. Lift the glade first. Move `character-glade-light.coax`'s and
   `character-glade.coax`'s lighting numbers and functions into shared Metaverse Engine
   owners: the profile in `metaverse-engine/world/environment.coax`, the evaluation in
   `metaverse-engine/render/shade/environment.coax`, and the presentation in `post.coax` or
   `filmic.coax`. Then make `metaverse/style-preset.coax`'s painterly preset point at
   that profile as its default. The `cg_*` names may stay as thin delegates while
   consumers move. Character Create's pinned captures must match before and
   after.
1. Before changing any render path, record pinned frames and numerical probes:
   - Character Create: the glade, a character close-up, and the camera turn.
   - Everglade II: the glade arrival, the open path in sun, and the deep canopy
     path.
   - Experimental.

   Record the active producer, package, compiler, raster, device and settings,
   so later comparisons have a concrete baseline.
2. Define versioned `EnvironmentProfile`, `EnvironmentTrack`,
   `EnvironmentRegion`, `EnvironmentState` and `ResolvedEnvironment` records.
   Profiles own bounded, irregular time knots, including one-knot constants;
   cyclic wrap, duplicate-time rejection and interpolation are specified.
   Colors interpolate in linear space; directions use a defined normalized
   interpolation with an antipodal rule. Identifiers and flags never interpolate
   numerically. Resource transitions use explicit bounded blend references.
3. Add map defaults and sphere/box/polygon-with-height regions, inner/outer
   transition bands, priorities and stable-ID tie breaking. Specify overlapping
   region normalization, fallback outside all regions and continuity at edges.
   Bound candidate lookup and work independently of the total authored world.
4. Adapt the existing day and weather functions. Keep fixed/cycle/real modes
   explicit, preserve Everglade's current schedule and wetness, and distinguish
   global weather from local visual overlays. Evaluate versioned condition
   inputs from known game state; the lighting resolver does not invent gameplay
   policy. Cache only under complete tick/state/profile identities.
5. Convert the existing four-key style preset to a compatibility profile.
   Painterly's compatibility profile is the glade default. Add Everglade II
   regions as glade deltas: the open glade, the sunlit path and the deep canopy.
   Add town, interior and river deltas when Everglade II has those places. Keep style response separate from the environment so a palette
   change does not switch the material model. Author content from Everglade's
   actual geometry and routes rather than relying on placeholder regions.
6. Route Everglade II's uniform writer, sky, terrain/props, ordinary avatars,
   detailed character `SceneLights`, river and effects through the resolved
   context. Remove their independent key/ambient/fog calculations from the
   enabled path. Compatibility exports may delegate to shared evaluation.
7. Make this the ordinary Everglade default once its gates pass. A temporary
   diagnostic comparison switch is allowed; an opt-in experiment alone does
   not finish the phase. Record remaining non-Everglade consumers separately.

**Exit:** Character Create's glade is unchanged within its bound, and it now
reads the shared default. Everglade II and Experimental read the same default,
and walking from Character Create into Everglade II shows no lighting jump at the
hand-over. A made painterly realm with no lighting authored gets the glade. Check-first tests cover midnight wrap, irregular knots, invalid/nonfinite
fields, overlaps, condition changes and bounded refusal. Fixed-state replay and
late-join evaluation agree. Actual browser readback shows world and character
receive the expected resolved values. Walking a pinned route through forest,
town, a doorway and river shows continuous transitions. The compatibility profile
meets a declared numerical/image bound; authored profile changes have separately
reviewed images. A malformed replacement cannot partially replace the active set.

**Remaining non-Everglade consumers** (recorded by #13067 for their own adoption; none reads the
resolved record yet, and Phase 1 does not change them):

| Consumer | Where its light comes from now | Adoption |
| --- | --- | --- |
| Everglade v1 (`/worlds/everglade`, `apps/openagents-web/everglade.coax`, `apps/openagents-web/gpu/everglade-v1/gpu.coax`, `gpu-glade.coax`, `glade-zone.coax`) | its own frozen key, ambient, weather and lamp path (`everglade.wgsl`, 633 vec4s) | an adapter in a later phase; v1 stays frozen |
| Everglade 5E (`everglade5e.coax`) | Everglade v1's shader (`everglade5e.wgsl`) | with v1's adapter |
| The fixture (`fixture.coax`, `gpu/fixture.coax`) | `metaverse/style.coax` presets at its own clock (`fixture.wgsl`, 71 vec4s) | the style-preset compatibility profile, once its shader reads the record |
| One-shot's Table Display glade (`oneshot.coax`, `oneshot-table.coax`) | its own shader constants (`oneshot.wgsl`, 442 vec4s) | the painterly default, as 5E Lab |
| Made realms that pick low poly or flat, or author a time of day or mist (`myrealm.coax`) | `metaverse_style_at` at the spec's time | a made realm's authored profile (painterly with nothing authored already gets the glade, #13066) |
| The Metaverse Engine examples (`metaverse-engine/examples/light-a-frame.coax`, `shade-a-material.coax`) and `metaverse/experimental-terrain.coax` | their own constants or the style presets | book chapters: they read the record when the light chapter does |

## Phase 2 — Shared surface inputs and a consistent linear pipeline

**Outcome:** lighting inputs have the same meaning across Everglade materials.

- Extend material/asset records for independent albedo, vertex illumination,
  indirect occlusion, normals, specular controls, transmission and emission.
  Specify neutral defaults and whether authored illumination is additive or a
  modulation. Do not multiply an already shaded color by lighting again.
- Preserve original vertex colors and their provenance during packing; expose
  authoring controls for colored room gradients and terrain modulation. Add
  explicit per-material selection so albedo tint cannot be mistaken for a baked
  illumination channel. Version derived packs when layouts change.
- Adopt one transfer-function contract in world, ordinary/detailed characters,
  foliage, water, particles and post. Reuse the private glade's validated ideas
  through shared owners, with fresh adoption checks. Remove duplicate gamma
  conversions only after verifying each source texture format.
- Share lighting terms while retaining painterly bands, smooth diffuse/specular,
  thin-leaf transmission and specialized character response. Upper, horizon and
  ground ambient colors remain independently authored. Allow material-specific
  specular exponents/strength and environment response.
- Keep HUD/display-authored overlays after scene presentation. Define a single
  scene output transform and explicit exceptions for already-authored skies.

**Everglade exit:** Everglade II's ground, trees, grass and characters decode once
and encode once, through the glade's transfer contract. No realm-private gamma
stays in the enabled path. When Everglade II gains a room, it has authored
colored illumination without a fake emissive wall, and its doorway transitions
continuously. Gray, saturated fabric,
skin, leaf and water probes retain expected linear values, including radiance
above one. World and character react to the same environment changes. Direct,
indirect, emission and authored-illumination debug views isolate the terms.

## Phase 3 — Shared local lights and scalable assignment

**Outcome:** lamps, spells and character attachments use one light inventory.

- Define stable light IDs, point/spot type, transform/bone attachment, linear
  color/intensity, attenuation start/end, optional distance color transition,
  cone angles, animation clock and shadow intent. Finite cutoff is authored;
  inverse-square behavior is an optional model, never an implicit replacement.
- Migrate Everglade's lamps and spell blasts into the shared inventory. Preserve
  current gameplay timings and daytime spell visibility. Evaluate animated
  attachment transforms before light admission; a particle sprite is not a light
  unless its source explicitly supplies one.
- Build bounded view/depth clusters and per-cluster index lists. Start with a
  checked CPU assignment oracle; choose CPU or GPU production construction from
  measured costs and compiler support. Share record layout and attenuation
  between surfaces, characters, water and the future fog path.
- Specify deterministic ranking, visibility/range filtering, overflow handling
  and fade hysteresis. Report candidates, admitted lights, dropped lights and
  maximum list occupancy. Total world lights, frame lights and per-cluster caps
  are different budgets. Replace the separate 16-world/eight-character selection
  paths only when actual consumers read the new lists.

**Everglade exit:** Everglade II's class-bar casts with several players, under a
dusk delta of the glade profile, form a dense spell workload. That workload lights the
world and characters consistently, including spots and one animated attachment.
Cluster output matches an independent all-lights reference on bounded scenes;
near/far boundaries and overflow are checked. Record CPU setup, GPU cost and
memory at the exact qualified rasters. Increasing inventory must not make every
fragment loop over all world lights.

## Phase 4 — Shared shadow passes, cascades and selected local shadows

**Outcome:** near detail and distant shadow coverage coexist without per-realm
shadow implementations.

- Own caster planning, stable cascade fitting, texel snapping, split transitions,
  bias, PCF filtering and depth resource declarations in shared Coax. Separate
  quality policy from math; retain a cheap bounded profile.
- Admit offscreen casters whose shadows reach visible receivers. Preserve
  skinning, vegetation motion, alpha cuts, LOD and fractional groom coverage in
  depth passes. Depth and color passes use the same admitted pose/geometry.
- Add a bounded shadow atlas for selected spotlights, with stable slot ownership,
  explicit ranking/fading and no allocation until needed. Point-light shadows
  follow only with a declared six-face cost and measured budget.
- Cache static caster work only under a complete identity of projection,
  geometry, material coverage and world edits. Moving geometry and changed
  buildings invalidate affected caches. Integrate loss/replacement cleanup.

**Everglade exit:** a walking camera, moving hero, canopy, modified building and
shadowed spotlight pass image and motion checks. Cascades blend without visible
coverage holes; static caches match uncached output within the declared bound.
Telemetry exposes map sizes, selected lights, caster counts and cache updates.

## Phase 5 — Shared indirect visibility and interior transitions

**Outcome:** objects sit in the world, and interiors retain authored atmosphere.

- Generalize the glade's normal/depth contact shading into a shared quality-scaled AO path:
  reconstruction, sampling, edge-aware filtering and indirect-only application.
  Keep baked sky visibility and authored AO as separate inputs with documented
  combination; avoid multiplying several equivalent occlusion estimates blindly.
- Add room/volume environmental classification and portal/doorway transition
  authoring. A closed room does not receive the full outdoor ambient merely
  because its sun contribution is shadowed. Preserve authored room illumination.
- Distinguish visibility culling, direct shadowing and ambient occlusion in data,
  passes and debug views. No one scalar is relabeled as all three.
- Provide optional bounded static ambient probes or authored directional ambient
  fields where room gradients need more than vertex illumination. These remain
  authored indirect light, not a claim of real-time multi-bounce GI.

**Everglade exit:** the glade's contact look carries onto Everglade II's path.
Forest roots, a moving character, and later a courtyard and a furnished room
when Everglade II has them, show contact without halos, double-darkening or darkened emissive
sprites. Doorway crossings preserve interior/exterior continuity. Direct-light
probes remain unchanged when only indirect occlusion is varied.

## Phase 6 — Layered analytic atmosphere and local fog volumes

**Outcome:** atmosphere varies by distance, height and place before temporal
volumetrics are introduced.

- Share distance/height density integration, sun-facing tint, independent layer
  colors, start/cap controls and bounded profile blending. The glade's height
  air, 30 m clear near field and g 0.62 forward lobe are the default layer.
  Preserve cheaper tiers.
- Add positioned/oriented local volumes with bounded shapes, falloffs, density,
  scattering color, emission, noise and animation. Keep volume placement distinct
  from broad environment selection and gameplay weather overlays.
- Define front-to-back composition of overlapping layers and analytical limits
  for zero density, large distance and camera-inside-volume cases. A local visual
  fog edit cannot change rain history or water simulation.

**Everglade exit:** low forest haze under Everglade II's canopy and the glade's clear
near field coexist, plus river mist and a room volume once those places exist. Pinned height/distance probes match integration references.
Terrain, characters, water and effects agree on atmosphere at shared positions.

## Phase 7 — Volumetric scattering and temporal integration

**Outcome:** local and directional lights illuminate a spatial atmosphere.

- Implement a bounded camera froxel grid with explicit depth mapping and quality
  extents. Inject global/local density, scattering and emission using Phase 6's
  records. Demonstrate required Coax compute and texture operations first.
- Reuse the shared clustered light inventory for local scattering; include
  directional visibility under an explicit cost budget. Specify the phase
  function, normalization, extinction units and integration behavior near zero.
- Integrate front to back as scattering plus transmittance. Declare whether each
  stored channel is opacity or transmittance. Opaque composition is
  `surface * transmittance + scattering`; other blend modes have explicit rules.
- Add reprojection, bounded history weighting and rejection for cuts, resize,
  disocclusion, region/light changes, package replacement and device loss. Noise
  and history use recorded seeds/frame inputs. Static pinning alone does not
  qualify a moving camera.
- Keep analytic-only quality profiles explicit. Their output is a documented
  reduced feature tier, with no fabricated volumetric qualification.

**Everglade exit:** a lantern in river mist, a spotlight cone and canopy light
shafts respond to lights and occluders. Motion captures cover cuts, fast turns,
moving lights and volume boundaries without persistent trails. Verify optical
depth/scattering numerically and resource cleanup through loss/recovery.

## Phase 8 — Sky, water, transparency and effects integration

**Outcome:** every visible material participates in the same environment.

- Drive authored sky geometry/textures, gradient layers, sun/moon, clouds and
  stars from resolved profiles. Specify sky depth, fogging and output-transform
  behavior, plus bounded blending when sky assets change.
- Feed water the resolved sun/ambient, selected lights, shadows and atmosphere.
  Preserve Fresnel, absorption, in-scatter, wave normals, foam, glints, caustics,
  wakes and underwater transitions. Declare reflection/refraction sources and
  formats; add scene-color refraction only with a bounded graph/resource owner.
- Define fog, grading and shadow behavior per opaque, cutout, alpha, additive,
  modulated, emissive and water material class. Transparent geometry integrates
  the volume up to its own depth without applying foreground scattering twice.
  Depth ordering and two-part water composition remain explicit contracts.
- Spell emitters and weather particles consume shared time/environment inputs
  while keeping emission and illuminating lights separate.

**Everglade exit:** riverbanks, swimming, waterfalls, rain, smoke and overlapping
spells pass controlled day/night/storm and camera-motion scenes. Water and
transparent effects neither erase fog nor receive it twice. Existing water
simulation and spell gameplay checks remain unchanged.

## Phase 9 — Authored grading, glow and presentation

**Outcome:** final color is authored per environment without hiding lighting
errors in arbitrary postprocessing.

- The glade's film response (exposure 1.18, saturation 1.16) is the painterly
  identity presentation. Every LUT and grade below is measured against it.
- Add versioned 3D LUT assets, identity grading, interpolation and bounded
  profile/state blends. Declare each LUT's input domain, range and transfer
  function, including HDR handling and its place relative to exposure/tone.
  Support channel-separable curves as an optimization only after proving
  equivalence for that asset. Handle missing identity data explicitly.
- Define camera presentation transitions independently from spatial surface
  lighting. Grade once at the declared stage; material-specific ramps are a
  separate operation. Preserve saturation and highlight hue deliberately.
- Consolidate exposure, tone shoulder, glow/bloom and screen-space shafts into
  shared graph profiles with explicit extraction/composition equations and
  quality settings. Keep volumetric scattering distinct from radial shafts.
- Make situational effects, underwater treatment and camera fades ordered,
  bounded records. Do not introduce automatic exposure without its own authored
  policy, temporal controls and acceptance scenes.

**Everglade exit:** identity LUT leaves the intended output unchanged; known
RGB ramps and cross-channel LUTs match references. Day/night and room transitions
blend continuously. Highlights, saturated spells, skin and HUD remain controlled
at desktop and phone settings. Diagnostic lighting views bypass presentation.

## Phase 10 — Complete host adoption, budgets and removal of duplicate paths

**Outcome:** the ordinary Everglade lighting path is shared, measured and
maintainable across its supported hosts.

- Browser WebGPU adoption happens during every preceding phase, not here at the
  end. Qualify the same source/package on Vulkan and the actual Metal adapter as
  each supports the required graph. Record native gaps explicitly; browser
  success cannot qualify them or change the desktop default by implication.
- Freeze quality profiles for phone, ordinary desktop and higher quality.
  Declare light/list capacities, shadow coverage/resolution, AO work, volume
  dimensions/history, LUT residency and transient/retained bytes. Choose exact
  limits from measured target devices before enabling each costly feature.
- Measure pinned full Everglade workloads with ordinary and detailed characters,
  dense lamps, spells, weather, foliage, water and interiors together. Record
  actual raster, CPU/GPU timings, percentiles, peak memory and quality settings;
  a missing hero or reduced raster is not a performance pass.
- Exercise resize, suspension, profile/pack reload, device loss and recovery.
  Every resource retires through the existing ownership model; history, atlas
  slots and cached uploads cannot cross replacement identities accidentally.
- Remove duplicated enabled lighting calculations and temporary rollout switches
  after equivalence/adoption evidence. Migrate other realms with their own
  receipts; private renderer improvements do not silently qualify the catalog.
- Update renderer documentation, chapter 13, chapter 71 and implementation status
  to reflect actual adopted behavior. Preserve historical receipts as historical.

**Exit:** browser plus each claimed native host has a complete current-source
Everglade receipt. Unsupported features choose an admitted quality graph before
submission, with a visible diagnostic in developer tools. Every ordinary draw
uses the shared contracts; no legacy light cap, color conversion or realm-private
shadow planner remains hidden in that path.

## Verification and delivery rules

Each phase lands as reviewable component and adoption changes. Component checks
may precede integration, but a phase stays open until its Everglade exit passes.
Do not defer all realm wiring, asset authoring or performance work to Phase 10.

1. Write `#test`/`#forall` checks before implementation. Admission, budgets and
   selection policy use `#authority;` where applicable and zero-survivor mutation
   gates. Keep arithmetic checks distinct from GPU integration claims.
2. Build independent small-scene references for profile blending, attenuation,
   cluster membership, shadow coverage, AO and fog integration. Compare actual
   shader values with declared tolerances; never infer a runtime feature from
   the presence of a source module or a shader name.
3. Keep a fixed capture suite:
   - Character Create's glade, close-up and camera turn. This is the identity
     reference: it must not drift.
   - Everglade II's arrival, open sunlit path, dense canopy, glade-to-path
     hand-over, dusk delta, crowded casts and moving hero.
   - Town at dusk, a lit room and its doorway, river mist and underwater, added
     as Everglade II gains those places.
   Include clocks around midnight and state/region boundaries. Pair stills with
   deterministic moving-camera/light sequences. Review complete frames as well
   as numeric probes; image hashes alone cannot judge intended appearance.
4. Run muted headless browser checks only. Add lighting controls and debug views
   to the existing developer surface: region IDs/weights, linear albedo,
   authored illumination, ambient, direct, local lights, occlusion, cascade ID,
   cluster occupancy, fog scattering/transmittance and pre/post-grade output.
5. Save receipts under `docs/verification/aethyr-lighting-pN-YYYY-MM-DD/` with
   source/compiler/package identities, authored asset hashes, host/device,
   settings, exact rasters, numeric bounds, captures, costs, refusals and known
   limitations. Pin pre-change evidence before replacing a path.
6. Keep commands below 15 minutes using focused units/shards. Run
   `coax/web/tool.sh tools-check` before pushing new tooling. Never rerun passing
   checks on unchanged sources merely to confirm them. New harnesses are Coax;
   an independent non-Coax oracle requires the repository's firm-reason header.

For every phase, record four statuses separately: implemented primitives,
Everglade adoption, browser qualification and native qualification. The final
deliverable is a playable, measured shared system, not a collection of examples.

## Scope and dependency order

The critical chain is the glade lifted into the shared painterly default →
profiles → material/color contracts → shared lights →
shadows/indirect visibility → atmosphere → complete material composition → final
grading and host qualification. Authoring and measurement continue throughout.
LUT decoding and bounded offline tools may be prepared earlier, but enabled
presentation waits for the color contract. Volumetrics depend on the actual
light inventory and resource graph, not an isolated fog demo.

This plan does not require ray tracing, real-time multi-bounce GI, a universal
metallic/roughness material, a specific filmic curve or a replacement physics
engine. Optional probe lighting, physical falloff and richer reflection methods
must earn their own bounded workload and adoption evidence. Owner-only physical
device actions or sign-offs, if required, go in workspace `NEEDS_OWNER.md`.

## Related current contracts

- [Renderer ownership, packages and qualification](../../../metaverse-engine/render/README.md)
- [Shared rendering integration and remaining boundaries](../../verification/aethyr-shared-render-audio-2026-11-06.md)
- [Light and shadow chapter](../../../metaverse-engine/book/ch013-light-and-shadow.md)
- [Sky, weather and time chapter](../../../metaverse-engine/book/ch071-sky-weather-and-the-time-of-day.md)
- [Character glade lighting and its private scope](../character-glade-lighting.md)
- [Water capabilities and integration](../water.md)
- [Repository invariants](../../../INVARIANTS.md)
