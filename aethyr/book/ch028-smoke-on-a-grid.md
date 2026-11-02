# 28. Smoke on a grid

> **Status: unimplemented — design draft.** Nothing here runs yet.

Smoke and air are gases: they move as a velocity field filling space, and
what matters is the swirl that field carries along. Stable fluids solves
the incompressible Navier–Stokes equations on a fixed grid with steps that
never blow up, however large the time step, at the cost of some numerical
viscosity. It is the base case for grid fluids and the simplest solver in
this part.

## What this chapter teaches

- Operator splitting for incompressible flow: advect, add forces, project.
- Semi-Lagrangian advection and why it is unconditionally stable.
- Pressure projection by solving a Poisson equation.
- Staggered (MAC) grids and boundary conditions.

## The physics

```jai
∂u/∂t + (u·∇)u = −∇p/ρ + ν ∇²u + f,   ∇·u = 0
advect:   u*(x) = u(x − Δt u(x))                // trace back, interpolate
project:  ∇² p = (ρ/Δt) ∇·u*;   u = u* − (Δt/ρ) ∇p
```

([Stam, *Stable Fluids*, SIGGRAPH 1999](https://doi.org/10.1145/311535.311548);
[Bridson, *Fluid Simulation for Computer Graphics*, 2015](https://doi.org/10.1201/9781315266008)).
The Poisson solve is the cost: preconditioned conjugate gradients or a
multigrid V-cycle. Vorticity confinement restores some of the swirl that
interpolation damps.

## The design in Aethyr

A bounded MAC grid with solid cells from rigid SDFs (chapter 21), CG with
incomplete-Cholesky or multigrid as profiles, and residuals recorded per
step. Passive scalars (smoke density, temperature) are advected the same
way and feed chapter 33's temperature sensor. Moving rigid boundaries set
the normal velocity in solid faces; forces back on bodies are a later
coupling profile.

## What it builds

- `aethyr/sim/grid-fluid.coax`: MAC grid, semi-Lagrangian advection,
  projection with CG and multigrid, scalars, vorticity confinement.

## Acceptance checks and oracles

- After projection `|∇·u|` is below the solver tolerance in every cell.
- Taylor–Green vortex: decay rate against the analytic `exp(−2νk²t)` at
  stated resolutions, with the numerical viscosity reported.
- Rest state stays at rest under gravity with pressure balancing it.
- Multigrid and CG give the same field within tolerance.

## How Genesis does it

Genesis includes a Stable Fluids solver for grid-based gases
([solvers](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/index.html)).
It is a small part of Genesis and of our plan; we include it for coverage
and as the first consumer of a Poisson solver.

## Dependencies

Chapter 21 SDFs and Psionic chapter 4 sparse solvers. Chapter 33 later reads
scalar fields as sensor outputs; that read-only integration is not a
prerequisite for the grid-fluid solve. From Coax: R21 layouts, R23 device
graphs for the CG loop.
