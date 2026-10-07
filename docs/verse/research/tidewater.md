# Tidewater for Verse water

This note compares [Tidewater](https://github.com/dgreenheck/tidewater), an
island fishing game by Dan Greenheck (DRG Software Solutions LLC), with
Verse's water system ([water.md](../water.md)) and its coast
([coast.md](../coast.md)). It was written on 2026-10-07 against Tidewater
commit
[`4811ba48d7`](https://github.com/dgreenheck/tidewater/tree/4811ba48d795197de5621985f404e765c0b7c0ef).
Paths below are relative to that tree. Tidewater runs on raw WebGPU and WGSL
with its own small engine, and targets an Apple M5 Pro.

**License.** Tidewater is MIT-licensed, unlike Water Pro by the same author,
whose license forbids reading its source. We can read Tidewater's code.
We still reimplement each technique in Rust and WGSL rather than copying, and
each commit that takes one credits "Tidewater by Dan Greenheck (DRG Software
Solutions LLC), MIT" and names the file it learned from.

**Constraints.** Every Verse tier runs under WebGL2 limits: no compute, 16
sampled textures (water uses 14), and eight varyings. Tidewater assumes
compute and temporal upscaling, so each harvest needs a path without compute
and a fixed kernel where Tidewater jitters. Effort is in agent-hours.

## Harvest list

| Rank | Technique | Tidewater | Verse issue | Tiers | Effort |
| --- | --- | --- | --- | --- | --- |
| 1 | Wave travel-time field over the bathymetry | `src/world/ShoreField.js` | C1 | All | 3–4 h |
| 2 | Analytic breakers per wave, with swash run-up | `src/ocean/ShoreWaves.js` | C1, C3 | Swash all; breakers Medium, High | 8–11 h |
| 3 | Mipmapped cascades sampled at the mesh's spacing | `src/ocean/WaterSurface.js:128` | W11, C1 | All | 3–4 h |
| 4 | Caustics splatted from the real wave surface | `src/ocean/Caustics.js` | W7, C4 | Medium, High | 4–6 h |
| 5 | Wind sea plus a distant swell in one spectrum | `src/ocean/OceanFFT.js:90` | C1 | All | 2 h |
| 6 | Closed-form in-scattering in the water column | `src/post/Underwater.js:320`, `src/ocean/WaterMaterial.js:494` | W7 | All | 2–3 h |
| 7 | Sea detail: gusts, slicks, and windrows | `src/ocean/SeaDetail.js` | C1, W9 | All | 2–3 h |
| 8 | Shadows taken where the refracted sun ray enters | `src/ocean/UnderwaterLighting.js:296` | W7, C4 | All | 1–2 h |
| 9 | Foam lace texture, then a beach foam and wetness field | `src/ocean/SurfFoam.js`, `src/ocean/ShoreSim.js` | C1 | All | 1–2 h, then 4 h |
| 10 | Reflections below the horizon darken by slope | `src/ocean/WaterMaterial.js:334` | W11 | All | 1 h |
| 11 | Spray placed at each breaker's lip and plunge point | `src/ocean/Breakers.js` | C1, C6 | All (CPU sprites) | 3 h |
| 12 | Keel cross-flow drag and a resistance hump for boats | `src/player/BoatController.js` | C3 | All | 1–2 h |
| 13 | Lens droplets after surfacing or in rain | `src/post/LensDroplets.js` | W7, W9 | All | 1–2 h |
| 14 | Per-view GPU timing bench | `src/core/Bench.js`, `src/core/Profiler.js` | W11 | Desktop, WebGPU | 2–3 h |
| 15 | Surf sound driven by the drawn waves | `src/audio/SoundScape.js:852` | C6 | All | 3 h |

## Surface and spectrum

- **Mipmapped cascades (rank 3).** Tidewater samples each cascade's mip chain
  in the vertex shader at `log2(spacing / texel) + 0.7`, so no vertex reads
  detail finer than its mesh. Our `water_waves` array has one level
  (`ocean.rs`) and `water_ocean_move` reads level 0 in every clipmap ring, so
  coarse rings alias at grazing angles. Build box mips on the worker (a third
  more upload); GLES 3.0 takes an explicit vertex level. W10 is closed, so
  this goes to W11 or C1.
- **Two wave systems (rank 5).** `OceanFFT.js:90` and `:213` sum a wind sea
  and a swell (1,200 km fetch, narrow spread), each with its own direction.
  Our `Spectrum` has one system; the coast wants a southwest swell under a
  local wind. Cascade 0 is the gameplay band, so the variance test must cover
  the sum.
- **Sea detail (rank 7).** `SeaDetail.js:51` samples one 256² noise tile at
  620 m, 230 m, and 1,100 × 70 m: gusts scale short-wave roughness by 0.5 to
  1.5, slicks appear below 13 m/s, windrows above. It breaks large-scale tile
  repetition. Pack it into `water_tile`'s spare channels to stay at 14
  textures.
- **Horizon reflection (rank 10).** `WaterMaterial.js:334` darkens reflections
  that point below the horizon and tilts them up by the unresolved slope. Our
  `water_shade` reads the sky cube unoccluded, which shows most on Low.
- **Smaller.** Capillary detail near the eye (`WaterSurface.js:297`), and
  culling clipmap blocks as `src/core/CDLOD.js` culls nodes (W11).

## Shore and surf

- **Travel-time field (rank 1).** `ShoreField.js:66` solves |∇T| = 1/√(g·d)
  over the bathymetry by fast marching from a plane swell at the edge, and
  stores arrival time, direction times exposure, and shoreline arrival.
  Crests bend around headlands and follow depth contours, and the exposure
  can replace the harbor's hand-made shelter mask. It is a deterministic CPU
  bake beside `water::bake`, as a channel pair in `water::field`'s atlas.
- **Breakers and swash (rank 2).** `ShoreWaves.js` drives each surf wave from
  that field: heights in sets with a bar and rip channels so waves peel
  (`:300`), breaking where Green's-law shoaling meets the depth (`:316`), a
  profile from skewed crest to tube to bore (`:380`), and state from the
  depth under the crest (`:470`). The swash is a closed-form run-up that ends
  per pixel on the analytic front (`:575`). Ours caps linear shoaling at
  McCowan's limit and draws a sine band at the waterline. It is ALU plus one
  lookup, but must fit the existing varyings.
- **Foam and wetness (rank 9).** `SurfFoam.js:30` bakes a bubble-strand lace
  texture that foam amount thresholds into mats, lace, and strands; it would
  improve every foam we draw. `ShoreSim.js` advects beach foam and keeps sand
  wetness drying over 28 s on a 768² grid; ours would be a 128² to 256² CPU
  field near the camera, like the ripple field.
- **Spray (rank 11).** `Breakers.js` finds crests on shore transects and emits
  drops and mist at the lip and plunge point by wave energy. The finder is
  analytic, so it ports to the CPU and our sprite budget.
- **Smaller.** Turbid surf (`ShoreWaves.js:493`) and light through thin
  breaking faces (`:528`).

## Wakes and boats

- **Boat handling (rank 12).** `BoatController.js` has a resistance hump past
  hull speed, cross-flow drag per station, and added mass by wetted
  fraction; our rowboat's keel is one rate.
- **Wake source.** `WakeSim.js` steps a 512² grid spectrally with dispersion
  blended over four depths, driven by a Havelock pressure field under the
  hull, so the Kelvin pattern emerges. The grid is too large for phones, but
  the pressure-head source and depth-aware dispersion fit High's iWave (C3,
  4 h).
