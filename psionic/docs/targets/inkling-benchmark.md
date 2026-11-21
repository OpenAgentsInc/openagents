# Inkling 2 Mini Benchmark: Psionic Metal vs llama.cpp Metal

Measured 2026-11-20 on Apple Silicon against the real 16.4 GB `Inkling-2-Mini-IQ4_NL.gguf` checkpoint, comparing Psionic's native Coax-compiled Metal runtime (`psionic-inkling-serve`, landed in `55bf398569`) against the `llama-server` Metal binary previously used by OpenAgents Desktop (`~/.openagents/bin/llama-server`, commit `29b52d3`).

## Executive Summary

| Metric | Psionic Metal (`psionic-inkling-serve`) | llama.cpp Metal (`llama-server`) | Ratio (llama.cpp / Psionic) |
| --- | ---: | ---: | ---: |
| **Cold startup to `/health`** | **0.60 s** | 11.29 s | **Psionic 18.7× faster** |
| **Short prompt TTFT (~30 tok)** | 2.04 s | **0.23 s** | 8.9× faster |
| **Medium prompt TTFT (~900–970 tok)** | 73.8 s | **1.47 s** | 50.3× faster |
| **Long prompt TTFT (~2,620–2,830 tok)** | 244.2 s | **4.88 s** | 50.0× faster |
| **Prefill throughput (medium/long)** | 11.6–13.2 tok/s | **538–617 tok/s** | 46×–47× faster |
| **Decode throughput (short ctx, ~30–40 tok)** | 14.8–15.9 tok/s | **41.4–41.9 tok/s** | 2.6×–2.8× faster |
| **Decode throughput (long ctx, ~2.6–2.8k tok)** | 10.6 tok/s | **40.0 tok/s** | 3.8× faster |
| **TPOT (short ctx / long ctx)** | 63 ms / 95 ms | **24 ms / 25 ms** | 2.6× / 3.8× lower |
| **Process RSS (`ps -o rss=`)** | 483 MiB\* | 18,015–18,759 MiB | *(see memory note)* |

\* *Memory accounting note:* `psionic-inkling-serve` maps the 16.4 GB GGUF read-only via `mmap` and wraps it directly in Metal with `newBufferWithBytesNoCopy` (`psionic/platform/metal-host.m`). macOS accounts those file-backed pages in the unified page cache / GPU wired pool rather than private process RSS, so `ps -o rss=` reports only KV cache, scratch buffers, and tokenizer metadata (~483 MiB). Both runtimes read from the same 16.4 GB checkpoint in unified memory.

**Bottom line:** Psionic Metal already runs genuine, numerically faithful Inkling 2 Mini inference natively on Coax-compiled Metal kernels with **18.7× faster startup** and exact greedy output parity on short prompts. However, **prefill is ~46× slower** because `ink_prefill` dispatches single-token matvecs in a loop rather than batched GEMM, and **decode is 2.6×–3.8× slower** (with noticeable degradation as context length grows).

---

## Test Environment & Methodology

