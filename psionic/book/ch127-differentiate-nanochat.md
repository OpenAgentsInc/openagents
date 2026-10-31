# 127. Differentiate nanochat

> **Status: unimplemented — design draft.** Nothing here runs yet.

Forward parity does not show that a model can be trained. nanochat's
`gpt.py` has features that a textbook decoder lacks, and each one needs a
correct gradient. This chapter writes and checks the backward pass for every
enabled feature.

## What this chapter covers

- Gradients for each enabled feature of the pinned `gpt.py`:
  - rotary positions and QK normalization with its scale;
  - grouped-query attention with sliding windows;
  - the ReLU-squared MLP;
  - the untied language-model head;
  - value embeddings and their gates;
  - residual and `x0` scalars;
  - smear and backout.
- Complete parameter coverage: every parameter receives a gradient, and a
  check fails if one does not.
- Tiny independent comparisons per feature, against finite differences and
  against an independent reference.

## Dependencies

- [Chapter 113](ch113-differentiate-a-decoder.md): backward operations for a
  causal decoder.
- [Chapter 125](ch125-execute-the-pinned-nanochat-gpt.md): the pinned
  forward pass.
- [Chapter 126](ch126-execute-nanochat-exactly.md): the complete inference
  profile.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): general reverse
  mode, where the IR covers the feature.

## Done means

- Each feature's gradient matches a central finite difference on a tiny
  configuration, within a stated tolerance.
- Removing any one feature changes a fixture, so each check tests something.
- Every parameter in the pinned layout receives a gradient.
- A few training steps on a tiny configuration match an independent
  reference's losses and updates.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
