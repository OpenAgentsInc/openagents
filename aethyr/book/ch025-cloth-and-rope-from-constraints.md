# 25. Cloth and rope from constraints

> **Status: unimplemented — design draft.** Nothing here runs yet.

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

## The design in Aethyr

Constraints are stored by kind in fixed arrays built from meshes (edges,
dihedral pairs, tetrahedra). Gauss–Seidel on a GPU requires graph coloring
so no two constraints in one color share a particle; the coloring is
computed at build and is deterministic, which makes the projection order —
and so the bits — fixed. Jacobi with averaging is the alternative profile.
Self-collision uses chapter 23's neighbors with a thickness; collision with
rigid bodies uses chapter 21's SDF queries and returns impulses through
chapter 29.

## What it builds

- `aethyr/sim/pbd.coax`: prediction, XPBD projection by kind, graph
  coloring, velocity update, damping, self- and rigid collision.
- Fixtures: a hanging rope, a draped cloth on a sphere, a pressured balloon.

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
