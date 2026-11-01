# 48. Integer matrix products

> **Status: unimplemented — design draft.** Nothing here runs yet.

Quantized weights save memory, but the products still need to be fast and
correct. This chapter quantizes activations to 8-bit blocks, multiplies them
with integer weights and accumulates exactly in 32-bit integers, then applies
the scales. The integer part is exact, so only the scaling and quantization
steps need a tolerance.

## What this chapter covers

- Activation blocks quantized to 8 bits, with a stored sum per block for the
  offset correction.
- Integer dot products accumulated exactly in `i32`.
- Two kernels:
  - a row kernel for a few tokens, as in decode;
  - a tiled int8 tensor-core kernel for whole prompts.
- Row interleaving so a SIMD or warp lane reads contiguous data.
- A separate numerical profile that states the error from quantizing
  activations.

## What it builds

- Planned integer product kernels and a CPU oracle that computes the same
  integer sums.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): the floating-point reference.
- [Chapter 37](ch037-a-coax-compute-subset.md): the device compute subset.
- [Chapter 50](ch050-quantized-weights-and-caches.md) uses these kernels for
  its weight formats; it builds on this chapter, not the other way round.

## Done means

- The integer accumulations equal the CPU oracle exactly for every fixture.
- The scaled result is within the stated tolerance of chapter 4's reference.
- Both the row kernel and the tiled kernel pass the same fixtures.
- The chapter measures throughput on the named device. A quantized profile
  never inherits the original weights' evidence.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): 8-bit activation
  blocks, `mmvq` row kernels and `mmq` tiles.
