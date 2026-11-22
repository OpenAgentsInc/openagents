# Psionic Inkling CUDA Hillclimb Report (2026-11-22)

**Tracking issue:** #13966 (Track B — Cross-platform Inkling engine boundary, Win32 portability & CUDA kernel hillclimbing)  
**Hardware:** NVIDIA GeForce RTX 4080 (16 GB VRAM, Ada `sm_89`), Linux, driver `595.71.05` (`coderos-4080`)  
**Checkpoint:** `Inkling-2-Mini-IQ4_NL.gguf` (17.6 GB, `phys=24`, `loop=[6..18)`, `K=3`, `slots=48`, 128 MoE experts / 8 active per token)  
**Constraint:** 100% Coax kernels (`psionic/lib/inkling/kernels-cuda.coax` emitted via `coax cuda --release`, using `#authority` `coax/lib/compute/mma.coax`). Zero cuBLAS, zero cuDNN, zero hand-written CUDA kernels.

---

## Executive Summary

Every software-reachable milestone in #13966 Track B is complete and verified live on RTX 4080:

| Metric | Baseline (start of session) | Final (verified 2026-11-22) | Target |
| --- | ---: | ---: | ---: |
| **Prefill (900-token prompt)** | `180 tok/s` (`156.2 tok/s` w/ `INK_NO_MMA=1`) | **`634.3 tok/s`** (`1.419s`, **4.06×** vs `INK_NO_MMA=1`) | `>= 500 tok/s` |
| **Prefill (1,219-token prompt)** | `107.3 tok/s` (scalar fallback) | **`632.7 tok/s`** (`1.927s`, **5.90×** vs scalar) | `>= 500 tok/s` |
| **Prefill (1,993-token prompt)** | — | **`598.0 tok/s`** (`3.333s`) | `>= 500 tok/s` |
| **Decode (128 tok, short ctx, CLI)** | `75.7 tok/s` | **`78.19 tok/s`** (`1.637s`, segmented CUDA Graphs) | preserve/improve `75.7 tok/s` |
| **Decode (`psionic-inkling-serve --ctx 32768`, warm prefix)** | — | **`83.1 tok/s`** (`74.3 tok/s` cold, `0.001s` prefix restore) | clean 32K serve |
| **Decode (64 tok after 900-tok prompt)** | `64.5 tok/s` | **`63.87 tok/s`** (`1.002s`) | preserve |
| **8 GB simulated VRAM (`INK_VRAM_MB=8192`, 1,219 tok)** | `38.7 tok/s` prefill | **`93.1 tok/s`** prefill, `35.89 tok/s` decode, **bit-identical** logits | clean streaming |
| **Warm NVRTC startup compile** | `~3.77s` | **`0.00s`** (PTX disk cache; full server load `1.43s`) | cached on disk |
| **Local-memory spills (`ptxas -v sm_89`)** | — | **`0 B` stack frame / `0` spill loads & stores** across all 47 kernels | zero spills |

---

## What Changed

### 1. Cross-Platform PTX Disk Cache ([`psionic/platform/cuda-host.c`](../platform/cuda-host.c))
- Added a 128-bit content-hash + byte-length + `sm_XY` + driver-version keyed PTX disk cache under `~/.openagents/cache/psionic` (POSIX) and `%LOCALAPPDATA%\.openagents\cache\psionic` (Windows), written atomically via temp file + rename (`MoveFileExA` on Win32).
- Controlled via `PSIONIC_PTX_CACHE` (override directory) and `PSIONIC_NO_PTX_CACHE=1` (bypass).
- Measured on RTX 4080 (`--test-quant-kernels`): cold compile **`3.77s`** → warm cache load **`0.00s`**.
- Added thread-local primary-context binding (`pc_ensure_ctx` using `__thread` on GCC/Clang/MinGW and `__declspec(thread)` on MSVC) to all CUDA Graph and `PcProgram` APIs so multi-threaded servers bind `g.context` automatically on worker pthreads.

