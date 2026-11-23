---
name: transcript-hillclimb
description: >-
  Use when auditing Antigravity and OpenAgents chat transcripts to measure
  rework rate, empty replies, verify/ship nudges, tool failures, polling waste,
  or daemon/journal hygiene and track improvements over time.
---

# Transcript Friction Hillclimb Workflow

## 1. Run the Live Friction Scorecard
From the repository root (read-only over local private logs in `~/.gemini/antigravity/brain/`, `~/Library/Application Support/Coder/`, and `~/.openagents/coder/sessions/`):

```sh
evals/session-history/run.sh friction
```

Optional flags:
- `--limit N` — analyze the `N` newest sessions per source (default `0` = all).
- `--json` — emit machine-readable JSON for diffing against previous baselines.
- `--out PATH` — save Markdown scorecard to a local scratch/private file (never commit private transcripts to git).

## 2. Refresh ATIF Evals Index (When Mining New Benchmark Tasks)
```sh
evals/session-history/run.sh ingest --limit 40
evals/session-history/run.sh mine
```

## 3. Hillclimb Loop
1. Compare current KPIs against the baseline table in [docs/audits/2026-11-22-transcript-friction-hillclimb.md](../../docs/audits/2026-11-22-transcript-friction-hillclimb.md).
2. Inspect the top 3 concrete transcripts driving the worst remaining KPI (never optimize on counts without reading the raw steps/turns).
3. Fix the root cause at the lowest durable layer (harness/runtime bug > git hook/build script > glob-scoped rule/skill > doc).
4. Re-run `evals/session-history/run.sh friction` after a week of real sessions to confirm the metric actually moved.
