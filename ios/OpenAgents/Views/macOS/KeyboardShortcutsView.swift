import SwiftUI

#if os(macOS)
struct KeyboardShortcutsView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Keyboard Shortcuts")
                .font(OAFonts.mono(.title3, 18))
                .foregroundStyle(OATheme.Colors.textPrimary)

            shortcutsRow("New Chat", "⌘N")
            shortcutsRow("Toggle Sidebar", "⌘B")
            shortcutsRow("Keyboard Shortcuts", "⌘/")
            shortcutsRow("Open Settings", "⌘,")
            shortcutsRow("Developer Tools", "⌘⌥D")
            shortcutsRow("Delete Session", "Delete")

            Spacer()
        }
        .padding(20)
        .frame(minWidth: 380, minHeight: 220)
        .background(OATheme.Colors.background)
    }

    private func shortcutsRow(_ name: String, _ key: String) -> some View {
        HStack {
            Text(name).font(OAFonts.mono(.body, 13)).foregroundStyle(OATheme.Colors.textPrimary)
            Spacer()
            Text(key).font(OAFonts.mono(.body, 13)).foregroundStyle(OATheme.Colors.textSecondary)
        }
    }
}
#endif

