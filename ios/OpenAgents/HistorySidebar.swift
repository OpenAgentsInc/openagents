import SwiftUI

struct HistorySidebar: View {
    var selected: LocalThreadSummary? = nil
    var onSelect: ((LocalThreadSummary, URL?) -> Void)? = nil
    @State private var items: [(LocalThreadSummary, URL?)] = []
    @State private var isLoading = false
    @State private var debugLines: [String] = []

    var body: some View {
        List {
            Section(header: Label("History", systemImage: "clock")) {
                if isLoading && items.isEmpty {
                    HStack {
                        ProgressView()
                        Text("Loading…")
                    }
                }
                if !items.isEmpty {
                    Text("\(items.count) chats found")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else if !isLoading {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("No chats found")
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
                ForEach(Array(items.prefix(20).enumerated()), id: \.0) { _, pair in
                    let row = pair.0
                    let isActive = (selected?.id == row.id && selected?.source == row.source)
                    Button(action: { onSelect?(row, pair.1) }) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(nonEmptyTitle(row) ?? "Thread")
                                .font(.body)
                                .fontWeight(isActive ? .semibold : .regular)
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
                    .listRowBackground(isActive ? Color.accentColor.opacity(0.12) : Color.clear)
                    .contentShape(Rectangle())
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
            var dbg: [String] = []
            // Fast path: Codex only by default; Claude behind feature flag
            let codexBases = LocalCodexDiscovery.discoverBaseDirs()
            var codexURLs: [URL] = []
            for b in codexBases { codexURLs.append(contentsOf: LocalCodexScanner.listRecentTopN(at: b, topK: 10)) }
            codexURLs.sort { LocalCodexScanner.fileMTime($0) > LocalCodexScanner.fileMTime($1) }
            if codexURLs.count > 10 { codexURLs = Array(codexURLs.prefix(10)) }
            var merged: [(LocalThreadSummary, URL?)] = codexURLs.map { url in
                let base = codexBases.first { url.path.hasPrefix($0.path) } ?? url.deletingLastPathComponent()
                let id = LocalCodexScanner.scanForThreadID(url) ?? LocalCodexScanner.relativeId(for: url, base: base)
                let updated = LocalCodexScanner.fileMTime(url)
                let row = LocalThreadSummary(id: id, title: nil, source: "codex", created_at: nil, updated_at: updated, last_message_ts: nil, message_count: nil)
                return (row, url)
            }
            if Features.claudeEnabled {
                let claudeExact = LocalClaudeDiscovery.scanExactProjectTopK(topK: 10)
                dbg.append("claudeExactProjectCount=\(claudeExact.count)")
                var claudeRows = claudeExact
                if claudeRows.isEmpty {
                    let claudeBases = LocalClaudeDiscovery.defaultBases()
                    dbg.append("claudeBases=\(claudeBases.map{ $0.path })")
                    var urls: [URL] = []
                    for b in claudeBases { urls.append(contentsOf: LocalClaudeDiscovery.listRecentTopN(at: b, topK: 10)) }
                    dbg.append("claudeTopKURLs=\(urls.map{ $0.lastPathComponent })")
                    claudeRows = urls.map { url in
                        let baseFor = claudeBases.first { url.path.hasPrefix($0.path) }
                        return LocalClaudeDiscovery.makeSummary(for: url, base: baseFor)
                    }
                }
                dbg.append("codexCount=\(codexURLs.count) claudeCount=\(claudeRows.count)")
                merged += claudeRows.map { ($0, nil) }
            } else {
                dbg.append("claude=disabled")
                dbg.append("codexCount=\(codexURLs.count) claudeCount=0")
            }
            // Sort only what we have; Codex-only is already top-10
            merged.sort { ($0.0.last_message_ts ?? $0.0.updated_at) > ($1.0.last_message_ts ?? $1.0.updated_at) }
            if merged.count > 20 { merged = Array(merged.prefix(20)) }
            DispatchQueue.main.async {
                withAnimation { self.items = merged }
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
            self.items = []
            self.debugLines = ["override=\(url.path)"]
            self.load()
        }
    }
}
#endif

#Preview {
    NavigationSplitView {
        HistorySidebar(selected: nil)
    } detail: {
        Text("Select a thread")
    }
}
