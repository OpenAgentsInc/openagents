# iOS Codex-First Structure (Rust Control-Service Model)

## Scope

Define the current iOS architecture for Codex administration with Rust control/runtime backends.

## Boundary rules

1. iOS uses control-service public APIs only.
2. iOS never calls runtime internal `/internal/v1/*` endpoints directly.
3. Live updates come from Khala websocket subscriptions.
4. Authority writes happen through authenticated HTTP APIs.
5. Product UI is Rust/WGPUI-only; Swift/SwiftUI is host bridge code only (no product chat/thread UI logic in Swift views).

## Modules

1. `AuthClient`: sign-in, refresh, session state.
2. `CodexWorkerClient`: list worker, snapshot, request/stop actions.
3. `KhalaClient`: ws connect/reconnect/watermark resume.
4. `ChatStateStore`: ordered event application + dedupe by `(topic, seq)`.
5. `UI`: chat + debug surfaces using shared Rust client-core models.

## Default environment

1. Base URL defaults to `https://openagents.com`.
2. No manual endpoint/token entry in normal UX.

## Required tests

1. Auth flow smoke (real device).
2. Worker list/snapshot load.
3. Stream reconnect/resume with no duplicate side effects.
4. Guardrails + parity lanes: `./scripts/local-ci.sh ios-codex-wgpui`.
