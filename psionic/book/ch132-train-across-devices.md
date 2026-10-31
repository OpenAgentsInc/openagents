# 132. Train across devices

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter runs training on one real homogeneous multi-device topology.
You choose one target, implement the gradient collectives it needs, and
measure how training scales. Broader distributed modes come in chapters
133–135.

## What this chapter covers

- One named topology, for example several identical GPUs in one machine.
- Gradient collectives, shared with the serving collectives of chapter 176
  and chapter 183.
- Sharded state and resume: each device's portion is saved and restored.
- Numerical comparisons against single-device training.
- Measured scaling from one device to the full topology.
- Collective recovery when a device fails.

## Dependencies

- A correct resumable single-device trainer:
  [chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md) and
  [chapter 129](ch129-train-a-whole-model-on-one-gpu.md).
- The selected device backend, such as
  [chapter 44](ch044-elephant-on-cuda.md)'s CUDA backend.
- Multi-device hardware is an owner budget decision.

## Done means

- Training on the topology matches single-device training on gradients and
  updates within a stated tolerance.
- An interrupted multi-device run resumes and continues correctly.
- Throughput is measured at each device count, with the topology, host and
  build named.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
