# Psionic + Inky vs. Generated Computers' Locals: Inkling 2 Mini & Coding-Agent Engineering Audit

**Date:** 2026-11-22  
**Status:** Comparative technical audit  
**Audited Revisions:**
- **OpenAgents (Psionic + Inky):** `origin/main` `f30b1d9aa2` (post-Inky Phase 1 `#13979`, Phase 2 `#13980`, Metal Sprints 1–55+, CUDA Track B `#13966`/`#13977`)
- **Generated Computers / Locals:** `origin/main` `40be249` (`https://github.com/Generated-Computers/Locals`, cloned at `~/work/projects/repos/Locals`)

**Related OpenAgents Docs & Issues:**
- [`docs/coder/inky-spec.md`](../../docs/coder/inky-spec.md) — Inky native Inkling 2.0 harness specification
- [`docs/desktop/openagents-v1-spec.md`](../../docs/desktop/openagents-v1-spec.md) — OpenAgents Desktop v1.0 (Inkling-only minimal release)
- [`psionic/docs/targets/inkling.md`](targets/inkling.md), [`targets/inkling-metal-report.md`](targets/inkling-metal-report.md), [`targets/inkling-benchmark.md`](targets/inkling-benchmark.md), [`inkling-cuda.md`](inkling-cuda.md)
- **Active Issue Chain:** `#13978` (Microcoder 7+ min failure root cause), `#13979` (Inky Phase 1 — closed), `#13980` (Inky Phase 2 — closed), `#13981` (Inky Phase 3 qualification — open), `#13982` (Inky Phase 4 Desktop cutover — open), `#13983` (Inky Phase 5 Microcoder retirement — open), `#13966` (Track B cross-platform CUDA engine — open)

---

## Executive Summary

Both **OpenAgents (Psionic + Inky)** and **Generated Computers' Locals** independently engineered custom Apple Silicon runtimes, serving stacks, and coding-agent harnesses around Thinking Machines' **Inkling 2 Mini** (`inkling2_loop`: 26.4B total / 3.4B active looped MoE, 24 physical layers → 48 logical visits, 5:1 SWA-1024 + shortconv). Because both teams worked from the same model physics in the same week, comparing them side by side exposes exact engineering trade-offs, blind spots, and high-leverage ideas we can adopt without compromising OpenAgents' Coax-native invariants.

### Headline Verdict

| Dimension | OpenAgents (`Psionic + Inky`) | Generated Computers (`Locals`) | Advantage |
| --- | --- | --- | --- |
| **Runtime & packaging** | Zero-dependency native C + Coax (`psionic-inkling-serve` ~200 KB + 1.2 MB kernel source; ~11–16 MB `--minimal` app); 0.22–0.40 s warm start | Python 3.11+ on `mlx>=0.32`, `mlx-lm>=0.32.2`, `numpy`, `outlines-core`, `llguidance` + inline Metal shaders | **Psionic** (shipping footprint, startup, zero Python/vendor runtime) |
| **Platform & GPU portability** | **macOS Metal 4 + Linux CUDA + Windows CUDA** (single `inkling-engine.h` contract, zero cuBLAS/cuDNN) | **macOS Apple Silicon only** (MLX) | **Psionic** (3 OSes, 2 GPU ecosystems) |
| **Checkpoint format & Mac RAM floor** | Re-quantized **`IQ4_NL + Q8_0 + F32` GGUF** (`16.4 GiB`); Metal hard-refuses `< 32 GB` Macs (`inkling-metal.c:1438`); CUDA streams experts on **16 GB / 8 GB VRAM** | **Original upstream safetensors NVFP4** (layers 0–19 bit-exact) + optimal-scale NVFP4 tail (layers 20–23) + 8-bit attn/head (**`15.6 GB` / `16.2 GB` w/ DSpark**) → runs cleanly on **24 GB Macs** | **Locals** on Apple Silicon RAM floor & native NVFP4; **Psionic** on discrete NVIDIA VRAM streaming |
| **Raw single-stream prefill (Apple Silicon)** | **629–792 tok/s** (Sprint 22) → **1,449 tok/s GPU** (Sprint 53, native Metal 4 `bfloat × bfloat` Neural Accelerators) on M5 Max | **300–322 tok/s** on M4 Pro (512-tok chunks, visit-by-visit eval for 0.99 GB peak activation cap) | **Psionic** (native Metal 4 MMA + in-tile IQ4/BF16 kernels) |
| **Raw single-stream decode (no drafts)** | **44–50 tok/s** default (`Q8_0` attn) / **75.01+ tok/s** (`INK_FAST_Q4=1`) on M5 Max; **75.7 tok/s** on 16 GB RTX 4080 | **35–37 tok/s** on M4 Pro (218 GB/s BW, ~29.5 ms vs 21 ms weight floor) | **Psionic** on raw kernel depth & CUDA; **Locals** is near M4 Pro hardware floor with 8-bit attention |
| **Speculative decoding (DSpark + lookup)** | **None** (every output token pays full 48-visit weight stream) | **Full `mtp.safetensors` (DSpark) + adaptive 3-gram prompt-lookup drafts** + `seq` multi-token verification & rewind → **75.6 tok/s** file write (2.05×), **64.8 tok/s** code thinking at 7k ctx | **Locals** (decisive single-stream coding speedup) |
| **Multi-request concurrency** | **None** — single `InkSession` serialized on `sess_mu` (`inkling-serve.c:1618`) | **Continuous batching** (`HybridBatchEngine`, up to 16 slots) + **`inkling_experts.xmm`** shared-expert dedup → **79 tok/s @ 4 reqs, 95 tok/s @ 8 reqs** on M4 Pro | **Locals** (enables parallel subagents & multi-chat) |
| **Prompt caching & persistence** | 2-slot in-memory KV + shortconv snapshots (`snaps[0]` pre-decode + `snaps[1]` post-decode): **91–99.7% cache hit** in active Inky turns; lost on chat switch or restart | Multi-entry `ChainCache` LRU (`1.25 GB` on 24 GB Mac) + `Inkling.restore` raw turn replay + **SSD cold-tier `DiskPromptCache`** (`system` + every-2,048-tok `user` checkpoints → **32k follow-up in 1.0–1.2 s** across restarts) | **Locals** (multi-conversation LRU + SSD persistence); **Psionic** (clean post-decode SWA-wrap-safe slot 1) |
| **Constrained decoding & tool grammars** | Zero grammar on Inky native tools (deliberate); CPU zero-alloc RFC 8259 automaton (`json-schema.coax`) for `response_format` only; `tool_choice` ignored | **GPU-walked zero-table byte DFA** (`walk.py`, `locals_dfa_walk`) with `typed_default=True` sorted Inkling tool schemas + forced spans + DSpark draft compatibility | **Locals** on strict tool grammar + forced spans; **Inky** avoids schema-order distortion |
| **Inkling 2.0 native protocol fidelity** | First-class `<\|content_tool_error\|>` (`200022`), `<\|content_invoke_tool_text\|>` (`200057`), interleaved `<\|content_thinking\|>`, continuous `reasoning_effort`, `/v1/responses`, variable `K=1..16` | `<\|content_thinking\|>`, `<\|content_text\|>`, `<\|content_invoke_tool_json\|>`, continuous effort; **no `<\|content_tool_error\|>`, no `<\|content_invoke_tool_text\|>`, no `/v1/responses`, fixed `K=3`** | **Psionic + Inky** |
| **Server hardening & cancellation** | Wildcard `CORS: *`, no `Host`/`Origin` check, **no client-disconnect abort during decode** (locks `sess_mu` up to 8,192 tokens after Stop/Esc!) | DNS-rebinding `Host` guard, strict loopback/webview `Origin` check, **1 s socket-liveness polling + immediate batch cancellation** during both prefill and decode | **Locals** (concrete P0 security & UX gap in Psionic) |
| **Server API breadth & ecosystem** | `/v1/chat/completions`, `/v1/responses`, `/health`, `/v1/models` | OpenAI Chat + Completions, **Anthropic `/v1/messages` + `count_tokens`**, **Ollama `/api/*`**, `/stats` + live HTML dashboard, `locals launch` for 9 coding tools | **Locals** |
| **Interactive coding-agent UX** | **Inky (`coax/lib/inky/`)**: 7 orthogonal native tools, head-only `read_file`, 16 KB inline `run_command`, 2-stage SWA compaction, `#authority` guards (524/524 mutants killed), mid-turn steers, follow-up queue, `gitguard`, AJR1 journals, zero UI churn (`mcworker-wire`) | No interactive chat UI of its own (delegates interactive UX to OpenCode/Claude Code/Cline/Aider) | **Inky** (purpose-built local Desktop agent) |
| **Autonomous multi-agent task synthesis** | Single-agent `/goal` loop (no parallel version racing inside Inky) | **`locals run`**: file-part planner + `k` temperature-diverse parallel workers + whole-task racers in `sandbox-exec` copies + after-write checks + combinatorial version mixing + blame-guided repair rounds | **Locals** (best-in-class batch-native parallel solver for small local models) |
| **Direction / LoRA adapters** | None wired into `inkling-metal.c` / `inkling-cuda.c` | `convert_inkling --adapter FILE.gguf`: rank-1 direction ablation folded into 8-bit attn/embed + `combine_without` kernel for untouched NVFP4 experts (+0.7 GB symlinked) | **Locals** |

