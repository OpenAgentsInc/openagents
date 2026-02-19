# Convex G7 Backup/Restore/Replay + Rollback Drill Report

Date: 2026-02-19  
Scope: Gate G7 (`#1766`) production-readiness drills in non-prod (`openagentsgemini/us-central1`)

## Drill Objectives

1. Verify backup/restore path on self-hosted Convex in a production-like environment.
2. Verify runtime projection replay path from runtime durable history.
3. Verify rollback path and measure RTO against the target.
4. Confirm staged rollout and on-call escalation docs are current.

## RTO Target

- Rollback target RTO: <= 5 minutes (300 seconds).

## Commands Executed

1. Baseline health
- `apps/openagents-runtime/deploy/convex/check-nonprod-health.sh`

2. Backup/restore drill
- `apps/openagents-runtime/deploy/convex/run-backup-restore-drill.sh`

3. Replay drill (runtime local with deterministic fixtures)
- `apps/openagents-runtime/deploy/convex/run-runtime-replay-drill.sh`

4. Rollback drill
- `apps/openagents-runtime/deploy/convex/run-rollback-drill.sh` (dry-run)
- `OA_CONVEX_ROLLBACK_DRILL_APPLY=1 apps/openagents-runtime/deploy/convex/run-rollback-drill.sh`

## Outcomes

Backup/restore:

- Export snapshot created: `/tmp/convex-nonprod-export-20260219-153949.zip`
- Import completed with no destructive changes (`Added 0 documents`).
- Drill duration: 4 seconds.

Replay:

- Seeded run id: `g7_drill_run_8130`
- Seeded worker id: `g7_drill_worker_8130`
- Reproject run result:
  - `scope=run entity=g7_drill_run_8130 result=ok write=applied duration_ms=63`
- Reproject worker result:
  - `scope=codex_worker entity=g7_drill_worker_8130 result=ok write=applied duration_ms=70`

Rollback:

- Current revisions at drill start:
  - backend: `oa-convex-backend-nonprod-00011-v4r`
  - dashboard: `oa-convex-dashboard-nonprod-00002-hwm`
- Rollback target revisions:
  - backend: `oa-convex-backend-nonprod-00010-kpf`
  - dashboard: `oa-convex-dashboard-nonprod-00001-8rs`
- Measured rollback RTO: 20 seconds.
- Measured restore duration: 18 seconds.
- Total rollback drill time: 38 seconds.
- Post-drill traffic restored to 100% on original revisions.

## Acceptance Criteria Status

1. Backup/restore and replay drill succeeds in production-like env: PASS.
2. Rollback drill completes within agreed RTO: PASS (20s <= 300s).
3. On-call docs and escalation paths are up to date: PASS.

## Documentation Updates Landed

- `apps/openagents-runtime/deploy/convex/OPERATIONS_RUNBOOK.md`
- `apps/openagents-runtime/deploy/convex/README.md`
- `docs/plans/active/convex-runtime-codex-master-roadmap.md`

## Remaining Work

- Execute staged cohort rollout in production with the same drill gates before each expansion step.
