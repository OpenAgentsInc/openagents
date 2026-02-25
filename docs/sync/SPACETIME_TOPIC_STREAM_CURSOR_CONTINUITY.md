# Topic-to-Stream Mapping and Cursor Continuity

Status: active
Date: 2026-02-25

## Purpose

Define deterministic migration from legacy Khala cursor semantics `(topic, seq)` to Spacetime
cursor semantics `(stream_id, seq)` while preserving replay and stale-cursor behavior.

## Canonical Mapping

1. `run:<run_id>:events` -> `runtime.run.<run_id>.events`
2. `worker:<worker_id>:lifecycle` -> `runtime.worker.<worker_id>.lifecycle`
3. `fleet:user:<user_id>:workers` -> `runtime.fleet.user.<user_id>.workers`
4. `fleet:guest:<guest_id>:workers` -> `runtime.fleet.guest.<guest_id>.workers`
5. `runtime.codex_worker_events` -> `runtime.codex.worker.events`
6. fallback topic -> `runtime.topic.<topic-with-colons-replaced-by-dots>`

Reverse mapping is supported for retained streams so old cursors can be translated and traced.

## Cursor Migration Strategy

Input:

1. legacy cursor: `{ topic, after_seq }`

Migration:

1. map `topic -> stream_id`
2. produce migrated cursor `{ stream_id, after_seq }` with unchanged sequence
3. resume against current stream window `{ oldest_seq, head_seq, replay_budget_events }`

Continuity rules:

1. If stream has no window yet, resume from migrated cursor.
2. If `after_seq < oldest_seq - 1`, treat as stale and require rebootstrap.
3. If `head_seq - after_seq > replay_budget_events`, treat as stale and require rebootstrap.
4. Otherwise resume replay/live tail from migrated cursor.

## Rebootstrap Path

On stale cursor:

1. emit reason codes:
   - `retention_floor_breach`
   - `replay_budget_exceeded`
2. reset cursor to stream bootstrap baseline (`after_seq = 0`) and request fresh snapshot.
3. apply snapshot atomically, then continue with ordered deltas.

## Verification Coverage

Coverage is implemented in `crates/autopilot-spacetime/src/mapping.rs`:

1. retained topic mapping tests
2. stream roundtrip tests
3. topic cursor migration tests
4. stale cursor retention-floor simulation
5. stale cursor replay-budget simulation
6. in-window resume simulation
