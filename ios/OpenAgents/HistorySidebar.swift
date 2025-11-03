import SwiftUI
#if canImport(OpenAgentsCore)
import OpenAgentsCore
#endif

struct HistorySidebar: View {
    #if canImport(OpenAgentsCore)
    @State private var rows: [ThreadSummary] = []
    @State private var isLoading = false
    #endif

    var body: some View {
        #if canImport(OpenAgentsCore)
        List {
            Section(header: Label("History", systemImage: "clock")) {
                if isLoading && rows.isEmpty {
                    HStack {
                        ProgressView()
                        Text("Loading…")
                    }
                }
                ForEach(rows.prefix(10), id: \.id) { row in
                    NavigationLink(value: row.id) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(nonEmptyTitle(row) ?? "Thread")
                                .font(.body)
                                .lineLimit(1)
                            HStack(spacing: 8) {
                                if let ts = (row.last_message_ts ?? row.updated_at) as Int64? {
                                    Label(relative(ts), systemImage: "time")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                providerBadge(for: row.source)
                            }
                        }
                    }
                }
            }
        }
        .navigationTitle("OpenAgents")
        .onAppear(perform: load)
        #else
        List {
            Section(header: Text("History")) {
                Text("Add local package 'OpenAgentsCore' to enable history.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .navigationTitle("OpenAgents")
        #endif
    }

    #if canImport(OpenAgentsCore)
    private func load() {
        guard !isLoading else { return }
        isLoading = true
        DispatchQueue.global(qos: .userInitiated).async {
            // Focus on Codex first for reliability
            let loaded = HistoryLoader.loadSummaries(.init(includeCodex: true, includeClaude: false, maxFilesPerProvider: 400, maxResults: 200))
            DispatchQueue.main.async {
                withAnimation { self.rows = loaded }
                self.isLoading = false
            }
        }
    }

    private func nonEmptyTitle(_ row: ThreadSummary) -> String? {
        if let t = row.title?.trimmingCharacters(in: .whitespacesAndNewlines), !t.isEmpty { return t }
        return nil
    }

    @ViewBuilder
    private func providerBadge(for source: String) -> some View {
        let s = source.lowercased()
        if s == "codex" {
            Label("Codex", systemImage: "curlybraces")
                .font(.caption)
                .foregroundStyle(.secondary)
        } else if s == "claude_code" || s == "claude" {
            Label("Claude Code", systemImage: "bolt")
                .font(.caption)
                .foregroundStyle(.secondary)
        } else {
            EmptyView()
        }
    }

    private func relative(_ ms: Int64) -> String {
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        let diff = max(0, now - ms)
        let sec = diff / 1000
        if sec < 5 { return "just now" }
        if sec < 60 { return "\(sec)s ago" }
        let min = sec / 60
        if min < 60 { return "\(min)m ago" }
        let hr = min / 60
        if hr < 24 { return "\(hr)h ago" }
        let day = hr / 24
        if day < 7 { return "\(day)d ago" }
        let week = day / 7
        return "\(week)w ago"
    }
    #endif
}

#Preview {
    NavigationSplitView {
        HistorySidebar()
    } detail: {
        Text("Select a thread")
    }
}
