# 118. One trainer for every model

> **Status: unimplemented — design draft.** Nothing here runs yet.

Each earlier chapter writes its own training loop. This chapter replaces
them with one interface for a model, a loss and an optimizer. An image
classifier and a tiny language model then train through the same code.
Framework track.

## What this chapter covers

- One interface that takes a model, a loss and an optimizer.
- Epochs and steps.
- Gradient clipping and gradient accumulation.
- Validation runs on a schedule.
- Learning-rate schedulers.
- Callbacks for logging and checkpoints.
- Cancellation that stops at a step boundary and leaves a valid checkpoint.

## Dependencies

- [Chapter 117](ch117-models-are-compositions-of-modules.md): modules and
  parameters.
- [Chapter 121](ch121-feed-the-accelerator.md): data loading.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): AdamW, Muon and
  their state. The plan's shared optimizer interface (SGD with momentum,
  AdamW and Muon behind one type) is a prerequisite.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md):
  checkpoints and resume.

## Done means

- An image classifier and a tiny language model train through the same
  trainer with no model-specific changes to it.
- The trainer reproduces a hand-written loop's loss and updates on a small
  model.
- A cancelled run resumes and reproduces the uninterrupted run.
- Clipping and accumulation match independent calculations on fixtures.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
