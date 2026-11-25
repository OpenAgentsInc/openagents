# Flawed · Gemini 4 Neon vs. Antigravity · Gemini 4 Neon: Head-to-Head Benchmark & Harness Hillclimb

**Date:** 2026-11-25  
**Shipped Commit:** `d7be8ab43b5c894a8595eb094b867f555584f335` (`origin/main`)  
**Installed Desktop Build:** `/Applications/OpenAgents.app` (`1.0.0`, build `34353`, `OAProfile = minimal`, `14,643,200` B / `14 MiB` Stage 2 budget)  
**Modified Runtime Sources:** `coder/tui/flawedworker.coax`, `flawed/wire.coax`

---

## 1. One-Sentence Bottom Line

Running the exact same `gemini-4-neon` model at `effort=high` (`300s` per-task wall cap) across 16 real repository coding tasks (`dev10` + 6 unseen `heldout6` tasks), **Flawed · Neon doubled base Antigravity · Neon's pass rate (`8/16 = 50.0%` vs. `4/16 = 25.0%`), solved all 4 commonly passed tasks `41.2%` faster (`60.8s` vs. `103.4s` avg), consumed `35%–46%` fewer prompt tokens than Antigravity, and recorded `0/41` edit failures and `0` HTTP/wire retries.**

---

## 2. Executive Summary

| Metric | Antigravity · Neon (`high`) | Flawed R0 · Neon (`high`) | **Flawed v4 · Neon (`high`)** | Flawed v4 vs. AGY / R0 |
|---|---:|---:|---:|---|
| **`dev10` pass rate** (Buckets A–E, 10 tasks) | 3/10 (30.0%)\* | 3/10 (30.0%) | **6/10 (60.0%)** | **2.0× pass rate (+30.0 pp)** |
| **`heldout6` pass rate** (unseen, 6 tasks) | 1/6 (16.7%) | — | **2/6 (33.3%)** | **2.0× pass rate (+16.7 pp)** |
| **Combined pass rate** (16 tasks) | **4/16 (25.0%)** | — | **8/16 (50.0%)** | **2.0× pass rate (+25.0 pp)** |
| **Mean solve time (3 common `dev10` passes)** | 102.5s | 177.0s | **59.7s** | **41.8% faster than AGY, 2.96× faster than R0** |
| **Mean solve time (all 4 common passes)** | 103.4s | — | **60.8s** | **41.2% faster than AGY (4–0 sweep)** |
| **`dev10` total prompt tokens**† | 8,386,183 (`84.3%` cached) | 14,996,546 (`88.4%` cached) | **5,459,594 (`82.7%` cached)** | **-34.9% vs. AGY, -63.6% vs. R0** |
| **`heldout6` total prompt tokens**† | 9,729,052 (`87.3%` cached) | — | **5,236,137 (`86.8%` cached)** | **-46.2% vs. AGY** |
| **`dev10` tool calls / LLM turns** | 236 tools / — | 443 tools / 211 turns | **215 tools / 105 turns** | **-51.5% tools, -50.2% turns vs. R0** |
| **`heldout6` tool calls** | 222 tools | — | **156 tools** | **-29.7% tools vs. AGY** |
| **Edit failures (all 16 tasks)** | 0 / 7 | 0 / 6 *(dev10)* | **0 / 41** | **100% edit precision** |
| **HTTP / wire retries (all 16 tasks)** | 0 | 1 (`thoughtSignature` wipe) | **0** | **Zero wire errors** |
| **`dev10` Vertex API cost** | *(AGY quota, \$0 billed)* | \$9.26 | **\$5.85** | **-36.8% cheaper at 2× pass rate** |
| **`heldout6` Vertex API cost** | *(AGY quota, \$0 billed)* | — | **\$4.27** | \$0.71/task avg |

*\*Fairness note on Antigravity `dev10`: In the initial 6-worker parallel Round 0 run, `agyworker` hit two transient errors (`sh_019` failed at `0.4s` with `plan model not specified` under 6-way startup contention, and `sh_005` failed at `17.6s` on an `<end_of_image>` glitch), producing raw `1/10`. We immediately re-ran both tasks cleanly at `--workers 2`; both passed (`sh_005` in `99.5s`, `sh_019` in `121.4s`). Every Antigravity comparison in this report uses the **healthy 3/10** Antigravity baseline.*

