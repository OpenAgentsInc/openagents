# Runtime Monitoring Assets

This directory contains baseline dashboards and alert rules for `openagents-runtime`.

## Artifacts

- Grafana dashboard: `grafana/openagents-runtime-ops-dashboard.json`
- Prometheus rules: `prometheus/openagents-runtime-alert-rules.yaml`

## Coverage

- Executor p95 latency
- Runtime 5xx ratio
- Stream completion ratio (`run.finished`/stream sessions)
- Lease steal rate
- Tool terminal failure spikes
- Provider circuit breaker open state
- Spend/policy denial anomalies
- Convex projection writes throughput
- Convex projection lag p95 (`lag_events`)
- Convex projection write failure ratio
- Convex projection drift incidents
- Convex projection replay failure visibility
- Convex token mint failure ratio (Laravel bridge)

## Runbook

- Operations + thresholds: `apps/openagents-runtime/docs/OPERATIONS_ALERTING.md`
- End-to-end correlation walkthrough: `apps/openagents-runtime/docs/OPERATIONS_ALERTING.md#end-to-end-request-correlation-walkthrough`
- Telemetry contract and label guardrails: `apps/openagents-runtime/docs/OBSERVABILITY.md`

## Notes

- `openagents_runtime_provider_breaker_state` alert/panel is intentionally pre-wired for issue `#1686`; expression includes `or vector(0)` so it is safe before breaker metrics are emitted.
- Alert rules assume Prometheus metric naming with dots converted to underscores.
