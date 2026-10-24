# Medieval Town implementation status

Status: browser prototype with a published Medieval Town demo, 2026-10-23.
The reference work continues; no perfect-parity or full-engine qualification
claim is made.

The [C/Wasm experiment](research/c-medieval-town-results-2026-10-23.md) adds
resident render preparation and a separately frozen movement comparison.
Current `c-frame` builds retain the current Rust controller, character, collision,
and editing behavior; C replaces source visibility, compaction, and clustered-light
preparation. The default runtime and selected engine-language decision remain in
place. See the [reproduction guide](research/c-town/README.md) for the
historical movement profile and benchmark commands.

## Reusable playable starter

The [courtyard starter](starter-game.md) adds an owned, asset-free game and the same
key-and-exit quest to Medieval Town. Its typed interactions, restart, editable
markers, save/reopen, and capability checks reuse the existing engine interfaces.
Live progress stays separate from authored saves.

## Cooked first play

[Cell streaming](cell-streaming.md) adds a bounded entry and complete-town stage,
verified dependency packages, collision-ready walking boundaries, candidate
activation, and cache/resource counters. Final full-town captures match the
complete baseline; entry quality and current residency limits are explicit.

## Implemented slice

[Compiled Town batches and prepared lighting](research/town-compiled-batches-2026-10-24.md)
add opt-in renderer diagnostics, source mappings, per-view gathers, and exact
comparison tooling for #8. Original rendering remains the default. Fewer encoded
draw commands do not establish a qualified frame-rate improvement; the record
includes failed pacing and delivery checks.

[Agent workflow](agent-workflow.md) adds versioned inspection, context-bound
actions and captures, portable logs, replay, and independent image/game oracles.

[Runtime ownership](runtime-ownership.md) checks frame and activation access,
scoped borrows, and structural safe points. An optional bounded culling worker
experiment preserves the supported serial Town frame loop.

[Incremental props](incremental-props.md) add generation-checked derived tables,
dirty matrix uploads, mesh-aligned prop collision, and shared structural collision
outputs. Building and material changes retain the complete preparation path.

- React DOM controls, Effect Atom UI state, and project-scoped Effect 4 services
  on the editor origin, with a stable preview iframe on a separate loopback port.
- An independent preview worker with direct WebGPU, instanced indexed geometry,
  normal/packed material maps, HDR display conversion, near/far directional
  shadows, generated clouds or private source HDR skies, height fog, and
  source-authored point/spot lighting with physical or authored legacy falloff.
- A private source-town compiler with content-addressed buffers/maps, material
  overrides, placement identities, a preparation cache, and explicit unsupported
  source diagnostics. Its output stays outside the repository.
- A native/Wasm Rust kernel with fixed-step camera movement, local typed edits,
  inverse undo/redo, transformed building collision, doorways, stair ramps,
  upper-floor support, and derived editable-room walls.
- Local draft persistence, bounded command queues and UI summaries, direct light
  controls, reference-camera starting points, capture, and WGSL candidate checking
  that retains the working pipeline on compilation failure.
- Eight prepared catalog assets, including cottage and townhouse recipes. Their
  visual wall parts, semantic openings, floor patches, ramps, collision, and
  persisted wall removals use the same instance identity. Reticle selection and
  wall removal use ray intersections rather than an arbitrary nearest object.

Optional source metadata recovers authored camera transforms, lens settings, and
lights. Gallery-to-camera assignments remain reconstruction hypotheses, with
fitted fallback poses for the unmatched shots; none is an accepted shot match. Rendering a complete imported scene does not
make every source house a semantically editable building. Supported authored
house plans are a separate capability with their own traversal tests.

## Open Ground zone

The opt-in [Battle Lab](battle-crowd.md) adds a GPU-simulated crowd to the private
creator without changing the shared MMORPG player's authority. Its measured
capacity and recording evidence are documented separately from the normal MMO.
Rear ranks fire projectiles, eight trebuchets bombard the front, and a local
Rust-controlled participant can enter either army at a chosen map position.
First/third-person cameras, health, melee/firebolts, death, and respawn are
implemented; the extra armies and their combat health remain device-local.

Open Ground also supports the [four-class Quick MMORPG world](quick-mmorpg-port.md)
when its scene includes `mmorpgURL`. That profile uses a shared Rust server and
the same Rust movement kernel in Wasm, with browser prediction and remote
interpolation. It imports the original models, equipment, UI art, scenery, and
effects while keeping our grass and sky. The guide separates source behavior,
intentional adaptations, and verification evidence. The local encounter below
remains a separate asset-selected configuration.

