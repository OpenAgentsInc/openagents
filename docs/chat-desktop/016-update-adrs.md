# Issue #16: Create/Update ADRs for Architecture Changes

## Phase
Phase 5: Testing & Documentation

## Priority
Medium - Documentation and decision tracking

## Description
Create or update Architecture Decision Records (ADRs) to document the significant architectural decisions made during the ChatGPT-style interface transformation.

## Current State
- ADRs exist for core architecture (ADR-0001 through ADR-0006)
- No ADR documenting the desktop chat interface architecture
- Navigation pattern change (dashboard → three-pane) not documented
- Session management approach not documented

## Target State
- New ADR documenting macOS chat interface architecture
- Updates to existing ADRs if needed (e.g., ADR-0003 on Swift app)
- Clear rationale for three-pane layout decision
- Documentation of session persistence approach
- Links to related issues and code references

## Acceptance Criteria
- [x] Create ADR-0007: "macOS Chat Interface Architecture"
- [ ] Update ADR-0003 if needed (Swift app architecture)
- [x] Follow existing ADR format and tone
- [x] Include alternatives considered
- [x] Document consequences (positive and negative)
- [x] Link to relevant GitHub issues/docs
- [ ] Review by team/maintainers (if applicable)
- [x] Follow guidelines in `docs/adr/AGENTS.md`

## Status Update — ADR Created
- Added `docs/adr/0007-macos-chat-interface-architecture.md` documenting:
  - NavigationSplitView desktop layout (sidebar + chat; inspector deferred)
  - Shared chat state via BridgeManager/TimelineStore
  - macOS Option A (local JSON‑RPC adapter + server publisher)
  - Tinyvex persistence via SessionUpdateHub
  - Alternatives considered and consequences

Next:
- Review if ADR‑0003 needs a brief addendum to reference Option A on macOS; if so, update and cross‑link to ADR‑0007.

## Technical Details

### ADR-0007: macOS Chat Interface Architecture

Create: `docs/adr/0007-macos-chat-interface-architecture.md`

