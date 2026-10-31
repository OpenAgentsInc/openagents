# 19. Joints in generalized coordinates

> **Status: unimplemented — design draft.** Psionic's Panda controller
> ([Psionic chapter 170](../../psionic/book/ch170-close-the-simulated-control-loop.md)) uses
> kinematics and a mass matrix supplied by MuJoCo; this chapter plans our own
> articulated-body dynamics so the engine no longer needs an external
> simulator. Nothing here runs yet.

Joined bodies can be simulated as free bodies held together by constraints,
but a robot arm has seven joints and seven numbers describe its whole pose.
Simulating it in those *generalized coordinates* keeps the joints exact by
construction (no drift apart), makes joint limits and motors natural, and is
what controllers expect. The price is that the equations of motion couple
every link: moving the shoulder changes the effective inertia felt at the
wrist.

## What this chapter teaches

- Spatial (6D) vectors and the tree structure of an articulated body.
- Forward kinematics along the tree.
- The mass matrix by the composite rigid-body algorithm (CRBA), the bias
  forces by recursive Newton–Euler (RNEA), and forward dynamics either by
  factoring `M` or by the articulated-body algorithm (ABA) in `O(n)`.
- Joint types: revolute, prismatic, ball, free, fixed, and screw.

## The physics

With joint positions `q`, velocities `q̇`, applied joint torques `τ` and
constraint forces `f` acting through Jacobian `J`:

```jai
M(q) q̈ + c(q, q̇) = τ + Jᵀ f         // c = Coriolis, centrifugal and gravity terms
```

Each joint `i` has a motion subspace `S_i` (one column for a revolute or
prismatic joint, six for a free joint) and a parent `λ(i)`. Velocities
propagate outward, `v_i = X_{λ(i)→i} v_{λ(i)} + S_i q̇_i`, and forces inward.
RNEA evaluates the left side for given `q, q̇, q̈` in one outward and one
inward pass. CRBA builds `M` column by column from composite inertias
`I_i^c = I_i + Σ_{children} X* I_j^c X`. ABA solves for `q̈` directly with
articulated inertias and costs `O(n)` instead of `O(n³)`
([Featherstone, *Rigid Body Dynamics Algorithms*, 2008](https://doi.org/10.1007/978-1-4899-7560-7)).
A screw joint couples rotation and translation by a pitch `p`: one
coordinate `θ`, motion subspace `[ŝ; p ŝ]`.

## The design in Aethyr

A model is a flat, topologically sorted array of links with parent indices,
joint types, joint axes, link inertias and fixed transforms, imported from
URDF or MJCF by chapter 35 or written directly. Kinematic topology is fixed
at build; per-environment variation (chapter 31) changes masses, inertias,
link transforms and joint parameters, never the tree.

Both forward-dynamics routes are kept. CRBA plus a Cholesky factorization of
`M` is what the contact solver of chapter 22 needs anyway (it reuses the
factor). ABA is the cheaper route for unconstrained steps and an independent
oracle for the first. Passes over the tree run level by level, so on a GPU
one kernel launch handles every link at one depth across every environment.

Integration in generalized coordinates is semi-implicit Euler by default,
with joint damping and armature (rotor inertia added to `M`'s diagonal)
treated implicitly, which is what makes stiff position-controlled joints
stable at ordinary time steps. Ball and free joints keep a quaternion and
integrate it as chapter 18 does.

## What it builds

- `aethyr/sim/articulation.coax`: model arrays, forward kinematics,
  RNEA, CRBA, ABA, Jacobians for any link point, and the integrator.
- Fixtures: a double pendulum, a seven-joint arm with the Panda's published
  inertias, and a screw joint.

## Acceptance checks and oracles

- Double pendulum: energy conserved to the integrator's stated bound over
  10⁵ steps; chaotic trajectories compared only over a short horizon.
- `M(q)` symmetric positive definite at sampled `q`; CRBA's `M` equals the
  matrix assembled from `n` RNEA calls with unit accelerations.
- Round trip: `RNEA(q, q̇, ABA(q, q̇, τ)) = τ` to rounding, at random states.
- Jacobian columns against finite differences of forward kinematics.
- Panda gravity torques and `M` at recorded joint states against values
  recorded from MuJoCo 3.3.1 (the evaluation backend Psionic chapter 170 already
  pins), within a stated tolerance; the recordings, not MuJoCo, are the
  oracle in checks.
- Screw joint: one turn advances the pitch exactly.

## How Genesis does it

Genesis's rigid solver works in generalized joint coordinates, runs forward
kinematics through the articulation tree and builds the mass matrix from
composite rigid-body inertias
([forward dynamics](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/rigid_solver/forward_dynamics.html)).
It treats MuJoCo as a design reference and validation target, with an
explicit compatibility mode for checks
([coding guidelines](https://github.com/Genesis-Embodied-AI/genesis-world/blob/main/CODING_GUIDELINES.md));
version 1.4.5 added runtime screw constraints
([release](https://github.com/Genesis-Embodied-AI/genesis-world/releases/tag/v1.4.5)).
We follow the same formulation. Our difference is in the checks: ABA and
CRBA are kept as two independent routes that must agree, and the MuJoCo
comparison is against frozen recordings rather than a live dependency.

## Dependencies

Chapter 18's integrators, Psionic chapter 4's linear algebra (Cholesky) and Psionic chapter
107's transforms for the Jacobian checks. From Coax: units and frames (R12),
robot-model import (R17) and bounded control (R14) for the controllers in
Psionic chapter 167.
