# 117. Models are compositions of modules

> **Status: unimplemented — design draft.** Nothing here runs yet.

Up to here each model is written by hand from tensors. This chapter adds the
public API that lets you build a new network from parts: modules,
parameters and buffers. Framework track.

## What this chapter covers

- `Module` and `Parameter` types.
- Named submodules and parameter trees, with stable names for checkpoints.
- Persistent and non-persistent buffers.
- Train and eval modes.
- Initialization rules per parameter.
- Freezing parameters and sharing one parameter between modules.
- The documented public API, with examples for each feature.

## What it builds

- A planned module layer over chapter 38's tensor IR and chapter 106's
  autodiff.

## Dependencies

- [Chapter 3](ch003-a-tensor-is-a-view-of-memory.md) through
  [chapter 6](ch006-one-tensor-several-devices-and-dtypes.md): tensors,
  operators and devices.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): reverse mode.
- [Chapter 40](ch040-add-your-own-operator.md): custom operators inside
  modules.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): parameter
  names in checkpoints.

## Done means

- A person who has never worked on Psionic builds an MLP, a CNN, a small
  residual network and a transformer from the documented public API, without
  touching runtime code.
- Freezing a parameter leaves it unchanged after training steps.
- A shared parameter receives the sum of its gradients from every use.
- Train and eval modes change dropout and similar operations as documented.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
