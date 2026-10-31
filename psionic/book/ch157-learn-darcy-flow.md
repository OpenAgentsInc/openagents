# 157. Learn Darcy flow

> **Status: unimplemented — design draft.** Nothing here runs yet.

Darcy flow describes fluid pressure through a porous material. The PhysicsNeMo
Darcy FNO example trains an FNO to predict the pressure field from the
permeability field. This chapter reimplements that architecture and training
recipe in Psionic, on data that you generate yourself, and compares
predictions with a numerical solver.

## What this chapter covers

- A Darcy solver that generates permeability and pressure pairs from a
  seeded random field generator, with bounded grid sizes.
- The FNO architecture from the inspected PhysicsNeMo recipe: lifting layer,
  spectral layers from chapter 156 and projection layer.
- The training recipe: loss, optimizer, schedule and batch size.
- Held-out evaluation on fields the model never saw, compared with the
  solver.
- The recipe is not a pretrained checkpoint. The inspected recipe generates
  its own data.

## Dependencies

- [Chapter 156](ch156-a-spectral-neural-operator.md): the spectral layer.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): the optimizer.
- [Chapter 109](ch109-learn-a-physical-system.md): a solver-backed learning
  pattern with held-out parameters.

## Done means

- The solver passes its own checks (for example convergence under grid
  refinement) before it produces training data.
- Training and held-out fields come from disjoint seeds.
- Relative L2 error against the solver is reported on held-out fields.
- The chapter does not claim the model replaces the solver.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), "Physics begins with
  our own model".