### 2. Zero-Spill Tensor Core Prefill Kernels ([`psionic/lib/inkling/kernels-cuda.coax`](../lib/inkling/kernels-cuda.coax), [`psionic/platform/inkling-cuda.c`](../platform/inkling-cuda.c))
- **Root-cause of initial 222 tok/s plateau:** Inspecting NVRTC PTX with `ptxas --gpu-name=sm_89 -v` exposed `192 B` (`ink_q8_mma`) and `80 B` (`ink_moe_*_mma`) local-memory spills caused by indexing per-lane fragment arrays with runtime loop variables inside branchy `for lane: 0..<32` blocks.
- **Zero-spill refactor:** Unrolled per-lane fragments into scalar arrays (`a0..a3`) with constant offsets (`lane*4 + 0..3`) and straight-line `for lane` blocks before `cu_mma_bf16_16x8x16`. Verified **`0 bytes stack frame, 0 bytes spill stores, 0 bytes spill loads`** across every kernel in the module:
  - `ink_proj_mma` (`16×32`, 90 regs, 0 spills): fuses all 6–8 attention Q8\_0 projections into a single warp-MMA launch via `proj_desc` word 9 (`rtiles_all`).
  - `ink_q8_mma` (`16×16`, 56 regs, 0 spills): standalone Q8\_0 Tensor Core GEMM (`attn_output`).
  - `ink_moe_gu_iq4_mma`, `ink_moe_gu_q8_mma`, `ink_moe_down_iq4_mma`, `ink_moe_down_q8_mma` (56 regs each, 0 spills): split by quant format, `16` output rows × up to `16` tokens/task, branchless `ink_iq4_bf16`, hoisted loads before dequantization, with two-pass streaming (`dispatch_moe_mma`: Pass 1 executes all in-cache expert hits, Pass 2+ streams misses in chunks of at most `c->cap` experts without cache thrashing).
  - `ink_moe_reduce_post` (40 regs, 0 spills): hoisted 8-way reduction + 32-bit wrapping indices (`0.086 ms → 0.031 ms/step`).
  - `ink_ctrl_pack_bf16` (38 regs), `ink_ctrl_gate_mma` (40 regs, 0 spills), `ink_ctrl_out_mma` (45 regs, 0 spills): Tensor Core batched cross-pass recurrence control gate (`0.322 ms → 0.078 ms/step`).

### 3. Online-Softmax GQA Prefill Attention & `INK_MAX_BATCH = 128`
- Implemented `ink_attn_online` (`INK_OA = 256` threads, `13.9 KB` smem, `0` spills) processing up to 8 Q vectors sharing 1 KV head (`q_in_g = min(gqa, 8)`, `t_step = 8 / q_in_g`), reusing each 8-key K/V tile across the group, handling exact sink logit `0` via initial `m = 0, s = 1`, and eliminating the `attn_scores` DRAM buffer during prefill.
- Kept single-token `K_ATTN` for decode (`np == 1`, sizing `attn_scores` for 1 token only = `256 KB` at 4K / `2 MB` at 32K) and tightened `scratch_bytes` (`86 MB` at 4K, `89 MB` at 32K) so RTX 4080 retains **`114/128`** cached experts per layer at 4K and **`106/128`** at 32K.
- Raised `INK_MAX_BATCH` from `32` to `128` (`128` on >= 12 GiB free, `64` on 8–12 GiB, `16` below 8 GiB).

### 4. CUDA Graph Decode (`dev_pos` + Resident & Segmented Streaming Graphs)
- Added trailing `pos_buf:[]u32` parameter to `ink_clip`, `ink_kv1`, `ink_kv_store`, and `ink_attn` (`epos := ifx pos < 0 then cast(int)pos_buf[0] else pos`) so captured graphs stay invariant across token positions with only a 4-byte `dev_pos` upload per decode step.
- Split `dispatch_layer` into `dispatch_layer_pre` (attention through router) and `dispatch_layer_moe` (MoE tail).
- **Resident GPUs (`!s->streaming`, 24 GB+):** `decode_graph[2]` captures the entire single-token step (`K_EMBED_Q8` through `K_SOFTCAP`) into one graph.
- **Streaming GPUs (`s->streaming`, e.g. RTX 4080 16 GB):** `stream_seg[INK_MAX_SLOTS]` + `stream_tail[2]` capture every device launch between consecutive `stream_experts` host syncs, collapsing ~750 per-token kernel launches into 49 graph replays (48 layer segments + 1 tail).
- Measured on RTX 4080: 128-token CLI decode improved from `75.49 tok/s` (`INK_NO_GRAPH=1`) to **`78.19 tok/s`** with **bit-for-bit identical logits** (`max_abs=0.0000e+00`, `cos=1.000000000`). Escape hatch: `INK_NO_GRAPH=1`.

---

## Critical Bugs Caught & Fixed During Verification

1. **Non-dividing GQA write-write race in `ink_attn_online`:**
   - **Symptom:** Two identical prefill runs produced slightly different logits (`max_abs ~ 1.1`) for `batch >= 3`, while `batch = 1` and `batch = 2` were bit-identical.
   - **Root cause:** When `gqa` does not divide `8` (Inkling 2 Mini has `gqa = 3`, giving `q_in_g = 3` and `t_step = 8 / 3 = 2`), lanes `6..7` computed `dt = thread / 3 = 2 = t_step`, which passed `tok < np` for `np >= 3` and caused CTA `k` to compute `tok = tok0 + 2` (with an incomplete key range capped at `tok0 + 1`) and race with CTA `k + 1` writing `out[tok0 + 2, qh=0..1]`.
   - **Fix:** Added `dt < t_step` guard in all three initialization branches of `ink_attn_online` and upgraded the Coax `#test` to exercise `gqa = 3, np = 3` with reverse-order CTA execution (`itm = 1, 0`).
   - **Result:** 1,219-token prefill Run A vs Run B is now **bit-for-bit identical** (`cos=1.000000000`, `max_abs=0.0000e+00`), and `--verify-prefill-decode` tightened from `cos=0.999868, top1_diff=0.083` to **`cos=0.999957, top1_diff=0.01818`**.