- **Below-water refraction.** `RefractionPass.js` redraws the scene below sea
  level at half size with a guard band, so piers and hulls leave no holes;
  we fall back to the unrefracted pixel. High only (C3, C4, 5–6 h).

## Underwater and caustics

W7 and W9 are in progress elsewhere; these items are for those agents.

- **Splatted caustics (rank 4).** `Caustics.js:5` draws a 256² grid over the
  finest tile into a 512² target, additively, moving each vertex to where its
  refracted sun ray lands and writing the area ratio, so the folds match the
  visible waves. Two focal depths share R and G, the next cascade hides the
  tile, and three taps split the color. No compute, but Low has no slot.
- **In-scattering (rank 6).** `Underwater.js:320` and `WaterMaterial.js:494`
  integrate sun and sky scattering exactly along a path of changing depth,
  with a two-lobe Henyey–Greenstein phase. Our `water_inscatter` is a
  constant color; the presets need a scattering split.
- **Refracted shadows (rank 8).** `UnderwaterLighting.js:296` looks shadows up
  where the refracted sun ray meets the surface, so they sway and soften.
- **Lens droplets (rank 13).** `LensDroplets.js:67` draws clinging and sliding
  drops as small inverted lenses that dry over 9 s.
- **Smaller.** A meniscus that follows the crests (`Underwater.js:192`),
  half-resolution shafts (`:228`), and swaying marine snow
  (`src/fx/MarineSnow.js:79`).

## Measurement and sound

`Bench.js` renders fixed views and records GPU timestamps per pass; W11 can
copy the shape. `SoundScape.js:852` mirrors the shore waves on the CPU and
plays a crash, wash, and backwash where and when each wave does them (C6).

## Where ours is already better

- **Height queries:** an exact CPU sample within 2 mm of the drawn surface,
  where Tidewater reads the GPU back one to three frames late.
- **Spectrum:** cell-integrated, looping, and variance-matched within 2%;
  Tidewater's amplitudes are random and its foam repeats with each tile.
- **Tiers:** ours runs on WebGL2 and phones; Tidewater needs WebGPU compute.
- **Physics and rules:** exact submerged volumes, swimming media, breath,
  shared rowboats, and spells; Tidewater has a simple swim and no weather.
- **Reflection:** a planar mirror, a screen-space march, and an analytic hull
  mask on every tier.
