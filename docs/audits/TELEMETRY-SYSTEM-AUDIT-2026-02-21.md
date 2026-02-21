# Telemetry System Audit (2026-02-21)

Status: active  
Owner: `owner:infra` + `owner:runtime` + `owner:openagents.com`

## Scope

Audit current telemetry/observability systems in repo and define what a unified telemetry system should be for the Rust-only target architecture.

## Sources reviewed

- `apps/openagents.com/service/src/observability.rs`
- `apps/openagents.com/service/src/lib.rs`
- `apps/openagents.com/service/README.md`
- `apps/runtime/src/main.rs`
- `apps/runtime/src/server.rs`
- `apps/runtime/src/fanout.rs`
- `apps/runtime/docs/OBSERVABILITY.md`
- `apps/runtime/deploy/monitoring/README.md`
- `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
- `apps/runtime/deploy/monitoring/prometheus/khala-slo-alert-rules.yaml`
- `docs/protocol/client-telemetry-v1.md`
- `proto/openagents/sync/v1/client_telemetry.proto`
- `apps/openagents.com/resources/js/lib/posthog.ts`
- `apps/openagents.com/resources/js/hooks/use-posthog-event.ts`
- `apps/openagents.com/web-shell/scripts/perf-soak-signoff.sh`
- `apps/lightning-ops/src/main.rs`

## Current telemetry systems

### 1) Rust control service (openagents-control-service)

Implemented:

1. Structured logs via `tracing` (`json` default).
2. Request ID propagation middleware (`x-request-id`).
3. Security/audit event stream through `AuditEvent` records (auth/session/route-split/sync-token events).
4. In-process counters (`increment_counter`) logged under `oa.metric`.

Gaps:

1. No Prometheus/OpenTelemetry metrics export endpoint.
2. Counters are in-memory and reset on process restart.
3. No centralized sink abstraction beyond structured logs.

### 2) Rust runtime service (`apps/runtime`)

Implemented:

1. Structured JSON logs in runtime binary startup.
2. Internal JSON diagnostics endpoints for Khala delivery/fanout:
   - `/internal/v1/khala/fanout/metrics`
   - `/internal/v1/khala/fanout/hooks`
3. Delivery counters include poll/queue/fairness/slow-consumer data in process memory.

Gaps:

1. No `/metrics` Prometheus endpoint in runtime server.
2. Alert rules and dashboards reference metric families not emitted by current Rust runtime process.
3. Observability docs still contain legacy/BEAM naming and stale references (for example module names that are not part of current Rust codepaths).

### 3) Client telemetry contract (proto-first)

Implemented:

1. Canonical schema exists:
   - `proto/openagents/sync/v1/client_telemetry.proto`
2. Contract doc and fixture coverage exist:
   - `docs/protocol/client-telemetry-v1.md`
   - `crates/openagents-proto/tests/client_telemetry_contract.rs`

Gaps:

1. No active runtime/control ingestion endpoint for client telemetry events.
2. No client-core emitter module in `crates/openagents-client-core`.
3. Alerting references `openagents_sync_client_telemetry_event_count` despite no active ingestion/export path in current Rust services.

### 4) Web analytics (legacy Laravel/Inertia app)

Implemented:

1. PostHog browser analytics client initialization and capture helper.
2. User identification via PostHog identify component.

Gaps:

1. This lane is tied to legacy web runtime, not Rust control/web-shell runtime.
2. No unified correlation with Rust request/audit IDs.

### 5) Web-shell performance telemetry artifacts

Implemented:

1. `perf-soak-signoff.sh` records boot/auth/sync latency budgets and memory growth.
2. Results written to JSON artifacts in `apps/openagents.com/web-shell/perf/`.

Gaps:

1. Artifact-based release evidence only; not a live observability backend.
2. No centralized dashboard/query store for historical trend analysis.

### 6) Lightning observability smoke artifacts

Implemented:

1. `smoke:observability` and full-flow commands emit structured JSON and `events.jsonl` artifacts.

Gaps:

1. Artifact-centric smoke outputs are not part of a shared telemetry backend.
2. Correlation is mostly per-command/request-id and not unified with control/runtime telemetry fabric.

## Overall assessment

Current telemetry is functional but fragmented:

1. Control service has useful audit logs and request correlation.
2. Runtime has useful internal delivery counters for Khala diagnostics.
3. Contract-level client telemetry exists in proto, but runtime ingestion is not implemented.
4. Legacy PostHog analytics still represent the most mature product analytics lane, but that lane is not aligned with Rust target architecture.
5. Monitoring assets (Prometheus/Grafana) are ahead of runtime emission implementation.

This means we currently have good local diagnostics and some good contracts, but no single coherent telemetry plane spanning Rust control + runtime + client surfaces.

## What a unified telemetry system should entail

### A) One canonical telemetry model (proto-first)

1. Keep proto as schema authority for telemetry envelopes and event kinds.
2. Split event classes explicitly:
   - `audit_event`
   - `service_metric`
   - `client_diagnostic`
   - `performance_budget_sample`
3. Keep privacy constraints enforceable in schema (hashed actor scope, no raw PII payload fields in client diagnostics).

### B) End-to-end correlation contract

1. Standardize correlation keys across all surfaces/services:
   - `x-request-id`
   - `traceparent` / `tracestate`
   - `session_id`
   - `device_id` (hashed/pseudonymous for analytics lanes)
   - topic/run/worker identifiers where operationally needed
2. Require propagation in:
   - web-shell client
   - iOS client
   - desktop client
   - control service -> runtime internal calls

### C) Rust-native telemetry export path

1. Adopt OpenTelemetry in Rust services for logs/metrics/traces export.
2. Add service metrics endpoint/export (Prometheus or OTLP collector sink) for control and runtime.
3. Preserve existing structured audit logs, but route them to centralized storage with retention/query policy.

### D) Implement client telemetry ingestion now (using existing proto)

1. Add control-plane authenticated endpoint for `ClientTelemetryEvent` ingestion.
2. Validate schema version + surface + build metadata at ingest.
3. Aggregate into metrics used by Khala SLO rules (reconnect/auth-failure/replay status).
4. Add backpressure/drop policy and bounded local queue on clients.

### E) Unify release evidence with live observability

1. Keep perf/signoff JSON artifacts.
2. Also emit sampled metrics from those runs into telemetry backend for trend and regression monitoring.
3. Keep dashboards and alert rules synchronized to actually emitted metric names.

### F) Data governance and retention policy

1. Define retention per class:
   - audit/security events (longer retention)
   - high-volume diagnostics (shorter retention)
2. Define redaction policy per field class (PII/secrets/token material forbidden).
3. Add CI checks ensuring telemetry schema + alert rule references are consistent with emitted metrics.

## Recommended immediate next steps

1. Implement runtime/control emitted metrics parity with `apps/runtime/deploy/monitoring/*` rule expectations.
2. Implement client telemetry ingestion endpoint and connect existing proto schema.
3. Add `openagents-client-core` telemetry emitter module shared by web/desktop/iOS.
4. Normalize observability docs to Rust-only terminology and remove legacy module references.
5. Add a telemetry conformance test that fails when alert rule metric names have no emitting source in Rust services.
