# iOS Codex WGPUI Migration Program

Date: 2026-02-22
Status: active
Owner: autopilot-ios

## Objective

Replace SwiftUI-powered iOS Codex product UI/state orchestration with Rust/WGPUI so iOS follows the same app-server protocol and UI/runtime architecture as web/desktop.

## Non-negotiable invariants

1. Product UI must be Rust/WGPUI for iOS Codex surfaces.
2. Swift/SwiftUI is host bridge only (bootstrapping + OS integration).
3. Live transport remains Khala WebSocket only; no SSE lanes.
4. Authority writes remain authenticated HTTP APIs.

Canonical references:
- `docs/plans/active/rust-migration-invariant-gates.md` (`INV-11`)
- `apps/autopilot-ios/docs/ios-codex-first-structure.md`
- `docs/ARCHITECTURE-RUST.md`

## Current implementation inventory (to migrate)

SwiftUI/UI lane:
- `apps/autopilot-ios/Autopilot/Autopilot/ContentView.swift`

Swift orchestration lane:
- `apps/autopilot-ios/Autopilot/Autopilot/CodexHandshakeViewModel.swift`
- `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexClient.swift`
- `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexModels.swift`
- `apps/autopilot-ios/Autopilot/Autopilot/CodexHandshakeMatcher.swift`

Rust lane already present:
- `crates/openagents-client-core/*` (auth/khala/control helpers + FFI)
- `crates/wgpui/src/platform.rs` (`ios` background renderer)
- `crates/openagents-app-state/*` (shared app reducer/intent model)
- `crates/openagents-ui-core/*` (shared tokens/primitives)

## App-server protocol features that must remain supported

1. Auth/session:
- `POST /api/auth/email`
- `POST /api/auth/verify`
- `POST /api/auth/refresh`
- `GET /api/auth/session`
- `POST /api/auth/logout`

2. Worker/runtime:
- `GET /api/runtime/codex/workers`
- `GET /api/runtime/codex/workers/{workerId}`
- `POST /api/runtime/codex/workers/{workerId}/events`
- `POST /api/runtime/codex/workers/{workerId}/requests`
- `POST /api/runtime/codex/workers/{workerId}/stop`

3. Sync:
- `POST /api/sync/token`
- `WS /sync/socket/websocket` (`phx_join`, `sync:subscribe`, `sync:update_batch`, `sync:heartbeat`, replay/resume)

4. Control methods:
- `thread/start`
- `thread/resume`
- `turn/start`
- `turn/interrupt`
- `thread/list`
- `thread/read`

5. Event semantics:
- receipts: `worker.response`, `worker.error`
- handshake: `ios/handshake`, `desktop/handshake_ack`
- codex events: `thread/started`, `turn/*`, `error`, `item/*`, delta methods

## Execution issues

1. `OA-IOS-WGPUI-CODEX-001`: migration program + invariant anchoring
2. `OA-IOS-WGPUI-CODEX-002`: port domain/state machine to Rust shared crates
3. `OA-IOS-WGPUI-CODEX-003`: WGPUI conversation surface
4. `OA-IOS-WGPUI-CODEX-004`: WGPUI auth/worker/handshake/operator surfaces
5. `OA-IOS-WGPUI-CODEX-005`: Rust HTTP + Khala orchestration
6. `OA-IOS-WGPUI-CODEX-006`: Rust control request lifecycle and receipts
7. `OA-IOS-WGPUI-CODEX-007`: iOS host bridge for direct WGPUI input/focus
8. `OA-IOS-WGPUI-CODEX-008`: cutover/remove SwiftUI product surfaces
9. `OA-IOS-WGPUI-CODEX-009`: CI and verification gates

## Definition of done

1. All issues above are closed with verification evidence.
2. iOS Codex product UI/state flow is Rust/WGPUI-owned.
3. No SwiftUI product UI remains in the iOS Codex production path.
4. Real-device runbook passes for login, worker load, handshake, message turn, interrupt, reconnect/resume.
