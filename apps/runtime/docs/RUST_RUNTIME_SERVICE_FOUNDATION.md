# Rust Runtime Service Foundation

Status: Introduced by OA-RUST-033 and expanded through OA-RUST-047.

This document defines the initial Rust runtime service footprint inside `apps/runtime`.

## Goals

1. Provide a buildable Rust runtime service entrypoint in `apps/runtime`.
2. Establish module boundaries for authority writes, orchestration, and projectors.
3. Expose baseline health/readiness and runtime contract smoke routes.
4. Port worker lifecycle authority basics (registration, heartbeat, status transitions).
5. Enforce deterministic run state transitions for replay-safe runtime events.
6. Introduce durable append-only runtime event log with idempotency/ordering safeguards.

## Current shape

- Cargo package: `openagents-runtime-service` (`apps/runtime/Cargo.toml`)
- Entrypoint: `apps/runtime/src/main.rs`
- Service wiring: `apps/runtime/src/lib.rs`
- HTTP handlers: `apps/runtime/src/server.rs`
- Boundaries:
  - `apps/runtime/src/authority.rs`
  - `apps/runtime/src/history_compat.rs`
  - `apps/runtime/src/orchestration.rs`
  - `apps/runtime/src/projectors.rs`
  - `apps/runtime/src/workers.rs`
  - `apps/runtime/src/types.rs`

## Endpoint contract (foundation scope)

- `GET /healthz` returns service/build metadata.
- `GET /readyz` returns authority/projector readiness state.
- `POST /internal/v1/runs` creates a run and appends `run.started`.
- `POST /internal/v1/runs/:run_id/events` appends runtime events.
- `GET /internal/v1/runs/:run_id` reads current run state.
- `GET /internal/v1/runs/:run_id/receipt` returns deterministic runtime receipt artifact.
- `GET /internal/v1/runs/:run_id/replay` returns deterministic replay JSONL artifact.
- `GET /internal/v1/projectors/checkpoints/:run_id` reads latest projector checkpoint.
- `GET /internal/v1/projectors/run-summary/:run_id` returns projected run read model.
- `GET /internal/v1/projectors/drift?topic=<topic>` returns drift detection metadata.
- `POST /internal/v1/workers` registers worker ownership/lifecycle state.
- `GET /internal/v1/workers/:worker_id` reads owner-scoped worker state.
- `POST /internal/v1/workers/:worker_id/heartbeat` updates worker liveness.
- `POST /internal/v1/workers/:worker_id/status` applies deterministic status transitions.
- `GET /internal/v1/workers/:worker_id/checkpoint` reads worker lifecycle projection checkpoint.
- `GET /internal/v1/khala/topics/:topic/messages` returns replay/live frames with deterministic cursor semantics and delivery policy metadata.
- `GET /internal/v1/khala/fanout/hooks` returns fanout hooks plus delivery metrics and ranked topic windows.
- `GET /internal/v1/khala/fanout/metrics` returns delivery metrics and ranked topic windows (operator-focused).

## Operational notes

1. Runtime event authority now uses a durable JSONL append log; run/projector read models remain in-memory during bootstrap.
2. Run events are durably appended to `RUNTIME_EVENT_LOG_PATH` (JSONL) before in-memory run projection updates.
3. Event append requests support idempotency (`idempotency_key`) and optimistic ordering checks (`expected_previous_seq`).
4. Runtime can emit deterministic receipt (`openagents.receipt.v1`) and replay (`REPLAY.jsonl`) artifacts from authoritative run events.
5. Projection checkpoints/read-model summaries are persisted (`RUNTIME_CHECKPOINT_PATH`) and recovered on restart.
6. Projection apply is idempotent (`seq <= checkpoint.last_seq` no-op) and drift hooks record sequence gaps.
7. Run transitions are validated against a deterministic state machine (`created -> running -> terminal/canceling` lanes) before events are accepted.
8. Runtime authority persistence and full projector parity are delivered in follow-on OA-RUST issues.
9. Shadow-mode parity harness (`runtime-shadow-harness`) compares legacy vs Rust artifacts and enforces cutover gate thresholds.
10. Authority cutover is controlled by `RUNTIME_AUTHORITY_WRITE_MODE`; legacy write freeze is controlled by `LEGACY_RUNTIME_WRITE_FREEZE`.
11. Khala live delivery path is wired through an internal fanout seam (`FanoutDriver`) with bounded in-memory adapter and external-driver hooks.
12. Khala topic polling enforces strict `stale_cursor` semantics (`410` with deterministic replay-floor metadata) and successful poll responses expose replay bootstrap metadata (`oldest_available_cursor`, `head_cursor`, `next_cursor`, `replay_complete`).
13. Stale-cursor payloads include deterministic reason metadata (`reason_codes`, `qos_tier`, `replay_budget_events`, `replay_lag`) so clients can distinguish retention-floor vs replay-budget failures.
14. Khala topic polling enforces sync token auth, topic scope ACL matrix, worker ownership checks, and deterministic denied-path reason codes.
15. Existing Elixir runtime remains present as the migration source until cutover milestones are complete.
16. Workflow history compatibility fixtures (`apps/runtime/fixtures/history_compat/run_workflow_histories_v1.json`) are replayed by `history_compat` tests to gate deterministic upgrade safety for runtime orchestration behavior.
17. Khala polling applies bounded backpressure policy: capped poll limits, minimum poll interval guard, slow-consumer strike/eviction policy, and deterministic reconnect jitter hints.
18. Delivery telemetry includes queue depth, dropped-message counts, poll throttle counters, and recent disconnect causes for operational triage.
19. Khala publish paths enforce topic-class publish-rate and payload-size limits with deterministic violation reason codes.

