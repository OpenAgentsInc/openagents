# 2026-03-08 Psionic vs llama.cpp GPT-OSS Performance Audit

> Updated 2026-03-09 after the CUDA/runtime checkpoint that landed the perf
> groundwork through GitHub issues `#3242` through `#3246` and produced a new
> live benchmark on the local RTX 4080 host. This file is the current audit for
> the GPT-OSS throughput gap; later product truth still lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `crates/psionic/docs/ROADMAP.md`, and the referenced issues.

## Scope

- Psionic GPT-OSS path:
  - `crates/psionic/psionic-serve/src/gpt_oss.rs`
  - `crates/psionic/psionic-serve/src/lib.rs`
  - `crates/psionic/psionic-backend-cuda/src/lib.rs`
  - `crates/psionic/psionic-backend-cuda/src/kernels/quantized_matvec.cu`
  - `crates/psionic/psionic-backend-cuda/build.rs`
  - `crates/psionic/psionic-models/src/harmony.rs`
- `llama.cpp` reference path:
  - `~/code/llama.cpp/src/models/openai-moe-iswa.cpp`
  - `~/code/llama.cpp/src/llama-graph.cpp`
  - `~/code/llama.cpp/src/llama-context.cpp`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/mmvq.cu`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/vecdotq.cuh`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/fattn.cu`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/mmid.cu`
- Harmony reference:
  - `~/code/harmony/src/chat.rs`
  - `~/code/harmony/src/encoding.rs`

## Benchmark Contract

- Model:
  - `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`
- Host:
  - local NVIDIA host with `NVIDIA GeForce RTX 4080`
- API flow:
  - `POST /v1/chat/completions`
  - `127.0.0.1:8099`
  - `-c 4096`
  - `-ngl 999`
  - warm once, time the second request
- Deterministic prompt:
  - system and developer messages match the local benchmark script
  - user asks for exactly one sentence:
    `HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

## Measured History

### Original baseline

- Psionic:
  - `37` completion tokens in `2.211s`
  - `16.74 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.245s`
  - `171.22 tok/s`
- Gap:
  - `10.23x`

### Current checkpoint

- Psionic:
  - `37` completion tokens in `1.037s`
  - `35.70 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.249s`
  - `168.53 tok/s`
- Gap:
  - `4.72x`
- Psionic improvement over original baseline:
  - `2.13x`

The visible output text matched exactly in the current benchmark:

`HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

## What Landed Between The Two Measurements

The performance gap is smaller than it was, and the reason is concrete.

The current Psionic checkpoint added:

- benchmark JSON summaries and request-level perf receipts
- device-resident CUDA step-plan buffers in `psionic-serve`
- CUDA KV-cache mirrors, CUDA RMSNorm, CUDA RoPE, and CUDA decode attention
- CUDA router top-k softmax and CUDA MoE projection / aggregation surfaces
- a single CUDA submission per generated token instead of one submission per
  layer plus a separate logits submission
- `Q8_1` scratch quantization plus `Q8_0 x Q8_1` fast-path kernels
- CUDA build tuning in `psionic-backend-cuda/build.rs`:
  - `-O3`
  - `--use_fast_math`
  - architecture detection from `CUDAARCHS`, `PSI_CUDA_ARCH`, or `nvidia-smi`

Two correctness fixes were also necessary before the faster path was usable:

- GGML `Q8_1` block size had to be corrected to `36` bytes, not `34`
- `Q8_1` scratch sizing had to be widened to the maximum actual projection
  width, not just hidden size

That work is why Psionic moved from `16.74 tok/s` to `35.70 tok/s` without changing
the benchmark contract or delegating execution to `llama.cpp`.

## Current Hard Evidence From The Psionic Benchmark

The Psionic benchmark receipt for the timed request currently reports:

- `step_count = 37`
- `layer_visit_count = 888`
- `host_to_device_bytes = 426240`
- `device_to_host_bytes = 29761024`
- `submission_count = 37`
- `sync_count = 37`
- `kernel_launches = 17871`

Important interpretation:

- the execution-boundary work helped a lot
  - decode is down to one submission and one sync per generated token
