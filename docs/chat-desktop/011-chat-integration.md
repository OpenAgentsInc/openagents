# Issue #11: Wire Chat Components to BridgeManager (Send/Receive Messages)

## Phase
Phase 4: Integration & Features

## Priority
Critical - Core functionality

## Description
Connect all chat UI components to `BridgeManager` using the shared prompt dispatch/timeline path (as on iOS). Avoid bespoke WS plumbing in views; let `PromptDispatcher` and `TimelineStore` handle JSON‑RPC and update shaping.

## Current State
- UI components exist (chat area, composer, message renderers)
- BridgeManager has chat state properties
- JSON-RPC methods defined but not fully integrated
- No complete message flow from input → bridge → display

## Target State
- User types message in composer → sends via BridgeManager → appears in chat
- Desktop agent sends updates → BridgeManager receives → updates UI
- Tool calls, plans, reasoning blocks render correctly
- Real-time updates with proper state management
- Error handling for failed sends/receives
- Loading states during agent processing

## Status
Completed (implemented on main)

What shipped
- Composer wired to `BridgeManager.sendPrompt(text:)` convenience (shared dispatcher path).
- `TimelineStore` mirrors `session/update` notifications to `BridgeManager.updates` (already in place).
- ChatAreaView renders updates, including tool call status and reasoning blocks.
- Plan updates render via `PlanView` (entries).
- Loading indicator shows when agent is processing (based on latest update).
- Auto-scrolls to bottom on new messages.
- Session history and timeline load via Tinyvex history RPCs.
- Conversation titles generated on-demand via `ConversationSummarizer` and stored in `BridgeManager.conversationTitles`.

## Acceptance Criteria
- [x] Composer sends messages via `BridgeManager.sendPrompt(text:)`
- [x] `TimelineStore` receives `session/update` and mirrors to `BridgeManager.updates`
- [x] Chat area reactively renders new updates
- [x] Tool calls update status (running → complete/error)
- [x] Plans update step-by-step progress (rendered as entries)
- [x] Thinking blocks render correctly
- [x] Error messages display in chat (tool update error state)
- [x] Loading indicator shows during agent processing
- [x] Auto-scroll on new messages
- [x] Session history/timeline loads via Tinyvex history RPCs

## Technical Details

### BridgeManager Methods (shared pattern)

```swift
// Use the shared dispatcher + timeline like iOS
extension BridgeManager {
    func sendPrompt(text: String, desiredMode: ACPSessionModeId? = nil) {
        dispatcher?.sendPrompt(text: text, desiredMode: desiredMode, getSessionId: { self.currentSessionId }, setSessionId: { self.currentSessionId = $0 })
    }

    func fetchRecentSessions() { dispatcher?.fetchRecentSessions { [weak self] in self?.recentSessions = $0 } }

    func loadSessionTimeline(sessionId: String) {
        currentSessionId = nil
        timeline.clearAll()
        dispatcher?.loadSessionTimeline(sessionId: sessionId) { [weak self] arr in
            self?.timeline.replaceAll(with: arr)
        }
    }

    // Title generation using ConversationSummarizer (OpenAgentsCore)
    func generateConversationTitleIfNeeded() {
        guard let sid = currentSessionId, conversationTitles[sid] == nil else { return }
        let msgs: [ACPMessage] = ACPTimelineViewModel.fromSessionNotifications(updates).messages
        Task { @MainActor in
            let title = await ConversationSummarizer.summarizeTitle(messages: msgs)
            conversationTitles[sid] = title
        }
    }
}
```

Notes on server integration

- Continue to rely on `DesktopWebSocketServer` + `SessionUpdateHub` to translate and persist ACP updates.
- Client (macOS UI) should not call server internals directly; use the shared dispatcher or a macOS‑specific dispatcher that targets `AgentRegistry`/`SessionUpdateHub`.

### Composer Integration
```swift
// In ComposerMac.swift or ChatAreaView
private func sendMessage() {
    guard !messageText.isEmpty else { return }
    bridgeManager.sendPrompt(text: messageText)
    messageText = ""
}
```

### Real-time Updates
Make sure `ChatAreaView` observes `bridgeManager.updates`:

```swift
struct ChatAreaView: View {
    @EnvironmentObject var bridgeManager: BridgeManager

    var body: some View {
        // ...
        ForEach(bridgeManager.updates.indices, id: \.self) { index in
            MessageBubbleView(
                update: bridgeManager.updates[index],
                bridgeManager: bridgeManager
            )
        }
        // ...
    }
}
```

### Session Persistence
- Use existing Tinyvex history RPCs exposed by the server:
  - `tinyvex/history.recentSessions` → recent list
  - `tinyvex/history.sessionTimeline` → full timeline

### Loading States
```swift
// In ChatAreaView
@State private var isAgentProcessing = false

var body: some View {
    // ...
    if isAgentProcessing {
        HStack {
            ProgressView()
                .scaleEffect(0.7)
            Text("Agent is thinking...")
                .font(OAFonts.mono(size: 13))
                .foregroundColor(OATheme.Colors.textSecondary)
        }
        .padding()
    }
    // ...
}

// Detect agent processing from latest update
.onChange(of: bridgeManager.updates) { updates in
    guard let lastUpdate = updates.last else {
        isAgentProcessing = false
        return
    }

    // If last update is from user, agent is processing
    isAgentProcessing = lastUpdate.role == .user
}
```

## Dependencies
- Issue #2 (BridgeManager chat state)
- Issue #3 (Composer)
- Issue #5 (Chat area)

## Blocked By
- Issue #2
- Issue #3
- Issue #5

## Blocks
None - This completes the core chat functionality

## Estimated Complexity
High (6-8 hours)

## Testing Requirements
- [ ] Build succeeds for macOS target
- [ ] User can send messages
- [ ] Messages appear in chat immediately (optimistic update)
- [ ] Agent responses appear in real-time
- [ ] Tool calls render and update status
- [ ] Plans render with step progress
- [ ] Thinking blocks render correctly
- [ ] Errors display properly in UI
- [ ] Sessions persist and reload correctly
- [ ] Conversation titles generate automatically
- [ ] Loading states show during processing
- [ ] Multiple sessions can be created and switched
- [ ] Integration test: full conversation flow works end-to-end

## References
- ACP Spec: https://agentclientprotocol.com/
- Current iOS integration: `ios/OpenAgents/Bridge/BridgeManager.swift`
- JSON-RPC server: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/`
- WebSocket client: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/`
