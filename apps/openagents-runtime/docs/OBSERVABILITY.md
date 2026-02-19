# Runtime Observability

This document defines the telemetry contract for `openagents-runtime` with explicit cardinality guardrails.

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

## Dashboards and alerts

- Dashboard artifact: `apps/openagents-runtime/deploy/monitoring/grafana/openagents-runtime-ops-dashboard.json`
- Alert rules: `apps/openagents-runtime/deploy/monitoring/prometheus/openagents-runtime-alert-rules.yaml`
- Operator runbook: `apps/openagents-runtime/docs/OPERATIONS_ALERTING.md`
