# 105. Make a model fail, then find out why

> **Status: unimplemented — design draft.** Nothing here runs yet.

A training bug often produces no error. The loss still goes down, or it goes
down a little less. This chapter takes a classifier that works, breaks it on
purpose in six ways, and builds a diagnostic that points at each fault. You
learn to read a training run before you trust it.

## What this chapter covers

- A working baseline: chapter 103's deadline classifier with one hidden
  layer, trained with chapter 104's tape.
- Six planted faults:
  1. Gradients that accumulate across steps because they are never cleared.
  2. Targets paired with the wrong examples.
  3. A loss mask that includes padding.
  4. A learning rate that is too large.
  5. Initialization that makes every hidden unit identical.
  6. A training-only random operation, such as dropout, left on at
     evaluation.
- Diagnostics:
  - activation distributions per layer;
  - gradient norms and update sizes per parameter;
  - train and validation curves;
  - overfitting one small batch before a full run.
- For each fault, the expected symptom and the diagnostic that shows it.

## What it builds

- Planned training diagnostics for the CPU trainer: per-step records of
  activation statistics, gradient norms and update-to-weight ratios.

## Dependencies

- [Chapter 103](ch103-learning-with-arrays.md): the classifier and its data.
- [Chapter 104](ch104-gradients-as-a-program.md): the tape and its
  gradients.
- [Chapter 6](ch006-one-tensor-several-devices-and-dtypes.md): seeded
  dropout.

## Done means

- Each of the six faults produces a diagnostic that points at that fault, and
  not at a different one.
- Repairing each fault restores the baseline's loss and held-out accuracy
  under the declared profile.
- Overfitting one small batch reaches near-zero loss on the repaired model
  and fails to do so for the faults that should block it.
- Every run is reproducible from its seed.

## Sources

- [Chapter plan](PLAN.md)
