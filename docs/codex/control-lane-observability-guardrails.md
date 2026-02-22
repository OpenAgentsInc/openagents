# Codex Control Lane Observability and Guardrails

Status: Active  
Date: 2026-02-22  
Issue: #1952 (P4-03)

This document defines the operational signals and policy guardrails for the mobile-to-desktop Codex control lane:

- `POST /api/runtime/codex/workers/{workerId}/requests`

Related contracts:

- `docs/protocol/codex-worker-control-v1.md`
- `docs/protocol/codex-worker-events-v1.md`
- `apps/runtime/docs/OBSERVABILITY.md`

## Correlation ID Contract

Correlation is keyed by request ID end to end:

1. iOS sends `request.request_id` and `X-Request-Id` on `/requests`.
   - Implementation: `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexClient.swift`
2. Laravel control API validates `request.request_id` and allowlisted `request.method`.
   - Implementation: `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
3. If `X-Request-Id` is missing, Laravel promotes `request.request_id` to `x-request-id` before runtime dispatch.
4. Runtime/desktop execution emits terminal receipts (`worker.response` or `worker.error`) with the same `request_id`.
   - Contract: `docs/protocol/codex-worker-control-v1.md`
   - Desktop handling: `apps/autopilot-desktop/src/main.rs`
5. iOS receipt reconciliation uses `request_id` as the terminal dedupe key.
   - Implementation: `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexModels.swift`

## Control Lane Telemetry

Laravel emits structured logs for each control request:

- `runtime codex control request dispatched`
- `runtime codex control request failed`

Log fields:

- `worker_id`
- `request_id`
- `method`
- `request_version`
- `source`
- `correlation_id`
- `delivery_lag_ms` (derived from `request.sent_at`)
- `duration_ms` (control proxy dispatch time)
- `status`
- `ok`
- `error` (failed only)

## Dashboard and Alert Spec

Create a `Codex Control Lane` dashboard from the structured logs above (or derived log-based metrics).

1. Command failure ratio
   - Signal: `failed_count / (failed_count + dispatched_count)`
   - Group by: `method`, `source`, `request_version`
   - Alert:
     - warning: ratio `> 0.02` for 10m
     - critical: ratio `> 0.05` for 10m
2. Delivery lag p95
   - Signal: p95(`delivery_lag_ms`)
   - Group by: `method`, `source`
   - Alert:
     - warning: p95 `> 3000` ms for 10m
     - critical: p95 `> 10000` ms for 10m
3. Control dispatch duration p95
   - Signal: p95(`duration_ms`)
   - Group by: `method`
   - Alert:
     - warning: p95 `> 1500` ms for 10m
     - critical: p95 `> 5000` ms for 10m
4. Stale cursor recovery pressure (runtime lane dependency)
   - Signal: `sum(increase(openagents_runtime_sync_replay_catchup_duration_ms_count{status="stale_cursor"}[10m]))`
   - Existing SLO alert source: `apps/runtime/deploy/monitoring/prometheus/khala-slo-alert-rules.yaml`
   - Existing service alert source: `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`

## Guardrails (Active)

Policy controls on `POST /api/runtime/codex/workers/{workerId}/requests`:

1. Method allowlist enforced before runtime dispatch:
   - `thread/start`
   - `thread/resume`
   - `turn/start`
   - `turn/interrupt`
   - `thread/list`
   - `thread/read`
2. Required `request.request_id` validation for idempotency/correlation.
3. Rate limit middleware on control request lane: `throttle:60,1`.
4. Non-allowlisted methods fail validation and are not forwarded to runtime.

Implementation:

- `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
- `apps/openagents.com/routes/api.php`

## Verification

Primary tests for this slice:

- `apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php`
  - allowlist rejection before dispatch
  - request_id -> `X-Request-Id` fallback forwarding
  - route rate limiting
- `apps/autopilot-ios/Autopilot/AutopilotTests/AutopilotTests.swift`
  - `/requests` sends `X-Request-Id` matching `request.request_id`
