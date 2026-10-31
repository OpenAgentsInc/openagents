# 135. Fully sharded training

> **Status: unimplemented — design draft.** Nothing here runs yet.

Data parallelism keeps a full copy of the training state on every device.
Fully sharded training splits parameters, gradients and optimizer state
across devices, so a model whose training state exceeds one GPU's memory
can still train. Framework track.

## What this chapter covers

- Sharded parameters, gradients and optimizer state.
- All-gather of parameters before each layer runs, prefetched ahead of use.
- Reduce-scatter of gradients after backward.
- Checkpoints saved in one shard layout and restored into a different one.
- A memory account per device for each state type.

## Dependencies

- [Chapter 133](ch133-a-tensor-spread-over-devices.md): sharded layouts and
  redistribution.
- [Chapter 134](ch134-data-parallel-training.md): the data-parallel loop.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): the
  checkpoint record, extended with shard layouts.
- [Chapter 116](ch116-train-within-a-memory-budget.md): memory accounting.

## Done means

- A model whose training state exceeds one GPU's memory trains and resumes.
- Sharded training matches data-parallel training on updates within a stated
  tolerance on a model that fits both ways.
- A checkpoint restored into a different shard layout continues the run
  correctly.
- Scaling and per-device peak memory are measured.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
