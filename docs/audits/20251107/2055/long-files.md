# Long Files & Refactor Targets

The following files exceed reasonable size for maintainability. Propose modularization per responsibility boundaries and ADRs.

## DesktopWebSocketServer.swift — 1,620 lines

- Path: ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift
- Issues: Monolithic server with transport, routing, history/Tinyvex, and session update broadcasting intertwined; contains deprecated code path at 313.
- Actions:
  - Extract: `Transport` (NWListener lifecycle), `JsonRpcRouter`, `HistoryApi` (Tinyvex), `SessionUpdateHub`, `BonjourService`.
  - Unit tests per module; integration tests for end‑to‑end bridge.
  - Remove deprecated raw JSONL hydrate.

## ExploreOrchestrator.swift — 1,150 lines

- Path: ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift
- Issues: Overgrown orchestrator; likely contains multiple flows (prompting, tool runs, retries, recovery).
- Actions:
  - Split into reducers/state machine components (planning, tool exec, error recovery).
  - Move data types to OrchestrationTypes.swift if not already isolated.

## BridgeManager.swift — 576 lines

- Path: ios/OpenAgents/Bridge/BridgeManager.swift
- Issues: UI‑facing bridge manager mixing connection state, prompts, and timeline updates.
- Actions:
  - Extract `ConnectionManager` (WebSocket client wrapper), `PromptDispatcher`, `TimelineStore`.
  - Define protocols for testability; inject into views via environment.

## SimplifiedIOSView.swift — 557 lines; SimplifiedMacOSView.swift — 432 lines

- Paths: ios/OpenAgents/SimplifiedIOSView.swift, ios/OpenAgents/SimplifiedMacOSView.swift
- Issues: Complex SwiftUI views; hard to reason about.
- Actions: Extract subviews (composer, message list, tool call rows, error banners) and view models.

## ChatHomeView.swift — 457 lines; HistorySidebar.swift — 388 lines

- Paths: ios/OpenAgents/Views/ChatHomeView.swift, ios/OpenAgents/HistorySidebar.swift
- Issues: Mixed navigation/composition/state.
- Actions: Extract navigation state, list item rows, and filtering/search components.

## MobileWebSocketClient.swift — 404 lines

- Path: ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift
- Issues: Connection management + JSON‑RPC handling + retries combined.
- Actions: Extract `ReconnectPolicy`, `RequestManager`, and decode utilities.

Notes:

- Keep file names aligned with primary type per repo conventions.
- Add unit tests when splitting to preserve behavior.

