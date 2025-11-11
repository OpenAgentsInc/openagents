# Simplified macOS UI Implementation

**Date**: November 7, 2025
**Author**: Claude (Sonnet 4.5)
**Status**: ✅ Completed and Building

## Context

The macOS app previously showed a full chat history UI with a NavigationSplitView containing:
- Left sidebar: List of recent Claude Code and Codex sessions with automatic discovery
- Right detail pane: Full ACP thread view showing conversation history
- Bridge status chip at the top of the sidebar
- Automatic title generation using Foundation Models

The request was to hide all of this complexity and show only:
1. Bridge status (prominent, with spinner or success indicator)
2. Link to setup instructions
3. Working directory selector

The goal is to simplify the macOS UI to focus on configuration and bridge management, treating the Mac as a "command center" for mobile agents rather than a full chat interface.

## Requirements Gathered

Through clarifying questions, established:
- **Instructions link behavior**: Show modal/sheet with setup instructions
- **Post-directory selection**: Just set the directory and wait (no auto-start)
- **Thread view visibility**: Hide it completely for now
- **Code preservation approach**: Wrap in conditional/flag (easy to restore)

## Implementation

### 1. Feature Flag (`Features.swift`)

Added a new feature flag to control UI mode:

```swift
static var simplifiedMacOSUI: Bool {
    if ProcessInfo.processInfo.environment["OPENAGENTS_SIMPLIFIED_MACOS"] == "0" { return false }
    if UserDefaults.standard.object(forKey: "simplified_macos_ui") != nil {
        return UserDefaults.standard.bool(forKey: "simplified_macos_ui")
    }
    return true // Default to simplified UI
}
```

**Design rationale**:
- Default: `true` (show simplified UI out of the box)
- Can be disabled via env var `OPENAGENTS_SIMPLIFIED_MACOS=0`
- Can be toggled via UserDefaults `simplified_macos_ui` key
- Consistent with existing feature flag patterns in the codebase

**File**: `ios/OpenAgents/Features.swift`

### 2. Working Directory State (`BridgeManager.swift`)

Extended `BridgeManager` with working directory persistence (macOS only):

```swift
#if os(macOS)
@Published var workingDirectory: URL? = nil
private static let workingDirectoryKey = "oa.bridge.working_directory"

func setWorkingDirectory(_ url: URL) {
    workingDirectory = url
    saveWorkingDirectory(url)
}

func loadWorkingDirectory() {
    if let path = UserDefaults.standard.string(forKey: Self.workingDirectoryKey) {
        let url = URL(fileURLWithPath: path)
        if FileManager.default.fileExists(atPath: path) {
            workingDirectory = url
            log("workdir", "Loaded working directory: \(path)")
        }
    }
}

private func saveWorkingDirectory(_ url: URL) {
    UserDefaults.standard.set(url.path, forKey: Self.workingDirectoryKey)
    log("workdir", "Saved working directory: \(url.path)")
}
#endif
```

