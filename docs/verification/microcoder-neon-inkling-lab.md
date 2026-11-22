# Microcoder Neon / Inkling / Two-Device Optimization Lab

> Empirical engineering log comparing pure **Gemini 4 Neon** (Vertex AI), pure local **Inkling 2 Mini** (Apple M5 Max Metal 4, `\$0.00`), second-device **Qwen3.5-9B** (`coderos-4080` NVIDIA RTX 4080 CUDA over Tailscale, `\$0.00`), and **Neon + Inkling** multi-model combos on the stratified 27-task OpenAgents session-history coding benchmark (`evals/session-history/tasks/`, Buckets A–E).

## Methodology & Holdout Discipline

- **Dev slice (9 tasks, 1–2 per bucket):** `sh_001`, `sh_004` (Bucket A: edit precision), `sh_008`, `sh_010` (Bucket B: multi-file), `sh_013`, `sh_017` (Bucket C: verification), `sh_019`, `sh_021` (Bucket D: error recovery), `sh_023` (Bucket E: long context).
- **Held-out slice (18 tasks):** Kept completely untouched throughout Iterations 1–5B; evaluated only in the final 27-task verification to confirm zero overfitting.
- **Physical devices exercised:**
  - **Device 1 (macOS):** Apple M5 Max (`100.127.107.31`, 128 GB unified memory), resident Metal 4 `llama-server` serving `Inkling-2-Mini-IQ4_NL.gguf` (16 GB, `32K` ctx, `127.0.0.1:18794`) + Vertex AI `us` multi-region (`gemini-4-neon`).
  - **Device 2 (NixOS / CoderOS):** `coderos-4080` (`100.74.238.61` over Tailscale), NVIDIA RTX 4080 16 GB VRAM serving `Qwen3.5-9B-Q4_K_M.gguf` (5.3 GB, `32K` ctx, CUDA 12.8, `6.6 GiB / 16 GiB` VRAM, `100.74.238.61:18795`).

---

## Live API & Hardware Probes (Root-Cause Findings)

1. **Vertex AI `gemini-4-neon` (`us` & `global`):**
   - Rejects explicit Vertex `cachedContents` (HTTP 400: `"Model gemini-4-neon does not support explicit context caching"`).
   - Rejects `thinkingLevel: "minimal"` (HTTP 400; `"low"` is minimum).
   - Implicit prompt cache hit rate on Vertex averaged **0%–4%** across multi-step runs, meaning **75.5% of baseline Neon spend (`\$1.38 / \$1.83`) was uncached input tokens** at `\$3.00/M` input and `\$15.00/M` output/reasoning.
   - Under tight `\$0.12–\$0.25` per-task budgets, baseline Microcoder's `10K–29K`-token prompts gave Neon only 3–5 steps before hitting budget stops (`6/6` baseline failures were `ended: stopped`).
2. **Local Metal 4 & CUDA (`inkling-2-mini` / `qwen35-9b`, `\$0.00`):**
   - Baseline failed 7/9 tasks due to seven concrete Microcoder handicaps uncovered via trace forensics:
     1. Missing `inkling-2-mini` / `hybrid-exec-v1` entries in `GW_WINDOWS` (`card.coax`), followed by `mc_budget(32768)` quadruple-taxing local 32K models down to a 16 KB `log_limit` (~5K tokens), thrashing Metal/CUDA KV caches from `10.2K → 1.1K` tokens every 3 steps.
     2. `GW_NO_STRICT_SCHEMA` on `psionic` caused empty Step-1 replies and lost structured fields in fallback parsing.
     3. Missing `max_tokens` cap on `psionic` allowed GBNF repetition loops (`slab.coax:1701-1800,...`) to hang entire 300 s timeouts on a single attempt.
     4. Step-0 seeding disabled on `local: true` runs (`mcworker.coax`), and `mc_seed_anchor_line` picked top-of-file `#ifdef __APPLE__` (line 9) over target function `cd_view` (line 280), while wasting a seed read on binary `coax/bin/coax`.
     5. Placing `"view PATH:A-B"` inside `commands[]` executed `/usr/bin/view` (interactive vim), burning 120–240 s per task until command timeout.
     6. `UINSTR_DEFAULT` (`user-instructions.coax`) appended `"push the commit"` at the tail of the system prompt, causing local 9B models to trip `git push` safety guards even after solving tasks.
     7. After cloud-to-local handback in hybrid runs, `low_cache_calls` set by paid Vertex calls continued firing rolling compaction on local `psionic`, collapsing post-handoff KV cache to `1,176` tokens.

