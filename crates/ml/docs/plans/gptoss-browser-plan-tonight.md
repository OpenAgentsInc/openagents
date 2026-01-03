# GPT-OSS Browser Plan (Tonight Slice)

## Intent

Ship a feasibility vertical slice that proves the **browser + wgpu inference spine** works:

`GGUF parse → range fetch → GPU buffer → Q8_0 dequant + matmul → CPU readback compare`.

If this passes, everything else is "just more kernels."

---

## Tonight MVP (non-negotiable)

Goal: **one end-to-end compute path runs in-browser**:

1) Parse GGUF tensor table.
2) Range-fetch one **Q8_0** tensor slice.
3) Upload to GPU.
4) Run **one quantized linear** (small shape).
5) Read back output and compare to CPU reference (loose tolerance).

---

## Scope Cuts (hard)

- **Q8_0 only** (no Q4_K_M).
- **No MoE, no attention, no KV, no tokenizer/Harmony**.
- **No large buffers**: treat binding limits as tiny. Use tiled buffers even for one tensor.
- **No optimization pass**: correctness > speed tonight.

---

## Feasibility Gates (must pass)

### Gate A — GGUF Index
- Parse GGUF header + tensor table.
- Output `{name, ggml_type, dims, offset, nbytes}`.
- Confirm at least one Q8_0 tensor found.

### Gate B — Deterministic I/O
- Range-fetch a tensor slice by `(offset, nbytes)` and hash it.
- Same request yields same hash (cache optional).

### Gate C — GPU Compute
- Upload slice into `wgpu::Buffer`.
- Run compute shader that does:
  - Q8_0 dequant (f32 first; f16 optional later)
  - `Y = X @ W` for a **small shape** (e.g., `[1×K] @ [K×N]`, K,N <= 512)

### Gate D — Correctness
- CPU reference dequant + matmul.
- Read back GPU `Y` and compare within tolerance.

If Gate D passes, the browser spine is real.

---

## Minimal Kernel Plan

Start dumb, then optimize later:

- **Q8_0 block layout**: each block has a scale + 32 int8 values.
- Implement **dequant + dot** or **dequant + tiny matmul**.
- Avoid fancy tiling until correctness is proven.

---

## Recommended File Layout (minimal)

- `gguf/` — header + tensor table parsing
- `io/` — range fetch + small cache
- `gpu/` — device + buffer helpers + pipeline creation
- `kernels/` — WGSL (Q8_0 dequant + matmul)
- `tests/` — CPU reference dequant + matmul

---

## Execution Order (Tonight)

1) **GGUF parser**: load header + tensor table, dump tensor list.
2) **Range fetch**: pull a Q8_0 tensor slice, hash and verify determinism.
3) **CPU reference**: dequant + matmul for a tiny shape.
4) **WGSL kernel**: dequant + matmul for the same shape.
5) **Readback compare**: pass Gate D.

---

## Inputs

- Start with any **small GGUF** that includes Q8_0 (fast iteration).
- Once spine is proven, swap in GPT-OSS tensor slices.

---

## After Tonight (Phase 2)

Only after Gate D passes:

1) Scale GEMM shapes + tiling
2) RMSNorm + RoPE
3) Dense attention (single block)
4) MoE routing + expert cache
5) Tokenizer + Harmony last

---

## Reality Check

We do **not** promise "20B runs" tonight.
We prove the **browser inference spine** works. That's the only win that matters tonight.