---

## 1. Context: Where OpenAgents (Psionic + Inky) Stands Right Now

Reading the open and recently closed Inky/Inkling issues establishes the exact backdrop for this audit:

1. **Completed Foundation (`#13820–#13825`, `#13979`, `#13980`):**
   - **Psionic Metal 4 (`ch266–ch271` + Sprints 1–55+):** Built an end-to-end native Coax-compiled Metal 4 runtime (`psionic/lib/inkling/kernels.coax` + `psionic/platform/inkling-metal.c`) beating `llama.cpp` across decode, medium/long prefill (reaching **1,449 tok/s GPU prefill** via native Metal 4 `bfloat × bfloat` Neural Accelerators in Sprint 53 and **75.01 tok/s** decode under `INK_FAST_Q4=1`), warm startup, and RSS.
   - **Psionic CUDA Track B (`#13966`, `#13977`, `9041103cb7`):** Extracted the backend-agnostic `inkling-engine.h` contract, added Win32 `CreateFileMappingA` / Winsock2 / `nvcuda.dll` portability, and brought up Coax-emitted CUDA kernels (`kernels-cuda.coax` + `inkling-cuda.c`) with `#authority` VRAM budgeting (`inkling-budget.coax`) and **per-layer host-to-VRAM expert streaming**, achieving **75.7 tok/s decode** on a 16 GB RTX 4080 (vs `llama.cpp` 8.6 tok/s) and bit-identical 42.3 tok/s on an 8 GB VRAM plan.
   - **Root Cause `#13978` (Open — tracks full rollout):** On 2026-11-22, Microcoder on local Inkling 2 Mini looped 7+ minutes across 37 shell commands failing to locate its own system prompt (`MC_SYSTEM`). Forensic AJR analysis showed structural incompatibility between Inkling 2 Mini (a 3.4B-active native tool-calling model) and Microcoder's monolithic single-JSON `next_action` schema (4 KB middle-cut `MC_SPILL_SHOW`, sticky `state.notes`, `MC_EARLY_NUDGE`, duplicate `workmem`, contradictory commit overrides, and greedy `temp=0.0` repetition lock).
   - **Inky Phase 1 (`#13979`, Closed `0fe366840e`):** Upgraded `psionic-inkling-serve` and `coax/lib/gateway/` with post-decode slot-1 KV + shortconv prefix snapshots (verified **91–99.7% cache hit** even after >4K-token thinking steps), native `<|content_tool_error|>` (`200022`) and `<|content_invoke_tool_text|>` (`200057`), `cached_tokens` + `reasoning_tokens` usage telemetry, deterministic `seed`, and a thinking-loop circuit breaker inside `channel == 1`.
   - **Inky Phase 2 (`#13980`, Closed `f30b1d9aa2`):** Built the clean-sheet pure-Coax Inky harness (`coax/lib/inky/{protocol,tools,format,prompt,compact,guard,loop,run,selftest}.coax`) and drop-in `coder/tui/inky.coax` binary speaking `mcworker-wire.coax`, with **396/396** (`tools.coax`) and **128/128** (`guard.coax`) authority mutants killed.

2. **Remaining Open Gates (`#13981`, `#13982`, `#13983`, `#13966`):**
   - **`#13981` (Inky Phase 3):** Head-to-head live qualification of `inky` vs `mcworker` on `psionic-inkling-serve --ctx 32768` across Tier 2 Desktop, Tier 4 multi-language coding, interactive chat / `#13978` self-inspection regression, and 25+ step compaction/recall stress.
   - **`#13982` (Inky Phase 4):** Cut OpenAgents Desktop v1.0 (`--minimal`) over from `mcworker` to `inky` across macOS, Linux, and Windows bundles + Tier 3 live `.app` E2E verification.
   - **`#13983` (Inky Phase 5):** Formally deprecate Microcoder (`coax/lib/microcoder`, `mcworker`) and close `#13978`.
   - **`#13966` (CUDA Track B):** Reach `>= 500 tok/s` CUDA prefill (via `cu_mma_bf16_16x8x16` Tensor Core GEMM), CUDA Graphs, 24 GB fully resident verification, and live Windows verification.

---

## 2. Stack Philosophy, Packaging & Platform Portability

| Aspect | Psionic (`psionic/platform/inkling-*`, `psionic/lib/inkling/`) | Locals (`locals/inkling*.py`, `locals/models/inkling2_loop.py`) |
| --- | --- | --- |
| **Implementation languages** | Coax (`kernels.coax` 4,155 LOC, `kernels-cuda.coax` 1,531 LOC, `json-schema.coax` 961 LOC, `inkling-budget.coax`) + C11 (`inkling-metal.c` 3,123 LOC, `inkling-cuda.c` 1,784 LOC, `inkling-serve.c` 2,406 LOC, `inkling-gguf.c` 757 LOC) + thin Obj-C `metal-host.m` | Python (`inkling2_loop.py` ~820 LOC, `inkling.py` 1,064 LOC, `inkling_experts.py` 151 LOC, `inkling_draft.py` 212 LOC, `convert_inkling.py` 387 LOC) + inline Metal C++ strings compiled via `mx.fast.metal_kernel` |
| **External inference deps** | **Zero.** No MLX, no PyTorch, no `llama.cpp`, no MPS/Accelerate, no cuBLAS/cuDNN, no build-time CUDA SDK (`libcuda`/`nvcuda` loaded dynamically via `dlopen`/`LoadLibraryA`) | `mlx>=0.32`, `mlx-lm>=0.32.2`, `numpy`, `huggingface_hub`, `outlines-core`, `llguidance` (for stock baseline), Python `>=3.11` |
| **Shipped artifact size** | `psionic-inkling-serve` ~200 KB + `inkling-kernels.metal` 1.2 MB (inside ~11–16 MB `--minimal` `OpenAgents.app`) | Python virtualenv + MLX native wheels (~150+ MB) |
| **Startup latency** | **0.22–0.40 s** warm startup (`mmap` read-only + Metal `newBufferWithBytesNoCopy` + compiled kernel cache) | Several seconds (Python import, `mlx-lm` model instantiation, `_Consts` projection stacking, `Drafter.bind` LM-head folding, kernel JIT) |
| **Target platforms** | macOS Apple Silicon (Metal 4), Linux x86_64 NVIDIA (CUDA), Windows x86_64 NVIDIA (CUDA; MinGW cross-compiled, runtime untested) | macOS 14+ Apple Silicon only (M1–M5) |
| **Model scope** | Dedicated `inkling2_loop` engine (v1.0 single-model product) alongside other Psionic targets (GPT-OSS, DeepSeek, Parakeet, Whisper) | Multi-model server (Qwen3.5, Gemma 4, Flint 40B MoE, LFM2.5/LFM3, MiniCPM5, Qwenputer RISC-V, Inkling 2 Mini) |

**Engineering takeaway:** Psionic's zero-dependency native C + Coax architecture is vastly better suited for shipping a signed, self-contained desktop application across macOS, Linux, and Windows. Locals' Python + MLX stack lets a single author prototype 5+ custom architectures in weeks, at the cost of macOS-only lock-in and Python runtime overhead.

---

## 3. Weight Formats, Quantization Fidelity & Hardware Memory Floor

This is one of the sharpest structural divergences between the two implementations.

### 3.1 Side-by-Side Checkpoint Comparison

