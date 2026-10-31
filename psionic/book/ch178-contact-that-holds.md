# 178. Contact that holds

> **Status: unimplemented — design draft.** Nothing here runs yet.

Explicit coupling fails exactly where robots need it most: a stiff finger
squeezing a soft object, a grasp held for seconds, cloth caught between two
bodies. Two implicit methods fix this differently. A semi-analytic primal
(SAP) solver treats rigid–deformable contact as one convex problem solved
with the deformable body's implicit step, so contact forces never overshoot.
Incremental potential contact (IPC) puts a barrier on distance into the
energy, so nothing ever interpenetrates, at a much higher cost.

## What this chapter teaches

- Why explicit coupling oscillates for stiff interfaces.
- SAP: compliant contact as a convex optimization over velocities, solved
  with the implicit FEM step.
- IPC: log barriers on distance, continuous collision detection in the line
  search, and smoothed friction.
- Why both need f64, and why neither is differentiable in our first plan.

## The physics

SAP minimizes, over next-step velocities `v`,

```jai
ℓ(v) = ½ |v − v*|²_A + Σ_c ℓ_c(J_c v)
```

where `A` is the linearized dynamics (mass plus implicit stiffness), `v*`
the free-motion velocity and `ℓ_c` a convex contact cost from a compliant
(regularized) friction model, so the problem is strictly convex with a
unique solution ([Castro et al., *An Unconstrained Convex Formulation of
Compliant Contact*, IEEE T-RO 2022](https://doi.org/10.1109/TRO.2022.3209077)).
IPC instead minimizes the implicit incremental potential plus a barrier

```jai
B(d) = −κ (d − d̂)² log(d / d̂)  for 0 < d < d̂, else 0
```

on every primitive pair's distance `d`, with a line search that never steps
past the first time of impact, guaranteeing no intersection
([Li et al., *Incremental Potential Contact*, SIGGRAPH 2020](https://doi.org/10.1145/3386569.3392425)).

## The design in Psionic

Both are coupler profiles in chapter 165's matrix: `sap` for rigid–FEM in
f64, `ipc` for FEM–FEM and shells with rigid obstacles, also f64. Both
require the implicit FEM of chapter 175. Both are refused on Metal (no f64)
and refused with gradients. SAP comes first: it reuses chapter 170's Newton
machinery. IPC is large (distance queries for every point–triangle and
edge–edge pair, CCD, barrier Hessians) and is planned as its own module
with an independent distance-query oracle.

## What it builds

- `psionic/lib/sim/sap.coax` and later `psionic/lib/sim/ipc.coax`.
- Fixtures: a finger squeezing a pad and holding it for 10 s, a cube
  grasped and lifted, two cloths stacked on a sphere (IPC).

## Acceptance checks and oracles

- Grasp held 10 s: slip below a stated bound; explicit coupling on the same
  scene shows the oscillation SAP removes (recorded as a contrast check).
- SAP solution satisfies its optimality conditions (independent checker).
- IPC: zero intersections at every step, checked by an exact triangle
  intersection test; distances never below zero.
- Matrix: SAP/IPC with gradients, and on Metal, are refused.

## How Genesis does it

Genesis offers an SAP coupler for rigid–FEM in f64 with implicit FEM and no
gradients, and IPC coupling through libuipc
([coupling](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/coupling/index.html),
[libuipc](https://github.com/spiriMirror/libuipc)). We plan both, written
in Coax rather than wrapping an external library, with the same
restrictions stated as refusals.

## Dependencies

Chapters 169 (time of impact), 170, 175, 177. From Coax: R15 f64 strict
profiles, R23.
