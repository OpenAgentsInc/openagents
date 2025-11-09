# Issue #13: Add Keyboard Shortcuts and Accessibility Features

## Phase
Phase 4: Integration & Features

## Priority
Medium - UX and accessibility enhancement

## Description
Implement comprehensive keyboard shortcuts and accessibility features to make the macOS app fully keyboard-navigable and accessible.

## Current State
- Some keyboard shortcuts defined in individual components
- No unified keyboard shortcut scheme
- Accessibility labels/hints not implemented
- No keyboard-only navigation support

## Target State
- Complete keyboard shortcut scheme (⌘N, ⌘K, ⌘I, etc.)
- Full keyboard navigation (tab, arrow keys)
- VoiceOver support with proper labels
- Keyboard shortcuts help screen (⌘?)
- Focus management for optimal UX
- Escape key behaviors (close sheets, cancel actions)

## Acceptance Criteria
- [ ] All major actions have keyboard shortcuts
- [ ] Keyboard shortcuts documented in help screen
- [ ] Full keyboard navigation with Tab/Shift+Tab
- [ ] Arrow key navigation in session sidebar
- [ ] VoiceOver labels for all interactive elements
- [ ] Focus indicators visible and clear
- [ ] Escape key closes sheets/popovers
- [ ] Return key activates default actions
- [ ] Accessibility audit passes (Xcode Accessibility Inspector)

## Technical Details

### Keyboard Shortcuts Reference

#### Global Shortcuts
```swift
// In ChatMacOSView.swift or Commands
.keyboardShortcut("n", modifiers: .command) // New chat
.keyboardShortcut("k", modifiers: .command) // Agent selector
.keyboardShortcut("b", modifiers: .command) // Toggle sidebar
.keyboardShortcut("i", modifiers: .command) // Toggle inspector
.keyboardShortcut(",", modifiers: .command) // Settings
.keyboardShortcut("d", modifiers: [.command, .option]) // Developer tools
.keyboardShortcut("w", modifiers: .command) // Close window
.keyboardShortcut("f", modifiers: .command) // Search sessions
.keyboardShortcut("/", modifiers: .command) // Show shortcuts help
```

#### Context-Specific Shortcuts
```swift
// In Composer
.keyboardShortcut(.return, modifiers: []) // Send message (Return)
// Shift+Return for newline (handled in NSTextView delegate)

// In Session Sidebar
.keyboardShortcut(.upArrow, modifiers: []) // Previous session
.keyboardShortcut(.downArrow, modifiers: []) // Next session
.keyboardShortcut(.delete, modifiers: []) // Delete selected session

// In Message View
.keyboardShortcut("c", modifiers: .command) // Copy message
.keyboardShortcut("a", modifiers: .command) // Select all text
```

