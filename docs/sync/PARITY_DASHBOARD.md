# Khala Dual-Publish Parity Dashboard

This dashboard is the runtime parity control-plane for KHALA-021.

Goal: track Convex-vs-Khala parity drift during dual-publish windows and block cutover if mismatch trends are unstable.

## Metric source

Runtime emits parity telemetry from `OpenAgentsRuntime.Sync.ParityAuditor`:

- `openagents_runtime.sync.parity.cycle.count`
- `openagents_runtime.sync.parity.cycle.sampled`
- `openagents_runtime.sync.parity.cycle.mismatches`
- `openagents_runtime.sync.parity.cycle.mismatch_rate`
- `openagents_runtime.sync.parity.cycle.max_abs_lag_drift`
- `openagents_runtime.sync.parity.cycle.avg_abs_lag_drift`
- `openagents_runtime.sync.parity.entity.count`
- `openagents_runtime.sync.parity.entity.abs_lag_drift`
- `openagents_runtime.parity.failure.count` (with `class="sync_dual_publish"`)

## Required panels

1. Mismatch rate (5m and 1h):
`openagents_runtime.sync.parity.cycle.mismatch_rate{status=~"ok|mismatch|empty"}`

2. Mismatch count:
`sum(rate(openagents_runtime.sync.parity.cycle.mismatches[5m]))`

3. Parity failures by reason:
`sum by (reason_class) (rate(openagents_runtime.parity.failure.count{class="sync_dual_publish",component="sync_parity_auditor"}[5m]))`

4. Lag drift p95:
`histogram_quantile(0.95, sum(rate(openagents_runtime.sync.parity.entity.abs_lag_drift_bucket[5m])) by (le))`

5. Missing-doc mismatches:
`sum(rate(openagents_runtime.sync.parity.entity.count{status="mismatch",reason_class="khala_missing"}[5m]))`

## Gate thresholds

- `mismatch_rate` must remain `< 0.01` over 1h before broadening rollout.
- `khala_missing` mismatch reason should trend to `0`.
- `abs_lag_drift` p95 should remain at `0` in steady state.

## Drill outputs

For cutover drills, capture:

- dashboard screenshots at start/mid/end of drill,
- mismatch reason breakdown export (CSV/JSON),
- link to runtime logs for parity failures with `component=sync_parity_auditor`.
