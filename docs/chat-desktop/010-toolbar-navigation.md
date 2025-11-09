# Issue #10: Add Toolbar Navigation to Settings/Developer

## Phase
Phase 3: Settings & Developer Pages

## Priority
Low - Supporting feature

## Description
Add toolbar buttons and menu items to access the Settings and Developer views from the main chat interface.

## Current State
- Settings and Developer views created in Issues #8 and #9
- No navigation to access them from main interface
- Need macOS-native toolbar integration

## Target State
- Toolbar buttons for Settings (gear icon) and Developer (wrench icon)
- Keyboard shortcuts:
  - ⌘, for Settings
  - ⌘⌥D for Developer
- Menu bar items under app menu
- Proper window management (sheets vs separate windows)

## Acceptance Criteria
- [ ] Add Settings button to toolbar (gear icon)
- [ ] Add Developer button to toolbar or menu
- [ ] Keyboard shortcuts ⌘, and ⌘⌥D work
- [ ] Menu items in app menu (OpenAgents → Settings, Developer Tools)
- [ ] Settings opens as sheet
- [ ] Developer opens as sheet or separate window (configurable)
- [ ] Proper window state management (only one instance at a time)

## Technical Details

### Toolbar Implementation
```swift
// In ChatMacOSView.swift
@State private var showSettings = false
@State private var showDeveloper = false

var body: some View {
    NavigationSplitView(...) {
        // Main content
    }
    .toolbar {
        ToolbarItemGroup(placement: .navigation) {
            AgentSelectorView()
        }

        ToolbarItemGroup(placement: .automatic) {
            // New Chat button
            Button(action: createNewChat) {
                Image(systemName: "square.and.pencil")
            }
            .help("New Chat")
            .keyboardShortcut("n", modifiers: .command)

            Divider()

            // Settings button
            Button(action: { showSettings = true }) {
                Image(systemName: "gearshape")
            }
            .help("Settings")
            .keyboardShortcut(",", modifiers: .command)

            // Developer button (in menu only, not toolbar)
        }
    }
    .sheet(isPresented: $showSettings) {
        SettingsView()
            .environmentObject(bridgeManager)
            .environmentObject(tinyvexManager)
    }
    .sheet(isPresented: $showDeveloper) {
        DeveloperView()
            .environmentObject(bridgeManager)
            .environmentObject(tinyvexManager)
    }
}

private func createNewChat() {
    Task {
        await bridgeManager.startNewSession()
    }
}
```

### Menu Bar Integration
```swift
// In OpenAgentsApp.swift or dedicated Commands file
import SwiftUI

struct OpenAgentsCommands: Commands {
    @FocusedBinding(\.showSettings) var showSettings: Bool?
    @FocusedBinding(\.showDeveloper) var showDeveloper: Bool?

    var body: some Commands {
        // Replace default preferences with custom settings
        CommandGroup(replacing: .appSettings) {
            Button("Settings...") {
                showSettings? = true
            }
            .keyboardShortcut(",", modifiers: .command)
        }

        // Developer menu
        CommandMenu("Developer") {
            Button("Developer Tools...") {
                showDeveloper? = true
            }
            .keyboardShortcut("d", modifiers: [.command, .option])

            Divider()

            Button("Open Logs Folder") {
                openLogsFolder()
            }

            Button("Reset Application Data...") {
                resetApplicationData()
            }
        }

        // Help menu additions
        CommandGroup(after: .help) {
            Button("Report Issue...") {
                openURL("https://github.com/OpenAgentsInc/openagents/issues")
            }

            Button("View Documentation") {
                openURL("https://github.com/OpenAgentsInc/openagents/docs")
            }
        }
    }

    private func openLogsFolder() {
        // Open logs directory in Finder
        if let logsURL = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask).first?
            .appendingPathComponent("Logs")
            .appendingPathComponent("OpenAgents") {
            NSWorkspace.shared.open(logsURL)
        }
    }

    private func resetApplicationData() {
        // Show confirmation dialog
        let alert = NSAlert()
        alert.messageText = "Reset Application Data?"
        alert.informativeText = "This will delete all sessions, settings, and cached data. This action cannot be undone."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Reset")

        if alert.runModal() == .alertSecondButtonReturn {
            // Perform reset
            UserDefaults.standard.removePersistentDomain(forName: Bundle.main.bundleIdentifier!)
            // Clear database
            // Restart app
        }
    }

    private func openURL(_ urlString: String) {
        if let url = URL(string: urlString) {
            NSWorkspace.shared.open(url)
        }
    }
}

// In OpenAgentsApp.swift
@main
struct OpenAgentsApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .commands {
            OpenAgentsCommands()
        }
    }
}
```

### Focused Bindings Setup
```swift
// Create FocusedValues extensions
extension FocusedValues {
    struct ShowSettingsKey: FocusedValueKey {
        typealias Value = Binding<Bool>
    }

    struct ShowDeveloperKey: FocusedValueKey {
        typealias Value = Binding<Bool>
    }

    var showSettings: Binding<Bool>? {
        get { self[ShowSettingsKey.self] }
        set { self[ShowSettingsKey.self] = newValue }
    }

    var showDeveloper: Binding<Bool>? {
        get { self[ShowDeveloperKey.self] }
        set { self[ShowDeveloperKey.self] = newValue }
    }
}

// In ChatMacOSView
.focusedSceneValue(\.showSettings, $showSettings)
.focusedSceneValue(\.showDeveloper, $showDeveloper)
```

### Alternative: Separate Windows
If separate windows are preferred over sheets:

```swift
// Settings Window
struct SettingsWindowGroup: Scene {
    var body: some Scene {
        Window("Settings", id: "settings") {
            SettingsView()
                .frame(minWidth: 700, minHeight: 500)
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .keyboardShortcut(",", modifiers: .command)
    }
}

// Developer Window
struct DeveloperWindowGroup: Scene {
    var body: some Scene {
        Window("Developer Tools", id: "developer") {
            DeveloperView()
                .frame(minWidth: 800, minHeight: 600)
        }
        .keyboardShortcut("d", modifiers: [.command, .option])
    }
}

// In OpenAgentsApp
@main
struct OpenAgentsApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }

        SettingsWindowGroup()
        DeveloperWindowGroup()
    }
}
```

## Dependencies
- Issue #8 (Settings view)
- Issue #9 (Developer view)

## Blocked By
- Issue #8
- Issue #9

## Blocks
None - Navigation enhancement

## Estimated Complexity
Low-Medium (2-3 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] Toolbar buttons appear and work
- [ ] Keyboard shortcuts work (⌘, and ⌘⌥D)
- [ ] Menu items appear in app menu
- [ ] Settings sheet opens correctly
- [ ] Developer sheet/window opens correctly
- [ ] Only one instance of each window at a time
- [ ] Help menu links open in browser

## References
- SwiftUI Commands: https://developer.apple.com/documentation/swiftui/commands
- macOS toolbar patterns: System apps (Mail, Safari) for UX reference
- FocusedValues: https://developer.apple.com/documentation/swiftui/focusedvalues