- host-to-device traffic is no longer the main problem
  - only about `11.5 KB` per token in the timed lane
- device-to-host traffic is still dominated by full logits readback
  - about `804 KB` per token
- kernel launch count is still very high
  - about `483` launched operations per generated token

Timing caveat:

- the current stage timing fields are trustworthy for broad direction but not
  for precise per-stage attribution after the single-submission rewrite
- the byte counters, submission count, sync count, and end-to-end benchmark
  numbers are the solid evidence to use for optimization decisions right now

## Exact Reason Psionic Is Still Slower

This is no longer a generic "CPU vs GPU" story. The current gap is now mostly a
kernel-shape and execution-shape story.

### 1. Psionic still uses much weaker quantized CUDA kernels than `llama.cpp`

This is now the largest remaining reason for the gap.

Psionic's core quantized decode kernel in
`crates/psionic/psionic-backend-cuda/src/kernels/quantized_matvec.cu` still launches one
CUDA block per output row in `quantized_matvec_q8_1_kernel(...)`. Each block is
just `128` threads reducing over the block count for that row, then writing one
`f32` result.

That is simple and correct, but it is not a high-throughput shape for GPT-OSS,
especially for the final logits projection where:

- output rows are the whole vocabulary: `201088`
- input width is `2880`
- each token requires a full-vocab projection

By contrast, `llama.cpp` does not use a one-block-per-row reduction for this
class of work. Its quantized matvec path in `ggml-cuda/mmvq.cu` and
`ggml-cuda/vecdotq.cuh` is warp-specialized and type-specialized:

- `mul_mat_vec_q(...)` chooses `nwarps` and `rows_per_cuda_block` from the
  destination shape
- `vec_dot_q8_0_q8_1(...)` and `vec_dot_mxfp4_q8_1(...)` use higher VDR
  settings and tighter warp-level packing
- the kernels are built to cooperate with fused bias / gate / GLU paths instead
  of only serving as isolated row reducers

That difference alone explains why Psionic can now run the full GPT-OSS pipeline on
GPU and still remain far behind `llama.cpp`.

### 2. Psionic still reads the full logits vector back to the host every token

The current timed request moved `29761024` bytes from device to host over
`37` generated tokens. That is almost exactly the full vocabulary logits tensor
every step:

- `201088` logits
- `4` bytes each
- `804352` bytes per token

This comes directly from `plan.logits_buffer.read_f32()?` in
`crates/psionic/psionic-serve/src/gpt_oss.rs`.

That means Psionic still pays:

- full-vocab device-to-host copy
- sync before readback
- host-side sampling over the returned `Vec<f32>`

`llama.cpp` keeps the decode execution far tighter around the ggml backend and
does not expose a "copy the full logits tensor back to Rust every step" seam in
the way Psionic currently does.

Even if the final sampler still needs host involvement, Psionic should not need to
materialize the entire logits vector on the CPU every token.

### 3. Psionic now has one submission per token, but `llama.cpp` still has the stronger execution shape

Moving from per-layer submissions to one submission per token was correct and
worth it. The current `submission_count = 37` and `sync_count = 37` prove that.

But Psionic still manually encodes every op into that submission from Rust each
token. `llama.cpp` builds the OpenAI-MoE decode as a graph in
`src/models/openai-moe-iswa.cpp` and `src/llama-graph.cpp`, then lets the ggml
backend scheduler execute that graph shape.

That gives `llama.cpp` three advantages Psionic still lacks:

- less Rust-side per-token orchestration
- better opportunities for backend-level fusion and scheduling
- a much more natural route to CUDA graph capture and reuse

Psionic has a reusable decode-step plan. It does not yet have a `llama.cpp`-class
backend execution shape.

### 4. Psionic attention is CUDA-backed now, but it is not flash-attention-class

The current Psionic attention kernel is a straightforward decode kernel:

- one block per head
- shared-memory logits and weights
- scalar loops over head dimension and active KV window
- explicit sliding-window cap logic in the kernel itself

