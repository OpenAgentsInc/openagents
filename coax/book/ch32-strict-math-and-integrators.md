# 32. Strict math, integrators and deterministic jobs

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R15 (spec stage 10).

[Chapter 16](ch16-determinism.md) explains why the same inputs must give the
same results, and [chapter 14](ch14-jobs.md) treats a job's captures as
effects. This chapter plans the next step: a memory model, a job scheduler
whose schedules are checked, strict math profiles, and integrators and event
location as library code on those profiles.

## What this chapter will cover

- The memory model and atomic ordering.
- A job scheduler with disjoint partitions and defined reductions.
- Strict and deterministic numeric profiles.
- Fixed-point types and what they don't prove.
- Integrators: RK4, leapfrog, higher-order symplectic compositions and
  Dormand–Prince 5(4).
- Event location inside a step.

## Outline

1. **Memory model.** Atomic ordering is specified before any concurrent API
   ships.
2. **Jobs.** A scheduler, disjoint partitions, `for_parallel`, worker-local
   context and scratch, cancellation and defined reductions, all on R11
   effect and region summaries. Conflicting jobs are rejected before a
   schedule is called safe. A serial implementation stays as the oracle.
3. **Strict profiles.** Pinned math implementations; rules for rounding,
   contraction and reassociation; explicit random-generator state; ordered
   iteration. SIMD behavior is defined under these profiles before faster
   lowerings are enabled.
4. **Fixed point.** Fixed-point types need exact scale, rounding and
   overflow vectors. They are not a drop-in proof of a different floating
   algorithm.
5. **Fixed-step integrators.** RK4, symplectic leapfrog and velocity Verlet,
   and a higher-order symplectic composition for long gravitational coasts.
   Each is generic over an R12 state type and states what it conserves.
6. **Adaptive integration.** Dormand–Prince 5(4) with deterministic step
   control: a pinned error norm, safety factor and step limits. It reports
   accepted and rejected steps.
7. **Event location.** Find the earliest root of declared event functions
   inside a step by bracketing and a bounded root finder, and split the step
   there. It has an iteration budget and a typed non-convergence result under
   [chapter 31](ch31-bounded-control.md)'s contract.
8. **Choosing a tolerance.** You choose a tolerance from an observable error
   budget (arrival position, closest approach, energy drift), not from a tick
   rate.

## The consumer and the gate

- The selected ECS or simulation kernel replays with 1, 2, 4 and 8 workers
  under stress, and forbidden effects and aliases are rejected.
- Results are compared on each architecture claimed, including serialized
  state and reduction order. Host `libm` equivalence on one Mac is not
  cross-platform determinism.
- Integrator fixtures: a Kepler orbit closed after many periods (bounded
  energy error for the symplectic method, measured drift for the others);
  step-halving convergence at the stated order; an adaptive run that
  reproduces its step sequence bit for bit; and an event (a burn cutoff or a
  swept contact) located to tolerance in a step that would otherwise jump
  past it.
- Scheduler and validation overhead is measured against the serial loop.

## Done when

- R15 ships with those fixtures and measurements.
- [Chapter 16](ch16-determinism.md)'s planned sections that depend on it
  are updated.
- This chapter's integrator examples run under `check-examples.sh` and
  print their measured errors.

## Dependencies

- R3 regions ([chapter 6](ch06-memory.md)), R4 libraries
  ([chapter 8](ch08-generics.md)), R11 effects and R1 target rules.
- R12 state types ([chapter 29](ch29-units-and-frames.md)) and R14 solver
  results ([chapter 31](ch31-bounded-control.md)).
- Used by Aethyr chapters
  [18](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch018-rigid-bodies-and-time-steps.md),
  [40](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch040-propagate-with-a-stated-error.md)
  and [46](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch046-time-warp-that-stops-for-every-event.md).
