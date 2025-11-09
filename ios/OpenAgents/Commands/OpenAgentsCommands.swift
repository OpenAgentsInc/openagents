import SwiftUI

#if os(macOS)
struct OpenAgentsCommands: Commands {
    @FocusedBinding(\.showSettings) var showSettings: Bool?
    @FocusedBinding(\.showDeveloper) var showDeveloper: Bool?
    @FocusedBinding(\.showKeyboardShortcuts) var showKeyboardShortcuts: Bool?
    @FocusedValue(\.toggleSidebar) var toggleSidebar: (() -> Void)?
    @FocusedValue(\.toggleInspector) var toggleInspector: (() -> Void)?
    @FocusedValue(\.deleteSelectedSession) var deleteSelectedSession: (() -> Void)?

    var body: some Commands {
        // Replace default settings menu
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { showSettings? = true }
                .keyboardShortcut(",", modifiers: .command)
        }

        // View menu additions
        CommandMenu("View") {
            Button("Toggle Sidebar") { toggleSidebar?() }
                .keyboardShortcut("b", modifiers: .command)
            Button("Toggle Inspector") { toggleInspector?() }
                .keyboardShortcut("i", modifiers: .command)
        }

        // Help menu additions
        CommandMenu("Help") {
            Button("Keyboard Shortcuts…") { showKeyboardShortcuts? = true }
                .keyboardShortcut("/", modifiers: .command)
        }

        // Developer menu
        CommandMenu("Developer") {
            Button("Developer Tools…") { showDeveloper? = true }
                .keyboardShortcut("d", modifiers: [.command, .option])
            Divider()
            Button("Open Logs Folder") { openLogsFolder() }
        }

        // Global delete mapping (acts on focused sidebar)
        CommandGroup(after: .textEditing) {
            Button("Delete Session") { deleteSelectedSession?() }
                .keyboardShortcut(.delete, modifiers: [])
        }
    }

    private func openLogsFolder() {
        if let logsURL = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask).first?
            .appendingPathComponent("Logs").appendingPathComponent("OpenAgents") {
            NSWorkspace.shared.open(logsURL)
        }
    }
}

// Focused values for toggling sheets from commands
extension FocusedValues {
    struct ShowSettingsKey: FocusedValueKey { typealias Value = Binding<Bool> }
    struct ShowDeveloperKey: FocusedValueKey { typealias Value = Binding<Bool> }
    struct ShowKeyboardShortcutsKey: FocusedValueKey { typealias Value = Binding<Bool> }
    struct ToggleSidebarKey: FocusedValueKey { typealias Value = () -> Void }
    struct ToggleInspectorKey: FocusedValueKey { typealias Value = () -> Void }
    struct DeleteSelectedSessionKey: FocusedValueKey { typealias Value = () -> Void }
    var showSettings: Binding<Bool>? {
        get { self[ShowSettingsKey.self] }
        set { self[ShowSettingsKey.self] = newValue }
    }
    var showDeveloper: Binding<Bool>? {
        get { self[ShowDeveloperKey.self] }
        set { self[ShowDeveloperKey.self] = newValue }
    }
    var showKeyboardShortcuts: Binding<Bool>? {
        get { self[ShowKeyboardShortcutsKey.self] }
        set { self[ShowKeyboardShortcutsKey.self] = newValue }
    }
    var toggleSidebar: (() -> Void)? {
        get { self[ToggleSidebarKey.self] }
        set { self[ToggleSidebarKey.self] = newValue }
    }
    var toggleInspector: (() -> Void)? {
        get { self[ToggleInspectorKey.self] }
        set { self[ToggleInspectorKey.self] = newValue }
    }
    var deleteSelectedSession: (() -> Void)? {
        get { self[DeleteSelectedSessionKey.self] }
        set { self[DeleteSelectedSessionKey.self] = newValue }
    }
}
#endif
