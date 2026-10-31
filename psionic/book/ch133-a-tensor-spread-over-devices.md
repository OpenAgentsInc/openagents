# 133. A tensor spread over devices

> **Status: unimplemented — design draft.** Nothing here runs yet.

Distributed training needs a way to say where each part of a tensor lives.
This chapter adds a device mesh and layouts to the tensor IR: replicated,
sharded and partially reduced. One operation then runs under several
layouts and must match a single-device result. Framework track.

## What this chapter covers

- A device mesh with named axes.
- Tensor layouts over the mesh:
  - replicated on every device;
  - sharded along a tensor dimension;
  - partially reduced, where each device holds a partial sum.
- Collectives: all-reduce, all-gather, reduce-scatter and all-to-all.
- Placement propagation through the IR: each operation's output layout
  follows from its inputs.
- Redistribution between layouts when an operation needs a different one.

## Dependencies

- [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md): the
  tensor IR that carries the layouts.
- [Chapter 6](ch006-one-tensor-several-devices-and-dtypes.md): owned device
  storage and transfers.
- [Chapter 132](ch132-train-across-devices.md): the first collectives on a
  real topology.

## Done means

- One operation, such as a matrix product, runs under several input layouts
  and matches a single-device reference within its profile.
- Placement propagation gives the documented output layout for each
  operator in a fixed set of cases.
- Every redistribution is checked by gathering the result and comparing it
  with the single-device value.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
