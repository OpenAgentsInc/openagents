# 269. Inkling 2 Mini on Metal

> **Status: unimplemented — design draft.** This chapter specifies Coax-generated
> Metal compute kernels and unified-memory residency planning for running
> Inkling 2 Mini interactively on Apple Silicon Macs with ≥32 GB RAM. No Metal
> NVFP4/MXFP8 kernels or Inkling residency plan are implemented here yet.

Building on [chapter 47](ch047-elephant-on-metal.md) (Metal shaders from
Coax) and [chapter 72](ch072-gpt-oss-on-metal.md) (gpt-oss on Metal), this
chapter moves Inkling 2 Mini's 3-pass looped MoE forward pass onto Apple
Silicon's GPU.

Inkling 2 Mini's ~20 GB weight footprint leaves roughly 10–12 GB on a 32 GB Mac
for macOS, the OpenAgents Desktop shell, KV cache across 3 loop passes, and
activation scratchpads. Explicit memory budgeting is therefore part of the
kernel contract, not an afterthought.

## What the chapter will build

1. **Fused NVFP4 and MXFP8 matvec/matmul kernels.** Emit Metal kernels from
   Coax that dequantize NVFP4 micro-blocks (most layers) and MXFP8 micro-blocks
   (last four layers) on the fly in registers during GEMV/GEMM, avoiding
   materialized FP16/FP32 weight copies.
2. **Looped MoE dispatch on Metal.** Execute per-pass top-k expert routing,
   gather/scatter, and per-pass KV-cache reads/writes in one command buffer per
   token.
3. **Residency and memory-pressure planner.** Compute exact static weight,
   per-pass KV-cache, and scratchpad bytes before loading; refuse to allocate
   beyond the device's recommended working-set budget or cap context length
   explicitly.
4. **CPU-to-Metal numerical parity.** Verify every kernel and full-model decode
   step against [chapter 268](ch268-execute-inkling-2-mini-on-cpu.md).

## Acceptance checks

- **Worked calculation:** compute the exact byte budget (weights + per-pass KV
  cache + scratchpad) for a 32 GB Mac at 4k, 16k and 32k context lengths.
- **Independent parity:** compare Metal single-token logits and 32-token greedy
  decode against the CPU reference in chapter 268 within declared tolerances.
- **Refused inputs:** reject context-length requests whose KV cache exceeds the
  device's admitted memory budget with a structured diagnostic.
