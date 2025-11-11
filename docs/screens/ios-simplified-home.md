# iOS Simplified Home Screen

**Platform**: iOS 16.0+
**File**: `ios/OpenAgents/SimplifiedIOSView.swift`
**Status**: Active (default as of v0.3)
**Feature Flag**: `Features.simplifiedIOSUI`

## Overview

The iOS simplified home screen is a clean, focused interface that displays the essential connection and configuration information for OpenAgents on mobile. It replaces the previous full workspace exploration UI with three core sections: bridge connection status, working directory info, and enabled agents.

## Design Philosophy

### Mobile-First Simplicity

This screen embodies the principle that **mobile is for monitoring, not configuring**. Instead of overwhelming users with exploration controls, session management, and complex workflows, we present only the information needed to verify the system is working:

1. **Bridge connection** - Are we connected to the Mac?
2. **Working directory** - Where are agents working?
3. **Enabled agents** - Which coding assistants are available?

That's it. Everything else is managed on macOS.

### Why Simple on iOS?

The iOS app's role is fundamentally different from the macOS app:

- **iOS as Client**: The iPhone/iPad connects to the Mac to view and interact
- **Mac as Server**: The Mac runs the bridge server and manages configuration
- **Observation > Configuration**: On iOS, you're monitoring status, not changing settings

Users configure working directories and enable agents on macOS. The iOS app just displays this information so they know the system is properly set up and ready to use.

## Visual Design

### Color Palette

All colors come from our unified design system (`Theme.swift`):

- **Background**: `#08090a` - Deep offblack for the main canvas
- **Card/Borders**: `#23252a` - Subtle gray for section backgrounds
- **Text Primary**: `#f7f8f8` - Nearly white for headings and main text
- **Text Secondary**: `#d0d6e0` - Light gray for supporting text
- **Text Tertiary**: `#8a8f98` - Mid gray for hints and placeholders
- **Success Green**: `#04A545` - Used for checkmarks and "connected" status
- **Accent Blue**: Standard iOS accent for active elements

### Typography

**Berkeley Mono everywhere.** Just like macOS, every text element on iOS now uses Berkeley Mono, creating a cohesive, unified experience across platforms. This reinforces our identity as a developer-focused tool.

Implementation: `Fonts.swift` sets Berkeley Mono as the `primary` font on all platforms (previously iOS used Inter, but we switched to Berkeley Mono for consistency).

### Layout Structure

```
┌─────────────────────────────────────┐
│  ☰  Home                            │  ← Toolbar (hamburger + title)
├─────────────────────────────────────┤
│                                     │
│  OpenAgents                         │  ← Title (Berkeley Mono 32pt)
│  Mobile Command Center              │  ← Subtitle (Berkeley Mono 15pt)
│                                     │
│  🌐 Bridge Connection               │  ← Section Header
│  ┌───────────────────────────────┐ │
│  │ ✓ Connected                   │ │  ← Status Card
│  │ 127.0.0.1:9099                │ │
│  └───────────────────────────────┘ │
│                                     │
│  📁 Working Directory               │  ← Section Header
│  ┌───────────────────────────────┐ │
│  │ 📁 openagents                 │ │  ← Directory Info
│  │ /Users/.../openagents         │ │
│  └───────────────────────────────┘ │
│                                     │
│  ⚙️ Enabled Agents                  │  ← Section Header
│  ┌───────────────────────────────┐ │
│  │ OpenAI Codex            ✓     │ │  ← Agent Row
│  │ Managed on macOS              │ │
│  │                               │ │
│  │ Claude Code             ✓     │ │
│  │ Managed on macOS              │ │
│  └───────────────────────────────┘ │
│                                     │
└─────────────────────────────────────┘
```

## Component Breakdown

### 1. Toolbar

**Purpose**: Standard iOS navigation with menu access and screen title

**Components**:
- **Hamburger menu** (left): Opens navigation sheet (future: settings, help, about)
- **Title** (center-left): "Home" to indicate current screen
- Uses `ChatHeaderToolbar` for consistent iOS navigation patterns

### 2. Bridge Connection Section

**Purpose**: Show connection status to macOS server at a glance

**States**:
- **Idle**: Circle icon (gray) - No connection established
- **Discovering**: Loading spinner - Searching for Mac on network
- **Connecting**: Loading spinner - Connecting to Mac
- **Connected**: ✓ Checkmark (green) - Successfully connected with host:port displayed
- **Error**: Error message in red

**Key Decision**: Fixed height card (`minHeight: 60`) prevents layout shift during state changes. Connection info (host:port) displays below status text when connected.

**Bridge Protocol**: Connection established via WebSocket, handshake includes working directory in `_meta` field of initialize response.

### 3. Working Directory Section

**Purpose**: Display which directory agents are operating in

**States**:
- **Set**: Shows folder icon, directory name, and full path
- **Not set**: Shows folder.badge.questionmark icon with "Not set" message

