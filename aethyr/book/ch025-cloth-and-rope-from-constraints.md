# 25. Cloth and rope from constraints

> **Status: partly implemented.** [`aethyr/physics/sim/pbd.coax`](../physics/sim/pbd.coax)
> (441 mutants: 412 killed, 29 marked equivalent with their reason)
> runs XPBD distance constraints over chapter 23's particle store on the CPU
> in f64, with a deterministic graph coloring, Gauss–Seidel and Jacobi
> projection profiles, substeps, damping, and a fixed sphere with thickness
> and Coulomb friction. Its checks measure a compliant link's period, a
> rope's catenary and a cloth draped on a sphere. Bending and volume
> constraints, self-collision, collision through chapter 21's distance
> fields, two-way coupling and device kernels are not built yet.

Cloth stretches very little, bends easily and folds onto itself. Simulating
it with stiff springs needs tiny time steps. Position-based dynamics (PBD)
takes a different route: predict where particles want to go, then move them
directly until each constraint (this edge keeps its length, these two
triangles keep their angle) is satisfied. Extended PBD (XPBD) gives those
constraints a physical compliance, so stiffness no longer depends on time
step and iteration count.

## What this chapter teaches

- Constraints as functions `C(x) = 0` or `C(x) ≥ 0` on particle positions.
- The PBD loop: predict, project constraints, update velocities.
- XPBD's compliance and Lagrange multipliers.
- Stretch, bending, volume and collision constraints, for cloth, rope and
  soft blobs.
- Gauss–Seidel versus Jacobi projection, and determinism on GPUs.

## The method

Each step predicts `x̃ = x + Δt v + Δt² M⁻¹ f_ext`, then for each constraint
`C_j` with compliance `α_j` (inverse stiffness) and multiplier `λ_j`:

```jai
α̃       = α_j / Δt²
Δλ_j    = (−C_j(x) − α̃ λ_j) / (∇C_j M⁻¹ ∇C_jᵀ + α̃)
Δx      = M⁻¹ ∇C_jᵀ Δλ_j;   λ_j += Δλ_j
v       = (x_new − x_old) / Δt           // after all iterations
```

