# Khala Dual-Path Parity Dashboard

This dashboard is the runtime cutover gate for KHALA-021.

Goal: track parity drift between the legacy reactive lane and the Khala lane during dual-path operation and block rollout when mismatch trends are unstable.

## Metric Source

Runtime emits telemetry from `OpenAgentsRuntime.Sync.ParityAuditor`:

- `openagents_runtime.sync.parity.cycle.count`
- `openagents_runtime.sync.parity.cycle.sampled`
- `openagents_runtime.sync.parity.cycle.mismatches`
- `openagents_runtime.sync.parity.cycle.mismatch_rate`
- `openagents_runtime.sync.parity.cycle.max_abs_lag_drift`
- `openagents_runtime.sync.parity.cycle.avg_abs_lag_drift`
- `openagents_runtime.sync.parity.entity.count`
- `openagents_runtime.sync.parity.entity.abs_lag_drift`
- `openagents_runtime.parity.failure.count` with `class="sync_dual_publish"`

## Required Panels

1. Mismatch rate (5m and 1h)
`openagents_runtime.sync.parity.cycle.mismatch_rate{status=~"ok|mismatch|empty"}`

2. Mismatch count
`sum(rate(openagents_runtime.sync.parity.cycle.mismatches[5m]))`

3. Parity failures by reason class
`sum by (reason_class) (rate(openagents_runtime.parity.failure.count{class="sync_dual_publish",component="sync_parity_auditor"}[5m]))`

4. Lag drift p95
`histogram_quantile(0.95, sum(rate(openagents_runtime.sync.parity.entity.abs_lag_drift_bucket[5m])) by (le))`

5. Missing-document mismatches
`sum(rate(openagents_runtime.sync.parity.entity.count{status="mismatch",reason_class="khala_missing"}[5m]))`

## Gate Thresholds

- `mismatch_rate` must remain `< 0.01` over 1h before broadening rollout.
- `khala_missing` mismatch reason should trend to `0`.
- `abs_lag_drift` p95 should remain at or near `0` in steady state.

## Drill Outputs

For cutover drills, capture:

- dashboard screenshots at start/mid/end of drill,
- mismatch reason breakdown export (CSV/JSON),
- runtime log links for `component=sync_parity_auditor` failures.