**Key Decision**: Working directory is **read-only** on iOS. It's set on macOS and transmitted during the bridge handshake. iOS just displays it for reference.

**Implementation**: Working directory received from macOS server's `InitializeResponse._meta["working_directory"]` and stored in `BridgeManager.workingDirectory`.

### 4. Enabled Agents Section

**Purpose**: Display which coding assistants are available

**Agents**:
1. **OpenAI Codex** - Managed on macOS
2. **Claude Code** - Managed on macOS

**Visual Design**:
- Green checkmark on right indicates agent is enabled
- "Managed on macOS" text below name clarifies these are not iOS settings
- Lighter background (`border.opacity(0.5)`) for all rows since they're informational, not interactive

**Key Decision**: Agent configuration is **read-only** on iOS. Users enable/disable agents on macOS; iOS just displays which ones are available. This prevents confusion about where configuration happens.

## Implementation Details

### Feature Flag System

```swift
// Features.swift
static var simplifiedIOSUI: Bool {
    if ProcessInfo.processInfo.environment["OPENAGENTS_SIMPLIFIED_IOS"] == "0" { return false }
    if UserDefaults.standard.object(forKey: "simplified_ios_ui") != nil {
        return UserDefaults.standard.bool(forKey: "simplified_ios_ui")
    }
    return true // Default to simplified
}
```

This allows easy toggling for development/testing:
```bash
# Disable simplified UI
defaults write com.openagents.desktop simplified_ios_ui -bool false

# Or via environment variable
OPENAGENTS_SIMPLIFIED_IOS=0 open OpenAgents.app
```

### Conditional Rendering

```swift
// ChatHomeView.swift (iOS section)
var body: some View {
    if Features.simplifiedIOSUI {
        SimplifiedIOSView()
    } else {
        fullChatView
    }
}
```

The full workspace exploration UI remains in the codebase, just hidden. This was intentional - we may want to bring it back as an advanced mode or developer option.

### Working Directory Sync

Working directory flows from macOS to iOS during bridge initialization:

1. **macOS**: `BridgeManager.workingDirectory` loaded from UserDefaults on startup
2. **macOS**: `DesktopWebSocketServer.workingDirectory` set by BridgeManager
3. **macOS**: Server includes working directory in initialize response `_meta` field
4. **iOS**: `MobileWebSocketClient` extracts working directory from initialize response
5. **iOS**: Delegate callback passes working directory to `BridgeManager.workingDirectory`
6. **iOS**: `SimplifiedIOSView` displays working directory from BridgeManager

### State Management

All state comes from `BridgeManager`:

```swift
@EnvironmentObject var bridge: BridgeManager

// Bridge status (connected, discovering, etc.)
bridge.status

// Working directory from macOS (String path)
bridge.workingDirectory

// Connection details
case .connected(host: String, port: Int)
```

View is entirely reactive - when BridgeManager state changes, UI automatically updates.

## User Flow

### First Launch

1. App opens, shows simplified screen
2. Bridge auto-connects to last known Mac or discovers via Bonjour
3. Status changes: Idle → Discovering → Connecting → Connected
4. Working directory appears when handshake completes
5. Enabled agents shown with checkmarks

### Typical Use

After initial connection, this screen is mostly passive:
- Bridge status confirms connection to Mac
- Working directory shows where agents are operating
- Agent list shows what's available

User typically navigates away from this screen to actually interact with agents. The home screen is just a "system status" dashboard.

### Reconnection

If connection drops:
1. Status changes to "Discovering..." or "Connecting..."
2. Working directory and agents remain displayed (last known state)
3. When reconnected, working directory updates if changed on Mac

## Design Decisions Deep Dive

### Why Read-Only on iOS?

**Everything on this screen is read-only** because:

1. **Single source of truth**: macOS is the server, it owns the configuration
2. **Prevents sync conflicts**: If iOS could change settings, we'd need two-way sync
3. **Clearer mental model**: "Configure on Mac, monitor on iPhone"
4. **Simpler implementation**: No need for setting change RPCs, just display what Mac sends

If users want to change working directory or toggle agents, they open the macOS app.

### Why No "Disconnect" Button?

The bridge connection is **automatic and persistent**:
- Connects on app launch
- Reconnects automatically if dropped
- No manual control needed

Adding a disconnect button would imply users should manage the connection, but we want it to "just work" in the background.

### Why Show Working Directory?

Even though it's read-only, displaying the working directory is valuable:

1. **Verification**: Users can confirm agents are operating in the correct project
2. **Context**: When viewing agent output, knowing the working directory provides context
3. **Debugging**: If something goes wrong, working directory helps diagnose issues

### Why Berkeley Mono on iOS?

Originally, iOS used Inter (system font) while macOS used Berkeley Mono. We switched iOS to Berkeley Mono for:

1. **Brand consistency**: Same font across platforms reinforces identity
2. **Developer aesthetic**: Berkeley Mono screams "developer tool"
3. **Visual coherence**: Code snippets and UI text in the same font family
4. **Distinctive look**: We don't look like every other iOS app

