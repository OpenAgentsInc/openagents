# 28. Smoke on a grid

> **Status: partly implemented.** [`aethyr/sim/grid-fluid.coax`](../../aethyr/sim/grid-fluid.coax)
> (996 mutants: 963 killed, 33 marked equivalent with their reason)
> runs stable fluids on a 2D MAC grid on the CPU in f64 (profile
> `stable-fluids-2d-f64-v1`): semi-Lagrangian advection, implicit viscosity
> and pressure projection, both by preconditioned conjugate gradients with
> their residuals recorded, smoke as a passive scalar with buoyancy, and
> periodic or closed boundaries. 3D, multigrid, vorticity confinement, solid
> cells from distance fields, moving boundaries and device kernels are not
> built yet.

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

## What runs today

[`aethyr/sim/grid-fluid.coax`](../../aethyr/sim/grid-fluid.coax) keeps a 2D
grid of up to 64 by 64 cells (`Gf`). Velocity lives on the cell faces, `u`
on the vertical faces and `v` on the horizontal ones; pressure and smoke
live at the cell centers. The grid is periodic or a closed box; in a box the
normal velocity on each wall is zero and the tangential velocity slips.

`gf_step` takes one step in four parts:

1. `gf_advect` traces each face, and each smoke cell, back through the old
   velocity by the midpoint rule and interpolates bilinearly there. It
   samples only values that already exist, so it cannot blow up.
2. Gravity and buoyancy (an upward acceleration per unit of smoke) are
   added to the faces.
3. `gf_diffuse` solves `(I − νΔt∇²)u′ = u` for each component.
4. `gf_project` solves `∇²p = ∇·u/Δt` and subtracts `Δt∇p`, which leaves
   every cell divergence-free.

Advection comes first, on a field that the last step left divergence-free.
In the order Stam's paper gives, forces added before advection are carried
by an unprojected field near the walls, and a box of still air under gravity
starts to move. Both solves are matrix-free conjugate gradients with a
Jacobi preconditioner on the 5-point Laplacian. Each stops at a relative
residual of 10⁻¹⁰, and the step records its iteration count and final
residual. The pressure system of a periodic grid or a closed box is
singular, with the constants as its null space, so the solver removes the
mean of the right-hand side and of the starting guess.

The example [`smoke-and-vortex.coax`](../../aethyr/examples/smoke-and-vortex.coax)
runs the Taylor–Green vortex on three grids, still air under gravity and a
rising puff of smoke:

<!-- source: aethyr/examples/smoke-and-vortex.coax example -->
```jai
main :: () {
  g: [1] Gf; work: [1] Gf_Work;
  print("Taylor-Green vortex, side 2 pi, nu = 0.01, 1 s at dt 0.05:\n");
  for n: [3] int.[16, 32, 64] {
    if !gf_taylor_green(g[..], n, 0.01) { return; }
    e0 := gf_energy(g[..]); most := 0;
    for k: 0..<20 {
      gf_step(g[..], work[..], 0.05, d2(0.0, 0.0), 0.0);
      if g[0].project_iterations > most { most = g[0].project_iterations; }
    }
    fit := -log(gf_energy(g[..]) / e0) / 4.0;
    print("  % x %: energy ratio % (exact exp(-4 nu t) = %), fitted nu %, numerical %; divergence %; up to % CG iterations\n",
          n, n, r6(gf_energy(g[..]) / e0), r6(exp(-0.04)), r6(fit), r6(fit - 0.01), r6(gf_max_div(g[..])), most);
  }

  // A closed 1.6 m box of still air, 20 steps under gravity.
  if !gf_new(g[..], 16, 16, 0.1, false) { return; }
  for k: 0..<20 { gf_step(g[..], work[..], 0.05, d2(0.0, -9.81), 0.0); }
  fastest := 0.0;
  for k: 0..<GF_FACES { fastest = fmax(fastest, fmax(abs(g[0].u[k]), abs(g[0].v[k]))); }
  print("still air under gravity: fastest face % m/s; pressure difference over 1 m % Pa per unit density\n",
        r6(fastest), r6(g[0].p[0] - g[0].p[10 * 16]));

  // Warm smoke in a 0.8 x 1.6 m box, lifted 4 m/s^2 per unit of smoke.
  if !gf_new(g[..], 16, 32, 0.05, false) { return; }
  for j: 2..<5 { for i: 6..<10 { g[0].smoke[j * 16 + i] = 1.0; } }
  print("smoke: mean height % m\n", r4(gf_smoke_height(g[..])));
  for k: 0..<60 {
    gf_step(g[..], work[..], 0.02, d2(0.0, 0.0), 4.0);
    if k % 20 == 19 {
      print("  % s: mean height % m, divergence %\n", r4(cast(f64) (k + 1) * 0.02), r4(gf_smoke_height(g[..])), r6(gf_max_div(g[..])));
    }
  }
}
```

