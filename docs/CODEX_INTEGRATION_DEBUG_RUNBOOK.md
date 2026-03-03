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
4. If the incident is chat-driven CAD, capture:
   - tool call name (`openagents.cad.intent` / pane tool)
   - tool response code (`OA-*`)
   - CAD failure class (`tool_transport`, `intent_parse_validation`, `dispatch_rebuild`).

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
3. For CAD incidents, inspect the active chat progress block rows:
   - phase
   - tool result
   - rebuild result
   - failure class
   - retries.

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
2. CAD chat-build release gate:
   - `./scripts/cad/release-gate-checklist.sh`
3. Targeted lane tests:
   - `cargo test -p autopilot-desktop thread_lifecycle_notifications_are_normalized -- --nocapture`
   - `cargo test -p autopilot-desktop apps_and_remote_skill_export_emit_notifications -- --nocapture`
   - `cargo test -p autopilot-desktop labs_api_smoke_commands_emit_responses_and_notifications -- --nocapture`
4. Targeted CAD integration tests:
   - `cargo test -p autopilot-desktop cad_chat_build_e2e_harness -- --nocapture`
   - `cargo test -p autopilot-desktop cad_chat_build_e2e_harness_week1_gripper_matches_golden -- --nocapture`
   - `cargo test -p autopilot-desktop tool_bridge -- --nocapture`

## Common Failure Patterns

1. `no rollout found` on thread resume:
   - stale thread metadata; remove stale thread and start new thread.
2. Parse failures in server requests/notifications:
   - check for upstream method/shape drift and update lane normalization/typing.
3. Missing features in UI despite command support:
   - verify pane command registration (`pane_registry`) and hit-action routing (`pane_system`, `input`).
4. Lane startup/disconnect loops:
   - inspect app-server availability, cwd validity, and wire-log output.
5. CAD tool parse/shape failures:
   - inspect `openagents.cad.intent` payload shape and `intent_json` schema.
6. CAD dispatch/rebuild failures:
   - inspect CAD checkpoint payload and reducer rebuild queue events.
   - confirm no stale pending request IDs.
7. CAD tool intentionally disabled:
   - check `OPENAGENTS_CAD_INTENT_TOOL_ENABLED`.
   - when disabled, expected code is `OA-CAD-INTENT-DISABLED`.

## Escalation Data To Include

1. Commit hash.
2. `Codex Diagnostics` screenshot text values.
3. Relevant wire log excerpts (request + response/notification pair).
4. Failing test command/output.
5. Whether failure reproduces against local `~/code/codex` app-server.
6. CAD checkpoint snippet from tool response (`details.checkpoint`) if incident involved CAD turn.

## Related Docs

- `docs/codex/CODEX_PANE_CAD_TOOLING.md`
- `docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md`
- `docs/codex/CAD_CHAT_BUILD_RELEASE_RUNBOOK.md`
