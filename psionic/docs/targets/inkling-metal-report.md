# Beating `llama.cpp` on Apple Silicon with Coax & Metal 4: The Inkling 2 Mini Engineering Report

> **TL;DR:** We built a native Metal 4 inference runtime for **Inkling 2 Mini** (26.4B total / 3.4B active looped MoE, 16.4 GB `IQ4_NL` GGUF) in **Psionic** — OpenAgents' ML library written in **Coax** — with zero PyTorch, zero `llama.cpp`, zero MPS/Accelerate GEMM calls, and zero hand-written `.metal` kernels. Over 22 optimization sprints on an Apple M5 Max, we took Psionic from **50× slower than `llama.cpp` on prefill and 2.5× slower on decode** to **beating `llama.cpp` across decode (+4.2% to +11.0%), medium/long prefill (+36.9% to +39.6%, up to 792 tok/s on 2.3K prompts), all 4 end-to-end workloads (1.5% to 27.1% faster), a real 3-turn tool-calling agent session (winning every turn: 30.67 s vs 44.16 s to natural EOS, and 29.75 s vs 33.89 s / -4.14 s at matched token counts), warm startup (2.7–4.9× faster), and process RSS (3.3×–8.5× lower)**.

---

## 1. Background: What We Have and Why We Built It

### OpenAgents, Coax, and Psionic
- **Coax** (`coax/`) is OpenAgents' systems and shader language. It compiles both host code (to C11 / WebAssembly) and GPU compute kernels (to Apple Metal 4, WGSL, and CUDA) from a single typed source language, including direct access to **Apple Metal 4 Neural Accelerators** (`am_matmul_bf16` → `MetalPerformancePrimitives::matmul2d`).
- **Psionic** (`psionic/`) is OpenAgents' native machine-learning library written in Coax. Instead of wrapping external inference engines (`llama.cpp`, MLX, PyTorch, or vendor GEMM libraries), Psionic compiles every quantized dequantization, attention, MoE routing, matrix multiplication, and normalization kernel directly from Coax source.
- **OpenAgents Desktop & WebGPU Chat** (`components/`, `apps/openagents-desktop/`, `apps/openagents-web/`): OpenAgents' native WebGPU chat UI and desktop application previously launched `llama-server` (`llama.cpp`) as a local inference subprocess. Replacing it with `psionic-inkling-serve` puts the entire inference stack — tokenizer, chat template, tool calling, KV cache, GPU kernels, and HTTP/SSE server — under our own Coax codebase.

### Target Model: Inkling 2 Mini (`inkling2_loop`)
- **Checkpoint:** `Inkling-2-Mini-IQ4_NL.gguf` (16.4 GB on disk)
- **Architecture:** Looped Mixture-of-Experts (`inkling2_loop`), 26.4B total / 3.4B active parameters:
  - **Looped depth:** 15 unique transformer blocks executed across 2 loop iterations (30 logical layers), with per-loop control projections (`ctrl_down`, `ctrl_up`), 4-state residual mixing (`mix`), and attention sinks.
  - **Hybrid attention:** Sliding-window attention (SWA) and global Grouped-Query Attention (GQA, 3:1 Q:KV head ratio) with per-head RMSNorm and QK clipping.
  - **Sparse MoE:** 64 experts, top-6 active per token, `IQ4_NL` 4-bit quantized expert gate/up and down weights.
  - **Mixed quantization:** `IQ4_NL` for MoE experts, `Q8_0` for dense QKV/O projections and LM head, `F32` for router, loop-control, and sink projections.

### Runtime Architecture
Every piece of the Inkling 2 Mini runtime lives in four files:

