## Issue #27: Load Persisted Titles from Tinyvex

### Phase
Phase 4 — Integration & Polish

### Priority
Medium — Consistent titles

### Description
When recent sessions load in the macOS sidebar, fetch and display any user-defined titles persisted in Tinyvex so titles survive app restarts and are consistent across devices using the desktop server.

### Acceptance Criteria
- [x] BridgeManager fetches titles for each recent session on load
- [x] Titles stored in `conversationTitles` and used by sidebar rows
- [x] Build remains green

### Implementation
- BridgeManager+Mac
  - After `fetchRecentSessions` completes, iterate sessions and call `tinyvex/history.getSessionTitle` via RPC; update `conversationTitles` for non-empty values.
- LocalJsonRpcClient
  - Implemented local paths for set/get (done in #26).
- Sidebar
  - Already uses `conversationTitles[session_id]` if present.

### Files
- ios/OpenAgents/Bridge/BridgeManager+Mac.swift

### Status
Completed and merged to `main`.

