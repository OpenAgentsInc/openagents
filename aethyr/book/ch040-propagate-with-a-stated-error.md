# 40. Propagate with a stated error

> **Status: unimplemented — design draft.** Aethyr already propagates two-body
> Kepler orbits and hands ships between spheres of influence
> (`orbit_propagate`, `orbit_dominant`, `orbit_soi_blend` in
> [`aethyr/orbit.coax`](../../aethyr/orbit.coax)); this chapter plans
> integrators whose error is stated per observable. Nothing here runs yet.

Propagation is the question "where will it be". Every method answers it with
some error, and the error that matters depends on what the caller asks: an
arrival position, a closest approach, or energy over a thousand orbits. This
chapter offers several propagators, each with an error budget on named
observables, and refuses cases its model cannot represent.

## What this chapter teaches

- Analytic two-body coasts, patched conics with sphere-of-influence handoff,
  and restricted n-body dynamics with chapter 39's bodies as sources and
  vessels as test particles.
- Fixed-step RK4, symplectic leapfrog and higher-order compositions, and
  adaptive Dormand–Prince 5(4).
- Error budgets per observable, with tolerance, step count and cost reported.
- A relativistic envelope, and constant proper acceleration as its own mode.

## The method

The restricted n-body vessel equation, with body positions from the
ephemeris rather than integrated:

```jai
r' = v
v' = Σ_j μ_j (r_j(t) − r) / |r_j(t) − r|³ + a_thrust
```

Leapfrog (kick–drift–kick) is symplectic: its energy error stays bounded
over many periods rather than drifting. Yoshida-style compositions of
leapfrog raise the order while keeping that property. Dormand–Prince 5(4)
adapts its step from the embedded error estimate; its energy error drifts,
and that drift is measured, not assumed away.

Each propagation returns its own report:

```jai
PropagationReport :: struct {
    method: Method; steps: int; evaluations: int;
    tolerance: f64;
    arrival_error_m: f64;        // against a reference run or closed form
    closest_approach_error_m: f64;
    energy_drift: f64; angular_momentum_drift: f64;
}
```

The Newtonian profile declares an envelope: `v/c` below a threshold and the
potential `|Φ|/c²` below another. A state outside it is refused. An ideal
constant-proper-acceleration segment uses its closed forms,
`v = c tanh(aτ/c)`, `t = (c/a) sinh(aτ/c)`, `x = (c²/a)(cosh(aτ/c) − 1)`,
as a separate mode. The ideal 1 AU, 1 g flip-and-burn peaks near 0.00404 c,
inside the Newtonian envelope.

## What exists today

[`aethyr/orbit.coax`](../../aethyr/orbit.coax) solves Kepler's equation for
ellipses (`kepler_elliptic`) and hyperbolas (`kepler_hyperbolic`) to 1e-12,
propagates with `orbit_propagate`, and hands states between bodies with
`orbit_soi_radius`, `orbit_body_soi`, `orbit_relative`, `orbit_absolute` and
`orbit_dominant`. `orbit_burn_step` integrates a finite burn with RK4. Its
checks keep an orbit's energy and angular momentum to 1e-9 and close Mars's
687-day period. There is no n-body propagator, no symplectic or adaptive
integrator, and no error report attached to a result.

## What it builds

- `aethyr/space/propagate.coax`: two-body, patched-conic and restricted
  n-body models behind one interface.
- `aethyr/space/integrators.coax`: RK4, leapfrog, Yoshida compositions and
  Dormand–Prince 5(4), each returning a `PropagationReport`.
- `aethyr/space/envelope.coax`: the Newtonian envelope and the
  constant-proper-acceleration mode.

## Acceptance checks and oracles

- Two-body propagation against Kepler's closed form.
- Many-period closure: bounded symplectic energy error, against the measured
  drift of the adaptive integrator over the same span.
- n-body against chapter 39's pinned ephemeris over a stated span.
- The patched-conic versus n-body difference for a transfer reported as a
  number.
- Step-halving convergence at each method's stated order.
- An out-of-envelope state refused; the proper-acceleration mode against its
  closed forms.

## Dependencies

[Chapter 39](ch039-ephemerides-time-scales-and-frames.md)'s states and
time scales; [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s fixed step;
[chapter 18](ch018-rigid-bodies-and-time-steps.md)'s integrator profiles.
From Coax: R15 integrators and deterministic schedules.