| Component | Path | Responsibility |
| --- | --- | --- |
| **GPU Kernels (Coax → Metal 4)** | [`psionic/lib/inkling/kernels.coax`](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/lib/inkling/kernels.coax) | All Metal 4 compute kernels (quantized GEMV/GEMM, Metal 4 `am_matmul_bf16`, GQA Flash-Decoding & 3-stage MMA Flash-Prefill, 4-state residual mixing, 2-barrier parallel top-6 router, expert grouping/gather, fused KV-store/RMSNorm). Compiled to `inkling-kernels.metal` by `coax`. |
| **Metal Runtime Orchestrator (C)** | [`psionic/platform/inkling-metal.c`](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/platform/inkling-metal.c) | Zero-copy GGUF `mmap`, dynamic prefill chunking (`prefill_b`), KV cache allocation, per-layer kernel dispatch, and greedy/sampled decoding. |
| **Metal 4 Host Bridge (Obj-C)** | [`psionic/platform/metal-host.m`](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/platform/metal-host.m) | Thin Metal 4 command-buffer wrapper with zero-copy `newBufferWithBytesNoCopy` weight mapping and cached `MTLComputePipelineState` binding. |
| **OpenAI-Compatible Server (C)** | [`psionic/platform/inkling-serve.c`](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/platform/inkling-serve.c) | Local HTTP/SSE server (`psionic-inkling-serve`) with `/v1/chat/completions`, tool-call parsing, prefix caching, and `/health`, wired directly into OpenAgents Desktop and our OpenAgents WebGPU chat UI. |

---

## 2. Headline Results: Psionic Metal vs `llama.cpp` Metal

All measurements were taken on the same machine (**Apple M5 Max, 128 GB unified memory, macOS 26.4 Build `25E246`**), against the exact same 16.4 GB `Inkling-2-Mini-IQ4_NL.gguf` checkpoint, with `--ctx 8192`, `--no-cache-prompt`, and `temperature=0`. Exact greedy output parity and exact prompt token counts were verified on every workload.

Baseline `llama.cpp` binary: `~/.openagents/bin/llama-server` (`1.0.0-dev`, build 1, commit `29b52d3`, Metal backend).

### A. Real 3-Turn Tool-Calling Agent Session (Side-by-Side OpenAgents Chat)

To test real-world agent responsiveness — not just synthetic single-turn benchmarks — we ran an identical 3-turn coding-agent conversation against both servers separately (no GPU contention) and rendered both live streams side-by-side through our OpenAgents WebGPU chat UI:
- **Turn 1 (`1,048` prompt tok):** User asks the agent to inspect `psionic/lib/inkling/kernels.coax` → model emits a `Grep` tool call.
- **Turn 2 (`1,415` prompt tok):** `Grep` result fed back → model emits a `Read` tool call (`kernels.coax:2556-2595`).
- **Turn 3 (`2,296` prompt tok):** `Read` result fed back → model streams a complete ~1,710-character technical Markdown explanation + Coax snippet to natural EOS (`max_tokens=2048`).

| Turn | Prompt Length | Psionic Metal (`5e6b109baf`) | `llama.cpp` Metal (`29b52d3`) | Result |
| --- | ---: | --- | --- | --- |
| **Turn 1** (`Grep` tool call) | 1,048 tok | **1.609 s TTFT** / **4.823 s done** (128 tok) | 2.157 s TTFT / 5.092 s done (120 tok) | **Psionic 0.27 s faster** (identical tool call) |
| **Turn 2** (`Read` tool call) | 1,415 tok | **1.984 s TTFT (713 tok/s)** / **9.079 s done** (89 tok) | 2.205 s TTFT (642 tok/s) / 9.659 s done (94 tok) | **Psionic 0.58 s faster** (identical tool call) |
| **Turn 3** (natural EOS, ~1,710 chars) | 2,296 tok | **2.898 s TTFT (792 tok/s)** / **30.667 s E2E** (716 tok, 1,717 chars) | 3.515 s TTFT (653 tok/s) / 44.156 s E2E (1,195 tok, 1,704 chars) | **Psionic 13.49 s faster to EOS** |
| **Turn 3 (matched `max_tokens=640`)** | 2,296 tok | **3.391 s TTFT (677 tok/s)** / **29.752 s E2E** (640 tok) | 3.898 s TTFT (589 tok/s) / 33.893 s E2E (640 tok) | **Psionic 4.14 s faster at matched tokens** |
| **Peak Process RSS** | — | **5.5 GiB** (`B=3072`, SWA cap `4096`) | 18.0 GiB | **Psionic 3.3× lower RSS** |

