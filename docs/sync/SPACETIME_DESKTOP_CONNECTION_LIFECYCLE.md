# Spacetime Desktop Connection Lifecycle

Status: active  
Updated: 2026-02-25

## Scope

Desktop runtime sync now tracks a first-class connection lifecycle with explicit reconnect policy, token-lease refresh behavior, and UI-visible health snapshots.

Implementation surface:
- `apps/autopilot-desktop/src/sync_lifecycle.rs`
- `apps/autopilot-desktop/src/main.rs` (`RuntimeCodexSync` stream loop + token lease handling)
- `crates/autopilot_app/src/lib.rs` (`RuntimeAuthStateView` sync-health fields)
- `crates/autopilot_ui/src/render.rs` (Runtime Login status rendering)

## Lifecycle States

Per worker stream, desktop records:
- `idle`
- `connecting`
- `live`
- `backoff`

Disconnect reasons are normalized to:
- `stream_closed`
- `token_refresh_due`
- `stale_cursor`
- `unauthorized`
- `forbidden`
- `network`
- `unknown`

## Reconnect and Refresh Policy

- Exponential reconnect backoff starts at `250ms` and caps at `8000ms`.
- Token lease refresh (`refresh_after_in`) triggers an immediate fast reconnect path (`100ms`) and forces sync token remint.
- `stale_cursor` disconnects trigger replay cursor reset (`cursor=0`) before reconnect.
- `unauthorized` / `forbidden` / `token_refresh_due` reconnect plans force auth refresh from disk before reconnect.

## UI Health Contract

Runtime auth view now includes sync-health fields:
- `sync_worker_id`
- `sync_connection_state`
- `sync_connect_attempts`
- `sync_reconnect_attempts`
- `sync_next_retry_ms`
- `sync_last_disconnect_reason`
- `sync_last_error`
- `sync_token_refresh_after_in_seconds`

These are surfaced in the Runtime Login pane so desktop operators can observe current sync status and reconnect posture.

## Verification

Lifecycle tests (simulated disconnect/reconnect sequences):
- `cargo test -p autopilot-desktop sync_lifecycle -- --nocapture`
