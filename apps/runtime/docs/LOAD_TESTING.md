# Runtime Load and Chaos Testing

This suite validates production-shaped runtime behavior for concurrency, streaming, cancellation, and recovery.

## Test Suite

- `apps/runtime/test/openagents_runtime/load/runtime_shape_load_test.exs`
- `apps/runtime/test/openagents_runtime/load/khala_projection_load_chaos_test.exs`

Scenarios covered:

1. Concurrent SSE sessions with delayed event production (slow-client shape).
2. Burst frame ingestion with contiguous event sequencing.
3. Cancel storms with idempotent terminal cancellation.
4. Executor-loss recovery (pod-kill equivalent) via janitor + stream cursor resume continuity.
5. Sustained runtime event bursts with bounded Khala projection lag.
6. Codex worker heartbeat bursts with projection checkpoint convergence.
7. Khala sink failure chaos with replay-based projection recovery.
8. Runtime/Khala restart-reconnect chaos drills (forced reconnect, stale cursor, token-expiry reconnect guard).

## Run Commands

From `apps/runtime/`:

- `mix test test/openagents_runtime/load/runtime_shape_load_test.exs`
- `mix test test/openagents_runtime/load/khala_projection_load_chaos_test.exs`
- `mix test --include load`
- `apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh`

## What to inspect

- Run status transitions (`created/running/canceled/succeeded/failed`).
- Event log continuity (`seq` monotonic with no gaps).
- Stream cursor behavior (`cursor` resumes to strict `seq > cursor`).
- Janitor recovery behavior after stale lease detection.
- Khala projection checkpoint convergence (`last_runtime_seq` tracks runtime `latest_seq`).
- Khala failure-mode posture (runtime writes remain durable when sink fails; replay restores projections).

## Pass/Fail Criteria

- Pass: runtime event append/worker ingest operations remain successful under load and chaos.
- Pass: Khala projection checkpoints converge to runtime latest sequence after replay.
- Pass: projection lag/write failure/drift/replay alerts and dashboard panels exist in monitoring assets.
- Fail: runtime write path blocks on projection sink failures.
- Fail: replay cannot restore projection checkpoint state to runtime truth.
- Fail: monitoring assets are missing Khala hardening signals.

## Latest Report

- `apps/runtime/docs/reports/2026-02-19-khala-runtime-projector-load-chaos-report.md`
- `apps/runtime/docs/reports/2026-02-21-runtime-khala-restart-reconnect-chaos-report.md`
- Restart/reconnect drill runbook: `apps/runtime/docs/RESTART_RECONNECT_CHAOS.md`

## Relationship to Operations

Alert and dashboard artifacts for these failure modes:

- `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`
- `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
- `apps/runtime/docs/OPERATIONS_ALERTING.md`
