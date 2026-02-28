# Codex Integration Debug Runbook

This runbook is for diagnosing Codex app-server integration failures in `autopilot-desktop`.

## Fast Triage

1. Open `Codex Diagnostics` pane.
2. Check:
   - `State`
   - `Last action`
   - `Last error`
   - `Last command failure`
   - `Last snapshot error`
3. Confirm whether failures are command rejections, lane disconnects, or protocol parse/notification drift.

## Live Observability

1. In `Codex Diagnostics`, inspect:
   - notification method counters
   - server request counters
   - raw event stream
2. Compare observed methods with expected app-server methods:
   - `crates/codex-client/src/client.rs`
   - `SUPPORTED_CLIENT_REQUEST_METHODS`
   - `SUPPORTED_SERVER_NOTIFICATION_METHODS`
   - `SUPPORTED_SERVER_REQUEST_METHODS`

## Wire Log Capture

1. Enable wire log in `Codex Diagnostics` (default `/tmp/openagents-codex-wire.log`).
2. Reproduce failure.
3. Disable wire log (pane restarts Codex lane).
4. Inspect captured frames:
   - outbound request method and params
   - inbound response/notification shape
   - JSON parse errors or method mismatches

## Repro/Verification Commands

1. Protocol parity + Codex smoke gate:
   - `./scripts/lint/codex-protocol-parity-gate.sh`
2. Targeted lane tests:
   - `cargo test -p autopilot-desktop thread_lifecycle_notifications_are_normalized -- --nocapture`
   - `cargo test -p autopilot-desktop apps_and_remote_skill_export_emit_notifications -- --nocapture`
   - `cargo test -p autopilot-desktop labs_api_smoke_commands_emit_responses_and_notifications -- --nocapture`

## Common Failure Patterns

1. `no rollout found` on thread resume:
   - stale thread metadata; remove stale thread and start new thread.
2. Parse failures in server requests/notifications:
   - check for upstream method/shape drift and update lane normalization/typing.
3. Missing features in UI despite command support:
   - verify pane command registration (`pane_registry`) and hit-action routing (`pane_system`, `input`).
4. Lane startup/disconnect loops:
   - inspect app-server availability, cwd validity, and wire-log output.

## Escalation Data To Include

1. Commit hash.
2. `Codex Diagnostics` screenshot text values.
3. Relevant wire log excerpts (request + response/notification pair).
4. Failing test command/output.
5. Whether failure reproduces against local `~/code/codex` app-server.