| Property | Upstream Release (`thinkingmachines/Inkling-2-Mini`) | Locals Build (`locals.convert_inkling`) | Psionic Runtime (`Inkling-2-Mini-IQ4_NL.gguf`) |
| --- | --- | --- | --- |
| **Container** | Safetensors (`model-00001-of-00001.safetensors` + `mtp.safetensors`) | Sharded MLX Safetensors + `model-draft.safetensors` | Single GGUF v3 file (`cbdf3fa9...`) |
| **Disk / weight size** | 19.5 GB (+ 0.64 GB draft) | **15.6 GB** (**16.2 GB** w/ DSpark; 17.5 GB w/ `--tail mxfp8`) | **16.4 GiB (17.6 GB)** (no draft bundled) |
| **MoE experts L0–19** | **Native NVFP4** (E2M1, E4M3 scale/16, F32 global scale/expert) | **Native NVFP4 — bit-for-bit as released** | Re-quantized **`IQ4_NL`** (4-bit nonlinear LUT, block-32, F16 scale) |
| **MoE experts L20–23** | **Native MXFP8** (E4M3, UE8M0 scale/32) | Re-quantized to **NVFP4** via optimal E4M3 scale search (`shifts=-3..3`, **8.6% rel error** vs MLX's 9.5%); optional `--tail mxfp8` | Re-quantized **`IQ4_NL`** (or `Q8_0` on L0/L23 depending on GGUF recipe) |
| **Attention projections (Q/K/V/R/O/Gate)** | BF16 | **8-bit affine** (gs=64) | **`Q8_0`** default (opt-in load-time repack to `Q4_0` via `INK_FAST_Q4=1`) |
| **Embed & LM head** | BF16 | **8-bit affine** (gs=64) | **`Q8_0`** default (opt-in `Q4_0` via `INK_FAST_Q4=1` on `output`) |
| **Routers, loop gates, sinks, norms** | BF16 / F32 | F32 (`router`, `loop gates`, `sconv` taps) / BF16 | F32 in GGUF → lossless parallel load-time cast to **F16** (`build_fast_weights`, transposed `[rank, dim]` for ctrl gain/forget, pre-folded `ffn_norm` into router) |
| **Bundled draft model (`mtp.safetensors`)** | BF16 (0.64 GB) | **Included by default** (`model-draft.safetensors`, BF16) | **Not loaded** |
| **Apple Silicon RAM floor** | 32 GB recommended | **24 GB Mac** (tested on M4 Pro 24 GB w/ GPU wired limit raised to 20 GB; 32k ctx leaves 12–14% RAM free; 64,512 max ctx) | **Hard-refuses `< 32 GB`** (`inkling-metal.c:1438`: `ram_mb < 32768`); 32K ctx leaves 9,334 MiB headroom on 32 GB Mac (`2.78 GiB` private RSS) |
| **Discrete NVIDIA GPU floor** | N/A | Unsupported | **16 GB VRAM** (`75.7 tok/s` streaming 106–114/128 experts/layer) down to **8 GB VRAM** (`42.3 tok/s`, bit-identical logits!) |
| **Variable loop count (`K`)** | `K=3` (`TML-d24-varloop-research`: variable) | Fixed to config `loop_passes` (tested `K=1,2`: 0/30 & 2/30 vs 30/30 at `K=3`) | **Supported natively** (`ink_open_ex(..., loop_count_override)` `K=1..16`, verified on `K=3,5`) |
| **Direction / LoRA adapters** | Community GGUF LoRAs (e.g., AtomicChat uncensored) | **First-class** (`convert_inkling --adapter`): folds `(I - dd')W` into 8-bit attn/embed; untouched NVFP4 experts ablated at runtime in `combine_without` (+0.7 GB symlinked) | Not implemented on Inkling engine |

### 3.2 Critical Findings on Quantization & RAM Floor

1. **Psionic's unrealized NVFP4/MXFP8 capability:** Psionic *already* implements `#authority` exact NVFP4/MXFP8 reference decoders (`psionic/lib/inkling-weights.coax`, Chapter 266) and exact `0.00e+00` Metal and CUDA dequantization kernels (`ink_dv_nvfp4`, `ink_dv_mxfp8` verified via `--test-quant-kernels`). Yet the live runtime (`inkling-gguf.c`, `inkling-metal.c`, `inkling-cuda.c`) only supports GGUF types `F32`, `Q8_0`, and `IQ4_NL`. Supporting Thinking Machines' original safetensors NVFP4/MXFP8 release directly — plus a Locals-style optimal-scale NVFP4 conversion for layers 20–23 — would simultaneously:
   - preserve layers 0–19 **bit-for-bit as trained** (avoiding post-hoc NVFP4 → FP16 → `IQ4_NL` double-quantization loss),
   - reduce checkpoint bytes from `17.6 GB` (`16.4 GiB`) down to `~15.6 GB`, and
   - unlock **24 GB Apple Silicon Macs** (M3 Pro / M4 Pro / M5 Pro 24 GB), a huge slice of developer laptops currently refused by `ram_mb < 32768`.
2. **Important quality warning on `INK_FAST_Q4=1` (4-bit attention):** In Sprint 31–40 (`895a307d18`), Psionic added opt-in `INK_FAST_Q4=1` (`Q8_0 → Q4_0` on attention projections, `lm_head`, and L0/23 MoE) which lifted M5 Max decode from ~49 to **75.01 tok/s** and passed 3/3 Tier 2 coding smoke tasks. Locals independently tested 4-bit attention and 4-bit `lm_head` on Inkling 2 Mini and **explicitly rejected 4-bit attention**: over 1,024 tokens, 4-bit attention dropped top-1 next-token agreement against 8-bit to **79%** and degraded perplexity from **18.5 → 24.8** (saving <2 ms/tok on M4 Pro), whereas 4-bit `lm_head` retained 96.4% agreement. **Action:** Keep `INK_FAST_Q4` strictly opt-in (off by default) unless a full Tier 2 + Tier 4 + perplexity audit proves `Q4_0` attention does not degrade multi-step coding reliability.
3. **Psionic's CUDA expert-streaming architecture is a genuine leap:** Keeping all non-expert tensors resident (~1.25 GiB) while streaming the 15.07 GiB of MoE experts into a per-layer LRU + lowest-router-score eviction cache (`inkling-cuda.c`, `#authority inkling-budget.coax`) allows 16 GB RTX 4080 cards to hit **75.7 tok/s decode** (8.8× faster than `llama.cpp` CUDA's layer-split offload at 8.6 tok/s) with bit-identical logits down to 8 GB VRAM. Locals has no equivalent because it targets unified-memory Macs exclusively.

---

## 4. GPU Kernel Engineering & Raw Throughput Comparison

### 4.1 Architectural Approach to Inkling 2 Mini's 48-Visit Graph

Inkling 2 Mini executes 48 logical layer visits per token with unusually high non-matmul operator density: depth attention (`mix`), low-rank loop message gates (`gate_rank=64`), 4 short convolutions (`k`, `v`, `post_attn`, `post_mlp`), L2 QK/R clipping, relative-distance bias projections, per-head attention sinks, output gating, top-8-of-128 routing, and softcapped SwiGLU (`SITU`).

- **Locals (`locals/inkling.py` + `locals/inkling_experts.py`, ~1,215 LOC):**
  - Stock MLX spent **34 ms of a 55 ms step** on ~6,000 tiny Python/MLX ops/token (125/visit) vs only 21 ms on weight reads.
  - Locals collapses each stretch between weight reads into **6 fused Metal kernels per visit**:
    1. `mix` (depth attention + optional post-attn conv + residual + low-rank loop gate + layer norm; 1,024-thread TG, `REDUCE2`),
    2. `kv_prep` (sink logit + dynamic conv tap projections + K/V shortconv + Q/K/R L2 clip + V RMSNorm + relative bias table),
    3. `attend` (3 query heads per KV head in one TG, reading K/V once over SWA ring with bias, sink, and output sigmoid gate),
    4. `route` (router scores + top-8 selection),
    5. `act` (softcapped SwiGLU on interleaved gate/up),
    6. `combine` (weighted expert sum + post-MLP shortconv + residual; plus `combine_without` for direction adapters).
  - Stacks `q_proj, r_proj, gate_proj, k_proj, v_proj` into one quantized matrix when inputs coincide (`_Consts.stack`), leaving dense 8-bit projections to MLX's own quantized matmul.
  - **Shared-expert deduplication (`locals/inkling_experts.py`):** MLX's `fp_gather_qmv` reads an expert's 4.5 MB weights separately for *every* `(token, expert)` pair. Locals writes `slots` (single-GPU-thread expert grouping, no host sync) + `xmm` (unpacked 2 NVFP4 nibbles at once via `fp4x2_` half2 bit-trick, holding inputs of up to `M=2` tokens per expert slot in registers, fusing softcapped SwiGLU into gate/up writeback). Cuts 48-visit expert compute on M4 Pro from **59.4 → 41.5 ms** at 8 tokens and **115 → 77 ms** at 16 tokens (`M <= 32`).
  - **Prefill strategy (`inkling2_loop.py` + `inkling.soft`):** Uses MLX's tiled expert GEMM in **512-token chunks**, adds a fused `soft` Metal kernel (bias + mask + softmax + sink in one pass), scores sliding layers in `QUERY_BLOCK=256` bands, and forces **visit-by-visit evaluation (`EVAL_TOKENS=128`)** to hold peak activation memory to **0.99 GB** on 24 GB Macs (vs 1.97 GB un-chunked).

