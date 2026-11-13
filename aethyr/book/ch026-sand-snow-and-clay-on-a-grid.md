# 26. Sand, snow and clay on a grid

> **Status: partly implemented.** [`aethyr/physics/sim/mpm.coax`](../physics/sim/mpm.coax)
> (449 mutants: 437 killed, 12 marked equivalent with their reason)
> runs MLS-MPM with APIC transfers in 2D on a dense grid, on the CPU in f64
> (profile `mpm-mls-2d-f64-v1`), with neo-Hookean, fixed corotated and
> Drucker–Prager sand materials. Snow, clay, muscle, 3D, a sparse grid,
> rigid coupling through CPIC, the sorted per-block reduction and device
> kernels are not built yet.

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

## What runs today

[`aethyr/physics/sim/mpm.coax`](../physics/sim/mpm.coax) keeps up to 4096
particles (`Mpm`) over a grid of up to 64 by 64 cells on the unit square.
Each particle carries its position, velocity, affine matrix `C` and
deformation gradient `F`. `mpm_step` scatters mass and momentum to the
nodes with quadratic B-spline weights, adds the stress term of MLS-MPM in
the same pass, adds gravity on the grid, applies the walls, and gathers the
new `v`, `C` and `F` back. Nothing moves into the outer three cells; the
side walls slip, and the floor grips what presses on it and lets go of what
lifts off.

The scatter runs in particle order on one thread, so a step gives the same
bits on every run. The sorted per-block reduction that keeps that true on
many workers is not built yet.

Each material is an energy density (`mpm_energy`) and its first
Piola–Kirchhoff stress (`mpm_stress`). The 2D SVD that the corotated and
sand models need is the polar rotation of `F` followed by the eigenvectors
of the symmetric part, with no trigonometric function. Sand uses the Hencky
energy on log strain and, after each step, `mpm_sand_project` returns `F`
to the Drucker–Prager cone with a 30° friction angle (Klár et al., section
7.3). A stretched grain cell lets go of all its strain, because sand cannot
pull.

The example [`sand-and-jelly.coax`](../../aethyr/examples/sand-and-jelly.coax)
drops a block of jelly and collapses two columns of sand against a wall:

<!-- source: aethyr/examples/sand-and-jelly.coax example -->
```jai
// The lowest particle's height and the mean height.
Heights :: struct { low: f64; mean: f64; }
heights :: (m: [] Mpm) -> Heights {
  out: Heights; out.low = 1.0;
  for p: 0..<m[0].count { out.low = fmin(out.low, m[0].x[p].y); out.mean += m[0].x[p].y; }
  out.mean /= cast(f64) m[0].count;
  return out;
}

main :: () {
  // A 0.2 m jelly block, E = 20 kPa, nu = 0.3, dropped from 0.5 m.
  m: [1] Mpm;
  w := Mpm_World.{n = 64, dt = 0.0002, gravity = d2(0.0, -9.81), mu = mpm_mu(20000.0, 0.3), la = mpm_la(20000.0, 0.3)};
  w.volume = (0.5 / 64.0) * (0.5 / 64.0);
  print("jelly: % particles\n", mpm_block(m[..], w, d2(0.4, 0.5), d2(0.6, 0.7), d2(0.0, 0.0), Mpm_Material.corotated));
  for k: 0..<5000 {
    mpm_step(m[..], w);
    if k % 500 == 499 {
      h := heights(m[..]);
      print("  % s: lowest % m, mean % m\n", r4(cast(f64) (k + 1) * w.dt), r4(h.low), r4(h.mean));
    }
  }

  // Sand columns 0.1 m wide against the left wall, 0.1 m and 0.2 m tall.
  for tall: [2] f64.[0.1, 0.2] {
    s: [1] Mpm;
    v := Mpm_World.{n = 64, dt = 0.0002, gravity = d2(0.0, -9.81), mu = mpm_mu(100000.0, 0.3), la = mpm_la(100000.0, 0.3)};
    v.rho = 1600.0; v.volume = (0.5 / 64.0) * (0.5 / 64.0);
    floor_y := 3.0 / 64.0; wall_x := 3.0 / 64.0;
    n := mpm_block(s[..], v, d2(wall_x, floor_y), d2(wall_x + 0.1, floor_y + tall), d2(0.0, 0.0), Mpm_Material.sand);
    for k: 0..<5000 { mpm_step(s[..], v); }
    reach := 0.0; top := 0.0;
    for p: 0..<s[0].count { reach = fmax(reach, s[0].x[p].x - wall_x); top = fmax(top, s[0].x[p].y - floor_y); }
    print("sand column, aspect ratio %: % particles; after 1 s it reaches % m from the wall (runout %), % m tall\n",
          r4(tall / 0.1), n, r4(reach), r4((reach - 0.1) / 0.1), r4(top));
  }
}
```

