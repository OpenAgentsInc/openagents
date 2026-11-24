# gBench × OpenAgents Gym: Inkling 2 Mini Baseline & Hillclimb Verification (2026-11-24)

**Scope:** End-to-end integration of `google-deepmind/gbench` (`v0.5.0.post1`, Apache-2.0) into OpenAgents Desktop Gym (`gBench` 5th top-bar pill) and pure-Coax `gbench-run` harness, deprecating GPT-OSS in favor of local Inkling 2 Mini checkpoints on Psionic Metal (`llama-server` as comparator). All benchmark runs below executed locally at `$0.00` against real GGUF/LoRA checkpoints in `~/.openagents/models/`.

---

## 1. Environment & Harness Readiness

| Component | Verified Value |
|---|---|
| Upstream gBench | `google-deepmind/gbench` `v0.5.0.post1` (commit `d3e4283`), installed editable into `~/.openagents/gbench/venv` (Python `3.12.13`) |
| Offline HF Tokenizer | `~/.openagents/models/inkling-2-mini/hf-tokenizer/` extracted directly from `Inkling-2-Mini-IQ4_NL.gguf` (200,058 tokens, 424,413 merges, 16 special tokens, official Jinja `chat_template`) |
| Psionic Runtime | `psionic/build/psionic-inkling-serve` + `inkling-kernels.metal` (patched for `/tokenize`, `/detokenize`, `/metrics`, `return_token_ids`, multimodal HTTP 400 rejection, `enable_thinking=false` enforcement, `INK_DEFAULT_EFFORT`, and no-think text-header prefill) |
| Comparator Runtime | `~/.openagents/bin/llama-server` (`--jinja`, Metal `-ngl 99`) |
| `#authority` Policy | `coax/lib/gbench/policy.coax` — **416 mutants, 404 killed, 0 survived, 12 equivalent** |
| Wire Schemas | `proto/coder/gbench-gym-v1.json` (5 schemas in `proto/registry.json`) — **22/22 wire-check vectors passed** |
| Installed Checkpoints | `inkling-2-mini` (`IQ4_NL`, 16.4 GiB), `inkling-2-mini-uncensored-lora` (`IQ4_NL + 42 MB LoRA`), `inkling-2-mini-uncensored-q4k` (`AD-Q4_K`, 15.4 GiB) |

---

## 2. Pillar 1 — Golden Set & Chat-Template Integrity (`oa-template-smoke`)

All runs executed under strictly locked gBench scaffold **`sc_a170fa54`** (`--golden-only`, `--allow-local-golden-exec` on macOS, 14 applicable cases + 2 multimodal `NOT_APPLICABLE`, 17-token `chat_template_leak_check`).

| Variant | Backend | Run ID | Golden Pass | Token Leaks | Boot | Peak RSS | Wall | Notes |
|---|---|---|---:|---:|---:|---:|---:|---|
| `inkling-2-mini` (IQ4_NL) | Psionic Metal | `gb-20261124-074916` | **13 / 14 (92.9%)** | **0** | 2.69 s | 17.4 GiB | **26.1 s** | Refused `safety_boundary` cleanly (`I can’t help...`); failed ASCII substring check only due to Unicode apostrophe `U+2019`. |
| `inkling-2-mini-uncensored-lora` | Psionic Metal | `gb-20261124-074916` | **13 / 14 (92.9%)** | **0** | 2.37 s | 17.5 GiB | 34.2 s | Confirmed LoRA active (answered `safety_boundary` instead of refusing); 0 leaks, 4/4 template pairs & 2/2 tool calls passed. |
| `inkling-2-mini-uncensored-q4k` | Psionic Metal | `gb-20261124-064021` | 13 / 13 graded (1 timeout) | **0** | 6.96 s | 1.44 GiB | 253.7 s | `safety_boundary` hit HTTP timeout due to `8.2 tok/s` fallback on `Q4_K`/`Q5_0` tensors (see §3). |
| `llamacpp-inkling-2-mini` | `llama.cpp` Metal | `gb-20261124-064021` | 13 / 14 (92.9%) | **0** | 1.58 s | 19.3 GiB | 62.1 s | 2.38× slower wall time than Psionic Metal on the identical IQ4_NL GGUF. |

