# 131. Train the same model eager and compiled

> **Status: unimplemented — design draft.** Nothing here runs yet.

Compilation must change speed and memory, not what the model learns. This
chapter compiles the whole training step and compares it with eager
training on the same model, data order and random-number policy. Framework
track.

## What this chapter covers

- Whole-step compilation: forward, backward and the optimizer step in one
  program.
- Bounded dynamic batch sizes through chapter 39's guarded specialization.
- Fusion of forward and backward operations.
- Activation liveness: saved values freed as soon as backward reads them.
- One comparison setup: the same model, the same data order and the same RNG
  policy, run eager and compiled.

## Dependencies

- [Chapter 39](ch039-compile-a-general-tensor-program.md): symbolic shapes
  and plan caching.
- [Chapter 114](ch114-compile-the-backward-pass.md): the compiled backward
  pass.
- [Chapter 129](ch129-train-a-whole-model-on-one-gpu.md) and
  [chapter 130](ch130-profile-a-training-step.md): GPU training and its
  profile.

## Done means

- Eager and compiled runs agree on gradients, updates and final metrics
  within a tolerance stated before the comparison.
- The report gives memory, compile cost and steady-state step time for both.
- A batch size outside the admitted bound is refused, not silently
  recompiled without limit.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
