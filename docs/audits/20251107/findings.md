# Detailed Findings

This document organizes code audit findings by area with concrete examples and file references.

## Code Structure & Size

- Oversized Swift files increase change risk and reduce readability:
  - `ios/OpenAgents/AcpThreadView.swift:1` (~1,759 lines): Combines view, state, timeline building, JSON transforms, and custom subcomponents (`ToolCallCell`).
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift:1` (~1,464 lines): Server, handshake, JSON-RPC routing, process launching, tailing, and thread listing in one class.
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift:1` (~1,150 lines): Mixed planning, tool execution, FM session management, and streaming logic.

Recommendations
- Split these into smaller units by responsibility:
  - `AcpThreadView.swift` → `AcpThreadViewModel` (pure timeline + state), `AcpThreadView` (UI only), reuse `ACP/Renderers/ToolCallView.swift` instead of inline `ToolCallCell`.
  - `DesktopWebSocketServer` → connection/handshake, JSON-RPC router, agent process/tailer, threads listing. Add unit tests per component.
  - `ExploreOrchestrator` → planning, execution, streaming/reporting—consider an internal protocol to decouple tool IO from FM logic.

## Duplication & Reuse

- Duplicated tool-call rendering logic:
  - `ios/OpenAgents/ACP/Renderers/ToolCallView.swift:1` provides a complete tool-call renderer.
  - `ios/OpenAgents/AcpThreadView.swift:1490` inlines a similar `ToolCallCell` with a duplicate `prettyShellCommand` implementation (`ios/OpenAgents/AcpThreadView.swift:1599`).
  - Recommendation: Remove inline `ToolCallCell` and reuse `ToolCallView`. Extract `prettyShellCommand` and command-array parsing into a shared helper under `OpenAgentsCore` so both call sites share identical behavior.

- JSON helper duplication inside a view file:
  - `jsonFromAnyEncodableObject` / `jsonFromAnyEncodable` / `jsonValueFromFoundation` live in `ios/OpenAgents/AcpThreadView.swift:1411`.
  - `AnyEncodable` already has `toJSONValue()` (`ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/agent.swift:140`).
  - Recommendation: Move reusable JSON conversion helpers into `OpenAgentsCore` and avoid defining them inside views.

- Minor duplicate imports:
  - `ios/OpenAgents/Bridge/BridgeManager.swift:1` imports `OpenAgentsCore` twice.

## Logging & Observability

- High volume of `print` statements across app+core sources (160+ occurrences), including hot UI paths and bridge server:
  - Examples:
    - `ios/OpenAgents/ContentView.swift:21`
    - `ios/OpenAgents/Bridge/BridgeManager.swift:190`
    - `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift:33, 417, 569, 741` (and many others)
    - `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:44, 68, ...`
  - Recommendation: Introduce a centralized `OpenAgentsLog` wrapper backed by `os.Logger`. Use categories (app, bridge, server, client, ui). Gate verbose logs (`debug`, `trace`) under DEBUG builds and keep info/warn minimal in release. Add privacy annotations for PII.

## Error Handling

- `fatalError` in app initialization:
  - `ios/OpenAgents/OpenAgentsApp.swift:27` on model container creation.
  - Recommendation: Prefer a user-visible error view and retry paths for non-critical storage initialization; `fatalError` may be acceptable for irrecoverable developer errors, but consider a softer failure in production builds.

- Bridge server error paths log and continue without structured errors:
  - `DesktopWebSocketServer` prints on failures to find CLI, run shells, or tail files.
  - Recommendation: Standardize error reporting via JSON-RPC error responses and make failure states observable via delegate and/or structured logs.

## Bridge Protocol & Security

- Current code follows ADR-0004 JSON-RPC `initialize` (no token). Docs still mention Hello/Ack with token.
  - Code: `DesktopWebSocketServer.handleTextMessage` handles `initialize` and proceeds without auth.
  - Docs: `docs/ios-bridge/README.md:1` describes token-based handshake.
  - Recommendation: Update docs to match ADR-0004 (JSON-RPC only) OR implement token pairing (QR) and enforce it at handshake.

- Default host is developer-specific:
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeConfig.swift:5` → `defaultHost = "192.168.1.11"`.
  - Recommendation: Use last-known endpoint, simulator loopback for dev, and empty default otherwise.

## Concurrency & State

- Manual `DispatchQueue` usage across server/client could be modernized:
  - `DesktopWebSocketServer` receive loop and timers on custom queues.
  - `AcpThreadView` uses background recompute for timeline but performs a lot of stateful logic in the view.
  - Recommendations:
    - Adopt `async/await` where feasible.
    - Consider an actor for server client/process state.
    - Move timeline compute into a non-UI type (view model or `OpenAgentsCore`) to ease testing and decouple UI.

## UI Composition

- `ios/OpenAgents/AcpThreadView.swift:1` contains view + logic + subviews and standalone helpers:
  - Recommendation: Extract long sections into dedicated subviews (`ThoughtsGroupView`, `MessageList`, `ToolCallsList`) and a view model (`@StateObject`) handling the timeline. Keep rendering small.

- `SimplifiedIOSView.swift` and `SimplifiedMacOSView.swift` are fairly large; further composition into smaller views will improve testability and reuse.

## Tests

- Strong test suite in `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/` and UI/integration tests under `ios/OpenAgentsTests/` and `ios/OpenAgentsUITests/`.
- Opportunities:
  - Add unit tests for the extracted `prettyShellCommand` helper and command-array parsing (if moved to core).
  - Add tests around server JSON-RPC routing and error responses once modularized.
  - Consider performance tests for timeline recompute and large update streams.

## Docs & ADRs

- ADR-0004 (bridge) is aligned with code; `docs/ios-bridge/README.md` needs an update to reflect JSON-RPC `initialize` and the current discovery story (Bonjour behind a flag; manual connect default).
- Ensure `docs/ios-bridge/pairing.md` clearly delineates current reality (no auth) and planned pairing (token/QR or Tailscale guidance).

## Hygiene, TODOs & Build Artifacts

- TODO stubs present:
  - Voice input button action in `ios/OpenAgents/FloatingMicButton.swift:20`.
  - A regression test notes a TODO if client-side classification fix is implemented: `ios/OpenAgentsTests/MessageClassificationRegressionTests.swift:96`.

- Local build artifacts detected:
  - `ios/build/` exists locally. Ensure `ios/build/` is ignored by `.gitignore` to prevent accidental commits.

## Prioritized Recommendations

P0 (Now)
- Refactor `AcpThreadView` into view + view model + reuse existing `ToolCallView`.
- Extract `prettyShellCommand` and command parsing to core; delete duplicates.
- Replace `print` with centralized logging wrapper using `os.Logger` and DEBUG gating.
- Update default host handling; persist last-known; simulator loopback for dev.
- Update `docs/ios-bridge/README.md` to JSON-RPC initialize; align with ADR-0004.

P1 (Next)
- Modularize `DesktopWebSocketServer` and add unit tests for router/process/tailer.
- Migrate to async/await where feasible; consider actors for server state.
- Move JSON conversion helpers out of views into `OpenAgentsCore`.
- Add CI workflows (build + tests) and SwiftLint/SwiftFormat config.

P2 (Later)
- Implement token/QR pairing or formally recommend Tailscale-only usage until TLS/pinning lands.
- Improve orchestration error handling and reporting.

