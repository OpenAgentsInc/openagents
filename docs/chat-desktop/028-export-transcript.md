## Issue #28: Export Transcript (JSON/Markdown)

### Phase
Phase 4 — Integration & Polish

### Priority
Medium — Share and archive chats

### Description
Add the ability to export the current session’s transcript as JSON (ACP updates) or Markdown (readable conversation). Expose actions in the Inspector header.

### Acceptance Criteria
- [x] Export JSON saves `[ACP.Client.SessionNotificationWire]` for the current session
- [x] Export Markdown saves a readable transcript (User/Assistant/Plan/Tool lines)
- [x] Save panel prompts for filename; sensible defaults
- [x] Build remains green

### Implementation
- InspectorPaneView
  - Added an export menu (upload icon) with “Export JSON…” and “Export Markdown…” actions.
  - JSON: encodes filtered `bridge.updates` for the current session using `JSONEncoder`.
  - Markdown: summarized conversation with role labels and simple plan/tool entries.
  - Uses `NSSavePanel` with `.json` and `.md` defaults; UTType fallback for markdown.

### Files
- ios/OpenAgents/Views/macOS/InspectorPaneView.swift

### Status
Completed and merged to `main`.