- **Psionic Metal (`psionic/lib/inkling/kernels.coax` + `inkling-metal.c`, 7,278 LOC, 97 kernels):**
  - Owns **every single kernel** from Coax source (`coax metal-compute`), including all quantized matvecs/matmuls, Metal 4 Neural Accelerators, and host command-buffer encoding (`metal-host.m` with IMP-cached dispatch and `MTLResourceHazardTrackingModeUntracked`).
  - **Decode (Sprints 1–55+):** Aggressive cross-operation fusion and dispatch collapse:
    - Pre-computed pass-level `messages` for all 12 looped layers in one concurrent wave (`0` extra barriers).
    - Fused `K_MIX_CTRL_PREP_QKV`, `K_DV_Q4_CTRL_DOWN_3`, `K_CTRL_UP_APPLY_F16_3`, `K_RMSNORM_3`, `K_DV_Q4_QRG` (Q+R+Gate in one 128-thread kernel), `K_CONV_KV_BOTH` (32-lane zero-TGEM), `K_ATTN_GQA_1PASS` (18-TG single-pass Flash-Decoding with fused Q-clip & R-clip), `K_DV_Q8_CONV`/`K_DV_Q4_CONV` (fused post-attention shortconv + residual).
    - Pre-folded `ffn_norm` into `FL->ffn_gate_inp_normed` + `ink_dv_f16_norm` so FFN RMSNorm and the 32-lane zero-barrier warp `ink_router` run concurrently (`-36` barriers/tok).
    - 2-row-per-warp `packed_ushort2` IQ4/Q4 decode (`am_dot_q4_2_write`, `am_dot_iq4_situ_2_write`, `am_dot_iq4_moe_2_write` sharing `float4` activation loads) + fused post-MoE shortconv + in-place MoE down + GPU argmax (`ink_argmax_s1/s2`).
  - **Prefill (Sprints 3–53):** Native Metal 4 **`MetalPerformancePrimitives::matmul2d`** (`relaxed_precision=true`) engaging M5 Neural Accelerators directly:
    - In-tile `IQ4_NL` → `threadgroup bfloat` MoE GEMM (`am_matmul_iq4_bf16`, `TM=128`/`256`, zero `w_bf16` DRAM staging), native `bfloat × bfloat` dense GEMM (`am_matmul_bf16_x16`), FP16 tensor-core MMA Flash-Prefill with window-bounded 32-lane warp softmax (`-22.4 GB` out-of-window zero writes), and dynamic single-pass `prefill_b` (`3072` at `ctx<=8192`, `2048` at `<=16384`, `512/1024` at `32768`).

- **Psionic CUDA (`psionic/lib/inkling/kernels-cuda.coax` + `inkling-cuda.c`, 3,315 LOC):**
  - Block-major device weight relayout (`ink_relayout`) for coalesced 32-thread warp reads, 256-entry LUT table lookups for `IQ4_NL` (`ink_moe_gu_t`/`ink_moe_down_t`), sign-free `byte ^ 128` `Q8_0` dot products, 8-projection batched launch (`ink_proj_p`), fused K/V (`ink_kv1`) and post-conv residual (`ink_sum_post`), cutting token launches from ~1,900 → ~750.
  - Prefill currently decodes blocks per token (`180 tok/s` on RTX 4080); Tensor Core `cu_mma_bf16_16x8x16` prefill GEMM and CUDA Graphs remain open under `#13966`.

### 4.2 Throughput Comparison (With Honest Hardware Normalization)

> **Important hardware caveat:** Psionic Metal was measured on an **Apple M5 Max (128 GB, ~546 GB/s memory bandwidth, M5 Neural Accelerators)**; Locals was measured on an **Apple M4 Pro (24 GB, ~218 GB/s measured peak bandwidth, no M5 Neural Accelerators)**; Psionic CUDA was measured on an **NVIDIA RTX 4080 (16 GB, ~520 GB/s effective kernel BW, streaming experts from host RAM)**. Direct token/s comparisons must account for the ~2.5× bandwidth gap and M5 Neural Accelerators.

| Workload | Psionic Metal (M5 Max 128 GB, GGUF) | Psionic CUDA (RTX 4080 16 GB, GGUF streaming) | Locals MLX (M4 Pro 24 GB, NVFP4 + 8-bit) | Notes |
| --- | ---: | ---: | ---: | --- |
| **Cold prefill (1k–7k tok)** | **629–792 tok/s** (Sprint 22) → **1,449 tok/s GPU** (Sprint 53, 2,059 tok) | **180 tok/s** (900 tok; pre-Tensor-Core) | **300–322 tok/s** (6,997 tok in 23.3 s, 512-tok chunks) | Psionic M5 Neural Accelerator prefill is ~4.5× Locals M4 Pro (and ~2.2× `llama.cpp` on the same M5 Max) |
| **Decode, 1 req, no drafts (prose)** | **44 tok/s** (default `Q8_0`) / **75.01 tok/s** (`INK_FAST_Q4=1`) | **75.7 tok/s** (short) / **64.5 tok/s** (900 ctx) | **35–37.4 tok/s** (stock MLX: 17.1) | Normalized for bandwidth (218 vs 546 GB/s = 2.5×), Locals' 36 tok/s on M4 Pro ≈ ~85–90 tok/s bandwidth-scaled on M5 Max (smaller 3.6 GB/tok NVFP4+8bit footprint vs Psionic's 4.1 GB/tok GGUF default!) |
| **Decode, 1 req, tool file write (coding)** | **44–75 tok/s** (same as prose — no speculation) | **64–75 tok/s** (no speculation) | **75.6 tok/s** (2.05× over base via DSpark + lookup + `xmm`) | **Locals on an M4 Pro matches Psionic `INK_FAST_Q4=1` on an M5 Max for coding file writes solely via speculative decoding!** |
| **Decode, 1 req, new file via tool** | **44–75 tok/s** | **64–75 tok/s** | **59.6 tok/s** (1.59× over base) | DSpark accelerates even newly authored code |
| **Decode, 4 concurrent reqs (aggregate)** | Serialized (**1× single-stream**) | Serialized (**1× single-stream**) | **79 tok/s** (2.26× single-stream) | `HybridBatchEngine` + `inkling_experts.xmm` |
| **Decode, 8 concurrent reqs (aggregate)** | Serialized (**1× single-stream**) | Serialized (**1× single-stream**) | **95 tok/s** (2.71× single-stream; stock MLX: 48) | Critical for parallel subagents / `locals run` |
| **Cached multi-turn TTFT (3k–7k ctx)** | **0.12–0.18 s** (4.3k–5.4k ctx, 99.6% hit, post-decode slot 1) | Supported via same `inkling-serve.c` | **0.5–0.8 s** (7k ctx, RAM or SSD!) | Psionic faster on hot RAM slot; Locals persists across chats & restarts |
| **Cached long-context TTFT (32k ctx)** | Hot slot only (cold after chat switch/restart) | Hot slot only | **1.0–1.2 s** from SSD (vs 142 s cold) | Locals' SSD cold tier is a game-changer on 24–32 GB Macs |

**Key insight from bandwidth normalization:** Why does Locals hit 36 tok/s (non-speculative) on a 218 GB/s M4 Pro while Psionic's default `Q8_0` hits ~44–50 tok/s on a ~546 GB/s M5 Max? Partly checkpoint byte width (**3.6 GB/tok** in NVFP4+8-bit vs **4.10 GB/tok** in mixed `IQ4_NL+Q8_0+F32` GGUF, noted in `inkling-metal-report.md` §4 #6) and partly because Psionic's `Q8_0` decode kernels hadn't received the same 2-row-per-warp `packed_ushort2` treatment that `Q4_0` got in Sprint 55 (`981d9d1271`, which hit 75+ tok/s). And once Locals turns on **DSpark**, a 24 GB M4 Pro writes code files at **75.6 tok/s** — demonstrating just how massive a lever DSpark would be on top of Psionic's M5 Max kernels (**projected 100–160+ tok/s**!).

---

## 5. Speculative Decoding: Locals' DSpark + Prompt-Lookup Architecture

This is the single highest-ROI inference capability that Locals has and Psionic currently lacks.

### 5.1 How Locals Integrates Thinking Machines' DSpark (`locals/inkling_draft.py`)

Thinking Machines ships a 0.64 GB BF16 multi-token draft model (`mtp.safetensors`, called **DSpark**) alongside Inkling 2 Mini. Locals' implementation is remarkably clean (only 212 lines in `locals/inkling_draft.py`):

