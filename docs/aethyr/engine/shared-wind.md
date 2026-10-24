# Shared wind and live effects

Implemented in the editable Open Ground profile, 2026-10-23. This is the first
shared effects interface and one fixed composable recipe. It does not implement
a general Niagara graph compiler.

## Try it

Run [Open Ground](../../../apps/aethyr/README.md#run-open-ground-locally), then open **Effects**.
Adjust wind speed, direction, and gust strength. Grass height, width, and wind
response update immediately. **Preview impact** plays a flash, sparks, smoke,
and debris six meters ahead of the player. Change wind direction to move the
smoke and grass together. The preview causes no gameplay damage or collision.
**Reset** restores the default parameters; reloading also restores defaults.

The parameter area scrolls independently, leaving the preview and reset buttons
visible. Keyboard changes in the panel do not move the player. Escape closes the
panel and returns focus to its button.

## Shared field

[`WindFieldSettings`](../../../apps/aethyr/packages/renderer/shared-wind.ts) defines
`aethyrforge.wind.v1`. Its inputs are a normalized horizontal `[x, z]` direction,
base speed in meters per second, gust strength, spatial scale in meters,
phase rate in radians per second, seed, and enabled flag. Two continuous sine
gusts vary speed across world space and time. Base speed is bounded to 30 m/s;
maximum gust strength can double the instantaneous speed. Disabled wind yields
zero velocity.

CPU sampling and generated WGSL use the same equations and constants. Three
aligned `vec4` values carry the field to the grass shader. A separate presence
marker distinguishes a missing field from explicitly disabled wind. Callers
without a shared field retain their existing appearance and behavior.

- Grass samples the field on the GPU and applies its own bending response.
  Wind disabled, zero speed, or zero response also stops blade flutter.
- Analytic ambient particles can opt into wind with an emitter response. The
  exact time integral at the emitter position preserves seeking and birth times.
  Existing Town profiles retain their previous motion until explicitly opted in.
- Impact smoke samples the field at its origin and mid-age, then applies an
  age-scaled displacement. This is an analytic visual approximation to advection.

The field is independent of React, Effect Atom, and UI frame timing. It models
horizontal wind, without obstacles, turbulence volumes, or fluid simulation.

## Live recipe updates

[`SceneEffectsSettings`](../../../apps/aethyr/packages/runtime/scene-effects.ts) combines wind,
grass appearance, and impact parameters. Effect 4 services send bounded typed
requests to the preview worker. Effect Atom exposes accepted settings to React.
Slider edits are coalesced to at most one request per UI animation frame.

The worker validates the complete candidate before applying it. Invalid values
leave the accepted settings and active effects unchanged. Parameter edits reuse
GPU buffers and pipelines; they preserve player pose, animation time, particle
birth times, and event identity. Player and read-only modes reject these actions.
Settings are not authored world data and are not persisted.

[`ImpactRecipe`](../../../apps/aethyr/packages/renderer/impact-recipe.ts) composes flash, sparks,
smoke, and debris modules. The panel exposes intensity, smoke amount, and spark
count. A fixed pool holds at most 64 four-second cosmetic events; overflow is
explicitly rejected. At maximum module counts the recipe uses 19,200 vertices,
within the existing 24,576-vertex effect buffer. Up to four unshadowed flash lights
reuse clustered lighting. Rendering uses the existing vertex storage with an
additional alpha draw when smoke or debris is present.

Live recipe edits retain each event's ID, seed, start time, and age. Capture-time
sampling can seek retained events without advancing or expiring the live pool.
This pool is not an effect history archive. Smoke is unlit, analytic billboards
without depth sorting; debris is cosmetic ballistic billboards. Neither changes
authoritative gameplay, destruction, physics, or navigation.

## Verification

Focused tests cover field validation and CPU sampling, uniform packing, legacy
fallbacks, analytic particle seeking, atomic recipe updates, event capacity,
capture rewind, typed requests, player restrictions, and renderer buffer reuse.

`tools/verify-shared-wind.mjs` exercises the actual controls and worker in Chrome,
compares fixed-time images, and samples the same field in a WebGPU compute shader.
`tools/verify-ground.mjs` checks baseline walking, running, jumping, camera modes,
FPS, grass motion, and rejection of gameplay abilities.

The isolated Mac browser check used Chrome 155 at 1280 × 720. Sixteen CPU/WebGPU
samples differed by at most 0.000047 m/s. Wind-off grass had zero changed pixels
between fixed-time foreground captures; enabled wind changed 57–76% of those
pixels. Separate captures isolated smoke amount and opposite wind directions.
Live edits retained player pose and event start time, with renderer-accounted
GPU storage constant at 213,703,732 bytes. Baseline walking with mouse look
averaged 60.22 render submissions per second over 11.59 seconds; its HUD showed
60 FPS. These are local submission and UI measurements, not GPU completion
timings, physical display measurements, or mobile performance qualification.

Captures and browser reports remain outside Git alongside the private assets.
