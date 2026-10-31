# 69. Original MXFP4 products use matrix hardware

> **Status: unimplemented — design draft.** Nothing here runs yet.

The original gpt-oss experts are stored as MXFP4. Chapter 66 multiplies them
with `f32` arithmetic. This chapter runs those products on the GPU's matrix
units under a separately named numerical profile, and it measures whether the
change is worth its error.

## What this chapter covers

- Staging bounded tiles of original coefficients exactly, decoded from the
  packed MXFP4 blocks.
- Splitting `f32` activations into checked BF16 limbs where the matrix units
  need BF16 inputs.
- Accumulating tile products under a new named profile.
- Freezing the profile's error bounds before comparing with independent tiny
  vectors and retained original vectors.
- Keeping the exact route and top-token gates from chapter 66.
- Measuring the selected-expert bottleneck before and after.
- The `f32` reference profile stays in place. The new profile never replaces
  it silently.

## Dependencies

- [Chapter 60](ch060-original-mxfp4-weights.md): the MXFP4 decoder.
- [Chapter 66](ch066-a-complete-cuda-decoder.md): the CUDA decoder and its
  frozen comparison rule.
- [Chapter 68](ch068-coax-kernels-cooperate.md): cooperative tiles and matrix
  primitives.

## Done means

- Error bounds are written down before any comparison runs.
- Tiny and retained original vectors meet those bounds.
- Expert routes and top tokens still match chapter 66's gates on the fixture
  prompts.
- The chapter reports selected-expert time before and after on the RTX 4080.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 5.
- [Making Psionic serving faster](../docs/serving/performance.md):
  matrix-hardware products from original MXFP4 (P2).