*†Token-accounting note: Vertex AI `promptTokenCount` (`flawed` `in_tok`) **includes** `cachedContentTokenCount` (`cache_pct = cached / in_tok`), whereas `agyworker` reports uncached `input_tokens` separately from `cached_tokens` (`total_prompt = in_tok + cached_tok`, `cache_pct = cached / (in_tok + cached)`). Both are normalized onto the same total-prompt basis in the summary table above.*

---

## 3. Experimental Setup & Methodology

- **Benchmark corpus:** Real historical software engineering tasks from `evals/session-history/tasks/`, each defined by an exact historical `base_sha`, an authentic user prompt, and an automated command grader (`coax test`, `coax check`, `py_compile`, Swift/C checks, etc.).
- **Stratified splits (zero data leakage):**
  - **`dev10` (10 tasks — 2 per difficulty bucket A–E):** Used to diagnose bottlenecks and iterate from Round 0 → v1 → v2 → v3 → v4.
    - Bucket A (edit precision): `sh_001_claude_todos_wrap`, `sh_005_claude_large_chat_open`
    - Bucket B (multi-file): `sh_008_claude_sidebar_composer_parity`, `sh_012_openagents_inkling_parser`
    - Bucket C (verification): `sh_003_antigravity_utf8_cleanup`, `sh_014_claude_inline_code_padding`
    - Bucket D (error recovery): `sh_018_claude_checker_budget_e0381`, `sh_019_claude_sign_realtime_clock`
    - Bucket E (long context): `sh_023_claude_agents_md_autoload`, `sh_025_codex_claude_history_compact`
  - **`heldout6` (6 unseen tasks — never inspected during hillclimbing):** Evaluated once after v4 froze to verify out-of-sample generalization: `sh_002_openagents_worker_recall`, `sh_011_antigravity_backend_foot_label`, `sh_013_antigravity_mobile_row_id`, `sh_022_claude_desktop_core_stack`, `sh_024_codex_mount_why_diagnostics`, `sh_027_antigravity_skeleton_shimmer`.
- **Workspace isolation:** Every `(arm, task)` run executes inside a fresh detached `git worktree` checked out at `base_sha`, with `coax/bin/coax` symlinked and `GIT_CONFIG` `remote.origin.pushurl=no_push` preventing accidental pushes.
- **Identical model & budget parameters:**
  - **Antigravity arm (`agyworker`):** `CODER_AGY_MODE=turbo`, `CODER_AGY_INTERACTIVE=0`, `model=gemini-4-neon`, `effort=high`.
  - **Flawed arm (`flawedworker`):** `FLAWED_BACKEND=vertex`, `FLAWED_PERMISSION_MODE=allow_all`, `FLAWED_MAX_TURNS=48`, `model=flawed:gemini-4-neon`, `effort=high`.
  - **Wall-clock cap:** `300s` per task; a task only counts as `PASS` if it finishes cleanly before the timeout **and** the external command grader exits `0`.
  - **Strict anti-overfitting rule:** Every harness change was general software-engineering infrastructure (prompts, wire protocol, tool ergonomics, budgets, caching); zero task-specific rules, repo-specific hacks, or hidden effort downgrades (`thinkingLevel="high"` remained `high` throughout).

---

## 4. Round-by-Round Progression (`dev10`)

