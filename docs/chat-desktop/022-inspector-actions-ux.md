## Issue #22: Inspector Actions & UX Refinements

### Phase
Phase 4 — Integration & Polish

### Priority
Low-Medium — Improve inspection workflow

### Description
Enhance the Inspector pane with common actions and clearer selection feedback. Users can now copy, save, reveal, and open tool call JSON; selected rows in the chat timeline are highlighted.

### Acceptance Criteria
- [x] Copy JSON button
- [x] Save JSON (Save Panel) to a chosen file
- [x] Reveal in Finder and Open in default editor
- [x] Formatted/Raw toggle affects copy/save output
- [x] Tool call rows highlight when selected

### Implementation
- InspectorPaneView
  - Added actions: Copy, Save (NSSavePanel), Reveal in Finder, Open in editor.
  - Keeps a per-call last saved URL to enable Reveal/Open.
  - Added segmented control to switch Formatted/Raw view; copy/save reflects current mode.
  - Shows Tool, Status, and Call ID metadata.
- ChatAreaView
  - Tool call/update rows now show a subtle accent stroke when selected.

### Files
- ios/OpenAgents/Views/macOS/InspectorPaneView.swift
- ios/OpenAgents/Views/macOS/ChatAreaView.swift

### Status
Completed and merged to `main`.

