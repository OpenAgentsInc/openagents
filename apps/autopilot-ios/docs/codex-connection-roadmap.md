# Autopilot iOS Codex Connection Roadmap

Status: active

## Goal

Deliver reliable iOS administration of Codex workers through the Rust control/runtime stack.

## Architecture

1. iOS -> control-service HTTP APIs for commands/mutations.
2. iOS -> Khala websocket for projection updates.
3. Runtime remains execution authority.
4. Control-plane remains auth/session/token authority.

## Phases

1. Foundation
- stabilize auth refresh + device session behavior.
- lock shared wire/domain models from `proto/`.

2. Read path parity
- worker list, worker snapshot, run summaries.
- websocket subscribe/replay/resume with persisted watermarks.

3. Admin actions
- send request/stop commands via control-service.
- show receipts/errors with deterministic UX states.

4. Real-device handshake lane
- desktop worker selected.
- iOS sends handshake event.
- desktop/runtime emits ack.
- iOS marks connected state.

5. Reliability hardening
- reconnect chaos tests.
- stale cursor recovery tests.
- telemetry + SLO dashboards.

## Non-goals

1. Direct iOS-to-local runtime authority writes.
2. WebSocket command RPC.
3. Manual endpoint configuration in default UX.