| Round / Arm | Pass Rate | Timeouts | Total Wall (Avg) | Solved Common-3 Avg (`sh_001/005/019`) | Total Turns | Total Tools | Edits / Fails | HTTP Retries | Total Prompt Tokens | Output + Think Tokens | Vertex Cost |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **Antigravity · Neon (healthy)** | 3/10 (30%) | 7 | 2418.1s (241.8s) | 102.5s | —\* | 236 | 6 / 0 | 0 | 8,386,183 | 32,488\* | — |
| **Flawed R0 · Neon (baseline)** | 3/10 (30%) | 7 | 2641.7s (264.2s) | 177.0s | 211 | 443 | 6 / 0 | 1 | 14,996,546 | 137,642 | \$9.26 |
| **Flawed v1 · Neon** | 3/10 (30%) | 7 | 2360.7s (236.1s) | 83.4s | 135 | 342 | 6 / 0 | 0 | 13,460,472 | 133,650 | \$9.51 |
| **Flawed v2 · Neon** | 3/10 (30%) | 7 | 2398.2s (239.8s) | 95.8s | 191 | 540 | 8 / 0 | 16† | 8,778,419 | 151,819 | \$8.14 |
| **Flawed v3 · Neon** | 3/10 (30%) | 7 | 2315.3s (231.5s) | 68.2s | 160 | 460 | 8 / 0 | 16† | 8,180,802 | 150,305 | \$8.41 |
| **Flawed v4 · Neon (shipped)** | **6/10 (60%)** | **4** | **2150.8s (215.1s)** | **59.7s** | **105** | **215** | **27 / 0** | **0** | **5,459,594** | **155,805** | **\$5.85** |

*\*`agyworker` reports one outer task attempt per run and excludes internal thought tokens from `out_tok`, whereas Vertex Gemini `output_tokens` sums both visible candidate tokens and `thoughtsTokenCount`.*  
*†In Rounds 2 & 3, the newly introduced `<system-reminder>` text block exposed a pre-existing Vertex Gemini wire bug (`HTTP 400` on sibling `text` parts inside `functionResponse` turns), causing 2 history-wipe retries per task across 8 multi-turn tasks (`16` retries total). Fixing that wire bug in v4 dropped retries `16 → 0` and immediately doubled the pass rate `3/10 → 6/10`.*

---

## 5. Autopsy Findings & Architectural Improvements (Claude Code & T3 Code Sources)

Each round was driven by direct trajectory inspection paired with patterns audited from the **Claude Code teardown** (`/Users/christopherdavid/work/cc/`) and **T3 Code teardown** (`docs/t3/` + `coax/lib/agent/engine/cache-policy.coax`):

### 5.1 Round 0 Autopsy → Flawed v1 (2.12× faster solved tasks, beating AGY 3–0 on speed)

