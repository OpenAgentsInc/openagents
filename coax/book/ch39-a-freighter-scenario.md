# 39. A freighter scenario, end to end

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This chapter uses roadmap R12, R13, R15, R16 and
> R18–R20 (spec stage 10).

The roadmap's second physical-world consumer is one spacecraft scenario: a
freighter loads cargo, transfers to a moving destination, rides out a fault
and docks. It runs on the `verse/` space modules, which Orbit League and
Belt 2200 use today
([space primitives](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/space.md)),
and it is the scenario behind
[Aethyr chapters 39–48](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch039-ephemerides-time-scales-and-frames.md).
This chapter follows one run through each language facility it needs. It is
a study profile, not flight qualification.

## What this chapter will cover

- Heliocentric, body-fixed, ship and drum-interior states as typed frames.
- Astronomical time scales and time warp that never moves an event.
- Integrators chosen from an error budget, with events located inside a
  step.
- Observations that carry light time, and a controller that can't read
  truth.
- Engineering, game and presentation profiles over one state.

## Outline

1. **The scenario as data.** Cargo mass, tanks, drive, radiator and
   destination, each parameter with an R20 provenance record
   ([chapter 37](ch37-provenance-and-rule-modules.md)).
2. **Frames.** The states in `aethyr/orbit.coax` and `aethyr/gravity.coax`
   move onto R12 types ([chapter 29](ch29-units-and-frames.md)). A full state
   transform includes the `ω × r` term, and a position-only transform applied
   to a velocity is refused. Heliocentric positions are f64; an f32 position
   there is refused.
3. **Time.** TDB for the ephemeris, UTC for the log, converted through pinned
   tables ([chapter 30](ch30-clocks-and-ticks.md)). An instant outside the
   leap-second table is refused.
4. **Loading cargo.** Mass and inertia change, so the burn plan changes. The
   check: Δv against the rocket equation.
5. **The transfer.** A Lambert guess, then a finite burn on an R15 integrator
   ([chapter 32](ch32-strict-math-and-integrators.md)). Burn cutoff is an
   event located inside the step, not at a tick.
6. **Time warp.** The same event order and final state at warp 1 and 10⁶.
7. **Signals.** Ship-to-station messages take light time: about 499.0 s one
   way at 1 AU ([chapter 35](ch35-sensors-and-actuators.md)). A controller that
   reads the station's true state instead of a received one is refused.
8. **The fault.** A drive cutoff mid-burn, a leaking tank or a damaged
   radiator from the R19 corpus ([chapter 36](ch36-fault-scenarios.md)). The
   ship keeps a propagated trajectory; it does not freeze.
9. **Docking.** The frame changes from heliocentric to the station's body
   frame at a recorded instant.
10. **Replay and profiles.** Replay the run at two warp factors with R16
    ([chapter 33](ch33-replay-and-snapshots.md)). Engineering and game
    profiles give the same trajectory where no rule module or invented
    parameter differs, and a presentation-only change leaves recorded state
    bit for bit identical.

## The consumer and the gate

The consumer is the freighter run and the Verse space games. The gates come
from the phases: a rotating-frame state round trip against the inertial
result (R12); a UTC–TDB conversion compared with published values (R13); a
Kepler orbit closed after many periods and an event located to tolerance
(R15); the same events at two warp factors (R16); one-way and round-trip
light time (R18); a fault that leaves a propagated trajectory (R19); and
provenance in the report, with invented parameters refused under the
engineering profile (R20).

## Done when

- The phases above have shipped with these gates.
- The chapter's examples run under `check-examples.sh`, with exact output.
- Aethyr chapter 48's regression suite passes at engineering fidelity on the
  same types.
- Hypothetical propulsion stays in declared rule modules, never in the
  engineering profile.

## Dependencies

- Chapters [29](ch29-units-and-frames.md), [30](ch30-clocks-and-ticks.md),
  [32](ch32-strict-math-and-integrators.md), [33](ch33-replay-and-snapshots.md)
  and [35](ch35-sensors-and-actuators.md) to [37](ch37-provenance-and-rule-modules.md).
- [Chapter 16](ch16-determinism.md) for strict profiles.
- Aethyr chapters 39–48, and the
  [physics and simulation design reference](https://github.com/OpenAgentsInc/openagents/blob/main/docs/research/expanse-physics-and-simulation-design-reference.md).
