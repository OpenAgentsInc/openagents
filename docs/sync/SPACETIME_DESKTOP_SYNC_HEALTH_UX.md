# Spacetime Desktop Sync Health UX

Status: active  
Updated: 2026-02-25

## Scope

Desktop Runtime Login now exposes sync-health indicators that let operators diagnose connectivity and replay posture without digging through logs.

Implementation surface:
- `apps/autopilot-desktop/src/sync_lifecycle.rs`
- `apps/autopilot-desktop/src/main.rs` (`RuntimeCodexSync` replay/lifecycle updates)
- `crates/autopilot_app/src/lib.rs` (`RuntimeAuthStateView` sync-health fields)
- `crates/autopilot_ui/src/render.rs` (Runtime Login pane rendering)

## Indicators

Runtime Login status now includes:
- connection state and reconnect posture (`state`, attempts, next retry)
- disconnect reason and last sync error
- token lease refresh countdown (`refresh_after_in_seconds`)
- replay cursor and replay target
- replay lag (`target - cursor`)
- replay progress percent (`cursor / target`)

## Actionable Guidance

Runtime Login surfaces explicit operator actions when sync degrades:
- `unauthorized` / `forbidden`: re-authenticate in Runtime Login
- `token_refresh_due`: token lease rotated; reconnect/remint path is active
- `stale_cursor`: replay cursor rewound and rebootstrap path is active
- short token lease (`<=60s`): warning that reconnect/refresh is imminent

## Verification

Lifecycle/replay metric tests:
- `cargo test -p autopilot-desktop sync_lifecycle -- --nocapture`

Desktop integration checks:
- `cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture`
- `cargo test -p autopilot-desktop sync_apply_engine -- --nocapture`
