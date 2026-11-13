# 70. Rivers, ponds and the sea

> **Status: unimplemented — design draft.** Today water is a complete
> private system in `verse/`: phases W1–W11 of
> [docs/verse/water.md](../../docs/verse/water.md) are done in
> [`verse/water.coax`](../../verse/water.coax) and its sibling modules, and
> Everglade and the water lab (`/realms/waterlab`) use them. No other realm
> can load water through the OpenAgents Metaverse Engine. This chapter plans water as a Metaverse Engine
> module with the same formulas and checks. The Metaverse Engine now has the bounded
> gameplay volume/surface queries described below; the full wave, force,
> rendering and realm-adoption chapter remains a design draft.

Water touches most of the engine at once: physics pushes bodies, the
renderer draws a surface and what is under it, the camera can go below the
line, and the character can wade, swim or row. This chapter makes water one
module that any realm declares, instead of code that belongs to Everglade.

## What this chapter teaches

- How to describe water bodies as data: kind, outline, level, density,
  waves and current.
- How to keep a wave surface deterministic by snapping every frequency to a
  whole multiple of 2π/T, so a phase is an integer counter.
- How buoyancy, drag and currents enter the momentum ledger as named terms.
- How to split the gameplay surface, which every client computes bit for
  bit, from the visual detail, which each client chooses.

## Outline

1. **Water bodies.** A `WaterBody` is a pond, river, waterfall, ocean,
   pool, puddle or marsh. It has an outline (a polygon, a river `Course`
   with widths, or everywhere), a level (constant, or a profile along the
   course), a density (`FRESH`, `SALT`), up to eight Gerstner terms and an
   optional current.
2. **A surface on the clock.** `WaveSet` frequencies snap to whole multiples
   of 2π/T. `water_body_sample` at tick N equals stepping `water_advance`
   from tick 0 bit for bit, and the clock folds on the period. A period of
   2^32 ticks or more is refused, so the phase products stay exact in
   `u64`.
3. **Point queries.** `WaterSet` grids a zone's bodies so `water_sample` and
   `water_bodies_overlapping` test only bodies whose bounds cover a cell.
   This plugs into [chapter 7](ch007-ask-the-world-where-things-are.md)'s
   query module as one more query kind.
4. **Flow.** A `FlowGrid` is still, uniform, or `water_flow_river`:
   divergence-free potential flow round obstacles, solved from the stream
   function by successive over-relaxation.
5. **Forces.** `water_submerged` computes exact spherical caps, sliced
   capsules and clipped cuboids. `water_apply` adds buoyancy at the centre
   of buoyancy, linear and quadratic drag relative to the water, and angular
   damping. Each force is a `Push` under `water.buoyancy` or `water.drag`,
   recorded by `water_record` in the momentum ledger.
6. **The spectral sea.** JONSWAP from wind speed and fetch with the TMA
   depth factor, finite-depth dispersion, and directional spreading. Mode
   amplitudes come from the spectrum integrated over each cell, and a
   radix-2 inverse FFT makes height, choppy displacement, slopes and the
   Jacobian per tile ([`verse/water-spectrum.coax`](../../verse/water-spectrum.coax)).
7. **The ocean at scale.** A clipmap of nested grids whose levels snap to
   twice their spacing, with no cracks at level seams, a skirt to 40 km,
   and streamed 8 m cells in a pool of 48 slots
   ([`verse/water-ocean.coax`](../../verse/water-ocean.coax)). Streaming
   changes cost, never what is drawn.
8. **The look and under the water.** Seven presets (Jerlov absorption,
   in-scatter, foam, roughness, shore band, crest scattering), phone and
   desktop tiers (6 against 16 detail waves), the waterline across the near
   plane, and Beer–Lambert fog per channel
   ([`verse/water-look.coax`](../../verse/water-look.coax),
   [`verse/water-under.coax`](../../verse/water-under.coax)).
9. **People in the water.** Wading, swimming, diving and rowing as media
   the character controller of [chapter 9](ch009-a-character-moves.md)
   switches between (`MEDIUM_BOAT` and the wade rules in
   [`verse/water-wade.coax`](../../verse/water-wade.coax)); ripples, wakes
   and spray as effects from [chapter 72](ch072-effects-made-of-particles.md).

## What exists today

- [`aethyr/physics/query-fluid.coax`](../physics/query-fluid.coax): checked box/capsule
  trigger containment and gameplay fluid boxes with no surface, a flat
  surface or an existing heightfield sampler. Explicit rectangular holes
  remove fluid occupancy. Samples carry body identity, surface/depth, flow,
  kind, density, buoyancy and drag. Resistance is independent of buoyancy:
  `flow + (velocity - flow) / (1 + drag * dt)` slows a body without adding
  lift. Results publish whole or refuse. This is the bounded query profile
  from [chapter 7](ch007-ask-the-world-where-things-are.md), not completion
  of the waves, force ledger or rendering requirements in this chapter.

- [docs/verse/water.md](../../docs/verse/water.md): phases W1–W11, each with
  its port notes and differences from the previous Rust engine.
- [verse/water.coax](../../verse/water.coax) and
  [aethyr/physics/rigid.coax](../physics/rigid.coax): the W1 physics, with all
  fifteen of the old tests as `#test` checks.
- [everglade/legacy/v1/river.coax](../../everglade/legacy/v1/river.coax): Everglade's
  4 m river, its current round the bridge piles, and its rowboats.
- [aethyr/world/water-weather.coax](../world/water-weather.coax) and
  [verse/water-spells.coax](../../verse/water-spells.coax): rising water in
  rain, and spells that act on water.

## What it builds

- `aethyr/water/body.coax`: bodies, wave sets, sets and point queries.
- `aethyr/water/force.coax`: submersion, buoyancy, drag and ledger terms
  over Part IV's rigid bodies.
- `aethyr/water/sea.coax`: the spectrum, synthesis and the clipmap.
- `aethyr/render/water.coax`: presets, tiers and the underwater pass, as
  Coax shaders emitted by [chapter 11](ch011-the-wgsl-renderer.md)'s
  renderer.

## Acceptance checks and oracles

The chapter is done when:

- Every `#test` in today's `verse/water*.coax` modules passes against the
  Metaverse Engine modules with the same tolerances.
- The gameplay surface is the same bit for bit on a phone client and a
  desktop client at every tick, and a late joiner folds to the same tick.
- A body at rest on still water sleeps; waves or a current wake it.
- The momentum ledger balances for a floating body over a recorded run.
- The water lab stays under its budgets: 1 ms GPU on the phone tier, 3 ms on
  the desktop, and 64 KiB of uniforms.
- Everglade holds 60 FPS in all eight `perf.ts` cases with water in view.

## Dependencies

Chapters 7 (queries), 9 (the character), 11–13 (renderer, materials and
light), 16 (budgets), 18 (rigid bodies) and 38 (streaming, for the ocean's
cells). From Coax: R12 quantities for densities and levels, and R15 strict
math for the bit-for-bit surface.
