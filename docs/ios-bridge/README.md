# iOS ↔︎ Desktop WebSocket Bridge

This document explains the WebSocket‑only bridge that lets the iOS app control and mirror the desktop app (which owns filesystem access and runs agent CLIs). The desktop acts as a WebSocket server; the iOS app is a WebSocket client.

## Design Goals

- Real‑time updates (single persistent WebSocket)
- JSON-RPC 2.0 for all communication
- LAN‑first connectivity with Bonjour discovery (behind feature flag)
- No third‑party dependencies

## Overview

- Desktop (macOS): runs `DesktopWebSocketServer` and exposes a `ws://` endpoint on port 9099
- Mobile (iOS): uses `MobileWebSocketClient` to connect via Bonjour discovery or manual host entry
- Transport: JSON-RPC 2.0 over WebSocket with ACP method names
- Discovery: Bonjour `_openagents._tcp` service (requires multicast entitlement, currently behind feature flag)

## Protocol

All messages are **JSON-RPC 2.0** formatted and framed over WebSocket.

### Handshake (Initialize)

After WebSocket connection, the iOS client sends an `initialize` request:

**Request:**
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

**Response:**
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

### Available Methods

All methods follow JSON-RPC 2.0 format. See `rpc.swift` for complete list:

- `initialize` - Handshake (above)
- `session/new` - Create new agent session
- `session/prompt` - Send prompt to session
- `session/cancel` - Cancel running session (notification)
- `session/update` - Agent update stream (notification from server)
- `session/set_mode` - Set agent provider (Codex/Claude Code)
- `tinyvex/history.recentSessions` - Query session history
- `tinyvex/history.sessionTimeline` - Load session timeline
- `fs/read_text_file`, `fs/write_text_file` - File system (client-handled)
- `terminal/run` - Terminal execution (client-handled)

### Example: Start Session

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/new",
  "id": "2",
  "params": {}
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": "2",
  "result": {
    "session_id": "abc-123-def-456"
  }
}
```

### Example: Send Prompt

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/prompt",
  "id": "3",
  "params": {
    "session_id": "abc-123-def-456",
    "prompt": "List files in current directory"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": "3",
  "result": {}
}
```

### Example: Session Updates (Notification)

The server sends `session/update` notifications as the agent generates output:

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "session_id": "abc-123-def-456",
    "update": {
      "agent_message_chunk": {
        "content": {
          "text": {
            "text": "Let me list the files..."
          }
        }
      }
    }
  }
}
```

**Note:** Notifications have no `id` field and expect no response.

## Desktop Server (macOS)

Entry: `DesktopWebSocketServer` (Network.framework).

Responsibilities:

- Listen on TCP port 9099 and upgrade to WebSocket
- On new connection, wait for JSON-RPC `initialize` request
- Respond with `InitializeResponse` to complete handshake
- Route JSON-RPC requests to registered method handlers via `JsonRpcRouter`
- Send `session/update` notifications as agent generates output

The server uses `JsonRpcRouter` for method dispatch. Handlers are registered in `registerHandlers()`:

```swift
router.register(method: "session/new") { id, params, rawDict in
    // Create new session
    let sessionId = ACPSessionId(UUID().uuidString)
    let result = ACP.Agent.SessionNewResponse(session_id: sessionId)
    JsonRpcRouter.sendResponse(id: id, result: result) { text in
        client.send(text: text)
    }
}
```

## iOS Client

Entry: `MobileWebSocketClient` (URLSessionWebSocketTask).

Responsibilities:

- Connect to `ws://<host>:<port>` via Bonjour discovery or manual entry
- Send JSON-RPC `initialize` request immediately after connection
- Wait for `InitializeResponse` and only then mark connection as established
- Maintain receive loop for JSON-RPC notifications and responses
- Handle `session/update` notifications from server

Minimal usage:

```swift
import OpenAgentsCore

let client = MobileWebSocketClient()
client.delegate = self
client.connect(url: URL(string: "ws://192.168.1.10:9099")!)
```

Delegate example:

```swift
extension MyController: MobileWebSocketClientDelegate {
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient, workingDirectory: String?) {
        print("Connected! Working directory: \(workingDirectory ?? "nil")")
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCNotification method: String, payload: Data) {
        if method == "session/update" {
            // Handle session update notification
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        print("Disconnected: \(error?.localizedDescription ?? "no error")")
    }
}
```

## Discovery

Bonjour/mDNS discovery is available behind a feature flag. Until the iOS multicast entitlement is approved, discovery is disabled by default and the app uses Manual Connect.

**Current State:**
- Desktop advertises `_openagents._tcp` Bonjour service on port 9099
- iOS discovery requires multicast entitlement (currently behind feature flag)
- Default: Manual Connect via Bridge status chip (enter desktop LAN IP + port 9099)

**Enable Discovery:**
- Set `OPENAGENTS_ENABLE_MULTICAST=1` environment variable
- OR set `enable_multicast=true` in UserDefaults

See `docs/ios-bridge/discovery-and-permissions.md` for detailed permission requirements.

## Security

- **Transport:** Currently `ws://` on trusted LAN only
- **Authentication:** JSON-RPC `initialize` handshake validates protocol version
- **Future enhancements:**
  - Token-based pairing with QR code flow
  - TLS (`wss://`) with certificate pinning
  - Tailscale VPN for remote access (recommended for untrusted networks)

## Using Tailscale

Once both devices are in the same Tailscale network:

- Find the desktop's Tailscale IP (e.g., `100.x.y.z`)
- On iOS, use Manual Connect to enter `100.x.y.z:9099`
- Optionally, set up a stable MagicDNS name and use that instead of the IP
- Connection uses same JSON-RPC `initialize` handshake over Tailscale VPN tunnel

## Troubleshooting

- **Build errors referencing Network or `NWProtocolWebSocket`:**
  - Ensure macOS deployment target is macOS 12.0 or later
  - Desktop target should automatically link Network.framework

- **iOS cannot connect:**
  - Verify desktop server is running (check for "Started on ws://0.0.0.0:9099" in logs)
  - Try simulator first: automatically connects to `ws://127.0.0.1:9099`
  - On device: ensure both are on same LAN or connected via Tailscale
  - Check desktop logs for `initialize` request

- **Initialize handshake fails:**
  - Check protocol version mismatch (must be "0.2.x")
  - Verify JSON-RPC format (must have `"jsonrpc": "2.0"`)
  - Check client logs for "initialize ok; connected" message
  - Check server logs for "recv rpc request method=initialize"

- **JSON decoding errors:**
  - Enable verbose logging to see full JSON-RPC messages
  - Verify method names match `rpc.swift` constants
  - Check params structure matches ACP types

## Roadmap

- **Phase 1 (Complete):** JSON-RPC 2.0 over WebSocket, `initialize` handshake, session methods
- **Phase 2 (In Progress):** Agent provider registry (Codex/Claude Code), session mode selection
- **Future:**
  - Multicast entitlement approval for Bonjour discovery
  - Token-based pairing with QR code flow
  - TLS (`wss://`) and certificate pinning
  - Enhanced retry/backoff strategy