Run it with:

```shell
coax/bin/coax run aethyr/examples/sand-and-jelly.coax
```

```text
jelly: 625 particles
  0.1 s: lowest 0.4548 m, mean 0.5485 m
  0.2 s: lowest 0.3075 m, mean 0.4013 m
  0.3 s: lowest 0.0622 m, mean 0.1559 m
  0.4 s: lowest 0.0503 m, mean 0.1486 m
  0.5 s: lowest 0.2536 m, mean 0.351 m
  0.6 s: lowest 0.3563 m, mean 0.4552 m
  0.7 s: lowest 0.3628 m, mean 0.4614 m
  0.8 s: lowest 0.273 m, mean 0.3694 m
  0.9 s: lowest 0.0832 m, mean 0.1794 m
  1.0 s: lowest 0.0402 m, mean 0.1208 m
sand column, aspect ratio 1.0: 144 particles; after 1 s it reaches 0.1998 m from the wall (runout 0.9978), 0.0629 m tall
sand column, aspect ratio 2.0: 300 particles; after 1 s it reaches 0.3318 m from the wall (runout 2.3182), 0.0824 m tall
```

The jelly falls, squashes on the floor at about 0.4 s and bounces back to
about 0.36 m, below the 0.5 m it fell from, because the grid transfers lose
some energy each step. The sand columns spread to a runout `ΔL/L₀` of 1.00
at aspect ratio 1 and 2.32 at aspect ratio 2. Lube et al. (2005) measured
`ΔL/L₀ ≈ 1.2a` for two-dimensional columns below an aspect ratio of about
2.3, which gives 1.2 and 2.4. The taller column agrees within 4%, and the
squat one runs about 17% short.

The checks run with:

```shell
coax/bin/coax test aethyr/physics/sim/mpm.coax --library
```

They compare each material's stress with central differences of its energy,
and require `P(RF) = R P(F)`, an unchanged energy under rotation, and no
stress at rest. They rebuild `F` from its SVD with rotation factors. They
require the sand return mapping to land on the cone, to be idempotent, to
release all strain under volumetric stretch, and to leave pure compression
alone. They require one transfer to conserve mass and momentum exactly, and
one whole step without gravity or walls to conserve momentum. A soft block
set on the floor sags a few millimeters and springs back without sinking
through, and a full particle store refuses one more particle.

## The design in the Metaverse Engine

A dense bounded grid first, with a sparse block grid later as a measured
layout choice (R21) rather than a different solver. The scatter in P2G is
where determinism is lost on GPUs: atomic float adds give a different sum
each run. The deterministic profile sorts particles by grid block (chapter
23) and reduces per block in a fixed order; the fast profile uses atomics
and states that it is not bitwise reproducible. Constitutive models are
separate routines over `F`, each with its own energy and its return mapping
checked separately. CPIC (compatible particle-in-cell) lets thin rigid
shapes cut the grid, which chapter 29 uses for rigid coupling.

## What it still builds

- 3D, then a sparse block grid as a measured layout choice (R21).
- Snow (clamped singular values with hardening), von Mises clay and active
  muscle.
- The deterministic per-block reduction for many workers, sorting by block
  with chapter 23, and the atomic fast profile beside it.
- CPIC rigid coupling for chapter 29, and the gradients for chapter 32.
- The sand-column runout as a check within a stated band, and the elastic
  bounce against chapter 27's FEM.
- Device kernels through Psionic chapter 37's compute subset.

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

Chapter 23 sorting supplies the first forward MPM profile. Chapter 27's
elastic-bounce comparison is an independent cross-solver check once both
forward solvers exist.

Chapter 29 later adds coupling, and chapter 32 differentiates the forward
solver. Their checks remain required for coupled or gradient profiles;
neither is needed to compute the first forward trajectory. From Coax: R15
strict reductions and R21 layouts (sparse grids); R24 kernel derivatives
for the gradient extension.
