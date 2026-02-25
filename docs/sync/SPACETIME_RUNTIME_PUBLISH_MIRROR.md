# Spacetime Runtime Publish Mirror

Status: active
Date: 2026-02-25

## Scope

Runtime authority events continue to be produced by runtime authority paths, then mirrored into
Spacetime reducer storage for sync delivery parity. This mirror does not change authority ownership.

## Topic to Stream Mapping

Runtime fanout topic keys map to Spacetime `stream_id` as follows:

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
3. Sequence conflicts are retried with bounded backoff.
4. Duplicate idempotency keys are treated as success (idempotent duplicate).

## Parity Checks

For each mirrored event, runtime validates:

1. Stream id equals mapped stream id.
2. Sequence equals runtime fanout sequence.
3. Stored `payload_hash` equals canonical hash of mirrored payload.
4. Stored bytes equal encoded mirrored payload bytes.
5. Durable offset equals fanout sequence.

If parity validation fails, mirror publish returns an explicit error and increments failure metrics.
