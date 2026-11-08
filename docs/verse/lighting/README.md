# Coax Aethyr lighting implementation plan

Date: 2026-11-08. Source baseline: `7fab57cd75`.
Status: proposed implementation sequence; no phase is qualified by this document.

Build one shared lighting system for Coax Aethyr, with Everglade as its first
complete production consumer. It must support authored environmental character,
readable interiors, moving characters, many local lights, stable shadows, layered
atmosphere, water and effects, and controlled final color. Shared code that the
playable realm does not consume is an unfinished phase.

## Starting point

The existing system supplies useful parts that should be composed and migrated:

| Existing owner | Current capability | Boundary to close |
| --- | --- | --- |
| `aethyr/render/light.coax` | Key direction, two-color hemisphere ambient, snapped shadow space, haze and a bounded 16-light CPU list | Realms still construct lighting and shadow space; the CPU list is not the live realm light path |
| `verse/style-preset.coax` | Four time keys, sun/moon choice, style uniforms and clock modes | No general spatial environment profile resolver or irregular time tracks |
| `aethyr/render/sky.coax`, `vegetation.coax`, `shadow-filter.coax`, `post.coax` | Shared sky/cloud, vegetation, PCF offsets, tone, bloom and shaft arithmetic | Pass ownership and several material/color contracts remain consumer-specific |
| `verse/everglade/gpu.coax` | Real geometry shadows, baked sky visibility, lamp/spell lighting, room warmth, weather, water and post | One key shadow map; at most 16 lamp entries evaluated directly; lighting remains embedded in the realm shader |
| `aethyr/render/scene-lights.coax`, `character.coax` | Detailed characters use world lighting, shadows and post; up to eight selected point lights | Character and world light admission/evaluation are different paths |
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
| `aethyr/world/environment.coax` | Typed profiles, time tracks, regions, conditions, deterministic selection and blending |
| `aethyr/world/day.coax`, `weather.coax` | Adapters over simulation time and existing weather arithmetic; no second simulation clock |
| `aethyr/render/environment.coax` | Resolved environment records and CPU/GPU packing; spatial samples and camera presentation inputs |
| `aethyr/render/material.coax` | Surface inputs, authored illumination/occlusion semantics and material response |
| `aethyr/render/scene-lights.coax` | Shared scene light records and adapters, evolving the existing character contract |
| `aethyr/render/light-grid.coax`, `shadow.coax`, `occlusion.coax` | Light selection, shadow planning and indirect visibility |
| `aethyr/render/fog.coax`, `fog-volume.coax` | Analytic atmosphere, volume injection, scattering, integration and history |
| Existing `sky.coax`, `post.coax` and shared water hooks | Sky appearance, grading/presentation and consistent water integration |
| `verse/everglade/` | Authored profiles, placements and gameplay light/effect sources; consumes shared evaluation |

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

## Phase 1 — Shared environment profiles, used by Everglade

**Outcome:** the playable Everglade reads a shared environment resolver for its
key, ambient, sky, haze and existing post inputs. This is the first delivery.

1. Before changing the render path, record pinned Everglade frames and numerical
   probes for its current day/night, forest, open ground, interior, river and
   storm states. Record the active producer, package, compiler, raster, device
   and settings so later comparisons have a concrete baseline.
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
5. Convert the existing four-key style preset to a compatibility profile first.
   Add Everglade profiles for open ground, woodland, town/interior and river
   atmosphere. Keep style response separate from the environment so a palette
   change does not switch the material model. Author content from Everglade's
   actual geometry and routes rather than relying on placeholder regions.
6. Route Everglade's uniform writer, sky, terrain/props, ordinary avatars,
   detailed character `SceneLights`, river and effects through the resolved
   context. Remove their independent key/ambient/fog calculations from the
   enabled path. Compatibility exports may delegate to shared evaluation.
7. Make this the ordinary Everglade default once its gates pass. A temporary
   diagnostic comparison switch is allowed; an opt-in experiment alone does
   not finish the phase. Record remaining non-Everglade consumers separately.

**Exit:** check-first tests cover midnight wrap, irregular knots, invalid/nonfinite
fields, overlaps, condition changes and bounded refusal. Fixed-state replay and
late-join evaluation agree. Actual browser readback shows world and character
receive the expected resolved values. Walking a pinned route through forest,
town, a doorway and river shows continuous transitions. The compatibility profile
meets a declared numerical/image bound; authored profile changes have separately
reviewed images. A malformed replacement cannot partially replace the active set.

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

**Everglade exit:** a room has authored colored illumination without a fake
emissive wall; its doorway transitions continuously. Gray, saturated fabric,
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

**Everglade exit:** a dense night route and overlapping spell workload illuminate
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

- Generalize normal/depth contact shading into a shared quality-scaled AO path:
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

**Everglade exit:** courtyard contact, a furnished room, forest roots and a moving
character show contact without halos, double-darkening or darkened emissive
sprites. Doorway crossings preserve interior/exterior continuity. Direct-light
probes remain unchanged when only indirect occlusion is varied.

## Phase 6 — Layered analytic atmosphere and local fog volumes

**Outcome:** atmosphere varies by distance, height and place before temporal
volumetrics are introduced.

- Share distance/height density integration, sun-facing tint, independent layer
  colors, start/cap controls and bounded profile blending. Use the private
  glade's integrated-height work as a starting point, preserving cheaper tiers.
- Add positioned/oriented local volumes with bounded shapes, falloffs, density,
  scattering color, emission, noise and animation. Keep volume placement distinct
  from broad environment selection and gameplay weather overlays.
- Define front-to-back composition of overlapping layers and analytical limits
  for zero density, large distance and camera-inside-volume cases. A local visual
  fog edit cannot change rain history or water simulation.

**Everglade exit:** river mist, low forest haze, a room volume and a clear nearby
foreground coexist. Pinned height/distance probes match integration references.
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
3. Keep a fixed Everglade capture suite: open meadow, dense canopy, town at dusk,
   lit room/doorway, river mist, underwater, crowded night spells and moving hero.
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

The critical chain is profiles → material/color contracts → shared lights →
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

- [Renderer ownership, packages and qualification](../../../aethyr/render/README.md)
- [Shared rendering integration and remaining boundaries](../../verification/aethyr-shared-render-audio-2026-11-06.md)
- [Light and shadow chapter](../../../aethyr/book/ch013-light-and-shadow.md)
- [Sky, weather and time chapter](../../../aethyr/book/ch071-sky-weather-and-the-time-of-day.md)
- [Character glade lighting and its private scope](../character-glade-lighting.md)
- [Water capabilities and integration](../water.md)
- [Repository invariants](../../../INVARIANTS.md)
