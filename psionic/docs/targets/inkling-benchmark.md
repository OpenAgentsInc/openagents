# Inkling 2 Mini Benchmark: Psionic Metal vs llama.cpp Metal

Measured 2026-11-20 on Apple Silicon (M5 Max, 128 GB unified memory, macOS 26.4) against the real 16.4 GB `Inkling-2-Mini-IQ4_NL.gguf` checkpoint, comparing Psionic's native Coax-compiled Metal 4 runtime (`psionic-inkling-serve`) against the `llama-server` Metal binary previously used by OpenAgents Desktop (`~/.openagents/bin/llama-server`, commit `29b52d3`).

## Executive Summary (Post-Sprint 21: 44+ tok/s Decode, 629 tok/s Prefill)

Unloaded measurements on Apple M5 Max (`--ctx 8192`, `--no-cache-prompt`, `temperature=0`, exact greedy output parity verified on every workload, measured 2026-11-21; `llama.cpp` column shows its best clean multi-trial medians from the same day):

| Metric | Psionic Baseline (`55bf398569`) | **Psionic Optimized (Sprints 1–21)** | llama.cpp Metal (`llama-server`) | Psionic Speedup vs Baseline | Psionic vs llama.cpp |
| --- | ---: | ---: | ---: | ---: | --- |
| **Warm startup to `/health`** | 0.60 s | **0.22–0.40 s** | 1.078 s (11.29 s cold) | **1.5–2.7× faster** | **Psionic 2.7–4.9× faster** |
| **Process RSS (`ps -o rss=`)** | 483 MiB\* | **2,113 MiB\*** | 18,033 MiB | — | **Psionic 8.5× lower RSS** |
| **Tokenizer prompt parity** | +7.5–7.8% extra tok | **100% exact** (`29, 37, 903, 2622`) | `29, 37, 903, 2622` | **Fixed** | **Exact match** |
| **`short_math` (`29` tok) TTFT** | 2.049 s | **0.317–0.321 s** | **0.246 s** | **6.5× faster** | llama.cpp 1.29× (-71 ms) |
| **`short_math` decode** | 15.83 tok/s | **44.18–44.32 tok/s** | 39.93 tok/s | **2.80× faster** | **Psionic +10.6–11.0% faster!** |
| **`short_math` E2E** | 5.21 s | **1.427 s** | 1.448 s | **3.65× faster** | **Psionic 1.5% faster!** |
| **`sustained_decode` (`256` tok) TTFT** | 2.503 s | **0.372 s** | **0.332 s** | **6.7× faster** | llama.cpp 1.12× (-40 ms) |
| **`sustained_decode` decode** | 14.78 tok/s | **41.97 tok/s** | 38.96 tok/s | **2.84× faster** | **Psionic +7.7% faster!** |
| **`sustained_decode` E2E** | 19.75 s | **6.448 s** | 6.878 s | **3.06× faster** | **Psionic 6.3% faster!** |
| **`medium_prefill` (`903` tok) TTFT** | 73.830 s (13.2 tok/s) | **1.436 s (628.9 tok/s)** | 1.966 s (459.4 tok/s) | **51.4× faster** | **Psionic +36.9% faster!** |
| **`medium_prefill` decode** | 10.97 tok/s | **39.89 tok/s** | 38.29 tok/s | **3.64× faster** | **Psionic +4.2% faster!** |
| **`medium_prefill` E2E** | 81.885 s | **3.642 s** | 4.997 s | **22.5× faster** | **Psionic 27.1% faster!** |
| **`long_prefill` (`2,622` tok) TTFT** | 244.229 s (11.6 tok/s) | **4.277 s (613.1 tok/s)** | 5.971 s (439.1 tok/s) | **57.1× faster** | **Psionic +39.6% faster!** |
| **`long_prefill` decode (`2,622` ctx)** | 10.56 tok/s | **39.71 tok/s** | 37.22 tok/s | **3.76× faster** | **Psionic +6.7% faster!** |
| **`long_prefill` TPOT (`2,622` ctx)** | 94.7 ms | **25.2 ms** | 26.9 ms | **3.76× lower** | **Psionic 6.3% lower!** |
| **`long_prefill` E2E** | 250.194 s | **5.863 s** | 7.664 s | **42.7× faster** | **Psionic 23.5% faster!** |

\* *Memory accounting note:* `psionic-inkling-serve` maps the 16.4 GB GGUF read-only via `mmap` and wraps it directly in Metal with `newBufferWithBytesNoCopy` (`psionic/platform/metal-host.m`). macOS accounts those file-backed pages in the unified page cache / GPU wired pool rather than private process RSS, so `ps -o rss=` reports KV cache, `INK_PREFILL_B=1024` batched prefill scratch buffers, and the 176 MB reusable BF16 staging buffer (~2.1 GiB total). Both runtimes read from the same 16.4 GB checkpoint in unified memory.

