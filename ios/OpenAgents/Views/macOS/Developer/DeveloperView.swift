import SwiftUI
import OpenAgentsCore
#if os(macOS)
import AppKit
#endif

#if os(macOS)
struct DeveloperView: View {
    @EnvironmentObject var tinyvexManager: TinyvexManager
    @EnvironmentObject var bridgeManager: BridgeManager
    @Environment(\.dismiss) var dismiss

    enum DeveloperTab: String, CaseIterable { case database = "Database", nostr = "Nostr", logs = "Logs", diagnostics = "Diagnostics" }
    @State private var selectedTab: DeveloperTab = .database

    var body: some View {
        NavigationSplitView {
            List(DeveloperTab.allCases, id: \.self, selection: $selectedTab) { tab in
                Label(tab.rawValue, systemImage: icon(for: tab))
                    .font(OAFonts.mono(.body, 12))
            }
            .listStyle(.sidebar)
            .scrollContentBackground(.hidden)
            .background(OATheme.Colors.sidebarBackground)
            .frame(minWidth: 200)
        } detail: {
            VStack(spacing: 0) {
                HStack {
                    Text(selectedTab.rawValue)
                        .font(OAFonts.mono(.title3, 18))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Spacer()
                    Button("Done") { dismiss() }
                }
                .padding()
                Divider()
                ScrollView {
                    switch selectedTab {
                    case .database: DatabaseDevToolsView()
                    case .nostr: NostrDevToolsView()
                    case .logs: LogsViewerView()
                    case .diagnostics: DiagnosticsView()
                    }
                }
                .padding()
            }
        }
        .frame(minWidth: 860, minHeight: 600)
        .background(OATheme.Colors.background)
    }

    private func icon(for tab: DeveloperTab) -> String {
        switch tab { case .database: return "cylinder"; case .nostr: return "antenna.radiowaves.left.and.right"; case .logs: return "doc.text"; case .diagnostics: return "stethoscope" }
    }
}

// MARK: - Stubs
private struct DatabaseDevToolsView: View {
    @EnvironmentObject var tinyvexManager: TinyvexManager
    @State private var queryText = "SELECT COUNT(*) FROM acp_events;"
    @State private var results = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Database: \(tinyvexManager.dbPath)")
                .font(OAFonts.mono(.body, 12))
                .foregroundStyle(OATheme.Colors.textSecondary)
            TextEditor(text: $queryText)
                .font(OAFonts.mono(.body, 12))
                .frame(minHeight: 100)
                .border(OATheme.Colors.textSecondary.opacity(0.3))
            HStack {
                Button("Execute") { results = "Query execution is not implemented yet." }
                Button("Clear") { queryText = ""; results = "" }
                Spacer()
            }
            if !results.isEmpty {
                ScrollView { Text(results).font(OAFonts.mono(.body, 12)).textSelection(.enabled) }
                    .frame(minHeight: 160)
                    .border(OATheme.Colors.textSecondary.opacity(0.2))
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct NostrDevToolsView: View {
    @State private var relayURL = "wss://relay.damus.io"
    @State private var status = "Disconnected"
    @State private var eventJSON = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                TextField("Relay URL", text: $relayURL).textFieldStyle(.roundedBorder).font(OAFonts.mono(.body, 12))
                Button("Connect") { status = "Connected" }
                Text(status).font(OAFonts.mono(.body, 12)).foregroundStyle(status == "Connected" ? OATheme.Colors.success : OATheme.Colors.danger)
            }
            TextEditor(text: $eventJSON).font(OAFonts.mono(.body, 12)).frame(minHeight: 160).border(OATheme.Colors.textSecondary.opacity(0.3))
            HStack {
                Button("Parse") {}
                Button("Validate") {}
                Spacer()
                Button("Copy") { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(eventJSON, forType: .string) }
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct LogsViewerView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var filter = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            TextField("Filter logs...", text: $filter).textFieldStyle(.roundedBorder).font(OAFonts.mono(.body, 12))
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 6) {
                    ForEach(filteredLogs, id: \.self) { line in
                        Text(line).font(OAFonts.mono(.body, 11)).foregroundStyle(OATheme.Colors.textSecondary).textSelection(.enabled)
                    }
                }
            }
            HStack {
                Button("Copy All") { copyAll() }
                Spacer()
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    private var filteredLogs: [String] { filter.isEmpty ? bridgeManager.logs : bridgeManager.logs.filter { $0.localizedCaseInsensitiveContains(filter) } }
    private func copyAll() { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(filteredLogs.joined(separator: "\n"), forType: .string) }
}

private struct DiagnosticsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("macOS: \(ProcessInfo.processInfo.operatingSystemVersionString)").font(OAFonts.mono(.body, 12))
            Text("Bridge: \(statusText(bridgeManager.status))").font(OAFonts.mono(.body, 12))
            Text("Clients: \(bridgeManager.connectedClientCount)").font(OAFonts.mono(.body, 12))
            Text("Current Session: \(bridgeManager.currentSessionId?.value ?? "None")").font(OAFonts.mono(.body, 12))
            Button("Copy Debug Info") { copy() }
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    private func copy() {
        let s = "Bridge: \(statusText(bridgeManager.status))\nClients: \(bridgeManager.connectedClientCount)\nSession: \(bridgeManager.currentSessionId?.value ?? "None")"
        NSPasteboard.general.clearContents(); NSPasteboard.general.setString(s, forType: .string)
    }
}
    private func statusText(_ st: BridgeManager.Status) -> String {
    switch st {
    case .idle: return "Idle"
    case .advertising(let p): return "Advertising :\(p)"
    case .discovering: return "Discovering"
    case .connecting(let h, let p): return "Connecting to \(h):\(p)"
    case .handshaking(let h, let p): return "Handshaking with \(h):\(p)"
    case .connected(let h, let p): return "Connected to \(h):\(p)"
    case .error(let e): return "Error: \(e)"
    }
}
#endif
