## Issue #23: Sidebar Search Focus & UX

### Phase
Phase 4 — Integration & Polish

### Priority
Low — Keyboard/UX refinement

### Description
Improve search ergonomics in the Session Sidebar by adding a keyboard shortcut to focus the search field and a quick way to clear it.

### Acceptance Criteria
- [x] Cmd‑F focuses the sidebar search field
- [x] Escape clears the search if focused
- [x] Styling unchanged (OATheme black)

### Implementation
- SessionSidebarView
  - Added `@FocusState` and applied `.focused($isSearchFocused)` to the search `TextField`.
  - Added `.onExitCommand` to clear search with Escape when the search field is focused.
  - Exposed a focused scene value `focusSidebarSearch` that sets focus.
- Commands
  - Added `Find Sessions…` (Cmd‑F) in View menu; calls the focused scene value.

### Files
- ios/OpenAgents/Views/macOS/SessionSidebarView.swift
- ios/OpenAgents/Commands/OpenAgentsCommands.swift

### Status
Completed and merged to `main`.

