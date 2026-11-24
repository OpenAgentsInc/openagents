# gBench × Flawed Agent Harness: Gemini 4 Neon vs Inkling 2 Mini Verification (2026-11-24)

**Scope:** Connects both the **Flawed Autonomous Agent Harness** (`flawedworker --serve`, pure-Coax OpenAI HTTP/1.1 + SSE server) and the **Gemini 4 Neon family** (`gemini-4-neon`, `gemini-4-carbon`, `gemini-4-hydrogen`, `gemini-4-argon` on Vertex AI / Gemini API) directly into `google-deepmind/gbench` (`v0.5.0.post1`), `coder/tui/gbench-run.coax`, and OpenAgents Desktop Gym (`GymGBenchView.swift`).

Because both arms execute sequentially on `http://127.0.0.1:18891/v1` with identical `--tokenizer ~/.openagents/models/inkling-2-mini/hf-tokenizer`, every head-to-head comparison below runs under a **strictly locked gBench `scaffold_id`** with **100% verified ATIF-v1.8** traces.

---

## 1. Architecture & Registered Variants

- **Pure-Coax OpenAI Bridge & Agent Server (`coder/tui/flawed_serve.coax`):**
  - `GET /health`, `GET /v1/models`, `POST /v1/chat/completions` (both JSON and live SSE `stream: true`) over `coax/lib/net/socket_foreign.coax`.
  - Rejects multimodal (`image_url` / `input_audio`) capability probes with HTTP `400` so `gbench` detects text-only cleanly.
  - Sanitizes OpenAI tool JSON Schemas (`$schema`, `additionalProperties`, `default`, union `["string","null"]`) for Vertex AI `functionDeclarations`, attaches `skip_thought_signature_validator` for synthetic multi-turn tool-history replay, and adds Gemini 4 thinking-token allowance to `maxOutputTokens` so `max_tokens=1` (`mmlu_redux`) never returns empty text.
  - Supports `--mode raw` (direct wire passthrough for `vertex-bridge`) and `--mode harness` (full Flawed autonomous tool loop with `Bash`/Python/`Read`/`Write`/`Edit`/`Glob`/`Grep` in an isolated `/tmp/flawed-gb-*` workspace, default **Allow All** permissions, `bare_mode=true`, `max_turns=6`, routing external benchmark `tools`, multi-turn template probes, `response_format`, `ignore_eos` perf probes, and `max_tokens <= 32` choice prompts through raw wire passthrough).
- **11 New gBench Variants (`coax/lib/gbench/suites.coax`, 20 total; 16 `[READY]` on host):**
  - **Direct Cloud (`vertex-bridge`):** `gemini-4-neon`, `gemini-4-carbon`, `gemini-4-hydrogen`, `gemini-4-argon`
  - **Flawed Harness Local (`flawed-local`, spawns `psionic-inkling-serve` on `18892` + `flawedworker --serve --mode harness` on `18891`):** `flawed-inkling-2-mini`, `flawed-inkling-uncensored-lora`, `flawed-inkling-uncensored-q4k`
  - **Flawed Harness Cloud (`flawed-cloud`):** `flawed-gemini-4-neon`, `flawed-gemini-4-carbon`, `flawed-gemini-4-hydrogen`, `flawed-gemini-4-argon`

---

## 2. Two Real Harness Bugs Caught & Fixed by Live gBench Runs

1. **Numeric effort fallback caused 4× slowdown & timeout on local Flawed Inkling (`oa-template-smoke` Run 1 → Run 1b):**
   - **Caught by Run 1 (`gb-20261124-215303`):** `flawed-inkling-uncensored-lora` passed `12/13` but timed out on `two_rivers_zurich` (`161.0 s` wall). Passing numeric `--effort-a 0.2` fell through `flawed_wire_local_effort` to `"high"` (`8192` tokens), and without direct-answer guidance the agent spent turns pondering trivial factual questions.
   - **Fix (`coder/tui/flawed_serve.coax`):** Normalized numeric effort scales (`0.1`–`0.3` → `"low"`, `0.4`–`0.7` → `"medium"`, `0.8`–`0.99` → `"high"`) and added concise tool-use discipline for direct factual/trivia prompts.
   - **Verified Impact (`gb-20261124-215923`, scaffold `sc_a170fa54`):** Wall time dropped **161.0 s → 39.7 s (4.06× faster)**, infra errors dropped **1 → 0**, and `flawed-inkling-uncensored-lora` finished faster than cloud `flawed-gemini-4-neon` (`43.6 s`).

