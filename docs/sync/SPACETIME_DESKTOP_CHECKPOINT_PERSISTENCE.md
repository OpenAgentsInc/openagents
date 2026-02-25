# Spacetime Desktop Checkpoint Persistence

Status: active  
Updated: 2026-02-25

## Scope

Desktop runtime sync now persists per-worker stream replay checkpoints to local disk and restores those checkpoints on reconnect/restart.

Implementation surface:
- `apps/autopilot-desktop/src/sync_checkpoint_store.rs`
- `apps/autopilot-desktop/src/main.rs` (`run_worker_stream_loop`, `process_spacetime_update_batch`)

## Checkpoint Contract

Storage file:
- default path: `${LOCAL_DATA_DIR}/openagents/runtime-sync-checkpoints.v1.json`
- fallback path: `~/.openagents/runtime-sync-checkpoints.v1.json`
- schema version: `1`

Entry shape:
- `worker_id`
- `stream_id`
- `watermark`
- `updated_at` (RFC3339)

## Resume Selection Policy

Desktop resolves replay cursor from:
- local checkpoint watermark (if present)
- runtime-reported remote latest sequence (if available)

Decision rules:
- local <= remote: resume from local checkpoint
- local > remote: clamp to remote head (stale local checkpoint safety)
- local only: resume from local checkpoint
- remote only: seed from remote head
- neither: fallback to `0`

## Persistence/Recovery Behavior

- On successful watermark advance, desktop writes checkpoint updates to disk.
- On replay rewind (out-of-order/retry), desktop rewinds checkpoint to retry cursor.
- On stale-cursor reconnect reset, desktop rewinds checkpoint to `0`.
- Corrupt or unreadable checkpoint files recover as empty state without crashing stream startup.

## Verification

Checkpoint store tests:
- `cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture`

Related sync correctness tests:
- `cargo test -p autopilot-desktop sync_apply_engine -- --nocapture`
- `cargo test -p autopilot-desktop sync_lifecycle -- --nocapture`
