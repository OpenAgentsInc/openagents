# macOS ChatGPT-Style Interface Transformation

Complete redesign of the OpenAgents macOS desktop app from dashboard-style to ChatGPT-style conversational interface.

## Overview

This directory contains 18 GitHub issues that outline the complete transformation of the macOS app. The project involves:

- **Replacing** `SimplifiedMacOSView` dashboard with three-pane chat layout
- **Implementing** session history, chat area, and inspector panes
- **Moving** dashboard content to Settings and Developer views
- **Reusing** iOS chat components where possible
- **Adding** comprehensive keyboard shortcuts and accessibility
- **Testing** with integration test suite
- **Documenting** architecture decisions in ADRs

## Architecture

### Three-Pane Layout
- **Left Sidebar** (220-280px): Session history with search
- **Center Pane** (flex): Chat area with message timeline and composer
- **Right Inspector** (280-350px, collapsible): Tool call details and JSON viewer

### Key Technologies
- SwiftUI NavigationSplitView
- Berkeley Mono font (OAFonts)
- Liquid Glass materials (macOS 15+, with fallback)
- Agent Client Protocol (ACP) for messages
- TinyvexManager for session persistence
- Foundation Models for conversation titles

## Issues Breakdown

### Phase 1: Core Infrastructure (Issues #1-3)
Foundation for the three-pane layout and core components.

