# Issue #8: Create Settings View (Bridge, Directory, Agent, Orchestration)

## Phase
Phase 3: Settings & Developer Pages

## Priority
High - User-facing configuration

## Description
Create a Settings view that consolidates user-facing configuration from the current dashboard: Bridge Status, Working Directory, Agent Configuration, and Orchestration Console.

## Current State
- `SimplifiedMacOSView` has cards for:
  - Bridge Status (connection info, client count)
  - Working Directory (current dir, ability to change)
  - Agent Configuration (settings for agents)
  - Orchestration Console (config management)
- No dedicated Settings page exists

## Target State
- Tabbed or sectioned Settings view with:
  - **Connection**: Bridge status, server config, client info
  - **Workspace**: Working directory selection and management
  - **Agents**: Agent configuration and preferences
  - **Orchestration**: Orchestration config CRUD (reuse `OrchestrationConsoleView`)
- Accessible via toolbar button (gear icon) or ⌘,
- Sheet or dedicated window presentation
- macOS-native styling with proper form controls

## Status
Completed (implemented on main)

What shipped
- SettingsView with a sidebar of tabs (Connection, Workspace, Agents, Orchestration) using NavigationSplitView.
- Connection tab shows bridge status, advertising endpoint, and connected client count.
- Workspace tab supports changing working directory and selecting from recent directories (persisted in UserDefaults).
- Agents tab provides defaults (mode, reasoning toggle) via AppStorage.
- Orchestration tab embeds `OrchestrationConsoleView`.
- Toolbar gear button opens settings; ⌘, shortcut wired.

## Acceptance Criteria
- [x] Create `SettingsView.swift` with tabbed layout
- [x] **Connection tab**: Bridge status, connected clients
- [x] **Workspace tab**: Working directory picker, recent directories
- [x] **Agents tab**: Default agent mode, preferences
- [x] **Orchestration tab**: Embed `OrchestrationConsoleView`
- [x] Keyboard shortcut ⌘, to open settings
- [x] Persist simple preferences via UserDefaults/AppStorage
- [x] Proper form controls (text fields, pickers, toggles)

## Technical Details