---

## 3. Pillar 2 — Serving Latency & Throughput (`oa-perf-quick`)

Executed with gBench official local single-stream profile `--personal --text-only --input-lengths 256 --output-lengths 128` (12 prompts + 1 warmup, `batch_size=1`), strictly locked under scaffold **`sc_ad0b986f`**.

| Variant | Backend | Run ID | TTFT P50 | TPOT P50 | Throughput | Wall (12 req) | Speedup vs `llama.cpp` |
|---|---|---|---:|---:|---:|---:|---:|
| **`inkling-2-mini` (post prefill-header fix)** | **Psionic Metal** | `gb-20261124-075021` | **`243.4 ms`** | **`11.76 ms`** | **`73.3 tok/s`** | **`26.3 s`** | **2.17× TTFT / 2.23× tok/s** |
| `inkling-2-mini` (pre-fix baseline) | Psionic Metal | `gb-20261124-070422` | `274.1 ms` | `10.96 ms` | `76.1 tok/s` | `25.7 s` | 1.54× TTFT / 2.07× tok/s |
| `inkling-2-mini-uncensored-lora` | Psionic Metal | `gb-20261124-070851` | `288.9 ms` | `14.42 ms` | `61.0 tok/s` | `31.3 s` | 1.46× TTFT / 1.66× tok/s |
| `llamacpp-inkling-2-mini` | `llama.cpp` Metal | `gb-20261124-075021` | `528.4 ms` | `26.46 ms` | `32.9 tok/s` | `55.7 s` | 1.00× (baseline comparator) |
| `inkling-2-mini-uncensored-q4k` | Psionic Metal | `gb-20261124-070851` | `2066.4 ms` | `108.03 ms` | `8.2 tok/s` | `212.3 s` | 0.22× (missing native `Q4_K`/`Q5_0` Metal fast path) |

### Key Hardware / Kernel Finding
Inspecting the GGUF tensor tables directly explains the 9× throughput gap on `AD-Q4_K`:
- `Inkling-2-Mini-IQ4_NL.gguf`: `{'Q8_0': 186, 'F32': 478, 'IQ4_NL': 40}` — 100% hits Psionic Metal's native `IQ4_NL + Q8_0` fast kernels (**73–77 tok/s**).
- `Inkling-2-Mini-Uncensored-AD-Q4_K.gguf`: `{'F32': 334, 'Q8_0': 322, 'Q5_0': 24, 'Q4_K': 24}` — the 48 `Q4_K`/`Q5_0` MoE tensors fall off the native Metal fast path (**8.2 tok/s**).
- Applying the 42 MB **Uncensored LoRA overlay** onto `IQ4_NL` preserves native Metal kernels and delivers **61.0 tok/s** (**7.4× faster** than `AD-Q4_K` and **1.66–1.85× faster** than `llama.cpp` on base IQ4_NL).

---

## 4. Pillar 3 — Deterministic Local Evals & Empirical Serving Fix

### 4.1 Discovery & Root-Cause Fix (`psionic/platform/inkling-serve.c`)
Our initial 60-sample no-think sweep (`gb-20261124-071943`, scaffold `sc_22575691`) scored **50.0% (30/60)** on Base and **48.3% (29/60)** on LoRA, immediately exposing a major serving defect: **`mmlu_redux` scored `0.0% (0/10)`** on both Psionic and `llama.cpp`!

