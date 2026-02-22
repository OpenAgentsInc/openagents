# Production Stream Cutover (One-Shot + Rollback + Hard Gates)

Tracking: `OA-WEBPARITY-080`

Automation scripts:
- `apps/openagents.com/service/scripts/run-production-stream-cutover.sh`
- `apps/openagents.com/service/scripts/run-production-stream-contract-smoke.sh`

## Purpose

Execute a one-shot production cutover for the compatibility stream lane with:
1. pre/post stream contract smokes,
2. explicit rollback drill in the same release train,
3. hard gate thresholds before marking the cutover window successful.

## Inputs

Positional:
- `<stable-revision>`
- `<canary-revision>`

Environment:
- `BASE_URL` (default: `https://openagents.com`)
- `CONTROL_ACCESS_TOKEN` (required when `DRY_RUN=0`)
- `AUTH_TOKEN` (required when `DRY_RUN=0`)
- `APPLY_ROUTE_FLIP` (`0` verify-only, `1` apply route flip)
- `DRY_RUN` (`1` default, non-destructive rehearsal)
- `PROJECT`/`REGION`/`SERVICE` (Cloud Run drill targets)
- `RUN_DUAL_RUN` (`1` to require rust-vs-legacy diff)
- `LEGACY_BASE_URL` (required when `RUN_DUAL_RUN=1`)

Hard-gate thresholds:
- `MAX_ROUTE_FLIP_FAILURES` (default `0`)
- `MAX_STREAM_SMOKE_FAILURES` (default `0`)
- `MAX_CANARY_DRILL_FAILURES` (default `0`)
- `MAX_DUAL_RUN_FAILURES` (default `0`)
- `MAX_ERROR_BUDGET_CONSUMED_PERCENT` (default `5`)
- `SLO_ERROR_BUDGET_CONSUMED_PERCENT` (operator-provided observed value; default `0`)

## Dry-Run Rehearsal

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

## Live Production Execution

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

## What It Enforces

1. Route-flip verification/apply path is included in the cutover train.
2. Stream alias contracts are checked pre and post cutover (`/api/chat/stream`, `/api/chats/{id}/stream`):
   - `200` response
   - `content-type: text/event-stream; charset=utf-8`
   - `x-vercel-ai-ui-message-stream: v1`
   - `start`, `start-step`, `finish-step`, `finish`, single `[DONE]`
3. Canary rollback drill executes in the same train.
4. Hard gates fail the run if any threshold is exceeded.

## Artifacts

All outputs are emitted under:
- `apps/openagents.com/storage/app/production-stream-cutover/<timestamp>/`

Key files:
- `summary.json`
- `SUMMARY.md`
- per-step logs and delegated sub-run artifacts
