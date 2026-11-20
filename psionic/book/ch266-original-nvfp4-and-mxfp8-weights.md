# 266. Original NVFP4 and MXFP8 weights

> **Status: unimplemented — design draft.** This chapter specifies authenticated
> loading and exact CPU dequantization of Inkling 2 Mini's native NVFP4 and
> MXFP8 weight tensors. No NVFP4/MXFP8 decoder or checkpoint admission is
> implemented here yet.

[Chapter 60](ch060-original-mxfp4-weights.md) decoded MXFP4 weights for
gpt-oss, and [chapter 82](ch082-original-fp8-weights.md) decoded
block-scaled FP8 weights for Mistral. Inkling 2 Mini combines two native
microscaled formats in one ~20 GB checkpoint: **NVFP4** across most layers and
**MXFP8** across the last four layers (see the
[Inkling 2 Mini target specification](../docs/targets/inkling.md)).

This chapter builds the weight-admission and CPU dequantization layer so every
later chapter reads the original published checkpoint directly, without
converting to a third-party quantized container.

## What the chapter will build

1. **Checkpoint manifest and hash admission.** Pin the upstream Inkling 2 Mini
   revision, safetensors shard list, full SHA-256 digests, and complete tensor
   inventory (dtype, shape, scale-tensor pairing, and layer assignment).
2. **NVFP4 block decoder.** Unpack 4-bit E2M1 nibbles in fixed micro-blocks,
   apply per-block FP8 (E4M3) scales and per-tensor FP32 global scales, and
   emit exact F32 values on CPU.
3. **MXFP8 block decoder.** Decode 8-bit FP8 elements with shared E8M0
   power-of-two block exponents for the final four layers, matching the
   checkpoint's exact block size and rounding convention.
4. **Unified weight-view interface.** Present NVFP4, MXFP8 and unconverted
   BF16/F32 tensors through one read-only tensor view so the looped transformer
   in [chapter 267](ch267-a-looped-moe-transformer.md) does not branch on file
   layout.

## Acceptance checks

- **Hand calculation:** decode one 16-element NVFP4 micro-block and one
  32-element MXFP8 micro-block by hand, including subnormals, negative zero,
  and block/global scale multiplication.
- **Independent oracle:** compare CPU dequantized slices from both formats
  against an independent Python reference across random and boundary bit
  patterns.
- **Refused inputs:** reject missing scale tensors, mismatched block counts,
  nonfinite scales, and SHA-256 digest mismatches with explicit diagnostics.