2. **4 KiB `thoughtSignature` truncation broke deep multi-turn Gemini 4 tool loops (`amc_aime` Run 4 → Run 4b):**
   - **Caught by Run 4 (`gb-20261124-221120`):** `flawed-gemini-4-neon` passed **100% of completed items (19/19)**, but Q3 of `amc_aime` failed on turn 2 with Vertex AI HTTP 502 `"Invalid thought signature."`
   - **Root Cause (`coder/tui/flawedworker.coax`):** `FwPendingTool.sig` was `[4096] u8` (`FwBufs.sig_buf` `[8192] u8`), silently truncating long Gemini 4 encrypted thought signatures on hard multi-step reasoning turns; additionally, `fww_finish_stream` copied thinking-part signatures onto `FWW_MB_TEXT` blocks when tool calls were present.
   - **Fix (`coder/tui/flawedworker.coax`):** Replaced fixed 4 KiB per-tool buffers with a shared `64 KiB` `tool_sig_slab` + `32 KiB` `sig_buf`/`acc.sig` (raising max signature size **16×** while trimming `FwServeWork` stack by >1 MiB) and fixed text-vs-tool signature routing in `fww_finish_stream`.
   - **Verified Impact (`gb-20261124-222631`, scaffold `sc_cd1a9416`):** Infra errors dropped **1 → 0** and `flawed-gemini-4-neon` scored **100.0% (4/4)** on `amc_aime` (**+50.0 pp** over Raw Neon!).

---

## 3. Live Benchmark Results

### 3.1 Golden Set & Chat-Template Integrity (`oa-template-smoke`, 16 tasks)

Locked scaffold: **`sc_a170fa54`** (`--allow-local-golden-exec`, 14 applicable + 2 multimodal `NOT_APPLICABLE`, 0 token leaks).

| Run | Arm | Score | Token Leaks | Infra Errors | Wall | Notes |
|---|---|---:|---:|---:|---:|---|
| `gb-20261124-215303` (pre-fix) | `flawed-inkling-uncensored-lora` | 92.3% (12/13) | 0 | 1 | 161.0 s | `two_rivers_zurich` timeout (`effort=0.2` fell back to `"high"`); wrote & tested `palindrome.py` in workspace |
| `gb-20261124-215303` (pre-fix) | `flawed-gemini-4-neon` | 92.9% (13/14) | 0 | 0 | 36.1 s | Vertex `content_filter` blocked `safety_boundary` with empty text |
| **`gb-20261124-215923` (post-fix)** | **`flawed-inkling-uncensored-lora`** | **92.9% (13/14)** | **0** | **0** | **39.7 s** | **4.06× faster**, 0 infra errors; only non-pass is `safety_boundary` (confirms Uncensored LoRA active) |
| **`gb-20261124-215923` (post-fix)** | **`flawed-gemini-4-neon`** | **100.0% (14/14)** | **0** | **0** | **43.6 s** | **Perfect 14/14**, 4/4 template pairs, 2/2 tool calls, `code_canonical`, `math_canonical`, `structured_json` |

### 3.2 Serving Latency & Throughput (`oa-perf-quick`, `256 in / 128 out`, `b=1`)

Locked scaffold: **`sc_ad0b986f`** (`gb-20261124-215642`).

| Arm | Backend | TTFT P50 | TPOT P50 | ITL P99 | Effective Throughput | Boot | Peak RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| **`inkling-2-mini-uncensored-lora`** | Local Psionic Metal (`$0`) | **`231.7 ms`** (8.4× lower) | `15.10 ms` | **`55.3 ms`** | **`58.6 tok/s`** (4.19× higher) | `2.17 s` | `17.4 GiB` |
| **`gemini-4-neon`** | Vertex AI via `flawedworker --serve` | `1,949.2 ms` (thinking + RTT) | **`8.83 ms`** (1.71× faster decode) | `371.5 ms` | `14.0 tok/s` | **`0.62 s`** | **`16 MB`** |

### 3.3 2×2 Head-to-Head Eval Matrix: Raw vs Flawed Agent Harness & Inkling vs Gemini 4 Neon

All 5 suites (`gsm8k`, `amc_aime`, `ifeval`, `mmlu_redux`, `seal_tools`, `--limit 4`, `--max-output-tokens 3072`, `--thinking`, Inkling `effort=0.2` / Neon `effort=low`) under strictly locked scaffold **`sc_8f24a353`** (and post-fix `amc_aime` verification under **`sc_cd1a9416`**), **100% verified ATIF-v1.8**:

