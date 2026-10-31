# 152. A speech encoder

> **Status: unimplemented — design draft.** Nothing here runs yet.

Parakeet uses a FastConformer encoder: convolutional subsampling followed by
blocks that combine self-attention and convolution. This chapter builds that
encoder and compares it with fixed intermediate tensors from the reference.

## What this chapter covers

- Convolutional subsampling of the feature sequence, and how it changes
  sequence lengths.
- Length and mask propagation through every layer, so padded frames never
  affect valid ones.
- Conformer blocks: feed-forward modules, multi-head self-attention with its
  position encoding, and the depthwise convolution module.
- Batch inputs of different lengths, with each item's result equal to running
  it alone.

## Dependencies

- [Chapter 151](ch151-waveforms-and-spectral-features.md): spectral features.
- [Chapter 142](ch142-pixels-convolutions-and-patches.md): convolution
  operators.
- [Chapter 10](ch010-attention-without-training.md): the attention core.

## Done means

- Each block's output matches recorded intermediate tensors within a
  tolerance frozen before comparison.
- Output lengths match the reference for every admitted input length.
- A padded batch gives the same valid outputs as each item run alone.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Parakeet entry.
