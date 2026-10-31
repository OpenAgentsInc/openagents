# 172. Liquids from particles

> **Status: unimplemented — design draft.** Aethyr's water labs shade a
> surface; none simulates the liquid. This chapter plans smoothed-particle
> hydrodynamics (SPH) on chapter 171's particles. Nothing here runs yet.

A liquid has no shape of its own, and it is nearly incompressible: squeeze
water by a fraction of a percent and the pressure rises enormously. SPH
represents the liquid by particles that carry mass and estimates density,
pressure and viscosity from weighted sums over neighbors. The hard part is
incompressibility: an explicit pressure force needs tiny time steps, and
solving for pressure costs iterations.

## What this chapter teaches

- Kernel interpolation, and estimating a field and its gradient from
  particles.
- Weakly compressible SPH (WCSPH) with an equation of state.
- Divergence-free SPH (DFSPH): solving for pressure so density stays
  constant and velocity stays divergence-free.
- Viscosity, surface tension and boundaries.

## The physics

With a smoothing kernel `W(r, h)` (cubic spline or Wendland), the density
estimate and the symmetric pressure force are

```jai
ρ_i   = Σ_j m_j W(x_i − x_j, h)
a_i^p = −Σ_j m_j (p_i/ρ_i² + p_j/ρ_j²) ∇W(x_i − x_j, h)
```

WCSPH closes the system with Tait's equation `p = B((ρ/ρ₀)^γ − 1)` and
`γ = 7`, with stiffness `B` set for a chosen maximum density error and a CFL
time step `Δt ≤ λ h / c_s` ([Becker and Teschner, SCA 2007](https://doi.org/10.2312/SCA/SCA07/209-217)).
DFSPH instead solves two Jacobi-style systems per step, one making the
velocity field divergence-free and one correcting density, allowing much
larger steps ([Bender and Koschier, SCA 2015](https://doi.org/10.1145/2786784.2786796)).
Viscosity is an artificial-viscosity or XSPH term; boundaries are particle
layers with their own volume (Akinci et al. 2012), which also give two-way
coupling with rigid bodies.

## The design in Psionic

WCSPH is the first profile because it is explicit, simple and the easiest
reference; DFSPH follows with iteration count and tolerance as profile
fields and residuals reported per step. Time step is adaptive by CFL only if
the profile says so, and the chosen step sequence is recorded. Boundary
particles sampled from rigid shapes (chapter 171) carry the rigid body's
velocity, and the force they receive is the coupling force chapter 177
returns to the rigid solver.

## What it builds

- `psionic/lib/sim/sph.coax`: kernels, density, WCSPH, DFSPH, viscosity,
  boundary particles.
- Fixtures: hydrostatic column, dam break, a cube floating in a tank.

## Acceptance checks and oracles

- Kernel normalization `∫W = 1` and gradient symmetry, numerically.
- Hydrostatic column: pressure at depth `d` equals `ρ₀ g d` within the
  profile's stated density error; density error stays under its bound.
- Dam break: front position over time against the Martin and Moyce (1952)
  measurements, within a stated band.
- Archimedes: a floating cube settles at the draft `ρ_body/ρ_fluid` of its
  height.
- Momentum conserved by fluid–fluid forces (antisymmetry), checked exactly
  in sum.

## How Genesis does it

Genesis ships WCSPH and DFSPH particle-based liquids
([SPH solver](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/sph_solver/index.html));
its differentiability table lists SPH backward as a stub
([differentiable simulation](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/differentiable_simulation.html)).
We plan the same two methods. Our compatibility matrix states the gradient
status as "not supported" and refuses a gradient request, rather than
offering a stub.

## Dependencies

Chapter 171's neighbors; chapter 177's coupling for boundary forces.
Rendering of a surface from particles is chapter 182's. From Coax: R15,
R21, R23.
