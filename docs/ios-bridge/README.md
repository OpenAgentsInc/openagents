# iOS ↔︎ Desktop WebSocket Bridge

This document explains the WebSocket‑only bridge that lets the iOS app control and mirror the desktop app (which owns filesystem access and runs the Codex CLI). The desktop acts as a WebSocket server; the iOS app is a WebSocket client.

## Design Goals

- Real‑time updates (single persistent WebSocket)
- Simple token‑based authentication at handshake time
- LAN‑first connectivity with future support for Tailscale VPN
- No third‑party dependencies

## Overview

- Desktop (macOS): runs `DesktopWebSocketServer` and exposes a `ws://` endpoint on a configurable port.
- Mobile (iOS): uses `MobileWebSocketClient` to connect, authenticate, and exchange messages.
- Shared protocol: `BridgeMessages` defines Codable message types and helpers.

Future enhancements:

- Bonjour/mDNS for zero‑config discovery
- TLS (`wss://`) with local certificates
- QR‑code pairing to pass host:port and token

## Protocol

All messages are JSON‑encoded and framed over WebSocket.

Examples:

```json
{ "type": "Hello", "token": "<shared-secret>" }
```

```json
{ "type": "HelloAck", "token": "<shared-secret>" }
```

```json
{ "type": "Ping" }
```

```json
{ "type": "Pong" }

History list (Phase 1):

```json
{ "type": "threads.list.request", "data": { "topK": 20 } }
```

```json
{ "type": "threads.list.response", "data": { "items": [ { "id": "…", "title": "…", "source": "codex", "updated_at": 123, "last_message_ts": 123 } ] } }
```
```

Notes:

- `token` is a pre‑shared secret stored locally on both devices (see Security below).
- An envelope `{type,data}` is available (`WebSocketMessage.Envelope`) if/when we need to carry structured payloads.
  - Implemented messages: `threads.list.request/response` (desktop scans Codex history and returns summaries).

## Desktop Server (macOS)

Entry: `DesktopWebSocketServer` (Network.framework).

Responsibilities:

- Listen on a TCP port and upgrade to WebSocket.
- On new connection, wait for `Hello`.
- Verify token; reply `HelloAck` or close the connection.
- Start a receive loop for subsequent messages (e.g., ping/pong, future sync messages).

Minimal usage:

```swift
#if os(macOS)
import DesktopBridge

let server = DesktopWebSocketServer(token: "YOUR_TOKEN")
try server.start(port: 9099)
print("Server running on ws://127.0.0.1:9099")
#endif
```

## iOS Client

Entry: `MobileWebSocketClient` (URLSessionWebSocketTask).

Responsibilities:

- Connect to `ws://<host>:<port>`.
- Send `Hello { token }` immediately.
- Wait for `HelloAck` and only then mark the connection as established.
- Maintain a receive loop for updates.
- Composer: The iOS UI currently defaults to a messages‑only view (no input bar) when connected to a desktop bridge. Prompt entry is disabled by design for this workflow.

Minimal usage:

```swift
import MobileBridge

let client = MobileWebSocketClient()
client.delegate = self
client.connect(url: URL(string: "ws://192.168.1.10:9099")!, token: "YOUR_TOKEN")
```

Delegate example:

```swift
extension MyController: MobileWebSocketClientDelegate {
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        print("connected")
    }
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage) {
        // handle envelope
    }
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        // show error
    }
}
```

## Discovery (future work)
Bonjour/mDNS discovery is available behind a feature flag. Until the iOS multicast entitlement is approved for the app, discovery is disabled by default and the app uses Manual Connect instead.

- Enable discovery: set `OPENAGENTS_ENABLE_MULTICAST=1` (or `enable_multicast=true` in `UserDefaults`).
- Otherwise, use Manual Connect from the Bridge status chip and enter the desktop’s LAN IP and port `9099`.

See `docs/ios-bridge/discovery-and-permissions.md` for permissions and the fallback.

## Security

- Token: use a random, high‑entropy token stored in Keychain on iOS and in a secure local store on macOS. Consider a pairing flow where the desktop shows a QR code and the iOS app scans it to capture host:port + token.
- Transport: Start with `ws://` on trusted LAN. For untrusted networks or remote access, use:
  - Tailscale (recommended): connect via the Tailscale IP and keep `ws://` inside the VPN.
  - TLS: move to `wss://` with a certificate (self‑signed or managed). This requires certificate provisioning and pinning.
- Authorization: Reject state‑changing messages until handshake success. Consider refreshing the token periodically.

## Using Tailscale

Once both devices are in the same Tailscale network:

- Find the desktop’s Tailscale IP (e.g., `100.x.y.z`).
- On iOS, connect to `ws://100.x.y.z:9099` using the same token.
- Optionally, set up a stable MagicDNS name and use that instead of the IP.

## Troubleshooting

- Build errors referencing Network or `NWProtocolWebSocket`:
  - Ensure the macOS deployment target is macOS 12.0 or later.
  - Make sure the Desktop target links Network.framework (usually automatic).
- iOS cannot connect:
  - Verify the desktop server is running and reachable (try `ws://127.0.0.1:9099` locally first).
  - On device, ensure both are on the same LAN or connected via Tailscale.
  - Check that the token matches.
- JSON decoding errors:
  - Log incoming/outgoing frames. Ensure `type` strings and payload shapes match the protocol.

## Roadmap

- Bonjour‑based discovery
- TLS (`wss://`) and certificate pinning
- Thread/message synchronization messages (list, diff, post)
- QR‑based pairing and token rotation
- Backoff/retry strategy and reachability integration