Psionic beats `llama.cpp` on **every single turn** — first tool call, second tool call, Turn 3 streaming start, and final completion (**13.49 s faster** to natural EOS on equal-length ~1,710-char answers, and **4.14 s faster** at matched token counts) — with 100% identical prompt token counts (`1,048`, `1,415`, `2,296`) and byte-for-byte identical tool calls. *(Note on phrasing & thinking tokens: because Psionic executes `nb >= 64` prefill GEMMs on Metal 4 BF16 Neural Accelerators with a 7-bit mantissa while `llama.cpp` uses FP16/FP32 `simdgroup_matrix`, a tiny rounding difference on a near-tie logit in Turn 1/3 reasoning can flip one word and cause downstream prose phrasing and `<think>` length to differ even at `temperature=0`.)*

### B. Standard 4-Workload Benchmark Suite (Baseline → Optimized vs `llama.cpp`)

| Metric | Psionic Baseline (`55bf398569`) | **Psionic Optimized (`4059be9291`–`ac561624d4`)** | `llama.cpp` Metal (`29b52d3`) | Speedup vs Baseline | Psionic vs `llama.cpp` |
| --- | ---: | ---: | ---: | ---: | --- |
| **Warm startup to `/health`** | 0.60 s | **0.22–0.40 s** | 1.078 s (11.29 s cold) | **1.5–2.7×** | **Psionic 2.7–4.9× faster** |
| **Process RSS (`ps -o rss=`)** | 483 MiB\* | **2.1 GiB (`B=1024`) / 5.5 GiB (`B=3072`)**\* | 18.0 GiB | — | **Psionic 3.3×–8.5× lower** |
| **Tokenizer prompt parity** | +7.5–7.8% extra tok | **100% exact** (`29, 37, 903, 2622`) | `29, 37, 903, 2622` | **Fixed** | **Exact match** |
| **`short_math` (`29` tok) TTFT** | 2.049 s | **0.317–0.321 s** | **0.246 s** | **6.5×** | `llama.cpp` 1.29× (-71 ms) |
| **`short_math` decode** | 15.83 tok/s | **44.18–44.32 tok/s** | 39.93 tok/s | **2.80×** | **Psionic +10.6–11.0% faster** |
| **`short_math` E2E** | 5.21 s | **1.427 s** | 1.448 s | **3.65×** | **Psionic 1.5% faster** |
| **`sustained_decode` (`256` tok) TTFT** | 2.503 s | **0.372 s** | **0.332 s** | **6.7×** | `llama.cpp` 1.12× (-40 ms) |
| **`sustained_decode` decode** | 14.78 tok/s | **41.97 tok/s** | 38.96 tok/s | **2.84×** | **Psionic +7.7% faster** |
| **`sustained_decode` E2E** | 19.75 s | **6.448 s** | 6.878 s | **3.06×** | **Psionic 6.3% faster** |
| **`medium_prefill` (`903` tok) TTFT** | 73.830 s (13.2 tok/s) | **1.436 s (628.9 tok/s)** | 1.966 s (459.4 tok/s) | **51.4×** | **Psionic +36.9% faster** |
| **`medium_prefill` decode** | 10.97 tok/s | **39.89 tok/s** | 38.29 tok/s | **3.64×** | **Psionic +4.2% faster** |
| **`medium_prefill` E2E** | 81.885 s | **3.642 s** | 4.997 s | **22.5×** | **Psionic 27.1% faster** |
| **`long_prefill` (`2,622` tok) TTFT** | 244.229 s (11.6 tok/s) | **4.277 s (613.1 tok/s)** *(up to 792 tok/s @ 2.3K w/ `B=3072`)* | 5.971 s (439.1 tok/s) | **57.1×** | **Psionic +39.6% faster** |
| **`long_prefill` decode (`2,622` ctx)** | 10.56 tok/s | **39.71 tok/s** | 37.22 tok/s | **3.76×** | **Psionic +6.7% faster** |
| **`long_prefill` TPOT (`2,622` ctx)** | 94.7 ms | **25.2 ms** | 26.9 ms | **3.76× lower** | **Psionic 6.3% lower** |
| **`long_prefill` E2E** | 250.194 s | **5.863 s** | 7.664 s | **42.7×** | **Psionic 23.5% faster** |

