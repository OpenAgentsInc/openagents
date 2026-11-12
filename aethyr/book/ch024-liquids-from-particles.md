# 24. Liquids from particles

> **Status: partly implemented.** [`aethyr/physics/sim/sph.coax`](../physics/sim/sph.coax)
> (266 mutants: 256 killed, 10 marked equivalent with their reason)
> runs weakly compressible SPH (profile `wcsph-f64-v1`) on chapter 23's
> particles on the CPU in f64: the cubic spline kernel, density, Tait
> pressure, symmetric pressure and viscosity forces, fixed wall samples and
> a refused step past the CFL bound. A water column settles in a tank with
> the pressure gradient within 5% of `ρ₀g`. DFSPH, surface tension, the dam
> break against Martin and Moyce, moving boundaries, the floating cube and
> device kernels are not built yet.

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

## What runs today

[`aethyr/physics/sim/sph.coax`](../physics/sim/sph.coax) computes the
fluid on chapter 23's store and neighbor search. A particle of material
`SPH_FLUID` moves; one of `SPH_WALL` is a fixed boundary sample.

- `sph_w` and `sph_grad` are the cubic spline kernel with support `h`,
  normalized to integrate to 1 in three dimensions, and its gradient. The
  gradient is odd in the offset, so `∇W(−d)` is exactly `−∇W(d)`.
- `sph_density` sums each fluid particle's own mass and its neighbors'
  through the kernel, and applies Tait's equation with `γ = 7` and
  `B = ρ₀c²/7`, clamped at zero so the free surface does not pull.
- `sph_pair` is the force between two fluid particles: the symmetric
  pressure term above and Monaghan's viscosity on the mean density. Every
  factor is symmetric in the pair, so the force one particle feels is
  exactly the negative of the other's.
- `sph_step` finds neighbors, then computes density, pressure and forces,
  and moves the fluid by semi-implicit Euler. A step longer than the CFL
  bound `0.4h/c` is refused, and so is a full neighbor list, because a sum
  over a cut list would be wrong.

