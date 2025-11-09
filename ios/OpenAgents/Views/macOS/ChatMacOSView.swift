import SwiftUI

#if os(macOS)
struct ChatMacOSView: View {
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var showInspector: Bool = false
    private let defaultSidebarWidth: CGFloat = 260
    @State private var showSettings: Bool = false
    @State private var showDeveloper: Bool = false
    @State private var showKeyboardShortcuts: Bool = false

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SessionSidebarView()
                .navigationSplitViewColumnWidth(min: defaultSidebarWidth, ideal: defaultSidebarWidth, max: defaultSidebarWidth)
        } detail: {
            ChatAreaView()
                .navigationSplitViewColumnWidth(min: 800, ideal: 1200)
                .navigationTitle("")
        }
        .navigationSplitViewStyle(.balanced)
        .toolbar {
            ToolbarItem(placement: .automatic) {
                Button(action: { showSettings = true }) {
                    Image(systemName: "gear")
                }
                .keyboardShortcut(",", modifiers: .command)
            }
            ToolbarItem(placement: .automatic) {
                Button(action: { showDeveloper = true }) {
                    Image(systemName: "wrench.and.screwdriver")
                }
                .keyboardShortcut("d", modifiers: [.command, .option])
            }
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
        .sheet(isPresented: $showDeveloper) {
            DeveloperView()
        }
        .sheet(isPresented: $showKeyboardShortcuts) {
            KeyboardShortcutsView()
        }
        .focusedSceneValue(\.showSettings, $showSettings)
        .focusedSceneValue(\.showDeveloper, $showDeveloper)
        .focusedSceneValue(\.showKeyboardShortcuts, $showKeyboardShortcuts)
        .focusedSceneValue(\.toggleSidebar, { toggleSidebar() })
        .background(OATheme.Colors.background)
        .toolbarBackground(OATheme.Colors.background, for: .windowToolbar)
        .toolbarBackground(.visible, for: .windowToolbar)
    }

    @ViewBuilder
    private var splitView: some View { EmptyView() }

    private func toggleSidebar() {
        switch columnVisibility {
        case .all:
            columnVisibility = .detailOnly
        default:
            columnVisibility = .all
        }
    }
}
#endif