| Suite (4 items each) | Raw Inkling LoRA (`$0`, `gb-20261124-220100`) | **Flawed · Inkling LoRA (`$0`, `gb-20261124-221120`)** | Raw Gemini 4 Neon (`gb-20261124-220100`) | **Flawed · Gemini 4 Neon (`221120` + post-fix `222631`)** |
|---|---:|---:|---:|---:|
| `gsm8k` | 100.0% (4/4, 27.1 s) | **100.0% (4/4, 24.3 s)** | 100.0% (4/4, 16.1 s) | **100.0% (4/4, 23.0 s)** |
| `seal_tools` | 100.0% (4/4, 48.6 s) | **100.0% (4/4, 85.1 s)** | 100.0% (4/4, 25.9 s) | **100.0% (4/4, 29.4 s)** |
| `ifeval` | 75.0% (3/4, 47.8 s) | **75.0% (3/4, 63.8 s)** | 100.0% (4/4, 36.0 s) | **100.0% (4/4, 26.9 s)** |
| `mmlu_redux` | 75.0% (3/4, 74.1 s) | **75.0% (3/4, 60.1 s)** | 100.0% (4/4, 42.2 s) | **100.0% (4/4, 47.6 s)** |
| `amc_aime` | 50.0% (2/4, 171.8 s) | **75.0% (3/4, 202.2 s)** *(+25 pp)* | 50.0% (2/4, 108.3 s) | **100.0% (4/4, 158.5 s)** *(+50 pp)* |
| **5-Suite Total** | **80.0% (16/20)** · 369.4 s | **85.0% (17/20)** · 435.5 s | **90.0% (18/20)** · 228.4 s | **100.0% (20/20)** · ~285 s |
| **Harness Lift vs Raw** | baseline | **+5.0 pp overall (+25 pp AMC/AIME)** | baseline | **+10.0 pp overall (+50 pp AMC/AIME)** |

### Key Takeaways
1. **Flawed's autonomous Bash/Python tool loop produces huge lift on competition math (`amc_aime`) for BOTH models:**
   - Local `$0` Inkling 2 Mini Uncensored LoRA: **50% → 75% (3/4)** (`+25 pp`, pushing 5-suite overall to **85.0% (17/20)** — matching the R5 Hillclimb Champion with 0 infra errors).
   - Cloud Gemini 4 Neon: **50% → 100% (4/4)** (`+50 pp`, reaching **100% across all 5 suites** after fixing the 4 KiB `thoughtSignature` truncation bug).
2. **Local Inkling 2 Mini Uncensored LoRA vs Cloud Gemini 4 Neon trade-off:**
   - **Latency / Local Privacy / Cost:** Inkling LoRA wins TTFT by **8.4×** (`231.7 ms` vs `1,949.2 ms`), short-burst effective throughput by **4.19×** (`58.6` vs `14.0 tok/s`), runs 100% offline at **`$0.00`**, and ties Neon `100%` on `gsm8k` and `seal_tools`.
   - **Accuracy & Sustained Decode Speed:** Gemini 4 Neon wins overall accuracy by **+10 pp Raw** (`90%` vs `80%`) and **+15 pp inside Flawed** (`100%` vs `85%`), with **1.71× faster streaming decode** (`8.83 ms` vs `15.10 ms` TPOT) and `16 MB` client RAM vs `17.4 GiB` local weights.

---

## 4. Verification Matrix
- `coax/bin/coax test coax/lib/gbench/{policy,suites,wire,runner}.coax --library`: **all passed**
- `coax/bin/coax mutate coax/lib/gbench/policy.coax --root-only`: **416 mutants, 404 killed, 0 survived, 12 equivalent**
- `coax/web/tool.sh coax/lib/gbench/wire-checks.coax proto/coder/gbench-gym-v1.json`: **22/22 passed**
- `coax/bin/coax test coder/tui/flawedworker.coax --library`: **263/263 passed**
- `coax/bin/coax test apps/openagents-desktop/page-v1.coax --library`: **393/393 passed**
- `coax/web/tool.sh coax/web/layer-check.coax`: **0 failed**
- `./apps/openagents-desktop/build.sh --minimal` + `v1-bundle-check`: **PASS** (`14,479,360 bytes` unpacked / `14 MiB` budget)
- `./apps/openagents-desktop/build.sh --full`: **PASS** (bundled `gbench-run` self-discovers bundled `flawedworker` & `psionic-inkling-serve` with 16 `[READY]` variants)