The walls are frozen fluid particles placed on the fluid's lattice with
the fluid's mass. Each one mirrors the pressure and density of the
particle it pushes. These are the dummy particles of
[Adami, Hu and Adams, 2012](https://doi.org/10.1016/j.jcp.2012.05.005),
with the wall pressure mirrored instead of extrapolated from the fluid.
One layer is enough while the support is at most two spacings, because no
fluid particle then reaches past it. The particle mass comes from
`sph_lattice_mass`: rest density over the kernel sum of a full lattice, so
an interior particle starts at exactly `ρ₀`. With mass `ρ₀a³` the lattice
starts squeezed or stretched by the kernel's discretization error, and
Tait's equation turns a few percent of that into a burst.

The example [`water-column.coax`](../../aethyr/examples/water-column.coax)
settles a column of water in a tank:

<!-- source: aethyr/examples/water-column.coax example -->
```jai
main :: () {
  p: [1] Pstore; s: [1] Sph; g: [1] Pgrid; n: [1] Pneighbors;
  a := 0.04;
  s[0].h = 2.0 * a; s[0].sound = 30.0; s[0].viscosity = 0.03; s[0].gravity = d3(0.0, -9.81, 0.0);
  s[0].lo = D3_ZERO; s[0].hi = d3(0.4, 0.8, 0.4);
  // Walls 5 spacings apart hold a 4 x 4 x 12 column.
  m := sph_lattice_mass(a, s[0].h, s[0].rest);
  ok := sph_tank(p[..], d3(0.1, 0.1, 0.1), d3(0.1 + 5.0 * a, 0.1 + 14.0 * a, 0.1 + 5.0 * a), a, m);
  walls := p[0].count;
  ok = ok && sph_block(p[..], d3(0.1 + a, 0.1 + a, 0.1 + a), 4, 12, 4, a, m);
  if !ok { print("the tank does not fit\n"); return; }
  print("% wall samples, % fluid particles, h = % m, c = % m/s, CFL step % s\n", walls, p[0].count - walls, s[0].h, s[0].sound, r4(sph_cfl(s[..]) * 1000.0) / 1000.0);
  print("a step of 0.0011 s: %\n", ifx sph_step(p[..], s[..], g[..], n[..], 0.0011) == Sph_Refusal.step_too_long then "refused" else "taken");

  dt := 0.001;
  for k: 0..<2000 {
    if sph_step(p[..], s[..], g[..], n[..], dt) != Sph_Refusal.none { print("refused at step %\n", k); return; }
    if k % 500 == 499 {
      // Density error below the top two spacings; a surface particle has
      // fewer neighbors, so its estimate is low by construction.
      worst := 0.0; mean := 0.0; inside := 0; energy := 0.0; top := 0.0;
      for i: walls..<p[0].count { top = fmax(top, p[0].pos[i].y); energy += 0.5 * p[0].mass[i] * d3_len2(p[0].vel[i]); }
      for i: walls..<p[0].count {
        if p[0].pos[i].y > top - 2.0 * a { continue; }
        e := s[0].density[i] / s[0].rest - 1.0;
        worst = fmax(worst, abs(e)); mean += e; inside += 1;
      }
      print("  t = % s: density error below the surface mean %, worst %; kinetic energy % J; top particle at y = % m\n",
            r4(cast(f64) (k + 1) * dt), r4(mean / cast(f64) inside), r4(worst), r4(energy), r4(top));
    }
  }

  // Least squares of pressure on height, over the particles more than two
  // spacings below the top one: hydrostatics wants a slope of -rho0 g.
  top := 0.0;
  for i: walls..<p[0].count { top = fmax(top, p[0].pos[i].y); }
  sy := 0.0; sp := 0.0; syy := 0.0; syp := 0.0; count := 0.0;
  for i: walls..<p[0].count {
    y := p[0].pos[i].y;
    if y > top - 2.0 * a { continue; }
    sy += y; sp += s[0].pressure[i]; syy += y * y; syp += y * s[0].pressure[i]; count += 1.0;
  }
  slope := (count * syp - sy * sp) / (count * syy - sy * sy);
  print("pressure against height over % particles: slope % Pa/m, -rho0 g = % Pa/m, ratio %\n",
        count, r4(slope), -s[0].rest * 9.81, r4(slope / (-s[0].rest * 9.81)));
}
```

Run it with:

```shell
coax/bin/coax run aethyr/examples/water-column.coax
```

```text
316 wall samples, 192 fluid particles, h = 0.08 m, c = 30.0 m/s, CFL step 0.0010667 s
a step of 0.0011 s: refused
  t = 0.5 s: density error below the surface mean 0.0019, worst 0.0043; kinetic energy 0.0151 J; top particle at y = 0.5962 m
  t = 1.0 s: density error below the surface mean 0.0035, worst 0.0066; kinetic energy 0.0339 J; top particle at y = 0.5966 m
  t = 1.5 s: density error below the surface mean 0.0024, worst 0.0055; kinetic energy 0.0395 J; top particle at y = 0.5938 m
  t = 2.0 s: density error below the surface mean 0.0028, worst 0.0051; kinetic energy 0.0133 J; top particle at y = 0.5913 m
pressure against height over 152.0 particles: slope -9382.3903 Pa/m, -rho0 g = -9810.0 Pa/m, ratio 0.9564
```

Below the surface, density stays within 0.7% of `ρ₀`, the weak
compressibility that `c = 30 m/s` allows. The pressure gradient over the
same particles is 4.4% short of `ρ₀g`. The column is only four particles
wide, so walls and the free surface touch most of it, and it has not come
fully to rest: its kinetic energy still varies between 0.01 J and 0.04 J.
The viscosity, 0.03 m²/s, is artificial: with 0.0005 m²/s the same column
splashes out of the tank within half a second.

The checks run with:

```shell
coax/bin/coax test aethyr/physics/sim/sph.coax --library
```

They integrate the kernel to 1 within 0.2% for three supports, match the
gradient against a central difference, and check that it is odd. They check
Tait's equation, that every fluid pair's forces are exact negatives on a
jittered, moving, squeezed block, with the total within 10⁻¹² of the sum
of magnitudes, the refusals, that a wall pushes a particle straight out,
and that the lattice mass gives an interior particle exactly the rest
density.

## The design in Aethyr

WCSPH is the first profile because it is explicit, simple and the easiest
reference; DFSPH follows with iteration count and tolerance as profile
fields and residuals reported per step. Time step is adaptive by CFL only if
the profile says so, and the chosen step sequence is recorded. Boundary
particles sampled from rigid shapes (chapter 23) carry the rigid body's
velocity, and the force they receive is the coupling force chapter 29
returns to the rigid solver.

## What it still builds

- DFSPH, with iteration count and tolerance as profile fields and
  residuals reported per step, and an adaptive CFL step recorded per step.
- Akinci's boundary volumes for walls that are not on the fluid's lattice,
  and moving boundaries sampled from rigid bodies.
- Surface tension.
- The dam break against Martin and Moyce, and the floating cube through
  chapter 29's coupling.
- Device kernels through Psionic chapter 37's compute subset.

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

Chapter 23's neighbors supply the first fluid profile, with prescribed
boundary geometry and motion. It can qualify fluid forces and the dam break
before another solver responds to the fluid.

Chapter 29 later adds two-way boundary forces: the floating-cube acceptance
check needs that coupling and remains required for the coupled profile.
Rendering a surface from particles is chapter 34's observation integration,
not a prerequisite for computing the fluid state. From Coax: R15, R21, R23.
