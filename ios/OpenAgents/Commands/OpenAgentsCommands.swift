import SwiftUI

#if os(macOS)
struct OpenAgentsCommands: Commands {
    @FocusedBinding(\.showSettings) var showSettings: Bool?
    @FocusedBinding(\.showDeveloper) var showDeveloper: Bool?

    var body: some Commands {
        // Replace default settings menu
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { showSettings? = true }
                .keyboardShortcut(",", modifiers: .command)
        }

        // Developer menu
        CommandMenu("Developer") {
            Button("Developer Tools…") { showDeveloper? = true }
                .keyboardShortcut("d", modifiers: [.command, .option])
            Divider()
            Button("Open Logs Folder") { openLogsFolder() }
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
    var showSettings: Binding<Bool>? {
        get { self[ShowSettingsKey.self] }
        set { self[ShowSettingsKey.self] = newValue }
    }
    var showDeveloper: Binding<Bool>? {
        get { self[ShowDeveloperKey.self] }
        set { self[ShowDeveloperKey.self] = newValue }
    }
}
#endif

