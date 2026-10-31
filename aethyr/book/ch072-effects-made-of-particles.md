# 72. Effects made of particles

> **Status: unimplemented — design draft.** Today two private effect
> systems exist. [`verse/particles.coax`](../../verse/particles.coax) and
> [`verse/particle-motion.coax`](../../verse/particle-motion.coax) draw
> embers, dust and fireflies for the Haunted House and Hecate's Cemetery,
> and [`verse/everglade/gpu-fx.coax`](../../verse/everglade/gpu-fx.coax)
> draws Everglade's spell fire, smoke, sparks and shockwave rings. This
> chapter plans one effect module for every realm. Nothing in `aethyr/`
> runs yet.

Visual effects are particles that only look: embers off a candle, dust in
a room, spray off a wake, smoke from a spell. They don't push anything and
nothing pushes them. That makes them different from the simulated particles
of [chapter 23](ch023-particles-and-their-neighbors.md), which carry mass
and neighbours. Effects can then be stateless, drawn in one instanced pass,
and capped by a budget.

## What this chapter teaches

- How to compute a particle's position, light and size from its emitter,
  its index and the clock alone, with nothing kept between frames.
- How the CPU decides which emitters are lit this frame, as a bounded list.
- How to draw additive and premultiplied effects in the right order.
- How effect budgets refuse rather than drop silently.

## Outline

1. **Emitters as data.** An emitter is 16 floats:
   `x y z radius  r g b flicker  kind count size speed  seed 0 0 0`
   (`VERSE_EMITTER_FLOATS`). A frame has at most 24 emitters
   (`VERSE_EMITTERS`) of at most 64 particles each (`VERSE_PARTICLES_PER`).
   `verse_emitter_put` clamps the count and refuses an emitter that doesn't
   fit.
2. **Stateless motion.** `verse_particle_hash(seed, i, k)` gives a number in
   [0, 1) per particle. Its age is a function of time and kind: embers live
   1.6 s, dust 14 s, fireflies 7 s (`verse_particle_life`). Offset, glow and
   size are functions of the same inputs, so the shader and the CPU checks
   see the same numbers.
3. **Choosing emitters.** A realm's function picks emitters for the eye's
   position: dust in the room you stand in, embers off the nearest
   flickering flames, fireflies over nearby outdoor areas
   (`verse_particles_haunt`). The choice is a pure function of the scene
   and the eye.
4. **One shader source.** Particle motion is written once in Coax and
   emitted as WGSL by `coax wgsl` (`apps/openagents-web/gpu/particles.coax`
   generates `particles.wgsl`), as [chapter 11](ch011-the-wgsl-renderer.md)
   plans for every shader.
5. **Puffs with state.** Spell effects are puffs of three rows: position and
   size, linear colour and heat, and a streak axis with kind and seed.
   Kinds are a fire billow, smoke or dust, a glow or spark streak, and a
   ground ring. Smoke draws first, premultiplied over what is behind it;
   fire, glows and sparks add to the light after it, so fire glows through
   its own smoke.
6. **Events that start effects.** Breaks, landings and splashes from
   [chapter 73](ch073-things-that-break.md) and
   [chapter 70](ch070-rivers-ponds-and-the-sea.md) post bounded events
   (`DEMO_EVENTS`, 32 a frame) that start effects from their seed.
7. **Budgets.** Emitters, particles per emitter and puffs are declared
   budgets under [chapter 16](ch016-budgets-that-refuse.md). A phone tier
   takes lower caps, and going over is reported, not clipped silently.

## What exists today

- [verse/particles.coax](../../verse/particles.coax): emitter layout, caps
  and the Haunted House's emitter choice, with three `#test` checks.
- [verse/particle-motion.coax](../../verse/particle-motion.coax): ember,
  dust and firefly motion, with checks that embers rise and fade, dust stays
  in its room, and fireflies blink.
- [apps/openagents-web/gpu/particles.coax](../../apps/openagents-web/gpu/particles.coax):
  the instanced draw, the source of `particles.wgsl`.
- [verse/everglade/gpu-fx.coax](../../verse/everglade/gpu-fx.coax) and
  [apps/openagents-web/everglade-spells.coax](../../apps/openagents-web/everglade-spells.coax):
  spell fire and smoke in two passes.

## What it builds

- `aethyr/fx/emitter.coax`: the emitter record, caps and per-realm choice
  functions.
- `aethyr/fx/motion.coax`: stateless motion kinds, shared by the shader and
  the checks.
- `aethyr/fx/puff.coax`: puffs with heat and streaks, and their two-pass
  draw order.

## Acceptance checks and oracles

The chapter is done when:

- The CPU and the GPU agree on each particle's position and glow within a
  stated tolerance for a fixed set of emitters and times.
- The same clock and emitters give the same frame on every client, so an
  effect needs no network traffic beyond the event that starts it.
- No frame writes more emitters, particles or puffs than its budget, and an
  over-budget request is reported.
- The Haunted House, Hecate's Cemetery and Everglade goldens don't change
  when they move to the shared module.
- Effects never change simulation state: a recorded session replays to the
  same digests with effects on and off.

## Dependencies

Chapters 1 (the clock), 11 (the renderer), 13 (light, for additive
effects), and 16 (budgets). From Coax: [chapter 17](../../coax/book/ch17-gpu.md)
for GPU programs.
