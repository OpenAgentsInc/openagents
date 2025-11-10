import SwiftUI
#if os(macOS)
import AppKit
#endif
import UniformTypeIdentifiers
import OpenAgentsCore

#if os(macOS)
struct ChatMacOSView: View {
    @EnvironmentObject private var bridge: BridgeManager
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    private let defaultSidebarWidth: CGFloat = 260
    @State private var showSettings: Bool = false
    @State private var showDeveloper: Bool = false
    @State private var showKeyboardShortcuts: Bool = false

    var body: some View {
        // Two-column split (sidebar + detail) to match known-good structure
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
            // Inspector toggle removed in two-column layout
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
        // Inspector is currently disabled in two-column layout
        .focusedSceneValue(\.exportTranscriptJSON, { exportTranscriptJSON() })
        .focusedSceneValue(\.exportTranscriptMarkdown, { exportTranscriptMarkdown() })
        .focusedSceneValue(\.copyTranscriptMarkdown, { copyTranscriptMarkdown() })
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

    private func exportTranscriptJSON() {
        guard let sid = bridge.currentSessionId?.value else { return }
        let updates = bridge.updates.filter { $0.session_id.value == sid }
        guard let data = try? TranscriptExport.exportJSONData(updates: updates) else { return }
        let panel = NSSavePanel()
        panel.title = "Export Transcript (JSON)"
        panel.nameFieldStringValue = "openagents-\(sid.prefix(8)).json"
        panel.allowedContentTypes = [.json]
        panel.canCreateDirectories = true
        if panel.runModal() == .OK, let url = panel.url { try? data.write(to: url) }
    }

    private func exportTranscriptMarkdown() {
        guard let sid = bridge.currentSessionId?.value else { return }
        let updates = bridge.updates.filter { $0.session_id.value == sid }
        let md = TranscriptExport.exportMarkdown(updates: updates)
        guard let data = md.data(using: .utf8) else { return }
        let panel = NSSavePanel()
        panel.title = "Export Transcript (Markdown)"
        panel.nameFieldStringValue = "openagents-\(sid.prefix(8)).md"
        panel.allowedContentTypes = [UTType(filenameExtension: "md") ?? .plainText]
        panel.canCreateDirectories = true
        if panel.runModal() == .OK, let url = panel.url { try? data.write(to: url) }
    }

    private func copyTranscriptMarkdown() {
        guard let sid = bridge.currentSessionId?.value else { return }
        let updates = bridge.updates.filter { $0.session_id.value == sid }
        let md = TranscriptExport.exportMarkdown(updates: updates)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(md, forType: .string)
    }
}
#endif
