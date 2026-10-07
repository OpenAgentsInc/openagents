# Water

Status: specification, 2026-10-06, updated the same day with the owner's
answers to its open questions ([Owner decisions](#owner-decisions)). Nothing
in this document is implemented yet unless a section says so. The phases at
the end are tracked as GitHub issues on the
[OpenAgents project board](../project-board.md). The coastal zone that builds
on the ocean phases has its own specification, [The coast](coast.md).

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
- [Gameplay rules](#gameplay-rules)
- [Architecture](#architecture)
- [Budgets per tier](#budgets-per-tier)
- [Tests and captures](#tests-and-captures)
- [Phased plan](#phased-plan)
- [Coordination with the water demos](#coordination-with-the-water-demos)
- [Owner decisions](#owner-decisions)
- [References](#references)
- [Attribution](#attribution)

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
| B4 | **Ocean and coast**: an unbounded surface with spectral waves, a shoreline, and surf; its playable home is [the coast](coast.md) | WP | All, detail by tier | Sim for swell, Visual for detail |
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
| U8 | **Breath and suffocation** per SRD 5.2.1 ([Breath and suffocation](#breath-and-suffocation)) [SRD521] | New | All | Sim |

### Interaction and physics

| # | Capability | From | Tiers | Kind |
| --- | --- | --- | --- | --- |
| P1 | **Buoyancy with forces**: Archimedes' force at the center of the submerged volume of each sphere, capsule, and cuboid collider, from its density, so wood floats, stone sinks, and a plank floats on edge correctly [Kerner15] | WP (kinematic only), extended | CPU everywhere | Sim |
| P2 | **Drag and added damping**: linear and quadratic drag relative to the local water velocity, with angular damping, scaled by the submerged fraction | New | CPU | Sim |
| P3 | **Currents**: rivers carry bodies and swimmers along the flow field | New | CPU | Sim |
| P4 | **One flow field for looks and physics**: the field the shader advects with (V5) is the field the physics samples | New | CPU and GPU | Sim |
| P5 | **Swimming and wading for players and NPCs**: wade, swim on the surface, dive, climb out, tread water, with SRD 5.2.1 movement costs ([Swimming and wading](#swimming-and-wading)) [SRD521] | New | All | Sim |
| P6 | **Floating debris**: building pieces that fall into water float or sink by material, drift downstream, and pile against obstacles | New | CPU | Sim (gameplay chunks), Visual (cosmetic chunks) |
| P7 | **Interactive ripples**: a damped height field around the camera that every mover, impact, and spell writes into [Tessendorf04, Bridson07] | WP (wakes), extended to all movers | All (CPU), larger window on High | Visual |
| P8 | **Kelvin wakes** behind boats, swimmers, ducks, and drifting debris | WP | All | Visual |
| P9 | **Foam trails** behind movers that persist and drift with the flow | New | All | Visual |
| P10 | **Splashes and spray**: entry splashes scaled by closing speed and mass, crest spray against rocks, droplets, mist | WP (WebGPU only there), on every tier here | All | Visual |
| P11 | **Floating wildlife and props**: ducks, lily pads, rowboats, and barrels bob on the real surface | New | All | Visual (from P1 or W4) |
| P12 | **Fire meets water**: burning bodies that enter water are extinguished with steam | New | All | Sim |
| P13 | **Boats**: a rowboat a player can board and row, as a buoyant body with oar impulses, in the first round (W6; [Rowboats](#rowboats)) | New | All | Sim |

### Spells and water

| # | Capability | From | Kind |
| --- | --- | --- | --- |
| X1 | **Wind Wall** over water whips a band of spray and short steep ripples along the wall, and blocks floating bodies like a solid | New | Visual, plus the existing wall field |
| X2 | **Gust of Wind** drives a fan of ripples and pushes floating bodies along its line | New | Sim |
| X3 | **Meteor Swarm** into water throws a crown splash and a ring wave, flashes to steam, and leaves the burst's fire out on the water | New | Visual, plus the existing damage |
| X4 | **Lightning** (Thunderbolt, Call Lightning, Lightning Bolt, Storm of Vengeance) on water flashes across the surface and conducts to creatures in the water ([Lightning in water](#lightning-in-water)) | New | Sim |
| X5 | **Freezing**: cold spells turn water to slush, thin ice, or walkable ice by the [freezing table](#which-spells-freeze-water); bodies in the patch lock in place; ice thaws in stages | New | Sim |
| X6 | **Reverse Gravity** lifts water inside its cylinder into a column of globes and spray, and floating bodies rise with it; the water falls back when the spell ends | New | Visual, plus the existing field on bodies |
| X7 | **Thunderwave** pushes a ring wave out from the caster and shoves floating bodies | New | Sim |
| X8 | **Telekinesis and Levitate** lift bodies out of water with dripping and a pull-out splash | New | Visual |
| X9 | **Wall of Stone** across a stream dams it: the flow routes around the wall and the level behind it rises a little | New | Sim (flow field update) |
| X10 | **Black Tentacles** rise out of water with ripples and murk | New | Visual |
| X11 | **Water Walk**: up to ten creatures walk on any water surface, following the gameplay swell, and rise from below at 3 m/s | New | Sim |
| X12 | **Water Breathing**: up to ten creatures breathe underwater for 24 hours | New | Sim |
| X13 | **Control Water**: the four SRD modes, flood, part water, redirect flow, and whirlpool, as bounded level, trench, flow-field, and vortex events | New | Sim |
| X14 | **Create or Destroy Water**: rain in a cube that douses flames, or clearing fog and steam | New | Sim (flames, fog), Visual (rain) |
| X15 | **Sleet Storm, Fog Cloud, and Storm of Vengeance** over water: slush, fog banks, and the storm's round-by-round effects | New | Sim |

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
| R8 | **Weather state**: a deterministic per-zone schedule of clear, overcast, fog, rain, and storm, with spell overrides ([Weather](#weather)) | New | Sim |

### World scale and multiplayer

| # | Capability | From | Kind |
| --- | --- | --- | --- |
| M1 | **Streaming across 8 m cells**: water meshes, flow maps, and shore distance fields stream with the cells they cover | New | Engine |
| M2 | **Clipmap ocean**: camera-centred level-of-detail rings for unbounded water [Losasso04] | WP | Engine |
| M3 | **Simulated versus visual split**: the swell, levels, currents, ice, and buoyancy of gameplay bodies are authoritative; detail waves, ripples, foam, spray, caustics, and cosmetic debris are local | WP (determinism), extended | Engine |
| M4 | **Presets** as data: named looks from Jerlov water types, wind, and foam (clear pond, murky pond, mountain river, tropical coast, northern sea, storm, swamp, moonlit) | WP | Data |
| M5 | **Quality tiers** with a correct, plainer frame when a tier drops an effect | WP | Engine |

## Gameplay rules

These rules record the owner's answers of 2026-10-06
([Owner decisions](#owner-decisions)). They follow SRD 5.2.1 [SRD521]
through the [combat model](combat-model.md): real time on the world's fixed
clock, one tabletop round is 6 s, 5 ft is 1.5 m, and the simulation rolls
every check, attack, and save behind the scenes with `verse_world`'s seeded
dice. **Ours** marks a rule where the SRD is silent and we chose one. The
phase that implements a rule checks each SRD figure here against the SRD
5.2.1 text and corrects this document first if they differ.

### Swimming and wading

- **Media.** The controller's medium is `Ground`, `Wading` (water deeper
  than 0.4 m where the character stands), `Swimming` (deeper than 1.25 m, or
  no ground), or `Diving` (the eye point below the gameplay surface). The
  depths come from the gameplay surface (V4), never the visual one, so every
  client agrees.
- **Speed.** While swimming, each foot of movement costs 1 extra foot, or 2
  extra feet in Difficult Terrain, unless the creature has a Swim Speed
  (SRD 5.2.1). In real time a swimmer moves at half its walking speed, and
  at a third in Difficult Terrain water such as slush or against a Gust of
  Wind. Wading is Difficult Terrain (Ours), so a wader moves at half speed.
- **Swim Speed.** Freedom of Movement gives its target a Swim Speed equal to
  its Speed (SRD 5.2.1), which removes the extra cost. None of the Grove's
  Wild Shape forms (Brown Bear, Dire Wolf, Giant Eagle, Giant Spider) or
  Shapechange's dragon has a Swim Speed in its SRD 5.2.1 stat block, so they
  swim by the ordinary rule.
- **Climbing out.** A swimmer climbs out at a bank, jetty step, or ladder
  whose lip is no more than 0.6 m above the surface, with no check (Ours).
- **Underwater combat (SRD 5.2.1).** A melee weapon attack by a creature
  without a Swim Speed has Disadvantage unless the weapon is a dagger,
  javelin, shortsword, spear, or trident. A ranged weapon attack misses
  beyond its normal range and has Disadvantage within it unless the weapon
  is a crossbow, a net, or a thrown weapon such as a javelin, spear,
  trident, or dart. Anything fully underwater has Resistance to Fire
  damage. The SRD gives spell attacks no underwater penalty, and neither
  does Verse.
- **Falling into water (SRD 5.2.1).** A creature that falls into water uses
  its Reaction for a DC 15 Strength (Athletics) or Dexterity (Acrobatics)
  check, rolled behind the scenes; on a success the fall's damage is halved.
  This applies only in zones that deal falling damage.
- **Held swimmers (Ours).** A swimmer who is Paralyzed, Stunned, or asleep
  (Sleep) floats face up at the float line, drifts with the flow, and keeps
  breathing. Control spells don't drown anyone.
- **Frigid water (SRD 5.2.1 environmental effect).** A water preset may mark
  its water frigid: a creature can stay in it for a number of minutes equal
  to its Constitution score, then makes a DC 10 Constitution save at the end
  of each further minute or gains 1 Exhaustion level. Everglade's water is
  not frigid.

### Breath and suffocation

Breath follows SRD 5.2.1's suffocation rule as written, in real seconds:

| Step | Rule | Source |
| --- | --- | --- |
| Holding breath | 1 + Constitution modifier minutes, at least 30 s: modifier −1 or less gives 30 s, +0 gives 60 s, +1 gives 120 s, +2 gives 180 s, +5 gives 360 s | SRD 5.2.1 |
| When it starts | When the eye point goes below the gameplay surface | Ours |
| Out of breath | 1 Exhaustion level at the end of each turn, which is every 6 s | SRD 5.2.1 |
| Exhaustion | Each level subtracts 2 from every d20 Test (attacks, checks, and saves, all behind the scenes) and 5 ft from Speed, applied as a sixth of a 30-foot Speed per level | SRD 5.2.1 |
| Level 6 | The creature dies: 36 s after breath runs out | SRD 5.2.1 |
| Death in a zone without death | Where nothing can die, such as Everglade, the swimmer is defeated and surfaces at the nearest bank with its suffocation levels removed | Ours |
| Breathing again | Removes every Exhaustion level gained from suffocating at once | SRD 5.2.1 |
| Refilling breath | Breath refills linearly to full over 6 s with the eye point above the surface | Ours |
| Water Breathing | No breath is spent for the spell's 24 hours | SRD 5.2.1 |
| Water Walk | A target underwater is borne up at 60 ft (18 m) a round, 3 m/s | SRD 5.2.1 |
| Wild Shape and Shapechange | The form's own Constitution modifier sets the time | SRD 5.2.1 |
| Exhaustion from other causes | Frigid water's levels stay until a rest; breathing doesn't remove them | SRD 5.2.1 |

The HUD shows a breath bar under the health bar while the eye point is under
water, with the seconds left, a warning at 10 s, and the Exhaustion level as
a debuff icon. One constant, `BREATH_SCALE` (1.0), multiplies the hold time
if the owner later wants a game-scale duration.

### Spells and water

This section covers every spell on the Grove's bar (rows 1 to 4, the four
Circle of the Land lists, and Shapechange's breath), every spell on
Everglade's bar, and the water spells the Water Lab demo adds (see
[Coordination with the water demos](#coordination-with-the-water-demos)).

#### Ice states

| State | Walkable | What it does | Source |
| --- | --- | --- | --- |
| Slush | No | Floating ice crystals. Swimming through it is Difficult Terrain. It never holds anyone. | Ours |
| Thin ice | Only by luck | Each 3 m (10-foot) cell has a weight tolerance of 3d10 × 10 lb (3d10 × 4.5 kg: 14 to 136 kg, about 75 kg on average), rolled from the zone seed and the cell. When the weight on a cell exceeds it, the cell breaks and its occupants drop into the water. | SRD 5.2.1 (Thin Ice) |
| Walkable ice | Yes | A static collider at the surface height that bears any Medium or smaller creature. It is Slippery Ice: Difficult Terrain, and the first time in each 6 s that a creature moves onto it, a DC 10 Dexterity (Acrobatics) check, rolled behind the scenes, or Prone (knocked down for 1.5 s). | SRD 5.2.1 (Slippery Ice), collider ours |
| Floes | Floes 2 m across or larger bear one Medium creature | Broken ice: buoyant bodies (P1) that drift with the flow and melt. On swell, walkable ice forms as floes from the start. | Ours |

Ice thaws in stages: walkable ice becomes thin ice, thin ice breaks into
floes, and floes melt into open water. Each spell's row below gives the time
in each stage.

#### Which spells freeze water

The SRD gives every one of these spells an area and a duration, and none of
them a rule for freezing water, except that Sleet Storm, Ice Storm, and
Storm of Vengeance change the ground they fall on. So the freezing itself is
ours: the state follows the spell's cold damage and level, and the area is
the spell's SRD area.

| Spell (level) | On which bar | SRD area and duration | Water frozen | State and thickness | Holds | Thaw |
| --- | --- | --- | --- | --- | --- | --- |
| Ray of Frost (cantrip) | Grove, Polar land | A ray to 60 ft (18 m); the target's Speed drops 10 ft until the start of the caster's next turn | A disc of 1.5 m radius where the ray meets water | Slush, 2 cm | 6 s, the SRD's one round | Dissolves over 3 s |
| Ice Knife (1) | Grove row 2 | An attack to 60 ft, then a 5-foot-radius (1.5 m) burst of 2d6 cold, Dexterity save; instantaneous | A disc of 1.5 m radius under the burst | Thin ice, 3 cm | 6 s | Floes for 3 s |
| Sleet Storm (3) | Polar land, Water Lab | 150 ft (45 m); a 20-foot-radius, 40-foot-tall cylinder (6 m by 12 m); Concentration, up to 1 minute; its ground is Difficult Terrain and its exposed flames are doused | Still water in the cylinder | Slush, 5 cm | While the spell lasts, up to 60 s | Dissolves over 20 s |
| Ice Storm (4) | Grove row 3, Polar land | 300 ft (90 m); a 20-foot-radius, 40-foot-tall cylinder; 2d10 bludgeoning and 4d6 cold, Dexterity save; instantaneous; its ground is Difficult Terrain until the end of the caster's next turn | Still water in the 6 m radius | Walkable ice, 10 cm: hail-roughened Difficult Terrain for the first 6 s, as the SRD's hail, then Slippery Ice | 30 s | Thin ice for 10 s, then floes for 10 s |
| Cone of Cold (5) | Polar land | A 60-foot (18 m) cone; 8d8 cold, Constitution save; instantaneous; a creature it kills becomes a frozen statue until it thaws | Still water in the cone | Walkable ice, 15 cm, Slippery Ice | 60 s | Thin ice for 15 s, then floes for 15 s |
| Storm of Vengeance (9) | Grove row 4 | A storm cloud of 300-foot (90 m) radius (the Grove draws 60 ft); Concentration, up to 1 minute; in rounds 5 to 10, freezing rain deals 1d6 cold and makes the area Difficult Terrain | Still water under the cloud, from round 5 (24 s after the cast) | Thin ice, 2 cm, as a glaze | Until the spell ends | Floes for 15 s |

The Grove kit's Sleet Storm uses SRD 5.1's 40-foot radius (`Zone(40 ft)` in
`crates/verse-zone-grove/src/zones/grove/kit.rs`); W8 moves its area to SRD
5.2.1's 20-foot radius when it lands this table.

No other spell on either bar freezes water. These rules apply to every
freeze (all ours):

- **Flowing water.** Water whose flow is faster than 0.5 m/s never freezes
  past slush; its slush drifts with the flow and dissolves on the table's
  thaw time. A waterfall never freezes.
- **Swell.** Where the gameplay swell's amplitude is above 0.3 m, walkable
  ice forms as floes 2 to 4 m across that ride the swell, instead of a
  static collider.
- **Overlap.** Freezing frozen water keeps the stronger state and the longer
  hold time. Nothing stacks.
- **Caught at the surface.** A creature whose head is above water in water
  that turns to walkable ice is Restrained (rooted) until it breaks free
  with a Strength save against the caster's save DC every 6 s, or until the
  ice turns thin. The combat model's diminishing returns apply. Slush and
  thin ice hold no one. A floating body in the patch turns kinematic at its
  pose until the ice turns thin.
- **Caught underneath.** A fully submerged creature can't surface through
  walkable ice. It swims to an edge or breaks a cell: each 3 m cell is an
  object with AC 13 and 1 hit point for each centimeter of thickness, with
  Vulnerability to Fire damage and Immunity to Poison and Psychic damage.
- **Fire.** Fire damage whose area touches ice turns that ice to water at
  once, with steam, and anyone on it drops in.
- **Frozen statues.** A creature that Cone of Cold kills in the water is a
  frozen statue held in the ice until the ice thaws (SRD 5.2.1 statue, ours
  in ice).
- **Control Water** skips ice cells, because ice isn't water.

#### What the other spells do to water

| Spell (level) | On which bar | SRD area and duration | On water | Source |
| --- | --- | --- | --- | --- |
| Wind Wall (3) | Grove row 3, Everglade | A wall 50 ft long, 15 ft high, and 1 ft thick (15 m by 4.5 m by 0.3 m); Concentration, up to 1 minute; 4d8 bludgeoning, Strength save; fog, smoke, and gases can't pass; Small or smaller flying creatures and objects can't pass; loose lightweight material is carried up | Lifts a spray curtain along its foot and steep chop 1 m to each side; fog and steam can't cross it; floating Small or smaller objects can't cross it; swimmers can | SRD for fog and the lifted material; ours for spray and for applying the flying rule to floating objects |
| Gust of Wind (2) | Grove row 2 | A line 60 ft long and 10 ft wide (18 m by 3 m); Concentration, up to 1 minute; a failed Strength save pushes a creature 15 ft (4.5 m); moving toward the caster is Difficult Terrain; it disperses gas and vapor and puts out unprotected flames | Wind ripples and whitecaps along the line; a swimmer who fails the save is pushed 4.5 m; floating objects drift 4.5 m along the line every 6 s; swimming against it is Difficult Terrain; it clears fog, steam, and mist in the line | SRD for creatures, vapor, and flames; ours for floating objects and the ripples |
| Thunderwave (1) | Grove row 2 | A 15-foot (4.5 m) cube from the caster; 2d8 thunder, Constitution save; a failure pushes 10 ft (3 m); unsecured objects wholly inside are pushed 10 ft | A ring wave of 0.3 m from the cube's center; swimmers who fail and floating objects inside are pushed 3 m | SRD for the push; ours for the wave |
| Meteor Swarm (9) | Grove row 4, Everglade (destruction builds) | Four 40-foot-radius (12 m) spheres within 1 mile; 20d6 fire and 20d6 bludgeoning, Dexterity save; unattended flammable objects catch fire (Everglade draws six meteors with 4 m blasts) | A crown splash, a 1 m ring wave, and steam where a sphere meets water; no ground fire on water; ice in a sphere melts; fully submerged creatures have Resistance to the fire; floating objects that were in water in the last 60 s don't catch fire | SRD for Resistance; ours for the rest |
| Call Lightning (3) | Grove row 3 | A storm cloud 10 ft tall with a 60-foot (18 m) radius; each bolt is 3d10 lightning within 5 ft (1.5 m) of a point, Dexterity save; recalled each round; Concentration, up to 10 minutes; cast outdoors in a storm, it takes control of the storm and deals 1d10 more | A flash, a ripple ring, and a steam puff where a bolt meets water; [conduction](#lightning-in-water); in the zone's Storm weather ([Weather](#weather)), it deals the SRD's extra 1d10 | SRD for the storm bonus; ours for conduction |
| Thunderbolt (7, the Grove's own) | Grove row 4 | A Grove spell, not SRD: 12d10 lightning in a 3.4 m blast, Dexterity save | As Call Lightning, with conduction | Ours |
| Lightning Bolt (3) | Temperate land | A line 100 ft long and 5 ft wide (30 m by 1.5 m); 8d6 lightning, Dexterity save | A flash along the line; every point where the line crosses water is a conduction point | Ours |
| Shocking Grasp (cantrip) | Temperate land | A touch; lightning on a spell attack | No conduction: a spell attack never conducts | Ours |
| Storm of Vengeance (9) | Grove row 4 | Round 1, 2d6 thunder and Deafened on a failed Constitution save; round 2, acid rain, 4d6 acid; round 3, six bolts, 10d6 lightning each, Dexterity save; round 4, hail, 2d6 bludgeoning; rounds 5 to 10, freezing rain, 1d6 cold, Difficult Terrain, Heavily Obscured, and strong wind | Round 1, a ring of ripples; round 2, rain ripples (R2) that hiss; round 3, each bolt that meets water conducts; round 4, hail splashes; rounds 5 to 10, storm chop, a glaze of thin ice (freezing table), and its wind clears fog | SRD for the rounds; ours for the water |
| Fog Cloud (1) | Polar land | A 20-foot-radius (6 m) sphere within 120 ft; Concentration, up to 1 hour; Heavily Obscured until a strong wind such as Gust of Wind disperses it | A fog bank on the surface; the water is unchanged; underwater visibility is unchanged | SRD for the fog; ours for underwater |
| Create or Destroy Water (1) | Water Lab | 30 ft; creates up to 10 gallons, or rain in a 30-foot (9 m) cube that puts out exposed flames there; or destroys up to 10 gallons, or fog in a 30-foot cube; 10 more gallons or 5 ft more per slot level above 1 | Create: rain falls in the cube for 6 s with rain ripples, and puts out nonmagical flames and burning debris (P12); 10 gallons over 81 m² is 0.5 mm, so levels don't change. Destroy: clears Fog Cloud, steam, and mist in the cube; on open water it leaves a dimple 0.5 m deep that refills in 2 s | SRD for the amounts and flames; ours for the 6 s and the dimple |
| Control Water (4) | Water Lab | 300 ft; water in a cube up to 100 ft (30 m) on a side; Concentration, up to 10 minutes; one of four modes at a time | See [Control Water](#control-water) | SRD, with our implementation |
| Water Walk (3) | Water Lab | 30 ft; up to ten willing creatures; 1 hour; any liquid surface is solid ground; a target underwater rises 60 ft a round | Targets walk on the gameplay surface, riding the swell; a target under water rises at 3 m/s; they touch water, so conduction reaches them | SRD; ours for conduction |
| Water Breathing (3) | Water Lab | 30 ft; up to ten willing creatures; 24 hours | Targets spend no breath | SRD |
| Reverse Gravity (7) | Grove row 3, Everglade | A 50-foot-radius, 100-foot-tall cylinder (15 m by 30 m); Concentration, up to 1 minute; unanchored creatures and objects fall upward | Swimmers and floating bodies fall upward; the water itself isn't a creature or an object, so its level stays; a column of globes and spray rises inside the cylinder and falls back with a splash when the spell ends | SRD for bodies; ours for the water |
| Wall of Stone (5) | Circle of the Land (Arid), Everglade | Ten 10-foot panels; Concentration, up to 10 minutes | Panels stand on the bed. Across a stream they dam it: the flow re-solves around them and the level behind rises at most 0.3 m (X9) | Ours |
| Levitate (2), Feather Fall (1) | Everglade | Levitate lifts one creature; Feather Fall slows a fall to 60 ft a round | Levitate lifts a swimmer out with drips and a pull-out splash; a Feather Fall landing in water takes no damage and makes a small splash | SRD; ours for the effects |
| Fire spells: Produce Flame, Fire Bolt (cantrips), Burning Hands (1), Fireball (3), Wall of Fire (4), Fire Storm (7), the dragon's Fire Breath | Grove rows 2 and 3, Arid land, Shapechange | Their SRD areas; Fire Bolt and Fireball set unattended flammable objects alight | Steam where the area meets water, scaled by area; ice in the area melts; fully submerged creatures have Resistance to Fire, so a swimmer can dive under a Wall of Fire; floating objects that were in water in the last 60 s don't catch fire | SRD for Resistance; ours for the rest |
| Light: Moonbeam (2), Sunbeam (6), Sunburst (8), Starry Wisp (cantrip), Faerie Fire (1) | Grove rows 2 to 4 | Their SRD areas | Light reflects on the surface and makes shafts under it (W7); no effect on the water | Ours |
| Plants: Entangle (1), Spike Growth (2), Wall of Thorns (6) | Grove rows 2 and 3 | Their SRD areas on the ground | They grow from dry ground and from beds under wading depth (1.25 m or less), never from deeper water | Ours |
| Web (2) | Tropical land | Webs that aren't anchored between two solid masses or layered across a surface collapse | Webs over open water collapse | SRD |
| Clouds: Stinking Cloud (3), Insect Plague (5), Poison Spray (cantrip) | Tropical land, Grove row 2 | Their SRD areas | Drift over water like Fog Cloud; wind disperses them; no effect on the water | Ours |
| Acid Splash (cantrip) | Tropical land | A 5-foot-radius sphere | A hiss and ripples | Ours |
| Elementalism (cantrip) | Grove row 2 | Beckon Water makes a mist that dampens a 5-foot cube | Dampens characters in the cube (R6); no effect on bodies of water | SRD |
| Misty Step (2), Tree Stride (5) | Temperate land | Teleports | A caster who arrives over water arrives swimming | Ours |
| Everything else: Wild Shape, Return to Form, Wild Companion, Land's Aid, Nature's Sanctuary, Choose Land, Nature Magician, Wild Resurgence, Long Rest, Shillelagh, Healing Word, Conjure Animals, Polymorph, Mass Cure Wounds, Shapechange, Speak with Animals, Blur, Blight, Hold Person, Sleep, Freedom of Movement, Ray of Sickness, and the beasts' attacks; Everglade's sledgehammer | Grove, Everglade | Their SRD rules | No water effect beyond the swimming rules above (Freedom of Movement's Swim Speed, and held swimmers floating) | SRD; ours for held swimmers |

#### Control Water

The four modes follow SRD 5.2.1. The cube is up to 30 m on a side, and
switching modes ends the previous one.

| Mode | SRD 5.2.1 | Verse |
| --- | --- | --- |
| Flood | Standing water in the area rises by up to 20 ft (6 m) until the spell ends or the mode changes. In a large body of water, a 20-foot wave instead crosses the area and crashes; Huge or smaller vehicles in its path are carried across, and each one it strikes has a 25 percent chance to capsize. | In a body smaller than the cube, the level inside the cube rises by the chosen amount, up to 6 m, and the outline grows to the terrain at the new level, clipped to the cube's faces, which draw as standing walls of water. When the mode ends, the water falls back over 6 s (ours). In a body larger than the cube, a solitary 6 m wave crosses the cube in 6 s (ours) and carries rowboats; the 25 percent capsize roll is behind the scenes. |
| Part Water | A trench crosses the area with a wall of water to each side; when the mode ends, the trench refills over the next round. | The trench is 3 m wide (ours) along the caster's facing, the cube's length, down to the bed. Swimmers in its path move to the nearer wall (ours); bodies on the bed stay. It refills over 6 s. |
| Redirect Flow | Flowing water in the area moves in a chosen direction, even over obstacles or up walls, and resumes its course outside the area. | Flow-field cells in the cube take the chosen direction at the body's peak flow speed, at least 1 m/s. On still water it makes a 1 m/s current in that direction (ours; the SRD names only flowing water). |
| Whirlpool | Needs an area at least 50 ft square and 25 ft deep (15 m by 7.5 m). It is 5 ft wide at the base, up to 50 ft wide at the top, and 25 ft tall. A creature in the water within 25 ft is pulled 10 ft toward it each round; a creature entering it or ending its turn there takes 2d8 bludgeoning, half on a Strength save; leaving takes an action and a Strength (Athletics) check against the spell save DC. | A vortex flow field pulls swimmers within 7.5 m at 0.5 m/s (3 m every 6 s); damage lands on entry and every 6 s inside; a swimmer escapes by holding the swim-away input for 1 s, which makes the check. Floating bodies spiral in. A body too small or too shallow refuses the cast with the reason; every Everglade pond does, and the Water Lab's basin is sized for it. |

#### Lightning in water

The SRD's only damage rule for water is that anything fully underwater has
Resistance to Fire damage. It says nothing about lightning, so this rule is
ours:

1. A *contact point* is where a lightning effect meets a water surface: a
   bolt's strike point, each point where a line crosses water, or the center
   of a burst whose area overlaps water, projected onto the surface.
2. Every creature in the same water body within 6 m (20 ft) of a contact
   point that is wading, swimming, diving, or water walking, and not
   already in the spell's own area, makes the spell's saving throw. It
   takes half the spell's damage on a failure and none on a success.
3. A creature takes conducted damage once per cast, or once per bolt for
   Call Lightning and Storm of Vengeance, and never both direct and
   conducted damage from the same bolt.
4. A spell attack (Shocking Grasp) never conducts.
5. Ice, slush, and boats insulate: a creature standing on ice or sitting in
   a boat isn't in the water, and a walkable ice cell stops conduction
   across it.
6. Resistance and Immunity to Lightning damage apply as usual. Water
   Breathing gives no protection.

The radius is 6 m because it reaches across any of Everglade's ponds (4 to
6 m in radius) from a strike in the middle, while a strike at sea stays
local. Half damage on a failed save keeps a conducted hit below a direct
one.

### Rowboats

The owner approved boardable, rowable rowboats in the first round. They
land in W6 ([#10778](https://github.com/OpenAgentsInc/openagents/issues/10778)),
which already has buoyancy from W1, Everglade's water from W3, and wakes.
Until then, W3 floats them as bobbing props (P11).

- **Which boats.** The rowboats moored at Lantern Pond, Reed Pond, and the
  Thinking Pond (`layout/parks.rs`). The boathouse's rowboat stays a prop.
  [The coast](coast.md)'s harbor reuses the same boat.
- **Body.** A compound of cuboid colliders (bottom, sides, bow, stern, and
  two thwarts) with masses that give a draft of about 0.12 m empty and
  0.2 m with two aboard. Hull masking (S13) keeps water out of the hull.
- **Seats.** A rower and one passenger. A player boards with the interact
  key within 2 m of the boat: at once from a jetty or bank, or with a 1.5 s
  climb from the water. The same key leaves the boat, onto a jetty or bank
  within 2 m if there is one and into the water otherwise.
- **Rowing.** Forward and backward strokes, and turning by rowing one oar.
  Each stroke applies an impulse at each oarlock, at 0.8 strokes a second,
  for a top speed of about 1.5 m/s on still water. Current and wind add to
  it (P3).
- **Capsizing.** A boat capsizes when it rolls past about 60° and a gunwale
  goes under, or on Control Water's 25 percent roll. Its occupants fall in
  swimming. A capsized boat floats upside down, and a swimmer beside it
  rights it with the interact key (ours). Breaking it with the existing
  destruction drops its occupants in and floats its planks.
- **Rules.** The boat is a vehicle for the SRD: Control Water's wave carries
  it and may capsize it. Its occupants are out of the water for lightning.
- **Authority.** The host simulates the boat; the rower's inputs travel as
  intents; the boat's pose replicates as a NIP-MV shared body; passengers
  ride their seats and publish no pose of their own while seated. Ducks
  steer around boats.

### Weather

The owner left the source of weather to us. **Decision:** weather is a
deterministic schedule for each zone, a pure function of the zone's seed
and the world tick, the same contract as the gameplay surface; spells that
make weather lay bounded local areas over it as host events.

**Why:** a schedule computed from the tick needs no replicated stream, so
every client, a late joiner, a replay, and an offline session see the same
sky without messages, as they already see the same waves. A host-wide clock
would force one sky on zones with different climates (a temperate glade, a
stormy coast, a space station). Weather that only players trigger would
leave the world static, and the spells that do trigger it already travel as
host events with a start tick.

- **Climate.** Each zone declares a `Climate`: weights for Clear, Overcast,
  Fog, Rain, and Storm, and a wind range; or none, for indoor zones, the
  Grid, and Lagrange 1. Everglade is temperate and mostly clear; the coast
  is wetter and windier.
- **Schedule.** World time splits into 10-minute blocks. A hash of the zone
  seed and the block index draws each block's state from the climate;
  states blend over 60 s at block boundaries, and a seeded smooth noise
  varies rain intensity and wind direction inside a block. The sample is
  `Weather { state, rain, wind, fog, lightning }` at any tick.
- **SRD effects.** Heavy rain or a storm is SRD 5.2.1 Heavy Precipitation:
  the area is Lightly Obscured, Wisdom (Perception) checks have
  Disadvantage, and open flames go out. Wind of 9 m/s (20 mph) or more is
  Strong Wind: ranged weapon attacks have Disadvantage, open flames go out,
  and fog disperses. In Storm, Call Lightning takes control of the storm
  and deals 1d10 more.
- **Spell weather.** Create or Destroy Water's rain, Sleet Storm, Fog Cloud,
  Call Lightning's cloud, and Storm of Vengeance are local areas over the
  schedule, as host events with a start tick and a duration. They never
  change the zone's schedule.
- **Rising water.** R4's level rise reads the schedule's rain over the last
  30 minutes, bounded to 0.15 m, so it is deterministic too and needs no
  event.
- **Development control.** A capture and test control pins a state (for
  example, `--weather rain` on a capture example). Players have no weather
  control except spells.

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
decision (see [Owner decisions](#owner-decisions), still open) because it
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
  applies the flow, and follows [Swimming and wading](#swimming-and-wading)
  and [Breath and suffocation](#breath-and-suffocation) [SRD521].

### Spells and destruction

Spells interact through the same two layers:

- **Authoritative effects** go through `SpellWorld`: Gust of Wind and
  Thunderwave add a short field over floating bodies; Wall of Stone in a
  river marks flow-grid cells as solid and re-solves the stream's local
  potential flow (bounded to the cells near the wall); freezing adds an ice
  patch in the state the [freezing table](#which-spells-freeze-water) gives
  (a static collider for walkable ice, at the surface height, with a start
  tick and a thaw schedule) and pins bodies whose centers lie inside;
  lightning applies the [conduction rule](#lightning-in-water); Control
  Water's modes are level, trench, flow-field, and vortex events on the
  body ([Control Water](#control-water)); Water Walk and Water Breathing
  are timed effects on the character's medium and breath.
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
- **Character.** Walking into each pond wades, then swims; climbing out at
  a shallow bank works; the stream current pushes a swimmer at the field's
  speed. Breath, on a seeded clock: a Constitution modifier of −1 holds
  30 s, +0 holds 60 s, and +2 holds 180 s; out of breath, an Exhaustion
  level lands every 6 s and the sixth defeats the swimmer 36 s later;
  surfacing removes the suffocation levels; Water Breathing spends none.
- **Spells.** Scenario tests in `verse_world::spells::scenarios`, one or
  more for each row of the freezing table and each mode of Control Water:
  Ice Storm makes walkable ice that turns thin at 30 s and open at 50 s;
  Ray of Frost makes slush that holds no one; thin ice breaks under a load
  over its rolled tolerance; flowing water only slushes; Gust pushes a
  floating crate; Wall of Stone diverts a drifting plank; lightning
  conducts to a swimmer 5 m away and not to one 7 m away or on ice; the
  whirlpool refuses a pond and pulls a swimmer in a deep basin.
- **Boats.** Boarding from a jetty and from the water, rowing across a
  pond and back, capsizing past the roll limit, and two clients seeing the
  same boat pose.
- **Weather.** The schedule returns the same state for the same seed and
  tick on two clients; transitions blend over 60 s; a pinned state
  overrides it for captures.
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
| W3 | [#10775](https://github.com/OpenAgentsInc/openagents/issues/10775) | Everglade water: all four ponds swimmable with carved beds, Glade Run's level profile and generated flow field, the weir as a cascade with a plunge pool, swimming and wading for players and NPCs under SRD 5.2.1, breath and suffocation, every bank blocker removed, floating ducks, lily pads, and rowboat props | W1, W2 | 9 h |
| W4 | [#10776](https://github.com/OpenAgentsInc/openagents/issues/10776) | Spectral waves: CPU FFT with JONSWAP and directional spreading, cascades by tier, the baked tile for Low, whitecaps and the foam field, shallow-water dispersion and surf | W2 | 8 h |
| W5 | [#10777](https://github.com/OpenAgentsInc/openagents/issues/10777) | Scene color and depth copy, refraction, planar reflection, SSR on High, contact foam, and soft particles | W2 | 10 h |
| W6 | [#10778](https://github.com/OpenAgentsInc/openagents/issues/10778) | Interaction: ripple field, Kelvin wakes, foam trails, splash, spray, mist, and drip effects, floating debris from destruction, and boardable, rowable rowboats ([Rowboats](#rowboats)) | W1, W3 | 11 h |
| W7 | [#10779](https://github.com/OpenAgentsInc/openagents/issues/10779) | Underwater: submersion, fog, split waterline, Snell's window, distortion, sun shafts, motes, caustics on all submerged surfaces, underwater audio | W2, W5 | 8 h |
| W8 | [#10780](https://github.com/OpenAgentsInc/openagents/issues/10780) | Spells and water by [Spells and water](#spells-and-water): the ice states and freezing table, the other spells' effects, Control Water's four modes, Create or Destroy Water, Water Walk, Water Breathing, the lightning conduction rule, and fire meets water | W3, W6 | 12 h |
| W9 | [#10781](https://github.com/OpenAgentsInc/openagents/issues/10781) | Weather: the deterministic per-zone schedule ([Weather](#weather)), rain particles, rain ripples, puddles, wet surfaces and characters, bounded rising water | W2, W6 | 7 h |
| W10 | [#10782](https://github.com/OpenAgentsInc/openagents/issues/10782) | Ocean and scale: clipmap, cell streaming of water textures, a coastal test scene in a capture example, multiplayer checks of the simulated and visual split | W4, W5 | 10 h |
| W11 | [#10783](https://github.com/OpenAgentsInc/openagents/issues/10783) | Measurement: per-tier GPU and CPU timings on desktop, both web backends, and phones, budgets set from measurements, device runs recorded in `NEEDS_OWNER.md` | W7, W8, W9, W10 | 4 h |

The total is about 92 agent-hours. W1 can start at once; W2 and W4 to W7
can run in parallel lanes once W2 lands. The umbrella issue,
[#10784](https://github.com/OpenAgentsInc/openagents/issues/10784), is
blocked by every phase. The playable coastal zone is specified separately in
[The coast](coast.md) and tracked by
[#10796](https://github.com/OpenAgentsInc/openagents/issues/10796), blocked
by W4 and W10; it is not a phase of the water system.

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
- The demo agent is building a **Water Lab** with swimming and a water
  spell hotbar: Water Walk, Control Water's four modes (flood, part water,
  redirect flow, and whirlpool), Create or Destroy Water as rain, Sleet
  Storm, and Water Breathing. Those spells follow
  [Spells and water](#spells-and-water). Where the demo simplifies a rule,
  W8 brings it in line or records the difference here. The Lab's basin
  should be at least 15 m square and 7.5 m deep, so the whirlpool meets the
  SRD's minimum.

## Owner decisions

The owner answered the specification's open questions on 2026-10-06:

1. **Swimming in Everglade.** All of Everglade's ponds become swimmable,
   and every bank blocker goes. W3 carves Lantern Pond to 3 m at its center
   for diving, the Thinking Pond to 2.5 m, the Fern Pond to 2 m, and Reed
   Pond to 1.6 m. Glade Run stays a wading stream, 0.4 to 0.9 m deep, with a
   1.6 m plunge pool below the weir.
2. **An ocean zone.** Yes, specified for now and not built:
   [The coast](coast.md), tracked by
   [#10796](https://github.com/OpenAgentsInc/openagents/issues/10796),
   blocked by W4 and W10. W10 keeps a coastal test scene as a capture
   example; the zone builds on it.
3. **Freezing, and every other spell on water.** Decided from SRD 5.2.1 and
   the combat model: [Spells and water](#spells-and-water), with the
   freezing table, the other spells' effects, Control Water's modes, and
   the lightning conduction rule (ours, because the SRD is silent).
4. **Breath.** SRD 5.2.1 as written: hold breath for 1 + Constitution
   modifier minutes, at least 30 s, then the suffocation rule
   ([Breath and suffocation](#breath-and-suffocation)).
5. **Boats.** Rowboats are boardable and rowable in the first round, in W6
   ([Rowboats](#rowboats)).
6. **Weather.** Left to us: a deterministic per-zone schedule with spell
   overrides ([Weather](#weather)), in W9.

Still open:

1. **Compute on desktop.** Should the High tier request compute and storage
   buffers on capable desktop adapters, which allows larger GPU FFTs and
   ripple fields, at the cost of a second code path? The plan works without
   it.
2. **Water Pro itself.** Should we buy a license to compare looks
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
- [SRD521] *System Reference Document 5.2.1*, Wizards of the Coast LLC,
  CC-BY-4.0: the rules glossary (Swimming, Suffocation, Exhaustion,
  Underwater Combat, Falling), the environmental effects (Frigid Water,
  Heavy Precipitation, Slippery Ice, Strong Wind, Thin Ice), and the spells
  this document names. See [Attribution](#attribution).
- [Tessendorf01] Jerry Tessendorf, "Simulating Ocean Water," SIGGRAPH 2001
  course notes.
- [Tessendorf04] Jerry Tessendorf, "Interactive Water Surfaces," *Game
  Programming Gems 4*, 2004 (iWave).
- [Vlachos10] Alex Vlachos, "Water Flow in Portal 2," SIGGRAPH 2010.
- [Wallace16] Evan Wallace, "Rendering Realtime Caustics in WebGL,"
  medium.com/@evanwallace, 2016.
- Three.js Water Pro documentation, <https://docs.threejswaterpro.com/>,
  version 3.5.1, read 2026-10-06 (capability inventory only).

## Attribution

This work includes material taken from the System Reference Document 5.2.1
("SRD 5.2.1") by Wizards of the Coast LLC and available at
<https://dnd.wizards.com/resources/systems-reference-document>. The SRD 5.2.1
is licensed under the Creative Commons Attribution 4.0 International License
available at <https://creativecommons.org/licenses/by/4.0/legalcode>.
