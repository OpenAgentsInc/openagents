## Issue #25: Sidebar Inline Title Edit (Local)

### Phase
Phase 4 — Integration & Polish

### Priority
Low-Medium — Usability

### Description
Allow inline editing of conversation titles in the macOS Session Sidebar. Titles are saved locally in-memory (via `BridgeManager.conversationTitles`) to keep scope tight; Tinyvex schema doesn’t store titles yet.

### Acceptance Criteria
- [x] Double-click or context menu → Rename enters inline edit mode for a row
- [x] Enter commits the title; Esc cancels
- [x] Title stored in `BridgeManager.conversationTitles[session_id]`
- [x] Theming consistent with OATheme black

### Implementation
- SessionSidebarView
  - Added `editingTitleId` and `editingTitleText` state.
  - Shows `TextField` when a row is being edited; commits on submit and updates `bridge.conversationTitles`.
  - Context menu includes “Rename…”; double-clicking the title also starts edit.

### Files
- ios/OpenAgents/Views/macOS/SessionSidebarView.swift

### Status
Completed and merged to `main`. Persistence to Tinyvex can be added in a future issue (#26) if desired.

