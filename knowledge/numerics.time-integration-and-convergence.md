---
id: numerics.time-integration-and-convergence
version: 1
kind: method
title: Choose a time integrator for the physics and prove it converged
summary: >-
  Match the integrator to the system: symplectic methods such as velocity
  Verlet for conservative mechanics, implicit or adaptive stiff solvers for
  stiff springs, contacts, or diffusion, and event detection for impacts. Then
  verify with a step-halving convergence study and conserved quantities.
tags: [physics-simulation, ode, numerical-integration, stiffness, verlet, scipy]
applies_when: >-
  Writing or fixing a simulation that advances a physical system in time
  (rigid bodies, particles, springs, contact, orbits, reaction or diffusion
  equations) and whose output is compared with expected trajectories or values.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Hairer, Lubich, Wanner, Geometric Numerical Integration, 2nd ed. (Springer, 2006), chapters on symplectic integration and the Stormer-Verlet method"
    - "Hairer and Wanner, Solving Ordinary Differential Equations II: Stiff and Differential-Algebraic Problems, 2nd ed. (Springer, 1996)"
    - "SciPy documentation, scipy.integrate.solve_ivp: methods RK45, DOP853, Radau, BDF, LSODA; events"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

- **Conservative mechanics** (orbits, pendulums, molecular dynamics): explicit
  Euler gains energy every step. Semi-implicit (symplectic) Euler or velocity
  Verlet keep energy bounded over long runs at the same cost. Velocity Verlet:
  `v += a(x) dt/2; x += v dt; v += a(x) dt/2`.
- **Stiff systems** (very stiff springs, penalty contact, chemical kinetics,
  diffusion on fine grids): explicit methods need a step below roughly
  `2/|λ_max|` to stay stable, which shows up as sudden blow-up. Use an
  implicit method (`solve_ivp(method="Radau"|"BDF")`, or `"LSODA"` to switch
  automatically), supply a Jacobian when possible, or split the stiff linear
  part and treat it implicitly or exactly (IMEX, exponential integrators).
- **Contact and impacts:** locate the event instead of letting bodies
  interpenetrate: `solve_ivp(events=f)` with `f.terminal = True` and
  `f.direction`, apply the impact law (restitution, friction cone), then
  restart. Fixed-step penalty methods need small steps and damping.
- **Adaptive solvers:** set `rtol` and `atol` deliberately (`atol` scaled to
  each variable's magnitude) and use `t_eval` or dense output to report at the
  required times rather than at solver steps.
- Keep the same step, tolerance, and floating-point order a reference used
  when outputs must match it closely; a different but valid integrator gives a
  different trajectory in chaotic systems.

## How to check

Run a convergence study: halve the step (or tighten tolerances by 10x) and
confirm the error against the finest run shrinks at the method's order
(about 4x per halving for a second-order method). Track invariants such as
energy, momentum, or mass and confirm drift is bounded. Test a special case
with a known analytic solution (harmonic oscillator, free fall, linear decay)
before the full problem.
