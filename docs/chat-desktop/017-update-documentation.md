# Issue #17: Update AGENTS.md (CLAUDE.md symlink) with New macOS UI Architecture

## Phase
Phase 5: Testing & Documentation

## Priority
Medium - Developer onboarding and reference

## Description
Update `AGENTS.md` (project instructions; `CLAUDE.md` is a symlink) to reflect the new macOS chat interface architecture, replacing outdated dashboard references.

## Current State
- `CLAUDE.md` references `SimplifiedMacOSView` as main macOS view
- Dashboard cards mentioned (Bridge Status, Working Directory, etc.)
- No mention of chat interface, session management, or three-pane layout
- Architecture section doesn't reflect new UI paradigm

## Target State
- `CLAUDE.md` accurately describes new three-pane chat interface
- Dashboard references removed or moved to "deprecated" section
- Architecture section updated with chat UI components
- Common tasks updated (e.g., "Adding a new chat component")
- File structure reflects new macOS views directory
- Examples reference new components

## Acceptance Criteria
- [ ] Update "Key Components" section with macOS chat UI
- [ ] Update "Repository Layout" with new views structure
- [ ] Update "Architecture" section to mention three-pane layout
- [ ] Update "Common Tasks" with chat-related examples
- [ ] Remove/update outdated SimplifiedMacOSView references
- [ ] Add navigation to settings/developer views
- [ ] Update screenshots or diagrams (if any)
- [ ] Verify all file paths are correct

## Technical Details

### Updates to AGENTS.md

#### Section: Key Components → macOS App
**Current:**
```markdown
- **macOS App** (`ios/OpenAgents/`)
  - Native Swift macOS app (same Xcode project, different targets)
  - WebSocket server for iOS pairing and bridge communication
  - Desktop agent session management (Codex/Claude Code CLI integration)
  - Bonjour/mDNS discovery for zero-config LAN pairing
```

**Updated:**
```markdown
- **macOS App** (`ios/OpenAgents/`)
  - Native Swift macOS app (same Xcode project, different targets)
  - Three-pane chat interface (session history, chat area, inspector)
  - WebSocket server for iOS pairing and bridge communication
  - Desktop agent session management (Codex/Claude Code CLI integration)
  - Bonjour/mDNS discovery for zero-config LAN pairing
  - Settings and Developer tools accessible via toolbar
```

#### Section: Repository Layout
**Add under `ios/OpenAgents/`:**
```markdown
├── Views/
│   ├── macOS/                    # macOS-specific views
│   │   ├── ChatMacOSView.swift   # Main three-pane layout
│   │   ├── SessionSidebarView.swift
│   │   ├── ChatAreaView.swift
│   │   ├── InspectorPaneView.swift
│   │   ├── ComposerMac.swift
│   │   ├── Settings/
│   │   │   └── SettingsView.swift
│   │   └── Developer/
│   │       └── DeveloperView.swift
│   ├── NewChatView.swift         # iOS chat interface
│   ├── ACPTimelineView.swift     # Shared chat timeline
│   └── ...
```

#### Section: Common Tasks
**Add new task:**
```markdown
### Adding a New Chat Message Renderer

1. Create renderer in `ios/OpenAgents/ACP/` (platform-agnostic)
2. Add case to `MessageBubbleView` switch statement
3. Use `OATheme.Colors` for styling, `OAFonts.mono()` for text
4. Add detail sheet if needed (e.g., `ToolCallDetailSheet`)
5. Test on both iOS and macOS
6. Add integration test in `ChatUIRenderingTests.swift`

Example:
```swift
// In MessageBubbleView.swift
case .customContent(let content):
    CustomContentView(content: content)
        .onTapGesture {
            showDetailSheet = true
        }
```
```

**Add new task:**
```markdown
### Accessing Settings or Developer Tools on macOS

**Settings:**
- Click gear icon in toolbar
- Or press ⌘,
- Settings include: Bridge, Working Directory, Agent Config, Orchestration

**Developer Tools:**
- Select Developer > Developer Tools from menu bar
- Or press ⌘⌥D
- Tools include: Database inspector, Nostr tools, Logs, Diagnostics

**Previous dashboard view:**
- Deprecated as of v0.3.1
- Content migrated to Settings and Developer views
- See ADR-0007 for architecture decision rationale
```

