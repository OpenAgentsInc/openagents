import SwiftUI
import OpenAgentsCore

#if os(macOS)
struct SessionSidebarView: View {
    @EnvironmentObject private var bridge: BridgeManager
    @State private var searchText: String = ""
    @State private var hoveredId: String? = nil
    @State private var fallbackSessions: [RecentSession] = []

    var body: some View {
        VStack(spacing: 8) {
            // New Chat button
            Button(action: { bridge.startNewSession() }) {
                HStack(spacing: 8) {
                    Image(systemName: "plus.message")
                    Text("New Chat")
                }
                .font(OAFonts.ui(.body, 14))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 8)
            }
            .buttonStyle(.borderedProminent)
            .tint(OATheme.Colors.accent)
            .padding(.horizontal, 12)
            .padding(.top, 8)

            // Search bar
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(OATheme.Colors.textSecondary)
                TextField("Search sessions...", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(OAFonts.mono(.body, 12))
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

            Divider().background(OATheme.Colors.textTertiary.opacity(0.15))

            // Session list
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 4) {
                    ForEach(grouped.keys.sorted(by: groupSort), id: \.self) { label in
                        Text(label)
                            .font(OAFonts.mono(.caption, 11))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)

                        ForEach(grouped[label] ?? [], id: \.session_id) { s in
                            Button(action: { loadSession(s.session_id) }) {
                                HStack(alignment: .center, spacing: 8) {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(title(for: s))
                                            .font(OAFonts.mono(.body, 12))
                                            .foregroundStyle(OATheme.Colors.textPrimary)
                                            .lineLimit(1)
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
                                .padding(.horizontal, 12)
                                .padding(.vertical, 8)
                                .background(rowBackground(for: s))
                                .cornerRadius(8)
                            }
                            .buttonStyle(.plain)
                            .onHover { over in hoveredId = over ? s.session_id : nil }
                        }
                    }
                }
                .padding(.vertical, 6)
            }

            Spacer(minLength: 0)
        }
        .frame(minWidth: 220, idealWidth: 250, maxWidth: 280, maxHeight: .infinity)
        .background(OATheme.Colors.sidebarBackground)
        .onAppear {
            bridge.fetchRecentSessions()
            if bridge.recentSessions.isEmpty {
                loadFallbackFromFilesystem()
            }
        }
    }

    private var filtered: [RecentSession] {
        let source = bridge.recentSessions.isEmpty ? fallbackSessions : bridge.recentSessions
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

    private func title(for s: RecentSession) -> String {
        // Placeholder title until FM summarization integrates: id prefix
        return String(s.session_id.prefix(12)) + "…"
    }

    private func rowBackground(for s: RecentSession) -> Color {
        if let current = bridge.currentSessionId?.value, current == s.session_id {
            return OATheme.Colors.accent.opacity(0.18)
        }
        if hoveredId == s.session_id {
            return OATheme.Colors.textSecondary.opacity(0.06)
        }
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

    private func loadFallbackFromFilesystem() {
        DispatchQueue.global(qos: .userInitiated).async {
            // Scan Claude and Codex default locations for top recent sessions
            let claude = ClaudeScanner.scanTopK(topK: 20)
            let codex = CodexScanner.scanTopK(topK: 20)
            let rows = (claude + codex).sorted { ($0.last_message_ts ?? $0.updated_at) > ($1.last_message_ts ?? $1.updated_at) }
            let mapped: [RecentSession] = rows.map { r in
                RecentSession(
                    session_id: r.id,
                    last_ts: r.last_message_ts ?? r.updated_at,
                    message_count: Int64(r.message_count ?? 0),
                    mode: r.source
                )
            }
            DispatchQueue.main.async {
                self.fallbackSessions = mapped
            }
        }
    }
}
#endif