Run it with:

```shell
coax/bin/coax run aethyr/examples/smoke-and-vortex.coax
```

```text
Taylor-Green vortex, side 2 pi, nu = 0.01, 1 s at dt 0.05:
  16 x 16: energy ratio 0.704432 (exact exp(-4 nu t) = 0.960789), fitted nu 0.087591, numerical 0.077591; divergence 0.0; up to 13 CG iterations
  32 x 32: energy ratio 0.817944 (exact exp(-4 nu t) = 0.960789), fitted nu 0.05024, numerical 0.04024; divergence 0.0; up to 35 CG iterations
  64 x 64: energy ratio 0.887326 (exact exp(-4 nu t) = 0.960789), fitted nu 0.029886, numerical 0.019886; divergence 0.0; up to 73 CG iterations
still air under gravity: fastest face 0.0 m/s; pressure difference over 1 m 9.81 Pa per unit density
smoke: mean height 0.175 m
  0.4 s: mean height 0.2422 m, divergence 0.0
  0.8 s: mean height 0.3389 m, divergence 0.0
  1.2 s: mean height 0.4179 m, divergence 0.0
```

The vortex loses energy much faster than its viscosity alone explains. The
viscosity that fits its decay is the set 0.01 m²/s plus a numerical
viscosity from interpolation, and that extra falls by about half each time
the grid is refined: 0.078, 0.040 and 0.020 m²/s at 16, 32 and 64 cells a
side. Semi-Lagrangian advection with bilinear interpolation is first order,
which is exactly what that halving shows. Still air stays still to the
printed precision because the pressure grows downward by exactly `g` per
metre per unit density. The smoke rises at about 0.2 m/s.

The checks run with:

```shell
coax/bin/coax test aethyr/sim/grid-fluid.coax --library
```

They project a rough field on periodic and closed grids and require every
cell's divergence below 10⁻⁷ at the solver's tolerance. They keep still air
still under gravity, with the pressure difference over one metre equal to
`g`. They require the Taylor–Green vortex to decay at least as fast as
`exp(−2νt)` in amplitude, with less numerical viscosity on the finer grid,
and the smoke to rise straight up in a symmetric box.

## The design in Aethyr

A bounded MAC grid with solid cells from rigid SDFs (chapter 21), CG with
incomplete-Cholesky or multigrid as profiles, and residuals recorded per
step. Passive scalars (smoke density, temperature) are advected the same
way and feed chapter 33's temperature sensor. Moving rigid boundaries set
the normal velocity in solid faces; forces back on bodies are a later
coupling profile.

## What it still builds

- A 3D grid, and solid cells from rigid distance fields (chapter 21) with
  moving rigid boundaries setting the normal velocity.
- A multigrid V-cycle as a second solver profile, checked against CG, and an
  incomplete-Cholesky preconditioner.
- Vorticity confinement, and a higher-order advection profile to reduce the
  numerical viscosity measured above.
- Temperature as a second scalar for chapter 33's sensor.
- Device kernels through Psionic chapter 37's compute subset.

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