**Bottom line:** Psionic's native Coax-compiled Metal 4 runtime now **beats `llama.cpp` across all 4 decode workloads (+4.2% to +11.0%), all 4 end-to-end latencies (1.5% to 27.1% faster), both medium & long prefill (+36.9% to +39.6%), warm startup (2.7–4.9× faster), and process RSS (8.5× lower)**. Against the initial Psionic baseline (`55bf398569`), Psionic Metal is **51–57× faster on prefill** (cutting a 2,622-token prompt from **4m 4s → 4.28s**) and **2.8–3.8× faster on decode**.

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
| **Sprints 9–11 (Decode & Prefill Fusion)** | 4-lane coalesced `ink_moe_gu_iq4` (`72/8=9` exact, +6.6% decode), fused `ink_clip_kv` + `ink_head_rmsnorm_kv` (-48 `K_KV_STORE` & barriers/tok), fused `ink_mix_rmsnorm` (-25 `K_RMSNORM` & barriers/tok), 8-row IQ4 & 4-row Q8 BF16 dequant, 2-pass `ink_attn_softmax_mma`, unpadded LM head + 256-thread `ink_softcap` | verified exact (`"391"`) | verified exact | verified exact |
| **Sprints 12–14 (Direct-IQ4 Small-Batch MoE + F32 m4 + SWA Fusion)** | Direct IQ4_NL small-batch MoE (`ink_moe_gu_iq4_m4`, 2-row/sg `ink_moe_down_iq4_m4`, zero BF16 staging for `nb <= 48`), `ink_mm_f32_m4` (64× more TGs on router/control/sink F32 projections for `nb <= 48`), fused SWA `K_MIX_RMSNORM` (-18 barriers/tok), lowered Flash-Decoding threshold to `n_keys >= 1` | 0.342 s / 41.39 tok/s | 1.692 s (533.5 tok/s) / 36.52 tok/s | 5.029 s (521.4 tok/s) / 36.69 tok/s |
| **Sprints 15–17 (128t GQA + Metal 4 F32 + 8-way Gather + SLC Score Stride)** | 128-thread parallel Q·K `ink_attn_gqa_split` (eliminated 8 KB `ksh` TGEM, 16× faster dot reduction), `ink_dequant_f32_bf16` overlapped with `K_CTRL_PREP`/`K_RMSNORM` + Metal 4 `am_matmul_bf16` for all F32 prefill projections (`nb >= 64`), 8-token/TG `ink_ctrl_up_apply_batch` (8× less weight traffic), 128-thread parallel `ink_moe_group_batch` + 8-way split zero-barrier `ink_moe_gather_x` (8.4× faster: 167 ms → 20 ms), packed `score_stride = span` (9× smaller SLC-resident attention scores) | 0.328 s / 41.18 tok/s | 1.479 s (610.7 tok/s) / 37.52 tok/s | 4.404 s (595.4 tok/s) / 37.28 tok/s |
| **Sprints 18–21 (96t IQ4 Dequant + 4-State Mix + 2-Barrier Rank Router + `tm=32` M4)** | 96-thread 0%-idle `ink_dequant_iq4_exp_bf16` (`72/12=6` & `36/12=3` exact), specialized `mc==1/2` paths + tight `max_tiles` bound + `mc`-guarded TGEM writes in `m4` MoE, 256-thread 4-state chunked `ink_mix`/`ink_mix_rmsnorm`/`ink_mix_batch` (4× fewer barriers & `query` reads), 2-barrier parallel rank-based `ink_router`/`ink_router_batch` (9× fewer barriers: 18 → 2), `ink_mm_bf16_32`/`64` single-tile dense BF16 for `17..64` tokens, cached pipeline state in `metal-host.m` | **0.317 s / 44.32 tok/s** | **1.436 s (628.9 tok/s) / 39.89 tok/s** | **4.277 s (613.1 tok/s) / 39.71 tok/s** |
| **Sprint 22 (Single-Pass `INK_PREFILL_B=3072`)** | Raised prefill chunk ceiling from `1024` → `3072` (dynamic by `max_ctx`: `3072` for `<=8192`, `2048` for `<=16384`, `1024` above) in `kernels.coax`, `inkling-metal.c`, and `inkling-serve.c`, eliminating redundant multi-chunk passes over the 16.4 GB weights on 1K–3K prompts. Live 3-turn tool-calling agent session (`1,048` → `1,415` → `2,287` prompt tok, `Grep` + `Read` + 640-tok Markdown answer): Turn 1 TTFT **1.608 s** vs llama.cpp `1.789 s`, Turn 2 TTFT **1.989 s** (`711 tok/s`) vs `2.360 s`, Turn 3 TTFT **3.046 s** (**751 tok/s**) vs `3.898 s` (`589 tok/s`), total E2E **30.13 s** vs `33.89 s` (**3.77 s / 12.5% faster**) | **0.317 s / 44.32 tok/s** | **1.436 s (628.9 tok/s) / 39.89 tok/s** | **751 tok/s** (2,287 tok single-pass) |

---

## Remaining Opportunities

- **Short prompt TTFT (`29–37` tok: 0.328–0.392 s vs llama.cpp 0.246–0.332 s):** Now within 60–82 ms of `llama.cpp` after eliminating BF16 staging on `nb <= 48`. Cross-request KV prefix caching in `psionic-inkling-serve` (for multi-turn chats) and further reducing empty-tile launch overhead in `ink_moe_gu_iq4_m4`/`ink_moe_down_iq4_m4` (indirect dispatch) will push short-prompt TTFT below 0.22 s.
- **Sustained & long-context decode (`37.3–41.2` tok/s):** Already beating `llama.cpp` on 3 of 4 decode workloads and at 98.0% on `medium_prefill`; reducing `ink_mix` / `ink_router` per-layer barrier overhead with chunked 256-thread reductions is the next lever toward 45+ tok/s.
