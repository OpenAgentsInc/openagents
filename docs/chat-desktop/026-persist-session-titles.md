## Issue #26: Persist Session Titles to Tinyvex

### Phase
Phase 4 — Integration & Polish

### Priority
Medium — Data durability

### Description
Persist user-edited conversation titles to Tinyvex (SQLite) so titles survive app restarts and are visible across devices using the desktop server.

### Acceptance Criteria
- [x] Tinyvex DB schema includes a `conversation_titles` table
- [x] JSON‑RPC methods: `tinyvex/history.setSessionTitle`, `tinyvex/history.getSessionTitle`
- [x] macOS inline rename persists to DB via BridgeManager
- [x] Build remains green

### Implementation
- TinyvexDbLayer
  - Migration adds `conversation_titles(session_id TEXT PRIMARY KEY, title TEXT, updated_at INTEGER)`
  - Methods: `setSessionTitle(sessionId:title:updatedAt:)`, `getSessionTitle(sessionId:)`
- DesktopWebSocketServer
  - Registered handlers in `registerHistoryHandlers()` for `tinyvex/history.setSessionTitle` and `tinyvex/history.getSessionTitle`.
- LocalJsonRpcClient
  - Implemented local paths for both methods via `localSetSessionTitle`/`localGetSessionTitle` helpers (added to `DesktopWebSocketServer+Local.swift`).
- BridgeManager (macOS)
  - New helper `setSessionTitle(sessionId:title:)` issues JSON‑RPC to persist title.
- SessionSidebarView
  - Commits inline rename by updating local `conversationTitles` and calling `bridge.setSessionTitle`.

### Files
- ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift
- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift
- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Local.swift
- ios/OpenAgents/Bridge/LocalJsonRpcClient.swift
- ios/OpenAgents/Bridge/BridgeManager+Mac.swift
- ios/OpenAgents/Views/macOS/SessionSidebarView.swift

### Status
Completed and merged to `main`.

