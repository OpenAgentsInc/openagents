# Runtime Load and Chaos Testing

This suite validates production-shaped runtime behavior for concurrency, streaming, cancellation, and recovery.

## Test Suite

- `apps/openagents-runtime/test/openagents_runtime/load/runtime_shape_load_test.exs`

Scenarios covered:

1. Concurrent SSE sessions with delayed event production (slow-client shape).
2. Burst frame ingestion with contiguous event sequencing.
3. Cancel storms with idempotent terminal cancellation.
4. Executor-loss recovery (pod-kill equivalent) via janitor + stream cursor resume continuity.

## Run Commands

From `apps/openagents-runtime/`:

- `mix test test/openagents_runtime/load/runtime_shape_load_test.exs`
- `mix test --include load`

## What to inspect

- Run status transitions (`created/running/canceled/succeeded/failed`).
- Event log continuity (`seq` monotonic with no gaps).
- Stream cursor behavior (`cursor` resumes to strict `seq > cursor`).
- Janitor recovery behavior after stale lease detection.

## Relationship to Operations

Alert and dashboard artifacts for these failure modes:

- `apps/openagents-runtime/deploy/monitoring/grafana/openagents-runtime-ops-dashboard.json`
- `apps/openagents-runtime/deploy/monitoring/prometheus/openagents-runtime-alert-rules.yaml`
- `apps/openagents-runtime/docs/OPERATIONS_ALERTING.md`
