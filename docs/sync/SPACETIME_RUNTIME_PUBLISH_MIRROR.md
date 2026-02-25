# Spacetime Runtime Publish Mirror

Status: active
Date: 2026-02-25

## Scope

Runtime authority events continue to be produced by runtime authority paths, then mirrored into
Spacetime reducer storage for sync delivery parity. This mirror does not change authority ownership.

## Topic to Stream Mapping

Runtime sync topic keys map to Spacetime `stream_id` as follows:

1. `run:<run_id>:events` -> `runtime.run.<run_id>.events`
2. `worker:<worker_id>:lifecycle` -> `runtime.worker.<worker_id>.lifecycle`
3. `fleet:user:<user_id>:workers` -> `runtime.fleet.user.<user_id>.workers`
4. `fleet:guest:<guest_id>:workers` -> `runtime.fleet.guest.<guest_id>.workers`
5. `runtime.codex_worker_events` -> `runtime.codex.worker.events`
6. fallback -> `runtime.topic.<topic-with-colons-replaced-by-dots>`

## Idempotency and Ordering

1. Mirror publishes use deterministic idempotency key:
   - `topic:<topic>:seq:<sequence>:kind:<kind>`
2. Mirror append requires `expected_next_seq = sequence` for each stream.
3. Runtime retries transient publish failures with bounded backoff.
4. Duplicate idempotency keys are treated as success (idempotent duplicate).
5. Failed publishes are enqueued to durable outbox for replay on next publish cycle.

## Parity Checks

Runtime mirror health is validated by:

1. deterministic topic->stream mapping tests,
2. idempotent duplicate publish tests,
3. out-of-order/sequence conflict rejection tests,
4. durable outbox enqueue-on-failure tests.
