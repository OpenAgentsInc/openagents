# OpenClaw Drift Report Process

Date: 2026-02-19
Status: Active

## Purpose

Track capability drift between pinned OpenClaw SHAs in intake/parity artifacts and current upstream HEAD, with explicit next actions for re-ingestion.

## Inputs

- Intake records: `docs/plans/active/openclaw-intake/*.md`
- Parity fixtures: `apps/openagents-runtime/test/fixtures/openclaw/*.json`
- Upstream repository: `https://github.com/openclaw/openclaw.git` (overridable)

## Command

```bash
scripts/openclaw-drift-report.sh
```

Optional override:

```bash
OPENCLAW_UPSTREAM_URL=https://github.com/openclaw/openclaw.git scripts/openclaw-drift-report.sh
```

Strict CI gate (fail when actionable rows exist):

```bash
OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE=1 scripts/openclaw-drift-report.sh
```

## Output

- `docs/plans/active/openclaw-drift-report.md`

Report rows include:
- capability id
- pinned SHA
- upstream HEAD
- drift type (`in_sync`, `upstream_head_mismatch`, `missing_pin`, `invalid_sha`)
- recommended action

Report summary includes:
- drift counts by class
- actionable row count
- issue command templates for each actionable capability

## Cadence

- Weekly scheduled CI run (`.github/workflows/openclaw-drift-report.yml`)
- On-demand via workflow dispatch
- On changes to intake records, parity fixtures, or drift script

## Response Policy

For each `upstream_head_mismatch` or `missing_pin` row:
1. Open or update a dedicated ingestion issue.
2. Summarize upstream diff scope.
3. Refresh/extend parity fixtures and rerun harnesses.
4. Confirm port/adapt/adopt decision and rollout risk.

Enforcement:
- CI workflow runs drift generation with `OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE=1`.
- Any actionable row must be accompanied by an issue in-flight or an explicit pin refresh.