| Issue | Title | Complexity | Dependencies |
|-------|-------|------------|--------------|
| [#1](001-three-pane-layout-foundation.md) | Create Three-Pane NavigationSplitView Layout Foundation | Medium (2-3h) | None |
| [#2](002-bridge-manager-chat-state.md) | Extend BridgeManager with iOS Chat State Properties for macOS | Medium (2-4h) | None |
| [#3](003-macos-composer.md) | Build macOS Composer Component (NSTextView-based Input) | Medium (3-4h) | #2 |

### Phase 2: Main UI Components (Issues #4-7, #21-22)
Build the three panes and agent selector.

| Issue | Title | Complexity | Dependencies |
|-------|-------|------------|--------------|
| [#4](004-session-history-sidebar.md) | Implement Session History Sidebar with Search/Filtering | Medium-High (4-6h) | #1, #2 |
| [#5](005-main-chat-area.md) | Build Main Chat Area (Adapt iOS UpdatesListView/ACPTimelineView) | Medium-High (5-7h) | #1, #2, #3 |
| [#6](006-inspector-pane.md) | Create Collapsible Inspector Pane (Tool Details, JSON Viewer) | Medium (4-5h) | #1, #2 |
| [#7](007-agent-selector.md) | Add Agent/Model Selector to Toolbar/Header | Medium (3-4h) | #2 |
| [#21](021-inspector-pane-mvp.md) | Enable Inspector Pane (MVP) | Medium (3-4h) | #6 |
| [#22](022-inspector-actions-ux.md) | Inspector Actions & UX Refinements | Low-Medium (2-3h) | #21 |

### Phase 3: Settings & Developer Pages (Issues #8-10)
Migrate dashboard content to dedicated views.

| Issue | Title | Complexity | Dependencies |
|-------|-------|------------|--------------|
| [#8](008-settings-view.md) | Create Settings View (Bridge, Directory, Agent, Orchestration) | Medium-High (5-6h) | None |
| [#9](009-developer-view.md) | Create Developer View (Tinyvex, Nostr Dev Tools) | Medium-High (5-7h) | None |
| [#10](010-toolbar-navigation.md) | Add Toolbar Navigation to Settings/Developer | Low-Medium (2-3h) | #8, #9 |

### Phase 4: Integration & Features (Issues #11-14)
Wire everything together and add polish.

| Issue | Title | Complexity | Dependencies |
|-------|-------|------------|--------------|
| [#11](011-chat-integration.md) | Wire Chat Components to BridgeManager (Send/Receive Messages) | High (6-8h) | #2, #3, #5 |
| [#12](012-session-management.md) | Implement Session Management (New, Switch, Delete Sessions) | Medium-High (5-6h) | #2, #4, #11 |
| [#13](013-keyboard-accessibility.md) | Add Keyboard Shortcuts and Accessibility Features | Medium (4-5h) | All UI |
| [#14](014-theming-consistency.md) | Apply Consistent Theming (Liquid Glass, Berkeley Mono, OATheme) | Medium (4-6h) | All UI |

### Phase 5: Testing & Documentation (Issues #15-20)
Validate and document the implementation.

| Issue | Title | Complexity | Dependencies |
|-------|-------|------------|--------------|
| [#15](015-integration-tests.md) | Write Integration Tests for Chat Flow and Session Management | High (6-8h) | All implementation |
| [#16](016-update-adrs.md) | Create/Update ADRs for Architecture Changes | Low-Medium (2-3h) | All implementation |
| [#17](017-update-documentation.md) | Update AGENTS.md (CLAUDE.md symlink) with New macOS UI Architecture | Low (1-2h) | #16 |
| [#18](018-update-root-readme.md) | Update Root README with macOS Chat Interface and Links | Low-Medium (1-2h) | #16 |
| [#19](019-macos-quickstart-and-troubleshooting.md) | macOS Chat — Quickstart & Troubleshooting | Low-Medium (1-2h) | #17 |
| [#20](020-screenshots-and-diagrams.md) | Screenshots & Visual Overview (macOS Chat) | Low (1h) | #18, #19 |

## Estimated Total Effort

- **Phase 1**: 7-11 hours
- **Phase 2**: 16-22 hours
- **Phase 3**: 12-16 hours
- **Phase 4**: 19-25 hours
- **Phase 5**: 9-14 hours

**Total**: ~63-88 hours (approximately 8-11 working days for one developer)

## Implementation Strategy

### Recommended Approach
The issues are designed for **all-at-once** implementation (as per user requirements), but can be executed in phases for incremental testing:

1. **Phase 1** first (foundation)
2. **Phase 2** and **Phase 3** can be done in parallel by different developers
3. **Phase 4** requires Phase 1-2 completion
4. **Phase 5** should be done after all implementation

### Critical Path
Issues #1 → #2 → #3 → #5 → #11 → #12

These form the core chat functionality. Other issues can be worked on in parallel once foundations are in place.

## Key Files Created

### Views (macOS-specific)
- `ios/OpenAgents/Views/macOS/ChatMacOSView.swift` - Main layout
- `ios/OpenAgents/Views/macOS/SessionSidebarView.swift` - Session history
- `ios/OpenAgents/Views/macOS/ChatAreaView.swift` - Chat messages
- `ios/OpenAgents/Views/macOS/InspectorPaneView.swift` - Details pane
- `ios/OpenAgents/Views/macOS/ComposerMac.swift` - Message input
- `ios/OpenAgents/Views/macOS/Settings/SettingsView.swift` - Settings
- `ios/OpenAgents/Views/macOS/Developer/DeveloperView.swift` - Dev tools
- `ios/OpenAgents/Views/macOS/KeyboardShortcutsView.swift` - Help screen

### Files Modified
- `ios/OpenAgents/Bridge/BridgeManager.swift` - Add chat state
- `ios/OpenAgents/OpenAgentsApp.swift` - Switch main view
- `ios/OpenAgents/TinyvexManager.swift` - Session persistence
- `docs/adr/0007-macos-chat-interface-architecture.md` - New ADR
- `AGENTS.md` - Updated documentation (CLAUDE.md symlink)

### Files Deprecated
- `ios/OpenAgents/SimplifiedMacOSView.swift` - Replaced by new views

## Testing

### Manual Testing Checklist
- [ ] Three panes render correctly
- [ ] Can send and receive messages
- [ ] Sessions persist across app restarts
- [ ] Keyboard shortcuts work (⌘N, ⌘K, ⌘B, ⌘I, ⌘,, ⌘⌥D)
- [ ] Settings and Developer views accessible
- [ ] Theme consistency across all views
- [ ] VoiceOver navigation works

### Automated Testing
- Core bridge and ACP integration tests live in `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/` (e.g., `DesktopWebSocketServerComprehensiveTests.swift`, `BridgeServerClientTests.swift`).
- Add an app test target for macOS UI if needed (e.g., `OpenAgentsAppTests`/`OpenAgentsUITests`) to cover view wiring and rendering.
- UI rendering tests can use ViewInspector.
- Performance tests for large session lists.
- Target: 70%+ code coverage for new components.

## Documentation

### ADRs
- **ADR-0007**: macOS Chat Interface Architecture (new)
- **ADR-0003**: Swift Cross-Platform App (updated with amendment)

### Updated Docs
- `AGENTS.md` - Project instructions with new architecture (CLAUDE.md symlink also exists)
- This README - Overview and roadmap

## Keyboard Shortcuts Reference

| Shortcut | Action |
|----------|--------|
| ⌘N | New chat |
| ⌘K | Agent selector |
| ⌘B | Toggle sidebar |
| ⌘I | Toggle inspector |
| ⌘, | Settings |
| ⌘⌥D | Developer tools |
| ⌘/ | Show keyboard shortcuts |
| ⌘F | Search sessions |
| Return | Send message |
| Shift+Return | New line |
| Delete | Delete selected session |

## Questions or Issues?

- Review individual issue files for detailed implementation guidance
- Check ADR-0007 for architecture rationale
- Refer to AGENTS.md for coding conventions
- Look at existing iOS components for implementation patterns

---

Implementation notes

- macOS prompt dispatching should reuse the existing `PromptDispatcher` pattern used on iOS. Either:
  - Generalize `ios/OpenAgents/Bridge/PromptDispatcher.swift` to compile on macOS (remove `#if os(iOS)` and gate only platform‑specific bits), then wire it from `BridgeManager+Mac`, or
  - Add a lightweight `DesktopPromptDispatcher` that dispatches to `AgentRegistry`/`SessionUpdateHub` directly (bypassing WebSocket) and feeds a shared `TimelineStore`.
- Title generation should use `ConversationSummarizer.summarizeTitle(...)` in `OpenAgentsCore/Sources/OpenAgentsCore/Summarization/`.
- Follow ADR‑0002 ACP conventions; do not pass provider JSON directly to views.

Testing notes

- Core bridge and ACP behavior already has tests under `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/`.
- For macOS UI integration tests, create a new app test target or extend Core integration tests where feasible.

**Created**: 2025-01-08
**Status**: Ready for implementation
**Approved by**: User (plan mode)
