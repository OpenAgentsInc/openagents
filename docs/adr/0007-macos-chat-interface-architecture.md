# ADR-0007: macOS Chat Interface Architecture

## Status
Accepted

## Context
OpenAgents v0.3 introduced a Swift-only architecture (ADR-0003) with ACP as the canonical runtime contract (ADR-0002) and a JSON‑RPC WebSocket bridge (ADR-0004). The macOS UI originally shipped with a dashboard view optimized for monitoring, not for conversational workflows. We needed a desktop chat experience with:
- Session history sidebar and keyboard navigation
- Main chat area with ACP timeline renderers (messages, plans, tools)
- Optional inspector for tool details and JSON outputs
- Seamless reuse of iOS chat logic and renderers

Constraints:
- macOS 13.0+ target, SwiftUI
- Preserve ACP/JSON‑RPC semantics
- Persist sessions to Tinyvex (SQLite)
- Do not depend on external CLIs for tests

## Decision
Adopt a two/three‑pane `NavigationSplitView` as the macOS root, reusing the shared chat state and ACP renderers. On macOS, use a local JSON‑RPC adapter against `DesktopWebSocketServer` to keep ACP/JSON‑RPC semantics without a socket.

Key elements:
- Root layout: `ChatMacOSView` wraps a `NavigationSplitView` with sidebar (sessions) + content (chat). Inspector (right) remains hidden initially.
- Shared chat state: `BridgeManager` + `TimelineStore` mirror updates into `@Published` fields; `PromptDispatcher` drives session/new, prompt, set_mode.
- Server integration (Option A): `DesktopWebSocketServer` publishes `(method, payload)` notifications via Combine, and `LocalJsonRpcClient` invokes server handlers directly.
- Persistence: `SessionUpdateHub` appends updates to Tinyvex and broadcasts JSON‑RPC `session/update` notifications.

## Alternatives Considered
1) WebSocket loopback on macOS
   - Pros: Identical transport to iOS
   - Cons: Unnecessary overhead and failure modes on a single‑process app
2) Direct server API usage in UI (no JSON‑RPC)
   - Pros: Fewer types
   - Cons: Diverges from ACP transport; duplicates logic; harder to share with iOS

We selected Option A (local JSON‑RPC) to preserve ACP/JSON‑RPC semantics and minimize divergence from iOS while avoiding socket overhead.

## Consequences
Positive:
- Shared code paths across iOS and macOS (TimelineStore, PromptDispatcher)
- Consistent ACP/JSON‑RPC behavior
- Simpler testing (register mock providers directly on server)

Negative:
- Two code paths for transport (socket vs local adapter) require discipline to keep parity
- The inspector is deferred; three‑pane UX will land subsequently

## Implementation Notes
- UI
  - `ios/OpenAgents/Views/macOS/ChatMacOSView.swift`
  - `ios/OpenAgents/Views/macOS/SessionSidebarView.swift`
  - `ios/OpenAgents/Views/macOS/ChatAreaView.swift`
  - `ios/OpenAgents/Views/macOS/ComposerMac.swift`
- Bridge/state
  - `ios/OpenAgents/Bridge/BridgeManager.swift`, `BridgeManager+Mac.swift`
  - `ios/OpenAgents/Bridge/TimelineStore.swift`
  - `ios/OpenAgents/Bridge/PromptDispatcher.swift`
  - `ios/OpenAgents/Bridge/ConnectionManager.swift` (DesktopConnectionManager)
  - `ios/OpenAgents/Bridge/LocalJsonRpcClient.swift`
- Server
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/SessionUpdateHub.swift`
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Local.swift`

Testing:
- `ios/OpenAgentsTests/LocalJsonRpcClientIntegrationTests.swift` verifies `session/new`, `session/set_mode`, prompt flow via a mock provider and Tinyvex history.

## Links
- Issues: docs/chat-desktop/001…015 series (foundation, bridge, composer, sidebar, chat area, agent selector, settings, developer, keyboard, theming, tests)
- ADR‑0002 (ACP), ADR‑0003 (Swift app), ADR‑0004 (Bridge), ADR‑0005 (Liquid Glass)