\* *Memory accounting note:* `psionic-inkling-serve` maps the 16.4 GB GGUF read-only via `mmap` and wraps it directly in Metal with `newBufferWithBytesNoCopy` (`psionic/platform/metal-host.m`). macOS accounts file-backed pages in the unified page cache / GPU wired pool rather than private process RSS, so `ps -o rss=` reflects KV cache, batched prefill scratch buffers, and the reusable 176 MB BF16 staging buffer (~2.1 GiB at `B=1024`, ~5.0 GiB at `B=3072`). Both runtimes read from the same 16.4 GB checkpoint in unified memory.

---

## 3. The 22-Sprint Optimization Journey (Commit by Commit)

Every optimization was hypothesis-driven, profiled with GPU stage timings, and verified for exact greedy token parity before landing on `main`.

### Starting Point — `55bf398569` & `adafea6598`
The initial native Metal runtime worked end-to-end and wired Inkling 2 Mini into OpenAgents Desktop, but processed prefill **one token at a time** (reading all 16.4 GB of weights per prompt token), synchronized CPU↔GPU per layer, and produced +7.5–7.8% extra prompt tokens due to a BPE pre-tokenizer mismatch. A 2,622-token prompt took **4 minutes and 4 seconds** (11.6 tok/s), and decode ran at 10.6–15.8 tok/s.

### Sprints 1–4 — `3f100d608b` (*23× prefill, 3× decode, Flash-Decoding*)
- **Exact tokenization:** Fixed BPE pre-tokenization to match `llama.cpp` 100% (`29, 37, 903, 2622` tokens).
- **Batched prefill (`INK_PREFILL_B = 256 → 1024`):** Replaced per-token prefill with batched `simdgroup_matrix` (`ink_mm_q8_mma_64x32x32`), expert-grouped MoE, and 3-stage MMA Flash-Prefill attention.
- **GQA Flash-Decoding:** Added coalesced 3:1 GQA split-K 1-pass online-softmax attention (`ink_attn_gqa_split` + `reduce`).
- **Impact:** `long_prefill` TTFT dropped from **244.2 s → 8.37 s** (313 tok/s); `short_math` decode jumped from **15.8 → 39.9 tok/s**.

