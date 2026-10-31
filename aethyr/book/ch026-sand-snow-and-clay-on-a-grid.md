# 26. Sand, snow and clay on a grid

> **Status: unimplemented — design draft.** Nothing here runs yet.

Sand pours and then piles, snow compacts and fractures, clay keeps the shape
you press into it. These materials flow like liquids under some loads and
hold like solids under others, and they undergo huge deformations. The
material point method (MPM) handles them by carrying state on particles
(mass, velocity, deformation gradient) while solving momentum on a
background grid that is rebuilt each step. Particles never tangle, because
the grid, not a mesh, connects them.

## What this chapter teaches

- The particle-to-grid and grid-to-particle transfers, and affine particle
  in cell (APIC).
- Moving-least-squares MPM (MLS-MPM), which makes the update compact and
  fast.
- Deformation gradients and constitutive models: neo-Hookean elasticity,
  fixed corotated, snow plasticity, Drucker–Prager sand, von Mises clay,
  and active muscle.
- Grid boundary conditions and rigid coupling through the grid (CPIC).

## The physics

Each particle `p` carries `x_p, v_p`, an affine matrix `C_p` and a
deformation gradient `F_p`. With quadratic B-spline weights `w_ip`:

```jai
P2G:  m_i v_i = Σ_p w_ip m_p (v_p + C_p (x_i − x_p))
      − Δt Σ_p w_ip V_p⁰ (4/Δx²) P(F_p) F_pᵀ (x_i − x_p)
Grid: v_i ← v_i / m_i + Δt g;   apply boundaries
G2P:  v_p = Σ_i w_ip v_i;  C_p = (4/Δx²) Σ_i w_ip v_i (x_i − x_p)ᵀ
      F_p ← (I + Δt C_p) F_p;  x_p += Δt v_p;  then return F_p to its yield surface
```

`P(F)` is the first Piola–Kirchhoff stress from the chosen energy
([Hu et al., MLS-MPM, SIGGRAPH 2018](https://doi.org/10.1145/3197517.3201293)).
Snow clamps singular values of `F` and hardens with compression
([Stomakhin et al., SIGGRAPH 2013](https://doi.org/10.1145/2461912.2461948));
sand projects onto a Drucker–Prager cone in log-strain space
([Klár et al., SIGGRAPH 2016](https://doi.org/10.1145/2897824.2925906)).
Time steps obey `Δt ≤ C Δx / (|v|_max + c)`.

## The design in Aethyr

A dense bounded grid first, with a sparse block grid later as a measured
layout choice (R21) rather than a different solver. The scatter in P2G is
where determinism is lost on GPUs: atomic float adds give a different sum
each run. The deterministic profile sorts particles by grid block (chapter
23) and reduces per block in a fixed order; the fast profile uses atomics
and states that it is not bitwise reproducible. Constitutive models are
separate routines over `F`, each with its own energy and its return mapping
checked separately. CPIC (compatible particle-in-cell) lets thin rigid
shapes cut the grid, which chapter 29 uses for rigid coupling.

## What it builds

- `aethyr/sim/mpm.coax`: P2G/G2P with APIC, grid update and boundaries,
  constitutive models and return mappings, the deterministic reduction.
- Fixtures: elastic cube bounce, sand column collapse, snowball impact.

## Acceptance checks and oracles

- Each energy's stress against finite differences of the energy;
  rotation invariance of `P(RF) = R P(F)`.
- Return mappings land on the yield surface and are idempotent.
- Total mass and (without boundaries) momentum conserved exactly in sum
  over a step.
- Sand column collapse: runout distance against the scaling reported by
  Lube et al. (2005) for the column's aspect ratio, within a stated band.
- Elastic bounce against the same scene in chapter 27's FEM within a
  stated tolerance.
- Deterministic profile bitwise identical across worker counts and runs.

## How Genesis does it

Genesis's MPM solver supports elastic, plastic, sand, snow and liquid-like
materials and muscle, with CPIC rigid coupling and reverse-mode gradients
([MPM solver](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/mpm_solver/index.html),
[differentiable simulation](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/differentiable_simulation.html)).
MPM is one of the two solvers Genesis differentiates fully; it is ours too
(chapter 32). We add a deterministic scatter profile alongside the atomic
one.

## Dependencies

Chapter 23 sorting, chapter 29 coupling, chapter 32 for gradients. From
Coax: R15 strict reductions, R21 layouts (sparse grids), R24 kernel
derivatives.