### Keyboard Shortcuts Help Screen
```swift
// ios/OpenAgents/Views/macOS/KeyboardShortcutsView.swift
struct KeyboardShortcutsView: View {
    @Environment(\.dismiss) var dismiss

    struct ShortcutGroup {
        let title: String
        let shortcuts: [KeyboardShortcutInfo]
    }

    struct KeyboardShortcutInfo {
        let keys: String
        let action: String
    }

    private let shortcutGroups: [ShortcutGroup] = [
        ShortcutGroup(title: "General", shortcuts: [
            KeyboardShortcutInfo(keys: "⌘N", action: "New chat"),
            KeyboardShortcutInfo(keys: "⌘K", action: "Select agent"),
            KeyboardShortcutInfo(keys: "⌘,", action: "Settings"),
            KeyboardShortcutInfo(keys: "⌘W", action: "Close window"),
            KeyboardShortcutInfo(keys: "⌘/", action: "Show keyboard shortcuts"),
        ]),
        ShortcutGroup(title: "Navigation", shortcuts: [
            KeyboardShortcutInfo(keys: "⌘B", action: "Toggle session sidebar"),
            KeyboardShortcutInfo(keys: "⌘I", action: "Toggle inspector"),
            KeyboardShortcutInfo(keys: "⌘F", action: "Search sessions"),
            KeyboardShortcutInfo(keys: "↑/↓", action: "Navigate sessions"),
            KeyboardShortcutInfo(keys: "Tab", action: "Navigate interface"),
        ]),
        ShortcutGroup(title: "Messaging", shortcuts: [
            KeyboardShortcutInfo(keys: "Return", action: "Send message"),
            KeyboardShortcutInfo(keys: "⇧Return", action: "New line"),
            KeyboardShortcutInfo(keys: "Esc", action: "Cancel/close"),
        ]),
        ShortcutGroup(title: "Developer", shortcuts: [
            KeyboardShortcutInfo(keys: "⌥⌘D", action: "Developer tools"),
            KeyboardShortcutInfo(keys: "⌥⌘L", action: "View logs"),
        ]),
    ]

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Keyboard Shortcuts")
                    .font(OAFonts.mono(size: 18, weight: .semibold))
                    .foregroundColor(OATheme.Colors.textPrimary)

                Spacer()

                Button(action: { dismiss() }) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(OATheme.Colors.textSecondary)
                }
                .buttonStyle(.plain)
                .keyboardShortcut(.escape)
            }
            .padding()

            Divider()

            // Shortcuts grid
            ScrollView {
                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 24) {
                    ForEach(shortcutGroups, id: \.title) { group in
                        VStack(alignment: .leading, spacing: 12) {
                            Text(group.title)
                                .font(OAFonts.mono(size: 12, weight: .semibold))
                                .foregroundColor(OATheme.Colors.textSecondary)
                                .textCase(.uppercase)

                            VStack(alignment: .leading, spacing: 8) {
                                ForEach(group.shortcuts, id: \.keys) { shortcut in
                                    ShortcutRowView(shortcut: shortcut)
                                }
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .padding()
            }

            Divider()

            // Footer
            HStack {
                Spacer()
                Button("Done") {
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
            }
            .padding()
        }
        .frame(width: 700, height: 500)
        .background(Material.ultraThin)
    }
}

struct ShortcutRowView: View {
    let shortcut: KeyboardShortcutsView.KeyboardShortcutInfo

    var body: some View {
        HStack {
            Text(shortcut.keys)
                .font(OAFonts.mono(size: 13, weight: .semibold))
                .foregroundColor(OATheme.Colors.accent)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(OATheme.Colors.background.opacity(0.5))
                .cornerRadius(6)
                .frame(width: 80, alignment: .leading)

            Text(shortcut.action)
                .font(OAFonts.mono(size: 13))
                .foregroundColor(OATheme.Colors.textPrimary)
        }
    }
}
```

### Show Shortcuts Help
```swift
// In ChatMacOSView
@State private var showKeyboardShortcuts = false

.keyboardShortcut("/", modifiers: .command)
.sheet(isPresented: $showKeyboardShortcuts) {
    KeyboardShortcutsView()
}

// Also add to Help menu
CommandMenu("Help") {
    Button("Keyboard Shortcuts") {
        showKeyboardShortcuts = true
    }
    .keyboardShortcut("/", modifiers: .command)
}
```

### Keyboard Navigation

#### Session Sidebar Navigation
```swift
// In SessionSidebarView.swift
@FocusState private var focusedSessionId: String?
@State private var selectedSessionId: String?

List(sessions, id: \.id, selection: $selectedSessionId) { session in
    SessionRowView(session: session)
        .focused($focusedSessionId, equals: session.id)
}
.onKeyPress(.upArrow) {
    navigateSessions(direction: .up)
    return .handled
}
.onKeyPress(.downArrow) {
    navigateSessions(direction: .down)
    return .handled
}
.onKeyPress(.return) {
    if let selectedId = selectedSessionId {
        bridgeManager.loadSession(selectedId)
    }
    return .handled
}
.onKeyPress(.delete) {
    if let selectedId = selectedSessionId {
        deleteSession(selectedId)
    }
    return .handled
}

private func navigateSessions(direction: Direction) {
    guard let currentId = focusedSessionId,
          let currentIndex = sessions.firstIndex(where: { $0.id == currentId }) else {
        focusedSessionId = sessions.first?.id
        return
    }

    let nextIndex = direction == .up ? currentIndex - 1 : currentIndex + 1
    if sessions.indices.contains(nextIndex) {
        focusedSessionId = sessions[nextIndex].id
        selectedSessionId = sessions[nextIndex].id
    }
}
```

