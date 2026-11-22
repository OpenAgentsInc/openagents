# Session History Evals (`ATIF-v1.8`)

This directory holds the Session History Evals extraction pipeline, schemas, and sanitized benchmark tasks specified in [`docs/coder/session-history-evals-spec.md`](../../docs/coder/session-history-evals-spec.md) (GitHub `#13830`).

## Overview

Real engineering trajectories from local agent harnesses (**Claude Code**, **Codex**, **OpenAgents Desktop**, **Antigravity**, and **OpenCode**) are converted read-only into canonical Harbor **`ATIF-v1.8`** (`.atif.jsonl` logs + `openagents.atif-manifest.v1` manifests, per [`NIP-ATIF`](../../nostr/nips/openagents/NIP-ATIF.md)) using the shared library in `coax/lib/nostr/atif/`.

Raw histories and converted `.atif.jsonl` caches stay private on disk under `~/.openagents/evals/session-history/` and are **never** committed to git.

## Layout

Repository (`evals/session-history/`):

- `README.md` — this guide
- `run.sh` — thin CLI entrypoint (`ingest`, `mine`, `sanitize`, `run`, `compare`)
- `schema/task.schema.json` — sanitized task schema compatible with `evals/openagents-desktop/tier2` and `tier4/golden-template.json`, extended with ATIF `source` provenance and `baseline` metrics
- `schema/baseline-metrics.schema.json` — schema for `index.jsonl` entries, `inventory.json`, and baseline comparison summaries
- `tools/ingest.coax` — read-only Stage 1 converter (`ATIF-v1.8` logs + manifests + `index.jsonl` + `inventory.json`)
- `tools/mine.coax` — Stage 2 episode miner & Bucket A–E stratifier over `.atif.jsonl` logs
- `tools/sanitize.coax` — Stage 3 secret/PII scrubber, path normalizer, and git-worktree hermeticity gate (`--verify`)
- `tools/compare.coax` — Stage 5 baseline comparison scorecard and failure-taxonomy tagger
- `tasks/` — committed sanitized, hermeticity-verified eval task JSONs
- `results/` — gitignored run outputs (`summary.json`, `scorecard.md`, per-case artifacts)

Local private store (`~/.openagents/evals/session-history/`):

```text
~/.openagents/evals/session-history/
  atif/<source>/<trajectory_id>.atif.jsonl
  atif/<source>/<trajectory_id>.manifest.json
  index.jsonl
  inventory.json
```

## Usage (Phase 1 — Ingest & Index)

Run `ingest.coax` via `coax/web/tool.sh`:

```sh
# Sample 5 newest sessions per source across all supported harnesses
coax/web/tool.sh evals/session-history/tools/ingest.coax --limit 5

# Ingest a single source harness (claude | codex | openagents | antigravity | opencode)
coax/web/tool.sh evals/session-history/tools/ingest.coax --source antigravity --limit 20

# Write to a custom output directory
coax/web/tool.sh evals/session-history/tools/ingest.coax --source claude --limit 5 --out /tmp/sh-evals

# Discover and print inventory without writing .atif.jsonl files
coax/web/tool.sh evals/session-history/tools/ingest.coax --dry-run
```

### Flags

| Flag | Default | Description |
| --- | --- | --- |
| `--source NAME` | `all` | One of `all`, `claude`, `codex`, `openagents`, `antigravity`, `opencode`. |
| `--limit N` | `0` (all) | Maximum sessions to ingest per source (newest first by mtime). |
| `--out DIR` | `~/.openagents/evals/session-history` | Output root for `atif/`, `index.jsonl`, and `inventory.json`. |
| `--home DIR` | `$HOME` | Override home directory used to locate local harness histories. |
| `--dry-run` | off | Walk source roots and emit `inventory.json` without writing `.atif.jsonl` or manifests. |

### Source Filtering Rules

- **Claude Code** (`~/.claude/projects/<project>/<sessionId>.jsonl`): keeps `user`, `assistant`, `system`, titles, `cwd`, `gitBranch`, `model`, and token usage. Drops `api-request-*`, `file-history-*`, `queue-operation`, `atis-latch`, `progress`, and replaces inline base64 images with `[image:sha256]`.
- **Codex** (`~/.codex/sessions/YYYY/MM/DD/*.jsonl`): maps `session_meta`, `turn_context`, `response_item` (`message`, `custom_tool_call`, `custom_tool_call_output`, `function_call`, `function_call_output`, `reasoning`), and `event_msg` (`token_count`).
- **OpenAgents Desktop** (`~/.openagents/coder/sessions/*.ajr`, `*.jsonl`): decodes `AJR1` binary journal frames via `coax/lib/agent/journal.coax`.
- **Antigravity** (`~/.gemini/antigravity/brain/<id>/.system_generated/logs/transcript.jsonl` and `~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript.jsonl`): reuses `coax/lib/agent/observe/antigravity.coax` (`USER_INPUT` → `user`, `PLANNER_RESPONSE` → `agent`, `GENERIC` → `observation`).
- **OpenCode** (`~/.local/share/opencode/`): inspects local session storage when present.

