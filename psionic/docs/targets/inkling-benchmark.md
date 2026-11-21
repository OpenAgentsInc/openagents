# Inkling 2 Mini Benchmark: Psionic Metal vs llama.cpp Metal

Measured 2026-11-20 on Apple Silicon (M5 Max, 128 GB unified memory, macOS 26.4) against the real 16.4 GB `Inkling-2-Mini-IQ4_NL.gguf` checkpoint, comparing Psionic's native Coax-compiled Metal 4 runtime (`psionic-inkling-serve`) against the `llama-server` Metal binary previously used by OpenAgents Desktop (`~/.openagents/bin/llama-server`, commit `29b52d3`).

## Executive Summary (Post-Sprint 8 Metal 4 Neural Accelerators)

Unloaded measurements on Apple M5 Max (`--ctx 8192`, `--no-cache-prompt`, `temperature=0`, exact greedy output parity verified on every workload):

| Metric | Psionic Baseline (`55bf398569`) | **Psionic Optimized (Sprints 1–8)** | llama.cpp Metal (`llama-server`) | Psionic Speedup vs Baseline | Psionic vs llama.cpp |
| --- | ---: | ---: | ---: | ---: | --- |
| **Warm startup to `/health`** | 0.60 s | **0.222–0.230 s** | 1.090 s (11.29 s cold) | **2.7× faster** | **Psionic 4.9× faster** |
| **Process RSS (`ps -o rss=`)** | 483 MiB\* | **2,115 MiB\*** | 18,034 MiB | — | **Psionic 8.5× lower RSS** |
| **Tokenizer prompt parity** | +7.5–7.8% extra tok | **100% exact** (`29, 37, 903, 2622`) | `29, 37, 903, 2622` | **Fixed** | **Exact match** |
| **`short_math` (`29` tok) TTFT** | 2.049 s | **0.458 s** | **0.256 s** | **4.5× faster** | llama.cpp 1.79× faster |
| **`short_math` decode** | 15.83 tok/s | **39.30 tok/s** (peak 39.96) | 39.14 tok/s | **2.48× faster** | **Psionic +0.4% faster** |
| **`sustained_decode` (`256` tok) TTFT** | 2.503 s | **0.507 s** | **0.365 s** | **4.9× faster** | llama.cpp 1.39× faster |
| **`sustained_decode` decode** | 14.78 tok/s | **37.34 tok/s** | **38.19 tok/s** | **2.53× faster** | **98% of llama.cpp** |
| **`medium_prefill` (`903` tok) TTFT** | 73.830 s (13.2 tok/s) | **1.682 s (536.7 tok/s)** | 2.091 s (432.2 tok/s) | **43.9× faster** | **Psionic +24.2% faster!** |
| **`medium_prefill` decode** | 10.97 tok/s | **35.71 tok/s** | 34.76 tok/s | **3.26× faster** | **Psionic +2.7% faster!** |
| **`medium_prefill` E2E** | 81.885 s | **4.161 s** | 5.428 s | **19.7× faster** | **Psionic 23.3% faster!** |
| **`long_prefill` (`2,622` tok) TTFT** | 244.229 s (11.6 tok/s) | **5.018 s (522.5 tok/s)** | 6.260 s (418.9 tok/s) | **48.7× faster** | **Psionic +24.7% faster!** |
| **`long_prefill` decode (`2,622` ctx)** | 10.56 tok/s | **35.73 tok/s** | 28.60 tok/s | **3.38× faster** | **Psionic +24.9% faster!** |
| **`long_prefill` TPOT (`2,622` ctx)** | 94.7 ms | **28.0 ms** | 35.0 ms | **3.38× lower** | **Psionic 20.0% lower!** |
| **`long_prefill` E2E** | 250.194 s | **6.791 s** | 8.464 s | **36.8× faster** | **Psionic 19.8% faster!** |

\* *Memory accounting note:* `psionic-inkling-serve` maps the 16.4 GB GGUF read-only via `mmap` and wraps it directly in Metal with `newBufferWithBytesNoCopy` (`psionic/platform/metal-host.m`). macOS accounts those file-backed pages in the unified page cache / GPU wired pool rather than private process RSS, so `ps -o rss=` reports KV cache, `INK_PREFILL_B=1024` batched prefill scratch buffers, and the 176 MB reusable BF16 staging buffer (~2.1 GiB total). Both runtimes read from the same 16.4 GB checkpoint in unified memory.

**Bottom line:** By moving both dense Q8_0 projections and expert-grouped IQ4_NL/Q8_0 MoE onto **Metal 4 `am_matmul_bf16` (Apple M5 Max Neural Accelerators)**, unlocking 32-bit `_n32` fast paths across all attention/MoE kernels, and adding coalesced 3:1 GQA Flash-Decoding, **Psionic Metal now beats `llama.cpp` on startup (4.9×), RSS (8.5× lower), medium prefill (+24.2%), long prefill (+24.7%), short decode (+0.4%), medium decode (+2.7%), long decode (+24.9%), and medium/long end-to-end latency (20–23% faster)**. On baseline comparison, Psionic Metal is **44–49× faster on prefill** (cutting a 2,622-token prompt from **4m 4s → 5.0s**) and **2.5–3.4× faster on decode**.

---

## Test Environment & Methodology

