# 30. Typed clocks and multirate ticks

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R13 (spec stage 10).

A simulation reads several clocks. Simulation time can pause, scale or jump.
Steady time only moves forward. Civil time follows a calendar and can be
corrected. Today Coax code uses integer step counters and plain numbers for
all three. This chapter shows how each clock will get its own instant type,
how tasks at different rates will share data with a declared maximum age, and
how time acceleration will stop at every scheduled event.

## What this chapter will cover

- Three instant types: simulation, steady and civil.
- Explicit conversions between clocks, with a named source.
- Reading a clock as an effect.
- Tick schedules for tasks that run at different rates.
- Astronomical time scales and time acceleration.

## Outline

1. **Instants and durations.** Each clock domain is its own instant type,
   built on [chapter 29](ch29-units-and-frames.md)'s durations. Subtracting
   two instants of one clock gives a duration. Subtracting instants of two
   clocks is refused.
2. **Conversions.** A conversion between clocks names its source, such as a
   recorded mapping or a pinned offset.
3. **Clock reads are effects.** Reading any clock is an R11 effect, so a pure
   procedure can't read the time.
4. **Tick schedules.** Each task declares its rate. Each connection between
   tasks declares what a faster reader sees between slower updates (for
   example, sample-and-hold of the latest value) and a maximum age.
5. **Stale reads.** A read older than its maximum age is a typed error
   ([chapter 7](ch07-errors.md)), not a silently stale value.
6. **Deterministic order.** Given rates and phases, the order of tasks in a
   tick is defined and inspectable.
7. **Astronomical time scales.** TAI, TT, TDB (the argument of planetary
   ephemerides) and UTC are distinct instant types. Conversions use pinned
   tables (leap seconds, the TT–TDB series) whose identity is part of the
   conversion. An instant outside a table's coverage is refused.
8. **Time acceleration.** A warp factor scales simulation time against
   steady time. A scheduled event (a burn boundary, a signal arrival, a
   contact) caps the next step, so warping never moves an event.
   [Chapter 32](ch32-strict-math-and-integrators.md) adds event location.

An uncompiled sketch:

```jai
// Sketch only: not accepted by coax0.
control :: task @rate(hz=500) (q: Hold(JointState, max_age=4ms)) { ... }
physics :: task @rate(hz=1000) () -> JointState { ... }
t_tdb := convert(TDB, t_utc, table=LeapSeconds_2026_07);
```

## The consumer and the gate

- The simulated robot's physics and control steps run at different rates
  from declared schedules.
- Verse's fixed-step update runs from a declared schedule; see Aethyr
  [chapter 1](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch001-a-frame-has-a-fixed-step.md).

Fixtures cover refused conversions between clock domains, simulated time
paused and stepped while steady time advances, sample-and-hold reads, a read
past its maximum age, and the exact task order over many ticks compared with
an independent reference trace. For the spacecraft scenario: a UTC–TDB
conversion compared with published values, a refused instant outside the
leap-second table, and the same event order at warp 1 and 10⁶.

## Done when

- R13 ships with those fixtures, and the robot and Verse consumers use it.
- Recorded clock boundaries are available for replay
  ([chapter 33](ch33-replay-and-snapshots.md)).
- This chapter's examples run under `check-examples.sh` in debug and
  release.

## Dependencies

- R12 durations ([chapter 29](ch29-units-and-frames.md)).
- R2 typed errors ([chapter 7](ch07-errors.md)) and R11 clock effects.
- Used by Aethyr chapters
  [1](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch001-a-frame-has-a-fixed-step.md),
  [39](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch039-ephemerides-time-scales-and-frames.md)
  and [46](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch046-time-warp-that-stops-for-every-event.md).
