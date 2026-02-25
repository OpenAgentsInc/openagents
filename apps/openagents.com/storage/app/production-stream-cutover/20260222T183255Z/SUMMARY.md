# Production Stream Cutover

- Generated at: 20260222T182857Z
- Base URL: https://openagents.com
- Dry run: 1
- Apply route flip: 0
- Stable revision: stable-revision
- Canary revision: canary-revision
- Overall status: passed
- Totals: 6 pass / 0 fail
- Gate metrics:
  - route_flip_failed: 0 (max 0)
  - stream_smoke_failed: 0 (max 0)
  - canary_failed: 0 (max 0)
  - dual_run_failed: 0 (max 0)
  - error_budget_consumed: 0% (max 5%)

| Step | Status | Detail | Artifact |
| --- | --- | --- | --- |
| route-flip | pass | dry-run rehearsal: route flip not applied |  |
| stream-smoke-pre | pass | command succeeded | `/Users/christopherdavid/code/openagents/apps/openagents.com/storage/app/production-stream-cutover/20260222T183255Z/stream-smoke-pre/summary.json` |
| stream-smoke-post | pass | command succeeded | `/Users/christopherdavid/code/openagents/apps/openagents.com/storage/app/production-stream-cutover/20260222T183255Z/stream-smoke-post/summary.json` |
| canary-rollback-drill | pass | command succeeded | `/Users/christopherdavid/code/openagents/apps/openagents.com/storage/app/production-stream-cutover/20260222T183255Z/canary-rollback-drill/summary.json` |
| dual-run-diff | pass | dual-run gate skipped (RUN_DUAL_RUN=0) |  |
| slo-error-budget-gate | pass | within budget (0% <= 5%) |  |