2. **Worker-thread CUDA context failure in `psionic-inkling-serve` under CUDA Graphs:**
   - **Symptom:** `psionic-inkling-cli` succeeded on the main thread, but `psionic-inkling-serve --ctx 32768` logged `cuGraphInstantiate failed: CUDA error 201 (invalid device context)` on HTTP worker pthreads.
   - **Root cause:** `cuGraphInstantiate` does not accept a `CUstream` and requires a current `CUcontext` on the calling host thread; HTTP requests in `inkling-serve.c` execute on a different thread than `ink_open`.
   - **Fix:** Added zero-overhead TLS-cached `pc_ensure_ctx()` (`cuCtxSetCurrent(g.context)`) to `cuda-host.c` graph/program entry points (using `_MSC_VER` guard so MinGW GCC compiles with zero warnings).
   - **Result:** `psionic-inkling-serve --ctx 32768` now serves both `/v1/chat/completions` (`74.3 tok/s` cold) and `/v1/responses` (`83.1 tok/s` warm with `23/23` prefix-cached tokens) cleanly with CUDA Graphs active.

---

## Complete Verification Matrix

| Gate | Command / Setup | Result |
| --- | --- | --- |
| Coax kernel unit tests | `coax test psionic/lib/inkling/kernels-cuda.coax --library` | **8/8 passed** (0 warnings, 0 `E0400` notes) |
| Coax VRAM planner tests | `coax test psionic/lib/inkling-budget.coax --library` | **6/6 passed** |
| Coax MMA authority tests | `coax test coax/lib/compute/mma.coax --library` | **4/4 passed** |
| Quant kernel parity (GPU) | `psionic-inkling-cli --test-quant-kernels` | `nvfp4=0.00e+00 mxfp8=0.00e+00 iq4=0.00e+00 ok=true` |
| PTX spill audit (`sm_89`) | `ptxas --gpu-name=sm_89 -v` on cached `.ptx` | **0 spills** across all 47 entry functions |
| Prefill vs decode parity | `psionic-inkling-cli --verify-prefill-decode` | `cos=0.999957 top1_diff=0.01818 max_abs=0.625 ok=true` |
| CUDA Graph vs non-graph decode | 128-tok greedy decode, default vs `INK_NO_GRAPH=1` | **`cos=1.000000000 max_abs=0.0000e+00 identical=True`** (`78.19` vs `75.49 tok/s`) |
| Long prefill determinism | 1,219-tok prompt, Run A vs Run B | **`cos=1.000000000 max_abs=0.0000e+00 identical=True`** (`632.7 tok/s`) |
| MMA vs non-MMA prefill parity | 900-tok prompt (`634.3` vs `156.2 tok/s`) | `cos=0.999777`, exact top-1 (`200001`) and exact top-5 match |
| 16 GB vs 8 GB VRAM parity | 1,219-tok prompt, default vs `INK_VRAM_MB=8192` | **`cos=1.000000000 max_abs=0.0000e+00 identical=True`** (`93.1 tok/s` @ 8 GB) |
| Low-VRAM refusal | `INK_VRAM_MB=3000` | Clean refusal before allocation (`needs 3845 MB`, `max_ctx=0`) |
| Live HTTP server (`--ctx 32768`) | `/v1/chat/completions` + `/v1/responses` (`"What is 17 * 23?"`) | Both return `"17 * 23 = 391"`, `cached_tokens=23` in `0.001s`, `74.3` / `83.1 tok/s` |
| Windows cross-compile | `TARGET=windows CC=x86_64-w64-mingw32-gcc inkling-cuda-build.sh` | Clean 0-warning build (`cli.exe` 200 KB, `serve.exe` 295 KB) |
| macOS Metal build | `psionic/platform/inkling-build.sh` | Clean build (no regression on shared engine/server/CLI) |

---

## Remaining Hardware-Gated Items for #13966

1. **24 GB+ resident GPU run (RTX 3090 / 4090 / 5090):** Exercises the already-wired single-graph `decode_graph[2]` path with all 128 experts resident per layer (`0` host syncs per decode token).
2. **Native Windows NVIDIA execution:** Run the MinGW-built `psionic-inkling-cli.exe` and `psionic-inkling-serve.exe` on a Windows NVIDIA machine to verify Win32 `LoadLibraryExA`, `CreateFileMappingA`, PTX disk cache under `%LOCALAPPDATA%`, and Winsock HTTP serving end-to-end.
3. **Desktop Tier 2 & 27-task Session History evals:** Run through OpenAgents Desktop once the Linux/Windows desktop local model host integration (Track B3/B4) points at `psionic-inkling-serve`.
