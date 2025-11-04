import SwiftUI
import OpenAgentsCore

struct HistorySidebar: View {
    var selected: LocalThreadSummary? = nil
    var onSelect: ((LocalThreadSummary, URL?) -> Void)? = nil
    @State private var items: [(LocalThreadSummary, URL?)] = []
    @State private var isLoading = false
    @State private var debugLines: [String] = []
    @State private var titles: [String: String] = [:] // key: "source::id" → summary title

    var body: some View {
        List {
            Section(header: Label("History", systemImage: "clock")) {
                if isLoading && items.isEmpty {
                    HStack {
                        ProgressView()
                        Text("Loading…")
                            .font(.caption)
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                }
                if items.isEmpty && !isLoading {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("No chats found")
                            .font(.caption)
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        if !debugLines.isEmpty {
                            ForEach(debugLines.prefix(8), id: \.self) { line in
                                Text(line).font(.footnote).foregroundStyle(OATheme.Colors.textSecondary).lineLimit(2)
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
                            Text(displayTitle(for: row) ?? "Thread")
                                .font(.body)
                                .fontWeight(isActive ? .semibold : .regular)
                                .foregroundStyle(OATheme.Colors.textPrimary)
                                .lineLimit(1)
                            HStack(spacing: 8) {
                                if let ts = (row.last_message_ts ?? row.updated_at) as Int64? {
                                    Label(relative(ts), systemImage: "clock")
                                        .font(.caption)
                                        .foregroundStyle(OATheme.Colors.textTertiary)
                                }
                                providerBadge(for: row.source)
                            }
                        }
                    }
                    .listRowBackground(isActive ? OATheme.Colors.selection : Color.clear)
                    .contentShape(Rectangle())
                }
            }
        }
        .scrollContentBackground(.hidden)
        .background(OATheme.Colors.sidebarBackground)
        .navigationTitle("")
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
                // Try cache + async generation for visible Codex rows
                for (row, url) in merged.prefix(20) {
                    guard let u = url else { continue }
                    let rowKey = key(for: row)
                    let mtime = LocalCodexScanner.fileMTime(u)
                    Task {
                        if let cached = await TitleCache.shared.get(path: u.path, mtime: mtime) {
                            await MainActor.run { self.titles[rowKey] = cached }
                        } else {
                            summarizeRow(row, url: u, mtime: mtime)
                        }
                    }
                }
            }
        }
    }

    private func nonEmptyTitle(_ row: LocalThreadSummary) -> String? {
        if let t = row.title?.trimmingCharacters(in: .whitespacesAndNewlines), !t.isEmpty { return t }
        return nil
    }

    private func key(for row: LocalThreadSummary) -> String { "\(row.source)::\(row.id)" }

    private func displayTitle(for row: LocalThreadSummary) -> String? {
        if let t = nonEmptyTitle(row) { return t }
        return titles[key(for: row)]
    }

    private func summarizeRow(_ row: LocalThreadSummary, url: URL, mtime: Int64) {
        let rowKey = key(for: row)
        if titles[rowKey] != nil { return }
        Task.detached(priority: .utility) {
            do {
                // Prefer head scan to capture the very first user message
                var strategy = "first_user_5"
                var lines = try headJSONLLines(url: url, maxBytes: 600_000, maxLines: 4000)
                var thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: url.path))
                var msgs = thread.events.compactMap { $0.message }.filter { $0.role == .user || $0.role == .assistant }
                msgs.sort { $0.ts < $1.ts }
                var title = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: true)
                if title.isEmpty {
                    // Fallback to tail if head chunk didn’t include first user text
                    strategy = "tail_fallback"
                    lines = try tailJSONLLines(url: url, maxBytes: 600_000, maxLines: 4000)
                    thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: url.path))
                    msgs = thread.events.compactMap { $0.message }.filter { $0.role == .user || $0.role == .assistant }
                    msgs.sort { $0.ts < $1.ts }
                    title = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: true)
                }
                if !title.isEmpty { print("[Titles] \(row.source)::\(row.id) strategy=\(strategy) title=\(title)") }
                guard !title.isEmpty else { return }
                let finalTitle = title
                await MainActor.run { self.titles[rowKey] = finalTitle }
                await TitleCache.shared.set(path: url.path, mtime: mtime, title: finalTitle)
            } catch {
                // ignore errors
            }
        }
    }

    @ViewBuilder
    private func providerBadge(for source: String) -> some View {
        let s = source.lowercased()
        if s == "codex" {
            Label("Codex", systemImage: "curlybraces")
                .font(.caption)
                .foregroundStyle(OATheme.Colors.textTertiary)
        } else if s == "claude_code" || s == "claude" {
            Label("Claude Code", systemImage: "bolt")
                .font(.caption)
                .foregroundStyle(OATheme.Colors.textTertiary)
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

// Efficient JSONL tail reader to avoid reading huge files fully
nonisolated private func tailJSONLLines(url: URL, maxBytes: Int, maxLines: Int) throws -> [String] {
    let fh = try FileHandle(forReadingFrom: url)
    defer { try? fh.close() }
    let chunk = 64 * 1024
    let fileSize = (try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.intValue ?? 0
    var offset = fileSize
    var buffer = Data()
    var totalRead = 0
    while offset > 0 && totalRead < maxBytes {
        let toRead = min(chunk, offset)
        offset -= toRead
        try fh.seek(toOffset: UInt64(offset))
        let data = try fh.read(upToCount: toRead) ?? Data()
        buffer.insert(contentsOf: data, at: 0) // prepend
        totalRead += data.count
        if buffer.count >= maxBytes { break }
    }
    var text = String(data: buffer, encoding: .utf8) ?? String(decoding: buffer, as: UTF8.self)
    if !text.hasSuffix("\n") { text.append("\n") }
    var lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
    if lines.count > maxLines { lines = Array(lines.suffix(maxLines)) }
    return lines
}

// Efficient head reader: read the first chunk of a JSONL file
nonisolated private func headJSONLLines(url: URL, maxBytes: Int, maxLines: Int) throws -> [String] {
    let fh = try FileHandle(forReadingFrom: url)
    defer { try? fh.close() }
    let toRead = min(maxBytes, (try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.intValue ?? maxBytes)
    let data = try fh.read(upToCount: toRead) ?? Data()
    var text = String(data: data, encoding: .utf8) ?? String(decoding: data, as: UTF8.self)
    if !text.hasSuffix("\n") { text.append("\n") }
    var lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
    if lines.count > maxLines { lines = Array(lines.prefix(maxLines)) }
    return lines
}

#Preview {
    NavigationSplitView {
        HistorySidebar(selected: nil)
    } detail: {
        Text("Select a thread")
    }
}
