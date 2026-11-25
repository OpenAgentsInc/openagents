# Flawed · Gemini 4 Neon vs. Antigravity · Gemini 4 Neon & Raw Neon: Benchmark, gBench & Custom Workflow Hillclimb

**Date:** 2026-11-25  
**Previous Milestone Commit (Flawed v4):** `d7be8ab43b5c894a8595eb094b867f555584f335` (`origin/main`)  
**Current Champion:** **Flawed v8** (`coder/tui/flawedworker.coax`, `coder/tui/flawed_serve.coax`, `flawed/wire.coax`, `evals/antigravity-workflow/`)  
**Bundle Gate (`--minimal`):** `PASS v1-bundle-check (macos stage 2, unpacked=14,643,200 B / 14 MiB budget)`

---

## 1. One-Sentence Bottom Line

Running the exact same `gemini-4-neon` model at `effort=high` (`300s` per-task cap) across **24 real OpenAgents repository coding tasks** (`dev10` + 6 unseen `heldout6` + 8 custom `workflow8` tasks mined directly from our recent Antigravity chats), **Flawed v8 · Neon achieved `3.60×` base Antigravity · Neon's pass rate (`18/24 = 75.0%` vs. `5/24 = 20.8%`), solved all 5 commonly passed tasks `58.2%` faster (`54.4s` vs. `130.3s` avg, `2.39×`), used `39%–91%` fewer prompt tokens on those tasks, recorded `0/157` edit failures and `0` HTTP/wire retries, and also outperformed raw `gemini-4-neon` on locked-scaffold `google-deepmind/gbench` across 10 hard eval suites (`35/38 = 92.1%` in `1767.8s` vs. `34/38 = 89.5%` in `1988.9s`, `-11.1%` wall time, `0` infra errors, `0` truncations) plus `13/13` Golden Set (`-29.6%` faster).**

---

## 2. Executive Summary (All Three Tracks)

### 2.1 Repo Coding & Real Antigravity-Workflow Tracks (`effort=high`, `300s` cap)

| Metric | Antigravity · Neon (`high`) | Flawed R0 (Baseline) | Flawed v4 (Prev Shipped) | **Flawed v8 (Champion)** | Flawed v8 vs. AGY |
|---|---:|---:|---:|---:|---|
| **`dev10` pass rate** (Buckets A–E, 10 tasks) | 3/10 (30.0%)\* | 3/10 (30.0%) | 6/10 (60.0%) | **8/10 (80.0%)** | **`2.67×` pass rate (+50.0 pp)** |
| **`heldout6` pass rate** (unseen, 6 tasks) | 1/6 (16.7%) | — | 2/6 (33.3%) | **3/6 (50.0%)** | **`3.00×` pass rate (+33.3 pp)** |
| **`workflow8` pass rate** (8 real Antigravity-chat workflows) | 1/8 (12.5%) | — | — | **7/8 (87.5%)** | **`7.00×` pass rate (+75.0 pp)** |
| **Combined pass rate (all 24 tasks)** | **5/24 (20.8%)** | — | — | **18/24 (75.0%)** | **`3.60×` pass rate (+54.2 pp)** |
| **Timeouts (all 24 tasks)** | 19 | — | — | **5** | **-73.7% timeouts** |
| **Mean solve time (all 5 common passes)** | 130.3s | — | — | **54.4s** | **`58.2%` faster (`2.39×`, 5–0 sweep)** |
| **Edit failures (all 24 tasks)** | 0 / 18 | 0 / 6 *(dev10)* | 0 / 41 *(16 tasks)* | **0 / 157** | **100.0% edit precision** |
| **HTTP / wire retries (all 24 tasks)** | 0 | 1 | 0 | **0** | **Zero wire errors** |
| **Vertex API cost (`dev10` / `heldout6` / `workflow8`)** | *(AGY quota)* | \$9.26 / — / — | \$5.85 / \$4.27 / — | **\$6.97 / \$5.47 / \$6.65** | **\$0.80/task avg across 24 tasks** |