---

## Dev-9 Empirical Progression (13 Verified Runs)

### Pure Neon (`gemini-4-neon`, Vertex AI, `MC_DELEGATE_INKLING=0`)

| Run | Pass Rate | Cmd Grader Pass | Total Cost | Wall | Bucket A | Bucket D | Key Delta |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **`base-neon-dev9`** (baseline) | **3/9 (33%)** | 4/9 | `\$1.8279` | 381s | 2/2 (`\$0.2670`) | 1/2 | 6/6 failures budget-stopped (`sh_021` cmd passed) |
| **`iter1-neon-dev9`** | 3/9 (33%) | 3/9 | `\$1.5543` (`-15%`) | 375s | 2/2 (`\$0.1620`) | 1/2 | Lean prompt + initial rolling compaction; self-compaction bug found |
| **`iter1b-neon-dev9`** | 2/9 (22%) | 4/9 | `\$1.5707` (`-14%`) | 385s | 2/2 (`\$0.1615`) | 0/2 | `min_steps=2` fixed self-compaction; exposed `wm_task_check` bug on `sh_021` |
| **`iter2-neon-dev9`** | **4/9 (44%)** | **6/9** | `\$1.5835` (`-13%`) | 478s | 2/2 (`\$0.1374`) | **2/2** | `wm_task_tokens_within` unlocked `sh_021`; `sh_008` & `sh_010` cmd passed |
| **`iter2b-neon-dev9`** | **4/9 (44%)** | 5/9 | `\$1.4481` (`-20.8%`) | **379s** | 2/2 (`\$0.1224`) | **2/2 (`\$0.2209`)** | 0% search bloat, 0% edit mismatch, 0% compaction drop |
| **`iter3-neon-dev9`** | 3/9 (33%) | 5/9 | `\$1.4792` (`-19.1%`) | 438s | 2/2 (`\$0.1308`) | 1/2 | Passing-check 1.2 KB trim + Coax cue; Bucket E `sh_023` & `sh_010` cmd passed |
| **`iter4-neon-dev9`** | 3/9 (33%) | **6/9** | `\$1.4690` (`-19.6%`) | 613s | 2/2 (`\$0.1071`) | 1/2 | `sh_004` dropped to `\$0.0434` (2 att, `-68%`); `sh_008`, `sh_010`, `sh_023` cmd passed (`min_steps=1` caused extra re-reads) |
| **`iter4b-neon-dev9`** | **4/9 (44%)** | 5/9 | **`\$1.3886` (`-24.0%`)** | 417s | **2/2 (`\$0.1063`, `-60.2%`)** | **2/2 (`\$0.2020`)** | Best Pure Neon: `sh_004` **`\$0.0412`** (13s, `-69.4%`), `sh_019` `\$0.1107`, `sh_021` `\$0.0914`, `sh_010` cmd passed (`\$0.2056 / \$0.20`) |
| **`iter5-neon-dev9`** *(8/9; `sh_017` worktree timeout)* | 4/8 (50%) | 5/8 | `\$1.2854` | 396s | **2/2 (`\$0.1018`, `-61.9%`)** | **2/2 (`\$0.2452`)** | `sh_004` **`\$0.0369`** (`-72.6%`); Step-0 early compaction raised `sh_010`/`sh_019` cost → reverted in Iter 5B |

### Pure Local Single-Device Runs (`\$0.00` Cost)

