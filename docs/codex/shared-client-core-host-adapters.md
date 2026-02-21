# Shared Client Core Host Adapters

Status: Active
Owner: OA-RUST migration lane
Related issues: OA-RUST-054 (`#1869`), OA-RUST-055 (`#1870`)

## Goal

Define the shared Rust client core contract used by web, desktop, and iOS surfaces so auth/session, command transport, and Khala replay behavior are implemented once.

## Shared crate

Source of truth lives in `crates/openagents-client-core/`.

Modules:
- `auth`: input normalization + auth/session transport interfaces.
- `command`: command input normalization + command transport interface.
- `codex_worker`: worker-event handshake envelope decoding shared by desktop + iOS bridge.
- `ffi`: C-ABI bridge exports for iOS host integration.
- `khala_protocol`: Phoenix/Khala frame parsing, update/error decoding, watermark and replay helpers.
- `sync_persistence`: persisted topic watermark schema + migration codec.

## Host adapter responsibilities

### Web (`apps/openagents.com/web-shell`)
- Use `openagents-client-core::auth` normalization before issuing auth challenge/verify requests.
- Use `openagents-client-core::command` normalization before thread-message commands.
- Use `openagents-client-core::khala_protocol` for WS frame decode and update/error application.
- Use `openagents-client-core::sync_persistence` for local storage encode/decode/migrations.

### Desktop (`apps/autopilot-desktop`)
- Reuse `openagents-client-core::khala_protocol` for WS frame build/parse and replay cursor behavior.
- Reuse `openagents-client-core::auth` normalization in runtime auth CLI + token flow.
- Keep desktop-specific session routing/handshake dispatch in desktop app layer.

### iOS (`apps/autopilot-ios`)
- Swift host consumes shared Rust client core through `RustClientCoreBridge.swift` and C-ABI bridge symbols from `openagents-client-core`.
- Rust-owned on iOS path: auth/code normalization, command text normalization, desktop handshake-ack extraction, Khala frame parsing.
- Artifact build script: `apps/autopilot-ios/scripts/build-rust-client-core.sh`.
- iOS host must own only platform concerns (lifecycle, backgrounding hooks, push/notifications, secure keychain bridge) while auth/sync/business rules move into Rust core.

## Invariants

1. No per-surface divergence in Khala frame/replay semantics.
2. No per-surface divergence in auth/code normalization semantics.
3. Persisted sync schema migrations are defined once in Rust and reused by all surfaces.
4. Surface code may adapt storage/network primitives, but not reinterpret protocol semantics.

## Verification gates

Minimum checks for any adapter change:
- `cargo test -p openagents-client-core`
- `cargo test -p openagents-web-shell`
- `cargo test -p autopilot-desktop runtime_codex_proto`
- `xcodebuild -project apps/autopilot-ios/Autopilot/Autopilot.xcodeproj -scheme Autopilot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test`
