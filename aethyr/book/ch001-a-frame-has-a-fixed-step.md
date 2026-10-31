# 1. A frame has a fixed step

> **Status: unimplemented — design draft.** Every realm keeps its own fixed
> step today: the walking controller in `verse/player.coax` steps at 1/64 s,
> Dead Gears' entity clock at 1/60 s, Orbit League at 1/120 s and Ice Cream
> Man at 1/30 s, each with its own catch-up rule. This chapter plans the one
> loop they will share. Engine-core task: `ec-loop` (#12609); the engine-core
> agent fills this chapter in as the module lands. The module itself,
> [aethyr/loop.coax](../loop.coax), landed on 2026-10-31 (one clock in whole
> nanoseconds, the 1/64 s step, a frame clamp and at most eight steps a
> frame); its listings and checks have not joined the chapter yet.

A display delivers frames when it can: 60 a second on most phones, 120 or 144
on fast screens, and sometimes a 300 ms stall when a tab wakes up. A
simulation that advances by whatever time a frame happened to take gives
different jumps, different landings and different collisions at every frame
rate. Aethyr separates the two clocks. Simulation advances only in fixed
steps; drawing shows a point between the last two steps. One owner advances
time, and nothing else does.

## What this chapter teaches

- Why a fixed step makes physics independent of the frame rate.
- The accumulator: carrying unstepped time between frames.
- Clamping a long frame, and what is dropped when it is clamped.
- Interpolating the drawn state between the last two steps.
- Choosing a step that is exact in binary.

## The method

Each frame the host reports the elapsed time `Δ`. The loop clamps it, adds it
to an accumulator `a`, runs as many whole steps `h` as fit, up to a cap
`N_max`, and reports how far the frame lies into the next step:

```jai
Δ'    = clamp(Δ, 0, Δ_max)              // a stall counts as at most Δ_max
a     = a + Δ'
n     = min(⌊a / h⌋, N_max)             // steps this frame
a     = a − n h                         // what is left
a     = (a ≥ h) ? backlog_rule(a) : a   // past the cap: drop, or keep one step
α     = a / h  in [0, 1)                // the drawing factor
draw  = lerp(state_prev, state_now, α)
```

Before each step the loop copies the current state to `state_prev`; after the
last step, drawing reads `lerp(prev, now, α)`. Angles interpolate along the
short way round (`wrap_angle(now − prev)`), and anything that teleports (a
respawn, a portal) marks a cut so the frame shows the new state rather than a
smear across the world.

The step is a power of two where the realm allows it. With `h = 1/64` s,
`n h` and the accumulator stay exact in binary floating point, so 64 steps
are exactly one second and no slack term is needed. A step of `1/60` needs a
tolerance (Dead Gears adds `1e-7` so that 0.05 s is three steps) and drifts
against wall time. Aethyr's default is `1/64`; a realm may declare another
rate, and the loop states it in the recording (chapter 5).

The backlog rule is a declared choice, not an accident. Dropping the whole
backlog keeps a stalled frame from making the next one slower (the walking
controller's rule); keeping at most one step of backlog keeps a little more
motion after a hitch (Dead Gears' rule). Both are deterministic given the
sequence of `Δ`; neither makes the simulation depend on `Δ` within a step.

## What exists today

- [verse/player.coax](../../verse/player.coax): `STEP :: 0.015625`,
  `MAX_STEPS :: 8`, `MAX_FRAME :: 0.25`; `advance_in` adds the clamped frame
  to `carry`, saves `prev_pos`, `prev_target`, `prev_face` and `prev_dist`
  before each `step`, and drops the backlog when the cap is reached;
  `alpha`, `verse_player_feet` and `verse_player_at` interpolate.
- [verse/deadgears/ecs.coax](../../verse/deadgears/ecs.coax): `Clock` and
  `ecs_clock_frame`, a 1/60 s step, a 0.1 s frame cap, eight steps at most,
  the leftover clamped to one step, and `alpha` for drawing.
- `apps/openagents-web/orbitleague.coax` steps physics at 120 Hz
  (`STEP`, `MAX_STEPS :: 12`); `verse/icecream/play.coax` uses
  `STEP :: 0.0333333333`.
- The browser host ([coax/web/host.js](../../coax/web/host.js)) calls
  `coaxial_frame(state, …, dt, still)` once per animation frame and supplies
  `coaxial_now_ms`, a monotonic clock. Native hosts do the same through
  `platform/host.c`.

Four loops with four constants and two backlog rules: a jump's height, and
whether the camera stutters, depend on which realm you are in.

## What it builds

- `aethyr/loop.coax`: `Loop` (step, frame cap, step cap, backlog rule,
  accumulator, step count, simulated time) and `loop_frame(loop, Δ) -> Loop`
  returning the number of steps to run and `α`; a `Prev` convention that a
  realm's state uses for interpolated fields; and `loop_cut` for teleports.
- The single owner: a realm's frame entry point calls `loop_frame` once and
  runs its step function `n` times. No other code reads the frame's `Δ`.

## Acceptance checks and oracles

- Steps are fixed at any frame rate: a scripted minute of input at 30, 60,
  120 and 144 FPS (and an irregular frame-time trace) produces identical
  physics states after the same number of steps, bit for bit.
- Drawing interpolates between the last two states: a 60 FPS capture of a
  jump shows no visible stutter; numerically, the drawn height over frames
  has no reversal on the rising arc and no repeated value while moving.
- Oracle: an independent integer model of the accumulator in units of `h`
  (exact for `h = 1/64`) predicts `n` and `α` for every frame of the trace.
- A stall of 10 s steps at most `N_max` times and follows the declared
  backlog rule; `α` stays in `[0, 1)`.
- Mutation testing of `aethyr/loop.coax`: 0 survivors.

## Dependencies

None in this book; this is the first piece. From Coax: checks and mutation
([Coax chapter 11](../../coax/book/ch11-writing-checks.md)), determinism
([Coax chapter 16](../../coax/book/ch16-determinism.md)), and later R13 typed
clocks and multirate ticks ([Coax roadmap](../../coax/ROADMAP.md)), which give
the step's simulation time its own type. Background:
[game-engine systems inventory, section 5](../../docs/research/game-engine-systems.md).
