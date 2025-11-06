# ADR 0006 — iOS ↔ Desktop WebSocket Bridge and Pairing

- Date: 2025-11-04
- Status: Accepted — Implemented (Phase 1)

## Context

We need a zero‑config, LAN‑first way for the iOS app to control/observe a desktop session that owns filesystem access and can launch provider CLIs. We already have a Rust bridge for Expo/Tauri; this ADR defines an Apple‑native, lightweight WebSocket bridge for the Swift app surfaces with simple discovery and a small, typed handshake.

Goals:
- No REST; a single persistent WebSocket connection.
- JSON‑RPC 2.0 with ACP method names as the primary transport.
- Bonjour/mDNS discovery so users don’t type LAN IPs.
- Optional token handshake retained as a legacy fallback; future QR pairing.

## Decision

Adopt an Apple‑native WebSocket bridge with Bonjour discovery and ACP over JSON‑RPC:

- Desktop (macOS)
  - Auto‑start `DesktopWebSocketServer` at app launch and advertise `_openagents._tcp`.
  - Speak JSON‑RPC 2.0; on first contact handle `initialize` and then accept `session/*` requests and send `session/update` notifications.
  - Accept `Hello`/`HelloAck` only as a fallback for legacy clients.

- Mobile (iOS)
  - Auto‑browse `_openagents._tcp` via `NetServiceBrowser` and connect to the first record.
  - Send `initialize` via JSON‑RPC; then use ACP methods (`session/new`, `session/prompt`, `session/cancel`) and consume `session/update` notifications.
  - Provide a simulator fallback to `ws://127.0.0.1:<port>`.

- Protocol (Phase 1)
  - JSON‑RPC 2.0 methods: `initialize`, `session/new`, `session/prompt`, `session/cancel` (notify), `session/update` (notify), `session/set_mode`.
  - Client‑handled services on macOS: `fs/read_text_file`, `fs/write_text_file`, `terminal/run`.
  - Legacy: `Hello/HelloAck`, `Ping`, `Pong` retained for compatibility only.

- Tokens
  - Dev default token supported for the legacy Hello path (see `BridgeConfig`).
  - Production token/QR pairing may be added; JSON‑RPC initialize remains the primary handshake.

## Rationale

- Zero‑config: Bonjour avoids manual IP entry; flows work out of the box on a LAN.
- Minimal moving parts: Network.framework on macOS and URLSessionWebSocketTask on iOS; no third‑party libs.
- Contract hygiene: small, typed handshake now; envelopes enable future controls without a new transport.

## Implementation

- SwiftPM module `OpenAgentsCore` contains:
  - `DesktopWebSocketServer` (Network.framework)
  - `MobileWebSocketClient` (URLSessionWebSocketTask)
  - `BridgeMessages`/`BridgeMessage` (envelope + Hello/Ping types)
  - `BridgeConfig` (service type, default port/token)
- App wiring:
  - `BridgeManager` starts the server on macOS and Bonjour‑discovers/connects on iOS; launched from `OpenAgentsApp`.
- Docs:
  - `docs/ios-bridge/README.md` — protocol and usage
  - `docs/ios-bridge/pairing.md` — pairing, tokens, discovery, failure modes

## Alternatives Considered

1) Reuse the Rust bridge directly
- Pros: one bridge for all surfaces.
- Cons: heavier dependency for the Swift app; extra packaging complexity for discovery/pairing.

2) HTTP control with polling
- Pros: simple to reason about.
- Cons: worse UX and battery; more moving parts; not aligned with our WS‑only policy.

## Consequences

- Additional Apple‑native bridge code to maintain (small footprint).
- Bonjour relies on LAN constraints; Tailscale/manual host needed in some environments.
- Token distribution for production requires a pairing flow; out of scope for Phase 1.

## Acceptance

- On macOS, launching the app starts a WS server and advertises `_openagents._tcp`.
- On iOS, launching the app discovers the desktop and connects automatically; JSON‑RPC `initialize` succeeds and streaming `session/update` is received.
- Simulator connects to `ws://127.0.0.1:9099`.
- No REST; only WebSocket.

## Follow‑ups

- Token QR pairing and persistence in Keychain.
- Tailscale/MagicDNS support and `wss://` with pinning.
- Define/enforce typed envelope messages for Tinyvex/ACP sync.
