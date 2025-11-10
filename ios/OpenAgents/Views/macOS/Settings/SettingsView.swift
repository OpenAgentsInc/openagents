import SwiftUI
import OpenAgentsCore
#if os(macOS)
import AppKit
#endif

#if os(macOS)
struct SettingsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @EnvironmentObject var tinyvexManager: TinyvexManager
    @Environment(\.dismiss) var dismiss

    enum SettingsTab: String, CaseIterable { case connection = "Connection", workspace = "Workspace", agents = "Agents", orchestration = "Orchestration" }
    @State private var selectedTab: SettingsTab = .connection

    var body: some View {
        NavigationSplitView {
            List(SettingsTab.allCases, id: \.self, selection: $selectedTab) { tab in
                Label(tab.rawValue, systemImage: icon(for: tab))
                    .font(OAFonts.mono(.body, 12))
            }
            .listStyle(.sidebar)
            .scrollContentBackground(.hidden)
            .background(OATheme.Colors.sidebarBackground)
            .frame(minWidth: 180, idealWidth: 200)
        } detail: {
            VStack(spacing: 0) {
                HStack {
                    Text(selectedTab.rawValue)
                        .font(OAFonts.mono(.title3, 18))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Spacer()
                    Button("Done") { dismiss() }
                        .keyboardShortcut(.defaultAction)
                }
                .padding()
                Divider()
                ScrollView {
                    Group {
                        switch selectedTab {
                        case .connection: ConnectionSettingsView()
                        case .workspace: WorkspaceSettingsView()
                        case .agents: AgentSettingsView()
                        case .orchestration: OrchestrationConsoleView()
                        }
                    }
                    .padding()
                }
            }
        }
        .frame(minWidth: 760, minHeight: 520)
        .background(OATheme.Colors.background)
    }

    private func icon(for tab: SettingsTab) -> String {
        switch tab {
        case .connection: return "antenna.radiowaves.left.and.right"
        case .workspace: return "folder"
        case .agents: return "cpu"
        case .orchestration: return "gearshape.2"
        }
    }
}

// MARK: - Connection
private struct ConnectionSettingsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SectionHeader(title: "Bridge Status")
            HStack {
                Circle().fill(statusColor).frame(width: 10, height: 10)
                Text(statusText)
                    .font(OAFonts.mono(.body, 13))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                Spacer()
                if case .advertising(let port) = bridgeManager.status {
                    Text("ws://0.0.0.0:\(port)")
                        .font(OAFonts.mono(.body, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
                Text("\(bridgeManager.connectedClientCount) client(s)")
                    .font(OAFonts.mono(.body, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            .padding()
            .background(OATheme.Colors.background.opacity(0.5))
            .cornerRadius(8)
            Spacer()
        }
        .frame(maxWidth: 640, alignment: .leading)
    }
    private var statusColor: Color {
        switch bridgeManager.status {
        case .advertising: return OATheme.Colors.success
        case .connecting, .handshaking, .discovering: return .yellow
        case .error: return OATheme.Colors.danger
        default: return OATheme.Colors.textSecondary
        }
    }
    private var statusText: String {
        switch bridgeManager.status {
        case .idle: return "Idle"
        case .advertising(let port): return "Advertising on port \(port)"
        case .discovering: return "Discovering"
        case .connecting(let h, let p): return "Connecting to \(h):\(p)"
        case .handshaking(let h, let p): return "Handshaking with \(h):\(p)"
        case .connected(let h, let p): return "Connected to \(h):\(p)"
        case .error(let s): return "Error: \(s)"
        }
    }
}

// MARK: - Workspace
private struct WorkspaceSettingsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var recent: [URL] = []
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SectionHeader(title: "Working Directory")
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Current Directory")
                        .font(OAFonts.mono(.caption, 11))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                    Text(bridgeManager.workingDirectory?.path ?? "Not set")
                        .font(OAFonts.mono(.body, 13))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                        .lineLimit(1)
                }
                Spacer()
                Button("Change…") { pickDirectory() }
            }
            .padding()
            .background(OATheme.Colors.background.opacity(0.5))
            .cornerRadius(8)

            if !recent.isEmpty {
                SectionHeader(title: "Recent")
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(recent, id: \.path) { url in
                        Button(action: { setDirectory(url) }) {
                            HStack {
                                Image(systemName: "folder")
                                Text(url.lastPathComponent)
                                    .font(OAFonts.mono(.body, 13))
                                Spacer()
                                Text(url.path)
                                    .font(OAFonts.mono(.caption, 11))
                                    .foregroundStyle(OATheme.Colors.textSecondary)
                            }
                            .padding(8)
                            .background(OATheme.Colors.background.opacity(0.3))
                            .cornerRadius(6)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            Spacer()
        }
        .onAppear { loadRecent() }
        .frame(maxWidth: 640, alignment: .leading)
    }
    private func pickDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url { setDirectory(url) }
    }
    private func setDirectory(_ url: URL) {
        bridgeManager.setWorkingDirectory(url)
        var r = recent
        r.removeAll { $0 == url }
        r.insert(url, at: 0)
        r = Array(r.prefix(5))
        recent = r
        if let data = try? JSONEncoder().encode(r) { UserDefaults.standard.set(data, forKey: "recentDirectories") }
    }
    private func loadRecent() {
        if let data = UserDefaults.standard.data(forKey: "recentDirectories"), let urls = try? JSONDecoder().decode([URL].self, from: data) { recent = urls }
    }
}

// MARK: - Agent prefs
private struct AgentSettingsView: View {
    @AppStorage("defaultAgentMode") private var defaultAgentMode: String = ACPSessionModeId.claude_code.rawValue
    @AppStorage("enableThinking") private var enableThinking = true
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SectionHeader(title: "Default Agent")
            Picker("", selection: $defaultAgentMode) {
                Text("OpenAgents").tag(ACPSessionModeId.default_mode.rawValue)
                Text("Claude Code").tag(ACPSessionModeId.claude_code.rawValue)
                Text("Codex").tag(ACPSessionModeId.codex.rawValue)
            }
            .pickerStyle(.segmented)
            .frame(width: 420)
            Divider()
            Toggle("Enable thinking/reasoning display", isOn: $enableThinking)
                .font(OAFonts.mono(.body, 13))
            Spacer()
        }
        .frame(maxWidth: 640, alignment: .leading)
    }
}

// MARK: - Common UI
private struct SectionHeader: View {
    let title: String
    var body: some View {
        Text(title.uppercased())
            .font(OAFonts.mono(.body, 12))
            .foregroundStyle(OATheme.Colors.textSecondary)
    }
}
#endif
