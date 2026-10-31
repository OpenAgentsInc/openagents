# 250. Can someone use Psionic instead of PyTorch?

> **Status: unimplemented — design draft.** Nothing here runs yet.

The framework track has one graduation sentence: a developer defines, trains,
fine-tunes, evaluates, saves, exports and deploys a new neural network
entirely in Coax and Psionic, on CPU and GPU, without installing or importing
PyTorch. This chapter tests that sentence with people and models that did not
shape the framework.

## What this chapter covers

- An externally consumable package: public APIs, operator documentation,
  examples, diagnostics and reproducible benchmarks.
- A model audit command. It checks a proposed model's operators, dtypes,
  gradients and backends before it allocates large GPU memory.
- Three published PyTorch tutorials rebuilt with public APIs only:
  1. a classifier;
  2. a convolutional model;
  3. a transformer training example.
- For each one, the record lists model-specific lines, unsupported
  operations, runtime modifications, effort, cold-start compile time,
  throughput, memory and final quality.
- The four acceptance projects of the framework track: train from scratch,
  a language model without PyTorch, import-fine-tune-serve, and train across
  GPUs and recover.

## Dependencies

- [Chapter 3](ch003-a-tensor-is-a-view-of-memory.md), [Chapter 5](ch005-a-standard-library-of-tensor-operators.md) and [Chapter 6](ch006-one-tensor-several-devices-and-dtypes.md): tensors, operators and devices.
- [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md), [Chapter 39](ch039-compile-a-general-tensor-program.md) and [Chapter 40](ch040-add-your-own-operator.md): the tensor IR, compilation and custom
  operators.
- [Chapter 106](ch106-differentiate-any-tensor-program.md), [Chapter 114](ch114-compile-the-backward-pass.md), [Chapter 117](ch117-models-are-compositions-of-modules.md), [Chapter 118](ch118-one-trainer-for-every-model.md), [Chapter 121](ch121-feed-the-accelerator.md) and [Chapter 124](ch124-adamw-and-explicit-precision.md):
  autodiff, compiled backward, modules, the trainer, data loading and
  optimizers.
- [Chapter 129](ch129-train-a-whole-model-on-one-gpu.md), [Chapter 130](ch130-profile-a-training-step.md) and [Chapter 131](ch131-train-the-same-model-eager-and-compiled.md): GPU training and profiling.
- [Chapter 8](ch008-models-travel-between-frameworks.md), [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md) and [Chapter 221](ch221-write-a-bounded-onnx-graph.md): checkpoints and interchange.
- [Chapter 133](ch133-a-tensor-spread-over-devices.md), [Chapter 134](ch134-data-parallel-training.md) and [Chapter 135](ch135-fully-sharded-training.md): distributed training.

## Done means

- An ordinary new model needs zero engine modifications.
- Each modification that was needed names a missing public abstraction, and
  that abstraction is added.
- The audit command refuses a model with an unsupported operator before any
  large allocation.
- Each tutorial's quality and resources are reported, and divergence from the
  PyTorch reference is measured, not assumed harmless.
- The chapter does not claim to replace PyTorch as everyone's ecosystem.

## Sources

- [Chapter plan](PLAN.md), Part XXIV and "A framework, not only engines".
