# Water

Status: specification, 2026-10-06. Nothing in this document is implemented
yet unless a section says so. The phases at the end are tracked as GitHub
issues on the [OpenAgents project board](../project-board.md).

This document specifies the water system for the Verse engine: every
capability of Three.js Water Pro, which the owner asked us to match, plus the
capabilities a game world with rigid-body physics, spells, destruction, and
multiplayer needs. Everything runs in our own renderers (`verse-pbr`'s
physical renderer and the imported, or battle, renderer) and our own physics
(`crates/physics`), on every platform Verse supports: desktop `wgpu`, the
browser through WebGPU or WebGL2 (`everglade-web`), and phones down to
OpenGL ES 3.0.

## Contents

- [Sources and ground rules](#sources-and-ground-rules)
- [What Water Pro does](#what-water-pro-does)
- [What Verse has today](#what-verse-has-today)
- [Capabilities](#capabilities)
- [Architecture](#architecture)
- [Budgets per tier](#budgets-per-tier)
- [Tests and captures](#tests-and-captures)
- [Phased plan](#phased-plan)
- [Coordination with the water demos](#coordination-with-the-water-demos)
- [Open questions for the owner](#open-questions-for-the-owner)
- [References](#references)

## Sources and ground rules

Three.js Water Pro is a commercial product by DRG Software Solutions LLC
(Dan Greenheck), version 3.5.1 at the time of writing. Its license forbids
publishing its source, building competing products, and reverse
engineering. We read only its public documentation at
<https://docs.threejswaterpro.com/> and its public purchase page at
<https://threejsroadmap.com/assets/threejs-water-pro>. We never had its
source, its shaders, or its demo bundle, and nothing here is derived from
them.

That differs from the [Fire Pro port](particles.md#effects): Fire Pro is
MIT-licensed on GitHub, so commit `551e79e86a` adapted its code with
attribution. Water Pro is not, so:

- Every technique here is reimplemented from published papers, books, and
  talks, listed in [References](#references) and cited at the point of use.
- No Water Pro parameter name, default value, preset, or texture is copied
  into code. Our defaults come from physics (for example, the dispersion
  relation, Jerlov water types, and densities) or from our own tuning.
- No commit, comment, or issue claims to be a port of Water Pro. Each says
  which public technique it implements.

What we reuse from the Fire Pro port is the *engineering* pattern, not code:

1. Shared WGSL lives in its own file and is spliced into both renderers by
   `verse_pbr::shading::source`, which expands `// VERSE_SHARED_SHADING`
   (`crates/verse-pbr/src/shading.rs`).
2. Lookup tables and noise are built in Rust once (a `OnceLock`), uploaded
   as small textures, and counted in the imported renderer's admission
   (`crates/verse-pbr/src/imported/admission.rs`).
3. Quality scales by tier through a small control vector in the frame
   uniform (Fire Pro's `fire_control`, 0, 8, or 16 march steps).
4. A capture example (`crates/verse-pbr/examples/fire_capture.rs`) renders
   fixed before and after pictures through both renderers, and a dated
   directory under `bench/verse/` holds the pictures, `capture.json`, and
   `validation.json`.
5. A unit test parses and validates every expanded shader for both the GLES
   and the default variant with `naga`.

## What Water Pro does

This inventory was taken from every page of the documentation on
2026-10-06. `D` stands for `https://docs.threejswaterpro.com`. Water Pro is a
single infinite ocean built on Three.js's WebGPU renderer and its shading
language (TSL), with WebGL2 as an automatic fallback.

### Water bodies

| Capability | Source |
| --- | --- |
| One infinite open ocean; calm "harbor and lake" looks come from wave tuning, not from a separate body type | `D/`, `D/guide/wave-tuning.html` |
| Rivers and shallow-water simulation are not supported and may be a separate purchase later | Purchase page FAQ |
| No lakes, pools, waterfalls, flow maps, or vertical water | Absent from every page |

### Waves

| Capability | Source |
| --- | --- |
| FFT ocean from a JONSWAP spectrum with frequency-dependent directional spreading, in physical units and the deep-water dispersion relation ω² = gk | `D/api/waves.html`, `D/glossary.html` |
| Controls: wind speed and direction, peak wavelength, amplitude, choppiness (horizontal displacement), spectrum peak enhancement, directional sharpness, a travelling-to-standing wave ratio, gravity, animation speed | `D/api/waves.html` |
| Sea-state guidance from calm to storm by wind speed, and four tuning recipes | `D/guide/wave-tuning.html` |
| Three cascades (swell, waves, ripples) with power-of-two resolutions; the largest tile sets how far away tiling repeats; per-cascade resolution can change at run time | `D/api/waves.html`, `D/guide/quality-levels.html` |
| Gerstner waves were the swell layer in versions 2.0 to 3.2 and were removed in 3.3 | `D/changelog.html`, `D/guide/migrating-from-v3-2.html` |
| Height and normal queries at a point, with an iterated inversion of horizontal displacement so the query tracks steep crests | `D/api/water-system.html` |

### Surface shading

| Capability | Source |
| --- | --- |
| Water color from physical constituents (algae, silt, dissolved stain) seeded from ten Jerlov water types, or custom per-channel Beer–Lambert absorption and a transmission color | `D/api/color.html` |
| Transparency only from absorption and Fresnel; no global opacity | `D/api/color.html`, `D/guide/migrating-from-v2.html` |
| Full dielectric Fresnel above and below the surface, giving Snell's window and total internal reflection | `D/api/water-system.html` (fresnel) |
| Screen-space refraction offset by the surface normal | `D/api/water-system.html` (fresnel) |
| Ambient surface foam, whitecaps from the surface's Jacobian with persistence and decay, wind-stretched foam, shoreline foam within a depth range, four foam textures or a custom one | `D/api/foam.html` |
| Whitecap energy kept in a camera-anchored foam field whose resolution scales with quality | `D/api/foam.html` |
| Screen-space reflections blended with a prefiltered sky reflection whose blur follows wave roughness and pixel footprint | `D/api/ssr.html`, `D/guide/custom-sky.html` |
| Subsurface scattering: light through wave crests seen against the sun | `D/api/sss.html` |
| Sun glints ("sparkle") with a near and far fade | `D/api/sparkle.html` |
| Detail and reflection sharpness fade where waves are too small to resolve | `D/changelog.html` (3.3) |
| A meniscus effect where the surface crosses the camera's near plane, with highlight and refraction | `D/api/waterline.html` |

### Underwater

| Capability | Source |
| --- | --- |
| Underwater fog as per-channel Beer–Lambert from the water color, continuous across the waterline, with a tint | `D/api/underwater.html` |
| Underwater screen distortion | `D/api/underwater.html` |
| Screen-space sun shafts at reduced resolution, centred on the refracted sun | `D/api/sun-shafts.html` |
| Ambient suspended particles, visible only when submerged | `D/api/particles.html` |
| A procedural sea floor (fractal displacement, sand and rock blend, custom textures) | `D/api/ocean-floor.html` |
| Caustics on the floor: two scrolling cellular-noise layers distorted by the wave normals, attenuated with depth, blocked by the sun's shadow map | `D/api/ocean-floor.html` |
| One switch that turns off all underwater work for above-water projects | `D/api/underwater.html` |

### Interaction

| Capability | Source |
| --- | --- |
| Buoyancy: height and tilt of up to about 25 multi-point objects (or 128 single points) from GPU readback with smoothing; kinematic, with no forces or mass | `D/api/buoyancy.html`, `D/guide/floating-objects.html` |
| Wakes: a dispersive height field (iWave) around the camera, Kelvin-shaped wakes from up to 16 moving generators, foam on breaking crests | `D/api/wake.html`, `D/guide/wake.html` |
| Spray: billboard plumes when a probe's closing speed on the surface crosses a threshold, both for objects hitting water and waves hitting objects; WebGPU and high quality only | `D/api/spray.html`, `D/guide/spray.html` |
| Rain: wind-tilted streak particles and analytic rain-ripple normals on the surface | `D/api/rain.html` |
| Masking water out of boat hulls and similar volumes through a screen-space mask | `D/api/masking.html`, `D/guide/water-masking.html` |
| No click or touch ripples, no splashes from particles, no wet surfaces | Absent from every page |

### Geometry, quality, and integration

| Capability | Source |
| --- | --- |
| A camera-centred clipmap with level-of-detail rings, or a fixed position | `D/api/water-system.html` |
| Five quality levels (low, medium, high, ultra, max) that set cascades, mesh density, scene color resolution, foam field size, and which effects exist; every effect can still be toggled at run time | `D/guide/quality-levels.html` |
| Eight look presets that leave the sky alone | `D/guide/presets.html` |
| Atmospheric distance fog with a sky blend | `D/api/fog.html` |
| A sun light the library owns, environment intensity, and a pluggable sky provider (environment maps only since 3.0) | `D/api/sun.html`, `D/guide/custom-sky.html` |
| Post-processing node with underwater fog, distortion, sun shafts, and the rain composite | `D/guide/post-processing.html` |
| Transparent objects without registration; one blended layer captured underwater | `D/guide/transparent-objects.html` |
| Deterministic mode: a seed, a fixed step, and an O(1) jump to any tick; time folds on an 8,192 s period; not bit-exact across GPUs, so floating objects must be networked by the game | `D/guide/multiplayer.html` |
| WebGPU features that WebGL lacks: spray; everything else has a WebGL path | `D/api/spray.html`, `D/changelog.html` |
| Performance guidance: disable underwater work above water, keep masks simple, compile shaders before the first frame; no published frame-time numbers | Changelog and guides |

## What Verse has today

There is no water system. Measured at `e0e737a3b9`:

- **Everglade's water is four flat discs and a ribbon.** `layout::PONDS`
  (`crates/verse-zone-everglade/src/zones/everglade/layout.rs`) lists
  Lantern Pond, Reed Pond, the Thinking Pond, and the Fern Pond as a center
  and a radius. `layout::STREAM` is Glade Run, eight points with a half width
  of 1.1 m. `draw.rs` builds a 32-segment disc and a ribbon 5 cm
  (`WATER_LIFT`) above the ground, as an opaque, dark, glossy
  `TexturedMaterial` (roughness 0.06) in the baked textured path. Its only
  reflection is the prefiltered sky cube every surface gets.
- **Nobody swims.** `layout::pond_blockers` puts a cross of two boxes in
  each pond, so the player stops at the bank. The stream is shallow and the
  player walks straight across it on dry-land rules. Ducks sit at
  `WATER_LIFT` (`wildlife.rs`). Pond beds are not carved: the heightfield
  (`verse_world::social::everglade::height`) runs flat under the water.
- **The renderer has no scene color copy, refraction, planar reflection, or
  screen-space reflection.** The scene target is multisampled, so its depth
  cannot be sampled. Only the high tier has a readable single-sample depth
  prepass, which feeds ambient occlusion and contact shadows
  (`pbr/screen.rs`). For the same reason, particles have no soft depth fade.
- **Every backend runs under WebGL2 limits.** `crates/verse/src/render.rs`
  requests `wgpu::Limits::downlevel_webgl2_defaults()` with eight
  inter-stage varyings on every backend. No shader uses compute or storage
  buffers. The low tier (`verse_engine::quality::Tier::Low`) is WebGL2,
  OpenGL ES, or a device without a floating-point scene target; medium is
  phones and WebGPU in a browser; high is a desktop with compute and four
  samples.
- **The physics has no fluid.** `crates/physics` has rigid bodies in `f64`,
  sphere, capsule, and cuboid colliders, a sequential-impulse contact
  solver, joints, XPBD rope, island sleep, a momentum ledger, restorable
  serialized state, and replay traces, stepped at 120 Hz. A `Field` gives
  an acceleration from a position and a velocity, which is enough for a
  linear drag but not for buoyancy, which depends on shape. The Genesis
  roadmap explicitly left SPH and fluid coupling unported.
- **Spells push through areas.** `verse_world::spells::fields` has
  `Area::{Box, Cylinder, Wall, Sphere}` and `SpellField`s summed into the
  spell world's field. The chamber's spell catalog has Wind Wall, Gust of
  Wind, Meteor Swarm, Reverse Gravity, Levitate, Feather Fall, Telekinesis,
  Black Tentacles, and Wall of Stone. The Grove's druid kit adds Ice Knife,
  Ice Storm, Ray of Frost, Call Lightning, Thunderbolt, and Thunderwave
  (`crates/verse-zone-grove/src/zones/grove/kit.rs`). Nothing freezes a
  surface.
- **Destruction already has two debris tiers.** Gameplay chunks larger than
  0.3 m are host-simulated and replicated; cosmetic chunks are client-only
  from a replicated seed ([destructible buildings](destructible-buildings.md)).
  Glade Run's footbridge, jetties, and rowboats already break.
- **Particles are CPU-simulated sprites.** `verse_core::fx::Particles`
  holds up to 4,096 particles in 256 effects, deterministic by seed, drawn
  by one vertex-buffer sprite pipeline that runs on every tier
  ([particles](particles.md)). There are no water effects yet.
- **There is no weather.** No rain, wind state, or wetness exists in any
  zone.

## Capabilities

Each row says where the capability comes from (**WP** for Water Pro, **New**
for ours), which tiers draw it, and whether it affects gameplay (**Sim**,
simulated and authoritative) or only looks (**Visual**). The tiers are
`verse_engine::quality::Tier`'s Low, Medium, and High.

### Water bodies

| # | Capability | From | Tiers | Kind |
| --- | --- | --- | --- | --- |
| B1 | **Pond and lake**: a closed outline at a fixed level over a carved bed | New (WP covers calm water only as an ocean look) | All | Sim |
| B2 | **River and stream**: a spline with width, a sloped level along its course, and a flow field | New (WP lists rivers as unsupported) | All | Sim |
| B3 | **Waterfall and cascade**: a vertical or steep sheet between two levels, with plunge pool foam and mist; Glade Run's weir is the first | New | All | Visual, plus a current at the base |
| B4 | **Ocean and coast**: an unbounded surface with spectral waves, a shoreline, and surf | WP | All, detail by tier | Sim for swell, Visual for detail |
| B5 | **Pool and basin**: a small rectangular or authored body inside architecture (fountains, baths, cauldrons, cisterns) | New (WP only masks pools out) | All | Sim |
| B6 | **Puddle**: a decal-like shallow body that rain fills and the sun dries | New | All | Visual |
| B7 | **Swamp and marsh**: shallow, murky water over walkable mud with reeds | New | All | Sim (wading) |
| B8 | **Ice**: any body, or part of one, frozen into a walkable sheet | New | All | Sim |

### Waves and motion

| # | Capability | From | Tiers | Kind |
| --- | --- | --- | --- | --- |
| V1 | **Gerstner waves**: a short sum of trochoidal waves, evaluated identically in Rust and WGSL, for gameplay swell and as the floor on every tier [Fournier86, Finch04] | WP (removed there in 3.3), kept here | All | Sim |
| V2 | **Spectral (FFT) waves**: a Tessendorf height and choppy-displacement field from a JONSWAP spectrum with directional spreading [Tessendorf01, Hasselmann73, Horvath15] in up to three cascades | WP | Low: one baked tile; Medium: two; High: three | Visual |
| V3 | **Wave controls**: wind speed and direction, fetch, peak wavelength, amplitude, choppiness, spread, standing-wave ratio, gravity, time scale | WP | All | Data |
| V4 | **Point queries**: height, normal, and surface velocity at a point and time, with displacement inversion | WP | CPU everywhere | Sim |
| V5 | **Flow maps**: two-phase flow-map advection of normals and foam so rivers visibly run around rocks and bends [Vlachos10] | New | All | Visual, with the same field driving currents (P3, P4) |
| V6 | **Wind ripples on still water**: a small tiling detail layer whose strength follows the wind | New | All | Visual |
| V7 | **Shoaling and surf**: waves slow, steepen, and break as depth falls (shallow-water dispersion ω² = gk·tanh(kh)) | New (WP is deep water only) | Medium, High | Visual |
| V8 | **Deterministic time**: wave state is a function of the world tick and a seed, so any client can jump to any tick in O(1) | WP | All | Sim |

### Shading

| # | Capability | From | Tiers | Kind |
| --- | --- | --- | --- | --- |
| S1 | **Fresnel** with an index of refraction of 1.33 above and below the surface, Snell's window and total internal reflection from below [Schlick94] | WP | All | Visual |
| S2 | **Sky reflection** from the existing prefiltered sky cube, its level picked by wave roughness and pixel footprint | WP | All | Visual |
| S3 | **Planar reflection** for flat bodies (ponds, pools, calm lakes): the scene mirrored at reduced resolution, one plane at a time | New (WP has none) | Medium (half resolution, one plane), High | Visual |
| S4 | **Screen-space reflection** for waves and anything a planar mirror misses [McGuire14] | WP | High | Visual |
| S5 | **Refraction** of what lies under the surface, offset by the normal | WP | Medium, High | Visual |
| S6 | **Depth color and absorption**: per-channel Beer–Lambert through the water column, from Jerlov water types or a custom absorption [Jerlov76] | WP | All (Low from baked depth) | Visual |
| S7 | **Subsurface scattering** through thin crests against the sun | WP | All | Visual |
| S8 | **Sun glints** with distance fade | WP | All | Visual |
| S9 | **Foam**: shoreline foam from a baked distance to the shore; whitecaps from the surface's Jacobian with persistence [Tessendorf01]; contact foam around rocks, piers, and floating bodies; foam advected by the flow | WP, plus contact and flow foam | All; persistence on Medium, High | Visual |
| S10 | **Caustics** on any surface under water (beds, piers, swimmers, sunken debris), not only a built-in floor [Guardado04, Wallace16] | WP (floor only), extended | Low: baked flipbook; Medium, High: from the live normals | Visual |
| S11 | **Detail fade**: normal detail and reflection sharpness fade where waves are smaller than a pixel | WP | All | Visual |
| S12 | **Waterline meniscus**: the line, highlight, and refraction where the surface crosses the near plane | WP | Medium, High | Visual |
| S13 | **Masking**: keep water out of boat hulls, barrels, and diving bells | WP | Medium, High (stencil); Low (hull geometry above the surface) | Visual |
| S14 | **Murk and turbidity** stirred up by movers in shallow water, settling over seconds | New | Medium, High | Visual |
| S15 | **Night and emissive water**: moon and lamp reflections, bioluminescent glow where disturbed | New | All | Visual |
| S16 | **Ice and frost material**: the frozen state of S1–S9, with cracks and trapped bubbles | New | All | Visual |

### Underwater

| # | Capability | From | Tiers | Kind |
| --- | --- | --- | --- | --- |
| U1 | **Camera submersion** from a CPU query, with a smooth transition | WP | All | Visual |
| U2 | **Underwater fog** as per-channel Beer–Lambert, continuous with S6 | WP | All | Visual |
| U3 | **Split waterline**: above and below shaded correctly when the near plane straddles the surface | WP | All (Low without refraction) | Visual |
| U4 | **Distortion** of the view | WP | Medium, High | Visual |
| U5 | **Sun shafts** through the surface | WP | Medium (low sample count), High | Visual |
| U6 | **Suspended motes** through the existing sprite pipeline | WP | All | Visual |
| U7 | **Sound**: a low-pass on the mix and an underwater ambience | New | All | Visual (audio) |
| U8 | **Breath and drowning** per the SRD's suffocation rule [SRD51] | New | All | Sim |

### Interaction and physics

| # | Capability | From | Tiers | Kind |
| --- | --- | --- | --- | --- |
| P1 | **Buoyancy with forces**: Archimedes' force at the center of the submerged volume of each sphere, capsule, and cuboid collider, from its density, so wood floats, stone sinks, and a plank floats on edge correctly [Kerner15] | WP (kinematic only), extended | CPU everywhere | Sim |
| P2 | **Drag and added damping**: linear and quadratic drag relative to the local water velocity, with angular damping, scaled by the submerged fraction | New | CPU | Sim |
| P3 | **Currents**: rivers carry bodies and swimmers along the flow field | New | CPU | Sim |
| P4 | **One flow field for looks and physics**: the field the shader advects with (V5) is the field the physics samples | New | CPU and GPU | Sim |
| P5 | **Swimming and wading for players and NPCs**: wade, swim on the surface, dive, climb out, tread water, with SRD movement costs [SRD51] | New | All | Sim |
| P6 | **Floating debris**: building pieces that fall into water float or sink by material, drift downstream, and pile against obstacles | New | CPU | Sim (gameplay chunks), Visual (cosmetic chunks) |
| P7 | **Interactive ripples**: a damped height field around the camera that every mover, impact, and spell writes into [Tessendorf04, Bridson07] | WP (wakes), extended to all movers | All (CPU), larger window on High | Visual |
| P8 | **Kelvin wakes** behind boats, swimmers, ducks, and drifting debris | WP | All | Visual |
| P9 | **Foam trails** behind movers that persist and drift with the flow | New | All | Visual |
| P10 | **Splashes and spray**: entry splashes scaled by closing speed and mass, crest spray against rocks, droplets, mist | WP (WebGPU only there), on every tier here | All | Visual |
| P11 | **Floating wildlife and props**: ducks, lily pads, rowboats, and barrels bob on the real surface | New | All | Visual (from P1 or W4) |
| P12 | **Fire meets water**: burning bodies that enter water are extinguished with steam | New | All | Sim |
| P13 | **Boats**: a rowboat a player can board and row, as a buoyant body with oar impulses | New | All | Sim |

### Spells and water

| # | Capability | From | Kind |
| --- | --- | --- | --- |
| X1 | **Wind Wall** over water whips a band of spray and short steep ripples along the wall, and blocks floating bodies like a solid | New | Visual, plus the existing wall field |
| X2 | **Gust of Wind** drives a fan of ripples and pushes floating bodies along its line | New | Sim |
| X3 | **Meteor Swarm** into water throws a crown splash and a ring wave, flashes to steam, and leaves the burst's fire out on the water | New | Visual, plus the existing damage |
| X4 | **Thunderbolt and Call Lightning** on water flash across the surface and conduct to swimmers within a radius | New | Sim (damage rule pending the owner) |
| X5 | **Ray of Frost, Ice Knife, and Ice Storm** freeze a patch of water into walkable ice for a duration; bodies in the patch lock in place; a frozen patch breaks under heavy impacts | New | Sim |
| X6 | **Reverse Gravity** lifts water inside its cylinder into a column of globes and spray, and floating bodies rise with it; the water falls back when the spell ends | New | Visual, plus the existing field on bodies |
| X7 | **Thunderwave** pushes a ring wave out from the caster and shoves floating bodies | New | Sim |
| X8 | **Telekinesis and Levitate** lift bodies out of water with dripping and a pull-out splash | New | Visual |
| X9 | **Wall of Stone** across a stream dams it: the flow routes around the wall and the level behind it rises a little | New | Sim (flow field update) |
| X10 | **Black Tentacles** rise out of water with ripples and murk | New | Visual |

### Weather and wetness

| # | Capability | From | Kind |
| --- | --- | --- | --- |
| R1 | **Rain particles**: wind-tilted streaks around the camera | WP | Visual |
| R2 | **Rain ripples** on every water surface, analytic in the shader | WP | Visual |
| R3 | **Puddles** that fill in low ground during rain and dry afterwards | New | Visual |
| R4 | **Rising water**: sustained rain raises pond and stream levels by a small bounded amount and strengthens the flow | New | Sim (bounded) |
| R5 | **Wet surfaces**: rain-darkened, glossier ground, roofs, and props, with streaks on vertical surfaces [Lagarde13] | New | Visual |
| R6 | **Wet characters**: anyone who swims or stands in rain darkens and drips for a while | New | Visual |
| R7 | **Splash-back** where rain hits hard ground, and drips from eaves | New | Visual |

### World scale and multiplayer

| # | Capability | From | Kind |
| --- | --- | --- | --- |
| M1 | **Streaming across 8 m cells**: water meshes, flow maps, and shore distance fields stream with the cells they cover | New | Engine |
| M2 | **Clipmap ocean**: camera-centred level-of-detail rings for unbounded water [Losasso04] | WP | Engine |
| M3 | **Simulated versus visual split**: the swell, levels, currents, ice, and buoyancy of gameplay bodies are authoritative; detail waves, ripples, foam, spray, caustics, and cosmetic debris are local | WP (determinism), extended | Engine |
| M4 | **Presets** as data: named looks from Jerlov water types, wind, and foam (clear pond, murky pond, mountain river, tropical coast, northern sea, storm, swamp, moonlit) | WP | Data |
| M5 | **Quality tiers** with a correct, plainer frame when a tier drops an effect | WP | Engine |

## Architecture

### Where the code lives

| Piece | Home | Why |
| --- | --- | --- |
| Water bodies, the surface model, point queries, flow fields, buoyancy, drag, and currents | `crates/physics/src/water/` (new module) | Generic mechanisms belong in `physics`, which has no renderer, I/O, or zone knowledge. The surface query must be the same code the physics steps with. |
| Water body records, presets, and spell interactions | `crates/verse-world` (`water.rs`, and spell hooks in `spells/`) | Rules and authority live with `SpellWorld` and the chamber's 120 Hz clock. |
| Swimming and wading | `physics::character` plus `verse-world` movement | The character controller already owns ground, gravity override, and push speed. |
| Shaders and GPU resources | `crates/verse-pbr/src/water/` with `water.wgsl`, spliced by `shading::source` | The Fire Pro pattern: one shared module for the physical and imported renderers. |
| CPU wave synthesis (FFT), ripple field, foam field | `crates/verse-pbr/src/water/` on a worker thread, or `verse-engine` if a second renderer needs it | Visual-only state, produced on the CPU so it works without compute. |
| Water effects (splashes, spray, mist, steam, rain, drips) | `assets/verse/fx/effects/*.toml`, sheets rendered by `scripts/blender/fx/` | The existing sprite pipeline runs on every tier. |
| Zone water | Each zone crate's layout (for example, Everglade's `layout::water`) | Layouts are Rust data today; water bodies follow suit. |

### The data model

A zone declares its water as a list of bodies. Proposed shape, in
`physics::water`:

```rust
pub struct WaterBody {
    pub id: WaterId,
    pub kind: BodyKind,          // Pond, River, Waterfall, Ocean, Pool, Puddle, Marsh
    pub outline: Outline,        // closed polygon, or a river spline with widths
    pub level: Level,            // constant, or a profile along a river's course
    pub bed: Bed,                // the depth below the level, from the zone heightfield
    pub waves: WaveSet,          // Gerstner terms plus an optional spectrum seed
    pub flow: Option<FlowField>, // 2D velocity on a grid in the body's frame
    pub density: f64,            // 1000 kg/m³ fresh, 1025 salt
    pub preset: PresetId,        // look only; never read by the physics
}
```

- **Outlines and levels are authored**; beds come from the zone's
  heightfield, so the bed and the walking ground are the same surface.
  Everglade's ponds become carved bowls in `height()` (a smooth radial
  profile to a depth set per pond), and the bank blockers go away when
  swimming lands.
- **The flow field** is a coarse grid (0.5 m cells for streams, 2 m for
  rivers) of 2D velocities. By default a build step generates it from the
  river spline and the obstacles inside the outline by solving for a
  divergence-free potential flow around them (the stream function on the
  grid, with the banks and rocks as boundaries). An artist can override it
  with a Blender-painted flow map (red and green channels as velocity),
  exported by a new `scripts/blender/flowmap.py`, which follows the
  [Blender pipeline](blender-pipeline.md). Either way the grid is checked in
  next to the zone pack and digested with it.
- **Derived data is baked at zone build time**: a per-body shore distance
  field (for shoreline foam on every tier), the per-vertex depth (for
  absorption on Low), and a tiling caustic flipbook per preset (for Low).
- **Presets** are TOML files in `assets/verse/water/presets/`, like effect
  files, with absorption from a Jerlov type or explicit coefficients, a
  scattering color, foam textures, and wind defaults. The physics never
  reads a preset.

### The surface model and determinism

The surface has two layers with different contracts:

1. **The gameplay surface** is the body's level plus a sum of at most eight
   Gerstner waves [Fournier86, Finch04], plus, for an ocean, the lowest FFT
   cascade synthesized on the CPU from a seeded spectrum. It is a pure
   function of the body, the seed, and the world tick: `t = tick × dt`,
   folded on a period that is a multiple of every wave's period (each
   wave's angular frequency is snapped to a multiple of 2π over the period,
   as Tessendorf suggests for looping). `physics::water::Surface::sample(x,
   z, tick)` evaluates it in `f64` and returns height, normal, and surface
   velocity, inverting the horizontal displacement with a few fixed-point
   iterations. The host steps buoyancy, swimming, and currents with it.
2. **The visual surface** adds the higher FFT cascades, wind ripples, flow
   detail, the ripple field, and rain. Each client produces it locally.
   Nothing gameplay-relevant reads it.

The shader evaluates the same Gerstner terms in `f32` from the same
uniform data, so a floating barrel sits on the drawn wave. A test holds the
WGSL and the Rust evaluation within a tolerance (see
[Tests and captures](#tests-and-captures)).

This follows the [destruction](destructible-buildings.md) and
[networking](networking.md) decisions: host authority with snapshots, not
lockstep, and seeded cosmetics that are never replicated. Water adds no new
replicated stream. Bodies that float are ordinary physics bodies whose
poses replicate as they do today; the wave state needs only the tick, the
seed, and each body's parameters, which ship with the zone. Ice patches,
level changes, and dams are host events with a start tick and a duration.

### Simulation: CPU everywhere, GPU where it pays

Phones on OpenGL ES 3.0 and WebGL2 have no compute shaders or storage
buffers, and every Verse backend currently runs under WebGL2 limits, so the
baseline is CPU simulation plus texture uploads, which every tier supports:

| Simulation | Where | Size | Per frame |
| --- | --- | --- | --- |
| Gerstner terms | CPU (physics) and vertex shader | ≤ 8 waves a body | Uniforms only |
| FFT cascades | CPU worker thread with a radix-2 FFT, uploaded as `Rgba16Float` displacement and `Rg16Float` slope textures (half-float textures are filterable on OpenGL ES 3.0; rendering to them is not required) | Low: none at run time, a baked looping tile; Medium: 2 × 64²; High: 3 × 128², or 3 × 256² with compute | Medium about 0.3 ms of a worker, High about 1 ms (to measure) |
| Ripple field | CPU, damped wave equation on a camera-centred grid with a fixed step [Bridson07]; iWave's dispersive kernel on High [Tessendorf04] | Low 64² at 0.25 m (16 m); Medium 128² at 0.2 m; High 256² at 0.15 m | One `R16Float` upload |
| Foam field | CPU, decays and advects with the flow | Same grid as ripples, `R8Unorm` | One upload |
| Buoyancy, drag, currents | CPU in `physics`, at 120 Hz | Per collider | See budgets |

A later High-tier option moves the FFT and ripples to compute shaders when
the renderer requests desktop limits on capable adapters. That is an owner
decision (see [Open questions](#open-questions-for-the-owner)) because it
splits the code path.

The CPU FFT is our own code or a reviewed, permissively licensed crate
(`rustfft` is MIT or Apache-2.0); the choice is made in the FFT phase
against wasm size and determinism. The FFT output is visual only, so
floating-point differences across machines are harmless.

### Rendering passes

Water draws after the opaque scene and before transparent particles:

1. **Opaque scene** (existing).
2. **Planar reflection** (Medium and High, new): before the scene pass, the
   nearest visible flat body's plane mirrors the camera; the scene draws
   into a half-resolution (Medium) or full-resolution (High) target with a
   clip plane, without particles, shadows, or screen-space effects.
3. **Scene color and depth copy** (Medium and High, new): the multisampled
   scene resolves into a single-sample color copy and a readable depth copy
   for refraction, depth absorption, contact foam, soft particles, and SSR.
   This also fixes the particles' missing soft depth fade. It is the
   largest structural change, so it lands as its own phase.
4. **Water surface**: the water mesh, depth-tested against the scene, with
   the shared `water.wgsl`. Front faces shade from above; back faces shade
   from below (Snell's window and total internal reflection).
5. **Underwater pass** (all tiers, inside the existing output pass): when
   the camera is submerged or the near plane straddles the surface, the
   output shader applies Beer–Lambert fog by view distance, a per-pixel
   above or below split from the surface height at the near plane, and on
   Medium and High distortion and sun shafts.
6. **Transparent particles** (existing), now with soft fade on Medium and
   High.

Caustics are applied inside the existing `photo.wgsl` and `scene.wgsl`
surface shading: for any fragment below a body's level and inside its
outline (a small uniform list of nearby bodies, at most eight), the shader
adds a caustic term from the live normal textures (Medium, High) or the
baked flipbook (Low), attenuated by depth and by the sun's shadow.

### Shaders per tier

`water.wgsl` follows `fire.wgsl`: one module expanded by
`verse_pbr::shading::source`, with `//#if GLES` blocks where GLSL ES 3.00
differs, and a `water_control` vector in the frame uniform that sets what a
tier draws. No feature uses `noperspective`, `textureNumLevels`, a depth
texture read with and without comparison, or more than eight inter-stage
varyings.

| Feature | Low (WebGL2, GLES 3.0) | Medium (phones, WebGPU) | High (desktop) |
| --- | --- | --- | --- |
| Mesh | Per-body mesh, 1–2 m spacing; clipmap with 3 rings for oceans | 0.5–1 m; 4 rings | 0.25–0.5 m; 5 rings |
| Displacement | Gerstner in the vertex shader | Gerstner plus 2 FFT cascades | Gerstner plus 3 FFT cascades |
| Normals | Baked looping FFT normal tile plus Gerstner | Live FFT slopes plus flow-advected detail | Same, with Jacobian whitecaps |
| Reflection | Sky cube | Sky cube plus one half-resolution planar mirror | Planar mirror plus SSR plus sky cube |
| Refraction | None; color from absorption over baked depth | Scene color copy, normal-offset | Same, with chromatic offset |
| Shore and contact foam | Baked shore distance | Plus depth-copy contact foam | Same |
| Persistent foam and trails | Foam field (CPU) | Foam field | Foam field |
| Ripples | 64² field | 128² field | 256² field |
| Caustics | Baked flipbook | From live normals | From live normals, two layers |
| Underwater | Fog and split waterline | Plus distortion and 8-sample shafts | Plus 16-sample shafts and meniscus |
| Rain | Ripples and streaks | Same, plus puddles | Same, plus wet streaks |

Every effect a tier drops leaves the frame correct and plainer, as
`quality.rs` requires.

### Coupling with `crates/physics`

New module `physics::water`:

- `trait Water` with `sample(x, z, tick) -> Option<Sample>` (height,
  normal, surface velocity, flow velocity, density, body id) and
  `bodies_overlapping(aabb)`. A zone's `WaterSet` implements it with a
  uniform grid over the zone so a lookup is O(1).
- `fn submerged(shape, pose, water) -> Submersion` returns the submerged
  volume, its centroid, and the wetted area for each `collision::Shape`:
  an exact spherical cap for spheres, a capsule as a cylinder and two caps,
  and a cuboid clipped against the local surface plane at its footprint,
  sampled at the four bottom corners on rough water (Kerner's
  triangle-clipping approach applied to boxes) [Kerner15].
- `fn apply(world, water, tick, dt)` adds, for each awake dynamic body:
  buoyancy ρ·g·V at the center of buoyancy; linear and quadratic drag on
  the velocity relative to the flow, scaled by the wetted fraction; and
  angular damping. Each force enters the momentum ledger as a named
  external term (`water.buoyancy`, `water.drag`), so the conservation tests
  stay meaningful.
- Body density comes from mass over collider volume, so the destruction
  rig's existing masses decide what floats: wood (about 500–700 kg/m³)
  floats, stone and brick sink.
- Sleep: a floating body rocking on waves never sleeps. A body on a still
  pond sleeps under the existing island rules once it settles, and wakes
  when a ripple source or contact touches its island.
- The character controller gains a medium: `Ground`, `Wading` (water above
  0.4 m of the 1.8 m body), `Swimming` (above 1.25 m, or no ground), and
  `Diving`. Swimming replaces gravity with buoyancy toward a float line,
  applies the flow, and doubles movement cost per the SRD's swimming rule
  [SRD51]. Breath follows the SRD's suffocation rule (holding breath for
  1 + Constitution modifier minutes, at least 30 seconds), shortened by an
  owner-chosen game scale if needed.

### Spells and destruction

Spells interact through the same two layers:

- **Authoritative effects** go through `SpellWorld`: Gust of Wind and
  Thunderwave add a short field over floating bodies; Wall of Stone in a
  river marks flow-grid cells as solid and re-solves the stream's local
  potential flow (bounded to the cells near the wall); freezing adds an ice
  patch (a static collider over the frozen region, at the surface height,
  with a start tick and duration) and pins bodies whose centers lie inside;
  lightning applies its damage rule to characters with a `Swimming` or
  `Wading` medium within a radius of the strike's surface point.
- **Visual effects** go through the client: impacts write impulses into the
  ripple field and foam field and spawn splash, spray, and steam effects;
  Reverse Gravity lifts a column of water as particles and a raised,
  pinched surface region inside its cylinder; Meteor Swarm landing in water
  swaps its ground fire for steam and a crown splash.
- **Destruction**: when the footbridge or a jetty breaks, gameplay chunks
  become buoyant bodies in Glade Run and drift along the flow until they
  lodge against the bank or a rock; cosmetic chunks float on the client
  only, within the existing per-tier cosmetic chunk budget. Burning debris
  that enters water is extinguished (P12).

### Streaming across 8 m cells

- Water meshes are cut on the same 8 m cells as the static geometry, so
  they cull and stream with them; an ocean uses its clipmap instead.
- A body's record (outline, levels, Gerstner terms, flow grid, shore field)
  is small and stays resident for the whole zone, because the physics
  queries it anywhere. Its GPU textures (flow map, shore field, depth)
  stream with the cells they overlap.
- The ripple and foam fields are camera-centred and scroll in whole cells
  of their own grid, so they never resample.
- Streaming budgets follow the existing residency rules in
  `verse_engine::streaming`; water textures count toward the zone's
  resident bytes.

## Budgets per tier

These are targets to measure in the measurement phase, not results. A
budget overrun draws less, never fails, as `quality::Overrun` does today.

| Budget | Low | Medium | High |
| --- | --- | --- | --- |
| GPU time for water, 1080p equivalent | ≤ 1.5 ms | ≤ 2.5 ms | ≤ 4 ms |
| Extra passes | None | Resolve copy, half-resolution mirror | Resolve copy, mirror, SSR |
| GPU memory for water | ≤ 8 MiB | ≤ 32 MiB | ≤ 96 MiB |
| Main-thread CPU for water per frame | ≤ 0.3 ms | ≤ 0.5 ms | ≤ 0.8 ms |
| Worker CPU for FFT and fields | ≤ 0.5 ms | ≤ 1 ms | ≤ 2 ms |
| Buoyant bodies simulated (host) | 32 | 64 | 128 |
| Cosmetic floating chunks | 32 | 64 | 256 |
| Ripple and wake sources per frame | 8 | 16 | 32 |
| Water fx particles (within the sprite budget of 160, 768, or 1,536) | 64 | 256 | 512 |
| Bodies in the shader's caustic and underwater list | 4 | 8 | 8 |

The physics budget stays inside the existing 1 ms per 120 Hz step on the
slowest supported phone: buoyancy is a handful of plane clips per body.

## Tests and captures

- **Shader validation.** `water.wgsl` and every shader it is spliced into
  join `shading::tests` and the `SHADERS` table in
  `crates/verse/src/gles_tests.rs`, so both variants parse, validate, and
  translate to GLSL ES 3.00 and Metal on any development machine.
- **Surface parity.** A test renders the Gerstner displacement of a fixed
  set of points to a small float target and compares it with
  `physics::water::Surface::sample` at the same tick, within 1 mm.
- **Spectrum.** The synthesized FFT tile's variance matches the integral of
  the input spectrum within 2%; the tile loops exactly on its period; a seed
  and a tick reproduce the same tile.
- **Physics.**
  - A cube of density 500 kg/m³ settles half submerged within 1%.
  - A sphere of density 1,100 kg/m³ sinks; one of 900 floats with the
    analytic draft.
  - A plank floats flat, not on end, and rights itself after a push.
  - Terminal sinking speed matches the quadratic-drag closed form.
  - A body dropped in Glade Run follows the flow field to the bend where
    the field sends it, in a seeded scenario with a replay trace.
  - The momentum ledger balances with the named water terms.
  - Serialized state restores bit for bit with bodies in water.
- **Character.** Walking into a pond wades, then swims; climbing out at a
  shallow bank works; breath runs out per the rule; the stream current
  pushes a swimmer at the field's speed.
- **Spells.** Scenario tests in `verse_world::spells::scenarios`: freezing
  makes a walkable patch that thaws on time; Gust pushes a floating crate;
  Wall of Stone diverts a drifting plank.
- **Captures.** `cargo run -p verse-pbr --example water_capture --
  OUTPUT_DIRECTORY` renders fixed views (a pond at noon and at dusk, Glade
  Run at the weir, the waterline half under, a debris spill in the stream)
  for every tier through both renderers, and writes `capture.json` and
  `validation.json` under `bench/verse/<date>/water/`, as the Fire Pro
  capture does. Each phase that changes the look adds a dated capture.
- **Web.** The `everglade-web` build draws the same views on WebGPU and
  with `?gl` on WebGL2, checked in the browser.

## Phased plan

Estimates are agent-hours at this repository's recent pace (for example,
the Fire Pro port landed as one change of about 900 lines with captures).
Each phase is one issue, merges on its own, and leaves every tier correct.

| Phase | Issue | Work | Blocked by | Estimate |
| --- | --- | --- | --- | --- |
| W0 | (demo agent) | Quick water demos, in progress in parallel | — | — |
| W1 | [#10773](https://github.com/OpenAgentsInc/openagents/issues/10773) | `physics::water`: bodies, Gerstner surface with deterministic time, point queries, flow grid, buoyancy, drag, currents, ledger terms; tests | — | 6 h |
| W2 | [#10774](https://github.com/OpenAgentsInc/openagents/issues/10774) | `water.wgsl` v1 for all tiers in both renderers: Gerstner mesh, Fresnel, sky reflection, absorption over baked depth, baked shore foam, glints, crest scattering, flow-advected detail, presets; capture example; GLES tests | W1 | 8 h |
| W3 | [#10775](https://github.com/OpenAgentsInc/openagents/issues/10775) | Everglade water: carved pond beds, Glade Run's level profile and generated flow field, the weir as a cascade, swimming and wading for players and NPCs, breath, bank blockers removed, floating ducks and lily pads | W1, W2 | 8 h |
| W4 | [#10776](https://github.com/OpenAgentsInc/openagents/issues/10776) | Spectral waves: CPU FFT with JONSWAP and directional spreading, cascades by tier, the baked tile for Low, whitecaps and the foam field, shallow-water dispersion and surf | W2 | 8 h |
| W5 | [#10777](https://github.com/OpenAgentsInc/openagents/issues/10777) | Scene color and depth copy, refraction, planar reflection, SSR on High, contact foam, and soft particles | W2 | 10 h |
| W6 | [#10778](https://github.com/OpenAgentsInc/openagents/issues/10778) | Interaction: ripple field, Kelvin wakes, foam trails, splash, spray, mist, and drip effects, floating debris from destruction, boats | W1, W3 | 8 h |
| W7 | [#10779](https://github.com/OpenAgentsInc/openagents/issues/10779) | Underwater: submersion, fog, split waterline, Snell's window, distortion, sun shafts, motes, caustics on all submerged surfaces, underwater audio | W2, W5 | 8 h |
| W8 | [#10780](https://github.com/OpenAgentsInc/openagents/issues/10780) | Spells and water: Wind Wall, Gust, Meteor Swarm, lightning, freezing to ice, Reverse Gravity, Thunderwave, Wall of Stone damming, fire meets water | W3, W6 | 8 h |
| W9 | [#10781](https://github.com/OpenAgentsInc/openagents/issues/10781) | Weather: rain particles, rain ripples, puddles, wet surfaces and characters, bounded rising water | W2, W6 | 6 h |
| W10 | [#10782](https://github.com/OpenAgentsInc/openagents/issues/10782) | Ocean and scale: clipmap, cell streaming of water textures, a coastal test scene, multiplayer checks of the simulated and visual split | W4, W5 | 10 h |
| W11 | [#10783](https://github.com/OpenAgentsInc/openagents/issues/10783) | Measurement: per-tier GPU and CPU timings on desktop, both web backends, and phones, budgets set from measurements, device runs recorded in `NEEDS_OWNER.md` | W7, W8, W9, W10 | 4 h |

The total is about 84 agent-hours. W1 can start at once; W2 and W4 to W7
can run in parallel lanes once W2 lands. The umbrella issue,
[#10784](https://github.com/OpenAgentsInc/openagents/issues/10784), is
blocked by every phase.

## Coordination with the water demos

Another agent is building quick water demos at the same time as this
specification. When this document was written, nothing of its work had
reached `main` (checked with `git log origin/main` on 2026-10-06). The
phases above are written so the demos are not duplicated:

- The demos are W0. They show looks quickly, likely as a demo scene or a
  capture example, without the data model, physics coupling, or tier
  fallbacks.
- Before starting W1 or W2, read the demo commits (`git log origin/main
  --grep -i water`) and adopt what fits: a demo shader becomes the
  starting point for `water.wgsl` if it validates for GLES; a demo scene
  becomes a `water_capture` view. Do not rebuild a look a demo already
  has; promote it into the shared module and add its tier fallbacks.
- If a demo changes Everglade's ponds or stream, W3 builds on that change
  rather than replacing it.

## Open questions for the owner

1. **Swimming in Everglade.** Should every pond and Glade Run become
   swimmable, replacing the bank blockers, or do some ponds stay walls for
   the social space?
2. **An ocean zone.** Water Pro is an ocean product, and Everglade has no
   sea. Should W10 build a coastal zone (or a lake large enough for swell
   and boats), and where does its portal go?
3. **Compute on desktop.** Should the High tier request compute and storage
   buffers on capable desktop adapters, which allows larger GPU FFTs and
   ripple fields, at the cost of a second code path? The plan works without
   it.
4. **Lightning on water.** What does Thunderbolt or Call Lightning do to
   swimmers: extra damage, a wider area, or only a visual? The SRD does not
   say.
5. **Freezing.** Which spells freeze water (Ray of Frost, Ice Knife, Ice
   Storm, or a new Cone of Cold), how large a patch, and for how long?
6. **Breath scale.** Use the SRD's minutes of held breath, or a game-scale
   duration?
7. **Boats.** Should rowboats be boardable and rowable in the first round
   (W6), or stay props?
8. **Weather.** Rain needs a weather state that Verse lacks. Is it a zone
   schedule, a host-wide clock, or player-triggered (for example, by a
   spell)?
9. **Water Pro itself.** Should we buy a license to compare looks
   side by side, given that its license forbids using its code in an open
   source project? This plan does not need it.

## References

- [Bridson07] Robert Bridson and Matthias Müller-Fischer, "Fluid
  Simulation for Computer Graphics," SIGGRAPH 2007 course notes (height
  field waves).
- [Finch04] Mark Finch, "Effective Water Simulation from Physical Models,"
  *GPU Gems*, chapter 1, NVIDIA, 2004.
- [Fournier86] Alain Fournier and William T. Reeves, "A Simple Model of
  Ocean Waves," SIGGRAPH 1986.
- [Guardado04] Juan Guardado and Daniel Sánchez-Crespo, "Rendering Water
  Caustics," *GPU Gems*, chapter 2, NVIDIA, 2004.
- [Hasselmann73] Klaus Hasselmann et al., "Measurements of Wind-Wave
  Growth and Swell Decay during the Joint North Sea Wave Project
  (JONSWAP)," *Deutsche Hydrographische Zeitschrift*, 1973.
- [Horvath15] Christopher J. Horvath, "Empirical Directional Wave Spectra
  for Computer Graphics," DigiPro 2015.
- [Jerlov76] Nils G. Jerlov, *Marine Optics*, Elsevier, 1976.
- [Kerner15] Jacques Kerner, "Water Interaction Model for Boats in Video
  Games," Game Developer (Gamasutra), 2015.
- [Lagarde13] Sébastien Lagarde, "Water drop" series (wet surfaces and
  rain), seblagarde.wordpress.com, 2012–2013.
- [Losasso04] Frank Losasso and Hugues Hoppe, "Geometry Clipmaps: Terrain
  Rendering Using Nested Regular Grids," SIGGRAPH 2004.
- [McGuire14] Morgan McGuire and Michael Mara, "Efficient GPU Screen-Space
  Ray Tracing," *Journal of Computer Graphics Techniques*, 2014.
- [Schlick94] Christophe Schlick, "An Inexpensive BRDF Model for
  Physically-based Rendering," *Computer Graphics Forum*, 1994.
- [SRD51] *System Reference Document 5.1*, "Climbing, Swimming, and
  Crawling" and "Suffocating," Wizards of the Coast, CC-BY-4.0
  ([notice](SRD-5.1-NOTICE.md)).
- [Tessendorf01] Jerry Tessendorf, "Simulating Ocean Water," SIGGRAPH 2001
  course notes.
- [Tessendorf04] Jerry Tessendorf, "Interactive Water Surfaces," *Game
  Programming Gems 4*, 2004 (iWave).
- [Vlachos10] Alex Vlachos, "Water Flow in Portal 2," SIGGRAPH 2010.
- [Wallace16] Evan Wallace, "Rendering Realtime Caustics in WebGL,"
  medium.com/@evanwallace, 2016.
- Three.js Water Pro documentation, <https://docs.threejswaterpro.com/>,
  version 3.5.1, read 2026-10-06 (capability inventory only).