With `α = 0` this is classic PBD ([Müller et al., 2007](https://doi.org/10.1016/j.jvcir.2007.01.005));
with `α > 0` it is XPBD, which approximates implicit integration of the
corresponding elastic energy ([Macklin, Müller and Chentanez, MIG 2016](https://doi.org/10.1145/2994258.2994272)).
Substepping with one iteration each converges better than many iterations
in one step ([Macklin et al., SCA 2019](https://doi.org/10.1145/3309486.3340247)).

## What runs today

[`aethyr/physics/sim/pbd.coax`](../physics/sim/pbd.coax) keeps distance
constraints in fixed arrays (`Pbd`): two particle indices, a rest length, a
compliance `α` in metres per newton and a multiplier `λ`. A particle of mass
zero in chapter 23's store is pinned. `pbd_rope` and `pbd_cloth` build a
rope pinned at both ends and a square cloth with structural and shear links.

`pbd_color` colors the constraints greedily in index order: each link takes
the lowest color that neither of its particles already has. It then
counting-sorts the links by color, so the projection order, and therefore
every bit of the result, is fixed by the constraint list alone. A link that
would need more than 62 colors is a refusal, and adding a link drops the
coloring, so a Gauss–Seidel step on an uncolored set refuses to run.

`pbd_step` runs one frame in substeps, with one projection per substep
([Macklin et al., SCA 2019](https://doi.org/10.1145/3309486.3340247)): it
predicts, resets `λ`, projects every link once by the profile, pushes
particles out of the sphere, and sets each velocity from the substep's
motion. Two profiles project:

- `pbd-gauss-seidel-f64-v1` goes color by color, so each link sees the moves
  of the colors before it. Links of one color share no particle, so a color
  could run in any order, or all at once on a device.
- `pbd-jacobi-f64-v1` computes every link from the same positions and moves
  each particle by the average of its links' moves.

The sphere keeps particles `thickness` metres outside its surface. Without
friction, cloth draped on a sphere slides off, so the push-out also applies
Coulomb friction to positions: the substep's sliding is undone up to the
friction coefficient times the depth pushed out
([Macklin et al., 2014](https://doi.org/10.1145/2601097.2601152)).

The example [`rope-and-cloth.coax`](../../aethyr/examples/rope-and-cloth.coax)
runs the three fixtures:

<!-- source: aethyr/examples/rope-and-cloth.coax example -->
```jai
// The period of a 1 kg mass on a link of compliance 0.01 m/N from an anchor,
// from two upward crossings of the rest length.
period :: (substeps: int, profile: Pbd_Profile) -> f64 {
  p: [1] Pstore; c: [1] Pbd;
  ok := pstore_add(p[..], D3_ZERO, D3_ZERO, 0.0, 0, 0) == 0 && pstore_add(p[..], d3(0.0, -1.1, 0.0), D3_ZERO, 1.0, 0, 0) == 1;
  ok = ok && pbd_link(c[..], 0, 1, 1.0, 0.01) == 0 && pbd_color(c[..], 2) == 1;
  w := Pbd_World.{dt = 1.0 / 600.0, substeps = substeps, profile = profile};
  last := 0.1; t := 0.0; ups: [2] f64; n := 0;
  while ok && n < 2 && t < 10.0 {
    ok = pbd_step(p[..], c[..], w); t += w.dt;
    now := d3_len(p[0].pos[1]) - 1.0;
    if last < 0.0 && now >= 0.0 { ups[n] = t - w.dt * now / (now - last); n += 1; }
    last = now;
  }
  return ups[1] - ups[0];
}

main :: () {
  print("link of compliance 0.01 m/N, 1 kg: spring period 2 pi sqrt(m alpha) = % s\n", r6(2.0 * PI_D * 0.1));
  for s: [3] int.[1, 4, 16] {
    print("  % substeps: gauss-seidel % s, jacobi % s\n", s, r6(period(s, Pbd_Profile.gauss_seidel)), r6(period(s, Pbd_Profile.jacobi)));
  }

  // 21 particles of 0.1 kg on a 3 m rope pinned at x = -1 and x = 1.
  print("rope, 3 m between ends 2 m apart, after 10 s:\n");
  a := pbd_catenary_a(1.0, 3.0);
  for f: 0..<2 {
    p: [1] Pstore; c: [1] Pbd;
    pbd_rope(p[..], c[..], 21, 1.0, 3.0, 0.1, 0.0);
    colors := pbd_color(c[..], 21);
    w := Pbd_World.{substeps = 20, gravity = d3(0.0, -9.81, 0.0), damping = 4.0, profile = cast(Pbd_Profile) f};
    for k: 0..<600 { if !pbd_step(p[..], c[..], w) { print("  not colored\n"); return; } }
    worst := 0.0; stretch := 0.0;
    for i: 1..<20 { worst = fmax(worst, abs(p[0].pos[i].y - pbd_catenary_y(a, 1.0, p[0].pos[i].x))); }
    for i: 0..<20 { stretch = fmax(stretch, abs(d3_dist(p[0].pos[i], p[0].pos[i + 1]) / 0.15 - 1.0)); }
    print("  %, % colors: middle at y = % m (catenary %), worst gap % m, worst stretch %\n",
          pbd_profile_name(cast(Pbd_Profile) f), colors, r6(p[0].pos[10].y), r6(pbd_catenary_y(a, 1.0, 0.0)), r6(worst), r6(stretch));
  }

  // A 12 x 12 cloth, 1.1 m square, dropped from 0.8 m onto a 0.3 m sphere.
  p: [1] Pstore; c: [1] Pbd;
  pbd_cloth(p[..], c[..], 12, 0.1, 0.8, 0.01, 0.0001);
  print("cloth: % particles, % links, % colors\n", p[0].count, c[0].count, pbd_color(c[..], p[0].count));
  w := Pbd_World.{substeps = 4, gravity = d3(0.0, -9.81, 0.0), damping = 1.0, sphere = true, center = d3(0.0, 0.2, 0.0), radius = 0.3, thickness = 0.01, friction = 0.5};
  for k: 0..<180 {
    _ := pbd_step(p[..], c[..], w);
    if k % 30 == 29 {
      closest := 10.0; low := 10.0;
      for i: 0..<p[0].count { closest = fmin(closest, d3_dist(p[0].pos[i], w.center)); low = fmin(low, p[0].pos[i].y); }
      print("  % s: closest % m from the center (radius plus thickness 0.31), lowest y % m, middle y % m\n",
            r6(cast(f64) (k + 1) / 60.0), r6(closest), r6(low), r6(p[0].pos[5 * 12 + 5].y));
    }
  }
}
```

Run it with:

```shell
coax/bin/coax run aethyr/examples/rope-and-cloth.coax
```

```text
link of compliance 0.01 m/N, 1 kg: spring period 2 pi sqrt(m alpha) = 0.628319 s
  1 substeps: gauss-seidel 0.628377 s, jacobi 0.628377 s
  4 substeps: gauss-seidel 0.628322 s, jacobi 0.628322 s
  16 substeps: gauss-seidel 0.628319 s, jacobi 0.628319 s
rope, 3 m between ends 2 m apart, after 10 s:
  pbd-gauss-seidel-f64-v1, 2 colors: middle at y = -1.007435 m (catenary -1.005267), worst gap 0.002169 m, worst stretch 0.000857
  pbd-jacobi-f64-v1, 2 colors: middle at y = -1.009066 m (catenary -1.005267), worst gap 0.003799 m, worst stretch 0.001714
cloth: 144 particles, 506 links, 8 colors
  0.5 s: closest 0.31 m from the center (radius plus thickness 0.31), lowest y -0.105953 m, middle y 0.500777 m
  1.0 s: closest 0.31 m from the center (radius plus thickness 0.31), lowest y -0.091255 m, middle y 0.501814 m
  1.5 s: closest 0.31 m from the center (radius plus thickness 0.31), lowest y -0.108547 m, middle y 0.501814 m
  2.0 s: closest 0.31 m from the center (radius plus thickness 0.31), lowest y -0.109829 m, middle y 0.501814 m
  2.5 s: closest 0.31 m from the center (radius plus thickness 0.31), lowest y -0.103209 m, middle y 0.501814 m
  3.0 s: closest 0.31 m from the center (radius plus thickness 0.31), lowest y -0.108533 m, middle y 0.501814 m
```

The compliant link rings at `2π√(mα)`, the period of a spring of stiffness
`1/α`, to within 0.01% with one substep and to within 10⁻⁶ s with 16:
compliance, not the substep count, sets the stiffness. The rope's middle
hangs 2.2 mm below the catenary through its ends with Gauss–Seidel and
3.8 mm below with Jacobi, which converges more slowly; 21 particles are a
coarse sampling of the curve. The cloth comes to rest on the sphere with its
corners hanging, and no particle is ever closer to the center than the
radius plus the thickness.

The checks run with:

```shell
coax/bin/coax test aethyr/physics/sim/pbd.coax --library
```

They hold the link's period within 0.2% of the spring's for 1, 4 and 16
substeps under both profiles, and a four-times heavier mass to twice the
period. They settle the rope onto the catenary within 2 cm under both
profiles, with the catenary's own length checked. They check that the
coloring is valid, in order and the same on every run, the refusals, that
no cloth particle ends a step inside the sphere by more than the thickness,
that a pinned particle never moves, and that an unconstrained particle
falls exactly as semi-implicit Euler predicts.

## The design in the Metaverse Engine

Constraints are stored by kind in fixed arrays built from meshes (edges,
dihedral pairs, tetrahedra). Gauss–Seidel on a GPU requires graph coloring
so no two constraints in one color share a particle; the coloring is
computed at build and is deterministic, which makes the projection order —
and so the bits — fixed. Jacobi with averaging is the alternative profile.
Self-collision uses chapter 23's neighbors with a thickness; collision with
rigid bodies uses chapter 21's SDF queries and returns impulses through
chapter 29.

## What it still builds

- Bending (dihedral) and volume constraints, and the pressured balloon
  fixture.
- Self-collision through chapter 23's neighbors with a thickness.
- Collision with rigid bodies through chapter 21's signed distance queries,
  and impulses back to them through chapter 29.
- Generated CUDA and Metal kernels, one launch per color, checked against
  the CPU profiles, and results compared across worker counts.

## Acceptance checks and oracles

- A single XPBD distance constraint with compliance `α` oscillates with the
  frequency of a spring of stiffness `1/α`, independent of substep count
  within a stated tolerance.
- Hanging rope settles to the catenary `y = a cosh(x/a)` within a stated
  error at high stiffness.
- Coloring is valid (no shared particle in a color) and identical across
  runs; results bitwise identical across worker counts.
- No cloth particle ends a step inside a rigid body by more than the
  thickness.

## How Genesis does it

Genesis provides a PBD solver for cloth, ropes and particle soft bodies with
XPBD-style constraints
([PBD solver](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/pbd_solver/index.html));
its PBD backward pass is a stub. We plan the same method, with a
deterministic coloring as a stated contract and gradient support marked
unsupported until chapter 32's tests cover it.

## Dependencies

Chapter 23 particles and chapter 21 SDF queries. The first rope or cloth
fixture directly supplies particles, constraints and prescribed collision
geometry; the rigid body in the penetration check need not respond to it.

Chapter 29 later adds two-way coupling, and chapter 35 supplies imported
meshes. Those paths need their own coupling and import checks before they
are admitted; they do not precede the first constraint solve. From Coax:
R15, R21, R23.
