import SwiftUI
import AppKit
import UniformTypeIdentifiers
import OpenAgentsCore

#if os(macOS)
struct ChatMacOSView: View {
    @EnvironmentObject private var bridge: BridgeManager
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
        } content: {
            ChatAreaView()
                .navigationSplitViewColumnWidth(min: 800, ideal: 1200)
                .navigationTitle("")
        } detail: {
            if showInspector {
                InspectorPaneView()
                    .navigationSplitViewColumnWidth(min: 260, ideal: 300, max: 360)
            }
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
            ToolbarItem(placement: .automatic) {
                Button(action: { toggleInspector() }) {
                    Image(systemName: "sidebar.trailing")
                }
                .keyboardShortcut("i", modifiers: .command)
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
        .focusedSceneValue(\.toggleInspector, { toggleInspector() })
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

    private func toggleInspector() {
        withAnimation { showInspector.toggle() }
    }

    private func exportTranscriptJSON() {
        guard let sid = bridge.currentSessionId?.value else { return }
        let updates = bridge.updates.filter { $0.session_id.value == sid }
        guard let data = try? JSONEncoder().encode(updates) else { return }
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
        let md = makeMarkdown(from: updates)
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
        let md = makeMarkdown(from: updates)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(md, forType: .string)
    }

    private func makeMarkdown(from updates: [ACP.Client.SessionNotificationWire]) -> String {
        var out: [String] = ["# OpenAgents Transcript\n"]
        for note in updates {
            switch note.update {
            case .userMessageChunk(let chunk):
                if case .text(let t) = chunk.content { out.append("\n**User**\n\n" + t.text + "\n") }
            case .agentMessageChunk(let chunk):
                if case .text(let t) = chunk.content { out.append("\n**Assistant**\n\n" + t.text + "\n") }
            case .agentThoughtChunk(let chunk):
                if case .text(let t) = chunk.content { out.append("\n> _Thinking:_ " + t.text.replacingOccurrences(of: "\n", with: " ") + "\n") }
            case .plan(let plan):
                let bullets = plan.entries.map { "- " + $0.content }.joined(separator: "\n")
                out.append("\n**Plan**\n\n" + bullets + "\n")
            case .toolCall(let call):
                out.append("\n`tool_call` \(call.name) id=\(call.call_id)\n")
            case .toolCallUpdate(let upd):
                out.append("`tool_call_update` id=\(upd.call_id) status=\(upd.status.rawValue)\n")
            case .availableCommandsUpdate, .currentModeUpdate:
                continue
            }
        }
        return out.joined(separator: "\n")
    }
}
#endif
