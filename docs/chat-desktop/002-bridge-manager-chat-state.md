# Issue #2: Extend BridgeManager with iOS Chat State Properties for macOS

## Phase
Phase 1: Core Infrastructure

## Priority
Critical - Required for chat functionality

## Description
Extend `BridgeManager` to include the iOS chat state properties (`updates`, `threads`, etc.) for the macOS target, enabling chat functionality on desktop. Reuse `TimelineStore` for update processing and unify the prompt dispatch path so macOS compiles the same code path as iOS.

## Current State
- `BridgeManager` has macOS-specific properties (`workingDirectory`, `connectedClientCount`)
- iOS has chat-specific properties (`updates`, `threads`, `availableCommands`)
- macOS doesn't track session updates or message history
- Platform-specific property separation prevents code reuse

## Target State
- Unified `BridgeManager` with all chat properties available on both platforms
- macOS tracks and renders chat messages via `updates` (mirrored from `TimelineStore`)
- Session management works identically on iOS and macOS
- Shared update processing logic across platforms via `TimelineStore`
- Shared prompt dispatching (generalize `PromptDispatcher` or provide `DesktopPromptDispatcher` with the same interface)

## Acceptance Criteria
- [ ] Add iOS chat properties to macOS BridgeManager build target (remove `#if os(iOS)` guards for chat state)
- [ ] Wire `TimelineStore` on macOS and mirror its publishers to `@Published` properties (`updates`, `availableCommands`, `currentMode`, `toolCallNames`, `rawJSONByCallId`, `outputJSONByCallId`)
- [ ] Provide a prompt dispatcher on macOS (generalize `PromptDispatcher` or add `DesktopPromptDispatcher`) and set it from `BridgeManager+Mac.start()`
- [ ] Add `threads`/recent sessions using existing Tinyvex history RPCs (`tinyvex/history.recentSessions`, `tinyvex/history.sessionTimeline`)
- [ ] Add `currentSessionId` tracking for macOS
- [ ] Add proper `@Published` property wrappers for SwiftUI reactivity
- [ ] Tests pass for both iOS and macOS targets

## Technical Details

### Files to Modify
- `ios/OpenAgents/Bridge/BridgeManager.swift`
- `ios/OpenAgents/Bridge/BridgeManager+Mac.swift`
- `ios/OpenAgents/Bridge/PromptDispatcher.swift` (widen platform guard) or new `DesktopPromptDispatcher.swift`

### Properties to Add/Unify
```swift
@MainActor
class BridgeManager: ObservableObject {
    // MARK: - Existing Properties (keep)
    @Published var status: Status = .disconnected
    @Published var workingDirectory: URL?
    @Published var connectedClientCount: Int = 0

    // MARK: - Chat State (add for macOS, already exists on iOS)
    @Published var updates: [ACP.Client.SessionNotificationWire] = []
    @Published var threads: [ThreadSummary] = []
    @Published var currentSessionId: ACPSessionId?
    @Published var availableCommands: [ACP.Client.AvailableCommand] = []
    @Published var toolCallNames: [String: String] = [:] // callId -> tool name
    @Published var outputJSONByCallId: [String: String] = [:] // For inspector

    // MARK: - Foundation Models (if not already present)
    @Published var conversationTitles: [String: String] = [:] // sessionId -> title
    @Published var conversationSummaries: [String: String] = [:] // sessionId -> summary
}
```

### Methods to Implement/Update
- Mirror iOS wiring in `BridgeManager+iOS.swift`:
  - Subscribe to a connection’s `notificationPublisher` (if using WS) OR wire the dispatcher to push updates into `TimelineStore` directly on macOS.
  - Mirror `TimelineStore` publishers to `@Published` fields (see iOS).
- Provide helpers for recent sessions and timeline loading via Tinyvex history RPCs (already defined on the server and used on iOS via `PromptDispatcher`).

### JSON-RPC Integration
- Continue to send `session/update` via `SessionUpdateHub` on the server (`DesktopWebSocketServer`).
- On macOS, prefer direct integration through `TimelineStore`/dispatcher rather than attempting to route updates back from server to UI via JSON‑RPC.

### Platform Considerations
- All properties should compile for both iOS and macOS targets
- No `#if os(macOS)` / `#if os(iOS)` needed for these properties
- Both platforms benefit from unified state management

## Dependencies
None - Core infrastructure change

## Blocked By
None

## Blocks
- Issue #5 (Main chat area - needs `updates` array)
- Issue #6 (Inspector pane - needs `outputJSONByCallId`)
- Issue #7 (Agent selector - needs `availableCommands`)
- Issue #11 (Chat integration - needs all methods)
- Issue #12 (Session management - needs `threads` and session methods)

## Estimated Complexity
Medium (2-4 hours)

## Testing Requirements
- [ ] Build succeeds for both iOS and macOS targets
- [ ] `handleSessionUpdate()` correctly populates `updates` array
- [ ] Tool call metadata extracted properly (`toolCallNames`, `outputJSONByCallId`)
- [ ] Session management methods work (new session, load session)
- [ ] Existing iOS chat functionality not broken
- [ ] Unit tests for update processing logic

## References
- Current iOS BridgeManager: `ios/OpenAgents/Bridge/BridgeManager.swift`
- Update types: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`
- JSON-RPC server: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`
- Timeline processing: `ios/OpenAgents/Bridge/TimelineStore.swift`
