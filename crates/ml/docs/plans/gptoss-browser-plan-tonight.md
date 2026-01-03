# GPT-OSS Browser Plan (Tonight Slice)

## Intent

Ship a feasibility vertical slice that proves the **browser + wgpu inference spine** works:

`GGUF parse → range fetch → GPU buffer → Q8_0 dequant + matmul → CPU readback compare`.

If this passes, everything else is "just more kernels."

---

## Progress (2026-01-02)

### Gate A — GGUF Index (complete)

Implemented a local GGUF parser + tensor dump tool and validated against the
downloaded GPT-OSS GGUF.

Command:

```bash
cargo run -p ml --no-default-features --features native --bin gguf_dump -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --limit 20
```

Observed:
- `version: 3`
- `tensor_data_offset: 13008832`
- `tensor_count: 459`
- Q8_0 tensors present (e.g., `output.weight`, `token_embd.weight`)
- Unknown ggml type `39` appears for expert weights (still indexed cleanly)

### Gate B — Deterministic Range Reads (complete)

New `gguf_range` tool supports hashed range reads with optional tensor lookup.

Command:

```bash
cargo run -p ml --no-default-features --features native --bin gguf_range -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --tensor output.weight --len 1048576 --repeat 2
```

Observed:
- `sha256: ff6dcca8ec6f88daa59b9a8d6c583e288e0a5a182d86556712c48b820b519352`
- `consistent: true` across 2 reads

### Gate C — GPU Compute (complete)

New `gguf_gate_c` tool runs a tiny Q8_0 matmul on GPU via wgpu and compares
against a CPU reference for the same slice.

Command:

```bash
cargo run -p ml --no-default-features --features native,wgpu --bin gguf_gate_c -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --tensor output.weight --k 128 --n 64
```

Observed:
- `max_abs_diff: 9.313226e-10`
- `mean_abs_diff: 3.012701e-10`
- Q8_0 dequant + matmul runs end-to-end on GPU with CPU match within tolerance

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
