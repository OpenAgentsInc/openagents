# 142. Pixels, convolutions and patches

> **Status: unimplemented — design draft.** Nothing here runs yet.

Every vision model in this book starts by turning an image file into a
tensor with exact conventions. A pixel range of `[0, 1]` versus `[0, 255]`, a
resize filter or a channel order changes every value downstream. This chapter
builds the image operations that Part XIII's models share, and checks each one
on tiny fixtures that you can verify by hand.

## What this chapter covers

- Image admission: width, height, channel count and bit depth as bounded
  inputs, with a typed refusal for anything outside the admitted profile.
- Preprocessing conventions as named data: channel order (RGB or BGR),
  value range, per-channel mean and standard deviation, and the resize rule
  (filter, alignment of corners, rounding of the output size).
- Resize and sampling: nearest, bilinear and bicubic interpolation with a
  stated edge rule, compared with an independent reference on small images.
- Convolution: 2D convolution with stride, padding, dilation and groups,
  built on chapter 5's operator registry, with forward and gradient checks.
- Patch projection: an image cut into fixed-size patches and projected by a
  linear layer, which is the first layer of a vision transformer.
- Position handling for patch grids, including interpolation of learned
  position tables when the input size differs from the training size.

## What it builds

- Planned: an image-preprocessing profile record and its CPU reference
  procedures in `psionic/lib/`, plus convolution and patch operators in the
  operator manifest.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): matrix products and stable
  reductions.
- [Chapter 5](ch005-a-standard-library-of-tensor-operators.md): the operator
  registry, convolution, pooling and interpolation.
- [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md): the
  tensor IR that the patch projection lowers into.
- [Chapters 103](ch103-learning-with-arrays.md) and
  [104](ch104-gradients-as-a-program.md): gradients, for the trained heads in
  later chapters.

## Done means

- Each preprocessing step matches an independent reference on tiny fixtures
  with an exact or stated per-value tolerance.
- Convolution forward and its gradient match a direct-sum reference and
  finite differences on every admitted stride, padding and group setting.
- A patch projection of a small image equals the equivalent strided
  convolution.
- Each malformed input (wrong channel count, zero size, size above the bound)
  produces a typed refusal and no output.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md).
