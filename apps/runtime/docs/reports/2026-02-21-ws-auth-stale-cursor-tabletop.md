# WS/Auth/Stale-Cursor Tabletop Rehearsal Report

Date: 2026-02-21
Scope: OA-RUST-071 incident runbook usability rehearsal

## Rehearsal Inputs

- Incident runbook: `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`
- Alert rules: `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
- Dashboard: `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`

## Tabletop Scenarios and Outcomes

1. WS auth failure ratio spike (`OpenAgentsRuntimeSyncSocketAuthFailureRatioHigh`)
- Triage path walked end-to-end from alert -> reject reason class -> key parity checks.
- Resolution path validated: key/config parity correction + targeted auth drill tests.
- Outcome: PASS (operator steps were explicit and executable).

2. Reconnect timeout storm (`OpenAgentsRuntimeSyncSocketTimeoutRateHigh`)
- Triage path validated: connection/reconnect churn inspection + heartbeat sanity + rollback gate.
- Resolution path validated: reconnect drill command and escalation trigger criteria.
- Outcome: PASS (runbook gave deterministic next actions).

3. Stale cursor spike (`OpenAgentsRuntimeSyncStaleCursorSpike`)
- Triage path validated: retention-floor checks + stale topic distribution + client resnapshot expectations.
- Resolution path validated: stale-cursor drill command and exit criteria.
- Outcome: PASS (clear mitigation and escalation boundaries).

## Verification Evidence

Alert and dashboard wiring checks:

- `rg --line-number "OpenAgentsRuntimeSyncSocketAuthFailureRatioHigh|OpenAgentsRuntimeSyncSocketTimeoutRateHigh|OpenAgentsRuntimeSyncStaleCursorSpike" apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
  - matched lines: 144, 155, 166
- `jq -r '.panels[] | select(.title|test("Sync Socket Auth Failure Ratio|Sync Socket Timeout Rate|Sync Stale Cursor Incidents")) | "\(.id): \(.title)"' apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`
  - panel IDs: 15, 16, 17

Runtime validation tests:

- `cd apps/runtime && mix test test/openagents_runtime_web/channels/sync_channel_test.exs test/openagents_runtime/telemetry/metrics_test.exs test/openagents_runtime/ops/monitoring_assets_test.exs`
  - result: 20 tests, 0 failures

## Follow-Up

1. Run this tabletop again after any sync auth protocol change.
2. Keep runbook links in alert annotations as part of monitoring asset CI checks.
