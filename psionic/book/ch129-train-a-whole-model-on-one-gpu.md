# 129. Train a whole model on one GPU

> **Status: unimplemented — design draft.** Nothing here runs yet.

Inference needs only forward kernels. Training also needs their gradients,
an optimizer on the device and a precision policy. This chapter moves the
whole training step onto one GPU, the RTX 4080, with no PyTorch. Framework
track.

## What this chapter covers

- GPU forward and backward passes.
- Gradient accumulation and optimizer updates on the device.
- BF16 and FP32 precision policies.
- The backward kernels that inference never needed:
  - matrix-product gradients;
  - normalization gradients;
  - attention gradients;
  - convolution gradients.
- All trainable state stays on the device between steps.
- No unintended CPU fallback. A missing device kernel is a refusal.

## Dependencies

- [Chapter 37](ch037-a-coax-compute-subset.md) and
  [chapter 44](ch044-elephant-on-cuda.md): generated device kernels and the
  CUDA backend.
- [Chapter 106](ch106-differentiate-any-tensor-program.md) and
  [chapter 114](ch114-compile-the-backward-pass.md): backward graphs.
- [Chapter 116](ch116-train-within-a-memory-budget.md): precision policy and
  memory accounting.
- [Chapter 117](ch117-models-are-compositions-of-modules.md) and
  [chapter 118](ch118-one-trainer-for-every-model.md): modules and the
  trainer.

## Done means

- An MLP, a CNN and a tiny transformer train on the RTX 4080 with all
  trainable state on the device between steps.
- A 10–50M-parameter decoder runs a complete training job with no PyTorch.
- Device gradients are checked against independent CPU gradients on small
  shapes.
- The run reports tokens per second, peak memory and a loss curve, and
  compares with an independently implemented PyTorch baseline.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
