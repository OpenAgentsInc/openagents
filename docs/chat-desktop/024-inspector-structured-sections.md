## Issue #24: Inspector Structured Sections

### Phase
Phase 4 — Integration & Polish

### Priority
Medium — Clarity for tool inspection

### Description
Refactor the Inspector pane to group details into clear sections: Arguments, Output, and Error. Use collapsible DisclosureGroups and consistent code styling.

### Acceptance Criteria
- [x] Arguments section shows pretty-printed JSON from the original tool call
- [x] Output section shows Formatted/Raw (based on segmented control)
- [x] Error section shows last error string if present
- [x] Sections are collapsible DisclosureGroups

### Implementation
- InspectorPaneView
  - Added helpers to find the call (ACPToolCallWire) and latest update (ACPToolCallUpdateWire) by `call_id` from `bridge.updates`.
  - Pretty-printed arguments JSON via `JSONEncoder/JSONSerialization`.
  - Output remains sourced from `BridgeManager`’s formatted/raw maps.
  - Error rendered in a distinct section with danger color.

### Files
- ios/OpenAgents/Views/macOS/InspectorPaneView.swift

### Status
Completed and merged to `main`.

