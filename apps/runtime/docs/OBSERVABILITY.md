# Runtime Observability

This document defines the telemetry contract for `runtime` with explicit cardinality guardrails.

## Cardinality Policy

- High-cardinality identifiers (`run_id`, `tool_call_id`, `authorization_id`, `traceparent`, etc.) are emitted in telemetry metadata for logs/traces and debugging joins.
- Metrics tags are restricted to bounded taxonomies only (status enums, reason classes, phase names, outcomes).
- Runtime metric tag policy lives in `OpenAgentsRuntime.Telemetry.Metrics`.

High-cardinality keys explicitly prohibited from metric labels:

- `run_id`
- `thread_id`
- `frame_id`
- `tool_call_id`
- `authorization_id`
- `lease_owner`
- `traceparent`
- `tracestate`
- `x_request_id`
- `seq`
- `cursor`
- `user_id`
- `guest_scope`

## Runtime Event Families

- `[:openagents_runtime, :executor, ...]`
  - `:run_started`
  - `:frame_processed`
  - `:terminal`
  - `:run_once`
- `[:openagents_runtime, :stream, ...]`
  - `:emit` (runtime event -> SSE mapping)
  - `:session` (tail lifecycle summary)
- `[:openagents_runtime, :tool, :lifecycle]`
  - run start/progress/terminal outcomes
- `[:openagents_runtime, :lease, :operation]`
  - acquire/renew/mark-progress outcomes
- `[:openagents_runtime, :janitor, ...]`
  - `:cycle`
  - `:resumed`
  - `:failed`
- `[:openagents_runtime, :policy, :decision]`
  - DS policy/spend decision and budget counters
- `[:openagents_runtime, :parity, :failure]`
  - parity failure-class envelope across policy/loop/network/manifest/workflow
- `[:openagents_runtime, :khala, :projection, ...]`
  - `:write` (result + lag_events)
  - `:write_failure`
  - `:drift` (reason classes include `summary_hash_mismatch`, `hash_and_lag_drift`, `projection_version_changed`, `checkpoint_ahead`)
  - `:replay`
- `[:openagents_runtime, :sync, :projection, :write]`
  - Khala read-model projection sink write outcomes
- `[:openagents_runtime, :sync, :stream, :append]`
  - Khala stream journal append outcomes
- `[:openagents_runtime, :sync, :retention, ...]`
  - `:cycle` (deleted row counts + topic count per retention pass)
  - `:topic` (per-topic deleted counts, stale-risk, oldest/head watermark, topic class, snapshot capability)
- `[:openagents_runtime, :sync, :socket, ...]`
  - `:connection` (connect/disconnect counts + active connection gauge source)
  - `:queue` (outbound queue depth + overflow status)
  - `:heartbeat` (server/client heartbeat traffic)
  - `:reconnect` (resume reconnect count)
  - `:slow_consumer` (throttle/disconnect policy actions)
  - `:timeout` (heartbeat timeout disconnect count)
- `[:openagents_runtime, :sync, :replay, ...]`
  - `:lag` (topic replay lag at catch-up/live boundaries)
  - `:budget` (topic replay lag vs QoS replay budget ceilings; exceeded-path detection)
  - `:catchup` (replay catch-up duration distribution)
- `[:openagents_runtime, :run_events, :notify]`
  - LISTEN/NOTIFY wakeup path
- `[:openagents_runtime, :agent_process, :stats]`
  - BEAM process-level queue/reduction stats

## Required Correlation

- Laravel forwards `traceparent`, `tracestate`, and `x-request-id`.
- Runtime attaches this context to telemetry metadata via `OpenAgentsRuntime.Telemetry.Events`.
- Join path: browser -> Laravel -> runtime -> tool/model.

## Operational Use

- Use metrics for fleet/system health and bounded alerting.
- Use logs/traces + event metadata for run-level incident debugging.
- Never add run/thread/tool identifiers as metric tags; update `metrics_test.exs` if taxonomy changes.

## Parity Failure Class Diagnosis

- Metric: `openagents_runtime.parity.failure.count`
- Tags: `class`, `reason_class`, `component`, `outcome`

Failure classes and primary emitters:

- `policy`
  - Emitter: `OpenAgentsRuntime.DS.Predict` when policy decision is denied.
  - Inspect alongside: `openagents_runtime.policy.decision.count`.
- `loop`
  - Emitter: `OpenAgentsRuntime.Runs.Executor` when loop detection transitions terminal failure.
  - Inspect alongside: `openagents_runtime.executor.terminal.count`.
- `network`
  - Emitter: `OpenAgentsRuntime.Tools.Network.GuardedHTTP` when outbound request is blocked.
  - Inspect alongside: `openagents_runtime.tool.lifecycle.count`.
- `manifest`
  - Emitter: `OpenAgentsRuntime.Tools.Extensions.ManifestRegistry` on rejected activation.
  - Inspect alongside: `[:openagents_runtime, :tools, :extensions, :manifest_validation]`.
- `workflow`
  - Emitter: `OpenAgentsRuntime.DS.Workflows.StructuredTasks` on budget/schema/step failures.
  - Inspect alongside DS workflow receipts and step receipts.

## Dashboards and alerts

- Dashboard artifact: `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`
- Dashboard artifact: `apps/runtime/deploy/monitoring/grafana/khala-slo-dashboard.json`
- Alert rules: `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
- Alert rules: `apps/runtime/deploy/monitoring/prometheus/khala-slo-alert-rules.yaml`
- Operator runbook: `apps/runtime/docs/OPERATIONS_ALERTING.md`

## Khala SLO Targets (OA-RUST-093)

Primary SLIs/SLOs:

1. WS auth failure ratio <= 3% over 10m window.
2. Replay lag p95 <= 250 events over 10m window.
3. Stale cursor incidents <= 50 per 10m per environment.
4. Reconnect telemetry does not exceed reconnect-storm threshold for sustained 10m.

Segmentation requirements:

1. Topic and topic class (`topic`, `topic_class`) where available.
2. Client surface and app version (`surface`, `app_version`) from client telemetry events.
3. Auth/replay reason classes (`reason_code`) for deny-path diagnostics.

Client telemetry contract authority:

- `proto/openagents/sync/v1/client_telemetry.proto`
- `docs/protocol/client-telemetry-v1.md`
