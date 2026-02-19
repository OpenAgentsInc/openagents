# OpenClaw Drift Report Process

Date: 2026-02-19
Status: Active with strict actionable gating

## Purpose

Track capability drift between pinned OpenClaw SHAs in intake/parity artifacts and current upstream HEAD, with explicit next actions for re-ingestion.

## Operational context after parity wave

This process became mandatory during the `#1740` to `#1746` parity wave because parity work moved from design intent to production runtime behavior. Once policy, loop detection, hooks, network guards, manifests, workflows, and telemetry were all implemented as OpenClaw-aligned runtime contracts, an untracked upstream drift stopped being a documentation nuisance and became a correctness risk. For that reason the drift script and local CI gating were hardened so they now produce actionable summaries and can fail local gates when unresolved drift rows remain. The goal is to keep parity honest by forcing each mismatch to have either a deliberate pin decision or a re-ingestion issue that explains how behavior will be reconciled.

When this process is used correctly, engineers do not wait for outages or parity regressions to discover that upstream semantics changed. They run the report, identify whether a capability is in sync or actionable, and then attach the follow-up decision to the intake chain before the next release cycle. That creates an auditable trail from upstream change to local fixture update, and it preserves confidence that runtime behavior still matches the parity contract we claim in the roadmap.

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

Strict local gate (fail when actionable rows exist):

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

- Local pre-push gate via `scripts/local-ci.sh all`
- On-demand via `scripts/openclaw-drift-report.sh`
- Optional operator cron job in trusted infrastructure
- On changes to intake records, parity fixtures, or drift script

## Response Policy

For each `upstream_head_mismatch` or `missing_pin` row:
1. Open or update a dedicated ingestion issue.
2. Summarize upstream diff scope.
3. Refresh/extend parity fixtures and rerun harnesses.
4. Confirm port/adapt/adopt decision and rollout risk.

Enforcement:
- Local CI runs drift generation with `OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE=1`.
- Any actionable row must be accompanied by an issue in-flight or an explicit pin refresh.

## How to use this process day to day

The practical daily workflow is to run the drift command from repository root after any intake, fixture, or parity module change, then immediately inspect the generated report before merging. If the report is clean, the change can proceed without extra process overhead because provenance is intact. If the report contains actionable rows, the merge should include either a pin correction or an issue link that records the planned reconciliation path. This keeps the drift process lightweight for stable periods and strict when upstream movement matters.

The immediate next step for the current repository state is to resolve the actionable rows already identified in `docs/plans/active/openclaw-drift-report.md`, especially any `missing_pin` entry, because pending pins weaken attribution and make future behavior diffs harder to reason about. Once those are resolved, maintaining this process is mostly routine CI hygiene rather than heavy project management.
