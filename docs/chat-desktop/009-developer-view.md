# Issue #9: Create Developer View (Tinyvex, Nostr Dev Tools)

## Phase
Phase 3: Settings & Developer Pages

## Priority
Low-Medium - Developer/power-user features

## Description
Create a Developer view that consolidates advanced/debugging tools from the current dashboard: Tinyvex Dev Tools and Nostr Dev Tools.

## Current State
- `SimplifiedMacOSView` has cards for:
  - Tinyvex Dev Tools (database inspection, query tools)
  - Nostr Dev Tools (event inspection, relay testing)
- No dedicated Developer page exists

## Target State
- Developer-focused view with tabs/sections:
  - **Database (Tinyvex)**: Schema viewer, query executor, data browser
  - **Nostr**: Event viewer, relay tester, key management
  - **Logs**: System logs, bridge logs, agent logs
  - **Diagnostics**: Debug info, performance metrics, system state
- Accessible via toolbar button (Developer menu) or ⌘⌥D
- Sheet or separate window presentation
- Monospaced fonts, JSON viewers, copy buttons

## Acceptance Criteria
- [ ] Create `DeveloperView.swift` with tabbed layout
- [ ] **Database tab**: Tinyvex schema, query executor, table browser
- [ ] **Nostr tab**: Event inspector, relay connection tester
- [ ] **Logs tab**: Filterable log viewer (bridge, agent, system)
- [ ] **Diagnostics tab**: System info, BridgeManager state, debug info
- [ ] Keyboard shortcut ⌘⌥D to open developer view
- [ ] Copy buttons for JSON, logs, debug info
- [ ] Export functionality for logs and data
- [ ] Search/filter capabilities

## Technical Details