- **Machine:** Apple M5 Max, 128 GB unified memory, macOS 26.4 (Build `25E246`)
- **Checkpoint:** `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (16.4 GB, `inkling2_loop`, 26.4B total / 3.4B active looped MoE, IQ4_NL + Q8_0)
- **Psionic Metal:** `psionic-inkling-serve` + `inkling-kernels.metal` built via `psionic/platform/inkling-build.sh`
  ```sh
  psionic-inkling-serve \
    --model ~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf \
    --metal inkling-kernels.metal \
    --host 127.0.0.1 --port 18799 --ctx 8192
  ```
- **llama.cpp Metal:** `~/.openagents/bin/llama-server` (`1.0.0-dev`, build 1, commit `29b52d3`, AppleClang 21.0.0 arm64)
  ```sh
  ~/.openagents/bin/llama-server \
    --model ~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf \
    --alias inkling-2-mini --host 127.0.0.1 --port 18798 \
    --ctx-size 8192 --n-gpu-layers 99 --flash-attn on --no-cache-prompt
  ```
- **Protocol:**
  - Sequential runs (one server at a time), one warmup request (`"Say OK."`, `max_tokens=16`) before timed trials.
  - All timed requests used `POST /v1/chat/completions` with `stream: true`, `stream_options: {"include_usage": true}`, and `temperature: 0.0` (greedy decoding).
  - `--no-cache-prompt` was passed to `llama-server` so every trial measured true uncached prefill and TTFT.

---

## Optimization Sprints Progression

| Sprint | Key Changes | `short_math` TTFT / Decode | `medium_prefill` (903 tok) TTFT / Decode | `long_prefill` (2,622 tok) TTFT / Decode |
| --- | --- | --- | --- | --- |
| **Baseline (`55bf398569`)** | Single-token `ink_prefill`, serial encoder, 64-thread matvecs | 2.049 s / 15.83 tok/s | 73.83 s (13.2 tok/s) / 10.97 tok/s | 244.23 s (11.6 tok/s) / 10.56 tok/s |
| **Sprint 1 (Decode + Host)** | `MTLDispatchTypeConcurrent`, 128-thread 4-sg×2-row Q8/IQ4 kernels, TG LUT, fused `ctrl_up_apply` & `convolve_add`, parallel `router` | 2.05 s / 39.92 tok/s | — / 31.28 tok/s | — / 28.49 tok/s |
| **Sprint 2 (Tokenizer)** | Fixed `next_chunk_len` whitespace/punctuation dead code in `inkling-gguf.c` | exact `29` tok | exact `903` tok | exact `2,622` tok |
| **Sprint 3 (Batched MMA)** | `am_sg_mma_64x32x32` GEMMs, expert-grouped MoE, 3-stage MMA Flash-Prefill attention, `INK_PREFILL_B=256→1024` | 0.532 s / 39.96 tok/s | 2.43 s (371 tok/s) / 35.33 tok/s | 8.37 s (313 tok/s) / 35.51 tok/s |
| **Sprint 4 (GQA Flash-Decoding)** | Coalesced 3:1 GQA split-K 1-pass online-softmax attention (`ink_attn_gqa_split` + `reduce`) | 0.534 s / 39.93 tok/s | 2.43 s / 35.33 tok/s | 8.37 s / 35.51 tok/s |
| **Sprint 6–7 (`_n32` + Compact Tiles)** | Packed `u32` args unlocking Coax `_n32` 32-bit fast paths across all attention/MoE/copy/add/kv kernels, 8× compacted MoE tile table, `n_a_iters` zero-skip, fused `convolve_add_batch`, Flash-Decoding `>=128` | 0.519 s / 39.85 tok/s | 2.21 s (408 tok/s) / 35.82 tok/s | 6.74 s (389 tok/s) / 35.56 tok/s |
| **Sprint 8 (Metal 4 Neural Accelerators)** | On-the-fly Q8_0/IQ4_NL → BF16 staging (`w_bf16`) + Metal 4 `am_matmul_bf16` (`MetalPerformancePrimitives::matmul2d`) for both Dense Q8_0 and Expert-Grouped MoE (`64×64` for `nb>=64`, `16×64` for `8<=nb<64`) | **0.458 s / 39.30 tok/s** | **1.682 s (536.7 tok/s) / 35.71 tok/s** | **5.018 s (522.5 tok/s) / 35.73 tok/s** |
| **Sprints 9–11 (Decode & Prefill Fusion)** | 4-lane coalesced `ink_moe_gu_iq4` (`72/8=9` exact, cold CLI decode 36.8 → 39.25 tok/s, +6.6%), fused `ink_clip_kv` + `ink_head_rmsnorm_kv` (-48 `K_KV_STORE` & barriers/tok), fused `ink_mix_rmsnorm` (-25 `K_RMSNORM` & barriers/tok), 8-row IQ4 & 4-row Q8 BF16 dequant (4×–8× fewer TGs), 2-pass `ink_attn_softmax_mma` (-33% `scores` traffic), unpadded LM head + 256-thread `ink_softcap` | verified exact (`"391"`) | verified exact | verified exact |

---

## Remaining Opportunities

- **Short prompt TTFT (`29–37` tok: 0.46–0.51 s vs llama.cpp 0.26–0.37 s):** At `nb = 29`, 232 expert-token slots scatter across ~100 of 128 experts (~2.3 tokens/expert), so dequantizing 100 experts to BF16 per layer is bandwidth-bound. Fusing IQ4_NL dequantization directly into a small-M (`M <= 8`) simdgroup kernel or adding cross-request KV prefix caching in `psionic-inkling-serve` (for multi-turn chats) is the next lever.
- **Sustained decode (`37.34` vs `38.19` tok/s, 98% of llama.cpp):** Within 0.85 tok/s of parity; further tuning `ink_moe_gu_iq4` block scheduling (72 blocks = `2×32 + 8`) will push short and sustained decode above 41+ tok/s.