### Accessibility Labels

#### Session Sidebar
```swift
// In SessionRowView.swift
.accessibilityLabel("Chat session: \(session.title)")
.accessibilityHint("Double-tap to open this chat session")
.accessibilityValue("Last updated \(relativeTimestamp(session.timestamp))")
.accessibilityAddTraits(isActive ? [.isSelected] : [])
```

#### Message Bubbles
```swift
// In MessageBubbleView.swift
.accessibilityLabel("\(roleLabel) message")
.accessibilityValue(messageText)
.accessibilityHint("Double-tap to view details")
```

#### Tool Calls
```swift
// In ToolCallView.swift
.accessibilityLabel("Tool call: \(toolName)")
.accessibilityValue("Status: \(status.description)")
.accessibilityHint("Double-tap to view execution details")
```

#### Composer
```swift
// In ComposerMac.swift
.accessibilityLabel("Message input")
.accessibilityHint("Type your message and press Return to send")
.accessibilityValue(text)
```

### Focus Management

#### Auto-focus composer on new chat
```swift
// In ChatAreaView.swift
@FocusState private var isComposerFocused: Bool

ComposerMac(...)
    .focused($isComposerFocused)

.onChange(of: bridgeManager.currentSessionId) { _ in
    // Focus composer when switching sessions
    isComposerFocused = true
}

.onAppear {
    // Focus composer on first load
    isComposerFocused = true
}
```

#### Focus trapping in modals
```swift
// In SettingsView, DeveloperView, etc.
.onAppear {
    // Trap focus within modal
}
.keyboardShortcut(.escape) // Close on Escape
```

### Accessibility Testing
Run Xcode Accessibility Inspector:
1. Product > Analyze > Accessibility
2. Fix any warnings/errors
3. Test with VoiceOver enabled (⌘F5)
4. Verify all interactive elements are announced correctly

## Dependencies
- All UI components (Issues #1-#7)

## Blocked By
None - Can be implemented incrementally

## Blocks
None - UX enhancement

## Estimated Complexity
Medium (4-5 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] All keyboard shortcuts work as documented
- [ ] Help screen displays all shortcuts
- [ ] Tab navigation flows logically through UI
- [ ] Arrow key navigation works in sidebar
- [ ] VoiceOver announces all elements correctly
- [ ] Focus indicators visible
- [ ] Escape closes sheets/popovers
- [ ] Return activates default actions
- [ ] No keyboard traps (can always navigate away)
- [ ] Accessibility audit in Xcode passes

## Status Update — Implemented (macOS)
- Implemented sidebar keyboard navigation via `List(selection:)` and selection handling.
- Wired Delete to remove selected session (confirmation alert), bound to Delete key via Commands.
- Added Cmd-B (Toggle Sidebar) and Cmd-/ (Keyboard Shortcuts…) menu commands.
- Implemented `KeyboardShortcutsView` and sheet presentation.
- Added basic accessibility labels to key controls (New Chat, Agent selector).
- Verified macOS build success for OpenAgents scheme (Debug).

Files:
- ios/OpenAgents/Views/macOS/SessionSidebarView.swift
- ios/OpenAgents/Views/macOS/KeyboardShortcutsView.swift
- ios/OpenAgents/Views/macOS/ChatMacOSView.swift
- ios/OpenAgents/Commands/OpenAgentsCommands.swift
- ios/OpenAgents/Views/macOS/AgentSelectorView.swift

## References
- macOS Human Interface Guidelines: Keyboard
- Apple Accessibility: https://developer.apple.com/accessibility/
- SwiftUI Accessibility: https://developer.apple.com/documentation/swiftui/view-accessibility