### 2.2 Locked-Scaffold `google-deepmind/gbench` Track (`effort=high`, `--thinking`, `--max-output-tokens 32768`, 0 infra errors, 0 truncations)

| Pillar / Suite | Scaffold ID | Raw `gemini-4-neon` | **Flawed · `gemini-4-neon`** | Delta |
|---|---|---:|---:|---|
| `amc_aime` (Competition Math) | `sc_a496c7c6` | `4/4 (100%)` · `356.8s` | **`4/4 (100%)` · `284.9s`** | **`-20.1%` faster** (Python verification) |
| `arc_agi` (Abstract Visual/Grid) | `sc_a496c7c6` | `4/4 (100%)` · `663.5s` | **`4/4 (100%)` · `920.8s`** | Tied `100%` |
| `causalbench` (Causal Inference) | `sc_a496c7c6` | `4/4 (100%)` · `86.2s` | **`4/4 (100%)` · `77.0s`** | **`-10.7%` faster** |
| `ifeval` (Instruction Following) | `sc_a496c7c6` | `3/4 (75%)` · `45.2s` | **`4/4 (100%)` · `57.2s`** | **`+25.0 pp` accuracy win** |
| `longbench_v2_code` (Long-Ctx Code QA) | `sc_a496c7c6` | `1/2 (50%)` · `221.1s` | **`1/2 (50%)` · `134.6s`** | **`-39.1%` faster**, 0 endpoint errors |
| `mmlu_pro` (10-Choice Hard STEM) | `sc_a496c7c6` | `4/4 (100%)` · `71.3s` | **`4/4 (100%)` · `59.0s`** | **`-17.3%` faster** |
| `nexusraven_api_eval` (Nested API Calls) | `sc_a496c7c6` | `3/4 (75%)` · `176.5s` | **`3/4 (75%)` · `35.6s`** | **`5.0×` faster (`-79.8%`)** |
| `seal_tools` (Multi-Step Tool Use) | `sc_a496c7c6` | `4/4 (100%)` · `70.7s` | **`4/4 (100%)` · `51.5s`** | **`-27.2%` faster** |
| `mmlu_redux` (Curated Reasoning) | `sc_101f2351` | `4/4 (100%)` · `48.6s` | **`4/4 (100%)` · `50.1s`** | Tied `100%` |
| `mrcr` (Multi-Round Long-Context) | `sc_101f2351` | `3/4 (99%)` · `248.9s` | **`3/4 (99%)` · `97.1s`** | **`2.56×` faster (`-61.0%`)** |
| **10-Suite Total (38 graded items)** | **Locked** | **`34/38 (89.5%)` · `1988.9s`** | **`35/38 (92.1%)` · `1767.8s`** | **`+2.6 pp` accuracy, `-221.1s` (`-11.1%`) faster** |
| **Golden Set (`oa-template-smoke`)** | `sc_a170fa54` | `13/13 (100%)` · `50.5s` | **`13/13 (100%)` · `35.6s`** | **`-29.6%` faster**, 0 leaks, 0 errors |

*\*Fairness note on Antigravity `dev10`: In the initial 6-worker parallel Round 0 run, `agyworker` hit two transient errors (`sh_019` failed at `0.4s` with `plan model not specified`, `sh_005` failed at `17.6s` on `<end_of_image>`), producing raw `1/10`. Both were re-run cleanly at `--workers 2` and passed (`sh_005` `99.5s`, `sh_019` `121.4s`). Every comparison uses the **healthy 3/10** Antigravity baseline.*  
*†Fairness note on gBench: An initial `8192`-token gBench run (`gb-20261125-175722`) gave Flawed `12/12 (100%)` vs. Raw `8/12 (66.7%)`, but inspection revealed Raw Neon had 2 `amc_aime` and 2/8 `arc_agi` attempts truncated by `--max-output-tokens 8192`. We rejected that unfair comparison, raised `flawed_serve.coax` high-effort raw thinking headroom to `16384`, and re-ran all 10 suites fairly at `--max-output-tokens 32768` (`gb-20261125-184709` + `gb-20261125-194516`), achieving **0 truncations** on both arms.*

