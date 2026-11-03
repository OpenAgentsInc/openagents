# 59. Dead Gears

> **Status: unimplemented — design draft.** Dead Gears runs today on its own
> private code at `/realms/dead-gears`: the rules in
> [verse/deadgears/](../../verse/deadgears/rules.coax) and the page
> [deadgears.coax](../../apps/openagents-web/deadgears.coax). This chapter
> plans rebuilding it only from Aethyr's earlier chapters. Nothing here runs
> yet.

Dead Gears is a ship survival game ported from aethyr-demo's
`dead_gears.cpp`: engineers hold a ship's hold against three waves of
creatures, repair the reactor and keep the lamps lit as the deck's hour moves
toward dusk. It is the realm with the most things moving at once: 200
creatures in 2 ms a frame. Its small entity system was written for it and is
where Aethyr's entities chapter comes from.

## What this chapter teaches

- A crowd under a frame budget: entities, systems in a fixed order, flow
  fields.
- Porting a game's rules with every check of the original test suite.
- Lighting that tells time: each wave later in the day.

## The method

- **Entities** ([chapter 6](ch006-entities-are-born-and-die.md)):
  `ecs.coax`'s handles (a 20-bit slot and a 12-bit generation), deferred
  destroys flushed after the systems, component bits, ordered iteration.
  These become Aethyr's entities, and Dead Gears keeps only its components.
- **Time** ([chapter 1](ch001-a-frame-has-a-fixed-step.md)): `ecs_clock_frame`'s
  fixed 1/60 s steps with a cap for a stalled frame become the one loop.
- **Collision and movement** ([chapters 7](ch007-ask-the-world-where-things-are.md),
  [8](ch008-ground-and-collision.md), [9](ch009-a-character-moves.md)): the
  deck's boxes, swept spheres, sliding walks and the walking grid, with flow
  fields toward the engineers.
- **Look** ([chapters 11](ch011-the-wgsl-renderer.md)–[14](ch014-figures-and-animation.md)):
  every visible thing an object in one store (transform rows, colour, glow),
  the sun, lamps and their shadows, engineers as figures and creatures as
  bone-box figures.
- **Input and HUD** ([chapters 4](ch004-input-becomes-typed-actions.md),
  [15](ch015-ui-and-the-hud.md)): keyboard and two thumbs; waves, health,
  repairs.
- **Budget** ([chapter 16](ch016-budgets-that-refuse.md)): 2 ms a frame for
  200 creatures, measured per system and refused when exceeded.
- **Saves and replay** ([chapters 3](ch003-state-starts-from-its-defaults.md),
  [5](ch005-replay-a-session-exactly.md)): saved at each wave and repair;
  a game replays from its inputs.

```jai
deadgears_step :: (w: *DeadGears, a: Actions) {
    systems_run(w, a);          // input, flow, creatures, combat, repairs, waves: fixed order
    entities_flush(*w.entities); // chapter 6: destroys take effect here
}
```

## What exists today

- [verse/deadgears/ecs.coax](../../verse/deadgears/ecs.coax): component
  masks over `aethyr/entity.coax`'s `EntityStore` (spawns join at the step's
  flush, as everywhere in Aethyr), `ecs_clock_frame`, per-system `Stats`.
  A rules check drives the same input through 30, 60, 120 and 144 FPS and
  uneven frames and finds the same fight at every fixed step.
- [arena.coax](../../verse/deadgears/arena.coax) (the deck, swept spheres,
  sliding walks, walking grid, flow fields) and
  [rules.coax](../../verse/deadgears/rules.coax) (every system and every
  check of the demo's `dead-gears-test.cpp`).
- [deadgears.coax](../../apps/openagents-web/deadgears.coax): the hull, the
  cover, the reactor, the cargo, the cutter's beams, the camera at your
  shoulder, the HUD, the deck's hour, lamps and alarms, the save; drawn by
  `deadgears.wgsl`.
- [verse/deadgears/test.sh](../../verse/deadgears/test.sh) (both at
  `--steps 4000000000`; a full game is about 2.8e9 steps) and
  [bench.sh](../../verse/deadgears/bench.sh) (WebAssembly, 200 creatures in
  2 ms).
- Edit rows: alarms, radar, sparks; EDIT pauses the fight.

## What it builds

- `rules.coax` and `arena.coax` keep the game, over Aethyr's entities
  (done), queries and loop.

## Acceptance checks and oracles

- Every check ported from `dead-gears-test.cpp` passes through the move
  (`verse/deadgears/test.sh`).
- `bench.sh` holds 200 creatures within 2 ms a frame in WebAssembly, and an
  over-budget frame is a test failure, not silent clipping.
- A stale handle never reaches a slot's next owner; destroying during
  iteration is safe (chapter 6's checks, run on the game's systems).
- A recorded game replays bit-identically natively and in the browser.
- `dead-gears.png`'s golden changes only where a fix is visible.

## Dependencies

Chapters 1–16.