**Design rationale**:
- `@Published` for SwiftUI observation (reactive UI updates)
- Persists to UserDefaults for session continuity
- Validates path existence on load (handles moved/deleted directories gracefully)
- Integrated logging for debugging
- macOS-only (iOS doesn't need this concept)

Modified `start()` to auto-load persisted directory on bridge initialization.

**File**: `ios/OpenAgents/Bridge/BridgeManager.swift`

### 3. Setup Instructions Sheet (`BridgeSetupInstructionsSheet.swift`)

Created a new modal sheet component with:

**Content structure**:
- Header: "Bridge Setup" with subtitle
- Status section: Current bridge status with colored indicator (green/blue/yellow/red)
- Numbered instructions (4 steps):
  1. Ensure Bridge is Running
  2. Connect Devices to Same Network
  3. Open OpenAgents on iOS
  4. Manual Connection (Optional)
- Troubleshooting section: Bullet points for common issues

**Key implementation details**:
```swift
struct BridgeSetupInstructionsSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var bridge: BridgeManager

    // ... status color logic
    private var statusColor: Color {
        switch bridge.status {
        case .connected: return OATheme.Colors.success
        case .advertising: return .blue.opacity(0.8)
        case .connecting, .handshaking: return .yellow.opacity(0.8)
        case .error: return OATheme.Colors.danger
        default: return OATheme.Colors.textTertiary
        }
    }
}
```

**Platform considerations**:
- Used `#if os(iOS)` guard for `.navigationBarTitleDisplayMode(.inline)` (not available on macOS)
- NavigationStack container for consistent presentation
- Dark mode optimized (`.preferredColorScheme(.dark)`)

**File**: `ios/OpenAgents/Bridge/BridgeSetupInstructionsSheet.swift`

### 4. Simplified Main View (`SimplifiedMacOSView.swift`)

Created the new main macOS UI with three sections:

#### Layout Strategy
- Centered vertical stack with spacers
- Maximum widths for focused content (400-500px)
- Consistent 40px padding
- Dark background with subtle rounded rectangles

#### Bridge Status Section
```swift
HStack(spacing: 16) {
    if case .advertising = bridge.status {
        // Static checkmark when advertising (success state)
        Image(systemName: "checkmark.circle.fill")
            .font(.system(size: 20))
            .foregroundStyle(OATheme.Colors.success)
    } else {
        // Loading spinner otherwise
        ProgressView()
            .scaleEffect(0.8)
    }

    VStack(alignment: .leading, spacing: 4) {
        Text(bridgeStatusText)
        // Port info and connected client count
    }
}
```

**Design rationale**:
- Visual distinction: spinner (connecting) vs checkmark (ready)
- Shows port number when advertising
- Shows connected client count when iOS devices are paired
- Includes error messages with red styling
- "View Setup Instructions" button below status card

#### Working Directory Section
```swift
if let dir = bridge.workingDirectory {
    // Show selected directory with icon, name, path, and "Change" button
} else {
    // Show empty state with prompt to select
    Button("Select Directory") {
        selectWorkingDirectory()
    }
    .buttonStyle(.borderedProminent)
    .controlSize(.large)
}
```

**Implementation notes**:
- Uses `NSOpenPanel` for directory picker (macOS only)
- Displays folder icon, name, and truncated path
- Persists selection automatically via `BridgeManager.setWorkingDirectory()`
- Empty state prominently encourages selection

**Platform guards**:
```swift
#if os(macOS)
import AppKit

struct SimplifiedMacOSView: View {
    // ...
    private func selectWorkingDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        // ...
    }
}
#endif
```

**File**: `ios/OpenAgents/SimplifiedMacOSView.swift`

### 5. ContentView Integration (`ContentView.swift`)

Modified the main ContentView to conditionally switch between UIs:

#### Before
```swift
#else
NavigationSplitView {
    HistorySidebar(selected: selectedRow, onSelect: { row, url in
        // ...
    })
} detail: {
    AcpThreadView(url: selectedURL, onTitleChange: { t in
        // ...
    })
}
#endif
```

#### After
```swift
#else
// macOS: conditionally show simplified UI or full navigation split view
if Features.simplifiedMacOSUI {
    SimplifiedMacOSView()
} else {
    NavigationSplitView {
        HistorySidebar(selected: selectedRow, onSelect: { row, url in
            // ...
        })
    } detail: {
        AcpThreadView(url: selectedURL, onTitleChange: { t in
            // ...
        })
    }
}
#endif
```

**Additional changes**:
- Toolbar visibility: `.toolbar(Features.simplifiedMacOSUI ? .hidden : .visible, for: .windowToolbar)`
- TopEdgeGradient: Only shown when not in simplified mode (no toolbar to blur with)

**Design rationale**:
- **Zero code deletion**: All existing UI code remains intact
- Simple boolean check for UI switching
- Future-proof: Can add more UI modes if needed
- Easy testing: Toggle feature flag to verify both modes

**File**: `ios/OpenAgents/ContentView.swift`

## Files Changed/Created

### Created
- ✅ `ios/OpenAgents/SimplifiedMacOSView.swift` (213 lines)
- ✅ `ios/OpenAgents/Bridge/BridgeSetupInstructionsSheet.swift` (180 lines)

### Modified
- ✅ `ios/OpenAgents/Features.swift` (+9 lines)
- ✅ `ios/OpenAgents/Bridge/BridgeManager.swift` (+24 lines macOS section)
- ✅ `ios/OpenAgents/ContentView.swift` (~15 lines for conditionals)

### Unchanged (but preserved behind flag)
- `ios/OpenAgents/HistorySidebar.swift` - Full chat history list
- `ios/OpenAgents/AcpThreadView.swift` - Conversation detail view
- All discovery logic (`LocalClaudeDiscovery`, `LocalCodexDiscovery`)
- All ACP renderers and components

## Build Results

```bash
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -sdk macosx -configuration Debug build
```

**Status**: ✅ **BUILD SUCCEEDED**

Initial build failed with:
```
error: 'navigationBarTitleDisplayMode' is unavailable in macOS
```

**Fix**: Added `#if os(iOS)` guard around the modifier in `BridgeSetupInstructionsSheet.swift`.

Second build: **Successful** with code signing and validation complete.

## User Experience

### On Launch
1. macOS app starts, bridge begins advertising automatically
2. Simplified UI loads showing:
   - Bridge status: "Ready for Connections" with green checkmark (if advertising)
   - Working directory: Empty state prompting selection
3. User can click "View Setup Instructions" for pairing help
4. User clicks "Select Directory" to choose agent working directory
5. Selection persists across app restarts

### When iOS Connects
- Bridge status shows: "(1 client connected)" under port number
- macOS UI remains the same (no session management UI)
- Future: Could add session activity indicators

### To Restore Full UI
Users/developers can restore the original chat history UI:

**Option 1 - UserDefaults**:
```bash
defaults write com.openagents.desktop simplified_macos_ui -bool false
```

**Option 2 - Environment Variable**:
```bash
export OPENAGENTS_SIMPLIFIED_MACOS=0
open OpenAgents.app
```

**Option 3 - Code Change**:
```swift
// Features.swift
static var simplifiedMacOSUI: Bool {
    return false // Force full UI
}
```

## Architecture Decisions

### Why Feature Flag Instead of Separate Target?
- **Simplicity**: Single codebase, one build target
- **Testing**: Easy to test both modes without changing build configs
- **Maintenance**: Less Xcode project complexity
- **Future flexibility**: Can add more modes (e.g., "power user" mode)

### Why Persist Working Directory?
- **User expectation**: Directory selection should persist like other preferences
- **Workflow continuity**: User shouldn't re-select on every launch
- **Foundation for future**: When agent sessions start, they'll need this directory

### Why Modal Sheet for Instructions?
- **Non-intrusive**: Doesn't clutter main UI
- **Focused content**: User explicitly requests setup help
- **Dismissible**: Easy to close and return to main UI
- **Reusable**: Could be triggered from menu bar or other contexts

### Why Separate View File Instead of Inline?
- **Clarity**: ContentView doesn't get cluttered with simplified logic
- **Maintainability**: Easier to iterate on simplified UI independently
- **Platform separation**: Clear `#if os(macOS)` boundary
- **Preview support**: Can preview SimplifiedMacOSView in isolation

## Future Enhancements

### Near-term
- [ ] Add "Active Sessions" section showing running agent processes
- [ ] Add "Recent Activity" timeline (last N agent actions)
- [ ] Add bridge logs viewer (expand BridgeStatusChip to full log panel)
- [ ] Add working directory validation (check for git repo, warn if system directory)

### Medium-term
- [ ] Add session management: Start/stop agent sessions from UI
- [ ] Add prompt input field to start new agent session
- [ ] Add iOS device list showing all connected clients
- [ ] Add pairing codes/tokens for security (ADR-0004 mentions future TLS)

### Long-term
- [ ] Add desktop session UI (chat interface on Mac as alternative to CLI)
- [ ] Add settings panel for bridge configuration (port, service name, etc.)
- [ ] Add metrics dashboard (session count, messages sent, etc.)
- [ ] Multi-user support (different working directories per iOS client)

## Testing Notes

### Manual Testing Required
- [x] Build succeeds on macOS
- [ ] App launches and shows simplified UI
- [ ] Bridge status shows "Ready for Connections" when advertising
- [ ] "View Setup Instructions" opens modal with correct content
- [ ] "Select Directory" opens NSOpenPanel
- [ ] Selected directory persists after app restart
- [ ] Feature flag toggle switches between simplified/full UI
- [ ] iOS connection updates client count in bridge status

### Automated Testing (Future)
- Unit tests for `BridgeManager` working directory persistence
- UI tests for directory picker flow
- Snapshot tests for SimplifiedMacOSView appearance
- Integration tests for feature flag switching

## Compatibility

- **Minimum macOS**: 13.0 (same as project minimum)
- **iOS Impact**: None (all changes guarded with `#if os(macOS)`)
- **Breaking Changes**: None (default behavior changes, but old UI available via flag)
- **Dependencies**: No new external dependencies

## References

- **ADR-0004**: iOS ↔ Desktop WebSocket Bridge and Pairing
- **ADR-0005**: Adopt Liquid Glass for Apple Platforms (UI theme consistency)
- **Project Guidelines**: `CLAUDE.md` - Swift style, feature flags, build discipline

## Commit Message

```
Implement simplified macOS UI with bridge status and working directory selection

Add feature flag `Features.simplifiedMacOSUI` (default: true) to toggle between
simplified configuration UI and full chat history UI on macOS.

New simplified UI shows:
- Bridge connection status with visual indicators
- Link to setup instructions modal
- Working directory selector with persistence

All existing chat history code preserved behind feature flag for easy restoration.

Files created:
- SimplifiedMacOSView.swift - Main simplified UI
- BridgeSetupInstructionsSheet.swift - Setup guide modal

Files modified:
- Features.swift - Added simplifiedMacOSUI flag
- BridgeManager.swift - Added working directory state (macOS)
- ContentView.swift - Conditional UI switching

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
```

## Lessons Learned

1. **Platform-specific SwiftUI modifiers require guards**: `.navigationBarTitleDisplayMode()` is iOS-only
2. **Feature flags are powerful**: Entire UI modes can be toggled without code deletion
3. **@Published + UserDefaults = Good UX**: Reactive state + persistence gives instant feedback with continuity
4. **Xcode project vs workspace**: Initially tried to build with workspace (doesn't exist), project file works
5. **Clarifying questions save time**: Understanding exact requirements prevented multiple iterations

## Notes for Future Maintainers

- The full chat history UI (`HistorySidebar`, `AcpThreadView`) is **not deprecated**, just hidden by default
- To add features to simplified UI, modify `SimplifiedMacOSView.swift`
- To modify full UI, work in `HistorySidebar.swift` and `ContentView.swift` with flag check
- Working directory is **not yet used** by agent sessions (future integration needed)
- BridgeManager already has all the bridge logic; UI just surfaces it differently

---

**Conclusion**: The simplified macOS UI provides a clean, focused entry point for users while preserving all existing functionality behind a feature flag. The implementation follows Swift and SwiftUI best practices, maintains platform separation, and builds successfully.
