# Flawed · Gemini 4 Neon vs Antigravity · Gemini 4 Neon — Head-to-Head Benchmark & Harness Hillclimb

Date: 2026-11-25  
Model: `gemini-4-neon` (Vertex AI) on both arms, identical `effort=high`, `300s` per-task wall cap  
Benchmark: 16 real repository engineering tasks from `evals/session-history/tasks/` (`dev10` stratified across Buckets A–E + 6 unseen `heldout6` tasks), graded in isolated detached git worktrees (`coax/bin/coax` symlinked, `remote.origin.pushurl=no_push`)

## Executive Summary

| Benchmark Slice | Antigravity · Neon (`high`) | Flawed R0 · Neon (`high`) | **Flawed v4 · Neon (`high`)** | Delta vs AGY / R0 |
|---|---:|---:|---:|---|
| **`dev10` (Buckets A–E, 10 tasks)** | 3/10 (30.0%)* | 3/10 (30.0%) | **6/10 (60.0%)** | **2.0× pass rate (+30pp)** |
| **`heldout6` (unseen, 6 tasks)** | 1/6 (16.7%) | — | **2/6 (33.3%)** | **2.0× pass rate (+16.7pp)** |
| **Combined (16 tasks)** | **4/16 (25.0%)** | — | **8/16 (50.0%)** | **2.0× pass rate** |
| **Mean solve time (4 common passes)** | 103.4s | 177.0s *(3 dev10)* | **60.8s** | **41.2% faster than AGY, 2.96× faster than R0** |
| **Edit failures (across all 16 tasks)** | 0 | — | **0 / 41 edits** | 100% edit precision |
| **HTTP / wire retries (all 16 tasks)** | 0 | 1 (`thoughtSignature` wipe) | **0** | Zero wire errors |
| **`dev10` turns / tools / Vertex cost** | — | 211 turns / 443 tools / \$9.26 | **105 turns / 215 tools / \$5.85** | **-50% turns, -51% tools, -36.8% cost** |

*\*Note on fairness: In the initial 6-worker parallel run, Antigravity Neon hit two transient errors (`sh_019` `plan model not specified` startup race at `0.4s` and `sh_005` `<end_of_image>` glitch at `17.6s`). Both were re-run cleanly at `--workers 2` and passed (`99.5s` and `121.4s`), so all comparisons above use the healthy 3/10 Antigravity Neon baseline.*

---

## Round-by-Round Autopsy & Teardown-Driven Improvements

Every change was general-purpose harness engineering grounded in trajectory autopsies and audited directly against the **Claude Code teardown** (`/Users/christopherdavid/work/cc/`) and **T3 Code teardown** (`docs/t3/` + `coax/lib/agent/engine/cache-policy.coax`):

### Round 0 Autopsy → Flawed v1 (`dev10`: 3/10, solved avg `177.0s → 83.4s`)
Empirical failures observed in Round 0 trajectories:
1. **Unsolicited `git commit`/`push` churn (`40–65s` wasted per solved task):** `~/.openagents/instructions.md` instructed committing/pushing, costing 10 extra turns on `sh_001` (`86.0s` solved → `149.6s` finish) and `37.3s` on `sh_005`.
2. **Bash head-only truncation:** `coax test` summary lines at the tail were dropped, forcing duplicate `| tail -n 30` runs.
3. **250-line `Read` without total lines or dedup:** Caused 2×–4× redundant reads (`44` reads / `20` duplicate paths on `sh_023`).
4. **Crippled `Grep`:** Emitted 90-char absolute worktree paths on every line, hardcoded `--max-count=10`, and lacked `output_mode`, `-C/-A/-B`, `-i`, `type`, `offset`.
5. **Truncated `thoughtSignature` → catastrophic history wipe (`sh_014`):** A 14.4K-token thinking turn overflowed `sig_buf` (`32 KiB`), Vertex returned HTTP 400, and recovery invoked `fww_emergency_compact(level=3)`, wiping `53,114 → 2,366` tokens.
6. **Turn-1 `git status` overhead & stale cloud cache:** 9/10 tasks spent Turn 1 on `git status && git log`.

