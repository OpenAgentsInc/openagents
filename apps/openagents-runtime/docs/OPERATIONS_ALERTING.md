# Runtime Operations: Dashboards and Alerts

This runbook defines the baseline operational thresholds for the Elixir runtime.

## Dashboard

Grafana dashboard artifact:

- `apps/openagents-runtime/deploy/monitoring/grafana/openagents-runtime-ops-dashboard.json`

## Alert Rules

Prometheus rule artifact:

- `apps/openagents-runtime/deploy/monitoring/prometheus/openagents-runtime-alert-rules.yaml`

## Alert Matrix

### Executor latency

- Alert: `OpenAgentsRuntimeExecutorLatencyP95High`
- Threshold: p95 `executor.run_once.duration_ms > 5000` for 10m
- Action:
  1. Check provider latency and timeout errors in traces/logs.
  2. Check janitor cycle, queue pressure, and lease churn.
  3. Validate tool runner timeout/failure rates.

### HTTP 5xx rate

- Alert: `OpenAgentsRuntimeHttp5xxRateHigh`
- Threshold: 5xx ratio > 2% for 10m
- Action:
  1. Slice failing endpoints by trace/request id.
  2. Confirm DB connectivity and LISTEN/NOTIFY health.
  3. Check recent deploy and schema compatibility.

### Stream completion ratio

- Alert: `OpenAgentsRuntimeStreamDoneRatioLow`
- Threshold: `run.finished` emit/session ratio < 95% for 15m
- Action:
  1. Check stream session outcomes (`tail_timeout` vs `client_closed`).
  2. Validate stream cursor continuity and run terminal events.
  3. Inspect cancel storms or downstream client disconnect spikes.

### Lease steal rate

- Alert: `OpenAgentsRuntimeLeaseStealRateHigh`
- Threshold: steals > 0.05/s for 10m
- Action:
  1. Check pod restarts and runtime process churn.
  2. Inspect stale lease expiration vs progress movement.
  3. Confirm janitor reconcile behavior and cooldown settings.

### Tool failure rate

- Alert: `OpenAgentsRuntimeToolFailureSpike`
- Threshold: terminal `failed|timeout` > 0.2/s for 10m
- Action:
  1. Split by tool and provider.
  2. Confirm upstream provider health and circuit breaker status.
  3. Evaluate timeout budgets and cancellation semantics.

### Provider circuit breakers

- Alert: `OpenAgentsRuntimeCircuitBreakerOpen`
- Threshold: any open breaker for 5m
- Action:
  1. Confirm upstream provider incident.
  2. Validate receipt-visible fallback behavior.
  3. Coordinate rollback/fallback policy if user impact persists.

### Spend/policy denial anomalies

- Alert: `OpenAgentsRuntimePolicyDenialAnomaly`
- Threshold: non-allowed decisions > 5% for 10m
- Action:
  1. Identify dominant denial reason (`budget_exhausted`, `denied_*`).
  2. Validate authorization envelope rollout and limits.
  3. Confirm no runaway loops consuming delegated budget.

## Guardrails

- High-cardinality identifiers stay in logs/traces, never metric labels.
- Tag policy is enforced by `test/openagents_runtime/telemetry/metrics_test.exs`.

## Validation

- `mix test test/openagents_runtime/ops/monitoring_assets_test.exs`
- `mix ci`
