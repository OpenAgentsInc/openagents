# 134. Data-parallel training

> **Status: unimplemented — design draft.** Nothing here runs yet.

In data-parallel training each device holds a full model copy and trains on
a different batch. The devices average their gradients before each update.
This chapter builds that loop and handles a failed rank. Framework track.

## What this chapter covers

- Gradient buckets: gradients grouped so that each all-reduce is large
  enough to be efficient.
- All-reduce of gradients across devices.
- Accumulation boundaries: devices reduce only at the step where they
  update.
- Communication overlapped with backward computation.
- Handling a failed rank: detect it, stop cleanly and restart from the last
  checkpoint.

## Dependencies

- [Chapter 133](ch133-a-tensor-spread-over-devices.md): mesh, layouts and
  collectives.
- [Chapter 118](ch118-one-trainer-for-every-model.md): the shared trainer.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md):
  checkpoints for restart.

## Done means

- One device and two devices are compared on gradient semantics: the
  averaged gradient equals the single-device gradient over the combined
  batch, within a stated tolerance.
- Throughput is measured at one and two devices, with and without overlap.
- A run with a killed rank restarts from its checkpoint and continues
  correctly.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
