# 116. Train within a memory budget

> **Status: unimplemented — design draft.** Nothing here runs yet.

Training uses far more memory than inference. This chapter accounts for
every byte and then trains the tiny decoder of chapter 115 under a fixed
memory cap in three ways. Each method trades memory for time, and you
measure both.

## What this chapter covers

- Where training memory goes, each with its own lifetime:
  - parameters;
  - gradients;
  - optimizer state;
  - saved activations;
  - workspace.
- Gradient accumulation with correct loss normalization and update timing.
- Activation checkpointing that recomputes chosen blocks during backward.
  Random operations in a recomputed block replay the same random numbers.
- A precision policy per operation: storage, multiplication, accumulation,
  gradients and optimizer state, each named separately.
- Loss scaling for low-precision gradients, with overflow and underflow
  fixtures.

## Dependencies

- [Chapter 115](ch115-train-the-whole-tiny-decoder.md): the model and its
  reference run.
- [Chapter 114](ch114-compile-the-backward-pass.md): saved-activation
  planning and recomputation.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): explicit precision
  and the optimizer state.
- [Chapter 6](ch006-one-tensor-several-devices-and-dtypes.md): f16 and bf16
  casts and seeded random tensors.

## Done means

- The tiny decoder trains under the fixed memory cap with accumulation, with
  checkpointing and with a reduced-precision policy.
- Gradients and updates for each method match the reference run within the
  tolerance its profile states.
- Recomputed operations reproduce their random numbers exactly.
- Each method reports measured memory saved and extra time.
- Overflow and underflow fixtures trigger loss-scale changes as specified.

## Sources

- [Chapter plan](PLAN.md)