- **Root Cause:** HELM / `mmlu_redux` sends `max_tokens=1` with `chat_template_kwargs={"enable_thinking": false}`. Because Inkling's official Jinja template terminates prompts at `<|end_message|>` (leaving the assistant turn un-opened), generation spent token #1 emitting the structural channel marker `<|message_model|>` (`id=200001`), exhausting `max_tokens=1` before emitting a single visible answer token.
- **Fix (`psionic/platform/inkling-serve.c`):** When `no_think && r.tools_json.len == 0 && !r.continue_final_message && !use_js`, pre-seed `<|message_model|><|content_text|>` onto the chat-completion prompt and initialize `channel = 2` (leaving `POST /tokenize` 100% bit-exact with Jinja).
- **Verified Impact (`gb-20261124-074738`, scaffold `sc_5a62a691`):**
  - `mmlu_redux` (`--limit 10`): **0.0% → 70.0% (7/10)** on patched Psionic Metal (+70.0 pp), while `llama-server --jinja` remained **0.0% (0/10)** on the identical GGUF.
  - `oa-perf-quick` TTFT P50 dropped **274.1 ms → 243.4 ms** (**-11.2%**) by eliminating 2 serial decode steps before first visible token.
  - `oa-template-smoke` Golden Set remained **13/14 with 0 token leaks**.

### 4.2 Post-Fix No-Think Variant Comparison (`oa-local-fast`, `--limit 5`)

| Variant (No-Think) | Run ID / Scaffold | `gsm8k` | `ifeval` | `mmlu_redux` | `seal_tools` | `amc_aime` | `arc_agi` | Overall | Decode Speed |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `inkling-2-mini` (Psionic IQ4_NL) | `gb-20261124-075206` (`sc_93f085d3`) | 100.0% | 80.0% | 60.0% | 96.2% | 40.0% | 0.0% | **60.0% (18/30)** | **67.4 tok/s** |
| `inkling-2-mini-uncensored-lora` | `gb-20261124-075206` (`sc_93f085d3`) | 100.0% | 80.0% | **80.0%** | **98.1%** | 40.0% | 0.0% | **63.3% (19/30)** | 56.6 tok/s |
| `inkling-2-mini-uncensored-q4k` | `gb-20261124-080118` (`sc_a4f9e507`) | 100.0% | 80.0% | 60.0% | — | — | — | 80.0% (12/15) | 9.1 tok/s |
| `llamacpp-inkling-2-mini` (`llama-server`) | `gb-20261124-080118` (`sc_a4f9e507`) | 60.0% | 20.0% | 0.0% | — | — | — | **26.7% (4/15)** | 39.7 tok/s |

Psionic Metal outperformed `llama-server --jinja` by **+53.3 pp** (80.0% vs 26.7%) on the identical base GGUF across `gsm8k + ifeval + mmlu_redux` because `llama-server` lacks native Inkling `<|content_thinking|>` / `<|content_text|>` channel parsing and fails `enable_thinking=false` / `max_tokens=1`.

---

## 5. `#authority`-Gated Hillclimbing Ledger (`gbench-run hillclimb`)

All rounds ran sequentially on `http://127.0.0.1:18891/v1` under strictly locked gBench `scaffold_id`, gated by `coax/lib/gbench/policy.coax` (`gb_admit_hillclimb`, 0 surviving mutants), with automatic license-checked + Ch 247 secret-redacted ATIF-v1.8 distillation into `~/.openagents/gbench/distill/`.

| Round | Run ID | Scaffold | Suites / Graded | Champion → Challenger | Score (Champ → Cand) | TPOT (Champ → Cand) | Verdict | Distilled ATIF |
|---:|---|---|---|---:|---:|---:|---|---:|
| **R1** | `gb-hc-20261124-080814` | `sc_471a4235` | `gsm8k,ifeval,amc_aime` (12)* | `inkling-2-mini` (`e=0.7`) → `inkling-effort-09` (`e=0.9`) | `75.0%` → **`83.3%`** (+8.3 pp) | `14.05` → `14.56 ms` | **PROMOTED** | 2 |
| **R2** | `gb-hc-20261124-083720` | `sc_8f24a353` | 5 suites × 4 (20) | `inkling-effort-09` (`e=0.9`) → `inkling-lora-e09` (`LoRA, e=0.9`) | `70.0%` → **`80.0%`** (+10.0 pp) | `13.42` → `18.55 ms` | **PROMOTED** | 3 |
| **R4** | `gb-hc-20261124-085404` | `sc_8f24a353` | 5 suites × 4 (20) | `inkling-lora-e09` (`e=0.9`) → `inkling-effort-02` (`Base, e=0.2`) | `75.0%` → **`80.0%`** (+5.0 pp) | `17.89` → **`14.25 ms`** | **PROMOTED** | 4 |
| **R5** | `gb-hc-20261124-090808` | `sc_8f24a353` | 5 suites × 4 (20) | `inkling-effort-02` (`Base, e=0.2`) → **`inkling-2-mini-uncensored-lora` (`LoRA, e=0.2`)** | `75.0%` → **`85.0%`** (+10.0 pp) | `14.42` → `17.59 ms` | **PROMOTED** | 4 |