1. **Architecture:** 3 dense transformer layers (`DraftLayer`, with RoPE + Q/K RMSNorm + SwiGLU) operating over an 8-token block (`[anchor_token, mask_embed × 7]`). Each draft layer attends over the 8-token block AND a tapped target context (`k_proj_ctx`, `v_proj_ctx`) formed by learned softmax weights (`fuse_weight`) over the target's hidden streams entering visits `[5, 23, 41]` and at final output (`Taps`).
2. **Pre-folded `MarkovHead` (`Drafter.bind`):** DSpark has a rank-256 recurrent `MarkovHead` that conditions each drafted position `i` on the token drafted at `i-1`. Naively applying `lm_head(x + hidden_out(latent))` 7 times would run a full `201k × 2304` output projection 7 times. Instead, at load time `Drafter.bind()` folds `fold_w = target.lm_head.weight @ hidden_out.weight` (`201k × 256`) and `fold_b = target.lm_head.weight @ hidden_out.bias` in 8,192-row chunks! Each recurrent step then adds only a cheap rank-256 `mx.addmm` to the precomputed base logits — keeping all 7 drafted tokens to **~12 ms** (~6.5–12 ms depending on context) on M4 Pro.
3. **Unified cache lifecycle (`Model.make_cache`):** Locals appends DSpark's 3 KV/sliding caches directly after the target's 96 caches (`48 × (KV, conv)`). Every target pass (prefill chunk or decode step) calls `taps.leave(...) → mtp.absorb(...)`. Consequently, **prompt caching, checkpoints, SSD disk saves, and `inkling.rewind()` carry DSpark's full-prompt context automatically** — crucial because DSpark keeps **6.2 of 7 tokens** on tool file writes when it has full prompt context vs only **2.8 of 7** with the reply alone!
4. **Adaptive dual-source draft scheduler (`hybrid_batch.py:_solo_draft`):**
   - Races **3-gram prompt-lookup drafts** (zero draft compute; ideal for verbatim copying) against **DSpark model drafts** (confidence-gated at `p > 0.8`, `draft_min_p`; ideal for JSON-escaped tool file writes where lookup drops to 50% acceptance while DSpark hits **97%**!).
   - Tracks a rolling 6-try mean step gain (`_draft_gain`) per source (probing the runner-up 1 in 16 steps so estimates don't go stale) and computes net step gain accounting for both draft latency (`draft_model_cost = 0.25` steps) and MoE per-token verification cost (`draft_token_cost = 0.25` steps/tok because each drafted token activates extra experts!).
   - Backs off exponentially (`32 → 64 → 128 → 256` tokens) when recent tries lose money (e.g., open-ended prose), keeping worst-case prose overhead to **-4%**.
5. **Exact multi-token verification & rewind (`inkling.py` + `inkling.rewind`):**
   - All 6 Metal step kernels accept a `SEQ=True` template parameter running up to `SEQ_MAX=32` consecutive tokens of one stream in a single pass (`25 + 6n ms` on M4 Pro with `xmm` shared-expert deduplication, vs `28 ms` for 1 token).
   - Records per-token shortconv states (`conv.ink_states`) and keeps sliding windows un-shifted during the verification pass so `inkling.rewind(cache, drop)` restores exact KV, shortconv, and DSpark `ink_feats` states for any rejected tail tokens.
   - Works seamlessly under walked constrained grammars (`WalkController.draft_step` prunes draft tokens against the DFA before target verification).

### 5.2 Feasibility of Porting DSpark + Lookup to Psionic

Psionic already has almost every primitive required to implement this natively in Coax + C:
- Psionic's `ink_prefill` ALREADY runs multi-token causal batches (`nb = 2..32`) with direct-IQ4 small-batch MoE kernels (`K_MOE_GU_IQ4_M4` / `K_MOE_DOWN_IQ4_M4`) and `K_MM_F32_M4`!
- Saving per-token shortconv states for `nb <= 16` tokens (`16 × 3.8 MB = 60 MB` scratch) and rolling back `s->pos` + SWA ring write pointers is straightforward because `s->kv_cap[il] = hp->swa + s->prefill_b` already retains >1,024 extra positions beyond the SWA window!
- Adding (a) a zero-extra-weight **prompt-lookup draft verifier** first (instant win on file edits/rewrites, zero download overhead), followed by (b) **DSpark (`mtp.safetensors`)** support in `psionic-inkling-serve` (Metal + CUDA) is the single highest-leverage performance upgrade on Psionic's post-v1.0 roadmap.

---

## 6. Serving Engine, Concurrency, Prefix Caching & Server Security Audit

### 6.1 Concurrency & Scheduling

- **Locals (`service.py` + `hybrid_batch.py`):**
  - Single GPU worker thread running `HybridBatchEngine.serve`: continuous batching up to `n_slots=16`, pipelined `mx.async_eval` (launching step `s+1` on device before CPU reads step `s`), uniform `(B, 1)` drip decoding, per-row `temperature`/`top_p`/`top_k`/`seed` (via deterministic `row_key(seed, draw_n)` Gumbel-max in one vocab sort), admission control (`_fits`, `_make_room`, `_growth_trim` every 32 steps), and automatic Metal OOM recovery (halves prompt cache, clears MLX pool, continues serving).
  - For Inkling specifically (`hybrid_batch.py:2287-2341`), when parallel agents arrive with shared prompts (like `locals run` versions), admission finds the top-2 longest common prefixes across waiting requests (`>= 64` tokens), prefills once, and caches them for siblings — cutting a 4-agent `locals run` task from `4,755 → 1,100–1,600` uncached prompt tokens and wall time from `49–50 s → 34–40 s`.
  - Also implements macOS Metal interactive-timeout buffer-drop recovery (`_read_chunk` / `_marks` / `_rewind` with `mx.synchronize` before each chunk retry).
- **Psionic (`psionic/platform/inkling-serve.c`):**
  - Thread-per-connection POSIX/Win32 socket server (`listen(srv, 32)`), but **all inference serializes on a single global `sv->sess_mu` mutex** around a single `InkSession` (`inkling-serve.c:1618`).
  - Fine for OpenAgents Desktop v1.0's primary single-active-chat workflow, but **blocks completely** whenever two Desktop chats run simultaneously, a background `/goal` overlaps an interactive chat, or future multi-agent fan-out is enabled.

### 6.2 Prefix Caching & Multi-Turn Parity (Two Parallel Solutions — and Remaining Psionic Gaps)

Both teams discovered the same fundamental Inkling 2 Mini caching traps: (a) 40 SWA-1024 + 48×4 shortconv caches cannot be naively trimmed backward; (b) multi-turn prompt re-rendering breaks prefix hits whenever reasoning is dropped or tool-call JSON formatting shifts.

| Caching Capability | Psionic (`inkling-metal.c` + `inkling-serve.c`, post-`#13979`) | Locals (`hybrid_batch.py` + `formats.py` + `server.py`) |
| --- | --- | --- |
| **In-memory cache capacity** | **2 fixed slots** (`snaps[0]` pre-decode + `snaps[1]` post-decode) sharing one live KV buffer | Multi-entry **`ChainCache` LRU** (`max_size=256`, `1.25 GB` tuned for Inkling on 24 GB Mac; prunes superseded `assistant`/`user` prefixes while keeping `system`) |
| **SWA wrap-safety after >1,024-tok decode** | **Solved in `#13979`:** Slot 1 saves post-decode at `n_prompt + n_gen` where `max_pos_written == sp`, guaranteeing `91–99.7%` hit rate even after a 4,096-token thinking turn | Solved via `BatchRotatingKVCache` copies per checkpoint/retired row (cost: ~160–480 MB per saved conversation state) |
| **System / user / reply checkpoints** | No intermediate checkpoints inside a prompt (only `n_prompt` and `n_prompt + n_gen`) | Automatic `system`, `user`, `reply`, and `shared` checkpoints from `checkpoint_tokens=192` (skipping redundant `user` copies within `_CP_GAP=32` tokens of a cached prefix) |
| **SSD cold-tier persistence** | **None.** Server restart or switching to another chat forces full cold prefill | **`DiskPromptCache`** (`~/.cache/locals/prompts`, 4 GB LRU cap, mode `0700`/`0600`, weight-fingerprinted): saves `system` snapshots AND (for Inkling) `user` conversation checkpoints every **2,048 tokens** → 32k-token follow-up in **1.0–1.2 s** even after restart! |
| **External client replay parity** | Relies on client (Inky) sending back exact `reasoning_content` + `args_json`; `format_inkling_prompt` always writes compact `{"name":"...","args":...}` | **`Inkling.restore` + `_raw` table + `_streams` token-ID cache:** Even if an external client (OpenCode/Cline/Claude Code) drops `reasoning_content` or re-formats whitespace, the server restores the original reply keyed by first `tool_call.id` and reuses exact token IDs |
| **Template `dictsort` parity** | `format_inkling_prompt` passes `tc->args_json` verbatim (works with Inky because Inky preserves the exact string emitted by `inkling-serve.c`) | `Inkling.typed_call` enforces sorted key order during generation so Jinja's `dictsort` matches generated tokens verbatim |

### 6.3 Concrete Server Gaps Found in `psionic/platform/inkling-serve.c`

Auditing `psionic/platform/inkling-serve.c` directly against `locals/server.py` surfaced **5 concrete server defects/gaps** that should be addressed in Psionic:

1. **P0 — No client-disconnect cancellation during decode (`inkling-serve.c:1511-1527, 1752-1935`):**
   - `emit_chat_delta`, `emit_responses_delta`, and `emit_chat_tool_call` discard the return value of `send_sse_event` / `send_all`. While prefill checks `send_all(fd, ": prefilling\n\n", 14) != 0` between chunks (`line 1691`), **the decode loop never checks whether the client socket closed**.
   - **Impact on OpenAgents Desktop & Inky:** When a user hits Stop/Esc mid-generation (or Inky aborts a step), `psionic-inkling-serve` keeps holding `sess_mu` and decoding up to `max_gen = 8192` tokens (potentially 100–180 seconds!) before the next user message or steer can even start prefill!
   - **Fix:** Propagate `send_sse_event` failure out of `emit_*` (and periodically `poll(POLLHUP|POLLERR)` for non-streaming or buffered tool-call intervals), immediately break out of the decode loop, skip post-decode snapshot save, and release `sess_mu`.
2. **P0 — Wildcard CORS (`Access-Control-Allow-Origin: *`) with zero `Host`/`Origin` checks (`inkling-serve.c:254, 267, 2212`):**
   - Any malicious webpage visited in the user's browser can issue cross-origin `POST http://127.0.0.1:18794/v1/chat/completions` requests (or DNS-rebind to it), stealing local GPU compute, reading responses, and wiping Desktop's active KV cache.
   - **Fix:** Adopt Locals' exact loopback security model (`_host_ok` + `_origin_ok` rejecting non-loopback/non-app `Origin` headers with `403`, echoing only allowed origins with `Vary: Origin`, or removing `Access-Control-Allow-Origin: *` altogether when only native Desktop/CLI clients talk to the port).
3. **P1 — Loop circuit breaker only covers `channel == 1` (`<|content_thinking|>`):**
   - Psionic's `detect_thinking_loop` (`inkling-serve.c:1469-1486, 1909-1933`) is actually *smarter* for Inkling than Locals' hard-abort because injecting `<|end_message|>` gracefully transitions out of circular reasoning into an answer or tool call instead of throwing away the turn.
   - However, `channel == 2` (`<|content_text|>`) and `channel == 3` (`<|content_invoke_tool_json|>`) have **no repetition detector** — if Inkling gets stuck repeating a line in prose or inside a `write_file` argument, it will spin all the way to `8,192` tokens. Add a secondary higher-threshold breaker for channels 2/3 (e.g., Locals' `reps=12..16`, grace window `1,024` tokens, ending with `finish_reason="length"`).
4. **P1 — Buffered tool-call SSE emission:**
   - During a long `write_file` call (`1,000–3,000` tokens), `inkling-serve.c:1869-1870` buffers the entire JSON in `tool_json` and emits nothing over SSE until `<|end_message|>` (causing 15–60 s of UI silence, and risking idle-stream timeouts if no keepalive is sent!). Both Psionic and Locals currently buffer Inkling tool calls whole (Locals only streams incremental `CallStream` deltas for Qwen3.5 XML), so emitting incremental `delta.tool_calls` argument chunks (or SSE keepalive comments during `channel == 3`) is an easy UX win.
5. **P2 — Unparsed OpenAI fields (`tool_choice`, `parallel_tool_calls`, `stop`):**
   - `parse_request_json` silently skips `tool_choice`, `parallel_tool_calls`, and `stop` (`line 1036`). Inky works around this cleanly by omitting `tools` on its final closing step (`ik_guard_disable_tools`), but external OpenAI clients expecting `tool_choice: "none"` or `stop` sequences will see silent no-ops. (Locals also fails closed with HTTP `400` on unsupported fields like `n != 1` or `logprobs: true`, preventing silent mismatches.)

---

## 7. Constrained Decoding & Grammar Architecture

| Dimension | Psionic (`psionic/lib/inkling/json-schema.coax` + `inkling-serve.c`) | Locals (`locals/walk.py`, `grammar.py`, `regex.py`, `forced.py`, `budget.py`, `pda.py`, `formats.py`) |
| --- | --- | --- |
| **Execution target** | **CPU** (Coax byte-level RFC 8259 automaton; checks argmax → top-64 → full-vocab fallback) | **GPU** (`locals_dfa_walk` Metal kernel walking all 201k tokens through byte DFA per step for `--walk`, or GPU gather over compiled `(Q, V)` tables) |
| **Memory footprint** | Negligible (~10 KB fixed Coax struct `JsSchema`/`JsState` + pre-decoded vocab pool) | `--walk` (default on Inkling): ~512 B/state (3 MB typical vs 1.3 GB host + 0.4 GB GPU for token tables) |
| **Used on Inkling tool calls?** | **No — intentionally disabled on Inky** (`inky-spec.md` §3.1 #6). Validated by `#authority` `coax/lib/inky/tools.coax` with `<\|content_tool_error\|>` feedback | **Yes — enabled by default on Inkling** (`walk_constraints=True`, `typed_default=True`). Enforces exact `<\|message_model\|>NAME<\|content_invoke_tool_json\|>{"name":...,"args":{...}}<\|end_message\|>` with sorted keys & typed values |
| **Forced-span jump-forward** | **None** (every structural token like `{"name":"read_file","args":{"path":"` is sampled 1 token at a time) | **Yes** (`ForcedPlanner` with canonical BPE boundary cover & healing; on solo Inkling, spans `>= 2` tokens feed in one `seq` pass at ~6–12 ms/tok instead of 30 ms/tok!) |
| **Budget-aware masking** | None (hitting `max_tokens` can truncate JSON mid-value) | Bellman-Ford `budget_tables` on table mode (guarantees valid closing before `max_tokens`); off on `--walk` |
| **Recursive schemas (`$ref` cycles)** | Up to `JS_MAX_DEPTH=16` via explicit stack in `JsState`, no `$ref` resolution | Full deterministic PDA (`pda.py`/`pdactl.py`) with GPU stack (`STACK_DEPTH=64`) in library mode; refused on table server |
| **Unsupported schema constructs** | `anyOf`/`oneOf`/`allOf`/`pattern`/`minimum`/`maximum`/`minItems`/`$ref` | Handles `allOf` merging, `anyOf`/`oneOf`, integer `minimum`/`maximum`, JSON-safe `pattern` rewriting, `ipv4`/formats |

### Trade-Off Analysis for Inky

- **Why Inky's zero-grammar stance is right for v1.0:**
  1. Inkling 2 Mini was post-trained specifically on `<|message_model|>NAME<|content_invoke_tool_json|>` and already passed **10/10 real multi-language coding evals** without tool-call grammar masking (`inkling-metal-report.md` §2C).
  2. Forcing a rigid key-sorted schema grammar (`Inkling.typed_call`) changes token probabilities on optional parameters and requires rejecting/simplifying complex JSON Schema constructs (and on Locals' `--walk` path, disables budget masking and healing).
  3. Because Inky returns structured, native **`<|content_tool_error|>`** diagnostics from `#authority tools.coax` whenever a tool argument is invalid, a rare malformed call recovers in 1 step at ~150 ms cached prefill.
- **Where Locals' decoding tech is worth adopting later:**
  1. **Forced-span jump-forward** (if Psionic adds multi-token `seq` passes for DSpark/lookup anyway) saves full decode steps on predictable structural prefixes.
  2. **Moving `json-schema.coax` masking onto the GPU** (if structured-output callers ever become latency-sensitive on 201k-token vocabularies when greedy argmax misses and hits the CPU full-vocab scan).
  3. **Optional `tool_choice: "required"` / named-tool enforcement** for non-Inky API clients.

---

## 8. Coding-Agent Harnesses Compared: Inky vs. Locals (`locals run` + Server Layer)

This is the second major pillar of the comparison. The two projects solve **complementary layers** of the local coding-agent problem:
- **Inky (`coax/lib/inky/` + `coder/tui/inky.coax`)** is an **interactive, multi-turn, single-agent Desktop/CLI coding harness** replacing Microcoder (`#13978–#13983`).
- **Locals** provides (a) a **server-side agent compatibility & guard layer** for 9 third-party coding agents (`OpenCode`, `Claude Code`, `Cline`, `Kilo Code`, `Aider`, `OpenHands`, `Hermes Agent`, `OpenClaw`, `Roo Code`), and (b) **`locals run`** (`locals/harness.py`, `plan.py`, `solve.py`), an **autonomous, batch-native, test-driven multi-agent parallel solver**.

### 8.1 Detailed Feature Comparison Matrix

| Dimension | OpenAgents Inky (`coax/lib/inky/`, `#13980`) | Locals Multi-Agent Harness (`locals run`, `harness.py` + `plan.py` + `solve.py`) | Locals Server Agent Layer (`server.py` + `oai.py`) |
| --- | --- | --- | --- |
| **Primary use case** | Interactive local coding assistant in OpenAgents Desktop (plain chat, repo exploration, multi-turn debugging, targeted edits, `/goal`) | Autonomous batch coding task with a test command (`locals run "TASK" --check "pytest"`) | Drop-in backend for external agents (`locals code`, `locals launch claude/cline/aider/...`) |
| **Implementation & verification** | Pure Coax, zero heap alloc, `#authority` verified (**396/396** `tools.coax` + **128/128** `guard.coax` mutants killed), `coax test` + `selftest.coax` | Python threads over HTTP to `locals serve`, monkeypatched unit tests (`test_harness.py`) + E2E proxy suite (`tests/e2e/`) | Python + extensive unit/E2E suite with real external CLIs |
| **Toolset** | **7 orthogonal native tools** (~550 tok): `read_file`, `edit_file`, `write_file`, `grep`, `list_dir`, `run_command`, `update_todos` (no `finish` tool — plain text ends turn) | **6 tools**: `list_files`, `read_file`, `write_file`, `edit_file`, `run`, **`finish`** (forced via `tool_choice="required"`, `parallel_tool_calls=False`) | Adapts whatever tools the client sends |
| **Parallel tool calls per step** | **Supported natively** (`IkLoop` + `GwToolStep` batching multiple `tool_calls` in order) | Disabled (`parallel_tool_calls=False`, 1 call/step) | Configurable per format (`parallel_tool_calls`, `--max-calls 6`) |
| **File reading (`read_file`)** | **200 lines / 16 KiB head-only** (NEVER middle-cuts!), numbered lines `N\ttext`, explicit next-line cursor, 8-hex FNV-1a file hash | Plain text cut at **6,000 characters** (no line numbers, no cursor, no hash) | Client-owned |
| **File editing (`edit_file`)** | Exact match (`replace_all` opt), compact diff hunk on success, **top-3 nearest lines on miss** | Exact match → **`_loose_replace` fallback** (ignores line-edge whitespace, then blank lines, and **auto-shifts `new` indentation** when unique!) → `_closest` diff-ratio snippet on miss | Client-owned |
| **Shell output (`run_command`)** | Async non-blocking `/bin/sh -c`, **16 KiB inline** verbatim → spills to disk with **8 KiB head + 8 KiB tail + spill path readable via `read_file`**; sets `is_error=true` (`<\|content_tool_error\|>`) | Sync bash (60 s timeout), returns `exit N` + **last 1,500 chars** only (1,000 on timeout); automatically runs `--check` after every `write_file`/`edit_file` | Client-owned |
| **System prompt design** | **<= 600 base tokens (~2.2 KB)**; zero-contradiction policy resolver (suppresses stock auto-commit conflict in code), bounded 4 KB user/workspace instructions, conditional 4-line `/goal` block, zero `MC_EARLY_NUDGE` | Short fixed `SYSTEM` (~120 words) + shared stage prefix (`project_digest` 6 KB + task + part plan) crafted for 100% sibling KV sharing | `_cwd_hint` appended at end of system prompt (`Working directory: ...`), `_volatile_to_end` moves dynamic timestamps/cwd blocks to end of system prompt (OpenHands: 1,555 → 13,714 cached tokens!) |
| **Multi-turn & context compaction** | **Append-only `IkHistory`** + interleaved `reasoning_content` + **SWA-aware 2-stage zero-LLM-call compaction** (`compact.coax`: Stage 1 at 70% masks >512 B old tool outputs & strips old reasoning; Stage 2 at 88% collapses oldest half at safe tool-pair boundary) + real alternating `MWW_RECALL` | None — intentionally **short-lived agents** (10 steps / 90 KB cap); spawns fresh fixers with clean context instead of compacting | Server-side cache checkpoints + `DiskPromptCache` + returns OpenAI/Anthropic `context_length_exceeded` strings clients compact on |
| **Loop & duplicate-call protection** | `#authority guard.coax`: allows 1st & 2nd identical read/grep/list, **short-circuits 3rd+ with native `<\|content_tool_error\|>`**; short-circuits 2nd identical passing check; resets on any mutation; soft warning at `max_steps-2`; tool-disabled closing step at `max_steps` (30 interactive / 60 `/goal`); + server thinking-loop breaker | Harness: nudges after 3 identical calls, 2 repeating calls, or 4 non-edit calls in a row; ends agent on 3 alternating pairs or 2 no-tool replies; `finish` refused while `--check` blames agent's part (up to 2×) | **Server-side cross-turn duplicate-call steering**: detects repeated call from previous turn, **re-decodes with an injected steering prefix** (`"The X call above was already made..."`), then temporarily removes the repeated tool from the grammar up to 2×! Plus `_looping` + `_line_looping` |
| **Interactive sampling** | **`temp=0.6, top_p=0.95, top_k=40`** interactive (`INKY_DETERMINISTIC=1` → `temp=0.0, seed=0`) | Temperature diversity across parallel versions: `_TEMPS = (0.2, 0.7, 0.9, 0.5, 1.0, 0.6, 0.8, 0.4)` | Server default `--temp 0.2`, per-request overrides honored |
| **Sandboxing & write safety** | `#authority` lexical + `realpath` confinement to `cwd`/spill dir; `gitguard` on shared checkouts; macOS/Linux sandbox profiles; `MWW_ASK` approval gating ONLY on mutating/shell ops (0 gates on read-only tools); durable `AJR1` journal logging exact model-visible bytes | Each agent works in an isolated temp copy (symlinking `node_modules`/`.venv`); macOS `sandbox-exec` write confinement on `run`; existing test files read-only unless task asks to change tests; **all-or-nothing atomic write-back** only when `--check` passes and source manifest hasn't drifted (failed runs archived to `~/.locals/runs/`) | Loopback Host/Origin checks |
| **Test-driven parallel synthesis** | None (single agent per chat; `/goal` iterates sequentially) | **Signature strength:** Splits task into independent file-owning code parts → test parts; spawns `k` parallel versions/part + `--race 2` whole-task racers (all sharing one GPU batch!); checks after every write; **combinatorial mix-and-match (`_pick` / `_search`)** across part versions; **blame-guided repair rounds** with import-graph widening (`_uses`) and tail-cutting (`_run_agents`) | N/A |

### 8.2 Deep Comparison of Agent Design Philosophy

1. **Where Inky Is Clearly Stronger (Interactive Local Coding Assistant):**
   - **Built for `#13978`-class open-ended repo work:** Inky's numbered, head-only `read_file` with explicit continuation cursors, `git grep`-backed `grep`, compact `list_dir`, 16 KiB inline + spill-readback `run_command`, zero approval friction on read-only tools, and SWA-aware 2-stage compaction are miles ahead of `locals run`'s 6 KB truncated `read_file` and 1.5 KB `run` tail for multi-turn codebase investigation and debugging.
   - **Native Inkling 2.0 error signaling:** Returning `<|content_tool_error|>` (`200022`) on failed commands, bad paths, edit misses, and duplicate calls aligns directly with Inkling's post-training distribution — something Locals never wires up.
   - **Formal `#authority` verification & truthful diagnostics:** Inky's path confinement, admission gating, and loop guard are pure I/O-free Coax modules with 0 surviving mutants across 524 mutations, and AJR1 journals record the exact bytes shown to the model (eliminating Microcoder's forensic blind spot).
   - **No awkward `finish` tool or `tool_choice="required"`:** Plain conversation ("Hello!") finishes in 1 call with 0 tools and zero schema overhead, whereas `locals run` forces every step through `tool_choice="required"` and an explicit `finish` tool.

2. **Where Locals Has Ideas Inky & OpenAgents Should Study Closely:**
   - **Whitespace/indentation-tolerant `edit_file` (`_loose_replace` in `locals/harness.py:350-384`):** Locals measured that **a third of all repair edits** on small local models missed exact string matching purely due to trailing spaces, blank lines, or indentation shift. `_loose_replace` tries exact match first, then line-stripped match (requiring a **unique** match!), then blank-line-stripped match, and shifts `new`'s indentation by the delta between the file's real indent and `old`'s indent. Inky currently does strict exact match + top-3 nearest-line error diagnostics (`ik_fmt_edit`). During Phase 3 (`#13981`), we should explicitly log `edit_file` miss counts; if whitespace/indent misses show up on Inkling 2 Mini, adding a strict-unique `_loose_replace` fallback in `coax/lib/inky/format.coax` is a clean, bounded upgrade.
   - **Volatile-block normalization (`_volatile_to_end` in `locals/server.py`):** Locals noticed external agents put timestamps/cwd mid-system-prompt, destroying prefix cache reuse across sessions (OpenHands jumped from 1,555 → 13,714 cached tokens when moved to the end). Inky's `prompt.coax` currently places `Environment: - OS / - Workspace / - Git repo` at the **top** of the system prompt (`prompt.coax:73-78`), *before* the ~300-token static `How to work:` rules and user instructions! Moving `Environment:` (which changes per worktree/branch) to the **bottom** of `ik_prompt_build` (after `How to work:` and `User instructions:`) lets multiple worktrees/chats share the entire tool declaration + static system instructions prefix once Psionic has multi-entry or SSD prefix caching!
   - **After-write automatic check feedback (`Run._after_write`):** In autonomous `/goal` mode (where a check command is known), `locals run` runs the check automatically right after `write_file`/`edit_file` and appends the blamed failure lines straight into the write tool's response — cutting half of small models' tool turns (no separate "now run pytest" step). Worth considering as an opt-in `/goal` accelerator post-v1.0.
   - **Batch-native parallel synthesis (`locals run`) for hard `/goal` tasks:** Once Psionic adds continuous batching (so 4–6 parallel agents cost ~2× instead of 6× wall time), combining Inky's superior per-agent tool loop with `locals run`'s **parallel versioned parts + whole-task racers + combinatorial `_pick` + blame-widened repair rounds** in isolated worktrees is a natural evolution for OpenAgents' autonomous `/goal` mode (and directly leverages Inkling 2 Mini's shared-expert scaling!).
   - **Server-side duplicate-call steering & `_line_looping`:** Inky's `guard.coax` catches exact repeated read/search calls (`count >= 3`) and passing checks (`count >= 2`), which handles identical loops cleanly, **but does not yet catch semantic/near-duplicate loops** (e.g., running `grep "foo"`, `grep "foo "`, `grep "Foo"` or alternating two slightly different commands — the exact failure mode noted in `#13978` line 33!). Watch for near-duplicate spin patterns during Phase 3 (`#13981`).

---

## 9. Prioritized Action Plan Mapped to Open OpenAgents Issues

### Tier 1 — Immediate Fixes Before Inky Phase 3–4 Cutover (`#13981`, `#13982`)

| Priority | Target | Concrete Change | Why / Evidence |
| --- | --- | --- | --- |
| **P0** | [`psionic/platform/inkling-serve.c`](../platform/inkling-serve.c) | **Abort decode immediately on client socket disconnect** (check `send_sse_event` return values in `emit_*` + non-blocking `poll` during `channel == 3` buffering; skip slot-1 save on abort; release `sess_mu`). | Right now hitting Stop/Esc in Desktop leaves `psionic-inkling-serve` locked on `sess_mu` decoding up to 8,192 tokens while the user's next message hangs. |
| **P0** | [`psionic/platform/inkling-serve.c`](../platform/inkling-serve.c) | **Lock down loopback HTTP security:** replace `Access-Control-Allow-Origin: *` with strict `Host` (DNS-rebinding) and `Origin` checks (allow only loopback / app schemes, `403` otherwise, `Vary: Origin`). | Prevents arbitrary websites in the user's browser from calling `127.0.0.1:18794`, exfiltrating answers, or evicting Desktop's KV cache. |
| **P1** | [`psionic/platform/inkling-serve.c`](../platform/inkling-serve.c) | **Extend repetition circuit breaker to `channel == 2` and `channel == 3`** (higher threshold / grace window, e.g., 12–16× repeat + 1,024-tok grace, cleanly terminating with `finish_reason="length"`). | `detect_thinking_loop` protects `channel == 1` cleanly, but text or tool-argument loops still run to 8,192 tokens. |
| **P1** | [`coax/lib/inky/prompt.coax`](../../coax/lib/inky/prompt.coax) | **Move per-workspace `Environment:` block below static `How to work:` rules** (keeping base prompt `<= 2.2 KB`). | Maximizes cross-worktree/cross-chat KV prefix sharing (same insight as Locals' `_volatile_to_end`), while placing `Workspace: <cwd>` closer to the model's 1,024-token SWA recency window! |
| **P1** | [`evals/openagents-desktop/tier2/`](../../evals/openagents-desktop/tier2/) (`#13981`) | **Instrument `edit_file` miss rate and near-duplicate tool loops** during Phase 3 qualification. If whitespace/indent misses exceed ~10% of edits, add a strict-unique `_loose_replace` fallback to `coax/lib/inky/format.coax`. | Locals found 1/3 of small-model repair edits missed on whitespace/indent alone; measure on Inkling 2 Mini before complicating `format.coax`. |
| **P2** | [`psionic/platform/inkling-metal.c`](../platform/inkling-metal.c) | **Keep `INK_FAST_Q4=0` as the shipped default** until full Tier 2 + Tier 4 quality parity is measured on `Q4_0` attention (and consider splitting `INK_FAST_Q4_HEAD` separately from attention). | Locals measured 4-bit attention dropping top-1 agreement to 79% and raising perplexity `18.5 → 24.8` on Inkling 2 Mini, while 4-bit `lm_head` kept 96.4%. |

### Tier 2 — High-Leverage Post-v1.0 Psionic Engine Roadmap (v1.0.1 / v1.1)

1. **Speculative Decoding: Prompt-Lookup + Thinking Machines' DSpark (`mtp.safetensors`):**
   - *Step 2a (zero extra weights):* Add a 3-gram prompt-lookup drafter + multi-token `seq` verification (`nb = 2..16`) + shortconv/SWA rewind in `inkling-metal.c` and `inkling-cuda.c`. Immediate speedup on file edits, refactors, and quoting.
   - *Step 2b (DSpark `0.64 GB`):* Load `mtp.safetensors` alongside the checkpoint, fold `lm_head @ markov_head.hidden_out` once at startup (201k × 256), tap visits `[5, 23, 41, out]`, store the 3 draft KV caches inside `InkSession` + prefix snapshots, and gate with Locals' adaptive step-gain rule (`p > 0.8`, backoff on prose).
   - *Expected impact:* **1.6–2.0× single-stream coding throughput** (**~100–160+ tok/s** on M5 Max file edits; **~120+ tok/s** on RTX 4080).
2. **Native Original NVFP4 + MXFP8 Safetensors Runtime & 24 GB Mac Unlock:**
   - Wire Psionic's already-verified `ink_dv_nvfp4` / `ink_dv_mxfp8` kernels (ch266/ch269) into a native safetensors loader with an optional converter (or direct optimal-scale NVFP4 tail conversion for layers 20–23 + 8-bit attention/head = **~15.6–16.2 GB**).
   - Eliminates GGUF double-quantization loss on layers 0–19 and lowers Apple Silicon Desktop's RAM floor from **32 GB → 24 GB** (matching Locals' M4 Pro 24 GB fit).
3. **Multi-Slot Prefix Cache + SSD Cold Tier (`DiskPromptCache`):**
   - Replace the 2-slot single-session snapshot with a bounded multi-conversation LRU + SSD cold tier under `~/.openagents/cache/psionic/prompts/` (mode `0700`/`0600`, keyed by model SHA-256 + engine version + token-prefix hash), saving system prompts and periodic 2,048-token conversation checkpoints.
   - Turns Desktop app restarts and multi-chat switching on 32K contexts from ~25–50 s cold prefill into **~1 s**.
4. **Continuous Batching + Shared-Expert Deduplication (`inkling-metal.c` & `inkling-cuda.c`):**
   - Replace single-request `sess_mu` with a multi-slot batch scheduler (`B = 1..8`) and expert-grouped multi-row decode kernels (building on Psionic's existing prefill expert grouping and Locals' `xmm` insight).
   - Unlocks simultaneous Desktop chats, background `/goal` alongside chat, and `locals run`-style parallel version synthesis at ~2–3× aggregate throughput.
5. **Incremental SSE Tool-Call Streaming & Server-Side Raw Replay (`_raw`):**
   - Stream `delta.tool_calls` arguments incrementally during `channel == 3` so Desktop shows live code writing during large `write_file` calls instead of pausing 15–60 s.
   - If Psionic exposes Inkling to external clients (OpenCode/Cline/Claude Code), add a bounded `_raw` table (like Locals' `Inkling.restore`) so clients that strip `reasoning_content` still hit Psionic's prefix cache.
6. **Track B CUDA (`#13966`) Completion:**
   - Land Coax `cu_mma_bf16_16x8x16` Tensor Core prefill (`180 → 500+ tok/s`), CUDA Graphs (removing ~750 launch/tok CPU overhead), and live 24 GB resident + Windows verification — consolidating Psionic's unique cross-platform lead over MLX-only servers like Locals.

### Tier 3 — What OpenAgents Should Deliberately NOT Adopt from Locals

1. **No Python / MLX runtime dependency:** Keep Psionic 100% native Coax + C across Metal 4 and CUDA with zero Python/vendor GEMM dependencies.
2. **No monolithic `tool_choice="required"` + explicit `finish` tool in Inky:** Inky's native Inkling multi-turn flow (plain text ends the turn; tools are optional) is structurally cleaner and directly avoids `#13978`-style forcing artifacts.
3. **No mandatory GPU-walked typed tool grammars on Inky by default:** Only consider enabling optional tool-call grammar enforcement if Phase 3 (`#13981`) shows measurable malformed-call failure rates that `<|content_tool_error|>` self-correction fails to resolve.
4. **Keep `<|content_tool_error|>` (`200022`) and variable loop depth (`K=1..16`):** Both are genuine Inkling 2.0 native capabilities that Psionic + Inky already leverage and Locals lacks.
