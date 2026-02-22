# OA-WEBPARITY-080 One-Shot Production Stream Cutover + Rollback Gates

Date: 2026-02-22  
Status: pass (automation + dry-run rehearsal; live production execution requires operator credentials)  
Issue: OA-WEBPARITY-080

## Deliverables

- Cutover runner: `apps/openagents.com/service/scripts/run-production-stream-cutover.sh`
- Stream contract smoke runner: `apps/openagents.com/service/scripts/run-production-stream-contract-smoke.sh`
- Cutover runbook: `apps/openagents.com/service/docs/PRODUCTION_STREAM_CUTOVER.md`
- Workflow automation removed (invariant: no `.github/workflows`).

## Hard Gates Added

The one-shot runner now enforces explicit thresholds for:
1. route-flip failures
2. stream-contract smoke failures
3. canary rollback drill failures
4. dual-run mismatches (optional gate)
5. observed error-budget consumption

The run fails if any gate exceeds its configured maximum.

## Dry-Run Rehearsal (Executed)

Command:

```bash
DRY_RUN=1 \
BASE_URL=https://openagents.com \
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service \
SLO_ERROR_BUDGET_CONSUMED_PERCENT=0 \
apps/openagents.com/service/scripts/run-production-stream-cutover.sh \
  stable-revision canary-revision
```

Result artifact:
- `apps/openagents.com/storage/app/production-stream-cutover/20260222T183255Z/summary.json`
- `apps/openagents.com/storage/app/production-stream-cutover/20260222T183255Z/SUMMARY.md`

Dry-run summary:
- overall_status: `passed`
- step_count: `6`
- passed: `6`
- failed: `0`

## Live Execution Command (Ready)

```bash
DRY_RUN=0 \
APPLY_ROUTE_FLIP=1 \
BASE_URL=https://openagents.com \
CONTROL_ACCESS_TOKEN=<admin-token> \
AUTH_TOKEN=<authenticated-user-token> \
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service \
SLO_ERROR_BUDGET_CONSUMED_PERCENT=<observed-percent> \
apps/openagents.com/service/scripts/run-production-stream-cutover.sh \
  <stable-revision> <canary-revision>
```

## Notes

- Route flip remains a first-class part of the same release train.
- Rollback drill is explicitly executed in the same train.
- Cutover artifacts include timestamps, revision inputs, gate outcomes, and per-step logs.
