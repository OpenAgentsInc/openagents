# Runtime Monitoring Assets

This directory contains baseline dashboards and alert rules for `runtime`.

## Artifacts

- Grafana dashboard: `grafana/runtime-ops-dashboard.json`
- Grafana dashboard: `grafana/spacetime-slo-dashboard.json`
- Prometheus rules: `prometheus/runtime-alert-rules.yaml`
- Prometheus rules: `prometheus/spacetime-slo-alert-rules.yaml`

## Coverage

- Executor p95 latency
- Runtime 5xx ratio
- Stream completion ratio (`run.finished`/stream sessions)
- Lease steal rate
- Tool terminal failure spikes
- Provider circuit breaker open state
- Spend/policy denial anomalies
- Spacetime projection writes throughput
- Spacetime projection lag p95 (`lag_events`)
- Spacetime projection write failure ratio
- Spacetime projection drift incidents
- Spacetime projection replay failure visibility
- Spacetime token mint failure ratio (Laravel bridge)
- Spacetime websocket auth failure ratio by reason code
- Spacetime replay lag/stale-cursor SLO budget alerts
- Client reconnect/auth-failure telemetry segmentation by surface/app version

## Runbook

- Operations + thresholds: `apps/runtime/docs/OPERATIONS_ALERTING.md`
- End-to-end correlation walkthrough: `apps/runtime/docs/OPERATIONS_ALERTING.md#end-to-end-request-correlation-walkthrough`
- Telemetry contract and label guardrails: `apps/runtime/docs/OBSERVABILITY.md`
- Client telemetry schema and privacy guidance: `docs/protocol/client-telemetry-v1.md`

## Notes

- `openagents_runtime_provider_breaker_state` alert/panel is intentionally pre-wired for issue `#1686`; expression includes `or vector(0)` so it is safe before breaker metrics are emitted.
- Alert rules assume Prometheus metric naming with dots converted to underscores.