## Usage (Phase 2 — Mine, Sanitize, Run & Compare)

```sh
# Mine candidate episodes from local ATIF-v1.8 logs and anchor to git SHAs
evals/session-history/run.sh mine --repo . --out /tmp/sh-cands --limit 10

# Sanitize a candidate task and run the hermeticity gate (base_sha fails, target_sha passes)
evals/session-history/run.sh sanitize \
  --in /tmp/sh-cands/sh_001.json \
  --out evals/session-history/tasks/sh_001.json \
  --verify --require-base-fail --repo .

# Run the hermeticity gate across the entire tasks/ directory
evals/session-history/run.sh sanitize --verify --require-base-fail --repo .

# Run committed tasks via evals/openagents-desktop (host_driver + mcworker) and emit scorecard
evals/session-history/run.sh run --model gemini-4-hydrogen --workers 4

# Re-compute comparison scorecard from an existing run directory
evals/session-history/run.sh compare --tasks evals/session-history/tasks --run evals/session-history/results/<run_id>
```

## Phase 3 Stratified Suite (`tasks/`, 27 Tasks)

All 27 tasks are anchored to real `work/openagents` trajectories in `~/.openagents/evals/session-history/atif/` with exact ATIF-v1.8 provenance (`trajectory_id`, `step_range`, `steps_digest`, `base_sha`, `target_sha`) and pass `./evals/session-history/run.sh sanitize --verify --require-base-fail --repo .` (27/27 fail on `base_sha`, 27/27 pass on `target_sha`, zero secrets/PII or `/Users/...` paths).

### Suite Breakdown

| Bucket | Count | Sources |
| --- | --- | --- |
| `A_edit_precision` | 6 | 3 claude, 2 openagents, 1 antigravity |
| `B_multi_file` | 5 | 2 claude, 1 codex, 1 antigravity, 1 openagents |
| `C_verification` | 6 | 3 claude, 2 antigravity, 1 openagents |
| `D_error_recovery` | 5 | 5 claude |
| `E_long_context` | 5 | 1 claude, 2 codex, 1 openagents, 1 antigravity |
| **Total** | **27** | **14 claude, 5 openagents, 5 antigravity, 3 codex** |

### Phase 3 Multi-Model Scorecard (Baseline vs `mcworker`)