## Khala backpressure policy defaults

Runtime config variables controlling Khala delivery policy:

- `RUNTIME_KHALA_POLL_DEFAULT_LIMIT` (default `100`)
- `RUNTIME_KHALA_POLL_MAX_LIMIT` (default `200`)
- `RUNTIME_KHALA_OUTBOUND_QUEUE_LIMIT` (default `200`)
- `RUNTIME_KHALA_FAIR_TOPIC_SLICE_LIMIT` (default `50`)
- `RUNTIME_KHALA_POLL_MIN_INTERVAL_MS` (default `250`)
- `RUNTIME_KHALA_SLOW_CONSUMER_LAG_THRESHOLD` (default `300`)
- `RUNTIME_KHALA_SLOW_CONSUMER_MAX_STRIKES` (default `3`)
- `RUNTIME_KHALA_CONSUMER_REGISTRY_CAPACITY` (default `4096`)
- `RUNTIME_KHALA_RECONNECT_BASE_BACKOFF_MS` (default `400`)
- `RUNTIME_KHALA_RECONNECT_JITTER_MS` (default `250`)
- `RUNTIME_KHALA_RUN_EVENTS_PUBLISH_RATE_PER_SECOND` (default `240`)
- `RUNTIME_KHALA_WORKER_LIFECYCLE_PUBLISH_RATE_PER_SECOND` (default `180`)
- `RUNTIME_KHALA_CODEX_WORKER_EVENTS_PUBLISH_RATE_PER_SECOND` (default `240`)
- `RUNTIME_KHALA_FALLBACK_PUBLISH_RATE_PER_SECOND` (default `90`)
- `RUNTIME_KHALA_RUN_EVENTS_REPLAY_BUDGET_EVENTS` (default `20000`)
- `RUNTIME_KHALA_WORKER_LIFECYCLE_REPLAY_BUDGET_EVENTS` (default `10000`)
- `RUNTIME_KHALA_CODEX_WORKER_EVENTS_REPLAY_BUDGET_EVENTS` (default `3000`)
- `RUNTIME_KHALA_FALLBACK_REPLAY_BUDGET_EVENTS` (default `500`)
- `RUNTIME_KHALA_RUN_EVENTS_MAX_PAYLOAD_BYTES` (default `262144`)
- `RUNTIME_KHALA_WORKER_LIFECYCLE_MAX_PAYLOAD_BYTES` (default `65536`)
- `RUNTIME_KHALA_CODEX_WORKER_EVENTS_MAX_PAYLOAD_BYTES` (default `131072`)
- `RUNTIME_KHALA_FALLBACK_MAX_PAYLOAD_BYTES` (default `65536`)

Policy behavior:

1. Requested poll limits above max are capped and reported in response metadata (`limit_applied`, `limit_capped`).
2. Polls faster than `RUNTIME_KHALA_POLL_MIN_INTERVAL_MS` return `429 rate_limited` with `retry_after_ms`.
3. Consumers with repeated lag above threshold are evicted with deterministic `409 slow_consumer_evicted` recovery details.
4. Reconnect guidance includes deterministic jitter (`recommended_reconnect_backoff_ms`) to reduce reconnect herd spikes.
5. Publish bursts above topic-class rate limits return `429 rate_limited` with reason code `khala_publish_rate_limited`.
6. Publish frames above topic-class payload limits return `413 payload_too_large` with reason code `khala_frame_payload_too_large`.
7. When the same principal actively polls multiple topics, per-request replay is fairness-capped by `RUNTIME_KHALA_FAIR_TOPIC_SLICE_LIMIT`.

## History compatibility gate

Run the compatibility harness directly:

```bash
cargo test -p openagents-runtime-service history_compat::tests
```

This gate is also wired into local CI via:

```bash
./scripts/local-ci.sh runtime-history
```