---

## 3. Experimental Setup & New Custom Workflow Benchmark (`evals/antigravity-workflow/`)

- **Track 1 (`evals/session-history/tasks/`):**
  - `dev10` (10 tasks, 2 per bucket A–E): `sh_001`, `sh_005`, `sh_008`, `sh_012`, `sh_003`, `sh_014`, `sh_018`, `sh_019`, `sh_023`, `sh_025`.
  - `heldout6` (6 unseen generalization tasks, untouched during hillclimbing): `sh_002`, `sh_011`, `sh_013`, `sh_022`, `sh_024`, `sh_027`.
- **Track 2 (`google-deepmind/gbench` via `coder/tui/gbench-run.coax`):**
  - Strictly locked `scaffold_id` comparing `gemini-4-neon` (`flawedworker --serve --mode raw`) against `flawed-gemini-4-neon` (`flawedworker --serve --mode harness`) at `--thinking --effort-a high --effort-b high --max-output-tokens 32768`.
- **Track 3 (`evals/antigravity-workflow/tasks/` — 8 custom workflow evals mined from real Nov 23–25 Antigravity chats):**
  - Every grader was verified to **FAIL on `base_sha`** (`rc != 0`) and **PASS on `target_sha`** (`rc == 0`):
    1. `wf_001_agy_default_effort_high` (chat `597403ca`): 14-file cross-layer Coax + Swift config change (`medium → high`).
    2. `wf_002_agy_sidebar_icons_12pt` (chat `24d8752e`): Desktop UI 12pt sidebar icon atlas preserving shared 14pt slots + `app-v1.coax` minimal checks.
    3. `wf_003_agy_authority_cache_expiry` (chat `ba6d40e0`, #14031): `#authority` Coax policy module with **`coax mutate --root-only` (0 surviving mutants)**.
    4. `wf_004_agy_inky_tool_json_leak` (chat `fe80b00c`): Cross-language Coax (`coax/lib/inky/loop.coax`) + C (`psionic/platform/inkling-serve.c`) native Metal build fix.
    5. `wf_005_agy_stall_watchdog_steer` (chats `099f2a5a`/`2aa782a6`): Multi-file worker runtime concurrency fix (stall ceiling + superseded-steer `end_turn` synchronization).
    6. `wf_006_agy_flawed_turn_invariants` (chat `a5a91943`): Wire-protocol structural conversation invariants & guards in `coder/tui/flawedworker.coax`.
    7. `wf_007_agy_shared_core_c_abi` (chat `ba6d40e0`, #14032): `#authority` Coax → C ABI export verified with `clang -Wall -Werror`.
    8. `wf_008_agy_desktop_sidebar_polish` (chat `ba6d40e0`, #14029): Desktop sidebar multi-select fill, 1.5pt focus ring, quiet receding titles, and `Cmd+Z` undo wiring.
- **Identical model & budget parameters:** Both arms run `gemini-4-neon`, `effort=high`, `300s` per-task cap, fresh detached `git worktree` at `base_sha`, `remote.origin.pushurl=no_push`. Every harness improvement is 100% general software-engineering infrastructure (zero task-specific hacks).

---

## 4. Round-by-Round Hillclimb Progression (`dev10` & `workflow8`)

| Round / Arm | `dev10` Pass | `workflow8` Pass | `dev10` Wall (Avg) | `dev10` Common-3 Avg (`001/005/019`) | `dev10` Edits / Fails | `dev10` Retries | `dev10` Cost | Key Change / Finding |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| **Antigravity · Neon** | 3/10 (30%) | 1/8 (12.5%) | 2418.1s (241.8s) | 102.5s | 6 / 0 | 0 | — | Baseline (`19/24` timeouts across all 24 tasks) |
| **Flawed R0** | 3/10 (30%) | — | 2641.7s (264.2s) | 177.0s | 6 / 0 | 1 | \$9.26 | Unsolicited `git commit`, head-only Bash, 250-line Read, 32K sig cap |
| **Flawed v1** | 3/10 (30%) | — | 2360.7s (236.1s) | 83.4s | 6 / 0 | 0 | \$9.51 | Session git snapshot, Read dedup, Edit fallback, Grep parity, Head+Tail Bash, 128K sig repair |
| **Flawed v2** | 3/10 (30%) | — | 2398.2s (239.8s) | 95.8s | 8 / 0 | 16† | \$8.14 | `<system-reminder>` pacing; exposed Stage-B Read trim + Gemini sibling-text bug |
| **Flawed v3** | 3/10 (30%) | — | 2315.3s (231.5s) | 68.2s | 8 / 0 | 16† | \$8.41 | Exempted `Read` from Stage-B (`maxResultSizeChars: Infinity`) + sub-range dedup |
| **Flawed v4** | 6/10 (60%) | — | 2150.8s (215.1s) | 59.7s | 27 / 0 | 0 | \$5.85 | Folded `FWW_MB_TEXT` into last Gemini `functionResponse.output` (`16 → 0` retries, `2×` pass rate) |
| **Flawed v5** | 7/10 (70%) | — | 1912.3s (191.2s) | **45.4s** | 47 / 0 | 0 | \$7.05 | Read Auto-TOC outline, Glob cwd-prefix fix, query dedup, time-aware nudges, `FLAWED_THINK_ADAPTIVE` Turn-1/wrap-up (`sh_023` PASS; 6/6 prior passes faster) |
| **Flawed v6** | **8/10 (80%)** | 5/8 (62.5%) | 1880.5s (188.1s) | 57.6s | 42 / 0 | 0 | \$7.35 | Surgical-edit discipline (`flawed/wire.coax` + prompt), `fww_hist_clear` reset, fair `131072`/1 MiB gBench serve (`sh_025` PASS; 8-suite gBench `28/30` vs `27/30`) |
| **Flawed v7 / v7.1** *(ablation)* | 6/10 (60%) | 6/8 (75.0%) | 1959.6s (196.0s) | 54.9s | 37 / 0 | 2 | \$5.88 | 64-symbol Auto-TOC + concise `low` post-green-test summary flipped `wf_004` (`237.7s`); **ablation proved** aborting active thinking at `90–95s` regressed `sh_014`/`sh_023`/`wf_005` by throwing away nearly finished reasoning |
| **Flawed v8 (Champion)** | **8/10 (80%)** | **7/8 (87.5%)** | **1824.8s (182.5s)** | **54.7s** | **46 / 0** | **0** | **\$6.97** | Removed `fww_think_runaway` (kept pure `150s` dead-stream network guard) + kept all proven v5–v7.1 wins (`18/24 = 75.0%` overall, `3/6` unseen `heldout6`, `7/8` `workflow8`) |

---

## 5. Key Engineering Improvements (v1–v8)

### 5.1 Flawed v1–v4 (Foundation — `3/10 → 6/10`, `0` retries)
- **Session git snapshot** (`cc/context.ts` → `fww_capture_git_snapshot`), **4 KiB per-file instruction caps**, and **git safety override** (never unsolicited `git commit`/`push`).
- **Upgraded `Read`** (`600` lines / `32 KiB` cloud, sub-range `FwReadDedup`, `FILE_UNCHANGED_STUB`, Stage-B opt-out), **`Edit`** (trailing-whitespace/CRLF fallback, nearest-line diagnostics, updated snippet), **`Grep`** (`output_mode`, `-C/-A/-B`, `-i`, `type`, `head_limit`, `offset`, relative paths), and **Head+Tail Bash ring buffer** (`8 KiB` head + `16 KiB` tail + `[Exit code: N]`).
- **`128 KiB` `thoughtSignature` buffers + surgical repair** (`skip_thought_signature_validator`), **T3 idle prompt-cache compaction** (`cp_cache_status`), and **1-to-1 Gemini `functionResponse` text folding** (eliminated all structural `HTTP 400` context wipes).

### 5.2 Flawed v5–v8 (Hillclimb Round 2 — `6/10 → 8/10` `dev10`, `2/6 → 3/6` `heldout6`, `7/8` `workflow8`, `35/38` gBench)
1. **Read Auto-TOC Symbol Outline (`fww_is_outline_line`, `fww_append_symbol_outline`):** When cloud `Read` truncates a large file, appends up to `64` unread top-level symbol/struct/test/heading lines (`L<num>: ...`) across Coax, C, Swift, Rust, Go, Python, TS, and Markdown so Neon jumps straight to target functions instead of blind pagination.
2. **Glob Path & Validation Fixes (`fww_tool_glob`):** Preserves `cwd`-relative prefixes even when `path` is passed (fixing missing-prefix edits), validates missing/non-directory `path` with explicit `is_err=true`, and emits explicit truncation notices at `150` hits.
3. **Read-Only Query Dedup (`FwRun.query_dedup`):** Short-circuits repeated identical `Grep`/`Glob` calls across turns until any `Edit`/`Write` mutates the workspace.
4. **Surgical-Edit Discipline (`flawed/wire.coax` `Edit` schema + `flawedworker.coax` prompt):** Instructs minimal, surgical `Edit` hunks (`<= ~80` lines, split multi-region refactors into small parallel `Edit` calls) and immediate test verification after substantial edit batches — cutting silent `functionCall` buffering from `100s+` down to `<= 60s` and holding **`0/157` edit failures**.
5. **Time-Aware & Long-Think-Aware `<system-reminder>` Pacing + True `FLAWED_THINK_ADAPTIVE` Step Conditioning:** Keeps 100% honest `effort=high` on all core reasoning, design, coding, and debugging steps; uses `medium` on cold Turn-1 file discovery and post-green-test checks, and `low` only on the verified final 2–4 bullet summary wrap-up once `finish_nudged` fires (`FLAWED_THINK_EXTENDED` and gBench `bare_mode` stay `high` on 100% of steps).
6. **Clean Dead-Stream Watchdog (`fww_stream_stalled`, `150s`):** Tracks `stream_last_rx_ms` strictly on parsed content/thought/tool deltas (never blank/keepalive SSE lines), recovering dead connections without interrupting active thinking or multi-edit tool batches.
7. **gBench Serving & Telemetry Hardening (`coder/tui/flawed_serve.coax`, `fww_hist_clear`):** Fixed `fww_hist_clear` to zero `total_in`/`total_out`/`total_cached`/`compact_count` per request; raised `FWS_REQ_BUF` to `1 MiB`, eliminated `512 KiB` stack buffer `cb` and `16 KiB` array-part truncation, aligned advertised cloud `max_model_len = 131072` with `1 MiB` buffers so `longbench_v2_code` runs with `0` endpoint errors on both arms, and raised raw high-effort thinking headroom to `16384` so Raw Neon comparisons have `0` artificial truncations.

---

## 6. Complete Per-Task Telemetry (All 24 Real Coding Tasks, `effort=high`, `300s` cap)

### 6.1 `dev10` Stratified Slice (`8/10` vs. AGY `3/10`)

| Task (Bucket) | Antigravity · Neon (Healthy) | Flawed v4 | **Flawed v8 (Champion)** | Flawed v8 Details |
|---|---|---|---|---|
| `sh_001_claude_todos_wrap` (A) | **PASS** `86.6s` (255K tok) | **PASS** `63.2s` | **PASS `30.1s`** | 6T, 10 tools, `1/0` ed, 64K in, \$0.15 (**`2.88×` faster than AGY**) |
| `sh_005_claude_large_chat_open` (A) | **PASS** `99.5s` (163K tok) | **PASS** `29.2s` | **PASS `27.9s`** | 5T, 4 tools, `1/0` ed, 27K in, \$0.08 (**`3.57×` faster than AGY**) |
| `sh_008_claude_sidebar_composer_parity` (B) | TO `301.5s` (`0 B`) | **PASS** `216.4s` | **PASS `149.4s`** | 8T, 19 tools, `2/0` ed, 415K in (`61.4%` ca), \$0.66 |
| `sh_012_openagents_inkling_parser` (B) | TO `301.5s` (`0 B`) | **PASS** `250.3s` | **PASS `235.1s`** | 15T, 37 tools, `7/0` ed, 639K in (`77.3%` ca), \$0.87 |
| `sh_003_antigravity_utf8_cleanup` (C) | TO `301.5s` | TO `301.5s` (`0` ed) | TO `301.5s` | 9T, 22 tools, `4/0` ed, 358K in, \$0.79 |
| `sh_014_claude_inline_code_padding` (C) | TO `301.5s` (`0 B`) | **PASS** `298.9s` | **PASS `238.1s`** | 11T, 18 tools, `6/0` ed, 463K in (`71.5%` ca), \$0.79 |
| `sh_018_claude_checker_budget_e0381` (D) | TO `301.5s` (`0 B`) | TO `301.5s` (`0` ed) | TO `301.5s` | 16T, 38 tools, `7/0` ed (`89.7%` ca), \$1.07 (unlocked `0 → 7` edits!) |
| `sh_019_claude_sign_realtime_clock` (D) | **PASS** `121.4s` (281K tok) | **PASS** `86.8s` | **PASS `106.2s`** | 7T, 13 tools, `4/0` ed, 171K in, \$0.32 (`-39.2%` tokens vs AGY) |
| `sh_023_claude_agents_md_autoload` (E) | TO `301.5s` (`0 B`) | TO `301.5s` | **PASS `249.6s`** | 12T, 42 tools, `11/0` ed, `0` fails, 1.10M in (`75.6%` ca), \$1.32 |
| `sh_025_codex_claude_history_compact` (E) | TO `301.5s` (`0 B`) | TO `301.5s` (`0` ed) | **PASS `185.4s`** | 11T, 29 tools, `3/0` ed, `0` fails, 596K in (`67.5%` ca), \$0.92 |

### 6.2 Unseen `heldout6` Generalization Slice (`3/6` vs. AGY `1/6`)

| Task (Bucket) | Antigravity · Neon | Flawed v4 | **Flawed v8 (Champion)** | Head-to-Head vs. AGY |
|---|---|---|---|---|
| `sh_002_openagents_worker_recall` (A) | **PASS** `106.2s` (427K tok) | **PASS** `64.0s` | **PASS `26.0s`** (6T, 7 tools, 38K tok, `2/0` ed, \$0.10) | **`4.09×` faster**, **`11.3×` fewer prompt tokens** |
| `sh_011_antigravity_backend_foot_label` (B) | TO `301.5s` (2.50M tok) | TO `301.5s` | TO `301.7s` (9T, 20 tools, 493K tok, \$0.38) | Tie (Flawed `5.1×` fewer tokens) |
| `sh_013_antigravity_mobile_row_id` (C) | TO `301.5s` (2.63M tok, `0` ed) | FAIL `295.5s` | FAIL `182.8s` (16T, 33 tools, 789K tok, `5/0` ed, \$0.83) | Flawed completed 5 edits; AGY `0` edits |
| `sh_022_claude_desktop_core_stack` (D) | TO `301.5s` (492K tok, `0` ed) | **PASS** `255.7s` | **PASS `88.8s`** (8T, 17 tools, 145K tok, `2/0` ed, \$0.34) | **Flawed PASSES (`-65%` faster than v4)**; AGY `0` edits |
| `sh_024_codex_mount_why_diagnostics` (E) | TO `301.5s` (2.46M tok, `0` ed) | TO `301.5s` (`0` ed) | **PASS `270.2s`** (13T, 32 tools, 1.38M tok, `4/0` ed, \$1.89) | **NEW unseen PASS**; AGY & v4 timed out with `0` edits |
| `sh_027_antigravity_skeleton_shimmer` (E) | TO `301.5s` (1.22M tok, `0` ed) | TO `301.5s` (`5` ed) | TO `301.5s` (18T, 40 tools, `12/0` ed, \$1.93) | Flawed applied 12 edits; AGY `0` edits |

### 6.3 Custom Antigravity-Workflow Slice `workflow8` (`7/8` vs. AGY `1/8`)

| Task (Workflow Pillar) | Antigravity · Neon | **Flawed v8 (Champion)** | Head-to-Head vs. AGY |
|---|---|---|---|
| `wf_001_agy_default_effort_high` (14-file Swift+Coax config) | TO `301.5s` (44 tools, 2.50M tok, `0` ed) | **PASS `230.0s`** (13T, 80 tools, `27/0` ed, 897K tok, \$1.10) | **Flawed PASSES**; AGY `0` edits |
| `wf_002_agy_sidebar_icons_12pt` (Desktop 12pt icon atlas) | **PASS** `237.7s` (19 tools, 905K tok, `3/0` ed) | **PASS `81.8s`** (7T, 17 tools, `4/0` ed, 146K tok, \$0.34) | **`2.91×` faster (`-65.6%`)**, **`6.2×` fewer tokens** |
| `wf_003_agy_authority_cache_expiry` (`#authority` + `coax mutate` 0 survivors) | TO `301.7s` (20 tools, `1/0` ed) | **PASS `212.4s`** (16T, 35 tools, `11/0` ed, 801K tok, \$0.79) | **Flawed PASSES** (100% mutants killed); AGY TO |
| `wf_004_agy_inky_tool_json_leak` (Psionic C + Inky Coax) | TO `301.5s` (25 tools, `0` ed) | **PASS `277.4s`** (12T, 29 tools, `9/0` ed, `0` fails, \$0.95) | **Flawed PASSES**; AGY `0` edits |
| `wf_005_agy_stall_watchdog_steer` (Worker concurrency/steer) | TO `301.5s` (17 tools, `0` ed) | **PASS `263.2s`** (8T, 37 tools, `17/0` ed, `0` fails, \$1.24) | **Flawed PASSES**; AGY `0` edits |
| `wf_006_agy_flawed_turn_invariants` (Wire-protocol invariants) | TO `301.5s` (14 tools, `0` ed) | TO `301.5s` (4T, 13 tools, `4/0` ed, \$0.69) | Tie (Flawed wrote 4 edits; AGY `0` edits) |
| `wf_007_agy_shared_core_c_abi` (`#authority` -> C ABI `-Wall -Werror`) | TO `301.5s` (34 tools, `3/0` ed) | **PASS `100.3s`** (8T, 18 tools, `5/0` ed, 238K tok, \$0.45) | **Flawed PASSES in `100s`**; AGY TO |
| `wf_008_agy_desktop_sidebar_polish` (Desktop UX + Cmd+Z undo) | TO `301.5s` (30 tools, `5/0` ed) | **PASS `177.1s`** (15T, 28 tools, `9/0` ed, 1.31M tok, \$1.09) | **Flawed PASSES in `177s`**; AGY TO |

---

## 7. Verification, Bundle Budget & Desktop Deployment (Flawed v8)

- **Authority unit & mutation checks:**
  - `coax/bin/coax test flawed/wire.coax --library`: **PASS**
  - `coax/bin/coax mutate flawed/wire.coax --root-only`: **0 surviving mutants**
- **Worker & Desktop unit checks:**
  - `coax/bin/coax test coder/tui/flawedworker.coax --library`: **267/267 passed** (including tests for `fww_append_symbol_outline`, `fww_adaptive_effort`, `fww_stream_stalled`, and `fww_hist_clear` telemetry reset)
  - `coax/bin/coax test apps/openagents-desktop/app-v1.coax --library`: **514/514 passed**
- **Repository & pre-push gates:** `./scripts/check.sh` passed (`every check passed`).
- **Desktop `--minimal` build & bundle budget:**
  - `./apps/openagents-desktop/build.sh --minimal`: built cleanly (`flawedworker` binary `615 KiB`, `+2 KiB` vs v4).
  - `v1-bundle-check`: **`PASS v1-bundle-check (macos stage 2, unpacked=14,643,200 B / 14 MiB budget)`** (zero extra unpacked growth vs v4).
- **Custom workflow benchmark validation:** All 8 graders in `evals/antigravity-workflow/tasks/wf_001..wf_008` verified to fail on `base_sha` and pass on `target_sha`.
