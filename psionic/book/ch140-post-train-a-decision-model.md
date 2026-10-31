# 140. Post-train a decision model

> **Status: unimplemented — design draft.** Nothing here runs yet.

Elephant and Mammoth answer typed decision questions with probabilities.
This chapter fine-tunes them with LoRA adapters. The goal is better decisions
with probabilities that stay honest. The pre-reset fine-tuning raised
accuracy and made confidence worse, so promotion here needs both of chapter
241's decision and probability gates.

## What this chapter covers

- A qualified backward pass through the hybrid decision stack, including
  Gated DeltaNet and the block-bidirectional mask of chapter 12.
- LoRA adapters: `W + (α/r)·BA` with `W` frozen. Only `A` and `B` carry
  gradients and optimizer state. Backward still passes through the frozen
  layers.
- Selected trainable regions.
- Losses: cross-entropy, Brier score and an abstention term.
- Resume from checkpoints.
- An adapter that records the base-model identity it is compatible with, and
  loads and unloads through the serving path.
- A quantized-base variant names its full recipe before it is called QLoRA.

## Dependencies

- [Chapter 14](ch014-execute-elephant.md) and
  [chapter 15](ch015-execute-mammoth.md): the decision forward path.
- [Chapter 16](ch016-are-the-probabilities-any-good.md): decision quality
  metrics.
- [Chapter 104](ch104-gradients-as-a-program.md) and
  [chapter 106](ch106-differentiate-any-tensor-program.md): gradients.
- [Chapter 241](ch241-gates-that-can-say-unverifiable.md): decision and
  probability gates.
- This chapter does not need nanochat RL.

## Done means

- A zero adapter reproduces the base model's outputs exactly.
- Only the intended parameters change during training, checked by digest.
- Task gains and retained abilities are evaluated separately on held-out
  data.
- An adapter is promoted only when it passes both the decision gate and the
  probability gate.
- The served model refuses an adapter for a different base identity.

## Sources

- [Chapter plan](PLAN.md)
