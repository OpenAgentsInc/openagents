import SwiftUI
import OpenAgentsCore

#if os(macOS)
struct SessionSidebarView: View {
    @EnvironmentObject private var bridge: BridgeManager
    @StateObject private var orchestrationVM = OrchestrationViewModel()
    @State private var searchText: String = ""
    @State private var hoveredId: String? = nil
    @State private var selectedSessionId: String? = nil
    @FocusState private var isSearchFocused: Bool
    @State private var editingTitleId: String? = nil
    @State private var editingTitleText: String = ""

    var body: some View {
        VStack(spacing: 8) {
            /*
            // Agent selector at top-left (temporarily hidden)
            AgentSelectorView()
                .padding(.horizontal, 12)
                .padding(.top, 8)
            */

            // New Chat button
            Button(action: { bridge.startNewSession() }) {
                HStack(spacing: 8) {
                    Image(systemName: "plus.message")
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Text("New Chat")
                        .foregroundStyle(OATheme.Colors.textPrimary)
                }
                .font(OAFonts.ui(.body, 14))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 8)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(OATheme.Colors.bgQuaternary) // medium gray from theme
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
                )
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 12)
            .padding(.top, 8)
            .keyboardShortcut("n", modifiers: .command)
            .accessibilityLabel("New Chat")

            /*
            // Search bar (temporarily hidden)
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(OATheme.Colors.textSecondary)
                TextField("Search sessions...", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(OAFonts.mono(.body, 12))
                    .focused($isSearchFocused)
                if !searchText.isEmpty {
                    Button(action: { searchText = "" }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(OATheme.Colors.background)
            )
            .padding(.horizontal, 12)
            .onExitCommand {
                if isSearchFocused && !searchText.isEmpty { searchText = "" }
            }
            */

            /* Divider hidden to simplify sidebar */
            // Divider().background(OATheme.Colors.textTertiary.opacity(0.15))

            // Session list with keyboard selection
            List(selection: $selectedSessionId) {
                ForEach(grouped.keys.sorted(by: groupSort), id: \.self) { label in
                    Section(header:
                                Text(label)
                                .font(OAFonts.mono(.caption, 11))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                    ) {
                        ForEach(grouped[label] ?? [], id: \.session_id) { s in
                            HStack(alignment: .center, spacing: 8) {
                                VStack(alignment: .leading, spacing: 2) {
                                    if editingTitleId == s.session_id {
                                        TextField("Title", text: $editingTitleText, onCommit: { commitEditTitle(for: s.session_id) })
                                            .textFieldStyle(.plain)
                                            .font(OAFonts.mono(.body, 12))
                                            .foregroundStyle(OATheme.Colors.textPrimary)
                                            .onExitCommand { cancelEditTitle() }
                                    } else {
                                        Text(title(for: s))
                                            .font(OAFonts.mono(.body, 12))
                                            .foregroundStyle(OATheme.Colors.textPrimary)
                                            .lineLimit(1)
                                            .onTapGesture(count: 2) { startEditTitle(for: s.session_id) }
                                    }
                                    HStack(spacing: 6) {
                                        Text(relativeTime(ms: s.last_ts))
                                            .font(OAFonts.mono(.caption, 11))
                                            .foregroundStyle(OATheme.Colors.textTertiary)
                                        if let m = s.mode {
                                            Text(modeLabel(m))
                                                .font(OAFonts.mono(.caption, 10))
                                                .foregroundStyle(OATheme.Colors.textSecondary)
                                                .padding(.horizontal, 6)
                                                .padding(.vertical, 2)
                                                .background(
                                                    RoundedRectangle(cornerRadius: 4, style: .continuous)
                                                        .fill(OATheme.Colors.border.opacity(0.25))
                                                )
                                        }
                                        Spacer(minLength: 0)
                                        Text("\(s.message_count) msgs")
                                            .font(OAFonts.mono(.caption, 10))
                                            .foregroundStyle(OATheme.Colors.textTertiary)
                                    }
                                }
                                Spacer(minLength: 0)
                            }
                            .tag(Optional(s.session_id))
                            .listRowBackground(Color.clear)
                            .contextMenu {
                                Button(role: .destructive) { confirmDelete(s) } label: {
                                    Label("Delete", systemImage: "trash")
                                }
                                Button { startEditTitle(for: s.session_id) } label: {
                                    Label("Rename…", systemImage: "pencil")
                                }
                                Button { bridge.clearSessionTitle(sessionId: s.session_id) } label: {
                                    Label("Reset Title", systemImage: "arrow.uturn.backward")
                                }
                            }
                            .onHover { over in hoveredId = over ? s.session_id : nil }
                        }
                    }
                }
            }
            .listStyle(.sidebar)
            .scrollContentBackground(.hidden)
            .background(
                OATheme.Colors.sidebarBackground
                    .ignoresSafeArea(.container, edges: .top)
            )
            .onChange(of: selectedSessionId) { _, newValue in
                guard let sid = newValue else { return }
                loadSession(sid)
            }