*\*In R1, `arc_agi` hit the 14-minute per-arm cap in thinking mode (`max_output_tokens=3072`), so R2–R5 ran the clean 5-suite thinking profile (`gsm8k,amc_aime,ifeval,mmlu_redux,seal_tools`, `--limit 4`, 20 graded samples/arm).*

### Winning Champion Breakdown (Round 5 — `inkling-2-mini-uncensored-lora`, `thinking=true, effort=0.2, max_output_tokens=3072`)

| Suite | Base IQ4_NL (`effort=0.2`) | **Champion: IQ4_NL + Uncensored LoRA (`effort=0.2`)** | Delta |
|---|---:|---:|---:|
| `gsm8k` | 100.0% (4/4, 74.5 tok/s, 16.7 s) | **100.0% (4/4, 58.8 tok/s, 28.9 s)** | 0.0 pp |
| `ifeval` | 50.0% (2/4, 55.0 tok/s, 35.9 s) | **100.0% (4/4, 57.9 tok/s, 30.4 s)** | **+50.0 pp** |
| `seal_tools` | 100.0% (4/4, 74.0 tok/s, 63.0 s) | **100.0% (4/4, 56.9 tok/s, 66.9 s)** | 0.0 pp |
| `mmlu_redux` | 75.0% (3/4, 57.4 s) | **75.0% (3/4, 72.7 s)** | 0.0 pp |
| `amc_aime` | 50.0% (2/4, 79.6 tok/s, 121.6 s) | **50.0% (2/4, 54.0 tok/s, 171.0 s)** | 0.0 pp |
| **Total** | **75.0% (15/20, 294.6 s)** | **85.0% (17/20, 369.9 s, Wilson 95% [64.0..94.8%])** | **+10.0 pp** |

### Key Hillclimbing Insights
1. **Low-effort thinking (`effort=0.2`) dominates `effort=0.9` on tool-use & speed:** At `effort=0.9`, over-thinking caused `seal_tools` to drop to `48.3%–58.1%` (`138 s` wall); lowering to `effort=0.2` restored **`100.0% (4/4)`** on `seal_tools` in `41.5–66.9 s` while cutting 5-suite wall time by **29–45%** (`517.5 s → 286.8–369.9 s`).
2. **Uncensored LoRA + `effort=0.2` is the new local Pareto Champion:** Combining AtomicChat's 42 MB Uncensored LoRA overlay (eliminating `ifeval` over-refusals: `50% → 100%`) with `effort=0.2` pushed 5-suite accuracy from **50.0%** (initial no-think baseline) → **60.0%** (post-fix no-think) → **85.0% (17/20)** with **0 harness errors**, **100% verified ATIF-v1.8**, and **13 license-gated, Ch 247 redacted trajectories** ready for Psionic W3.2 / W5 distillation.
3. **Next high-leverage hillclimb targets:**
   - Implement native Metal `Q4_K` / `Q5_0` MoE kernels in `psionic/platform/inkling-metal.c` (expected 7–9× speedup on `AD-Q4_K` and `AD-IQ3_S` community quantizations).
   - Add per-suite adaptive thinking effort or a thinking-budget ceiling for `amc_aime` / `arc_agi` so hard reasoning suites don't exhaust `max_output_tokens=3072`.
   - Evaluate `INK_SPEC_LOOKUP=1` (axis 5) on `oa-perf-agentic` (long-context multi-turn tool replay where n-gram prompt overlap is high).
