# 22. Contact as an optimization

> **Status: unimplemented — design draft.** Aethyr resolves contacts by
> sequential impulses ([`verse/contact.coax`](../../verse/contact.coax)).
> This chapter plans the constraint solver an articulated robot needs:
> contacts, joint limits and friction solved together as one convex
> problem. Nothing here runs yet.

A sequential-impulse solver visits one contact at a time and repeats until
the corrections are small. It is simple and fast, and it is what games use,
but it converges slowly for heavy stacks and for robots whose links are
coupled through the mass matrix: pushing on the gripper moves the whole arm.
Treating every constraint at once, as the solution of one optimization
problem, gives an answer that does not depend on visiting order and that
Newton's method reaches in a handful of iterations.

## What this chapter teaches

- Constraints as rows of a Jacobian: contact normals, friction directions,
  joint limits, equality constraints.
- Coulomb friction cones, and their pyramidal and elliptic approximations.
- Soft constraints: why a constraint with a little compliance makes the
  problem well posed, and how stiffness and damping are chosen per row.
- The convex formulation of MuJoCo, and solving it with Newton's method
  (Cholesky) or conjugate gradients.
- Warm starting, islands and sleeping.

## The physics

Write the unconstrained acceleration `a₀ = M⁻¹(τ − c)`. With constraint
Jacobian `J` (one row per normal, friction or limit direction), the
constrained acceleration minimizes

```jai
a* = argmin_a  ½ (a − a₀)ᵀ M (a − a₀) + s(J a − a_ref)
f  = −∂s/∂(J a)                              // constraint forces, J ᵀ f enters M a = τ − c + Jᵀ f
```

where `s` is a convex penalty that is zero inside the admissible set (no
pulling contacts, friction inside its cone) and grows quadratically outside,
with per-row weights from an impedance `d ∈ (0, 1)` and a reference
acceleration `a_ref = −b (J v) − k r` built from the constraint's velocity
and violation `r` ([Todorov, ICRA 2014](https://doi.org/10.1109/ICRA.2014.6907751);
[MuJoCo computation](https://mujoco.readthedocs.io/en/stable/computation/index.html)).
The friction cone `|f_t| ≤ μ f_n` is either kept elliptic or replaced by a
pyramid of `2k` edges. Because the objective is convex and piecewise
quadratic, Newton's method with an exact line search converges in a few
iterations; each iteration solves with the Hessian `M + Jᵀ D J` restricted to
active rows, by Cholesky. Conjugate gradients avoids the factorization at
the cost of more iterations.

## The design in Aethyr

The solver collects rows from chapter 21's manifolds and chapter 19's
joint limits into fixed-capacity arrays per environment, in the canonical
pair order of chapter 20. Two methods are profiles: `newton-cholesky`
(default; it shares the Cholesky factor with forward dynamics and later
with the adjoint solve of chapter 32) and `cg`. Iteration count and
tolerance are part of the profile, and a run that hits the iteration limit
says so in its step report rather than returning silently.

Warm starting reuses last step's forces matched by feature IDs. Islands
(connected components of the contact graph) let resting groups sleep; like
body sleep in chapter 18 this is a profile option because it changes
results and breaks gradients.

Free bodies use the same solver with a free joint, so chapter 18's bodies
and a robot share one contact path. Aethyr's sequential-impulse solver stays
as a separate reference and as the cheap profile for games.

## What it builds

- `aethyr/sim/constraint.coax`: row assembly, impedance, pyramidal and
  elliptic cones, the primal Newton solver with line search, CG, warm
  start, islands.
- Fixtures: a box at rest, a 20-box stack, a sliding block on an incline at
  `μ` just above and below `tan θ`, a gripper holding a cube.

## Acceptance checks and oracles

- Incline: the block sticks when `tan θ < μ` and slides with acceleration
  `g(sin θ − μ cos θ)` when not, against the closed form.
- Optimality: at the returned `a*`, the KKT conditions hold to the stated
  tolerance (checked by an independent routine, not the solver).
- Newton and CG agree within tolerance; result independent of row order up
  to the stated tolerance (Newton) — and bitwise reproducible for a fixed
  order.
- Stack: 20 boxes stand for 10⁴ steps with drift below a stated bound.
- Agreement with recorded MuJoCo 3.3.1 contact forces on shared fixtures.

## How Genesis does it

Genesis's rigid solver enforces contacts, limits and equalities as soft
constraints in a convex optimization, with Newton (Cholesky) as default, CG
as an alternative, and pyramidal or elliptic friction
([constraints](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/rigid_solver/constraints.html),
[rigid solver source](https://github.com/Genesis-Embodied-AI/genesis-world/tree/v1.4.5/genesis/engine/solvers/rigid)).
We follow the same formulation. We add an independent KKT checker and
require iteration-limit hits to be reported, and keep sequential impulses as
a second, simpler reference.

## Dependencies

Chapters 19 (mass matrix, Jacobians), 20 and 21 (pairs and manifolds),
Psionic chapter 4's Cholesky. These suffice to qualify the forward contact
solve. Chapter 32 later reuses and differentiates the factorization; its
gradient checks remain required for that extended profile. From Coax: R15
strict reductions, R21 batched layouts, R23 device graphs for the Newton
loop.
