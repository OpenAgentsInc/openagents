# 177. Materials meet

> **Status: unimplemented — design draft.** Nothing here runs yet.

A gripper (rigid) squeezes a sponge (FEM), pours water (SPH) onto sand
(MPM). Each solver is correct on its own; the scene is only correct if the
forces between them balance. The simplest coupling is explicit: each
solver advances, then contacts between materials exchange impulses, in a
fixed order, once per substep. It is cheap and general, but it can be
unstable for stiff contacts, which chapter 178 addresses.

## What this chapter teaches

- Coupling as momentum exchange at an interface, and why Newton's third
  law must hold exactly in the sums.
- Rigid–particle, rigid–grid (MPM) and particle–mesh couplings.
- Explicit coupling's stability limit, and substepping.
- One-way versus two-way coupling.

## The method

For a contact between material `A` and `B` with relative normal velocity
`v_n < 0` at a point, the explicit coupler applies an impulse `j` to both,
with opposite signs:

```jai
j     = −(1 + e) v_n / (1/m_A,eff + 1/m_B,eff) n,   friction clamped to μ|j_n|
v_A  += j / m_A,eff;   v_B −= j / m_B,eff         // m_eff includes rotation for rigid bodies
```

For MPM, the rigid body's velocity is imposed on grid nodes it covers
(CPIC) and the change in grid momentum is returned to it as force and
torque. For SPH, boundary particles carry the body's velocity (chapter 172)
and their pressure forces sum to the body's force. The scheme is stable when
`Δt` resolves the stiffest interface; the step report says when it does not.

## The design in Psionic

The coupler is a stage in chapter 165's schedule with `prepare` (collect
interface points from each solver) and `exchange` (apply equal and opposite
impulses). Each pair of solvers has its own routine; pairs without one are
refused at build. The order in which pairs exchange is fixed and recorded.
A momentum ledger (the same idea as Verse's) sums every impulse the coupler
applies; its total must be zero.

## What it builds

- `psionic/lib/sim/couple.coax`: interface collection and exchange for
  rigid–SPH, rigid–MPM, rigid–PBD, rigid–FEM and MPM–SPH, with the ledger.
- Fixtures: a cube dropped into water, a sphere sinking into sand, a cloth
  draped over a rigid table, a finger pressing a pad.

## Acceptance checks and oracles

- Ledger: total coupling impulse exactly zero in each substep.
- Floating cube equilibrium (Archimedes) as in chapter 172, now with the
  cube rigid and two-way coupled.
- A rigid sphere resting on sand settles to a penetration within a stated
  band; resting on a pad, to the FEM contact solution.
- Instability case: stiff pad and heavy body at large `Δt` is reported by
  the step report, and resolved by substeps.

## How Genesis does it

Genesis's default explicit coupler exchanges momentum between solver pairs
in a prepare/advance/exchange/finalize substep
([coupling](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/coupling/index.html),
[coupler source](https://github.com/Genesis-Embodied-AI/genesis-world/tree/v1.4.5/genesis/engine/couplers)).
We follow the same design and add the zero-sum ledger as a checked
invariant.

## Dependencies

Chapters 170 to 176. From Coax: R12 units for forces and impulses, R15.