Trade-off: Berkeley Mono is less readable than Inter at small sizes, but we use 14-16pt throughout, so readability is fine.

## Accessibility

### VoiceOver

All elements have semantic labels:
- Bridge status announces: "Bridge Connection, Connected, 127.0.0.1:9099"
- Working directory announces: "Working Directory, openagents, /Users/christopherdavid/code/openagents"
- Agent rows announce: "OpenAI Codex, Managed on macOS, enabled"

### Dynamic Type

Font sizes scale with user's preferred text size:
- Title: `.largeTitle` (32pt base, scales up to ~40pt)
- Section headers: `.headline` (16pt base)
- Body text: `.body` (14-15pt base)

### Color Contrast

All text meets WCAG AA contrast requirements:
- Primary text (#f7f8f8) on background (#08090a): 19.1:1
- Secondary text (#d0d6e0) on background: 14.8:1
- Tertiary text (#8a8f98) on background: 8.4:1

## Performance

### Lightweight Rendering

This screen is fast because:
- ScrollView with VStack (efficient for small lists)
- No complex layouts (just VStacks and HStacks)
- Minimal state updates (only when bridge status changes)
- No heavy computation

### State Synchronization

Working directory and connection status update via BridgeManager's `@Published` properties:
- Automatic UI refresh when values change
- No manual observation needed (SwiftUI handles it)
- Changes propagate instantly from bridge layer to UI

### Memory Footprint

Minimal memory usage:
- No cached chat history (not needed on home screen)
- No images or media (just SF Symbols)
- String properties only (working directory path, host, port)

## Testing

### Manual Testing Checklist

- [ ] Toolbar displays with hamburger menu and "Home" title
- [ ] Bridge status shows "Discovering..." on launch
- [ ] Green checkmark appears when connected
- [ ] Host and port display correctly when connected
- [ ] Working directory shows path received from macOS
- [ ] Working directory shows "Not set" when nil
- [ ] Agent list displays OpenAI Codex and Claude Code
- [ ] All fonts use Berkeley Mono
- [ ] Status card height stays fixed during state changes
- [ ] ScrollView scrolls smoothly on small screens

### Edge Cases

**No working directory**: If macOS hasn't set a working directory, iOS shows "Not set" with helpful message.

**Connection drops**: Status changes to "Discovering..." or error state, but working directory/agents remain displayed (last known state).

**Very long paths**: Working directory path truncates with ellipsis in middle (`.truncationMode(.middle)`) to keep folder name visible.

**Small screens**: ScrollView allows vertical scrolling if content exceeds screen height (iPhone SE, etc.).

## Future Enhancements

### Potential Additions

1. **Settings Button**: Access app settings from toolbar (right side)
2. **Connection History**: Show recent connections in menu sheet
3. **Quick Actions**: Shortcuts to common tasks (not yet defined)
4. **Status Notifications**: Push notifications when agents complete tasks
5. **Agent Details**: Tap agent to see more info (version, capabilities, etc.)

### What We Won't Add

1. **Working Directory Picker**: That's macOS's job, not iOS
2. **Agent Configuration**: Enable/disable is on macOS only
3. **Session Management**: Handled in other screens (not home)
4. **File Browser**: Use macOS for file operations

If users want these features, they can use the macOS app or we can add them to non-home iOS screens.

## Comparison to Full UI

### What Was Removed

The original iOS UI had:
- NavigationStack with complex toolbar
- Bridge status banner at top
- "Start" button to trigger workspace exploration
- Streamed ACP updates list with tool calls and progress
- Plan state view showing exploration steps
- FM analysis status with progress percentage
- JSON inspector for debugging tool calls

All of this still exists in ChatHomeView (behind the feature flag).

### What Was Kept

The simplified UI still uses:
- BridgeManager for connection state
- NavigationStack for future navigation
- ACP protocol (just not rendered in the UI)
- Theme and font systems
- Toolbar pattern (ChatHeaderToolbar)

### Migration Path

Users who want the full UI can re-enable it:

```bash
# Temporarily
OPENAGENTS_SIMPLIFIED_IOS=0 open OpenAgents.app

# Permanently
defaults write com.openagents.desktop simplified_ios_ui -bool false
```

We may eventually add a settings toggle: "Advanced Mode" to switch between simplified and full UI at runtime.

## Conclusion

The simplified iOS home screen is a deliberate exercise in **mobile-appropriate design**. By removing configuration and focusing on status display, we've created an interface that's:

- **Fast to understand** - Three sections, all read-only information
- **Easy to scan** - Bridge status at top, details below
- **Honest about limitations** - "Managed on macOS" makes it clear where configuration happens
- **Consistent with macOS** - Same font, same colors, same design language

This is the iOS counterpart to the simplified macOS home. Together, they form a coherent system where:
- **macOS** = Configuration & hosting
- **iOS** = Monitoring & interaction

**Simplicity on iOS isn't a limitation. It's recognition that mobile and desktop serve different purposes.**
