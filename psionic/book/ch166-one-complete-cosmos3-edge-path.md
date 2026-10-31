# 166. One complete Cosmos3-Edge path

> **Status: unimplemented — design draft.** Nothing here runs yet.

Cosmos3-Edge is an NVIDIA world model with autoregressive and diffusion
towers and its own codecs. This chapter qualifies one bounded conditioned
generation path through it. It states which path and which modalities work,
and keeps numerical and physical evaluation separate.

## What this chapter covers

- The pinned Cosmos3-Edge revision and a complete manifest of the tensors the
  chosen path uses.
- Exact codecs for the chosen modalities, using chapter 165's codec
  contract.
- The autoregressive tower and the diffusion tower on the chosen path, with
  the scheduler from chapter 146.
- One bounded conditioned generation: fixed input frames, a fixed condition,
  a fixed seed and a fixed output length.
- Numerical evaluation: reference tensors at each stage.
- Task evaluation: a stated measure of whether the generated frames are
  physically plausible for a fixed scenario, reported apart from numerical
  parity.
- Terms: the model uses the OpenMDW-1.1 release, and the card includes
  NVIDIA-owned data, so complete reproduction of the original corpus is not
  established.

## Dependencies

- [Chapter 165](ch165-sequences-of-observations.md): observation sequences
  and contracts.
- [Chapter 146](ch146-noise-diffusion-and-flow.md): diffusion and flow.
- [Chapter 143](ch143-elephant-and-mammoth-see.md) and
  [chapter 144](ch144-see-with-c-radio.md): mature vision paths.
- Aethyr worlds for test scenarios
  ([Aethyr chapter 31](../../aethyr/book/ch031-many-worlds-in-one-step.md)).

## Done means

- Each stage on the chosen path matches recorded reference tensors within a
  tolerance frozen before comparison.
- The task evaluation is reported separately from numerical parity.
- The chapter states that visually plausible output does not prove
  physically accurate simulation.

## Sources

- [Chapter plan](PLAN.md), Part XVIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Cosmos entries.
