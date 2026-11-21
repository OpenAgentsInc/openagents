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

# Run committed tasks via evals/openagents-desktop (host_driver + mcworker) and emit scorecard
evals/session-history/run.sh run

# Re-compute comparison scorecard from an existing run directory
evals/session-history/run.sh compare --tasks evals/session-history/tasks --run evals/session-history/results/<run_id>
```

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
