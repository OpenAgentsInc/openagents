# 42. Reach a moving target

> **Status: unimplemented — design draft.** Aethyr already plans impulsive
> transfers: Lambert by universal variables, Hohmann, and a planner over
> flight times (`orbit_lambert`, `orbit_hohmann`, `orbit_transfer`,
> `orbit_plan` in [`aethyr/orbit.coax`](../../aethyr/orbit.coax)). This
> chapter plans finite-thrust rendezvous with path constraints and gravity
> assists. Nothing here runs yet.

Rendezvous is a boundary-value problem: arrive at a future position with a
required future velocity. A ship aimed at where Mars is now will miss, and
one that crosses Mars's path at the wrong speed has made a flyby, not an
arrival. This chapter solves for the control history that meets both ends
and every path constraint, and says which constraint binds when none does.

## What this chapter teaches

- Lambert solutions as initial guesses.
- The ideal constant-acceleration benchmark.
- Finite-thrust direct shooting and collocation.
- Path constraints: crew acceleration, propellant reserve, exclusion zones.
- Gravity assists as flybys whose energy change follows encounter geometry.

## The method

The ideal flip-and-burn between stationary endpoints a distance `D` apart,
at constant acceleration `a`, accelerating half way and braking half way:

```jai
t     = 2 √(D / a)
v_max = √(a D)
Δv    = 2 √(a D)
```

Over 1 AU (149,597,870,700 m, `g₀` = 9.80665 m/s²) that gives 9.041 days,
383.0 km/s peak and 766.0 km/s Δv at 0.1 g; 5.220 days, 663.4 km/s and
1,326.8 km/s at 0.3 g; 2.859 days, 1,211.2 km/s and 2,422.4 km/s at 1 g.
It ignores gravity, target motion and turning time, so it is a benchmark,
not a timetable.

A real transfer discretizes the thrust history `u(t)` and solves:

```jai
minimize   J(u)                      // propellant, time, or a weighted mix
subject to x(t_f) = x_target(t_f)    // position and velocity
           |a(t)| ≤ a_crew(t)        // crew tolerance (chapter 44)
           m(t_f) ≥ m_reserve
           x(t) ∉ exclusion zones
```

Direct shooting propagates with chapter 40 and adjusts `u` by Newton steps
on the terminal residual; its sensitivities `∂x(t_f)/∂u` come from
Psionic's [chapter 107](../../psionic/book/ch107-transform-functions-not-just-tensors.md)
`jvp`/`vjp`. Collocation instead makes the states at nodes unknowns and
enforces the dynamics as constraints, which handles long, sensitive arcs
better. Both start from `orbit_lambert`'s impulsive solution. A flyby is
solved in the planet's frame: the hyperbolic turn angle `δ` with
`sin(δ/2) = 1 / (1 + r_p v_∞² / μ)` rotates `v_∞`, and the heliocentric
energy change follows from that rotation.

## What exists today

[`aethyr/orbit.coax`](../../aethyr/orbit.coax) has `Lambert` and
`orbit_lambert`, `Hohmann` and `orbit_hohmann`, `Transfer` and
`orbit_transfer`, `orbit_plan` (the cheapest over a range of flight times,
or the quickest under a delta-v budget), and `orbit_departure`,
`orbit_arrival` and `orbit_arrival_state` for the hyperbolas that leave and
reach a world. Its checks recover a known arc's velocities to a millimetre a
second and give Earth to Mars by Hohmann as 2.94 + 2.65 km/s in 259 days,
with the planner within 60 m/s. Belt 2200
([`apps/openagents-web/belt.coax`](../../apps/openagents-web/belt.coax))
flies these impulsive plans. There is no finite-thrust solver, no path
constraint and no flyby model.

## What it builds

- `aethyr/space/rendezvous.coax`: the benchmark, shooting and collocation
  solvers over chapter 40 and 41's models, with constraint reporting.
- `aethyr/space/flyby.coax`: planet-frame flybys and their heliocentric
  effect.

## Acceptance checks and oracles

- The benchmark's 1 AU values above reproduced to their stated digits.
- A solved transfer, re-propagated by chapter 40, arrives within its
  position and velocity tolerances.
- Aiming at the target's present position is shown to miss by the target's
  motion over the flight time.
- A flyby's heliocentric energy change matches its planet-frame turn.
- An infeasible request is refused with the binding constraint named.
- Method reference: [NASA Goddard trajectory optimization](https://ntrs.nasa.gov/citations/20230006127).

## Dependencies

[Chapter 40](ch040-propagate-with-a-stated-error.md) and
[chapter 41](ch041-burns-consume-mass.md); Psionic chapter 107's
transforms. Crew limits arrive with [chapter 44](ch044-inside-a-turning-accelerating-ship.md).
