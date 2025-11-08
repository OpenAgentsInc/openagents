# Pairing (iOS ↔ Desktop Bridge)

This document describes how the iOS app finds and pairs with the desktop WebSocket bridge.

## TL;DR

- Desktop app (macOS) automatically starts a WebSocket bridge on launch at `ws://0.0.0.0:9099`
- Discovery is behind a feature flag; by default, iOS uses Manual Connect (enter the desktop LAN IP + `9099`)
- When multicast is approved and enabled, the app advertises `_openagents._tcp` and iOS browses and connects automatically
- Handshake uses JSON-RPC 2.0 `initialize` request/response

## Why Bonjour

We want a zero‑config, LAN‑first pairing story. Bonjour (mDNS) lets iOS discover the desktop without manual IP entry. Until Apple approves the multicast entitlement for the app, discovery is disabled by default and Manual Connect is the recommended path. In constrained or remote setups, we also support Tailscale (VPN) or manual host entry.

## Handshake

The bridge uses JSON-RPC 2.0 for all communication, including the initial handshake:

**1. WebSocket Connection**
- iOS establishes WebSocket connection to `ws://<host>:9099`

**2. Initialize Request**
- iOS → Desktop: JSON-RPC `initialize` request

```json
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "id": "1",
  "params": {
    "protocol_version": "0.2.2",
    "client_capabilities": {},
    "client_info": {
      "name": "openagents-ios",
      "title": "OpenAgents iOS",
      "version": "0.1.0"
    }
  }
}
```

**3. Initialize Response**
- Desktop → iOS: JSON-RPC success response

```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "result": {
    "protocol_version": "0.2.2",
    "agent_capabilities": {},
    "auth_methods": [],
    "agent_info": {
      "name": "openagents-mac",
      "title": "OpenAgents macOS",
      "version": "0.1.0"
    },
    "_meta": {
      "working_directory": "/path/to/workspace"
    }
  }
}
```

**4. Connection Established**
- After successful `initialize` response, connection is ready
- iOS can now send `session/new`, `session/prompt`, etc.
- Desktop sends `session/update` notifications as agent generates output

## Authentication (Future)

Current implementation validates protocol version during `initialize` handshake. Future enhancements:

- **Token-based pairing:** QR code flow to securely transfer connection credentials
- **Keychain storage:** Persist paired desktop hosts in iOS Keychain
- **Token rotation:** Periodic refresh for long-lived connections

## Failure Modes

- **Different Wi‑Fi networks (guest vs main):** Bonjour discovery fails → use Tailscale or manual host entry
- **Firewalls:** Block port 9099 inbound on macOS → allow in System Settings or change port
- **Protocol version mismatch:** Desktop rejects `initialize` if version incompatible → update app
- **Handshake timeout:** iOS disconnects if no `initialize` response within timeout → check server is running

## Roadmap

**Current State:**
- JSON-RPC 2.0 `initialize` handshake ✅
- Manual Connect fallback ✅
- Bonjour discovery behind feature flag ✅

**Future:**
- Multicast entitlement approval for automatic Bonjour discovery
- Token-based pairing (desktop shows QR, iOS scans; stored in Keychain)
- Tailscale MagicDNS integration
- TLS (`wss://`) with certificate pinning

## Developer Notes

**Configuration:**
- Service type: `_openagents._tcp`
- Default port: `9099`
- Protocol version: `0.2.2`
- Handshake method: `initialize`

**Code Paths:**
- Desktop server: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`
- JSON-RPC router: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/JsonRpcRouter.swift`
- iOS client: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift`
- RPC method constants: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift`
- Bridge config: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeConfig.swift`
- App wiring: `ios/OpenAgents/Bridge/BridgeManager.swift`

**Key Types:**
- `ACP.Agent.InitializeRequest` - Client handshake request
- `ACP.Agent.InitializeResponse` - Server handshake response
- `JSONRPC.Request`, `JSONRPC.Response` - JSON-RPC 2.0 envelopes
- `ACPRPC` - Method name constants (e.g., `ACPRPC.initialize`, `ACPRPC.sessionNew`)