Six concrete harness failures were diagnosed in Round 0 logs:
1. **Unsolicited `git commit`/`push` churn (`40–65s` wasted per solved task):** `~/.openagents/instructions.md` instructed committing and pushing after checks passed. On `sh_001`, Flawed solved and verified at `86.0s` (tying Antigravity's `86.6s`), then burned `63.6s` (10 extra turns) attempting `git commit`/`push`. Same on `sh_005` (`67.4s → 104.7s`).
2. **Head-only Bash truncation:** `coax test` truncated off the final pass/fail summary line, forcing full duplicate re-runs with `| tail -n 30`.
3. **250-line `Read` without total lines or dedup:** Caused 2×–4× redundant reads (`44` reads / `20` duplicate paths on `sh_023`; `30` reads / `11` duplicate paths on `sh_008`).
4. **Crippled `Grep`:** Prepended ~90-char absolute `/var/folders/...` worktree paths to every line, hardcoded `--max-count=10`, and lacked `output_mode`, `-C`/`-A`/`-B`, `-i`, `type`, and `offset`.
5. **Truncated `thoughtSignature` → catastrophic context wipe (`sh_014`):** After a 14.4K-token thinking turn, Gemini's `thoughtSignature` overflowed `sig_buf` (`32 KiB`), Vertex returned `HTTP 400`, and reactive recovery invoked `fww_emergency_compact(level=3)`, wiping `53,114 → 2,366` tokens and forcing a restart from Turn 0.
6. **Turn-1 `git status` overhead & stale cloud cache:** 9/10 tasks wasted Turn 1 running `git status && git log`.

**Implemented in v1:**
- **Cache-stable session git snapshot** (`cc/context.ts` → `fww_capture_git_snapshot` in `flawedworker.coax`): captures branch, short status, and recent 5 commits once per session/cwd via `git --no-optional-locks`, eliminating Turn-1 git calls without busting prompt caching.
- **Git safety override + 4 KiB per-file instruction caps + parallel tool-batching prompt** (`cc/constants/prompts.ts` → `fww_build_system_prompt`, `fww_append_instr_capped`).
- **Upgraded `Read`** (`cc/tools/FileReadTool/` → `fww_tool_read`): total-line header (`[File: ... | total lines: N | showing from line M]`), explicit `offset=K` continuation hint, `FILE_UNCHANGED_STUB` dedup cache (`FwReadDedup`, invalidated on `Edit`/`Write`/compaction), and `FWW_FILE_MAX` bumped to `512 KiB`.
- **Upgraded `Edit`** (`cc/tools/FileEditTool/` → `fww_tool_edit`): trailing-whitespace/CRLF fallback matcher (`fww_find_trimmed_match`), nearest-line `±5` diagnostic snippet on miss (`fww_find_nearest_line`), match line numbers on multi-match, and updated numbered snippet on success.
- **Full-parity `Grep`** (`cc/tools/GrepTool/` → `fww_tool_grep` + `flawed/wire.coax`): workspace-relative paths via `proc_cwd`, `output_mode` (`content`, `files_with_matches`, `count`), `-C`/`-A`/`-B`, `-i`/`case_insensitive`, `type`, `head_limit`, `offset`, and explicit truncation hints.
- **Streaming Head+Tail Bash ring buffer** (`FwAsyncCmd`, `fww_cmd_push_line`, `fww_cmd_finalize`, `fww_cmd_tool_result`): preserves both leading errors and trailing test summaries with an explicit omitted-byte marker and `[Exit code: N]` on non-zero exits (plus partial output on timeouts).
- **`128 KiB` `thoughtSignature` buffers + surgical repair** (`fww_hist_strip_thought_sigs`): expanded `sig_buf` and `acc.sig` to `128 KiB`, `tool_sig_slab` and `sse_line` to `256 KiB`, `req_body` to `1 MiB`, and replaced destructive level-3 compaction on signature errors with Gemini's official `skip_thought_signature_validator` sentinel so 100% of conversation history is preserved.
- **T3 / Layer-2 idle prompt-cache compaction** (`coax/lib/agent/engine/cache-policy.coax` `cp_cache_status`): automatically runs free in-place microcompaction when a cloud session resumes after the 5-minute prompt-cache TTL.

### 5.2 Rounds 1–3 Autopsies → Flawed v2, v3, v4 (3/10 → 6/10, 0 retries, -36.8% cost)

Round 1 beat Antigravity on every solved task (`83.4s` vs. `102.5s`), yet all 7 harder tasks still timed out at `300s` with `0 B` diff: trajectories showed Flawed had all target files read within `12–61s`, then spent the remaining `240–288s` in open-ended exploration (`24–40` files read, `154K–288K` tokens, `$3.08` on `sh_023`) and `60–165s` single-turn mental-simulation spirals.

Three decisive findings drove v2 → v3 → v4:
1. **Claude Code `# Output efficiency` + bounded exploration + `<system-reminder>` checkpoints (`cc/constants/prompts.ts:132, 200-242, 416-428`):**
   - Instructed Flawed to prefer the simplest direct implementation, batch independent `Edit` calls in one response, verify early via fast compiler/test runs instead of multi-minute mental simulation, and stop immediately once verified.
   - Added cache-safe `<system-reminder>` pacing nudges (appended only to the newest `TOOL_RESULTS` turn, never mutating the cached prefix): Stage 1 at 4 read-only turns (or 3 turns with `≥96 KiB` tool output), Stage 2 at 7 read-only turns, and a finish-when-verified reminder at 7 post-edit steps.
   - Tightened per-tool caps (`Grep`: `240`-char line cap, `16 KiB` total, `--max-count=15`, default `head_limit=60`; `Bash`: `8 KiB` head + `16 KiB` tail; `Glob`: skip `build`, `target`, `dist`, `.venv`, `.cache`, `DerivedData`).
2. **Rediscovering Claude Code's `FileReadTool: maxResultSizeChars: Infinity` invariant (`cc/book/chapters/06-executing-tools.md:335`) in Round 2 → fixed in v3:**
   - Round 2's Stage-B aggregate tool-result budget (`64 KiB` cloud / `16 KiB` local) initially trimmed `Read` outputs *after* `FwReadDedup` had cached `end_line`, causing re-read thrashing (`16` reads of `loop.coax` on `sh_023`, `13` reads of `agyworker.coax` on `sh_003`).
   - v3 strictly exempted `Read` from Stage-B (`nm_s != "Read"`), kept `Read` cleanly self-bounded (`600` lines / `32 KiB` cloud, `300` lines / `12 KiB` local), and upgraded `FwReadDedup` to sound sub-range coverage (`offset >= slot.offset && req_end <= slot.end_line`).
3. **Critical latent Vertex AI Gemini wire bug caught in Round 3 → fixed in v4:**
   - Inspecting Round 3's `wire.jsonl` revealed that every multi-turn task received `HTTP 400` on the exact turns where `<system-reminder>` fired (turns 4 and 8), dropping context `30K–68K → ~3K` twice per task.
   - Root cause: `fww_build_gemini_body` emitted a sibling `{"text": ...}` part inside a `role: "user"` `functionResponse` turn—which Vertex AI Gemini strictly forbids (affecting not only `<system-reminder>` nudges, but also Desktop mid-turn steers `MWW_STEER` and wrap-up prompts!).
   - **Fix in v4 (`coder/tui/flawedworker.coax` `fww_build_gemini_body`):** Fold any trailing `FWW_MB_TEXT` blocks cleanly onto the end of the last `functionResponse.response.output` string (`\n\n`-separated), guaranteeing `parts[]` contains **only** `functionResponse` parts matching `functionCall` 1-to-1.
   - **Immediate impact:** Retries dropped `16 → 0`, `dev10` pass rate doubled `3/10 → 6/10` (`sh_008`, `sh_012`, `sh_014` all flipped `TIMEOUT → PASS`; `sh_023` went from `0 B` diff / `\$3.08` to `6.3 KB` + new `instructions.coax` / `\$0.91`), and total `dev10` cost fell `\$9.26 → \$5.85`.

---

## 6. Complete Per-Task Telemetry

### 6.1 `dev10` Stratified Slice (`effort=high`, `timeout=300s`)

| Task (Bucket) | Antigravity · Neon (Healthy) | Flawed R0 · Neon | Flawed v1 | Flawed v4 (Shipped) | Flawed v4 Details |
|---|---|---|---|---|---|
| `sh_001_claude_todos_wrap` (A) | **PASS** `86.6s` (12 tools, 255K tok) | **PASS** `149.6s` (19T, 27 tools, \$0.55) | **PASS** `75.0s` (8T, 12 tools, \$0.16) | **PASS `63.2s`** | 8T, 11 tools, `1/0` ed, 97K in (`71.3%` ca), 3.4K out, \$0.15, `648 B` diff |
| `sh_005_claude_large_chat_open` (A) | **PASS** `99.5s` (6 tools, 163K tok) | **PASS** `104.7s` (16T, 15 tools, \$0.24) | **PASS** `64.6s` (5T, 4 tools, \$0.07) | **PASS `29.2s`** | 5T, 4 tools, `1/0` ed, 27K in (`43.0%` ca), 1.4K out, \$0.07, `874 B` diff |
| `sh_008_claude_sidebar_composer_parity` (B) | TO `301.5s` (36 tools, `0 B`) | TO `301.5s` (21T, 57 tools, \$1.42) | TO `301.5s` (19T, 43 tools, \$1.03) | **PASS `216.4s`** | 15T, 29 tools, `5/0` ed, 982K in (`80.1%` ca), 18.1K out, \$0.98, `9,288 B` diff |
| `sh_012_openagents_inkling_parser` (B) | TO `301.5s` (37 tools, `0 B`) | TO `301.5s` (19T, 40 tools, \$0.66) | TO `301.5s` (10T, 32 tools, \$0.48) | **PASS `250.3s`** | 12T, 24 tools, `4/0` ed, 463K in (`83.7%` ca), 22.4K out, \$0.62, 2 new `.coax` files |
| `sh_003_antigravity_utf8_cleanup` (C) | TO `301.5s` (14 tools, `12.3 KB` diff) | TO `301.5s` (13T, 23 tools, \$0.74) | TO `301.5s` (13T, 20 tools, \$0.72) | TO `301.5s` | 11T, 20 tools, `0/0` ed, 567K in (`82.0%` ca), 20.0K out, \$0.68 |
| `sh_014_claude_inline_code_padding` (C) | TO `301.5s` (16 tools, `0 B`) | TO `301.5s` (16T, 29 tools, \$0.69, 1 retry) | TO `301.5s` (12T, 15 tools, \$0.72) | **PASS `298.9s`** | 11T, 17 tools, `6/0` ed, 441K in (`72.1%` ca), 30.9K out, \$0.88, `14,476 B` diff |
| `sh_018_claude_checker_budget_e0381` (D) | TO `301.5s` (21 tools, `0 B`) | TO `301.5s` (26T, 67 tools, \$1.24) | TO `301.5s` (19T, 62 tools, \$1.42) | TO `301.5s` | 15T, 32 tools, `0/0` ed, 1.30M in (`90.1%` ca), 16.9K out, \$0.81 |
| `sh_019_claude_sign_realtime_clock` (D) | **PASS** `121.4s` (13 tools, 281K tok) | **PASS** `276.8s` (23T, 38 tools, \$0.69) | **PASS** `110.5s` (14T, 27 tools, \$0.55) | **PASS `86.8s`** | 9T, 18 tools, `4/0` ed, 257K in (`82.0%` ca), 7.4K out, \$0.28, `5,162 B` diff |
| `sh_023_claude_agents_md_autoload` (E) | TO `301.5s` (20 tools, `0 B`) | TO `301.5s` (33T, 85 tools, \$1.66) | TO `301.5s` (19T, 64 tools, \$3.08) | TO `301.5s` | 8T, 34 tools, `6/0` ed, 842K in (`82.6%` ca), 24.4K out, \$0.91, `6,303 B` + `instructions.coax` |
| `sh_025_codex_claude_history_compact` (E) | TO `301.5s` (61 tools, `0 B`) | TO `301.5s` (25T, 62 tools, \$1.38) | TO `301.5s` (16T, 63 tools, \$1.27) | TO `301.5s` | 11T, 26 tools, `0/0` ed, 488K in (`82.2%` ca), 10.8K out, \$0.48 |

### 6.2 `heldout6` Unseen Generalization Slice (`effort=high`, `timeout=300s`)

| Task (Bucket) | Antigravity · Neon | **Flawed v4 · Neon** | Head-to-Head Comparison |
|---|---|---|---|
| `sh_002_openagents_worker_recall` (A) | **PASS** `106.2s` (14 tools, 427K tok, `1/0` ed) | **PASS `64.0s`** (7T, 9 tools, 75K tok, `1/0` ed, \$0.14, `2,255 B` diff) | Flawed v4 **39.7% faster**, **36% fewer tools**, **5.7× fewer prompt tokens** |
| `sh_011_antigravity_backend_foot_label` (B) | TO `301.5s` (46 tools, 2.50M tok, `0` ed) | TO `301.5s` (21T, 34 tools, 1.86M tok, `0` ed, \$1.03) | Tie (both timed out; Flawed 26% fewer tools & tokens) |
| `sh_013_antigravity_mobile_row_id` (C) | TO `301.5s` (54 tools, 2.63M tok, `0` ed) | FAIL `295.5s` (24T, 41 tools, 1.60M tok, `6/0` ed, \$1.09, `5,038 B` diff) | Flawed v4 completed 6 edits (`5 KB` diff) before grader check; AGY made 0 edits |
| `sh_022_claude_desktop_core_stack` (D) | TO `301.5s` (24 tools, 492K tok, `0` ed) | **PASS `255.7s`** (10T, 16 tools, 242K tok, `2/0` ed, \$0.39, `4,926 B` diff) | **Flawed v4 PASSES**, AGY times out with 0 edits (Flawed 2× fewer tokens) |
| `sh_024_codex_mount_why_diagnostics` (E) | TO `301.5s` (47 tools, 2.46M tok, `0` ed) | TO `301.5s` (8T, 27 tools, 664K tok, `0` ed, \$0.62) | Tie (Flawed 43% fewer tools, 3.7× fewer tokens) |
| `sh_027_antigravity_skeleton_shimmer` (E) | TO `301.5s` (37 tools, 1.22M tok, `0` ed) | TO `301.5s` (9T, 29 tools, 799K tok, `5/0` ed, \$0.99, `10,718 B` diff) | Flawed v4 wrote `10.7 KB` across 5 edits; AGY made 0 edits |

---

## 7. Verification, Bundle Budget & Desktop Deployment

All checks ran directly against the shipped commit `d7be8ab43b`:

- **Authority unit & mutation checks:**
  - `coax/bin/coax test flawed/wire.coax --library`: **119/119 passed**
  - `coax/bin/coax mutate flawed/wire.coax --root-only`: **577 mutants, 565 killed, 0 survived, 12 marked equivalent**
- **Worker & Desktop unit checks:**
  - `coax/bin/coax test coder/tui/flawedworker.coax --library`: **267/267 passed** (including new tests for Read sub-range dedup/invalidation, Edit trimmed-whitespace fallback & nearest-line diagnostics, streaming Head+Tail Bash + exit code, surgical `thoughtSignature` repair, and 1-to-1 Gemini `functionResponse` text folding)
  - `coax/bin/coax test apps/openagents-desktop/app-v1.coax --library`: **513/513 passed**
  - `coax/bin/coax test apps/openagents-desktop/page.coax --library`: **894/894 passed**
- **Live multi-turn Vertex AI Neon smoke test:** 3 turns (`Read` tool inspection → follow-up question → `/status`), all finished cleanly with `0` retries.
- **Repository & pre-push gates:** `./scripts/check.sh` passed (`every check passed`); rebased cleanly onto `origin/main` and pushed `d7be8ab43b` (`prepush: ok`).
- **Desktop build, bundle budget & install:**
  - `./apps/openagents-desktop/install.sh --minimal`: installed `/Applications/OpenAgents.app` (`1.0.0`, build `34353`, `OACommit = d7be8ab43b5c894a8595eb094b867f555584f335`, `OAProfile = minimal`).
  - `v1-bundle-check`: **`PASS` (`14,643,200` B / `14 MiB` macOS Stage 2 budget; `36.8 KiB` headroom)**.
  - Synced `/Applications/OpenAgents.app/Contents/Resources/flawedworker` → `~/.openagents/bin/flawedworker` and verified `--auth` (`ready: true`).

---

## 8. Caveats & Next Opportunities

- **Caveats:**
  - `300s` per-task cap is tight for `effort=high` multi-file tasks with deep reasoning; both arms hit `300s` on several Bucket C–E tasks (`sh_003`, `sh_018`, `sh_023`, `sh_025`, `sh_011`, `sh_024`, `sh_027`). Raising the timeout to `600s` would separate pure latency limits from genuine capability limits—notably `sh_023` (`6.3 KB` + `instructions.coax`) and `sh_027` (`10.7 KB` across 5 edits) were already mid-verification when killed at `300s`.
  - Sample size is `N = 16` (`10` dev + `6` heldout), so single-task variance is ±6.25 pp, though the 2× pass-rate advantage held identically across both slices (`6/10` vs. `3/10` and `2/6` vs. `1/6`) and Flawed won 4–0 on solve speed on every commonly solved task.
- **Highest-leverage next steps for v5:**
  1. **True per-step adaptive `thinkingLevel` (`FLAWED_THINK_ADAPTIVE`):** Use `medium` on pure mechanical verification/test-run steps after edits are written while keeping `high` on planning/implementation/debugging steps—reclaiming 30–60s of decode time on long tasks.
  2. **Parallel `Bash` execution:** Support running multiple independent read-only shell commands (`coax check A`, `coax check B`) concurrently instead of serially in one `FwAsyncCmd` slot.
  3. **Targeted `Read` outline mode / symbol lookup:** For large `.coax`/`.swift`/`.c` files (>800 lines), expose a compact symbol/line-index preview when requested so Bucket E tasks don't burn 4–5 turns locating functions.