### File Structure
```swift
// ios/OpenAgents/Views/macOS/Settings/SettingsView.swift
struct SettingsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @EnvironmentObject var tinyvexManager: TinyvexManager
    @Environment(\.dismiss) var dismiss

    enum SettingsTab: String, CaseIterable {
        case connection = "Connection"
        case workspace = "Workspace"
        case agents = "Agents"
        case orchestration = "Orchestration"

        var icon: String {
            switch self {
            case .connection: return "network"
            case .workspace: return "folder"
            case .agents: return "cpu"
            case .orchestration: return "gearshape.2"
            }
        }
    }

    @State private var selectedTab: SettingsTab = .connection

    var body: some View {
        NavigationSplitView {
            // Sidebar with tabs
            List(SettingsTab.allCases, id: \.self, selection: $selectedTab) { tab in
                Label(tab.rawValue, systemImage: tab.icon)
                    .font(OAFonts.mono(size: 13))
            }
            .listStyle(.sidebar)
            .frame(minWidth: 180, idealWidth: 200)
        } detail: {
            // Tab content
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
                    .keyboardShortcut(.defaultAction)
                }
                .padding()

                Divider()

                // Content
                ScrollView {
                    Group {
                        switch selectedTab {
                        case .connection:
                            ConnectionSettingsView()
                        case .workspace:
                            WorkspaceSettingsView()
                        case .agents:
                            AgentSettingsView()
                        case .orchestration:
                            OrchestrationConsoleView()
                        }
                    }
                    .padding()
                }
            }
        }
        .frame(minWidth: 700, minHeight: 500)
    }
}

// MARK: - Connection Settings
struct ConnectionSettingsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SectionHeaderView(title: "Bridge Status")

            // Status indicator
            HStack {
                Circle()
                    .fill(statusColor)
                    .frame(width: 12, height: 12)

                Text(bridgeManager.status.description)
                    .font(OAFonts.mono(size: 14))
                    .foregroundColor(OATheme.Colors.textPrimary)

                Spacer()

                if bridgeManager.status == .connected {
                    Text("\(bridgeManager.connectedClientCount) client(s)")
                        .font(OAFonts.mono(size: 12))
                        .foregroundColor(OATheme.Colors.textSecondary)
                }
            }
            .padding()
            .background(OATheme.Colors.background.opacity(0.5))
            .cornerRadius(8)

            // Server configuration
            SectionHeaderView(title: "Server Configuration")

            FormRowView(label: "Port") {
                Text("3030") // Get from config
                    .font(OAFonts.mono(size: 13))
            }

            FormRowView(label: "Service") {
                Text("_openagents._tcp")
                    .font(OAFonts.mono(size: 13))
            }

            Spacer()
        }
        .frame(maxWidth: 600, alignment: .leading)
    }

    private var statusColor: Color {
        switch bridgeManager.status {
        case .connected:
            return OATheme.Colors.success
        case .disconnected:
            return OATheme.Colors.danger
        default:
            return OATheme.Colors.textSecondary
        }
    }
}

// MARK: - Workspace Settings
struct WorkspaceSettingsView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var recentDirectories: [URL] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SectionHeaderView(title: "Working Directory")

            // Current directory
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Current Directory")
                        .font(OAFonts.mono(size: 11))
                        .foregroundColor(OATheme.Colors.textSecondary)

                    Text(bridgeManager.workingDirectory?.path ?? "Not set")
                        .font(OAFonts.mono(size: 13))
                        .foregroundColor(OATheme.Colors.textPrimary)
                }

                Spacer()

                Button("Change...") {
                    selectWorkingDirectory()
                }
            }
            .padding()
            .background(OATheme.Colors.background.opacity(0.5))
            .cornerRadius(8)

            // Recent directories
            if !recentDirectories.isEmpty {
                SectionHeaderView(title: "Recent Directories")

                VStack(alignment: .leading, spacing: 8) {
                    ForEach(recentDirectories, id: \.path) { directory in
                        Button(action: { setWorkingDirectory(directory) }) {
                            HStack {
                                Image(systemName: "folder")
                                    .foregroundColor(OATheme.Colors.accent)

                                Text(directory.lastPathComponent)
                                    .font(OAFonts.mono(size: 13))

                                Spacer()

                                Text(directory.path)
                                    .font(OAFonts.mono(size: 11))
                                    .foregroundColor(OATheme.Colors.textSecondary)
                                    .lineLimit(1)
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
        .frame(maxWidth: 600, alignment: .leading)
        .onAppear(perform: loadRecentDirectories)
    }

    private func selectWorkingDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false

        if panel.runModal() == .OK, let url = panel.url {
            setWorkingDirectory(url)
        }
    }

    private func setWorkingDirectory(_ url: URL) {
        bridgeManager.workingDirectory = url
        saveRecentDirectory(url)
    }

    private func loadRecentDirectories() {
        // Load from UserDefaults
        if let data = UserDefaults.standard.data(forKey: "recentDirectories"),
           let urls = try? JSONDecoder().decode([URL].self, from: data) {
            recentDirectories = urls
        }
    }

    private func saveRecentDirectory(_ url: URL) {
        var recents = recentDirectories
        recents.removeAll { $0 == url }
        recents.insert(url, at: 0)
        recents = Array(recents.prefix(5)) // Keep last 5

        if let data = try? JSONEncoder().encode(recents) {
            UserDefaults.standard.set(data, forKey: "recentDirectories")
        }

        recentDirectories = recents
    }
}

// MARK: - Agent Settings
struct AgentSettingsView: View {
    @AppStorage("defaultAgent") private var defaultAgent = "Claude Code CLI"
    @AppStorage("enableThinking") private var enableThinking = true
    @AppStorage("maxTokens") private var maxTokens = 100000.0

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SectionHeaderView(title: "Default Agent")

            FormRowView(label: "Default Agent") {
                TextField("Agent name", text: $defaultAgent)
                    .font(OAFonts.mono(size: 13))
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 200)
            }

            Divider()

            SectionHeaderView(title: "Preferences")

            Toggle("Enable thinking/reasoning display", isOn: $enableThinking)
                .font(OAFonts.mono(size: 13))

            FormRowView(label: "Max Tokens") {
                Slider(value: $maxTokens, in: 10000...200000, step: 10000)
                    .frame(width: 200)

                Text("\(Int(maxTokens))")
                    .font(OAFonts.mono(size: 13))
                    .frame(width: 60, alignment: .trailing)
            }

            Spacer()
        }
        .frame(maxWidth: 600, alignment: .leading)
    }
}

// MARK: - Utility Views
struct SectionHeaderView: View {
    let title: String

    var body: some View {
        Text(title)
            .font(OAFonts.mono(size: 12, weight: .semibold))
            .foregroundColor(OATheme.Colors.textSecondary)
            .textCase(.uppercase)
    }
}

struct FormRowView<Content: View>: View {
    let label: String
    @ViewBuilder let content: Content

    var body: some View {
        HStack(alignment: .center) {
            Text(label)
                .font(OAFonts.mono(size: 13))
                .foregroundColor(OATheme.Colors.textPrimary)
                .frame(width: 150, alignment: .leading)

            content
        }
    }
}
```

### Integration with Main View
```swift
// In ChatMacOSView
@State private var showSettings = false

.toolbar {
    ToolbarItem(placement: .automatic) {
        Button(action: { showSettings = true }) {
            Image(systemName: "gear")
        }
    }
}
.sheet(isPresented: $showSettings) {
    SettingsView()
        .environmentObject(bridgeManager)
        .environmentObject(tinyvexManager)
}
.keyboardShortcut(",", modifiers: .command)
```

### Reuse Orchestration Console
The Orchestration tab can directly embed the existing `OrchestrationConsoleView` which already has full CRUD functionality.

## Dependencies
None - Uses existing components

## Blocked By
None

## Blocks
None - Standalone settings page

## Estimated Complexity
Medium-High (5-6 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] All tabs render correctly
- [ ] Bridge status updates in real-time
- [ ] Working directory picker works
- [ ] Recent directories save/load
- [ ] Agent preferences persist
- [ ] Orchestration tab shows full console
- [ ] ⌘, keyboard shortcut works
- [ ] Settings changes take effect immediately

## References
- Current dashboard: `ios/OpenAgents/SimplifiedMacOSView.swift`
- Orchestration Console: `ios/OpenAgents/Views/OrchestrationConsoleView.swift`
- Tinyvex Manager: `ios/OpenAgents/TinyvexManager.swift`
- macOS Settings patterns: System Settings app for UX reference
