# macOS Simplified Home Screen

**Platform**: macOS 13.0+
**File**: `ios/OpenAgents/SimplifiedMacOSView.swift`
**Status**: Active (default as of v0.3)
**Feature Flag**: `Features.simplifiedMacOSUI`

## Overview

The macOS simplified home screen is a clean, focused interface that serves as the primary control panel for managing coding agents from your Mac. It completely replaces the previous full chat history UI with three essential sections: bridge status, working directory selection, and agent configuration.

## Design Philosophy

### Radical Simplicity

This screen embodies the principle that **less is more**. Instead of overwhelming users with chat history, session management, and complex navigation, we present only what's immediately necessary to get started:

1. **Connection status** - Is the bridge ready?
2. **Working directory** - Where should agents work?
3. **Agent selection** - Which coding assistants are available?

That's it. Everything else is hidden by default.

### Why Simple?

The macOS app's role is fundamentally different from the iOS app:

- **Mac as Server**: The Mac runs the bridge server and hosts agent sessions
- **iOS as Client**: The iPhone/iPad connects to view and interact with sessions
- **Configuration > Conversation**: On Mac, you're setting up infrastructure, not having conversations

Users don't need to see chat history on Mac because they'll primarily interact through iOS. The Mac is the "command center" - it just needs to be configured correctly and left running.

## Visual Design

### Color Palette

All colors come from our unified design system (`Theme.swift`):

- **Background**: `#08090a` - Deep offblack for the main canvas
- **Card/Borders**: `#23252a` - Subtle gray for section backgrounds and dividers
- **Text Primary**: `#f7f8f8` - Nearly white for headings and main text
- **Text Secondary**: `#d0d6e0` - Light gray for supporting text
- **Text Tertiary**: `#8a8f98` - Mid gray for hints and placeholders
- **Success Green**: `#04A545` - Used for toggle switches and "detected" status
- **Warning Yellow**: `#FEBF00` - Reserved for warnings
- **Danger Red**: `#e7040f` - Used for errors

### Typography

**Berkeley Mono everywhere.** Every text element on this screen uses Berkeley Mono, creating a cohesive, code-focused aesthetic. This was a deliberate choice:

- Reinforces the "developer tool" identity
- Creates visual consistency across all UI text
- Matches the monospace nature of code and terminal output
- Looks distinctive and memorable

Implementation: `Fonts.swift` sets Berkeley Mono as the `primary` font on macOS (while iOS continues using Inter for better system integration). All buttons explicitly wrap their text in `Text()` views with the font applied to ensure Berkeley Mono is used regardless of button style.

### Window Constraints

**Minimum Width**: 500px - Prevents the window from being resized too small, ensuring all content remains readable and properly formatted. Implemented with `.frame(minWidth: 500)` on the root view and `.defaultSize(width: 600, height: 800)` on the WindowGroup.

### Layout Structure

```
┌─────────────────────────────────────────────────────────────┐
│  OpenAgents                                                 │  ← Title (Berkeley Mono 32pt)
│  Desktop Command Center                                     │  ← Subtitle (Berkeley Mono 15pt)
├─────────────────────────────────────────────────────────────┤
│  🌐 Bridge Status        ⓘ View Setup Instructions          │  ← Header row (left + right aligned)
│  ┌─────────────────────────────────────────────────────┐   │
│  │ ✓ Ready for Connections                              │   │  ← Status Card (fixed height)
│  │ Port: 9099 • 1 client connected                      │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  ─────────────────────────────────────────────────────────  │  ← Divider
├─────────────────────────────────────────────────────────────┤
│  📁 Working Directory                                       │  ← Section Header (left-aligned)
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 📁 openagents                        [Change]        │   │  ← Selected Directory
│  │ /Users/.../code/openagents                           │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  ─────────────────────────────────────────────────────────  │  ← Divider
├─────────────────────────────────────────────────────────────┤
│  ⚙️ Configure Coding Agents                                 │  ← Section Header (left-aligned)
│  ┌─────────────────────────────────────────────────────┐   │
│  │ OpenAI Codex                          [Toggle On]   │   │  ← Agent Row (lighter bg when enabled)
│  │ Detected                                             │   │
│  │                                                      │   │
│  │ Claude Code                           [Toggle On]   │   │
│  │ Detected                                             │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘

(Minimum window width: 500px)
```

