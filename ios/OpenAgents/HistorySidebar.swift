import SwiftUI

struct HistorySidebar: View {
    @State private var rows: [LocalThreadSummary] = []
    @State private var isLoading = false
    @State private var debugLines: [String] = []

    var body: some View {
        List {
            Section(header: Label("History", systemImage: "clock")) {
                if isLoading && rows.isEmpty {
                    HStack {
                        ProgressView()
                        Text("Loading…")
                    }
                }
                if !rows.isEmpty {
                    Text("\(rows.count) chats found")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else if !isLoading {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("No chats found in Codex folders")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        if !debugLines.isEmpty {
                            ForEach(debugLines.prefix(8), id: \.self) { line in
                                Text(line).font(.footnote).foregroundStyle(.secondary).lineLimit(2)
                            }
                        }
                        #if os(macOS)
                        Button("Select Codex Folder…", action: selectCodexFolder)
                            .buttonStyle(.bordered)
                        #endif
                    }
                }
                ForEach(rows.prefix(10), id: \.uniqueKey) { row in
                    NavigationLink(value: row.id) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(nonEmptyTitle(row) ?? "Thread")
                                .font(.body)
                                .lineLimit(1)
                            HStack(spacing: 8) {
                                if let ts = (row.last_message_ts ?? row.updated_at) as Int64? {
                                    Label(relative(ts), systemImage: "clock")
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
    }

    private func load() {
        guard !isLoading else { return }
        isLoading = true
        DispatchQueue.global(qos: .userInitiated).async {
            // Discover both providers and load top-10 each
            let codexBases = LocalCodexDiscovery.discoverBaseDirs()
            var dbg: [String] = []
            let home = FileManager.default.homeDirectoryForCurrentUser.path
            let home2 = NSHomeDirectory()
            let def1 = URL(fileURLWithPath: home).appendingPathComponent(".codex/sessions", isDirectory: true).path
            let def2 = URL(fileURLWithPath: home2).appendingPathComponent(".codex/sessions", isDirectory: true).path
            let env = ProcessInfo.processInfo.environment["CODEXD_HISTORY_DIR"] ?? ""
            dbg.append("env CODEXD_HISTORY_DIR=\(env)")
            dbg.append("home=\(home) home2=\(home2)")
            dbg.append("defaultBaseCandidates=[\(def1), \(def2)]")
            dbg.append("codexBases=\(codexBases.map{ $0.path })")
            let codexRows = LocalCodexDiscovery.loadAllSummaries(topK: 10)
            let claudeBases = LocalClaudeDiscovery.defaultBases()
            dbg.append("claudeBases=\(claudeBases.map{ $0.path })")
            var claudeURLs: [URL] = []
            for b in claudeBases { claudeURLs.append(contentsOf: LocalClaudeDiscovery.listRecentTopN(at: b, topK: 10)) }
            let claudeRows: [LocalThreadSummary] = claudeURLs.map { url in
                let baseFor = claudeBases.first { url.path.hasPrefix($0.path) }
                return LocalClaudeDiscovery.makeSummary(for: url, base: baseFor)
            }
            var merged = codexRows + claudeRows
            merged.sort { ($0.last_message_ts ?? $0.updated_at) > ($1.last_message_ts ?? $1.updated_at) }
            if merged.count > 20 { merged = Array(merged.prefix(20)) }
            DispatchQueue.main.async {
                withAnimation { self.rows = merged }
                self.isLoading = false
                self.debugLines = dbg
            }
        }
    }

    private func nonEmptyTitle(_ row: LocalThreadSummary) -> String? {
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
}

#if os(macOS)
import AppKit
extension HistorySidebar {
    private func selectCodexFolder() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.title = "Select Codex sessions folder"
        panel.message = "Pick your .codex/sessions folder (or a parent containing JSONL)."
        if panel.runModal() == .OK, let url = panel.url {
            LocalCodexDiscovery.setUserOverride(url)
            self.isLoading = false
            self.rows = []
            self.debugLines = ["override=\(url.path)"]
            self.load()
        }
    }
}
#endif

#Preview {
    NavigationSplitView {
        HistorySidebar()
    } detail: {
        Text("Select a thread")
    }
}