**Implemented in v1 (`flawed/wire.coax`, `coder/tui/flawedworker.coax`):**
- **Cache-stable session git snapshot** (`cc/context.ts` pattern): captured once per session/cwd via `git --no-optional-locks`, eliminating Turn-1 `git status` while preserving prompt-cache stability.
- **Git safety override + 4 KiB per-file instruction caps + parallel tool-batching prompt** (`cc/constants/prompts.ts`).
- **Upgraded `Read` tool** (`cc/tools/FileReadTool/`): total-line header, explicit `offset=N` continuation hint, `FILE_UNCHANGED_STUB` dedup cache (invalidated on `Edit`/`Write`/compaction), `FWW_FILE_MAX` raised to `512 KiB`.
- **Upgraded `Edit` tool** (`cc/tools/FileEditTool/`): trailing-whitespace/CRLF fallback matcher, nearest-line `±5` diagnostic snippet on miss, match line numbers on multi-match, updated numbered context snippet on success.
- **Full-parity `Grep` tool** (`cc/tools/GrepTool/`): workspace-relative paths via `proc_cwd`, `output_mode` (`content`/`files_with_matches`/`count`), `-C`/`-A`/`-B`, `-i`, `type`, `head_limit`, `offset`.
- **Streaming Head+Tail Bash ring buffer** (`FwAsyncCmd`) with middle omitted-byte counter and explicit `[Exit code: N]` on non-zero exits.
- **`128 KiB` `thoughtSignature` buffers + surgical repair (`fww_hist_strip_thought_sigs`):** Swaps rejected signatures to Gemini's `skip_thought_signature_validator` sentinel instead of wiping conversation history.
- **T3 / Layer-2 idle cloud prompt-cache compaction** (`coax/lib/agent/engine/cache-policy.coax` `cp_cache_status`): automatically microcompacts old tool outputs for free when a cloud session resumes after the 5-minute cache TTL.

### Round 1–3 Autopsies → Flawed v2 / v3 / v4 (`dev10`: 3/10 → **6/10**, cost `\$9.51 → \$5.85`)
Round 1 beat Antigravity on all 3 solved tasks (`83.4s` vs `102.5s`), but all 7 harder tasks still timed out at `300s` with `0 B` diff due to exploration spirals (16–19 read-only turns, `154K–288K` context bloat) and long mental-simulation thinking passes.

1. **v2 (Claude Code `# Output efficiency` + Stage-B tool budget + `<system-reminder>` pacing):**
   - Slashed `sh_005` to `33.1s`, `sh_001` to `46.0s`, and cut `sh_023` cost from `\$3.08 → \$1.31`.
   - Caught an empirical bug reproducing Claude Code's exact `FileReadTool: maxResultSizeChars: Infinity` invariant (`cc/book/chapters/06-executing-tools.md:335`): Stage-B trimming `Read` outputs after dedup cached `end_line` caused re-read thrashing (`16` reads of `loop.coax`).
2. **v3 (Read opt-out of Stage-B + sub-range dedup + finish-when-verified checkpoint):**
   - Exempted `Read` from Stage-B aggregate trimming (self-bounded at `600` lines / `32 KiB` cloud, `300` lines / `12 KiB` local); kept Stage-B (`64 KiB` cloud / `16 KiB` local) for non-Read tools (`Grep`, `Bash`, `Glob`, `WebFetch`), capped `Grep` lines at `240` chars / `16 KiB`, tightened Bash head+tail to `8 KiB + 16 KiB`, skipped build directories in `Glob`, and added sub-range `Read` dedup coverage.
   - Caught a **critical latent Vertex AI Gemini wire bug** in `fww_build_gemini_body`: emitting a sibling `{"text": ...}` part inside a `functionResponse` turn (triggered by `<system-reminder>` nudges, Desktop mid-turn steers, and wrap-up prompts) returned `HTTP 400` from Vertex AI and invoked `fww_emergency_compact(level=3)`, wiping history twice per task (`~30K–68K → ~3K` tokens at turns 4 and 8).
3. **v4 (Strict 1-to-1 Gemini `functionResponse` wire folding):**
   - Folded any trailing `FWW_MB_TEXT` blocks (`<system-reminder>`, mid-turn steers, wrap-up prompts) cleanly into the last `functionResponse.response.output` payload in `fww_build_gemini_body`, ensuring `parts[]` strictly contains only `functionResponse` parts matching `functionCall` 1-to-1.
   - Result: **0 HTTP retries across all 16 tasks**, `dev10` jumped from **3/10 → 6/10**, and unseen `heldout6` doubled Antigravity (**2/6 vs 1/6**).

---

## Per-Task Breakdown

### `dev10` Stratified Slice (`effort=high`, `timeout=300s`)

