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

    enum DeveloperTab: String, CaseIterable { case database = "Database", orchestration = "Orchestration", nostr = "Nostr", logs = "Logs", diagnostics = "Diagnostics" }
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
                    case .orchestration: OrchestrationDevView()
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
        switch tab {
        case .database: return "cylinder"
        case .orchestration: return "gearshape.2"
        case .nostr: return "antenna.radiowaves.left.and.right"
        case .logs: return "doc.text"
        case .diagnostics: return "stethoscope"
        }
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

// MARK: - Orchestration Dev View
private struct OrchestrationDevView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var schedulerRunning = false
    @State private var activeConfigId: String? = nil
    @State private var schedulerNextWake: Int? = nil
    @State private var schedulerMessage: String = ""
    @State private var cycles = 0
    @State private var executed = 0
    @State private var completed = 0
    @State private var failed = 0
    @State private var cancelled = 0
    @State private var lastTs: Int64? = nil
    @State private var timer = Timer.publish(every: 5.0, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            SectionHeader(title: "Scheduler")
            HStack(spacing: 16) {
                Label("Status: \(schedulerRunning ? "running" : "stopped")", systemImage: schedulerRunning ? "play.circle.fill" : "pause.circle")
                if let next = schedulerNextWake {
                    Label("Next: \(Date(timeIntervalSince1970: TimeInterval(next)))", systemImage: "clock")
                }
            }
            .font(OAFonts.ui(.body, 13))
            .foregroundStyle(OATheme.Colors.textSecondary)
            Text(schedulerMessage)
                .font(OAFonts.ui(.caption, 12))
                .foregroundStyle(OATheme.Colors.textTertiary)
            HStack(spacing: 12) {
                Button("Reload") { Task { await reloadScheduler() } }
                    .buttonStyle(.bordered)
                Button("Run Now") { Task { await runNow() } }
                    .buttonStyle(.borderedProminent)
                    .tint(OATheme.Colors.accent)
            }

            Divider().padding(.vertical, 4)
            SectionHeader(title: "Coordinator")
            HStack(spacing: 16) {
                Label("Cycles: \(cycles)", systemImage: "arrow.2.circlepath")
                Label("Executed: \(executed)", systemImage: "hammer")
                Label("Completed: \(completed)", systemImage: "checkmark.circle")
                Label("Failed: \(failed)", systemImage: "xmark.circle")
                Label("Cancelled: \(cancelled)", systemImage: "stop.circle")
            }
            .font(OAFonts.ui(.body, 13))
            .foregroundStyle(OATheme.Colors.textSecondary)
            if let ts = lastTs {
                Text("Last Cycle: \(Date(timeIntervalSince1970: TimeInterval(ts)/1000.0))")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textTertiary)
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onAppear { Task { await refresh() } }
        .onReceive(timer) { _ in Task { await refresh() } }
    }

    private func refresh() async {
        await fetchSchedulerStatus()
        await fetchCoordinatorStatus()
    }

    private func fetchSchedulerStatus() async {
        struct Empty: Codable {}
        struct Status: Codable { let running: Bool; let active_config_id: String?; let next_wake_time: Int?; let message: String }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerStatus, params: Empty(), id: "dev-sched-status-\(UUID().uuidString)") { (resp: Status?) in
            guard let r = resp else { return }
            schedulerRunning = r.running
            schedulerNextWake = r.next_wake_time
            schedulerMessage = r.message
            activeConfigId = r.active_config_id
        }
    }

    private func fetchCoordinatorStatus() async {
        struct Empty: Codable {}
        struct Coord: Codable { let cycles_run: Int; let tasks_executed: Int; let tasks_completed: Int; let tasks_failed: Int; let tasks_cancelled: Int; let last_cycle_ts: Int64? }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateCoordinatorStatus, params: Empty(), id: "dev-coord-status-\(UUID().uuidString)") { (resp: Coord?) in
            guard let r = resp else { return }
            cycles = r.cycles_run
            executed = r.tasks_executed
            completed = r.tasks_completed
            failed = r.tasks_failed
            cancelled = r.tasks_cancelled
            lastTs = r.last_cycle_ts
        }
    }

    private func reloadScheduler() async {
        struct Empty: Codable {}
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerReload, params: Empty(), id: "dev-sched-reload-\(UUID().uuidString)") { (_: [String: AnyCodable]?) in }
    }

    private func runNow() async {
        struct Empty: Codable {}
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerRunNow, params: Empty(), id: "dev-sched-run-\(UUID().uuidString)") { (_: [String: AnyCodable]?) in }
    }
}
#endif
