# OA-WEBPARITY-058 Production Canary + Rollback Drill Report

Date: 2026-02-22
Status: pass (automation + dry-run rehearsal), live production execution pending operator approval
Issue: OA-WEBPARITY-058

## Deliverables

Automated canary/rollback drill runner:
- `apps/openagents.com/service/deploy/run-canary-rollback-drill.sh`

Automation runbook:
- `apps/openagents.com/service/docs/CANARY_ROLLBACK_DRILL_AUTOMATION.md`

Manual workflow dispatch:
- `.github/workflows/web-production-canary-rollback-drill.yml`

Local CI lane:
- `scripts/local-ci.sh canary-drill`
  - requires `OA_CANARY_STABLE_REVISION` and `OA_CANARY_CANARY_REVISION`

## Dry-Run Rehearsal (Executed)

Command:

```bash
DRY_RUN=1 \
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service \
apps/openagents.com/service/deploy/run-canary-rollback-drill.sh \
  stable-revision canary-revision
```

Result artifact:
- `apps/openagents.com/service/deploy/reports/canary-drill-20260222T155858Z/summary.json`
- `apps/openagents.com/service/deploy/reports/canary-drill-20260222T155858Z/SUMMARY.md`

Dry-run summary:
- overall_status: `passed`
- step_count: `7`
- passed: `7`
- failed: `0`

## Drill Sequence Enforced

1. Capture pre-drill traffic status
2. Set canary traffic: `5%`
3. Set canary traffic: `25%`
4. Set canary traffic: `50%`
5. Set canary traffic: `100%`
6. Rollback to stable revision `100%`
7. Capture post-drill traffic status

## Live Production Execution Command (Ready)

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service \
apps/openagents.com/service/deploy/run-canary-rollback-drill.sh \
  <stable-revision> <canary-revision>
```

## Notes

- Live production drill requires operator credentials and explicit approval.
- Dry-run rehearsal validates command sequencing, report generation, and rollback path wiring without mutating Cloud Run traffic.
