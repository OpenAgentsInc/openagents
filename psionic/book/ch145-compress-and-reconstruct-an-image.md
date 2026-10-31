# 145. Compress and reconstruct an image

> **Status: unimplemented — design draft.** Nothing here runs yet.

A learned image codec maps an image to a small latent tensor and back. World
models and diffusion models work in such latents. This chapter executes an
older, smaller codec, Cosmos-0.1-Tokenizer-CI8x8, as a teaching target. You
inspect its latents and reconstructions value by value.

## What this chapter covers

- The pinned Cosmos-0.1-Tokenizer-CI8x8 checkpoint: an image autoencoder that
  reduces each spatial dimension by 8.
- Its operations: convolution, down-sampling and up-sampling, a wavelet
  transform, normalization and attention blocks.
- The encoder path to the latent and the decoder path back to pixels, with
  intermediate tensors compared with recorded references.
- Reconstruction quality measured on fixed images (for example PSNR), kept
  separate from numerical parity with the reference.

## Dependencies

- [Chapter 142](ch142-pixels-convolutions-and-patches.md): convolution,
  sampling and preprocessing.
- [Chapter 10](ch010-attention-without-training.md): attention blocks.
- [Chapter 5](ch005-a-standard-library-of-tensor-operators.md): the shared
  operators.

## Done means

- Encoder intermediates, the latent and the reconstruction match recorded
  references within a tolerance frozen before comparison.
- Reconstruction quality is reported on a fixed image set.
- The chapter states its scope: this is an older teaching target, not a claim
  of Cosmos3 codec compatibility.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Cosmos notes.