            Spacer(minLength: 0)

            // Orchestration status at bottom
            OrchestrationSidebarSection(viewModel: orchestrationVM)
        }
        .frame(minWidth: 220, idealWidth: 250, maxWidth: 280, maxHeight: .infinity)
        .background(
            OATheme.Colors.sidebarBackground
                .ignoresSafeArea(.container, edges: .top)
        )
        .onAppear {
            bridge.fetchRecentSessions()
            orchestrationVM.setRPC(bridge.connection?.rpcClient)
        }
        .onChange(of: bridge.connection?.rpcClient) { _, newValue in
            orchestrationVM.setRPC(newValue)
        }
        // Expose delete action for Commands via focused scene value
        .focusedSceneValue(\.deleteSelectedSession) { [selectedSessionId] in
            if let sid = selectedSessionId, let s = bridge.recentSessions.first(where: { $0.session_id == sid }) {
                confirmDelete(s)
            }
        }
        /* Temporarily disable Cmd-F search focus without a search field */
        // .focusedSceneValue(\.focusSidebarSearch, { isSearchFocused = true })
    }

    private var filtered: [RecentSession] {
        let source = bridge.recentSessions
        guard !searchText.isEmpty else { return source }
        let q = searchText.lowercased()
        return source.filter { s in
            let idMatch = s.session_id.lowercased().contains(q)
            let modeMatch = (s.mode ?? "").lowercased().contains(q)
            return idMatch || modeMatch
        }
    }

    private var grouped: [String: [RecentSession]] {
        Dictionary(grouping: filtered) { s in
            let date = Date(timeIntervalSince1970: TimeInterval(s.last_ts) / 1000.0)
            return groupLabel(for: date)
        }
    }

    private func groupSort(_ a: String, _ b: String) -> Bool {
        let order: [String: Int] = [
            "Today": 0,
            "Yesterday": 1,
            "Last 7 Days": 2,
            "Last 30 Days": 3,
            "Older": 4
        ]
        return (order[a] ?? 99) < (order[b] ?? 99)
    }

    private func groupLabel(for date: Date) -> String {
        let cal = Calendar.current
        if cal.isDateInToday(date) { return "Today" }
        if cal.isDateInYesterday(date) { return "Yesterday" }
        let days = cal.dateComponents([.day], from: date, to: Date()).day ?? 999
        if days < 7 { return "Last 7 Days" }
        if days < 30 { return "Last 30 Days" }
        return "Older"
    }

    private func relativeTime(ms: Int64) -> String {
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        let diff = max(0, now - ms)
        let sec = diff / 1000
        if sec < 5 { return "now" }
        if sec < 60 { return "\(sec)s" }
        let min = sec / 60
        if min < 60 { return "\(min)m" }
        let hr = min / 60
        if hr < 24 { return "\(hr)h" }
        let day = hr / 24
        if day < 7 { return "\(day)d" }
        let week = day / 7
        return "\(week)w"
    }

    @State private var showDelete = false
    @State private var pendingDeleteId: String? = nil

    private func title(for s: RecentSession) -> String {
        if let t = bridge.conversationTitles[s.session_id], !t.isEmpty { return t }
        return String(s.session_id.prefix(12)) + "…"
    }

    private func startEditTitle(for id: String) {
        editingTitleId = id
        editingTitleText = bridge.conversationTitles[id] ?? ""
    }

    private func commitEditTitle(for id: String) {
        let newTitle = editingTitleText.trimmingCharacters(in: .whitespacesAndNewlines)
        if !newTitle.isEmpty { bridge.conversationTitles[id] = newTitle }
        if !newTitle.isEmpty { bridge.setSessionTitle(sessionId: id, title: newTitle) }
        editingTitleId = nil
        editingTitleText = ""
    }

    private func cancelEditTitle() {
        editingTitleId = nil
        editingTitleText = ""
    }

    private func rowBackground(for s: RecentSession) -> Color { // retained for hover styling in List
        if hoveredId == s.session_id { return OATheme.Colors.textSecondary.opacity(0.06) }
        return Color.clear
    }

    private func loadSession(_ sessionId: String) {
        bridge.currentSessionId = nil
        bridge.timeline.clearAll()
        bridge.loadSessionTimeline(sessionId: sessionId)
    }

    private func modeLabel(_ mode: String) -> String {
        switch mode {
        case "codex": return "Codex"
        case "claude-code", "claude_code": return "Claude"
        case "default_mode": return "Claude"
        default: return mode.capitalized
        }
    }

    // No filesystem fallback: Tinyvex (SQLite) is the single source of truth

    private func confirmDelete(_ s: RecentSession) {
        pendingDeleteId = s.session_id
        let alert = NSAlert()
        alert.messageText = "Delete Session?"
        alert.informativeText = "This will permanently delete the selected chat session."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Delete")
        if alert.runModal() == .alertSecondButtonReturn {
            bridge.deleteSession(s.session_id)
        }
    }
}
#endif
