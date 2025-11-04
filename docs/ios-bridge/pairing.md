# Pairing (iOS ↔ Desktop Bridge)

This document describes how the iOS app finds and pairs with the desktop WebSocket bridge and how tokens are exchanged.

## TL;DR

- Desktop app (macOS) automatically starts a WebSocket bridge on launch at `ws://0.0.0.0:9099`.
- Discovery is behind a feature flag; by default, iOS uses Manual Connect (enter the desktop LAN IP + `9099`).
- When multicast is approved and enabled, the app advertises `_openagents._tcp` and the iOS app browses and connects automatically.
- Handshake uses a pre‑shared token (dev default: `OA_DEV_TOKEN`).

## Why Bonjour

We want a zero‑config, LAN‑first pairing story. Bonjour (mDNS) lets iOS discover the desktop without manual IP entry. Until Apple approves the multicast entitlement for the app, discovery is disabled by default and Manual Connect is the recommended path. In constrained or remote setups, we’ll also support Tailscale (VPN) or manual host entry.

## Handshake

1. iOS → Desktop: `{"type":"Hello","token":"<token>"}` over WebSocket.
2. Desktop verifies token; responds: `{"type":"HelloAck","token":"<token>"}`.
3. Connection established; subsequent messages are sent as envelopes `{type,data}` (reserved for future sync controls).

Notes:
- For compatibility, the desktop accepts `{"token":"…"}` without a `type` field.
- The iOS client accepts HelloAck as a text or data frame.

## Tokens

- Dev builds use a static token `OA_DEV_TOKEN` (see `BridgeConfig`).
- Production should rotate to a user‑generated secret per desktop install.
- Storage
  - macOS: Keychain or user defaults (dev only).
  - iOS: Keychain.
- Distribution
  - Initially Bonjour only; future: QR code flow to transfer `{host,port,token}` securely.

## Failure Modes

- Different Wi‑Fi networks (guest vs main): Bonjour discovery fails → use Tailscale or manual host.
- Firewalls: block port 9099 inbound on macOS → allow or change port.
- Token mismatch: Desktop closes connection; verify the token.

## Roadmap

- Token QR pairing (desktop shows, iOS scans; stored in Keychain).
- Tailscale discovery (MagicDNS name) and `wss://` hardening.
- Bridge envelopes for Tinyvex/ACP sync controls.

## Developer Notes

- Service type: `_openagents._tcp`
- Default port: `9099`
- Code paths
  - Desktop server: `ios/OpenAgentsCore/Sources/DesktopBridge/DesktopWebSocketServer.swift`
  - iOS client: `ios/OpenAgentsCore/Sources/MobileBridge/MobileWebSocketClient.swift`
  - Bridge config: `ios/OpenAgentsCore/Sources/Bridge/BridgeConfig.swift`
  - App wiring: `ios/OpenAgents/Bridge/BridgeManager.swift`, `ios/OpenAgents/OpenAgentsApp.swift`