Run via `./evals/session-history/run.sh run --model <model> --workers 4` against `evals/openagents-desktop` `host_driver` + `mcworker` (using each task's `$0.12–$0.25` budget cap):

| Model | Overall Pass | Command Grader Pass | Budget-Stopped | `git push` Guard Trips | Total Cost | Total Wall | Top Failure Tags |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Observed Baseline (`claude-opus-5-5`, `gemini-4-neon`, `Gemini 4 Neon/Carbon`, `codex`) | 27/27 (100%) | 27/27 (100%) | - | - | - | 4367s | - |
| `gemini-4-hydrogen` | 7/27 (25%) | 14/27 (52%) | 15 | 6 | \$4.8971 | 1218s | `premature_done` (17), `edit_mismatch` (2), `compaction_drop` (1) |
| `gemini-4-neon` | 5/27 (18%) | 10/27 (37%) | 20 | 3 | \$6.3495 | 1361s | `premature_done` (19), `edit_mismatch` (2), `compaction_drop` (1) |
| `gemini-4-carbon` | 0/27 (0%) | 4/27 (15%) | 27 | 0 | \$7.2356 | 1026s | `premature_done` (23), `edit_mismatch` (2), `compaction_drop` (2) |

Key Phase 4 `mcworker` levers identified from the scorecard:
1. **Context/cost efficiency:** Per-attempt cost (`~$0.07–$0.12` on Carbon, `~$0.04–$0.08` on Neon) exhausts `$0.12–$0.25` per-task budgets in 2–4 attempts before completion (`budget_stopped` on 15/27 Hydrogen, 20/27 Neon, 27/27 Carbon).
2. **Unsolicited `git push` suppression:** Auto-loaded `AGENTS.md` triggers `git push` attempts after solving the task (6/27 Hydrogen, 3/27 Neon), tripping `no_guard_trips` despite passing the `command` grader.
3. **Coax syntax & edit discipline:** Repeated `E0216` (`..` vs `..<`) and `E0105` escape errors burn attempts before compiling.

### Phase 4 `mcworker` Improvements & Before/After Scorecard

Principled, general-purpose `mcworker` (`coax/lib/microcoder/`) improvements shipped in Phase 4:
1. **Lean workspace instruction cap (`instructions.coax`):** Tightened `MC_INSTR_MAX_BYTES` from `12288` to `4096` (~2,400 input tokens saved per attempt; truncated files still emit the full `##` heading index with line numbers for on-demand ranged views).
2. **Bounded step-0 seeding (`run.coax`):** Capped `mc_run_seed` to `MC_SEED_FILES = 3` and `MC_SEED_BUDGET = 12288` bytes (down from `32768`) so multi-file tasks don't dump 32 KB into Attempt 1 before the model searches.
3. **Tighter default view and command-output budgets (`tools.coax`):** Reduced unranged `view` default from `300` to `150` lines (`MC_VIEW_SHOW` `24576` → `12288`), spilled command preview (`MC_SPILL_SHOW` `8192` → `4096`), and kept command output head/tail (`MC_KEEP_HEAD/TAIL` `5900` → `3000`; full outputs remain in spill files and ranged `PATH:A-B` views still support up to 800 lines).
4. **Git push safety, worktree discipline & verbatim-edit guidance (`loop.coax`):** Made `MC_SYSTEM` explicit to never run `git push` unless the user's own message explicitly asks to push (even if workspace or default instructions mention pushing), work directly in `environment.cwd` without `git worktree list` / `git branch -a` churn, and copy `old` verbatim from a view without line numbers.

Re-run on the same 27-task suite (`./evals/session-history/run.sh run --model <model> --workers 4`, identical task budgets and graders):

| Model | Phase | Overall Pass | Command Grader Pass | Budget-Stopped | `git push` Guard Trips | Total Cost | Total Wall | Top Failure Tags |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `gemini-4-hydrogen` | Phase 3 (before) | 7/27 (25%) | 14/27 (52%) | 15 | 6 | \$4.8971 | 1218s | `premature_done` (17), `edit_mismatch` (2), `compaction_drop` (1) |
| `gemini-4-hydrogen` | **Phase 4 (after)** | **14/27 (51%)** | **16/27 (59%)** | **10** | **0** | **\$4.3282** | 1361s | `premature_done` (12), `compaction_drop` (1), `edit_mismatch` (0) |
| `gemini-4-neon` | Phase 3 (before) | 5/27 (18%) | 10/27 (37%) | 20 | 3 | \$6.3495 | 1361s | `premature_done` (19), `edit_mismatch` (2), `compaction_drop` (1) |
| `gemini-4-neon` | **Phase 4 (after)** | **8/27 (29%)** | 9/27 (33%) | **19** | **0** | **\$6.2481** | **1220s** | `premature_done` (19), `edit_mismatch` (0), `compaction_drop` (0) |
| `gemini-4-neon` | **Phase 5 Lab** ([log](../../docs/verification/microcoder-neon-inkling-lab.md)) | **10/27 (37%)** | **13/27 (48%)** | **17** | **0** | **\$5.0641** | 1624s | `premature_done` (14), `search_bloat` (3), `edit_mismatch` (0), `compaction_drop` (0) |
| `gemini-4-carbon` | Phase 3 (before) | 0/27 (0%) | 4/27 (15%) | 27 | 0 | \$7.2356 | 1026s | `premature_done` (23), `edit_mismatch` (2), `compaction_drop` (2) |
| `gemini-4-carbon` | **Phase 4 (after)** | 0/27 (0%) | 4/27 (15%) | 27 | **0** | **\$6.7253** | **1021s** | `premature_done` (22), `compaction_drop` (3), `edit_mismatch` (1), `search_bloat` (1) |

Bucket highlights (Phase 3 → Phase 4):
- **Hydrogen:** `A_edit_precision` 5/6 → **6/6 (100%)**, `B_multi_file` 0/5 → **3/5 (60%)**, `C_verification` 0/6 → **1/6 (16%)**, `D_error_recovery` 1/5 → **3/5 (60%)**, `E_long_context` 1/5 → 1/5 (20%).
- **Neon:** `A_edit_precision` 4/6 → **6/6 (100%)**, `D_error_recovery` 1/5 → **2/5 (40%)**.
- **Remaining Carbon bottleneck:** Vertex `us` multi-region returns `cachedContentTokenCount: null` (0% implicit cache hit), so 3–4 attempts on `$4/M`-input Carbon still reach `$0.12–$0.15` caps right as tasks finish (4/27 pass `command` grader before hitting budget). Explicit Vertex context caching or multi-turn cache hits are the next lever for Carbon.

## Verification

```sh
coax/bin/coax test coax/lib/nostr/atif/digest.coax
coax/bin/coax test coax/lib/nostr/atif/log.coax
coax/bin/coax test evals/session-history/tools/ingest.coax
coax/bin/coax test evals/session-history/tools/mine.coax
coax/bin/coax test evals/session-history/tools/sanitize.coax
coax/bin/coax test evals/session-history/tools/compare.coax
coax/web/tool.sh tools-check
```
