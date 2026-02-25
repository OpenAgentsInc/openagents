# Canary Rollback Drill Automation

Automated canary/rollback drill runner:
- `apps/openagents.com/deploy/run-canary-rollback-drill.sh`

This wraps:
- `apps/openagents.com/deploy/canary-rollout.sh`

and emits structured drill artifacts.

## Inputs

Required positional args:
- `<stable-revision>`
- `<canary-revision>`

Environment:
- `PROJECT` (optional; defaults to active gcloud project)
- `REGION` (default: `us-central1`)
- `SERVICE` (default: `openagents-control-service`)
- `DRY_RUN` (`1` for non-destructive rehearsal)
- `CANARY_STEPS` (comma-separated percentages, default `5,25,50,100`)
- `OUTPUT_DIR` (optional report output override)

## Run

Dry-run rehearsal:

```bash
DRY_RUN=1 \
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service \
apps/openagents.com/deploy/run-canary-rollback-drill.sh \
  stable-revision canary-revision
```

Live drill:

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service \
apps/openagents.com/deploy/run-canary-rollback-drill.sh \
  stable-revision canary-revision
```

## Output

Default output directory:
- `apps/openagents.com/deploy/reports/canary-drill-<timestamp>/`

Artifacts:
- `summary.json`
- `SUMMARY.md`
- step-level logs

The script exits non-zero on any failed drill step.
