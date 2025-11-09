# Issue #12: Implement Session Management (New, Switch, Delete Sessions)

## Phase
Phase 4: Integration & Features

## Priority
High - Core user feature

## Description
Implement complete session management: create new sessions, switch between sessions, delete sessions, and persist session history.

## Current State
- Session sidebar exists (Issue #4)
- BridgeManager has session methods stubs
- No full CRUD implementation for sessions
- Session persistence incomplete

## Target State
- Users can create new chat sessions (⌘N)
- Click session in sidebar to switch/load it
- Delete sessions with confirmation
- Sessions persist to database
- Session history loads on app launch
- Active session highlighted in sidebar
- Automatic session creation on first message

## Status
Completed (implemented on main)

What shipped
- New session creation via sidebar "New Chat" button and ⌘N.
- Switching sessions loads Tinyvex history via RPCs and renders timeline.
- Delete session ‘tinyvex/history.deleteSession’ implemented server-side; UI shows confirmation and refreshes list.
- Persistence handled by SessionUpdateHub on server; UI performs no direct DB writes.
- Session list loads on app start (sidebar onAppear) and after deletes.
- Active session highlighted in sidebar.
- Titles auto-generate lazily via ConversationSummarizer and surface in sidebar when available.
- Keyboard shortcut: ⌘N for new; delete available via context menu.

## Acceptance Criteria
- [x] New session creation via toolbar button or ⌘N
- [x] Switching sessions loads history via Tinyvex history RPCs
- [x] Delete session removes from database and UI (server method)
- [x] Delete confirmation dialog for safety
- [x] Sessions auto-save via `SessionUpdateHub`
- [x] Session list loads on app launch
- [x] Active session visually indicated in sidebar
- [x] Empty state when no sessions exist
- [x] Session titles auto-generate via `ConversationSummarizer`
- [x] Keyboard shortcuts work (⌘N)

## Technical Details

### BridgeManager Session Methods (shared)

```swift
// In BridgeManager.swift
extension BridgeManager {
    func startNewSession(desiredMode: ACPSessionModeId? = nil) {
        timeline.clearAll()
        dispatcher?.sendPrompt(text: "", desiredMode: desiredMode, getSessionId: { self.currentSessionId }, setSessionId: { self.currentSessionId = $0 })
    }

    func loadSessionTimeline(sessionId: String) {
        currentSessionId = nil
        timeline.clearAll()
        dispatcher?.loadSessionTimeline(sessionId: sessionId) { [weak self] arr in
            self?.timeline.replaceAll(with: arr)
        }
    }

    func fetchRecentSessions() { dispatcher?.fetchRecentSessions { [weak self] in self?.recentSessions = $0 } }
}
```

### ThreadSummary Model
Use the existing `OpenAgentsCore/ThreadSummary.swift` (snake_case fields) returned by Tinyvex history endpoints. Do not redefine this type in the app layer.

### Tinyvex History
Prefer the server’s history API over direct DB access from the app:
- `tinyvex/history.recentSessions` → array of recent `RecentSession`
- `tinyvex/history.sessionTimeline` → array of `ACP.Client.SessionNotificationWire`

This keeps persistence logic centralized in the server and complies with ADR‑0002 (ACP on the wire).

### UI Integration

#### New Session Button
```swift
// In SessionSidebarView.swift
Button(action: createNewSession) {
    Label("New Chat", systemImage: "plus.message")
        .frame(maxWidth: .infinity)
}
.buttonStyle(.bordered)
.keyboardShortcut("n", modifiers: .command)

private func createNewSession() {
    Task {
        await bridgeManager.startNewSession()
    }
}
```

#### Delete Session Confirmation
```swift
// In SessionRowView.swift
@State private var showDeleteConfirmation = false

// Delete button
if isHovered {
    Button(action: { showDeleteConfirmation = true }) {
        Image(systemName: "trash")
            .foregroundColor(OATheme.Colors.danger)
    }
    .buttonStyle(.plain)
}

.alert("Delete Session?", isPresented: $showDeleteConfirmation) {
    Button("Cancel", role: .cancel) {}
    Button("Delete", role: .destructive) {
        Task {
            await bridgeManager.deleteSession(session.id)
        }
    }
} message: {
    Text("This will permanently delete this chat session. This action cannot be undone.")
}
```

#### Context Menu (Alternative)
```swift
// In SessionRowView
.contextMenu {
    Button("Rename...") {
        // Show rename dialog
    }

    Button("Duplicate") {
        // Duplicate session
    }

    Divider()

    Button("Delete", role: .destructive) {
        showDeleteConfirmation = true
    }
}
```

### App Launch Sequence
```swift
// In ChatMacOSView.swift or App initialization
.onAppear {
    // Load all sessions from database
    bridgeManager.loadAllSessions()
}
```

### Auto-Save on Message
```swift
// In BridgeManager.handleSessionUpdate
func handleSessionUpdate(_ update: ACP.Client.SessionNotificationWire) {
    updates.append(update)

    // Auto-save update
    if let sessionId = currentSessionId {
        tinyvexManager.saveUpdate(update, for: sessionId)

        // Update thread summary
        if let index = threads.firstIndex(where: { $0.id == sessionId }) {
            threads[index].messageCount = updates.count
            if threads[index].firstMessage == nil, case .text(let text) = update.content {
                threads[index].firstMessage = String(text.text.prefix(100))
            }
            tinyvexManager.saveThread(threads[index])
        }
    }

    // ... rest of update handling
}
```

### Keyboard Shortcuts
```swift
// In ChatMacOSView.swift
.keyboardShortcut("n", modifiers: .command) // New session

// In SessionRowView
.keyboardShortcut(.delete, modifiers: []) // Delete selected (when focused)
```

## Dependencies
- Issue #2 (BridgeManager chat state)
- Issue #4 (Session sidebar)
- Issue #11 (Chat integration)

## Blocked By
- Issue #2
- Issue #4
- Issue #11

## Blocks
None - Completes session management feature set

## Estimated Complexity
Medium-High (5-6 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] New session creates and activates correctly
- [ ] Switching sessions loads correct history
- [ ] Delete removes session from UI and database
- [ ] Delete confirmation prevents accidental deletion
- [ ] Sessions persist across app restarts
- [ ] Active session highlighted in sidebar
- [ ] Auto-save works on every message
- [ ] Session titles generate automatically
- [ ] Empty state shows when no sessions
- [ ] Keyboard shortcuts work (⌘N, Delete)
- [ ] Multiple rapid session switches don't cause race conditions
- [ ] Large session history loads performantly

## References
- TinyvexManager: `ios/OpenAgents/TinyvexManager.swift`
- Session sidebar: Created in Issue #4
- iOS session management: Similar patterns in iOS app