```markdown
# ADR-0007: macOS Chat Interface Architecture

## Status
Accepted

## Context
OpenAgents v0.3 launched with a dashboard-style macOS interface (`SimplifiedMacOSView`) showing status cards for bridge, directory, and dev tools. While functional for monitoring, this didn't support the core use case: conversational interaction with coding agents.

The iOS app already had a mature chat interface (NewChatView, ACPTimelineView, message renderers). We needed to bring that conversational UX to macOS while leveraging the desktop's larger screen real estate.

**Requirements:**
- Conversational interface (like ChatGPT desktop)
- Session history management
- Tool call/plan/reasoning visualization
- Access to settings/dev tools (moved from main view)
- Keyboard-first navigation
- Consistency with iOS app where possible

**Constraints:**
- macOS 13.0+ support (no iOS-only APIs)
- Native Swift/SwiftUI (no Electron, no React)
- Reuse existing ACP renderers from iOS
- Maintain bridge architecture (ADR-0004)

## Decision
We will transform the macOS app to use a **three-pane NavigationSplitView** layout:

### Layout
1. **Left Sidebar (220-280px):**
   - Session history list
   - Search/filter
   - "New Chat" button
   - Grouped by date (Today, Yesterday, etc.)

2. **Center Pane (flex):**
   - Main chat area with message timeline
   - Composer at bottom (NSTextView-based for keyboard control)
   - Auto-scroll on new messages
   - Empty state when no session

3. **Right Inspector (280-350px, collapsible):**
   - Tool call details
   - JSON output viewer
   - Message metadata
   - Debugging information

### Key Architectural Choices

**Choice 1: Three-pane over two-pane or single-pane**
- *Alternative considered:* Two-pane (sidebar + chat) like ChatGPT desktop
- *Why three-pane:* Developer-focused app benefits from persistent inspector for tool execution details. Optional visibility maintains simplicity when not needed.

**Choice 2: Reuse iOS ACP renderers, adapt layout components**
- *Alternative considered:* Build all macOS components from scratch
- *Why reuse:* ToolCallView, PlanView, ReasoningSummaryView are platform-agnostic. Only layout/navigation differs between iOS and macOS. Reduces duplication and bugs.

**Choice 3: NSTextView-wrapped composer vs pure SwiftUI TextEditor**
- *Alternative considered:* SwiftUI TextEditor for simplicity
- *Why NSTextView:* Better keyboard control (Return = send, Shift+Return = newline). TextEditor has limited event handling in SwiftUI as of macOS 13.

**Choice 4: Sheet-based Settings/Developer views vs separate windows**
- *Alternative considered:* Separate windows (like System Settings)
- *Why sheets:* Simpler state management, modal focus aligns with "configure and return to chat" workflow. Can revisit if users request windowed settings.

**Choice 5: TinyvexManager for session persistence**
- *Alternative considered:* Core Data, SQLite directly, JSON files
- *Why Tinyvex:* Already in use for orchestration configs. Lightweight, schema-flexible. Consistent with existing storage patterns.

### Session Management
- Sessions auto-save on every message (no explicit save button)
- Foundation Models generate titles from first few messages
- Sessions stored in Tinyvex with full update history
- Switching sessions = load from database (not re-fetch from bridge)

### Settings/Dev Tools Migration
- Current dashboard cards moved to dedicated views:
  - **Settings:** Bridge, Working Directory, Agent Config, Orchestration
  - **Developer:** Tinyvex tools, Nostr tools, logs, diagnostics
- Accessible via toolbar buttons (⌘, for Settings, ⌘⌥D for Developer)

## Consequences

### Positive
- **Conversational UX:** Users can chat with agents naturally, matching ChatGPT desktop experience
- **Code reuse:** ~80% of iOS chat components work on macOS with minimal changes
- **Session history:** Easy to review past conversations, switch contexts
- **Keyboard-first:** All actions have shortcuts, tab navigation works
- **Developer-friendly:** Inspector pane surfaces tool execution details
- **Scalable:** Three-pane layout adapts to large screens better than mobile layouts

### Negative
- **Complexity:** More views to maintain than simple dashboard
- **Migration friction:** Users familiar with v0.3 dashboard need to learn new layout
- **NSTextView wrapper:** Adds AppKit dependency in SwiftUI app (necessary for keyboard control)
- **Testing surface:** Larger UI surface area = more integration tests needed

### Neutral
- **iOS/macOS divergence:** Layout differs significantly from iOS (but renderers stay shared)
- **Screen real estate:** Three panes require ~900px minimum width (acceptable for desktop)

## Implementation
Tracked in GitHub issues:
- #1: Three-pane layout foundation
- #2: BridgeManager chat state
- #3: macOS Composer
- #4: Session sidebar
- #5: Chat area
- #6: Inspector pane
- #7: Agent selector
- #8: Settings view
- #9: Developer view
- #10: Toolbar navigation
- #11: Chat integration
- #12: Session management
- #13: Keyboard shortcuts
- #14: Theming consistency
- #15: Integration tests
- #16: ADR updates
- #17: Documentation

**Files created:**
- `ios/OpenAgents/Views/macOS/ChatMacOSView.swift` - Main three-pane layout
- `ios/OpenAgents/Views/macOS/SessionSidebarView.swift` - Session history
- `ios/OpenAgents/Views/macOS/ChatAreaView.swift` - Message timeline
- `ios/OpenAgents/Views/macOS/InspectorPaneView.swift` - Tool details
- `ios/OpenAgents/Views/macOS/ComposerMac.swift` - Message input
- `ios/OpenAgents/Views/macOS/Settings/SettingsView.swift` - Settings UI
- `ios/OpenAgents/Views/macOS/Developer/DeveloperView.swift` - Dev tools
- (Additional supporting views)

**Files replaced:**
- `ios/OpenAgents/SimplifiedMacOSView.swift` - No longer entry point (moved to Settings/Developer)

## Related ADRs
- ADR-0002: Agent Client Protocol (ACP renderers reused)
- ADR-0003: Swift Cross-Platform App (macOS target)
- ADR-0004: iOS ↔ Desktop WebSocket Bridge (unchanged)
- ADR-0005: Liquid Glass (applied to macOS UI)
- ADR-0006: Foundation Models (conversation title generation)
 
Implementation notes for ADR-0007
- Reaffirm ADR‑0002 conventions: only ACP‑derived shapes across the app/bridge boundary; prefer snake_case for payload fields (ACP exceptions allowed per ADR‑0002).
- Document reuse of `TimelineStore` and a shared prompt dispatcher on macOS to align iOS/macOS chat wiring.

## References
- ChatGPT desktop app (UX inspiration)
- Xcode, VS Code, Linear (three-pane layout patterns)
- Apple HIG: macOS windowing and navigation
- Issues #1-#17 in `/private/chat-desktop/`
```

### Update ADR-0003 (If Needed)
If the Swift app ADR needs updates to reflect the new macOS UI:

```markdown
## Amendment (2025-01-08)
The macOS app has evolved from a dashboard-style interface to a conversational chat interface (see ADR-0007). This maintains the Swift-only architecture but shifts the UI paradigm from status monitoring to active agent interaction. All changes remain within SwiftUI/AppKit with no external dependencies.
```

### ADR Writing Guidelines (from `docs/adr/AGENTS.md`)
When writing ADRs as an AI agent:

1. **Be direct and honest:** Don't sugarcoat trade-offs. "This increases complexity" is better than "This provides enhanced flexibility."

2. **Focus on WHY, not WHAT:** The code shows what we did. ADRs explain why we made specific choices.

3. **Document alternatives:** What else did we consider? Why didn't we choose them?

4. **Use specific examples:** Reference actual files, types, functions from the OpenAgents codebase.

5. **Keep it concise:** 500-1000 words is ideal. Don't write a novel.

6. **Avoid corporate speak:** Write like a developer talking to another developer.

7. **Be critical of your own decisions:** Include consequences (both positive AND negative).

## Dependencies
- All implementation issues (#1-#14)

## Blocked By
- All implementation issues (need completed implementation to document)

## Blocks
None - Documentation is final step

## Estimated Complexity
Low-Medium (2-3 hours)

## Testing Requirements
- [ ] ADR follows existing format
- [ ] ADR tone matches AGENTS.md guidelines
- [ ] All alternatives considered are documented
- [ ] Consequences include both positive and negative
- [ ] Links to issues work
- [ ] File paths are correct
- [ ] Status is "Accepted" (after review)
- [ ] Added to ADR index (if applicable)

## References
- ADR template: `docs/adr/0000-template.md`
- ADR agent guidelines: `docs/adr/AGENTS.md`
- Existing ADRs: `docs/adr/0001-*.md` through `0006-*.md`
- ADR script: `docs/adr/new.sh`