| Run | Device & Model | Pass Rate | Cost | Wall | Highlights |
| --- | --- | --- | --- | --- | --- |
| **`base-inkling-dev9`** | M5 Max Metal 4 · `inkling-2-mini` | **2/9 (22%)** | `\$0.0000` | 2098s | `sh_001` (103s), `sh_004` (98s); `sh_021` tripped `git push` |
| **`iter1-4080-qwen35-dev9`** | `coderos-4080` RTX 4080 CUDA · `qwen35-9b` | 2/9 (22%) | `\$0.0000` | 1539s | `sh_001` 37s, `sh_004` 35s (2.8× faster than M5 Max) |
| **`iter2b-inkling-dev9`** | M5 Max Metal 4 · `inkling-2-mini` | 2/9 (22%) | `\$0.0000` | 2439s | `sh_004` 66s (`-32%`); 0 `git push` trips; `sh_021` cmd passed before vim hang |
| **`iter2b-4080-qwen35-dev9`** | `coderos-4080` RTX 4080 CUDA · `qwen35-9b` | **3/9 (33%)** | `\$0.0000` | 1866s | **`sh_010` (3-file Bucket B) PASSED in 158s (`\$0.0000`)!** |
| **`iter3-inkling-dev9`** | **M5 Max Metal 4 · `inkling-2-mini`** | **4/9 (44%)** | **`\$0.0000`** | 2268s | **DOUBLED BASELINE (`2/9 → 4/9`) & beat baseline Neon (`3/9, \$1.83`) at `\$0.00`!** Bucket A `2/2` + Bucket D `2/2` (`sh_019` 292s, `sh_021` 239s), 0% failed edits, 0 guard trips |
| **`iter3-4080-qwen35-dev9`** | `coderos-4080` RTX 4080 CUDA · `qwen35-9b` | 3/9 (33%) | `\$0.0000` | 1683s | `sh_001` 49s, `sh_004` **12.9s** (2 att), `sh_021` **58.9s**; `sh_010` hung on 300s GBNF loop |
| **`iter4b-4080-qwen35-dev9`** | `coderos-4080` RTX 4080 CUDA · `qwen35-9b` | 1/9 (11%) | `\$0.0000` | 2115s | `sh_001` **33.5s** (3.7× faster); `sh_004` solved in 18s then tripped `git push` (4 tasks tripped `git push` on generic `qwen35-9b`, confirming fine-tuned `inkling-2-mini`'s superior instruction discipline) |

### Neon + Inkling Multi-Model Combos

| Run | Combo Architecture | Pass Rate | Cmd Grader Pass | Total Cost | Cost vs Baseline | Highlights |
| --- | --- | --- | --- | --- | --- | --- |
| **`comboA-rtx4080-neon-dev9`** | Combo A1 v1 (`inkling-neon`: RTX 4080 + Vertex Neon) | **4/9 (44%)** | 5/9 | `\$1.4296` | `-21.8%` | 3-phase hybrid proven live (`sh_001` `\$0.0468`, **`sh_021` `\$0.0000` in 15s!**); Bucket A `2/2` + Bucket D `2/2`; `sh_010` cmd passed (`\$0.2083`) |
| **`comboA-v2-rtx4080-neon-dev9`** | Combo A2 v2 (RTX 4080 + Vertex Neon + budget fallback) | **4/9 (44%)** | **6/9** | **`\$1.2429`** | **`-32.0%`** | Bucket A `2/2` for **`\$0.0693`** (`sh_004` **`\$0.0248`**, `-81.6%`!); Bucket D `2/2` for `\$0.1997`; `sh_010` & `sh_023` cmd passed |
| **`comboA-v2-inkling-neon-dev9`** | **Combo A1 v2 (M5 Max `inkling-2-mini` + Vertex Neon)** | **4/9 (44%)** | 5/9 | **`\$1.1213`** | **`-38.7%`** | **Lowest total cost across all Dev-9 runs; 0/9 budget overruns, 0 failed edits, 0 guard trips!** Bucket A `2/2` (`\$0.0707`, `-73.5%`), Bucket D `2/2` (`\$0.1296`: `sh_019` `\$0.1014`, `sh_021` `\$0.0282`); `sh_010` cmd passed at `\$0.1904 < \$0.20` |
| **`comboA-v3-inkling-neon-dev9`** | Combo A1 v3 (M5 Max + Vertex Neon, Iter 5) | 3/9 (33%) | 5/9 | `\$1.2011` | `-34.3%` | **`sh_004` solved for `\$0.0000`** (70s); Bucket A **`\$0.0648`** (`-75.7%`); `sh_010` (`\$0.1800 < \$0.20`) & `sh_023` cmd passed — exposed 16.9K uncompacted closing-turn bug fixed in Iter 5B |
| **`comboB-neon-delegate-4080-dev9`** | Combo B v1 (Neon Architect + `level:"inkling"` sub-agents) | 3/9 (33%) | 5/9 | `\$1.4535` | `-20.5%` | `agent_tasks=0` on all 9 tasks (passive `MC_AGENTS` wording kept Neon solo) → strengthened `MC_AGENTS_INKLING` in Iter 5B |

---

## Implemented Architecture Improvements (Checkpoints #1 & #2)

1. **Universal Microcoder Core (`coax/lib/microcoder/`, `coax/lib/agent/workmem/`, `coax/lib/gateway/`):**
   - **Token-subsequence check matching (`workmem/task.coax`):** `wm_task_tokens_within` recognizes verification commands with inserted flags (`cc -fsyntax-only -I... stub.c`, `coax/bin/coax test --library ...`), eliminating false finish-refusal loops.
   - **Expanded check & search classifiers (`commit.coax`, `run.coax`):** `mc_cmd_kind` recognizes `python3 -m py_compile` and `cc/clang/gcc -fsyntax-only` as `MC_CMD_CHECK`; `mc_cmd_is_search` trims passing read-only search/listing outputs (`git grep`, `rg`, `grep`, `find`, `ls`, `git log`) to 2,400 B (spilling full text to disk), while passing checks trim to 1,200 B.
   - **Symbol-ordered Step-0 seeding & binary skip (`run.coax`):** Up to 4 task-mentioned files (`MC_SEED_BUDGET = 8192`, `MC_SEED_GREP_MAX = 4`), skipping `AGENTS.md`/`CLAUDE.md` and binary paths (`coax/bin/...`, images, `.gguf`, archives), anchoring large files on task symbols in prompt order while deprioritizing top-of-file platform macros (`__APPLE__`, `__linux__`, `_WIN32`). Enabled on live local runs while preserving `GW_REPLAY` C16 determinism.
   - **Cache-adaptive rolling compaction (`state.coax`, `run.coax`):** `mc_compact_keep_steps(st, keep, min_steps)` compacts older steps ONLY on `<25%` cache-hit paid cloud endpoints (`!is_local_gw`), preserving 2 verbatim steps up to 28 KB and capping low-cache paid views at 8 KB, while leaving local Metal/CUDA KV caches completely untouched even after cloud→local handback.
   - **Local 32K KV-cache & repetition-hang protection (`card.coax`, `psionic.coax`, `run.coax`, `microcoder_keys.coax`, `text_action.coax`):** `gw_psionic_strict` uses GBNF `json_schema` + `GW_CHAT_LEGACY_MAX_TOKENS` with `max_tokens = 4096` (eliminating 300 s GBNF repetition hangs) and an effective `57344` byte-budget window for `psionic` (~24K peak tokens inside 32K ctx, eliminating 11K-token KV-cache thrashing).
   - **Lean system prompt, sharpened Coax cue & safety tail (`loop.coax`, `instructions.coax`, `run.coax`):** Slimmed `MC_SYSTEM` (~1,350 tokens/call saved vs baseline), added minimal-edit guidance and Coax syntax cue (`no let/const/fn/default-args/multi-return`, `#test` compile-time `#run_steps`/`#steps`), and appended `MC_SAFETY_TAIL` to block unasked `git commit`/`git push`.
   - **`/usr/bin/view` vim-hang interceptor (`run.coax`):** `mc_cmd_view_arg` intercepts `"view PATH[:A-B]"` placed in `commands[]` and satisfies it via `mc_run_view` in <1 ms instead of hanging 120 s on interactive vim.
2. **Multi-Model Neon + Inkling Combos & Two-Device Pooling:**
   - **Combo A — `inkling-neon` / `hybrid-neon` (`run.coax`, `mcworker.coax`, `apps/openagents-desktop/page/`, `host_driver.c`):**
     - Local-first phase (`MC_HYBRID_LOCAL_DEFAULT_STEPS = 3`, `MC_HYBRID_LOCAL_DEFAULT_MS = 70s`, 2-fail escalation guard) solves easy/medium tasks for `\$0.00`.
     - Zero-amnesia escalation hands gathered context to Vertex `gemini-4-neon` (compacting only past 24 KB to 10 KB / 2 steps).
     - Compact 3K-token free closing turn (`mc_hybrid_ready_finish`) compacts history down to the final verified step and hands `finished:true` back to free local GPU (`\$0.00`, 3–6 s).
     - Predictive budget-aware cloud-to-local fallback (`MC_HYBRID_CLOUD_BUDGET_MICROS`) switches back to free local GPU before a paid cloud call would breach the chat's dollar budget ceiling.
   - **Combo B — `neon+inkling` (`action.coax`, `loop.coax`, `mcworker.coax`, `page/`):** Exposes `level: "inkling"` free local sub-agents to cloud runs whenever a local/LAN psionic server is reachable.
   - **Two-Device `PSIONIC_PEERS` Pooling (`psionic.coax`, `microcoder_keys.coax`):** Supports heterogeneous `URL#MODEL` peer lists spanning Apple M5 Max Metal 4 and `coderos-4080` NVIDIA RTX 4080 CUDA.
   - **Eval Harness Reliability (`evals/openagents-desktop/tier2/run_tasks.py`):** Bumped `git worktree add` timeout to 300 s and hardened `sb.remove()` against macOS parallel disk I/O timeouts.

---

## Pylon & Trajectory Contribution Pipeline Verification (Live)

- **`openagents pylon onboard --json`** (Apple M5 Max): `executor_selftest: true`, `tamper_refused: true`, `artifact_sha256: b1b6bbceedeb3c446791a4dd675acd092726a775ac30697ff2e6f8c59d32bc71`, `modules_installed: 10`, `ok: true`.
- **`openagents contribute selftest`**: `PASS (default_off=ok adversarial_redaction=clean verifier_gate=ok heldout_block=ok)`.
- **Live redacted `ATIF-v1.8` trajectory shard submission** (real verified `iter4b-neon-dev9` `sh_004` journal, isolated temp `$HOME`, keeping real home `contribute: OFF`):
  - Default-OFF refusal verified on real home (`REFUSED: contribution is OFF by default`).
  - Opt-in isolated submit: `ADMITTED shard=/tmp/oa-lab-runs/sh004-neon-atif-shard.json sha256=94bd69ed021745d8806001070a1c37d5c63ee5fb8d7f39bd5d64b19837f44169 bytes=19250` (`v: openagents.executor.dataset-shard.v1alpha1`, `format: ATIF-v1.8`, `quarantine_state: quarantined`, `verifier: test`, home path redacted to `~/skills/...`).

---

## Verification Status (Checkpoint #2)

- `coax/bin/coax test coax/lib/gateway/psionic.coax --library`: **passed**
- `coax/bin/coax test coax/lib/microcoder/commit.coax`: **passed**
- `coax/bin/coax test coax/lib/microcoder/loop.coax`: **247/247 passed**
- `coax/bin/coax test coax/lib/microcoder/run.coax`: **450/450 passed**
- `coax/bin/coax test coax/lib/microcoder/tools.coax`: **330/330 passed**
- `coax/bin/coax check coder/tui/codex-options-checks.coax` & `diff-cells-checks.coax`: **0 errors**
- `coax/web/tool.sh tools-check`, `wire-check`, `layer-check`: **0 failed**
- `evals/openagents-desktop/run.sh --tier 0 --tier 1`: **5/5 passed, 0 failed** (re-verified on Iteration 5B below)

---

*Next section (appended after Iteration 5B Two-Device Pool check and the Final Full 27-Task Dev-9 + Held-Out-18 Showdown across Pure Neon, Pure Inkling, and Hybrid Combo):*
- *Final 27-Task Full Benchmark Scorecard (Dev-9 vs. Held-Out-18)*