- **Machine:** Apple M5 Max, 128 GB unified memory, macOS 26.4 (Build `25E246`)
- **Checkpoint:** `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (16.4 GB, `inkling2_loop`, 26.4B total / 3.4B active looped MoE, IQ4_NL + Q8_0)
- **Psionic Metal:** `psionic-inkling-serve` + `inkling-kernels.metal` built via `psionic/platform/inkling-build.sh` from `origin/main` (commit `55bf398569` runtime)
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
  - One server ran at a time (sequentially) to avoid any GPU or memory-bandwidth contention.
  - Each server received one short warmup request (`"Say OK."`, `max_tokens=16`) before timed trials.
  - All timed requests used `POST /v1/chat/completions` with `stream: true`, `stream_options: {"include_usage": true}`, and `temperature: 0.0` (greedy decoding).
  - `--no-cache-prompt` was passed to `llama-server` so every trial measured true uncached prefill and TTFT (matching `psionic-inkling-serve`, which resets KV state per request).
- **Metric definitions (client-observed over SSE):**
  - **TTFT (Time To First Token):** wall-clock time from HTTP request send to first non-empty SSE delta (`reasoning_content` or `content`).
  - **TTFA (Time To First Answer token):** wall-clock time from request send to first non-empty `content` delta (after thinking completes).
  - **Effective Prefill tok/s:** `prompt_tokens / TTFT` (includes prefill + first token sample).
  - **Decode tok/s (steady-state):** `(completion_tokens - 1) / (E2E - TTFT)`.
  - **TPOT (Time Per Output Token):** `(E2E - TTFT) / (completion_tokens - 1)` in ms.
  - **E2E (End-to-End):** wall-clock time from request send until `[DONE]`.

---

## Results by Workload

Values below are means across trials (3 trials for `short_math`, 2 trials each for `sustained_decode`, `medium_prefill`, and `long_prefill`).

### 1. Short Prompt / Short Answer (`short_math`)

Prompt: `"What is 17 * 23? Answer with just the number."` (`max_tokens=128`)

| Metric | Psionic Metal (3 trials) | llama.cpp Metal (3 trials) | Ratio |
| --- | ---: | ---: | ---: |
| Prompt tokens | 30 | 29 | — |
| Completion tokens | 49 | 49 | exact match |
| **TTFT** | 2.049 s (2.036–2.074 s) | **0.242 s** (0.230–0.267 s) | 8.5× faster |
| **TTFA (first `content` token)** | 4.947 s | **1.341 s** | 3.7× faster |
| **Effective prefill tok/s** | 14.6 tok/s | **120.3 tok/s** | 8.2× faster |
| **Decode tok/s (steady-state)** | 15.83 tok/s (15.69–15.93) | **41.82 tok/s** (41.45–42.09) | 2.6× faster |
| **TPOT** | 63.2 ms | **23.9 ms** | 2.6× lower |
| **End-to-end latency** | 5.081 s | **1.390 s** | 3.7× faster |
| **Server-reported prefill / eval** | 1.92 s / 15.5 tok/s | 0.187 s (157 tok/s) / 39.9 tok/s | — |
| **Output (`content`)** | `"391"` (124-char reasoning) | `"391"` (124-char reasoning) | exact parity |

### 2. Short Prompt / Sustained 256-Token Decode (`sustained_decode`)

Prompt: `"Explain how a hash table resolves collisions with chaining versus open addressing. Be clear and concise."` (`max_tokens=256`)

| Metric | Psionic Metal (2 trials) | llama.cpp Metal (2 trials) | Ratio |
| --- | ---: | ---: | ---: |
| Prompt tokens | 37 | 37 | exact match |
| Completion tokens | 256 | 256 | hit cap |
| **TTFT** | 2.503 s (2.494–2.512 s) | **0.379 s** (0.296–0.461 s) | 6.6× faster |
| **Effective prefill tok/s** | 14.8 tok/s | **102.6 tok/s** | 6.9× faster |
| **Decode tok/s (steady-state)** | 14.78 tok/s (14.77–14.79) | **41.43 tok/s** (41.22–41.64) | 2.8× faster |
| **TPOT** | 67.6 ms | **24.2 ms** | 2.8× lower |
| **End-to-end latency** | 19.751 s | **6.534 s** | 3.0× faster |
| **Server-reported prefill / eval** | 2.37 s / 14.7 tok/s | 0.324 s (123 tok/s) / 41.1 tok/s | — |

### 3. Medium Prompt Prefill + Decode (`medium_prefill`)

Prompt: 4× repeated Python `merge_intervals` + `binary_search_left` reference block + complexity question (`max_tokens=128`).

| Metric | Psionic Metal (2 trials) | llama.cpp Metal (2 trials) | Ratio |
| --- | ---: | ---: | ---: |
| Prompt tokens | 971 | 903 | +7.5% in Psionic |
| Completion tokens | 89 | 117 | — |
| **TTFT** | 73.830 s (72.670–74.989 s) | **1.466 s** (1.409–1.523 s) | 50.4× faster |
| **TTFA (first `content` token)** | 80.458 s | — | — |
| **Effective prefill tok/s** | 13.2 tok/s | **617.0 tok/s** | 46.9× faster |
| **Decode tok/s (steady-state)** | 10.97 tok/s (10.29–11.65) | **40.52 tok/s** (40.31–40.73) | 3.7× faster |
| **TPOT** | 91.5 ms | **24.7 ms** | 3.7× lower |
| **End-to-end latency** | 81.885 s | **4.329 s** | 18.9× faster |
| **Server-reported prefill / eval** | 73.66 s / 10.9 tok/s | 1.391 s (650 tok/s) / 39.5 tok/s | — |
| **Output (`content`)** | `"O(n log n), dominated by sorting the intervals."` | `"O(n log n), dominated by sorting."` | both correct |

### 4. Long Prompt Prefill + Decode (`long_prefill`)

Prompt: 12× repeated Python reference block + `binary_search_left` edge-case question (`max_tokens=64`).

| Metric | Psionic Metal (2 trials) | llama.cpp Metal (2 trials) | Ratio |
| --- | ---: | ---: | ---: |
| Prompt tokens | 2,826 | 2,622 | +7.8% in Psionic |
| Completion tokens | 64 | 64 | hit cap in thinking |
| **TTFT** | 244.229 s (241.408–247.049 s) | **4.880 s** (4.783–4.977 s) | 50.0× faster |
| **Effective prefill tok/s** | 11.6 tok/s | **537.6 tok/s** | 46.5× faster |
| **Decode tok/s (steady-state)** | 10.56 tok/s (10.54–10.58) | **39.98 tok/s** (39.94–40.02) | 3.8× faster |
| **TPOT** | 94.7 ms | **25.0 ms** | 3.8× lower |
| **End-to-end latency** | 250.194 s | **6.456 s** | 38.8× faster |
| **Server-reported prefill / eval** | 244.03 s / 10.4 tok/s | 4.800 s (546 tok/s) / 38.2 tok/s | — |

---

## Analysis of Gaps & Optimization Roadmap

### 1. Prefill is the biggest bottleneck (~46× gap)

In `psionic/platform/inkling-metal.c`, `ink_prefill()` batches multiple tokens into a single Metal command buffer, but still calls `enqueue_step()` once per token:

- Each prompt token runs single-vector matvecs (`ink_dv_iq4`, `ink_dv_q8`, `ink_moe_gu_iq4`, `ink_moe_down_iq4`), streaming active weights from memory once **per prompt token** instead of amortizing weight reads across a token tile (`B = 32` or `64`).
- Consequently, prefill stays memory-bandwidth-bound at **11.6–14.8 tok/s** (identical to single-token decode) instead of becoming compute-bound like llama.cpp's batched Metal GEMM (**538–650 tok/s**).
- **Impact:** A 2,800-token coding prompt takes **~4 minutes** to reach first token on Psionic vs **4.9 seconds** on llama.cpp.
- **Fix:** Add batched matmul (`M × K × N` tiled GEMM) kernels in `psionic/lib/inkling/kernels.coax` for Q8_0 / IQ4_NL dense and MoE projections, plus batched causal attention prefill.

### 2. Decode throughput & context-length scaling (2.6×–3.8× gap)

- At short context (~30–40 tokens), Psionic decodes at **14.8–15.9 tok/s** vs llama.cpp's **41.4–41.9 tok/s** (~2.6× gap). That reflects kernel-level memory-bandwidth utilization in the Coax-emitted `ink_dv_iq4` / `ink_moe_*` matvecs compared to llama.cpp's simdgroup-vectorized Metal matvec kernels.
- At longer context (~971–2,826 tokens), Psionic decode drops from **15.8 → 11.0 → 10.6 tok/s** (+31 ms/tok), whereas llama.cpp stays nearly flat (**41.8 → 40.5 → 40.0 tok/s**, +1 ms/tok) using Flash Attention. Optimizing `ink_attn` (tiled online softmax / parallel KV-head reduction across sequence positions) will eliminate the context-length decode slowdown.

### 3. Cross-request KV prefix caching

- `psionic-inkling-serve` currently resets the KV cache on every request. In multi-turn agentic coding chats (where system prompt + tool schemas + prior turns are shared), prefix KV reuse avoids re-prefilling thousands of tokens on every turn. (`llama-server` includes LCP prefix caching by default; it was disabled here via `--no-cache-prompt` for an apples-to-apples uncached comparison.)

### 4. BPE tokenizer parity on code/whitespace

- On `short_math` and `sustained_decode`, Psionic's tokenizer matches llama.cpp within 0–1 tokens (30 vs 29; 37 vs 37) and produces identical greedy output on `short_math`.
- On code-heavy prompts (`medium_prefill`, `long_prefill`), Psionic's BPE tokenizer in `psionic/platform/inkling-gguf.c` emits **7.5–7.8% more tokens** (`971` vs `903`; `2,826` vs `2,622`), likely due to pre-tokenization differences on repeated indentation spaces. Aligning the pre-tokenizer regex with the GGUF tokenizer specification will cut prompt token count and ensure exact prompt-token parity on code.

### 5. Where Psionic already wins

- **Startup latency:** **0.60 s** vs **11.29 s** (18.7× faster) thanks to zero-copy `mmap` + `newBufferWithBytesNoCopy` without eager weight copying.
- **Zero third-party runtime dependency:** Pure Coax compute kernels (`kernels.coax` → `coax metal-compute` → `inkling-kernels.metal`) plus a small self-contained C/Objective-C Metal host, bundled directly into `OpenAgents.app`.
- **Numerical fidelity:** Exact token-for-token greedy match with llama.cpp on `short_math` (49 tokens, identical 124-character reasoning trace and final answer `"391"`).