#### Section: Architecture → v0.3 Swift-Only Architecture
**Add subsection:**
```markdown
### macOS Chat Interface (v0.3.1+)

The macOS app uses a **three-pane NavigationSplitView** layout:

- **Left sidebar**: Session history with search, grouped by date
- **Center pane**: Chat area with message timeline and composer
- **Right inspector**: Tool call details, JSON output, message metadata (collapsible)

**Key components:**
- `ChatMacOSView.swift` - Main three-pane container
- `SessionSidebarView.swift` - Session list with Foundation Models-generated titles
- `ChatAreaView.swift` - Message timeline using shared ACP renderers
- `ComposerMac.swift` - NSTextView-based input (Return = send, Shift+Return = newline)
- `InspectorPaneView.swift` - Developer-focused detail view

**Session management:**
- Sessions auto-save via TinyvexManager
- Switch sessions by clicking in sidebar
- Delete with confirmation (trash icon on hover)
- New session via ⌘N

**Settings and Developer tools:**
- Moved from main view to dedicated sheets
- Accessible via toolbar (⌘, and ⌘⌥D)
- See ADR-0007 for architecture decision

**Reused from iOS:**
- All ACP renderers (`ToolCallView`, `PlanView`, `ReasoningDetailSheet`)
- `UpdatesListView` patterns
- `BridgeManager` state management
- Theme system (`OATheme`, `OAFonts`)
```

#### Section: Deprecation Notes
**Add new subsection:**
```markdown
### v0.3.0 Dashboard (Deprecated in v0.3.1)

The initial macOS interface used `SimplifiedMacOSView` with a dashboard-style card layout:
- ❌ Bridge Status card
- ❌ Working Directory card
- ❌ Agent Configuration card
- ❌ Tinyvex Dev Tools card
- ❌ Nostr Dev Tools card
- ❌ Orchestration Console card

**Migration to v0.3.1:**
- Dashboard replaced with three-pane chat interface
- Settings content moved to Settings view (⌘,)
- Developer content moved to Developer view (⌘⌥D)
- See ADR-0007 for rationale and architecture details
```

### Files to Modify
- `AGENTS.md` (note: `CLAUDE.md` symlinks to this)

### Verification Steps
1. Read updated `AGENTS.md` in full (CLAUDE.md symlink)
2. Verify all file paths exist (`ChatMacOSView.swift`, etc.)
3. Verify all references are accurate (keyboard shortcuts, features)
4. Check for broken links or references
5. Ensure consistency with ADR-0007

## Dependencies
- Issue #16 (ADR-0007 must exist to reference it)

## Blocked By
- Issue #16

## Blocks
None - Final documentation step

## Estimated Complexity
Low (1-2 hours)

## Testing Requirements
- [ ] All file paths referenced exist
- [ ] All keyboard shortcuts are correct
- [ ] All feature descriptions match implementation
- [ ] Links to ADRs work
- [ ] No outdated references remain
- [ ] AGENTS.md renders correctly in GitHub/editors
- [ ] AI agents can understand new architecture from AGENTS.md

## References
- Current AGENTS.md: `AGENTS.md` (CLAUDE.md symlink exists)
- ADR-0007: Created in Issue #16
- File structure: `ios/OpenAgents/Views/macOS/`

## Example Additions

### Before (Current AGENTS.md)
```markdown
## Coding Style & Conventions
...
The macOS app displays a dashboard with cards for bridge status, working directory, and development tools.
```

### After (Updated AGENTS.md)
```markdown
## Coding Style & Conventions
...
The macOS app uses a three-pane chat interface for conversational interaction with coding agents. Session history, message timeline, and developer tools (inspector) are all accessible from the main view. Settings and advanced developer tools are available via toolbar buttons (⌘, and ⌘⌥D respectively).
```

### New Section: macOS Chat Interface Guide
```markdown
## macOS Chat Interface Guide

### Overview
The macOS app provides a ChatGPT-style conversational interface for interacting with coding agents.

### Layout
- **Left Sidebar (⌘B to toggle)**: Session history, search, new chat button
- **Center**: Message timeline with composer at bottom
- **Right Inspector (⌘I to toggle)**: Tool execution details, JSON output

### Keyboard Shortcuts
- ⌘N - New chat
- ⌘K - Select agent
- ⌘B - Toggle sidebar
- ⌘I - Toggle inspector
- ⌘, - Settings
- ⌘⌥D - Developer tools
- Return - Send message
- Shift+Return - New line in message

### Session Management
- **Create**: Click "New Chat" or press ⌘N
- **Switch**: Click session in sidebar
- **Delete**: Hover over session, click trash icon (or press Delete when focused)
- **Search**: Type in search box at top of sidebar

### Settings
Access via gear icon or ⌘,:
- **Connection**: Bridge status, server config
- **Workspace**: Working directory selection
- **Agents**: Agent preferences
- **Orchestration**: Orchestration config management

### Developer Tools
Access via menu or ⌘⌥D:
- **Database**: Tinyvex schema viewer, query executor
- **Nostr**: Event inspector, relay tester
- **Logs**: Filterable system logs
- **Diagnostics**: System info, debug data
```