### File Structure
```swift
// ios/OpenAgents/Views/macOS/Developer/DeveloperView.swift
struct DeveloperView: View {
    @EnvironmentObject var tinyvexManager: TinyvexManager
    @EnvironmentObject var bridgeManager: BridgeManager
    @Environment(\.dismiss) var dismiss

    enum DeveloperTab: String, CaseIterable {
        case database = "Database"
        case nostr = "Nostr"
        case logs = "Logs"
        case diagnostics = "Diagnostics"

        var icon: String {
            switch self {
            case .database: return "cylinder"
            case .nostr: return "antenna.radiowaves.left.and.right"
            case .logs: return "doc.text"
            case .diagnostics: return "stethoscope"
            }
        }
    }

    @State private var selectedTab: DeveloperTab = .database

    var body: some View {
        NavigationSplitView {
            // Sidebar
            List(DeveloperTab.allCases, id: \.self, selection: $selectedTab) { tab in
                Label(tab.rawValue, systemImage: tab.icon)
                    .font(OAFonts.mono(size: 13))
            }
            .listStyle(.sidebar)
            .frame(minWidth: 180, idealWidth: 200)
        } detail: {
            VStack(spacing: 0) {
                // Header
                HStack {
                    Text(selectedTab.rawValue)
                        .font(OAFonts.mono(size: 18, weight: .semibold))
                        .foregroundColor(OATheme.Colors.textPrimary)

                    Spacer()

                    Button("Done") {
                        dismiss()
                    }
                }
                .padding()

                Divider()

                // Content
                ScrollView {
                    Group {
                        switch selectedTab {
                        case .database:
                            DatabaseDevToolsView()
                        case .nostr:
                            NostrDevToolsView()
                        case .logs:
                            LogsViewerView()
                        case .diagnostics:
                            DiagnosticsView()
                        }
                    }
                    .padding()
                }
            }
        }
        .frame(minWidth: 800, minHeight: 600)
    }
}

// MARK: - Database Dev Tools
struct DatabaseDevToolsView: View {
    @EnvironmentObject var tinyvexManager: TinyvexManager
    @State private var queryText = ""
    @State private var queryResults: String = ""
    @State private var selectedTable: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            // Schema viewer
            GroupBox("Database Schema") {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(tables, id: \.self) { table in
                        Button(action: { loadTable(table) }) {
                            HStack {
                                Image(systemName: "tablecells")
                                    .foregroundColor(OATheme.Colors.accent)

                                Text(table)
                                    .font(OAFonts.mono(size: 13))

                                Spacer()

                                if table == selectedTable {
                                    Image(systemName: "checkmark")
                                        .foregroundColor(OATheme.Colors.accent)
                                }
                            }
                            .padding(8)
                            .background(table == selectedTable ? OATheme.Colors.accent.opacity(0.1) : Color.clear)
                            .cornerRadius(6)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding()
            }

            // Query executor
            GroupBox("Query Executor") {
                VStack(alignment: .leading, spacing: 12) {
                    Text("SQL Query")
                        .font(OAFonts.mono(size: 11))
                        .foregroundColor(OATheme.Colors.textSecondary)

                    TextEditor(text: $queryText)
                        .font(OAFonts.mono(size: 12))
                        .frame(minHeight: 100)
                        .border(OATheme.Colors.textSecondary.opacity(0.3))

                    HStack {
                        Button("Execute") {
                            executeQuery()
                        }
                        .buttonStyle(.borderedProminent)

                        Button("Clear") {
                            queryText = ""
                            queryResults = ""
                        }

                        Spacer()
                    }
                }
                .padding()
            }

            // Results
            if !queryResults.isEmpty {
                GroupBox("Results") {
                    ScrollView([.horizontal, .vertical]) {
                        Text(queryResults)
                            .font(OAFonts.mono(size: 11))
                            .textSelection(.enabled)
                            .padding()
                    }
                    .frame(minHeight: 200)
                }
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var tables: [String] {
        // Get tables from TinyvexManager
        ["sessions", "messages", "configs", "orchestration_runs"]
    }

    private func loadTable(_ table: String) {
        selectedTable = table
        queryText = "SELECT * FROM \(table) LIMIT 100;"
    }

    private func executeQuery() {
        // Execute query via TinyvexManager
        // For now, mock results
        queryResults = "Query executed successfully.\n\nRows: 5\nColumns: id, name, timestamp\n..."
    }
}

// MARK: - Nostr Dev Tools
struct NostrDevToolsView: View {
    @State private var eventJSON = ""
    @State private var relayURL = "wss://relay.damus.io"
    @State private var connectionStatus = "Disconnected"

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            // Relay tester
            GroupBox("Relay Connection") {
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        TextField("Relay URL", text: $relayURL)
                            .font(OAFonts.mono(size: 13))
                            .textFieldStyle(.roundedBorder)

                        Button("Connect") {
                            testRelay()
                        }

                        Text(connectionStatus)
                            .font(OAFonts.mono(size: 12))
                            .foregroundColor(statusColor)
                    }
                }
                .padding()
            }

            // Event viewer
            GroupBox("Event Inspector") {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Paste Nostr Event JSON")
                        .font(OAFonts.mono(size: 11))
                        .foregroundColor(OATheme.Colors.textSecondary)

                    TextEditor(text: $eventJSON)
                        .font(OAFonts.mono(size: 11))
                        .frame(minHeight: 200)
                        .border(OATheme.Colors.textSecondary.opacity(0.3))

                    HStack {
                        Button("Parse") {
                            parseEvent()
                        }

                        Button("Validate") {
                            validateEvent()
                        }

                        Button("Clear") {
                            eventJSON = ""
                        }

                        Spacer()

                        Button("Copy") {
                            copyToClipboard(eventJSON)
                        }
                    }
                }
                .padding()
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var statusColor: Color {
        connectionStatus == "Connected" ? OATheme.Colors.success : OATheme.Colors.danger
    }

    private func testRelay() {
        // Test relay connection
        connectionStatus = "Connecting..."
        // Mock for now
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            connectionStatus = "Connected"
        }
    }

    private func parseEvent() {
        // Parse and pretty-print event
    }

    private func validateEvent() {
        // Validate event signature
    }

    private func copyToClipboard(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }
}

// MARK: - Logs Viewer
struct LogsViewerView: View {
    @State private var logs: [LogEntry] = []
    @State private var filterText = ""
    @State private var selectedLevel: LogLevel = .all

    enum LogLevel: String, CaseIterable {
        case all = "All"
        case error = "Error"
        case warning = "Warning"
        case info = "Info"
        case debug = "Debug"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Filters
            HStack {
                Picker("Level", selection: $selectedLevel) {
                    ForEach(LogLevel.allCases, id: \.self) { level in
                        Text(level.rawValue).tag(level)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 300)

                Spacer()

                TextField("Filter...", text: $filterText)
                    .font(OAFonts.mono(size: 12))
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 200)

                Button("Clear") {
                    logs.removeAll()
                }

                Button("Export") {
                    exportLogs()
                }
            }

            Divider()

            // Log list
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 4) {
                    ForEach(filteredLogs) { log in
                        LogRowView(log: log)
                    }
                }
                .padding()
            }
            .background(Color.black.opacity(0.9))
            .cornerRadius(8)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onAppear(perform: loadLogs)
    }

    private var filteredLogs: [LogEntry] {
        logs.filter { log in
            (selectedLevel == .all || log.level.rawValue == selectedLevel.rawValue) &&
            (filterText.isEmpty || log.message.localizedCaseInsensitiveContains(filterText))
        }
    }

    private func loadLogs() {
        // Load logs from system/bridge/agent
        logs = [
            LogEntry(level: .info, timestamp: Date(), message: "Bridge server started on port 3030"),
            LogEntry(level: .debug, timestamp: Date(), message: "Client connected: iOS"),
            LogEntry(level: .warning, timestamp: Date(), message: "Working directory not set"),
        ]
    }

    private func exportLogs() {
        // Export logs to file
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "openagents-logs-\(Date().ISO8601Format()).txt"
        if panel.runModal() == .OK, let url = panel.url {
            let logText = logs.map { $0.description }.joined(separator: "\n")
            try? logText.write(to: url, atomically: true, encoding: .utf8)
        }
    }
}

struct LogEntry: Identifiable {
    let id = UUID()
    let level: LogsViewerView.LogLevel
    let timestamp: Date
    let message: String

    var description: String {
        "[\(timestamp)] [\(level.rawValue.uppercased())] \(message)"
    }
}

struct LogRowView: View {
    let log: LogEntry

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Text(timeString)
                .font(OAFonts.mono(size: 10))
                .foregroundColor(.gray)
                .frame(width: 80, alignment: .leading)

            Text(log.level.rawValue.uppercased())
                .font(OAFonts.mono(size: 10))
                .foregroundColor(levelColor)
                .frame(width: 60, alignment: .leading)

            Text(log.message)
                .font(OAFonts.mono(size: 11))
                .foregroundColor(.white)
        }
    }

    private var timeString: String {
        let formatter = DateFormatter()
        formatter.timeStyle = .medium
        return formatter.string(from: log.timestamp)
    }

    private var levelColor: Color {
        switch log.level {
        case .error: return .red
        case .warning: return .orange
        case .info: return .cyan
        case .debug: return .purple
        case .all: return .white
        }
    }
}

// MARK: - Diagnostics
struct DiagnosticsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            GroupBox("System Information") {
                VStack(alignment: .leading, spacing: 8) {
                    DiagnosticRowView(label: "macOS Version", value: ProcessInfo.processInfo.operatingSystemVersionString)
                    DiagnosticRowView(label: "App Version", value: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "Unknown")
                    DiagnosticRowView(label: "Build", value: Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "Unknown")
                }
                .padding()
            }

            GroupBox("Bridge State") {
                VStack(alignment: .leading, spacing: 8) {
                    DiagnosticRowView(label: "Status", value: bridgeManager.status.description)
                    DiagnosticRowView(label: "Connected Clients", value: "\(bridgeManager.connectedClientCount)")
                    DiagnosticRowView(label: "Current Session", value: bridgeManager.currentSessionId ?? "None")
                    DiagnosticRowView(label: "Total Updates", value: "\(bridgeManager.updates.count)")
                    DiagnosticRowView(label: "Working Directory", value: bridgeManager.workingDirectory?.path ?? "Not set")
                }
                .padding()
            }

            Button("Copy Debug Info") {
                copyDebugInfo()
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func copyDebugInfo() {
        let debugInfo = """
        macOS: \(ProcessInfo.processInfo.operatingSystemVersionString)
        App Version: \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "Unknown")
        Bridge Status: \(bridgeManager.status.description)
        Connected Clients: \(bridgeManager.connectedClientCount)
        """
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(debugInfo, forType: .string)
    }
}

struct DiagnosticRowView: View {
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .font(OAFonts.mono(size: 12))
                .foregroundColor(OATheme.Colors.textSecondary)
                .frame(width: 180, alignment: .leading)

            Text(value)
                .font(OAFonts.mono(size: 12))
                .foregroundColor(OATheme.Colors.textPrimary)
                .textSelection(.enabled)
        }
    }
}
```

### Integration
```swift
// In ChatMacOSView or App Menu
@State private var showDeveloper = false

// Menu item or toolbar
Menu("Developer") {
    Button("Developer Tools...") {
        showDeveloper = true
    }
    .keyboardShortcut("d", modifiers: [.command, .option])
}

.sheet(isPresented: $showDeveloper) {
    DeveloperView()
        .environmentObject(bridgeManager)
        .environmentObject(tinyvexManager)
}
```

## Dependencies
None - Uses existing managers

## Blocked By
None

## Blocks
None - Standalone developer tools

## Estimated Complexity
Medium-High (5-7 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] All tabs render correctly
- [ ] Database query executor works
- [ ] Nostr event inspector parses events
- [ ] Logs viewer displays and filters logs
- [ ] Diagnostics shows accurate system info
- [ ] Copy/export functions work
- [ ] ⌘⌥D keyboard shortcut works

## References
- Current dashboard: `ios/OpenAgents/SimplifiedMacOSView.swift`
- TinyvexManager: `ios/OpenAgents/TinyvexManager.swift`
- docs/nostr/README.md – current Nostr wiring and constraints
- Xcode console, Chrome DevTools for UX reference
