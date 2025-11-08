# Logging Guidelines

OpenAgents uses `os.Logger` via a thin wrapper `OpenAgentsLog` to unify categories and enable DEBUG-gated verbosity.

## Categories

- `app` — app lifecycle, initialization, state management
- `ui` — SwiftUI views and interactions
- `ui.timeline` — timeline rendering, diffs, performance hints
- `bridge` — bridge coordination (umbrella)
- `bridge.server` — DesktopWebSocketServer: lifecycle, JSON-RPC routing, session updates
- `bridge.client` — MobileWebSocketClient: connection, handshake, JSON-RPC traffic
- `orchestration` — session orchestration and tools
- `acp` — ACP encode/decode, updates, wire contract behavior

Use the most specific category that fits. Example:

```swift
OpenAgentsLog.bridgeServer.info("Listening on port \(port)")
OpenAgentsLog.bridgeClient.debug("<- notify method=\(method) bytes=\(payload.count)")
OpenAgentsLog.acp.error("Failed to encode SessionUpdate: \(error)")
OpenAgentsLog.uiTimeline.notice("Diff applied: items=\(count)")
```

## DEBUG Gating

For expensive logs or pretty prints, gate on `OpenAgentsLog.isDebugLoggingEnabled`:

```swift
if OpenAgentsLog.isDebugLoggingEnabled {
    let pp = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys])
    if let pp, let s = String(data: pp, encoding: .utf8) {
        OpenAgentsLog.bridgeClient.debug("pretty params:\n\(s, privacy: .public)")
    }
}
```

## Privacy Annotations

Always add `privacy: .private` for sensitive values (paths, tokens, user input). Use `.public` only when safe.