That is a huge improvement over the old CPU `attend_impl(...)`, but it is still
much simpler than `llama.cpp`'s flash-attention path in `ggml_flash_attn_ext(...)`
and the CUDA `fattn` kernels.

On this exact benchmark, attention is no longer the single biggest bottleneck,
but it still leaves performance on the table and blocks parity on longer or
less cache-friendly prompt shapes.

### 5. The MXFP4 expert fast path is still not fully trustworthy at real-model scale

This GPT-OSS model is mixed:

- many tensors are `Q8_0`
- expert tensors are `MXFP4`

Psionic now has `Q8_1` fast paths for:

- `Q8_0 x Q8_1` projection
- `MXFP4 x Q8_1` projection
- fused MoE gate/up and down aggregation variants

But the full real-model MXFP4 expert `Q8_1` fast path was still producing wrong
text in live generation. The current stable runtime therefore keeps a hybrid
policy:

- use the fast `Q8_1` path when both expert tensors are `Q8_0`
- keep the slower path for the real MXFP4 expert lane so output stays correct

That is the right product decision today, but it means the benchmark is still
paying for a slower expert path than `llama.cpp`, whose OpenAI-MoE graph uses
`build_moe_ffn(...)`, `ggml_mul_mat_id(...)`, and the CUDA grouped-expert
machinery in `ggml-cuda/mmid.cu`.

### 6. `llama.cpp` is tuned for this GPU class; Psionic is only beginning to be

`llama.cpp`'s CUDA quantized code has explicit architecture-aware tuning,
multiple kernel families, and extensive type coverage. Psionic only recently gained
basic build-time tuning via:

- `-O3`
- `--use_fast_math`
- selected `sm_` architecture

That helps, but it is the beginning of GPU tuning, not the end of it.

## The Single Biggest Remaining Bottleneck

The current checkpoint strongly suggests that the first optimization target for
`#3247` should be the final logits path, not another orchestration rewrite.

Why:

- the current benchmark is already at one submission per token
- host-to-device traffic is already small
- the remaining device-to-host traffic maps almost exactly to full-vocab logits
- the final projection is the widest matvec in the decode step
- Psionic's current kernel for that projection is still the simplest possible
  one-block-per-row reduction

If Psionic does not fix the logits projection kernel shape and the logits readback
contract, it will stay far behind even if smaller kernels improve.

## Clear Path To llama.cpp-Class Speed

The remaining sequence should be executed in this order:

1. Upgrade the quantized CUDA kernels for the real GPT-OSS shapes
   - replace the current one-block-per-row `Q8_0` and `MXFP4` kernels with
     warp-specialized, tiled MMVQ/MMQ-style kernels
   - tune for Ada / RTX 4080 specifically
   - target the output projection first because it dominates the timed lane
2. Stop reading the full logits tensor back to host every token
   - do on-device top-k / argmax selection or another truthful reduced-readback
     path
   - keep full-logits readback only when the API or diagnostics actually need it
3. Make the MXFP4 expert fast path numerically trustworthy
   - fix the real-model MXFP4 `Q8_1` expert path until it matches current text
     outputs
   - then switch the live GPT-OSS expert lane onto it
4. Add grouped expert execution that is closer to `ggml_mul_mat_id(...)`
   - compact and run work by expert instead of treating each selected expert as
     an isolated path
5. Strengthen the attention path toward flash-attention-class execution
   - keep the current kernel as the floor, not the ceiling
6. Move from "manual per-token encoded submission" to a true backend-captured
   decode execution shape
   - reuse graphs or CUDA graph capture for the stable decode lane

## Honest Status

Psionic is no longer "nowhere close." The current checkpoint is a real improvement:

- the exact Psionic-owned GPT-OSS HTTP flow works
- the visible benchmark output matches `llama.cpp`
- Psionic is about `2.13x` faster than it was at the start of the perf track

But Psionic is still not in the same speed class as `llama.cpp` on this host. The
current measured gap is still large enough that `#3247` and `#3248` must remain
open.

The correct summary is:

- the execution-boundary issues are mostly solved
- the remaining gap is now mostly kernel quality, logits readback policy, and
  grouped/fused GPU execution quality