The standalone [Open Ground profile](../../../apps/aethyr/README.md#run-open-ground-locally) uses
a continuous green plane, procedural blades adapted from Quick Grass, Medieval
Town's sky and lighting, and Quick_3D_MMORPG's Paladin. The private character
importer retains the source body, helmet, equipped Quaternius sword, original
base and normal maps, and all six animation clips: idle, walk, run, attack,
dance, and death. The source controller profile uses W/S for forward and backward
movement, A/D for turning, Shift for running, Space for an attack from idle,
and Backspace for a dance from idle. Its fixed following camera, animation
crossfades, and input-driven clip clocks adapt the reference behavior without a
Three.js runtime. Normal maps, direct GGX lighting, and the world's environment
reflections share the existing WebGPU character pass. Legacy town characters
keep their existing controller. See the
[controller source notice](../../../apps/aethyr/third_party/quick-mmorpg/NOTICE).

Clumps share growth direction,
height, and color. Curved blades sway in the wind and bend around the player,
with rounded shading, bounded near/far detail, and camera clearance. GPU
preparation feeds two indexed instanced draws. A separate ground material keeps
its green hue with restrained brightening as the blades become distant. The
[grass reference study](research/grass-reference-study-2026-10-23.md) prioritizes
the original Ghost of Tsushima talk and records the visual and browser checks.
Its compact overlay shows live FPS, player health, and alive and defeated zombie
counts. Nearby zombies and the Paladin use the Rust combat state; sword attacks
and damage update that state. The overlay briefly marks damage, reports death
or a cleared field, and offers **Restart** to begin the fight again. It does
not advance combat through React state. Buildings, props, quests, and
world-editing controls are absent. The profile has separate assets,
collision, and local ports. Prepare a base with `compile_ground_zone.py`, then
run `compile_quick_mmorpg_character.py --reference REPOSITORY --base BASE --out OUTPUT`
to copy that base and select the Paladin in a new private output. Run the importer
again with `--zombie --base PALADIN_OUTPUT --out ENCOUNTER_OUTPUT` to add the source
zombie model. The
[local launch instructions](../../../apps/aethyr/README.md#run-open-ground-locally) include both
stages. Source and derived art stay outside Git.
The zombie encounter passed eight browser checks at 1280 × 720, including real
pursuit, health loss, sword kills, death, movement lock, and restart. Eight active
zombies and grass submitted 59.95 frames per second over 15.63 seconds; the FPS
overlay had a median and minimum of 60. Health bars stay within eight percent of
the viewport width, and death releases the mouse for Restart. The capture shows continuous green
ground beyond the blades. These local checks reported no browser, network, or
WebGPU errors. `tools/verify-zombie-encounter.mjs` repeats the interaction check
with `AF_CAPTURE_DIR` set to a private output directory.

The compact **Effects** panel adds [shared wind and live effects](shared-wind.md).
Grass, analytic ambient particles, and cosmetic impact smoke consume the same
typed field. Open Ground exposes wind and grass parameters plus an impact preview
with flash lighting, sparks, smoke, and debris. Accepted parameter changes reuse
GPU storage and preserve the player, clock, and active event identities. Settings
are preview-only; generic effect graphs, saved recipes, and fluid simulation
remain unimplemented.

The integrated Chrome check at 1280 × 720 passed all 10 checks and measured
60.25 submitted frames per second over 15.37 seconds of walking, turning, and
running; the FPS overlay had a median of 60 and a minimum of 58. Walking reached
3.33 meters per second and running reached 6.66. The check
also verified source asset hashes, attack and dance locks, the following camera,
grass wind, level ground contact, and the absence of browser and WebGPU errors.
An exact-coordinate regression also keeps the player at ground height zero on
the large flat terrain triangles during idle movement and coasting. These are local
measurements, not a device-wide performance guarantee. Set `AF_CAPTURE_DIR` to
a private capture directory, then run
`openagents browser run -- node tools/verify-quick-mmorpg.mjs` against the local
server to repeat the interaction check. An independent source-loader comparison
of 218,538 animated vertex samples across all six clips found a maximum position
error below 0.000001 meters, including the equipped sword.

## Overview stage

The optional `medieval-overview` deployment now uses the same editor bridge,
independent worker, renderer, and fly camera to open a separate prepared Overview
map. Its compact controls offer reset, navigation, and screenshots. The profile
rejects authoring, shader/environment changes, and walking; it has no asset
catalog or draft persistence. Town, Haunted House, and Universe deployments keep
their existing behavior. The overview defaults to its own loopback ports 55306
and 55307, with exact operator-selected origin checks.

The private source map has 237 placement records: 235 visible vendor static
placements, one owned replacement for the source engine ground, and one giant
sky sphere explicitly excluded. The browser renders 236 instances and 309,158
triangles. Its camera, daylight, background art, and ground appearance are owned
choices; recovered source lighting is not reproduced by this presentation.
An optional private preparation adds all 25 source decal samples, including 15
clipped signs and 10 surface volumes, plus a grid using source spacing and colors
with owned line widths/filtering. Its isolated browser candidate renders 251
instances and 309,196 triangles while preserving all original static transforms
and geometry prefixes. The active local overview now uses that detail stage;
its full browser profile checks still pass with the added samples and grid.
Vendor marketing labels and Niagara remain omitted;
missing UV1 remains reported. This is a runnable overview stage, not
accepted reference-image parity. See the [preparation and launch steps](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md#overview-stage).

## OpenAgents embedding

The optional [OpenAgents host adapter](openagents-host.md) consumes the public
Rust/Wasm protocol SDK. It exposes read-only open/inspect/observe/close, bounded
request lookup, and complete camera checkpoint recovery for an approved private
artifact. Embeds hide local editing controls. This does not implement the remote
authorization, command, publishing, or persistence services in the full design.

## Current comparison

The source town is visible in the browser with working textures, sun/shadow
structure, a sky, and atmospheric depth. The full scene now includes all 8,096 day and 8,102 night source placements,
including the recovered merged-house meshes. Alpha-channel resize repairs remove
black pavement artifacts. Eight source-confirmed masked material recipes preserve
painted wood and unpainted door insets. Reference comparisons still show material,
exposure, atmosphere, water/effect, and camera composition discrepancies. The [reference plan](references/medieval-town.md)
and [shot guide](references/medieval-town-shots.md) remain the acceptance targets.
Selective backdrop textures now retain the original 4096-pixel normal detail
with 2048-pixel color and packed maps. The coherent private candidate preserves
all unrelated scene/material fields and the existing 128-ray GI sampling, and
all 16 scenic views pass browser loading and graphics-error checks. It adds about
124 MiB of texture mip allocation compared with the previous three 512-pixel
maps. This is a quality/memory tradeoff, not a measured speed improvement.
Private captures are retained outside Git. Screenshots read back the submitted
WebGPU presentation texture; they do not depend on direct WebGPU-canvas PNG
conversion, which failed in the tested Chrome build. The submission counter measures accepted render submissions, with at most two
frames in flight. It does not measure GPU execution or display presentation.
Shader replacement checks opaque, transparent, additive, shadow, and camera-depth
pipelines together. Captures can specify an animation time and snapshot the
requested camera before waiting for the GPU. Gallery checks accept a fixed time
from zero through 86,400 seconds and can save the surrounding editor UI;
camera culling and the unculled path produce byte-identical captures there.

Recent reviewed improvements preserve low authored reflectance in environment
highlights, extend prepared diffuse lighting above the roofs, and adjust the
visible daytime sky independently of surface lighting. The active private
fountain adds inlet foam coverage and source-energy foam without reducing its
daytime lower-stream detail. Recovered background mist cards now restore castle
haze, with an owned edge fade and three scene-wide intensity reductions that
preserve that haze while reducing mountain overfill.
The selected mountain mesh now uses recovered tangent frames and source scale
handling, with freshly prepared lighting and wind identities. All sixteen views
load successfully; left-side ridge contrast improves, while right-side brightness
still differs. These are measured local improvements, not accepted shot matches. Selected-tree
flutter, directional surface-sky transport, and water phase-function experiments
are available but remain inactive because their tested images did not establish
an improvement. The [asset guide](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md) distinguishes each
prepared profile and its checks.

## Scope still to qualify

The general binary ABI, full behavior VM, production origin/deployment security,
durable shared authoring, publishing, multiplayer, and complete semantic import
remain specified work. The current local JSON/Wasm bridge is documented in the
[kernel README](../../../crates/aethyr/kernel/README.md). The remaining Unreal material features,
other Niagara systems, full indirect light, water, and environment/postprocess matching remain
reconstruction work; converted material-map interpretation is provisional.
Optional private Unreal metadata now preserves authored camera and light records.
Source camera identities do not by themselves establish their gallery matches.
Five source glass materials and one unlit additive flame material use recovered
fragment recipes. All lighting, skies, emission, fog, and water use one 0.01
linear radiance storage scale, canceled by camera exposure. Source sun and moon
intensities are both 25 lux. Local lights retain their inverse-square flag:
309 use authored radial falloff and 179 use physical units. The owned fixed
camera exposure is 0.125 before the UI multiplier; the original project's
exposure configuration remains unknown. This is a calibration hypothesis,
not a recovered vendor setting. The qualified fabric-sway slice is described
below; glass refraction remains unsupported. A
half-resolution screen-space contact pass attenuates ambient fill after a depth
prepass. It is not indirect light or a replacement for geometric shadows.

An optional light-temperature model converts a Planckian-locus temperature to
linear Rec.709 RGB with unit XYZ luminance before gamut clipping. Positive
channels retain their energy rather than being normalized to a peak of one.
Untagged lights retain the earlier approximation. Eleven synthetic measurements
from the installed Unreal 5.8 color factory differ from the independently
derived public colorimetry by at most 0.000731 per component. This checks the
fixed-BT.709 factory, not the original project's working space or gallery lights.
The active private town uses this model with rebuilt atmosphere, IBL, and GI
layers and rebound, unchanged measured fog values. Twenty matched browser
captures preserve exact replay and camera settings; three published captures
match the reviewed candidate byte for byte. Night pools become warmer, while
the reference's amber lighting and several material colors still differ.
Light-model tags participate in local-bounce identities so old lighting cannot
silently activate with the new colors.

Twenty source-confirmed two-layer trim materials now evaluate both layers,
vertex-red painting, height masks, world-aligned aging, and detail normals.
Supplemental RGBA8 maps use a straight-alpha binary upload so zero-alpha height
data cannot erase RGB during browser decoding. The five three-layer terrain/roof materials now also evaluate source wetness,
vertex painting, and bounded BumpOffset. Fourteen foliage/fabric materials restore
source fuzz, gradients, masks, normal flatness, and alpha threshold; two-sided
transmission remains an explicit lighting approximation. Optional source wind
supports the selected fabric components below. Source HDR skies supply the background and fog. A separate owned single-scattering
atmosphere supplies distant lighting when prepared. It uses recovered scattering
coefficients and sun settings, with an explicit two-meter observation-height
assumption. It omits multiple scattering, ground feedback, and clouds; no source
captured cubemap was recovered. A bounded SH9/GGX split-sum profile supplies
distant diffuse and glossy surface lighting. Visible-sky brightness and tint do
not multiply this atmosphere lighting. The source Skylight gain, active sun
illuminance ratio, and shared storage scale are each applied once.
A bounded world-space probe field now replaces
diffuse sky lighting inside its central-district coverage. Distance moments
reduce interpolation through walls and provide an approximate directional sky
visibility bound for glossy surfaces. Day and night use separate source geometry
and moments. The current field tests material alpha during triangle traversal,
so a transparent leaf texel can reveal the next surface. An optional one-bounce
field samples source albedo, UV sets, and vertex painting. Its separate sky,
white-sun-per-lux, and local-light SH9 layers preserve independent live gains.
This is finite, prepared diffuse transport, not Lumen or reflected scene color.
The material approximation omits decals, parallax, fuzz, subsurface transport,
glass, water, and emissive bounce. Twenty painted plaster/stone materials now use recovered
vertex-blue layer weights, vertex-green dirt, and independent source UV1.
Missing UV1 remains explicitly counted; soil overlays are still affected.
Twenty-two sign and label decals per scenario now use their source atlases,
projection volumes, clipped opaque receiver geometry, and depth fade. They
remain lit alpha overlays rather than deferred normal/specular decals. The
other 758 surface decals per scenario now modify color, world normals, metallic,
F0, and roughness before lighting through one ordered draw and one three-layer
array binding. Mixed-depth edges use the nearest of four depth samples; exact
per-sample composition and source distance fading remain unsupported.
Three measured fountain basins and four inferred ballistic streams use a separate
water pass with refraction, Fresnel, absorption, and animated impact rings.
Streams widen toward their impacts and add deterministic ballistic spray.
Bounded screen-space reflections sample local scenery, with the source HDRI
as the fallback for misses. Animated stream displacement and smaller,
velocity-stretched spray improve breakup. Distant HDRI cannot supply offscreen
building reflections. One visible basin plane now receives a separate capped
half-resolution HDR scene reflection with mirrored winding and below-plane
clipping. Camera, scene, light, environment, and shader revisions invalidate its
cache; water is excluded from the reflection. Other basin heights retain their
fallback. Reflection captures omit screen-space AO and decals rather than
sampling data from the wrong camera; full reflection parity remains unqualified.
Source Skylight intensity and black lower-hemisphere lighting are retained
without changing the visible sky. Camera and each light view compact source
instances independently; custom WGSL disables this optimization because edited
vertex positions have no declared bounds. This reduces submitted work, not a
measured claim about GPU or display speed.

Probe files are checked against the scene geometry/material description and
selected lighting recipe before activation. Day and night may have independent
grids and visibility policies; GPU upload selects the matching scenario rather
than assuming one shared field. Bounce files additionally bind the exact
published base, coverage/material descriptions, finite coefficient bytes, and
content hashes. Changed sunlight direction disables its prepared sun layer;
unknown or changed local-light records disable only the local layer. Changing
sun intensity or color uses the existing unit-lux layer. Authored geometry or custom WGSL disables
the prepared field so an edit cannot continue using stale lighting. The live
sky profile remains available. The field does not cover every distant landmark.
The default field now has 3,315 probes, from -0.25 through 31.75 meters in height.
It preserves the former 1,755 probes and extends coverage above the roofs, so
upper surfaces can interpolate local lighting instead of falling back to global
sky. Ground and low-wall queries also stay inside its lower coverage; invalid
below-ground probes remain excluded by their records and moments. A selected
scenario uses 5,608,400 bytes for base records, bounce layers, and distance
moments, including the constant-fog companions; other renderer resources add to
that figure.

A separate optional static-surface field refines direct sky at selected source
vertices. Alpha-aware near and far rays prepare solar and constant-fog SH9;
fragment evaluation uses the mapped world normal. Source identities, geometry,
materials, transforms, and addressed vertices are checked before upload. Shared
instancing batches split around selected objects without changing source
addresses. Authored geometry and custom shaders disable the field; selected
moving geometry is rejected. Existing bounce and direct-light terms stay separate.
Sparse vertex maps now address recovered tangent seams without allocating records
for unrelated vertices. Their bounded GPU lookup preserves the earlier dense
layout; signed validity distinguishes diffuse-only and reflection records during
interpolation.

An optional rough-reflection profile deconvolves the Lambert SH9 bands and
reconvolves them with the existing GGX filter's zonal moments. It replaces the
global direct-sky reflection at qualified surfaces, without applying probe
visibility a second time. Roughness below 0.4 and negative SH reconstruction use
the existing reflection. SH9 cannot recover omitted directional detail, and
corner samples cannot resolve lighting changes within a large leaf card.

The current private candidate addresses 18,433 tree vertices and uses 10,764,944
GPU bytes across both modes, including its fallback. Synthetic checks cover 672
reflection cases, 21 rendered address cases, and 131,074 sparse queries. Eighteen
matched browser captures separate the default, diffuse-only, and reflection
paths: MT-18 mean RGB reference error changes from 0.15258 to 0.15124; MT-17 and
MT-11 remain byte-identical. Thirteen additional captures verify exact replay,
shader failure, edit invalidation, undo, custom shaders, and source restoration.
The field remains inactive pending further image review. These checks establish
an optional transport path, not foliage or gallery parity.

Environment reflections now respect the source's low-reflectance authoring
response: the split-sum table's grazing coefficient scales down below 2%
reflectance. A zero-reflectance surface no longer receives a white grazing
reflection, while ordinary reflectance retains the previous arithmetic. The
coefficient uses maximum-channel reflectance; it does not tint the grazing term
or transfer the suppressed reflection into diffuse light. Lookup-table bytes,
source gains, exposure, normals, and probe inputs stay unchanged. Ninety-nine
synthetic GPU cases qualify the coefficient, including 45 unchanged ordinary
reflectance cases. Actual browser checks cover all 16 scenic cameras, reverse
time captures, and live effects. In the tested dark leaf interiors, this
correction reduces linear brightness by 34.43%. Direct sunlight and local lights
now share this low-reflectance grazing response; the existing diffuse factor
stays unchanged. The extended 99-case GPU check has a maximum absolute error
of 0.000000098, with ordinary direct reflectance retaining its previous equation.
Fresh day, castle, and night browser comparisons pass without GPU or page errors.
Directional occlusion and foliage motion still differ substantially from the
reference.

The selected delonix leaf mesh now uses recovered source vertex frames in the
private town. Its separate vegetation profile preserves quantized vector lengths,
assembles the source tangent before interpolation, and leaves fuzzy-shading view
coordinates unnormalized. Only this mesh receives a cloned leaf recipe; one
conflicting tangent seam is split without changing any triangle's position or UV.
The earlier sixteen-view comparison did not exercise the cloned material because
its render batches still named the shared recipe. The compiler now rebuilds the
selected mesh's batches from its instance overrides, and runtime checks reject
that mismatch. Reviewed day, castle, and night captures exercise the corrected
binding; the published day capture matches the reviewed candidate exactly. In
MT18, 166,404 pixels change by more than one color code, while whole-frame
reference error stays nearly unchanged. This corrects the source binding without
resolving the bright foreground leaves or establishing gallery parity.

Supplemental maps use resolution buckets selected for each material and role.
Raising one material's resolution no longer enlarges every unrelated array layer.
Each source image decodes once during preparation, with separate sRGB and linear
GPU roles. Default bounds are 256 MiB decoded images and 512 MiB peak pooled GPU
allocation; ordinary textures, geometry, shadows, and other passes add to these
figures. Allocation failure disposes the candidate arrays.

Clustered lighting admits all 488 enabled nighttime local lights. Conservative
screen tiles and logarithmic depth lists preserve source order; an exhausted
list falls back to all lights instead of dropping illumination. The main camera
and planar reflection have separate lists. The nearest 32-light selection now
only chooses shadow candidates. Its shadow budget is four point lights and
sixteen spotlights: forty cached depth views.
Stable slots redraw only changed light views or changed geometry. This budget
does not provide shadows for every light in the town. Temperature uses a
three-band approximation. Analytic height fog uses the recovered source
density/falloff conversion. A separate local-light scattering pass now uses
the source volume density, gray albedo, phase distribution, 60-meter reach,
and each lamp's volumetric intensity. It adds light before display conversion
without applying extinction twice. Twenty-four midpoint samples run at half
resolution, capped at 640 by 360, with depth-aware upsampling. Existing cached
local shadows also constrain this light. A separate 32 × 18 × 24 incident-light
field adds occluded sky scattering from the existing probes. It reverses their
Lambert convolution, applies the source scattering direction, and combines solar
and constant fog lighting before clamping. Source SkyLight gain applies once;
exposure and background brightness do not enter this field. Invalid or out-of-grid
queries contribute no sky light. The field uses 110,592 texture bytes plus a
32-byte uniform buffer. Emissive volume scattering, multiple scattering, and
temporal accumulation remain absent; thin edges can be underfilled.
Directional volume scattering also uses the current sun/moon color and lux,
source volumetric strength, and cached near/far shadows. Bind groups reuse stable
resources between frames. Surface transport now uses source volume extinction
inside its enabled range, replacing the constant analytic color there. Analytic
height-fog color applies only outside that range; a nonzero volume start also
retains and composes the preceding analytic segment. Long upward and downward
rays use stable optical-depth integration. Surfaces, sky, water, and particles
share this range math. Volume light still comes from the opaque-depth pass;
transparent layers do not sample a separate light integral at each layer's depth.
These are visible quality limits.
No Lumen/Nanite equivalence is implied.

The visible HDRI is reconstructed independently of lighting. The source Skylight uses real-time
capture, the visible sky material is not tagged IsSky, and the scene contains an
authored atmosphere. The separate single-scattering map is an owned capture
hypothesis, not a recovered source-engine capture. Source day
exposure uses Histogram with min/max 0.5/6 and bias +1; night overrides min/max
4/4. Original project exposure semantics remain unknown. The owned fixed
exposure does not reproduce source automatic exposure. Kit views now use an owned,
versioned filmic display profile with source override admission, an 18% gray
anchor, monotone logistic tails, and highlight whitening. It applies the authored
WhiteClip 1 and day shadow contrast 0.85; the other film controls remain inherited.
Its overlap repair, shadow transition, whitening, and color-space treatment are
explicit approximations rather than an exact Unreal or ACES transform. An
optional measured display profile now replaces that curve for the source town.
It uses an original synthetic RGB chart rendered by a controlled Unreal 5.8
editor, with separately verified scene-linear input, fixed film settings, and
independent test colors. It does not recover the original gallery project.
The source wrapper binds the scenario and film/shadow controls; changed controls,
corrupt bytes, unsupported formats, and oversized responses cannot activate.
Exposure remains external. Logarithmic input shaping and trilinear 3D sampling
produce unclipped tone-linear color; the existing final output stage encodes
sRGB. The generated-sky path and unlinked source fallback retain their previous
behavior. Prepared response files and captures stay outside Git. The owned
[reference tools](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/reference/README.md) reproduce the synthetic test.
The selected 63-cubed response follows the measured logarithmic knots and uses
2,000,376 bytes per mode. Across 2,048 held-out day/night GPU comparisons, the
largest observed encoded-channel error is 0.000660, about 0.17 of one 8-bit code.
This measures the controlled display response, not complete gallery parity.
The recovered after-tone sharpen now runs separately
in display-linear color: four unfiltered neighbors at fixed 0.001 UV offsets,
their mean subtracted from the center, and a grayscale high-pass added back.
It is enabled only by the known active unbound source material. Source grain
remains unsupported. Explicit full-weight source-camera vignette overrides now
apply cosine-fourth lens falloff before the display response. The sensor-plane
radius uses the viewport aspect and diagonal. Across 35 controlled flat-chart
samples at 16:9, the GPU response differs by at most 0.000327 per linear
channel; applying falloff after tone mapping fails that comparison. Other aspect
ratios follow the owned geometric model. Inactive, absent, or partial overrides
remain unactivated because their inherited state is not qualified. The flat
reference does not qualify bloom interaction or complete source-camera grading.
The visible source dome now receives finite-path height fog. Its center and
radius come from validated spherical geometry and placement rather than a fixed
sky distance. Generated sky behavior remains unchanged. The bright distant
mountain is a separate backdrop mesh, so this sky correction does not qualify
its lighting. A third cached directional-shadow cascade now covers distant
geometry without changing the existing near/far projections. The two 2048-square
far maps share one two-layer depth texture, adding 16 MiB while preserving the
baseline sampled-texture count. In the controlled MT-11 comparison, one mountain
sample's scene-linear luminance fell by 76.4%; the MT-09 capture stayed identical.
This corrects missing distant self-shadowing, not all backdrop differences.
Visible kit-sky brightness now has its own control, independent of sky lighting,
probes, and reflection maps. Reference cameras use an explicit owned grade:
daytime exposure multiplier 3, or 4 for the fountain detail and café view, with
the visible background multiplier `0.28 / exposure`. Night cameras use
multiplier 1. These photographic starting points come from browser comparisons,
not recovered source-camera exposure values; lighting controls remain editable.
The initial three-view exposure comparison reduced measured surface deficits
from roughly two display-linear stops to within about one stop; its MT-02 plaza
and plaster samples were within 0.1 stop. A separate same-exposure comparison
across MT-02, MT-17, and MT-18 reduces clear-sky RGB RMSE from 0.326 to 0.104,
with sampled opaque regions byte-identical. It changes only the visible
background, independently of sun, ambient lighting, fog, and surface exposure.
Cloud placement, mountain lighting, and other material or geometric differences
remain visible.
The source's 67 flag-string spline components now deform along recovered
component-space Hermite curves in both modes. They previously overlapped as
undeformed segments at their actor origins. The incremental preparation keeps
original geometry prefixes and all material recipes intact, appends corrected
geometry/attributes, and rebuilds GI for the new geometry. One folded source
segment uses an explicit normal fallback that preserves original seams.
All 16 gallery views open with the corrected geometry; the café's overhead
flags and their shadows are visible again. General spline editing remains
separate work.
An optional source-selected sway profile now animates nine tent components and
67 flag-string components in each scenario. It preserves source component
pivots, axes, sphere radii, material controls, and vertex-green pinning. The same
vertex displacement runs in color, camera depth, directional/local shadows, and
planar reflections. Conservative bounds cover the displacement. Only 5,015
source triangles move; cached static shadow copies let affected views redraw
their moving subsets. The copies have a 136 MiB lifetime cap; the measured
three-camera run retained 98 MiB. The independent flutter branch, flame wind,
other foliage motion, and dynamic GI remain unsupported. Animation uses the
owned world clock; the gallery's original phase is unknown. Browser checks cover
zero-to-three-to-zero replay, shader replacement, camera retention, shadow work,
and all 16 scenic views. Repeated fixed-time captures match exactly.
MT-18 now uses a separate owned image-fit camera. Fresh house correspondences
have a median discrepancy below one pixel at 1280 × 720, compared with about
62 pixels for the previous source-camera hypothesis. The source camera stays
unchanged. Roof, chimney, and mountain relationships now align substantially
better. A further 24 cm camera refinement restores the three missing upper-right
frond groups without moving trees or changing opacity. Measured silhouette
overlap in that sky region improves from 0.1% to 60.7% intersection over union,
while fresh house correspondences improve from 1.10 to 0.84 pixels median.
Fixed-time 0 → 3 → 0 captures replay exactly. Remaining foliage lighting, other
foreground outlines, animation phase, and full-image parity remain unqualified.
Optional private fit metadata preserves the owned pose across preparation;
the original authored camera stays unchanged.
Kit fog uses the recovered constant inscattering color. An optional measured
atmosphere contribution now adds the source sky's distant-fog lighting. Its
controlled day/night fixtures independently check black/white receivers at
three distances, isolate atmosphere contribution, and test doubled sunlight.
The prepared profile binds the source atmosphere, sun, and fog settings; changed
sun direction/color or generated sky disables this contribution. Incident lux
scales it linearly. Source ambient-contribution RGB applies once, with no
Skylight gain, camera exposure, or scene-storage conversion baked into it.
The host applies the shared storage scale once and the existing analytic fog
weight outside the replaced volume range. The separate occluded sky field also
lights the local volume. Neither approximation reproduces the original gallery
project settings. Actual browser readback checks zero and doubled SkyLight gain,
solar/constant separation, and finite field values in day and night views. The
visible change is modest; broad color and atmosphere differences remain.
It does not sample
ground lamps from the visible panorama onto opaque walls; that previous blend
caused four white rectangles in the night fountain view and has been removed.

Prepared sky lighting now optionally includes analytic fog at the source
SkyLight's capture height. An asset-free reference isolates this from camera
fog and confirms a previously missing contribution. Preparation keeps solar
radiance and the authored constant fog color separate through diffuse SH,
roughness-filtered reflections, occluded probes, and one diffuse bounce.
Sunlight scales only the solar term; source SkyLight intensity scales their sum
once. Both reflection layers use the same texture binding, and optional probe
records append to the existing buffers. Synthetic GPU checks cover zero and
doubled sunlight, pre-clamp coefficient addition, independent local/sun bounce,
and water reflections. The source atmosphere approximation still differs from
Unreal, and this does not establish gallery-image parity.

An optional source-stream pass now replaces inferred jets with two recovered
curved meshes, their persistent local-space placement, exact source scale, and
animated noise. It draws eight daytime and six nighttime streams, including the
previously missing café fountain. Emission, exposure compensation, Fresnel
opacity, depth fade, and translucent composition follow recovered parameters.
Basins retain their separate reflection pass. Noise-controlled refraction now
searches opaque depth from one nearest stream interface, sampling a frozen
background into a separate output. Missing, foreground, and offscreen hits use
the original pixel. Excessive private-depth allocation falls back to ordinary
transparency. This does not reconstruct overlapping interfaces or hidden color.
An optional, versioned recipe also supports the source signed view-normal
screen displacement. Independent Unreal plane captures qualify its perspective
scale and one-centimeter depth fade; 33 WebGPU cases cover that model and its
fallbacks. The source-signed candidate remains inactive. Full fountain
appearance remains unqualified.
An owned dielectric profile supports clear transmission, exact Fresnel,
absorption, directional highlights, environment/screen reflections, and bounded
thin-sheet displacement. Twenty-eight synthetic GPU cases qualify those optical
and lighting contracts. Its first smooth-sheet candidate remains inactive.
The active private scene now uses a related turbulent profile with subdivided
stream geometry, bounded ripples, moving air entrainment, and up to 32 source
local-light highlights. The same deformation runs in the nearest-depth and color
passes. Thirty-one GPU cases check deformation, scattering, and physical/legacy
light units. Eighteen matched day/night/café captures at zero, three, and ten
seconds show finer flowing detail without graphics errors. Upper streams still
read too smoothly; impact breakup and nighttime highlights still differ from
the reference. Transparent-stream light shadows remain unsupported.
Optional prepared recipes also retain the source normal sampler's signed-red
scalar input. A versioned spatial filter integrates its positive-area coverage
before mip generation; it preserves the two Fresnel contributions without an
artist-selected gain. The normal candidate remains outside the active private
scene: five captured views still show coarse, pale stream contours. Source
temporal filtering and ripple emitters remain unsupported. A separate optional
source foam pass is implemented but its full source recipe remains inactive:
actual town captures produce broad white impact clouds rather than the
reference's fine spray. The active private scene instead uses an explicitly
owned subtle-spray profile: 1–3 cm droplets, shorter mouth mist, and different
rates, forces, and age curves. It retains source anchors, noise, and material
primitives. Spatial footprint expansion compensates opacity by area so small
droplets do not become opaque discs. Eighteen day/night/café captures show
localized spray at three and ten seconds, unchanged startup images, and no
graphics errors. This improves the preview without claiming source-system or
gallery equivalence. The turbulent stream reduces the older solid contours but
does not reproduce the reference fountain.
A read-only public exporter reproduces the required private metadata and data
maps; no licensed output is committed. See
[the water preparation guide](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md#fountain-water).

An optional owned billboard pass now renders source dust and chimney smoke.
The private compiler retains emitter placements, rates, lifetimes, size/alpha
curves, acceleration, drag, and emission. Complete lifecycle metadata admits
continuous source emission with zero warmup: no particles exist at world age
zero, and births accumulate at the unchanged source rates. Inputs without this
proof retain the earlier owned steady-state start. Sampling remains deterministic
at a requested time and retains at most 4,096 particles. Dust and smoke preserve source translucent composition;
smoke uses premultiplied emission,
exposure compensation, an 8 by 8 atlas, and soft opaque-depth intersections.
Analytic drift replaces source curl noise, and adjacent atlas-frame blending
omits motion vectors. Sprites use analytical fog rather than shadowed volume
lighting. Falling leaves and the broader Niagara runtime remain unsupported.
The public read-only effects exporter can reproduce the selected
metadata and private atlas from a licensed Unreal project.

A separate bounded mesh pass now renders source birds and bats, with source
meshes, materials, wing deformation, placements, continuous emission, and
zero warmup. Source sunlight and the composed SH9 sky irradiance illuminate
them; storage scale and fog apply once. The 128-cycle deterministic population,
upright flight basis, straight gull paths, bat drag/orbit approximation, and
alpha-to-coverage are owned choices rather than a Niagara execution engine.
The pass omits creature shadows and exact source temporal dithering. Sixteen
actual host captures covered four day/night views at zero, three, and ten
seconds. Time-zero images remain identical to the disabled pass; live captures
contain birds and bats without graphics errors. Exact gallery populations and
flight paths remain unqualified.

The optional falling-petal pass uses the source petal mesh, its original masked
vegetation material, and 26 supported emitters per scenario. A separate read-only
recovery verifies effective component radius/spawn controls, zero warmup, and
the absence of Niagara material overrides. Two blowing green-leaf emitters per
mode remain omitted because their actual source material texture inputs are
unconnected. Source geometry, cutout, scale, placements, and parameters are
preserved; deterministic gravity/drag/harmonic drift, random draws, rotation, and
global diffuse/subsurface lighting remain owned approximations. The pass writes
the shared main depth before later fog/refraction, but omits shadows, planar
reflections, and earlier SSAO/decal coverage. Twenty-four actual host captures
cover three views at zero, three, ten, and zero seconds, with identical startup
and repeated-zero images and no graphics errors. Its material pools and buffers
allocate about 32.3 MiB; this is a measured allocation, not a performance claim.

Dust uses the recovered squared-exponential radial formula with separate
finite-support emission and opacity masks. Smoke's emission includes its source
opacity factor before translucent blending. A bounded
subpixel filter preserves its projected HDR energy as the camera moves.
Bloom now integrates the source footprint before two contiguous, normalized
Gaussian passes; the former sparse nine-sample pattern produced visible copies
of bright dust sprites. The three half-resolution HDR images are capped at
640 by 360, with a maximum allocation of about 5.53 MB. This is an owned bloom
approximation, not the source engine's bloom implementation.
The active unbound daytime source volume disables bloom. That default now
survives source/mode changes independently of each artist's accepted light-glow
setting. Inherited nighttime bloom remains an owned default because the source
project's effective value has not been recovered.

Material-sidedness now follows source flags in color and camera depth, including
mirrored instance transforms and mirrored reflections. Camera-depth alpha also
includes vertex opacity. Shadow rendering retains the previous two-sided policy;
matching source shadow sidedness remains unqualified.

Normal mapping now derives an orthonormal tangent frame from source-facing
geometry and UV gradients, including mirrored charts and nonuniform transforms.
The final world normal receives the complete backface sign. Explicit OpenGL and
DirectX conventions also control the view basis used for parallax. The private
source compiler admits DirectX only after a complete normal-texture metadata
check; all 99 current source normal maps are linear with green-channel flipping
disabled. Missing proof leaves the convention unspecified and reports a warning.
Qualified vector normal samplers reconstruct positive Z from filtered XY before
layer blends or flattening. This encoding is independent of the tangent Y
convention; omitted encoding keeps RGB decoding. The compiler requires both
texture metadata and a supported recovered vector-sampler graph, so unrelated
water recipes do not inherit this rule. Compressed-texture artifacts remain
unqualified. Prepared diffuse transport ignores these two runtime fields because it
does not evaluate normal-map microgeometry; coverage, albedo, other material
fields, and texture references still affect its identity.

## Local Druid destruction

Medieval Town now supports Meteor Shower (1), Lightning Bolt (2), and Thunderbolt
(3) through the compact play controls. Aim at an actual wall, roof, or ground
surface and click **Cast**, click the world, or press Enter. Movement continues
during casting. **Restore town** clears local damage, debris, and spell effects.
These actions do not change the authored scene revision or publish world edits.

The importer divides eligible placed solids into cells no larger than
3.5 × 3.2 × 3.5 meters. Floors support cells from below; neighboring cells carry
support across placements. Losing the remaining supports collapses dependent
cells. The renderer clips the original textured geometry on demand, retaining
standing cells and drawing rotating fragments. Matching per-triangle ownership
removes collision only for destroyed cells. Ground, floors, and walkable stair
treads retain their original collision surfaces. Entire unsupported background
sources remain permanent; registered sources always retain complete cell coverage.

The current private day preparation contains 20,612 cells and 716,690 collision
triangles; night contains 22,174 cells and 720,028 triangles. Both pass the runtime
source/hash checks. Preparation rejects Float32 triangles below the kernel's
minimum area. The kernel permits 65,536 cells, two million static triangles, and
256 moving fragments. Rendering preflights the complete affected source set;
an over-budget cast fails before applying damage. Repeated casts can exhaust the
bounded geometry cache, requiring **Restore town**.

The three abilities adapt the public OpenAgents mechanics recorded in the
[gameplay notice](../../../apps/aethyr/third_party/openagents-gameplay/NOTICE): Meteor Shower casts
for 2.5 seconds and drops six meteors; Lightning Bolt is the immediate horizontal
beam; Thunderbolt casts for 1.2 seconds before its separate sky strike. Damage,
timing, support collapse, and collision run in the fixed-step Rust/Wasm kernel.
Particles and textured fragments use direct WebGPU. This is local play, without
multiplayer authority, mana costs, general-purpose rigid bodies, semantic room
editing, or persistent damage. Baked lighting does not rebuild after destruction.

The initial Druid release passed 46 kernel tests and a raw-Wasm smoke. Its private Node 24/Wasm probe
loads the real day collision in 1.38 seconds and its cells in 35.9 ms. At seed
12999, the same arch target produces the following single-run CPU observations:

| Ability        | Broken cells / source placements | Prepare cast | Simulation tick p95 |
| -------------- | -------------------------------: | -----------: | ------------------: |
| Lightning Bolt |                            3 / 1 |      2.33 ms |             1.43 ms |
| Thunderbolt    |                           21 / 9 |      1.88 ms |            14.62 ms |
| Meteor Shower  |                          96 / 60 |      5.34 ms |            11.49 ms |

Each actual destroyed set equals its prepared plan; an undamaged cell of the same
48-cell arch remains solid. Another CPU route stops a walking capsule at z=30.76
against the intact wall, then reaches z=32.20 through its opening in 30 simulation
ticks after Lightning Bolt. These measurements exclude browser rendering and
do not qualify performance. The private reports and source-derived fixtures stay
outside Git. Run `tools/verify-druid.mjs` through `openagents browser run`, with
private `AF_CAPTURE_DIR` and `AF_DRUID_FIXTURES` paths, to check actual controls,
submitted geometry, capsule movement, failed-shader retention, and reset. The
local browser check passes all 11 scenarios, including real W input stopping at
z=30.76 against the intact wall and crossing its cleared opening to z=36.15 after
Lightning Bolt. That release's settled rubble could block the route until its
20-second lifetime ended. Meteor Shower reaches the 256-fragment limit without overflowing geometry;
all three abilities submit effect, fragment, and standing-wall triangles. Failed
shader edits retain damage and pose, and restoration returns wall collision.
The short 1280 × 720 restored-view sample submits 58.1 frames per second; this is
not a whole-town or GPU execution benchmark. The hosted candidate and real public route each pass the same 11 browser checks.
The public route became ready after 43.69 seconds; the short restored-view sample
submits 58.5 frames per second. Asset startup remains slow and this view does not
qualify whole-town performance. The separate nine-check production character
regression passes; its third-person starting view submits about 28.6 fps over
3.04 seconds.

That release reserved 28 MiB of fixed geometry buffers and capped its CPU fracture
cache at 96 MiB. Actual source clipping preflight took 175/292/630 ms for
Lightning Bolt/Thunderbolt/Meteor Shower in the CPU probe, separately from the
kernel forecast above. Fragments preserve exterior source surfaces and materials;
new interior fracture faces are not capped. Spell sprites are absent from water
reflections, and damaged scenes use live direct lighting and image-based lighting
instead of the unchanged baked lighting. These visual limits remain.

## Finer building rubble

Every building uses a four-by-four-by-four fragment grid for new spell damage,
including Meteor Shower, Lightning Bolt, Thunderbolt, and the castle barrage.
Pieces have half the previous width and retain the original source surfaces and
textures. Exact two-word surface masks prevent empty fragments from becoming
invisible collision bodies. The kernel checks each cast's prepared masks before
applying damage.

The shared pool retains up to 1,024 stationary pieces across buildings, alongside
256 moving pieces. A deterministic sample across broken cells replaces the old
last-128 castle-only pool. Retired pieces enter the visible field after their
predicted landing, with at most 16 terrain queries per simulation tick. Their
collision uses permanent scenery rather than breakable roofs. Exact visible
backdrop terrain within 64 meters of the castle now participates in collision;
source triangles and destruction IDs elsewhere stay unchanged. **Restore town**
clears all of this local damage and rubble.

All spell preparation yields to rendering and input, supports cancellation, and
fails before damage if its geometry exceeds the reserved limits. Fixed geometry
buffers total 55.6 MB; the CPU fracture cache is capped at 160 MiB, including
working data retained during candidate preparation. An earlier Meteor-to-castle geometry trial, before the surface-targeting fix, fits 320,412 rubble vertices, 182,637 dynamic vertices,
207,651 standing vertices, and 138.1 MB of counted CPU data. A warm Restore followed
by Meteor peaks at 145.1 MB. These are CPU geometry observations, not GPU timing.

## Enemy knights

Medieval Town has eight animated knights on collision-checked town-square and
street patrols in both day and night scenes. Their fixed-step Rust simulation
owns movement, targeting, health, and death; the UI receives counts only.
Meteor Shower, Lightning Bolt, and Thunderbolt kill knights within their actual
impact area, with walls blocking damage. A delayed spell checks the knight's
position when it lands. Cast preparation keeps the chosen point while a knight
moves. Defeated knights stop, play their death animation once, and remain down
until **Restore town** revives them and clears building damage.

The owned implementation uses public Verse's route, gait, and combat behavior as
reference. The prepared [KayKit Knight](https://github.com/KayKit-Game-Assets/KayKit-Character-Pack-Adventures-1.0)
uses its original running and death clips, armor, sword, shield, and cape. Its
source is CC0; source art and prepared assets stay outside Git. One shared
instanced draw supports up to 16 knights with about 4.7 MB of GPU resources.
The current eight-knight patrols add about 1.2–1.7 ms to measured CPU step means
in the actual day/night town probes. These are local CPU observations, not
browser frame-rate qualification. Full actor poses are available through the
existing on-demand agent inspection, without mirroring them into React state.

## Surface spell targeting and download recovery

Selected spells display a depth-tested ring on the actual wall or ground. An
unlocked pointer aims through its screen position. Locked third-person aiming
uses a point above the avatar; first-person aiming uses the view center. The
camera pick and the player-to-target visibility check share the cast target.
Clicking to cast preserves the current pointer-lock state. The body-centered DOM
crosshair is removed. Ring geometry updates in the rendering worker without React
frame updates.

Meteor scatter reprojects within a bounded area around the selected surface, so
a different street cannot replace the wall under the ring.

Startup asset reads retry a transient network failure, an interrupted response
body, or a failed immutable-file length or hash check. Each retry starts a fresh
read and discards incomplete bytes. Same-origin reads allow three attempts within
60 seconds, with a 30-second attempt limit and bounded backoff. Closing the world
cancels pending work; missing files and invalid formats fail without retries.
Tests cover partial streams, short bodies, damaged bytes, and cancellation.
The [published release record](../../../scripts/deploy/aethyr/README.md#published-surface-targeting-and-finer-rubble-release-2026-10-23)
lists the immutable production versions and browser checks.

## Destroy the Castle

The Medieval Town ability **Destroy the Castle (4)** targets the compiled castle and its attached
flags automatically and turns the view toward their shared bounds
without teleporting the player. Casting starts four large meteors and two sky
lightning strikes every 0.4 seconds. The barrage keeps targeting surviving cells
until none remain. **Stop barrage** or Escape stops new waves; already falling
strikes still land. It can resume against the remaining castle. **Restore town**
rebuilds it and clears its rubble. This remains local play, without saved damage.

The checked optional `targetGroups.castle` manifest field identifies every castle
cell and admits only complete source placements. The elevated background assembly
uses its source-derived component base cells as structural foundations. The shared
rubble pool above retains textured fragments on the visible terrain after the
ordinary moving debris expires. Renderer-derived 64-bit surface masks prevent
empty interior cells from becoming invisible rubble bodies.
Collision preparation rejects permanent fallback for registered source triangles,
keeps simplified geometry inside original bounds, and preserves the kernel's
minimum Float32 triangle area.

Full castle geometry is prepared before the first strike, in cancellable batches
that yield to rendering and input. Day/night probes cover 6,954/6,834 cells;
compatible Restore reservations reuse their geometry. Inactive scenario caches
are discarded. Up to 64 effects fit the existing effect buffer.

The first castle release passed 60 kernel tests and day/night Wasm proofs. Those
proofs destroyed the exact castle group, left every town cell unchanged, and
retained its original 128-piece pool after the moving debris lifetime.
`tools/verify-castle.mjs` additionally checks
real controls, asynchronous preparation/cancellation, movement, actual source
collision, rendered rubble, failed-shader retention, and restoration in an
isolated browser using private source-bound fixtures.

The first castle release, `c1f78f2b939bb184438`, shipped at
[Medieval Town](https://openagents.com/demo/medieval-town). Its local and hosted
candidate browsers each pass 10 checks; the public route passes seven focused
checks, including complete source removal, lasting rendered rubble, failed-shader
retention, and restoration. The merged build passes 603 TypeScript tests, type
checking, and the spec check. The unchanged kernel passes 60 Rust tests. The
48 deployment checks pass. See the [release record](../../../scripts/deploy/aethyr/README.md#published-castle-barrage-release-2026-10-23)
for immutable identities, rollback, and the limited browser timing observations.

## Checks

The town now opens with Everglade/Grove's animated Ranger character and an owned
port of its first-/third-person controller. React controls and the V key select
the view; mouse look, WASD, Shift, Space, and wheel zoom feed the independent
worker and fixed-step Rust simulation. The camera pulls in around walls, hides
the avatar near the head, and eases back out. Switching perspective preserves
the player's feet. Animation uses actual admitted distance and velocity, so
pressing into a wall does not keep the character running in place.

The private character importer retains the original rig, source textures, and
seven movement clips. GPU skinning uses 75 joints; the avatar has 27,402 vertices
and 31,396 triangles. The current Grove “druid” uses that Ranger model; its earlier
green-outfit/staff proposal is not a separate implemented character. Attribution
and the public source commit are in the [character notice](../../../apps/aethyr/third_party/openagents-character/README.md).
The avatar participates in world lighting, fog, and depth. Character shadow
casting/receiving, foot IK, and automatic navigation remain unimplemented.

Prepared static town collision now supplies real floor, stair, and wall contacts
in addition to authored building colliders. Ground and floor geometry stays
exact, including walkable stair treads; other solids are simplified, with
foliage/effects/backdrops excluded.
Day and night have separate collision meshes bound to their source placements.
The [asset guide](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md#static-walking-surfaces) describes
preparation and limits. These static proxies do not follow animated source doors
or make the imported houses semantically editable.

The character browser check passes model loading, real keyboard/view controls,
wheel zoom, sprinting, one jump per press, actual source stair ascent, solid-wall
blocking, and rejected-shader position retention. The source collision probe
settles and walks from 12 sampled town positions without hanging above the floor.
In isolated Chrome 155 at 1280 × 720, the final town starting view submits 102
frames over 4.04 seconds, about 25.2 fps; this is a short local submission-rate
measurement, not GPU execution time or a 60 fps qualification. Private captures
and reports remain outside Git. The check closes its browser context when done.
Run it with `AF_CAPTURE_DIR` set to a private directory through
`openagents browser run -- node tools/verify-character.mjs`. Optional
`AF_CHARACTER_ROUTES` supplies private source-route fixtures;
`AF_VERIFY_CHARACTER_SHADER=1` adds the shader rejection regression.

The public [Medieval Town demo](https://openagents.com/demo/medieval-town) now
selects Druid destruction release `d97b1a501684c175224`, built from engine revision
`97b1a501684cf5e6b60ecb35e490c6af076fd5cc` with the unchanged version 7 host SDK.
Its compact play controls expose Meteor Shower, Lightning Bolt, Thunderbolt,
restoration, perspective changes, and movement hints. Prepared day/night cell
manifests and matching collision ownership are part of the immutable asset set.
The [deployment record](../../../scripts/deploy/aethyr/README.md#published-druid-destruction-release-2026-10-23)
pins assets, images, revisions, checks, and rollback. Earlier character-only
measurements remain in that document as historical observations.

The isolated browser checks exercise cottage entry/collision, reticle wall
removal, undo/redo, townhouse stairs, local draft reopening, failed shader
retention, and real PNG download. Native kernel checks also cover occupied-floor
removal and allocation-pressure atomicity.
Screenshot requests briefly reserve rendering while the GPU queue drains, so
normal frame submissions cannot starve a requested capture. The simulation and
requested screenshot pose remain independent of that readback.
World animation starts after the first prepared frame, so asset loading does not
advance particles. Internal preview checkpoints preserve world age across worker
reloads; shader edits and screenshot-time overrides do not reset it. An isolated
browser check covered a delayed cold load, failed shader retention, a genuine
worker restart, and a default screenshot after a restored 25-hour session. The
external OpenAgents checkpoint remains camera-only. Explicit screenshot times
retain their separate 24-hour bound; live sessions have an explicit one-year
bound rather than silently freezing at 24 hours. The gallery images' original
particle ages remain unknown.

An optional owned foliage transmission profile attenuates direct backlighting
with a bounded forward-scattering sheet approximation. Untagged materials retain
the previous arithmetic. Source recipe checks permit only this validated owned
extension on native vegetation clones, leaving their original material and
recovered evidence unchanged. CPU bounds and packing tests pass; this optional
profile passes a six-capture browser study: affected MT18 leaf interiors move
closer to the dark reference, while an untagged control is pixel-identical. It is
selected for the private film candidate; the active town look remains unchanged.

Offline film capture now accepts transient camera poses, exact world times, and
transferable raw RGBA output, and temporary source rotations. A manual session
stops live rendering between requested frames and pins output size. Browser-side
H.264 encoding keeps sustained exports from uploading full-size raw frames.
Camera rails, full-length recording checks, and visual review are described in the
[showcase reconstruction study](references/medieval-town-video.md). This does not
establish complete camera recovery or video parity.

Run `npm run check`, `npm test`, `tools/build-kernel.sh --test`,
`node tools/build-kernel.smoke.mjs`, and the focused private-import synthetic
tests in [the import guide](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md). Run the browser checks with
`AF_CAPTURE_DIR=/private/path openagents browser run -- node tools/verify-preview.mjs`.
Browser verification uses
a separate disposable profile and private capture directory. Reference-image
parity additionally requires the fixed settings and moving-camera checks in the
reference plan; compilation or a screenshot alone cannot establish it.

Public Verse camera, sky, PBR, and height-fog implementations were consulted as
references while implementing the owned browser host. No private backend code,
credentials, vendor geometry/textures, or reference JPEGs enter this repository.

The deployment-selected `personal-planet` profile opens a finite 64-metre kit courtyard. Its exact recipe source follows kernel history; the renderer evaluates its advancing or paused day cycle, including captures. Personal edits and saved cameras stay within courtyard bounds. Focused source/type/kernel checks pass; integrated personal-world browser qualification remains pending.

The [standalone modular player](modular-player.md) shares compiled world loading,
Rust controller transport, and rendering with the preview. Its empty, room, and
character builds have audited code and asset closures; the OpenAgents host adapter
remains a separate compatibility path.

The [representative benchmark suite](representative-benchmarks.md) shares C-town
measurement contracts across active CPU populations, owned and private Town
render routes, startup, typed edits, and prior workstream evidence. Town's frame
counter opens a compact performance readout with actual submission/drop counts.

The Town editor now exposes a 100,000-creature GPU spell swarm through the shared
typed action path. It uses the existing HDR pipeline and complete 310-triangle
creature mesh. The [phase record](research/town-spell-swarm-2026-10-23.md) covers
source-command paging, shadows, lighting preparation, quality checks, and measured
limits. Sustained smooth 60+ FPS remains unqualified; this optimization phase has
ended at the owner’s request.
