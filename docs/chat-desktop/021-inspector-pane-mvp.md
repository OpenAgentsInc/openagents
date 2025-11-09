## Issue #21: Enable Inspector Pane (MVP)

### Phase
Phase 2/4 — Components + Integration

### Priority
Medium — Developer visibility into tool calls

### Description
Turn on the right inspector pane on macOS and wire it to show tool call details and JSON output when the user taps a tool call row in the chat timeline. Keep OATheme black styling consistent.

### Acceptance Criteria
- [x] Three-column NavigationSplitView (sidebar + content + inspector)
- [x] Inspector toggles with ⌘I and toolbar button
- [x] Selecting a tool call row opens the inspector with details
- [x] Inspector shows tool name and pretty JSON output (or raw JSON)
- [x] Copy button copies JSON to clipboard
- [x] Theming aligns with OATheme black (no vibrancy/glass)

### Implementation
- Root: switched to 3‑column `NavigationSplitView` in `ChatMacOSView`, added inspector toggle (⌘I) and toolbar button.
- Inspector: new `InspectorPaneView` reads `BridgeManager.selectedToolCallId`, renders tool name and output JSON, with a copy button.
- Selection: `ChatAreaView` sets `bridge.selectedToolCallId` when tapping tool call/update rows.
- Commands: added `Toggle Inspector` in View menu (⌘I).

### Files
- ios/OpenAgents/Views/macOS/ChatMacOSView.swift — 3‑column split view + toggle
- ios/OpenAgents/Views/macOS/InspectorPaneView.swift — inspector UI (tool + JSON)
- ios/OpenAgents/Views/macOS/ChatAreaView.swift — row tap selects tool call
- ios/OpenAgents/Commands/OpenAgentsCommands.swift — Cmd‑I command wiring
- ios/OpenAgents/Bridge/BridgeManager.swift — `selectedToolCallId` state

### Notes
- Inspector remains minimal; future work can add richer renderers and structured sections.
- Keeps strict OATheme black surfaces as requested (no glass yet).

### Status
Completed and merged to `main`.

