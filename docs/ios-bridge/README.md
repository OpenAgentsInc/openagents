iOS ↔︎ Desktop WebSocket Bridge

This document explains the WebSocket-only bridge that lets the iOS app control and mirror the desktop app (which owns filesystem access and runs the Codex CLI). The desktop acts as a WebSocket server; the iOS app is a WebSocket client.

The design favors:
• Real-time updates (single persistent WebSocket)
• Simple token-based authentication at handshake time
• LAN-first connectivity with future support for Tailscale VPN
• No third-party dependencies

Overview

• Desktop (macOS): runs DesktopWebSocketServer and exposes a ws:// endpoint on a configurable port.
• Mobile (iOS): uses MobileWebSocketClient to connect, authenticate, and exchange messages.
• Shared protocol: BridgeMessages defines Codable message types and helpers.

Future enhancements:
• Bonjour/mDNS for zero-config discovery
• TLS (wss://) with local certificates
• QR-code pairing to pass host:port and token

Protocol

All messages are JSON-encoded and framed over WebSocket. The current minimal set:

Hello

{ "kind": "hello", "version": 1, "token": "<shared-secret>" }

HelloAck

{ "kind": "helloAck", "version": 1, "ok": true }

Ping

{ "kind": "ping", "nonce": "abcd-1234" }

Pong

{ "kind": "pong", "nonce": "abcd-1234" }

Notes:
• version enables protocol evolution. Both sides should check and allow backward-compatible changes.
• token is a pre-shared secret stored locally on both devices (see Security below).

Desktop server (macOS)

Entry point: DesktopWebSocketServer (Network.framework).

Responsibilities:
• Listen on a TCP port and upgrade to WebSocket.
• On new connection, wait for Hello.
• Verify token; reply HelloAck { ok: true } or close the connection.
• Start a receive loop for subsequent messages (e.g., ping/pong, thread sync messages in the future).

Minimal usage sketch:

```
#if os(macOS)
import DesktopBridge

let server = DesktopWebSocketServer(token: "YOUR_TOKEN")
try server.start(port: 9099)
print("Server running on ws://127.0.0.1:9099")
#endif
```

iOS client

Entry point: MobileWebSocketClient (URLSessionWebSocketTask).

Responsibilities:
• Connect to the server URL (ws://<host>:<port>).
• Send Hello { token, version } immediately.
• Wait for HelloAck and only then mark the connection as established.
• Maintain a receive loop for updates.

Minimal usage sketch:

```
import MobileBridge

let client = MobileWebSocketClient()
client.delegate = self
client.connect(url: URL(string: "ws://192.168.1.10:9099")!, token: "YOUR_TOKEN")
```

Delegate example:

```
extension MyController: MobileWebSocketClientDelegate {
    func bridgeClient(_ client: MobileWebSocketClient, didChangeState state: MobileWebSocketClient.State) {
        print("state: \(state)")
    }
    func bridgeClient(_ client: MobileWebSocketClient, didReceive event: BridgeEvent) {
        // handle messages
    }
    func bridgeClient(_ client: MobileWebSocketClient, didFail error: Error) {
        // show error
    }
}
```

Discovery (future work)

Use Bonjour/mDNS to advertise a service like _codexd._tcp. with a TXT record containing { version: 1 }. The iOS app can browse for this service and resolve to host:port, then connect via WebSocket.

Security

• Token: use a random, high-entropy token stored in Keychain on iOS and in a secure local store on macOS. Consider a pairing flow where the desktop shows a QR code and the iOS app scans it to capture host:port + token.
• Transport: Start with ws:// on trusted LAN. For untrusted networks or remote access, use:
   • Tailscale (recommended): connect via the Tailscale IP and keep ws:// inside the VPN.
   • TLS: move to wss:// with a certificate (self-signed or managed). This requires certificate provisioning and pinning.
• Authorization: All state-changing messages should be rejected until handshake success. Consider refreshing the token periodically.

Using Tailscale

Once both devices are in the same Tailscale network:
• Find the desktop’s Tailscale IP (e.g., 100.x.y.z).
• On iOS, connect to ws://100.x.y.z:9099 using the same token.
• Optionally, set up a stable MagicDNS name and use that instead of the IP.

Tests

The project includes Swift Testing tests for the handshake flow on macOS.

• File: Tests/BridgeTests/BridgeHandshakeTests.swift
• Positive case: client connects with the correct token and receives HelloAck.
• Negative case: wrong token leads to failed handshake or disconnect.

Run tests in Xcode: Product → Test (ensure a macOS destination).

Troubleshooting

• Build errors referencing Network or NWProtocolWebSocket:
   • Ensure your macOS deployment target is macOS 12.0 or later.
   • Make sure the Desktop target links Network.framework (usually automatic).
• iOS cannot connect:
   • Verify the desktop server is running and reachable (try ws://127.0.0.1:9099 locally first).
   • On device, ensure both are on the same LAN or connected via Tailscale.
   • Check that the token matches.
• JSON decoding errors:
   • Log incoming/outgoing frames. Ensure kind strings and payload shapes match the protocol.

Roadmap

• Bonjour-based discovery
• TLS (wss://) and certificate pinning
• Thread/message synchronization messages (list, diff, post)
• QR-based pairing and token rotation
• Backoff/retry strategy and reachability integration
