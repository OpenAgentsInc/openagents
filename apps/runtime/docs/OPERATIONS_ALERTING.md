# Runtime Operations: Dashboards and Alerts

This runbook defines the baseline operational thresholds for the Elixir runtime.

## Dashboard

Grafana dashboard artifact:

- `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`

## Alert Rules

Prometheus rule artifact:

- `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`

## Alert Matrix

### Khala projection SLO budgets

- Lag budget: p95 projection lag <= 25 runtime events over a 10m window.
- Error budget: projection write failures <= 1% over a 10m window.
- Drift budget: <= 3 drift incidents per 10m per service shard.
- Replay budget: zero replay failures in a rolling 15m window.

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

### Khala projection lag

- Alert: `OpenAgentsRuntimeKhalaProjectionLagP95High`
- Threshold: p95 `khala.projection.lag_events > 25` for 10m
- Action:
  1. Inspect projector throughput vs runtime event ingest rate.
  2. Check sink latency/errors and DB lock contention around projection checkpoints.
  3. If lag remains elevated, trigger scoped replay after incident stabilizes.

### Khala projection write failures

- Alert: `OpenAgentsRuntimeKhalaProjectionWriteFailureRatioHigh`
- Threshold: write failure ratio > 1% for 10m
- Action:
  1. Inspect sink failure reason classes (`khala_error`, `sink_exception`, auth errors).
  2. Validate Khala endpoint health and admin key availability.
  3. Start replay plan (`mix runtime.khala.reproject`) once sink health recovers.

### Khala projection drift incidents

- Alert: `OpenAgentsRuntimeKhalaProjectionDriftIncidentsHigh`
- Threshold: drift incidents > 3 over 10m
- Action:
  1. Check drift reason classes (`summary_hash_mismatch`, `projection_version_changed`, `checkpoint_ahead`).
  2. Validate deployment/version alignment across runtime and Khala schema.
  3. Run targeted reproject for affected run/worker IDs and verify checkpoint convergence.

### Khala projection hash mismatch

- Alert: `OpenAgentsRuntimeKhalaProjectionHashMismatchDetected`
- Threshold: any `summary_hash_mismatch` or `hash_and_lag_drift` incident in 10m
- Action:
  1. Treat as critical replay determinism signal; pause rollout expansion.
  2. Inspect drift reason metadata and affected projection scope (`run`/`codex_worker`).
  3. Run targeted replay/reproject for affected entities and confirm hash convergence.
  4. If mismatch persists after replay, escalate as schema/projection compatibility incident.

### Khala projection replay failures

- Alert: `OpenAgentsRuntimeKhalaProjectionReplayFailures`
- Threshold: any replay error in 15m
- Action:
  1. Inspect replay error reason and failing entity scope (`run` vs `codex_worker`).
  2. Confirm checkpoint table health and sequence continuity.
  3. Escalate before rollout expansion; replay failure blocks production promotion.

### Khala token mint failures

- Alert: `OpenAgentsKhalaTokenMintFailureRatioHigh`
- Threshold: token mint failure ratio > 1% for 15m
- Action:
  1. Check Laravel token mint endpoint health (`POST /api/khala/token`) and auth/session middleware failures.
  2. Split failures by class (`authz_denied`, `signing_error`, `upstream_unavailable`) in Laravel logs.
  3. Validate Khala auth key rotation state and runtime bridge config alignment.
  4. If failure ratio stays elevated, pause rollout of new subscription clients and use runtime fallback polling paths.

### End-to-end request correlation walkthrough

Use this flow to trace one worker action across browser -> Laravel -> runtime -> Khala projection telemetry.

1. Capture the request identifiers from the client call.
   - Required headers: `traceparent`, `tracestate`, `x-request-id`.
2. Confirm Laravel forwarded the same headers to runtime.
   - Proxy contract: `apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php`.
3. Confirm runtime response carries an `x-request-id` for runtime-side log correlation.
   - Internal API contract: `apps/runtime/docs/RUNTIME_CONTRACT.md`.
4. Locate runtime telemetry for projector writes and verify metadata carries forwarded correlation IDs.
   - Event family: `[:openagents_runtime, :khala, :projection, :write]`.
   - Correlation contract: `apps/runtime/docs/OBSERVABILITY.md`.
5. Validate project health in Grafana/Prometheus while tracing the same time window.
   - Dashboard: `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`.
   - Alerts: `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`.

### Parity failure class spikes

- Metric: `openagents_runtime.parity.failure.count`
- Class taxonomy: `policy`, `loop`, `network`, `manifest`, `workflow`
- Action:
  1. Split by `class`, `reason_class`, and `component` to identify dominant parity regression lane.
  2. Correlate with paired runtime surfaces:
     - `policy` -> `openagents_runtime.policy.decision.*`
     - `loop` -> `openagents_runtime.executor.terminal.count`
     - `network` -> guarded network block telemetry (`tools.network`)
     - `manifest` -> `tools.extensions.manifest_validation` outcomes
     - `workflow` -> DS structured workflow receipts + step receipts
  3. If class spike follows upstream parity import, run OpenClaw drift report and open ingestion follow-up issue.

## Restart/Reconnect Chaos Rehearsal Gate

Run the restart/reconnect chaos drill before runtime/Khala promotion and after reconnect-related code changes:

- `apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh`

Runbook/report references:

- `apps/runtime/docs/RESTART_RECONNECT_CHAOS.md`
- `apps/runtime/docs/reports/2026-02-21-runtime-khala-restart-reconnect-chaos-report.md`

## Guardrails

- High-cardinality identifiers stay in logs/traces, never metric labels.
- Tag policy is enforced by `test/openagents_runtime/telemetry/metrics_test.exs`.

## Validation

- `mix test test/openagents_runtime/ops/monitoring_assets_test.exs`
- `mix ci`