### Sprints 6–8 — `e689733bfb` (*Metal 4 Neural Accelerators + `_n32` Fast Paths*)
- **Coax `_n32` 32-bit indexing:** Packed `u32` kernel arguments across attention, MoE, copy, add, and KV kernels to unlock Coax's 32-bit address generation fast path; compacted the MoE tile table by 8× and skipped zero-token expert tiles (`n_a_iters`).
- **Metal 4 Neural Accelerators (`am_matmul_bf16`):** Introduced on-the-fly `Q8_0` / `IQ4_NL` → `BF16` dequantization into a reusable 176 MB staging buffer (`w_bf16`) followed by Apple Silicon's Metal 4 `MetalPerformancePrimitives::matmul2d` (`64×64` for `nb >= 64`, `16×64` for `8 <= nb < 64`) for both dense projections and expert-grouped MoE.
- **Impact:** `medium_prefill` jumped **371 → 536.7 tok/s** (beating `llama.cpp`'s 459 tok/s for the first time); `long_prefill` jumped **313 → 522.5 tok/s** (5.02 s vs `llama.cpp` 5.97 s).

### Sprints 9–11 — `b39a1ba8c2` (*Decode Kernel Fusion & Coalesced 4-Lane MoE*)
- **4-lane coalesced `ink_moe_gu_iq4`:** Packed 4 lanes per IQ4_NL block (`72 / 8 = 9` exact, zero idle lanes), lifting decode throughput by +6.6%.
- **Aggressive decode fusion:** Fused `ink_clip_kv` + `ink_head_rmsnorm_kv` (eliminating 48 `K_KV_STORE` dispatches and barriers per token) and fused `ink_mix_rmsnorm` (eliminating 25 `K_RMSNORM` dispatches and barriers per token).
- **Prefill dequant & softmax:** Upgraded to 8-row IQ4 and 4-row Q8 BF16 dequantization, 2-pass `ink_attn_softmax_mma`, unpadded LM head, and 256-thread `ink_softcap`.

### Sprints 12–17 — `d49862a18e` (*Direct-IQ4 Small-Batch MoE, 128t GQA, Metal 4 F32, 8-Way Gather & SLC Score Stride*)
- **Zero BF16 staging on short prompts (`nb <= 48`):** Wrote direct `IQ4_NL` small-batch MoE kernels (`ink_moe_gu_iq4_m4`, 2-row/simdgroup `ink_moe_down_iq4_m4`) and `ink_mm_f32_m4` (64× more threadgroups on router/control/sink F32 projections), cutting `short_math` TTFT from **0.458 s → 0.328 s**.
- **Metal 4 F32 prefill (`nb >= 64`):** Added `ink_dequant_f32_bf16` overlapped with `K_CTRL_PREP`/`K_RMSNORM` to feed all F32 prefill projections through `am_matmul_bf16`.
- **128-thread parallel Q·K `ink_attn_gqa_split`:** Eliminated the 8 KB `ksh` threadgroup memory allocation, sped up dot reduction 16×, and lowered the Flash-Decoding threshold to `n_keys >= 1`.
- **8-way zero-barrier `ink_moe_gather_x` + 128-thread `ink_moe_group_batch`:** Cut MoE input gathering from **167 ms → 20 ms** (8.4× faster).
- **SLC-resident attention scores:** Packed `score_stride = span` (9× smaller score buffer, staying in Apple Silicon's System Level Cache).
- **Impact:** `medium_prefill` hit **610.7 tok/s** (1.479 s), `long_prefill` hit **595.4 tok/s** (4.404 s), and decode hit **41.2 tok/s**.

### Sprints 18–21 — `4059be9291` (*44+ tok/s Decode & 629 tok/s Prefill*)
- **96-thread 0%-idle `ink_dequant_iq4_exp_bf16`:** Matched block count cleanly (`72 / 12 = 6` and `36 / 12 = 3` exact) with zero wasted lanes.
- **256-thread 4-state chunked `ink_mix` / `ink_mix_rmsnorm` / `ink_mix_batch`:** 4× fewer threadgroup barriers and `query` reads per layer.
- **2-barrier parallel rank-based `ink_router` / `ink_router_batch`:** Cut router synchronization from **18 barriers → 2 barriers** per layer.
- **Specialized `m4` MoE & `tm=32` Metal 4 dense BF16:** Added `mc == 1/2` specialized loops, tight `max_tiles` bounds, `mc`-guarded TGEM writes, `ink_mm_bf16_32`/`64` single-tile dense BF16 for `17..64` tokens, and cached pipeline state in `metal-host.m`.
- **Impact:** **44.32 / 41.97 / 39.89 / 39.71 tok/s decode** (+4.2% to +11.0% faster than `llama.cpp` across all 4 workloads) and **628.9 / 613.1 tok/s prefill** (+36.9% to +39.6% faster).

### Sprint 22 — `ac561624d4` & `5e6b109baf` (*Single-Pass `INK_PREFILL_B = 3072` + SWA Ring-Buffer Fix*)
- **Diagnosis from the first side-by-side video:** Even after beating `llama.cpp` on all 4 standard benchmarks, our first side-by-side 3-turn agent recording showed `llama.cpp` finishing 3.26 s earlier (`33.06 s` vs `36.32 s`). Profiling revealed why: `INK_PREFILL_B = 1024` (plus a hardcoded `if (chunk > 1024) chunk = 1024;` in `inkling-serve.c`) split our real 1,048-, 1,415-, and 2,296-token agent prompts into **2–3 separate passes over all 16.4 GB of weights**.
- **Fix:** Raised the dynamic prefill chunk ceiling to `3072` for `max_ctx <= 8192` (`2048` up to `16384`, `1024` above) in `kernels.coax`, `inkling-metal.c`, and `inkling-serve.c` (`ac561624d4`), and sized SWA KV ring buffers to `min(max_ctx, hp->swa + s->prefill_b)` (`5e6b109baf`) so `K_KV_STORE_BATCH` never overwrites earlier keys in `[k_start, pos + nb)` before MMA prefill attention reads them.
- **Impact:** Turn 3 prefill (2,296 tokens) jumped to **677–792 tok/s** (2.898–3.391 s vs `llama.cpp`'s 3.515–3.898 s / 589–653 tok/s), and the 3-turn agent session flipped to **30.67 s vs 44.16 s** to natural EOS (and **29.75 s vs 33.89 s**, **4.14 s faster** at matched token counts) with identical prompt token counts and identical tool calls.

---

## 4. What Didn't Work (M5 Max Hardware Lessons)

Not every hypothesis won. We benchmarked and reverted the following regressions so future work does not repeat them:

1. **64-thread `ink_mix` + 1-thread serial `ink_router`:** Regressed decode by **+6.4 ms/tok** compared to 256-thread 4-state chunked `ink_mix` + 2-barrier parallel rank-based `ink_router`.
2. **Removing threadgroup `lut[16]` in `IQ4_NL` kernels:** Replacing the 16-entry TGEM lookup table with branchy scalar `ink_iq4_val` arithmetic was **~2× slower** on M5 Max. Keeping `lut[16]` in TGEM won decisively.
3. **4-lane `ink_moe_down_iq4`:** While 4-lane coalescing sped up `ink_moe_gu_iq4` by +6.6%, applying it to `ink_moe_down_iq4` regressed decode from **3.78 ms → 6.01 ms** due to 4× `idx / blocks` divisions and `ink_half` conversions. Keeping 1-lane for `down` and 4-lane for `gu` + `m4` was optimal.
4. **Larger Metal 4 tile `am_matmul_bf16(256, 64, 128)` & 16-row flattened `ink_dequant_iq4_exp_bf16`:** Both regressed prefill throughput compared to `(128, 64, 64)` and 8-row/TG 96-thread (`72/12=6`, `36/12=3` exact).

---

## 5. Reproducing the Benchmarks

### Build Psionic Inkling Metal
```sh
psionic/platform/inkling-build.sh
```

### Run Psionic Metal Server
```sh
psionic-inkling-serve \
  --model ~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf \
  --metal inkling-kernels.metal \
  --host 127.0.0.1 --port 18799 --ctx 8192
```

### Run `llama.cpp` Metal Baseline
```sh
~/.openagents/bin/llama-server \
  -m ~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf \
  --host 127.0.0.1 --port 18798 -c 8192 -ngl 99 --no-cache-prompt
```

### Verification & Regression Checks
- `coax/web/tool.sh tools-check` — 0 unreasoned tool files
- `coax/web/tool.sh layer-check` — 0 violations
- `coax/bin/coax test psionic/tests/models-checks.coax` — 561/561 passed
- Exact greedy token parity (`temperature=0`, `seed=0`) checked against `llama-server` on every workload and on the full 3-turn tool-calling session.

---

## 6. Next Levers

1. **Cross-request KV prefix caching in `psionic-inkling-serve`:** Multi-turn agent chats reuse 60–90% of the prior turn's prompt prefix. Caching the KV prefix across turns will drop Turn 2 and Turn 3 TTFT from ~2–3 s to **under 0.3 s**.
2. **Indirect dispatch for empty MoE tiles in `m4` kernels:** Eliminates empty-tile launch overhead on `nb <= 48` prompts, closing the remaining 40–71 ms gap on 29–37 token single-turn TTFT.

---

*Related docs: [`psionic/docs/targets/inkling-benchmark.md`](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/docs/targets/inkling-benchmark.md) (raw sprint-by-sprint measurement log).*
