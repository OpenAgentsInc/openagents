# Spacetime Desktop Apply Engine

Status: active  
Updated: 2026-02-25

## Scope

Desktop runtime sync now enforces deterministic client apply semantics keyed by `(stream_id, seq)` for incoming worker events.

Implementation surface:
- `apps/autopilot-desktop/src/sync_apply_engine.rs`
- `apps/autopilot-desktop/src/main.rs` (`process_spacetime_update_batch`)

## Contract

Per stream:
- maintain a monotonic `watermark` (`last_applied_seq`)
- suppress duplicate deliveries (`seq <= watermark`)
- reject out-of-order gaps (`seq > watermark + 1`)
- require seeded snapshot/checkpoint before delta apply

Apply outcomes:
- `Applied { watermark }`
- `Duplicate { watermark }`
- `OutOfOrder { watermark, incoming }`
- `SnapshotRequired { watermark, incoming }`

## Runtime Behavior

- Worker stream ID is normalized as `runtime.codex.worker.events.<worker_id>`.
- On connect bootstrap, apply engine seeds stream checkpoint from known replay cursor.
- Incoming delta events are previewed before processing; only validated events are executed and then committed.
- If processing fails or sequence gaps are detected, replay cursor rewinds and apply watermark rewinds to preserve idempotent replay behavior.
- Stale cursor resets rewind stream checkpoint to `0` and force replay bootstrap.

## Verification

Apply engine tests:
- `cargo test -p autopilot-desktop sync_apply_engine -- --nocapture`