## Component Breakdown

### 1. Bridge Status Section

**Purpose**: Show connection readiness at a glance

**States**:
- **Idle**: No server running (shouldn't happen - auto-starts)
- **Advertising**: ✓ Ready for Connections (green checkmark)
- **Connecting/Handshaking**: Loading spinner (yellow)
- **Connected**: iOS device connected (green)
- **Error**: Red text with error message

**Key Decision**: Fixed height card (`minHeight: 60`) prevents layout shift when "1 client connected" text appears. Client count is inline with port number using a bullet separator: `Port: 9099 • 1 client connected`

**Setup Instructions Link**: Positioned in the header row (flush right, same line as "Bridge Status" label) for easy access. Opens `BridgeSetupInstructionsSheet` with step-by-step Bonjour pairing guide and troubleshooting tips. Using Berkeley Mono font for consistency.

### 2. Working Directory Section

**Purpose**: Let user choose where agents will operate

**States**:
- **No Directory Selected**: Large centered prompt with folder icon
- **Directory Selected**: Shows folder icon, name, and full path with "Change" button

**Key Decision**: Persists selection in UserDefaults (`BridgeManager.workingDirectory`) so it survives app restarts. This is foundational infrastructure - once set, it rarely changes.

**Why Not Auto-Detect?**: We could scan for git repos or recent project folders, but explicit selection is clearer and prevents accidents (like an agent operating in `/` or `~/Desktop`).

### 3. Configure Coding Agents Section

**Purpose**: Choose which coding assistants are available

**Agents**:
1. **OpenAI Codex** - Detected by checking `~/.codex/sessions/`
2. **Claude Code** - Detected by checking `~/.claude/`
3. ~~OpenAgents Coder~~ - Commented out for now (always available, no detection needed)

**Visual Design**:
- **No checkmarks on left** - We removed the detection indicators to reduce visual clutter
- **Green toggles on right** - Using our success green (`#04A545`) for the switches
- **Status text below name** - "Detected" or "Not found" for agents that require installation
- **Clickable rows** - Entire row is tappable, not just the toggle switch
- **Inverted color scheme** - Enabled agents have *lighter* background (`border.opacity(0.5)`), disabled agents have *darker* background (`card`). This makes active selections more visually prominent.
- **Disabled state** - Undetected agents are dimmed (50% opacity) and non-interactive
- **Berkeley Mono font** - All agent names and status text use Berkeley Mono for consistency

**Key Decision**: Order matters. We put OpenAI Codex first and Claude Code second because Codex is more widely adopted. OpenAgents Coder (our built-in agent) will be at the top when uncommented, since it's always available.

## Implementation Details

### Feature Flag System

```swift
// Features.swift
static var simplifiedMacOSUI: Bool {
    if ProcessInfo.processInfo.environment["OPENAGENTS_SIMPLIFIED_MACOS"] == "0" { return false }
    if UserDefaults.standard.object(forKey: "simplified_macos_ui") != nil {
        return UserDefaults.standard.bool(forKey: "simplified_macos_ui")
    }
    return true // Default to simplified
}
```

This allows easy toggling for development/testing:
```bash
# Disable simplified UI
defaults write com.openagents.app simplified_macos_ui -bool false

# Or via environment variable
OPENAGENTS_SIMPLIFIED_MACOS=0 open OpenAgents.app
```

### Conditional Rendering

```swift
// ContentView.swift (macOS section)
if Features.simplifiedMacOSUI {
    SimplifiedMacOSView()
} else {
    NavigationSplitView {
        HistorySidebar(...)
    } detail: {
        AcpThreadView(...)
    }
}
```

The full chat history UI remains in the codebase, just hidden. This was intentional - we may want to bring it back as an advanced mode or developer option.

### Agent Detection Logic

```swift
// SimplifiedMacOSView.swift
private func detectAgents() {
    let fm = FileManager.default
    let home = fm.homeDirectoryForCurrentUser

    // Detect Claude Code
    let claudeDir = home.appendingPathComponent(".claude")
    claudeDetected = fm.fileExists(atPath: claudeDir.path)

    // Detect Codex
    let codexDir = home.appendingPathComponent(".codex/sessions")
    codexDetected = fm.fileExists(atPath: codexDir.path)

    // Auto-enable if detected
    if claudeDetected { claudeEnabled = true }
    if codexDetected { codexEnabled = true }
}
```

Simple filesystem checks. We could be smarter (check for valid session files, parse config), but this "exists on disk" heuristic is good enough. If the directory exists, the user probably has it installed.

### State Management

All state is local to `SimplifiedMacOSView`:

```swift
@EnvironmentObject var bridge: BridgeManager  // Injected from app root
@State private var showInstructions = false   // Modal visibility
@State private var claudeDetected = false     // Detection results
@State private var codexDetected = false
@State private var claudeEnabled = true       // User toggles
@State private var codexEnabled = true
@State private var openagentsEnabled = true
```

We use `@EnvironmentObject` for `BridgeManager` because it's shared app-wide (iOS and macOS both need it). Everything else is view-local state.

**Why not persist agent toggles?** Because detection happens every time the view appears. If we persisted toggles, we'd need to sync them with detection state, handle conflicts (what if user disabled but then installed?), etc. Simpler to just enable detected agents by default and let the user toggle them off if needed.

## User Flow

### First Launch

1. App opens, shows simplified screen
2. Bridge auto-starts, status changes to "Ready for Connections"
3. No working directory selected - big "Select Directory" button prompts user
4. Agent detection runs in background
5. Detected agents show with green checkmarks and toggles on

### Typical Setup

1. User clicks "Select Directory"
2. Native macOS folder picker (`NSOpenPanel`) opens
3. User navigates to project folder (e.g., `~/code/my-app`)
4. Selection persists to UserDefaults
5. Screen updates to show selected folder with "Change" button
6. User reviews detected agents (Codex: ✓, Claude: ✗)
7. User can toggle agents on/off as desired

### Daily Use

After initial setup, this screen is mostly passive:
- Bridge status confirms server is running
- Working directory shows current config
- Agent toggles show what's available

User typically opens the iOS app to actually interact with agents. The Mac just sits there serving requests.

## Design Decisions Deep Dive

### Why Not Show Chat History?

**Original design** had a sidebar with recent Claude Code and Codex sessions, similar to the Claude.ai web UI. We removed it because:

1. **Mobile-first interaction**: Users will primarily chat on iPhone/iPad where they can see their hands and the conversation simultaneously
2. **Mac as infrastructure**: The desktop app's role is to host agent sessions, not display them
3. **Reduced complexity**: No need for session management UI, scrolling, search, etc.
4. **Faster iteration**: Simple UI means we can ship features faster without coordinating desktop/mobile views

If users want to see history on Mac, they can toggle the feature flag to restore the full UI.

### Why Not Auto-Start Sessions?

After selecting a working directory, we *could* automatically start an agent session. We don't because:

1. **Explicit intent**: User might just be configuring for later
2. **iOS is primary**: Sessions are meant to be started from mobile
3. **No prompt yet**: We'd need to ask "what do you want to do?" which adds another input field

Instead, we leave the Mac configured and idle. When the user opens the iOS app and sends a message, that's when a session starts.

### Why Dividers Between Sections?

We added subtle dividers (`Divider().background(OATheme.Colors.border)`) between sections to:

1. **Visual hierarchy**: Makes it clear where one section ends and another begins
2. **Breathing room**: Prevents the UI from feeling cramped
3. **SwiftUI best practice**: Native `Divider` component is the standard way to separate content
4. **Responsive**: Dividers stretch to fill available width, adapting to window size

### Why Left-Align Section Headers?

Originally, section headers (`Label("Bridge Status", systemImage: "network")`) were centered within their VStack. We changed to left-alignment because:

1. **Scan pattern**: Western readers scan left-to-right, top-to-bottom
2. **Consistency**: Content within sections is left-aligned, so headers should match
3. **Less "floaty"**: Centered text feels unanchored, left-aligned feels structured
4. **SwiftUI convention**: Most Apple system UIs left-align section headers (Settings, Files, etc.)

Implementation:
```swift
HStack {
    Label("Bridge Status", systemImage: "network")
        .font(OAFonts.ui(.headline, 16))
        .foregroundStyle(OATheme.Colors.textSecondary)
    Spacer()  // Pushes label to left
}
.frame(maxWidth: 500)
```

### Why Remove Detection Checkmarks?

Original design had green checkmarks next to detected agents:
```
✓ OpenAI Codex    [Toggle]
  Detected
```

We removed them because:

1. **Redundant**: "Detected" text already conveys the information
2. **Visual noise**: Three icons in one row (checkmark, text, toggle) felt cluttered
3. **Inconsistent semantics**: Checkmark usually means "selected" or "enabled", but detection is different from enablement

Now it's cleaner:
```
OpenAI Codex      [Toggle]
Detected
```

The toggle is the only interactive element, and the status text provides context.

### Why Green Toggles?

We set all toggle switches to use our success green (`#04A545`) via `.tint(OATheme.Colors.success)`. This:

1. **Brand consistency**: Green is our "active/enabled" color throughout the app
2. **Clear affordance**: Bright color makes it obvious the toggle is interactive
3. **Accessibility**: High contrast against dark background
4. **macOS convention**: System toggles are blue by default, but custom tinting is common in third-party apps

## Accessibility

### VoiceOver

All interactive elements have semantic labels:
- Buttons announce their text ("Change", "Select Directory")
- Toggles announce agent name and state ("OpenAI Codex, enabled")
- Status text is included in screen reader output

### Keyboard Navigation

Standard macOS tab-order works:
1. "View Setup Instructions" link
2. "Change" button (if directory selected) OR "Select Directory" button
3. Agent toggle switches (in order: Codex, Claude)

Space bar activates buttons and toggles switches.

### Color Contrast

All text meets WCAG AA contrast requirements:
- Primary text (#f7f8f8) on background (#08090a): 19.1:1
- Secondary text (#d0d6e0) on background: 14.8:1
- Tertiary text (#8a8f98) on background: 8.4:1

Borders and dividers use 30% opacity (#23252a @ 0.3) for subtle separation without sacrificing legibility.

## Performance

### Lightweight Rendering

This screen is fast because:
- No scroll views (everything fits on screen)
- No complex layouts (just VStacks and HStacks)
- No animations (static content)
- Minimal state updates (only when bridge status changes or user interacts)

### Efficient Detection

Agent detection runs once on `.onAppear`:
```swift
.onAppear {
    detectAgents()
}
```

It's just two filesystem checks (`FileManager.default.fileExists`), which complete in <1ms. No network calls, no heavy computation.

### State Persistence

Only one thing persists across launches: working directory. This is a single UserDefaults write/read, which is near-instant:

```swift
UserDefaults.standard.set(url.path, forKey: "oa.bridge.working_directory")
```

We don't persist agent toggles, bridge status, or any other transient state.

## Testing

### Manual Testing Checklist

- [ ] Bridge status shows "Ready for Connections" on launch
- [ ] Green checkmark appears when bridge is advertising
- [ ] Port number displays without comma (e.g., "9099" not "9,099")
- [ ] Client count appears inline when iOS connects
- [ ] Card height stays fixed when client count changes
- [ ] "View Setup Instructions" opens modal
- [ ] Modal displays bridge status and step-by-step guide
- [ ] "Select Directory" opens native folder picker
- [ ] Selected directory persists after app restart
- [ ] "Change" button opens folder picker again
- [ ] Agent detection correctly identifies installed tools
- [ ] Toggles use green color (#04A545)
- [ ] Entire agent row is clickable, not just toggle
- [ ] Undetected agents are dimmed and disabled
- [ ] All fonts use Berkeley Mono
- [ ] Dividers appear between sections
- [ ] Section headers are left-aligned

### Edge Cases

**No agents detected**: Both Codex and Claude show "Not found" with disabled toggles. User can still use the app (bridge works regardless of agents).

**Working directory deleted**: If the persisted directory no longer exists, `FileManager.fileExists` returns false and we don't set `bridge.workingDirectory`. UI shows "No working directory selected" as if it was never set.

**Bridge fails to start**: Status shows error message in red. User can click "View Setup Instructions" for troubleshooting tips.

**Window resize**: All content is centered with `maxWidth: 500` constraints, so it gracefully adapts to different window sizes.

## Future Enhancements

### Potential Additions

1. **Session Activity Indicator**: Show a subtle badge or icon when an agent session is active
2. **Recent Projects**: Quick-select from recently used working directories
3. **Agent Installation Links**: If an agent isn't detected, show a link to installation docs
4. **Bridge Logs Viewer**: Expandable section to view bridge server logs for debugging
5. **Advanced Settings**: Gear icon to access port configuration, service name, etc.
6. **Multiple Working Directories**: Support switching between multiple projects without re-selecting
7. **Agent Presets**: Save combinations of enabled agents for different workflows

### What We Won't Add

1. **Chat interface on Mac**: That's what iOS is for
2. **Session management**: Start/stop/resume is better handled on mobile
3. **File browser**: User should use Finder for file navigation
4. **Code editor**: We're an agent orchestrator, not an IDE
5. **Metrics dashboard**: Too much complexity for the simplified view

If users want these features, they can build them in the full UI mode (which is still in the codebase behind the feature flag).

## Comparison to Full UI

### What Was Removed

The original macOS UI had:
- NavigationSplitView with sidebar + detail pane
- HistorySidebar with recent session list (top 10-20)
- Session discovery via `LocalClaudeDiscovery` and `LocalCodexScanner`
- AcpThreadView showing full conversation history
- Foundation Models integration for title generation
- Session selection and navigation
- Raw JSON viewer for debugging

All of this still exists in the codebase, just hidden by default.

### What Was Kept

The simplified UI still uses:
- BridgeManager for connection state
- BridgeConfig for port/service configuration
- Bonjour/mDNS discovery (in BridgeSetupInstructionsSheet)
- ACP protocol (just not rendered in the UI)
- Theme and font systems
- Working directory management (newly added)

### Migration Path

Users who want the full UI can re-enable it:

```bash
# Temporarily
OPENAGENTS_SIMPLIFIED_MACOS=0 open OpenAgents.app

# Permanently
defaults write com.openagents.app simplified_macos_ui -bool false
```

We may eventually add a menu bar option: "View → Show Chat History" to toggle between modes at runtime without restarting.

## Conclusion

The simplified macOS home screen is a deliberate exercise in restraint. By removing everything except the essentials, we've created a tool that's:

- **Fast to set up** - Three sections, three decisions
- **Easy to understand** - No hidden complexity or unclear options
- **Reliable** - Fewer moving parts means fewer failure modes
- **Maintainable** - Less code, less surface area for bugs

This is the foundation. As OpenAgents evolves, we can add features incrementally (via the feature flag or new screens) without overwhelming new users.

**Simplicity is not a lack of features. It's a carefully curated presence of only what matters.**
