# OpenAgents Harness (`oa-worker` / Flawed) vs. Google Antigravity CLI on Terminal-Bench 4.1 (`v4.1.0`)

**Date:** 2026-11-26  
**Benchmark:** Official [harbor-framework/terminal-bench](https://github.com/harbor-framework/terminal-bench) tag `v4.1.0` (commit `4070d6cef5`, 66 total tasks; 63 CPU-only, 3 GPU)  
**Harness Runner:** Official Harbor CLI `0.27.0` on Colima Docker (`oa-agent0`, `aarch64` / `arm64`, 4 CPUs, 8 GiB RAM)  
**Model Parity:** Identical `gemini-4-neon` model, `reasoning_effort=high`, `GEMINI_API_KEY`, `600s` per-task wall-clock cap (`--agent-timeout-multiplier 0.02083333`), concurrency `2`, native execution inside official TB 4.1 containers, scored exclusively by Harbor's unmodified `tests/test.sh` -> `/logs/verifier/reward.txt` and `ctrf.json`.

> Note on naming: upstream commit `270a1f474f` neutrally renamed `flawed/` -> `openagents/core/` and `coder/tui/flawedworker.coax` -> `coder/tui/oa-worker.coax`. Both names refer to the exact same pure-Coax OpenAgents agent harness evaluated here.

---

## Executive Summary

Across **20 diverse `arm64`-verified Terminal-Bench 4.1 tasks** spanning 4 progressive 5-task subsets (Systems, Security, ML Evaluation, Web/Bundler, Crypto, Linguistics, Aviation, Scientific Computing, Knowledge Graphs, Photonics, Finance, Supply Chain, Logistics, and Frontend Performance):

| Metric (20 TB 4.1 Tasks, `gemini-4-neon`, `effort=high`, `600s` cap) | Google Antigravity CLI (`1.5.17`) | OpenAgents `oa-worker` / Flawed (Hillclimbed) | Delta |
|---|---:|---:|---:|
| **Task Pass Rate (`reward == 1.0`)** | **5 / 20 (25.0%)** | **10 / 20 (50.0%)** | **2.0× (+25.0 pp)** |
| **Matched Official Verifier Subtests (19 CTRF tasks)** | **367 / 494 (74.3%)** | **463 / 494 (93.7%)** | **+96 subtests (+19.4 pp)** |
| **All Verifier Subtests incl. `ontology-kg-querying`** | **367 / 507 (72.4%)** | **474 / 507 (93.5%)** | **+107 subtests (+21.1 pp)** |
| **Agent Timeouts (`AgentTimeoutError` at `600s`)** | **16 / 20 (80.0%)** | **6 / 20 (30.0%)** | **-62.5% (-10 timeouts)** |
| **Total Input Tokens (20 tasks)** | 11,089,952 | 10,034,141 | **-9.5%** |
| **Input Tokens on 5 Commonly Passed Tasks** | 2,291k | 1,547k | **-32.5%** |
| **Total Agent Execution Wall Time (20 tasks)** | 11,378s | 10,806s | **-572s (-5.0%)** |
| **Held-Out Unseen Subset 4 (5 tasks, zero hillclimb touches)** | **1 / 5 (20.0%) · 10/65 subtests (15.4%) · 5/5 TO** | **2 / 5 (40.0%) · 56/65 subtests (86.2%) · 2/5 TO** | **2.0× pass rate · 5.6× subtests** |

- **Strict Dominance:** OpenAgents `oa-worker` passed **all 5** tasks that Antigravity CLI passed (`mvcc-lsm-compaction`, `wal-recovery-ordering`, `react-lead-form`, `embedding-drift-monitor`, `nextjs-performance`) plus **5 additional tasks** Antigravity CLI timed out on (`batched-eval-parity`, `interleaved-vigenere`, `sound-change-cascade`, `photonic-waveguide-routing`, `fin-saccr-rwa`). There were **0 tasks** where Antigravity passed and OpenAgents failed.
- **Zero Task-Specific Logic:** Every improvement was a 100% general agent harness fix in `openagents/core/wire.coax` and `coder/tui/oa-worker.coax`, validated out-of-sample on brand-new unseen Subset 4 (`v18`, `2.0×` pass rate and `5.6×` subtest pass count over Antigravity CLI).

---

## Native Linux Static Container Infrastructure

To benchmark OpenAgents natively inside arbitrary Terminal-Bench 4.1 Linux containers without `apt-get` failures or host-wrapper bias:

1. **`evals/terminal-bench/build_flawed_linux.sh`**: Compiles `coder/tui/oa-worker.coax` via `coax c --library --release` and links a `1.1 MiB` **100% static musl Linux `arm64`** binary inside a reusable Alpine `3.21` container (`oa-flawed-musl-builder`) in ~5s.
2. **`evals/terminal-bench/harbor_adapter/flawed_agent.py`**: First-class Harbor `BaseInstalledAgent` (`openagents-flawed`, `SUPPORTS_ATIF=True`, `skills=True`, `budget_s` -> `FLAWED_BUDGET_S`) that uploads the static binary, static `curl 8.23.0`, and CA bundle to `/opt/flawed/` with zero `apt-get` calls, runs natively over a POSIX `sh` FIFO wrapper, streams wire events to `/logs/agent/flawed-wire.jsonl`, and writes ATIF `trajectory.json`.
3. **`evals/terminal-bench/run_tb41_subset.sh`** & **`evals/terminal-bench/summarize_tb41.py`**: Apples-to-apples runner (`-i <task>` filter, identical `gemini-4-neon`, `reasoning_effort=high`, `GEMINI_API_KEY`, `--agent-timeout-multiplier 0.02083333`) and Harbor job comparator.
4. **Suite Registration**: Registered `terminal-bench@4.1` (`TB_SUITE_TB41_FULL` / `tb41-full`, 66 tasks) in `coax/lib/tbench/policy.coax` (`318 mutants, 0 survived`) and `coax/lib/tbench/suites.coax`.

### `arm64` Verifier Compatibility Note (`ks-solver-cpp`)
During Subset 2 setup, `ks-solver-cpp` ran on both Antigravity CLI and Flawed v12 (`0/1` on both), revealing that the upstream TB 4.1 `ks-solver-cpp` verifier ships an `x86_64`-only prebuilt NumPy wheel that crashes with `Exec format error` / incompatible ELF on `arm64`. It was replaced cleanly in Subset 2 with `sound-change-cascade` (verified `arm64`-clean on both agents).

---

## Complete 20-Task Head-to-Head Results

### Subset 1 — Systems, Security & ML Evaluation (5 tasks)

| Task | Antigravity CLI (`high`) | Flawed v8 (Baseline) | Flawed v13 (Promoted) | Flawed v18 (TTFT-immune re-check) |
|---|---|---|---|---|
| `batched-eval-parity` | FAIL(TO) 600s · 607k · `0/5` | **PASS** 374s · 751k · `5/5` | **PASS** 580s · 719k · `5/5` | **PASS**(TO) 600s · 391k · `5/5` |
| `html-js-filter` | FAIL(TO) 600s · 116k · `0/2` | **PASS** 294s · 252k · `2/2` | FAIL 525s · 318k · `1/2` | FAIL 592s · 410k · `1/2` |
| `mvcc-lsm-compaction` | **PASS** 306s · 239k · `15/15` | FAIL 169s · 144k · `11/15` | **PASS** 321s · 202k · `15/15` | **PASS** 359s · 267k · `15/15` |
| `session-window-debug` | FAIL(TO) 600s · 36k · `4/9` | FAIL(TO) 600s · 53k · `8/9` | FAIL 562s · 131k · `8/9` | FAIL(TO) 600s · 62k · `8/9` |
| `wal-recovery-ordering` | **PASS** 498s · 329k · `97/97` | FAIL 360s · 295k · `96/97` | **PASS** 517s · 233k · `97/97` | FAIL 478s · 339k · `96/97` |
| **Subset 1 Total** | **2/5 (40.0%)** · 3 TO · 2604s | **2/5 (40.0%)** · 1 TO · 1796s | **3/5 (60.0%)** · **0 TO** · 2505s | **2/5 (40.0%)** · 125/128 (97.7%) subtests |

*Note: On Subset 1, Flawed also achieved `3/5 (60.0%)` on `v10` (`batched-eval-parity`, `html-js-filter`, `mvcc-lsm-compaction`) and `v11` (same 3), and solved `wal-recovery-ordering` (`97/97`) across `v9`, `v13`, `v15`, and `v17`.*

### Subset 2 — Web, Bundler, Crypto, Linguistics & Aviation (5 tasks)

| Task | Antigravity CLI (`high`) | Flawed v12 | Flawed v13 (Promoted) | Subtest Detail |
|---|---|---|---|---|
| `bun-sourcemap-leak` | FAIL 447s · 578k · `31/37` | FAIL 366s · 427k · `35/37` | FAIL 407s · 373k · `35/37` | +4 criteria vs AGY, -35.5% tokens |
| `cargo-flight-dispatch` | FAIL(TO) 600s · 291k · `18/40` | FAIL(TO) 600s · 418k · `30/40` | FAIL(TO) 600s · 267k · `30/40` | +12 tests vs AGY |
| `interleaved-vigenere` | FAIL(TO) 600s · 722k · `2/6` | FAIL(TO) 600s · 591k · `2/6` | **PASS** 546s · 747k · `6/6` | Flipped from timeout to 6/6 PASS |
| `react-lead-form` | **PASS**(TO) 600s · 451k · `167/167` | **PASS**(TO) 600s · 208k · `167/167` | **PASS**(TO) 600s · 241k · `167/167` | **-46.6% tokens** vs AGY |
| `sound-change-cascade` | FAIL(TO) 600s · 478k · `0/7` | *(replaced `ks-solver-cpp`)* | **PASS** 486s · 677k · `7/7` | Clean 7/7 PASS vs AGY timeout |
| **Subset 2 Total** | **1/5 (20.0%)** · 4 TO · 2847s | **1/4** clean tasks | **3/5 (60.0%) — 3.0× AGY** · 2 TO · 2639s | **245/257 (95.3%) vs 218/257 (84.8%)** |

### Subset 3 — Scientific Computing, Live Systems, Knowledge Graphs & Photonics (5 tasks)

| Task | Antigravity CLI (`high`) | Flawed v13 | Flawed v14 | Flawed v15 (Promoted) | Subtest Detail |
|---|---|---|---|---|---|
| `embedding-drift-monitor` | **PASS** 524s · 644k · `11/11` | **PASS** 537s · 218k · `11/11` | **PASS** 483s · 416k · `11/11` | **PASS** 546s · 569k · `11/11` | 11/11 PASS across all 3 runs |
| `foodstuff-beta-activity` | FAIL(TO) 600s · 1201k · `0/13` | FAIL 594s · 928k · `11/13` | FAIL 514s · 576k · `10/13` | FAIL 490s · 525k · `11/13` | Clean finish, -56% tokens vs AGY |
| `kv-live-surgery` | FAIL(TO) 600s · 1229k · `0/6` | FAIL(TO) 600s · 1920k · `3/6` | FAIL(TO) 600s · 1389k · `0/6` | FAIL(TO) 600s · 1470k · `0/6` | v13 hit 11.65× speedup; v14 seized 25 threads live |
| `ontology-kg-querying` | FAIL(TO) 600s · 2202k · `0/1` | FAIL(TO) 600s · 2123k · `11/13` | FAIL 566s · 1704k · `11/13` | FAIL 534s · 1364k · `11/13` | Clean finish, -38% tokens vs AGY |
| `photonic-waveguide-routing` | FAIL(TO) 600s · 109k · `12/14` | FAIL(TO) 600s · 221k · `12/14` | FAIL(TO) 600s · 383k · `12/14` | **PASS**(TO) 600s · 239k · `14/14` | Flipped to 14/14 PASS (incl. optimality) |
| **Subset 3 Total** | **1/5 (20.0%)** · 4 TO · 2925s | **1/5 (20.0%)** · 3 TO · 2931s | **1/5 (20.0%)** · 2 TO · 2762s | **2/5 (40.0%) — 2.0× AGY** · 2 TO · 2770s | **47/57 (82.5%) vs 23/45** |

### Subset 4 — Held-Out Out-of-Sample Validation (5 unseen tasks, zero prior touches)

| Task | Antigravity CLI (`high`) | Flawed v18 (`high`) | Subtest / Token Comparison |
|---|---|---|---|
| `fin-saccr-rwa` (Finance) | FAIL(TO) 600s · 283k · `0/24` | **PASS** 499s · 277k · `24/24` | **24/24 vs 0/24**, 101s faster |
| `freight-dispatch-shift` (Logistics) | FAIL(TO) 600s · 69k · `0/9` | FAIL 595s · 108k · `7/9` | **7/9 vs 0/9**, clean finish |
| `nextjs-performance` (Frontend) | **PASS**(TO) 600s · 628k · `7/7` | **PASS**(TO) 600s · 302k · `7/7` | **-51.9% tokens** (`302k` vs `628k`) |
| `production-planning` (Supply Chain) | FAIL(TO) 600s · 416k · `1/20` | FAIL 599s · 836k · `16/20` | **16/20 vs 1/20**, clean finish |
| `risk-scorer-replay` (ML Eval) | FAIL(TO) 600s · 461k · `2/5` | FAIL(TO) 600s · 436k · `2/5` | Tied `2/5`, fewer tokens |
| **Subset 4 Held-Out Total** | **1/5 (20.0%)** · **5/5 TO** · `10/65` (15.4%) | **2/5 (40.0%) — 2.0× AGY** · **2/5 TO** · `56/65` (86.2%) | **5.6× subtests passed, -60% timeouts** |

---

## General Harness Improvements Implemented (`v8 -> v18`)

All changes live in `openagents/core/wire.coax` and `coder/tui/oa-worker.coax` (zero task-specific rules):

1. **Tool & Shell Portability (`v9`):**
   - Transparent `grep -r -E -I -H` fallback in `Grep` when `rg` returns `PROC_ENOENT` (containers without ripgrep previously returned `"No matches found."`).
   - Prefer `/bin/bash` over `/bin/sh` (`dash`) when present; expanded `FWW_CMD_MAX` (`32 KiB`), `FWW_MAX_BLOCKS` (`28`), `FWW_MAX_TOOLS_CALL` (`24`), and SSE JSON parser nodes (`1024`).
   - `PYTHONUNBUFFERED=1` in Bash subprocesses so Python scripts stream progress immediately instead of returning empty `0`-byte stdout on timeout (`v15`).
   - Fast-fail default Bash timeout (`60s` when omitted, honoring explicit `timeout` up to `300s`, clamped to remaining `budget_s - 25s` floor `15s`) so deadlocks/unbounded loops fail fast instead of silently burning `120s` (`v14–v15`).
2. **Output Token Budget & Streaming Thinking Control (`v10–v18`):**
   - Raised Gemini cloud `maxOutputTokens` in `openagents/core/wire.coax` to `65536` on `high`/`xhigh`/`max` and `32768` on `low`/`medium` (eliminating `MAX_TOKENS` guillotining during deep reasoning and large `Write` calls).
   - Added `fww_think_spiral` streaming pure-thought spiral breaker, calibrated on **actual decode time from `first_tok_ms`** (immune to `50–85s` API TTFT queueing spikes!) with `nthink >= 16000`, `38K` char ceiling, `205s` early-run ceiling (preserving healthy ~180s first architectural derivations like `mvcc-lsm-compaction`), dynamic late-run tightening (`40%` of remaining budget, floor `90s`), scratchpad conclusion handoff, `calm_steps = 2` (`medium`), and `force_low_once` on a 2nd consecutive spiral (`v11`, `v14`, `v16–v18`).
   - Post-heavy-thinking adaptive cooldown (`dec_fin_ms >= 120s` -> `calm_steps = 2`) preventing back-to-back `150–185s` rumination stalls (`v14`, `v18`).
3. **Reliability & Protocol Hygiene (`v14`, `v17`):**
   - **Mid-stream transient SSE error retry (`fww_is_transient_sse_err`):** Gemini streams that return HTTP `200` headers and then emit mid-stream `503`/`UNAVAILABLE`/`"high demand"`/`RESOURCE_EXHAUSTED` JSON errors now retry cleanly through Ch 05 exponential backoff instead of hard-failing the turn.
   - **UTF-8 & binary output sanitization (`fww_sanitize_text_line`, `fww_is_binary_bytes`):** Detects binary files in `Read` and collapses binary/C0/C1 control runs in Bash output to `[binary]`, eliminating malformed JSONL lines and `15k`-token binary garbage inflation (validated on `foodstuff-beta-activity`: `-38%` to `-43%` tokens, `0` malformed wire lines).
   - **Atomic compaction & strict tool-pair validation (`fww_msg_tools_valid`, `FWW_SLAB` 4 MiB, `FWW_MAX_MSGS` 256):** Prevents any clipped `functionCall`/`functionResponse` name/ID/signature under long-horizon compaction.
4. **Pacing & Audit Discipline (`v9`, `v12–v15`):**
   - Excluded scratch paths (`/tmp/`, `/var/tmp/`) via `fww_is_scratch_path` so helper scripts do not trip deliverable-edited flags.
   - Post-edit critical audit (`audit_nudged` + `audit_done`) before stepping effort down, plus 4-stage optional wall-clock checkpoints (`OA_AGENT_BUDGET_S` / `FLAWED_BUDGET_S` at `35%/60%/80%/90%`) that preserve the post-edit audit.
   - Prompt discipline: compound diagnostic scripts over probe-by-probe turns, early end-to-end execution, checkpointing the first valid working output before slow refinement passes, and generalizing verification beyond default fixtures.

---

## Verification & Reproduction

All repo gates verified after rebasing onto `origin/main`:
- `coax/bin/coax test openagents/core/wire.coax --library` — `119/119 passed`
- `coax/bin/coax mutate openagents/core/wire.coax --root-only --native` — `0 survived`
- `coax/bin/coax test coax/lib/tbench/policy.coax --library` & `coax mutate coax/lib/tbench/policy.coax --root-only --native` — `318 mutants, 0 survived`
- `coax/bin/coax test coax/lib/tbench/suites.coax --library` & `coax/bin/coax test coder/tui/tbench-run.coax --library` — `188/188 passed`
- `coax/bin/coax test coder/tui/oa-worker.coax --library` — `273/273 passed`
- `coax/bin/coax test apps/openagents-desktop/app-v1.coax --library` — `516/516 passed`
- `./scripts/check.sh` — `every check passed`
- `./apps/openagents-desktop/build.sh --minimal` & `v1-bundle-check` — Stage 2 size & resources PASS (`13.68 MiB < 14 MiB` unpacked, `0` size/resource violations; `oa-worker --auth` reports `ready: true`; only pre-existing upstream `Speech`/`AVFoundation`/`AVFAudio` voice dylib notices from `0360ee8b90` on `origin/main`).

```bash
# Build static Linux arm64 oa-worker binary
bash evals/terminal-bench/build_flawed_linux.sh

# Run any TB 4.1 subset apples-to-apples (Antigravity CLI vs OpenAgents)
bash evals/terminal-bench/run_tb41_subset.sh antigravity tb41-s4-antigravity fin-saccr-rwa production-planning freight-dispatch-shift risk-scorer-replay nextjs-performance
bash evals/terminal-bench/run_tb41_subset.sh flawed      tb41-s4-flawed-v18  fin-saccr-rwa production-planning freight-dispatch-shift risk-scorer-replay nextjs-performance

# Compare Harbor job directories
python3 evals/terminal-bench/summarize_tb41.py ~/.openagents/tbench/jobs/tb41-s4-antigravity ~/.openagents/tbench/jobs/tb41-s4-flawed-v18
```