| Task | Bucket | Antigravity Neon | Flawed R0 Neon | Flawed v1 | **Flawed v4** | Flawed v4 Telemetry |
|---|---|---:|---:|---:|---:|---|
| `sh_001_claude_todos_wrap` | A | PASS `86.6s` | PASS `149.6s` | PASS `75.0s` | **PASS `63.2s`** | 8 turns, 11 tools, `1/0` edits, `\$0.15` |
| `sh_005_claude_large_chat_open` | A | PASS `99.5s` | PASS `104.7s` | PASS `64.6s` | **PASS `29.2s`** | 5 turns, 4 tools, `1/0` edits, `\$0.07` |
| `sh_008_claude_sidebar_composer_parity` | B | TO `301.5s` | TO `301.5s` | TO `301.5s` | **PASS `216.4s`** | 15 turns, 29 tools, `5/0` edits, `\$0.98` |
| `sh_012_openagents_inkling_parser` | B | TO `301.5s` | TO `301.5s` | TO `301.5s` | **PASS `250.3s`** | 12 turns, 24 tools, `4/0` edits, `\$0.62` |
| `sh_003_antigravity_utf8_cleanup` | C | TO `301.5s` | TO `301.5s` | TO `301.5s` | TO `301.5s` | 11 turns, 20 tools, `0` edits, `\$0.68` |
| `sh_014_claude_inline_code_padding` | C | TO `301.5s` | TO `301.5s` | TO `301.5s` | **PASS `298.9s`** | 11 turns, 17 tools, `6/0` edits, `\$0.88` |
| `sh_018_claude_checker_budget_e0381` | D | TO `301.5s` | TO `301.5s` | TO `301.5s` | TO `301.5s` | 15 turns, 32 tools, `0` edits, `\$0.81` |
| `sh_019_claude_sign_realtime_clock` | D | PASS `121.4s` | PASS `276.8s` | PASS `110.5s` | **PASS `86.8s`** | 9 turns, 18 tools, `4/0` edits, `\$0.28` |
| `sh_023_claude_agents_md_autoload` | E | TO `301.5s` | TO `301.5s` | TO `301.5s` | TO `301.5s` | 8 turns, 34 tools, `6/0` edits (`6.3 KB` + new file), `\$0.91` |
| `sh_025_codex_claude_history_compact` | E | TO `301.5s` | TO `301.5s` | TO `301.5s` | TO `301.5s` | 11 turns, 26 tools, `0` edits, `\$0.48` |

### `heldout6` Unseen Generalization Slice (`effort=high`, `timeout=300s`)

| Task | Antigravity Neon | **Flawed v4 Neon** | Flawed v4 Telemetry |
|---|---:|---:|---|
| `sh_002_openagents_worker_recall` | PASS `106.2s` (14 tools) | **PASS `64.0s`** | 7 turns, 9 tools, `1/0` edits, `\$0.14` |
| `sh_011_antigravity_backend_foot_label` | TO `301.5s` (46 tools) | TO `301.5s` | 21 turns, 34 tools, `0` edits, `\$1.03` |
| `sh_013_antigravity_mobile_row_id` | TO `301.5s` (54 tools) | FAIL `295.5s` | 24 turns, 41 tools, `6/0` edits (`5.0 KB`), `\$1.09` |
| `sh_022_claude_desktop_core_stack` | TO `301.5s` (24 tools) | **PASS `255.7s`** | 10 turns, 16 tools, `2/0` edits (`4.9 KB`), `\$0.39` |
| `sh_024_codex_mount_why_diagnostics` | TO `301.5s` (47 tools) | TO `301.5s` | 8 turns, 27 tools, `0` edits, `\$0.62` |
| `sh_027_antigravity_skeleton_shimmer` | TO `301.5s` (37 tools) | TO `301.5s` | 9 turns, 29 tools, `5/0` edits (`10.7 KB`), `\$0.99` |

---

## Verification

- `coax/bin/coax test flawed/wire.coax --library`: **119/119 passed**
- `coax/bin/coax mutate flawed/wire.coax --root-only`: **577 mutants, 565 killed, 0 survived, 12 marked equivalent**
- `coax/bin/coax test coder/tui/flawedworker.coax --library`: **267/267 passed** (including new tests for Read dedup/sub-range/invalidation, Edit trimmed-whitespace fallback & nearest-line diagnostics, streaming head+tail Bash + exit code, surgical `thoughtSignature` repair, and Gemini 1-to-1 `functionResponse` folding of trailing text/steers/reminders)
- Desktop integration, `./scripts/check.sh`, `./apps/openagents-desktop/build.sh --minimal`, `v1-bundle-check`, and installed `/Applications/OpenAgents.app` verified cleanly.
