import SwiftUI
import OSLog
import OpenAgentsCore
import Combine

struct HistorySidebar: View {
    var selected: LocalThreadSummary? = nil
    var onSelect: ((LocalThreadSummary, URL?) -> Void)? = nil
    @State private var items: [(LocalThreadSummary, URL?)] = []
    @State private var isLoading = false
    @State private var debugLines: [String] = []
    @State private var titles: [String: String] = [:] // key: "source::id" → summary title

    @EnvironmentObject var bridge: BridgeManager

    var body: some View {
        List {
            listContent
        }
        .scrollContentBackground(.hidden)
        .background(OATheme.Colors.sidebarBackground)
        .navigationTitle("")
        .onAppear(perform: load)
        #if os(iOS)
        .onChange(of: bridge.threads) { _, newVal in
            self.isLoading = false
            self.items = self.mapThreadsToItems(newVal)
        }
        .onChange(of: bridge.recentSessions) { _, newVal in
            self.isLoading = false
            self.items = self.mapRecentSessionsToItems(newVal)
        }
        #endif
    }

    @ViewBuilder
    private var listContent: some View {
        // Bridge status chip (desktop + mobile)
        BridgeStatusChip()
            .listRowBackground(Color.clear)

        HStack(spacing: 8) {
            Image(systemName: "clock").imageScale(.small)
                .foregroundStyle(OATheme.Colors.textTertiary)
            Text("History")
                .font(OAFonts.ui(.caption, 12))
                .foregroundStyle(OATheme.Colors.textTertiary)
        }
        .listRowBackground(Color.clear)
        #if os(iOS)
        .listRowSeparator(.hidden)
        #endif

        Group {
            if isLoading && effectiveItems().isEmpty {
                HStack {
                    ProgressView()
                    Text("Loading…")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
            }
            if effectiveItems().isEmpty && !isLoading {
                VStack(alignment: .leading, spacing: 6) {
                    Text("No chats found")
                        .font(OAFonts.ui(.caption, 12))
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
        }
        HistoryListView(
            items: effectiveItems(),
            selected: selected,
            limit: 10,
            titleFor: { row in displayTitle(for: row) },
            onSelect: onSelect
        )
    }

    

    // Helper methods to reduce View body complexity
    private func mapThreadsToItems(_ threads: [ThreadSummary]) -> [(LocalThreadSummary, URL?)] {
        threads.map { t in
            (LocalThreadSummary(
                id: t.id,
                title: t.title,
                source: t.source,
                created_at: t.created_at,
                updated_at: t.updated_at,
                last_message_ts: t.last_message_ts,
                message_count: t.message_count
            ), nil)
        }
    }

    private func mapRecentSessionsToItems(_ sessions: [RecentSession]) -> [(LocalThreadSummary, URL?)] {
        sessions.map { session in
            (LocalThreadSummary(
                id: session.session_id,
                title: nil,
                source: "tinyvex",
                created_at: nil,
                updated_at: session.last_ts,
                last_message_ts: session.last_ts,
                message_count: Int(session.message_count)
            ), nil)
        }
    }

    private func load() {
        guard !isLoading else { return }
        isLoading = true
        DispatchQueue.global(qos: .userInitiated).async {
            var dbg: [String] = []
            // iOS path: load recent sessions from Tinyvex DB
            #if os(iOS)
            DispatchQueue.main.async {
                self.isLoading = true
                self.bridge.fetchRecentSessions()
            }
            return
            #endif
            #if os(macOS)
            // macOS path: prioritize Claude Code, optionally include Codex
            var merged: [(LocalThreadSummary, URL?)] = []

            // Load Claude Code first (now default)
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
                merged = claudeRows.map { ($0, nil) }
                dbg.append("claudeCount=\(claudeRows.count)")
            }

            // Optionally include Codex
            let codexBases = LocalCodexDiscovery.discoverBaseDirs()
            var codexURLs: [URL] = []
            for b in codexBases { codexURLs.append(contentsOf: LocalCodexScanner.listRecentTopN(at: b, topK: 10)) }
            codexURLs.sort { LocalCodexScanner.fileMTime($0) > LocalCodexScanner.fileMTime($1) }
            if codexURLs.count > 10 { codexURLs = Array(codexURLs.prefix(10)) }
            let codexRows = codexURLs.map { url -> (LocalThreadSummary, URL?) in
                let base = codexBases.first { url.path.hasPrefix($0.path) } ?? url.deletingLastPathComponent()
                let id = LocalCodexScanner.scanForThreadID(url) ?? LocalCodexScanner.relativeId(for: url, base: base)
                let updated = LocalCodexScanner.fileMTime(url)
                let row = LocalThreadSummary(id: id, title: nil, source: "codex", created_at: nil, updated_at: updated, last_message_ts: nil, message_count: nil)
                return (row, url)
            }
            merged += codexRows
            dbg.append("codexCount=\(codexURLs.count)")

            // Sort by most recent, prioritizing Claude Code when timestamps are equal
            merged.sort { a, b in
                let aTs = a.0.last_message_ts ?? a.0.updated_at
                let bTs = b.0.last_message_ts ?? b.0.updated_at
                if aTs == bTs {
                    // Prioritize Claude Code when timestamps match
                    return a.0.source == "claude_code" && b.0.source != "claude_code"
                }
                return aTs > bTs
            }
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
            #endif
        }
    }

    private func effectiveItems() -> [(LocalThreadSummary, URL?)] {
        // On iOS, items are already populated by onChange observers
        return items
    }

    private func nonEmptyTitle(_ row: LocalThreadSummary) -> String? {
        if let t = row.title?.trimmingCharacters(in: .whitespacesAndNewlines), !t.isEmpty { return t }
        return nil
    }

    private func key(for row: LocalThreadSummary) -> String { "\(row.source)::\(row.id)" }

    private func displayTitle(for row: LocalThreadSummary) -> String? {
        if let t = nonEmptyTitle(row) { return sanitizeTitle(t) }
        if let t = titles[key(for: row)] { return sanitizeTitle(t) }
        return nil
    }

    private func sanitizeTitle(_ s: String) -> String {
        var t = s
        // Basic markdown removal: links [text](url) → text
        if let rx = try? NSRegularExpression(pattern: "\\[([^\\]]+)\\]\\([^\\)]+\\)", options: []) {
            t = rx.stringByReplacingMatches(in: t, range: NSRange(location: 0, length: t.utf16.count), withTemplate: "$1")
        }
        // Strip emphasis/code markers
        for mark in ["**","*","__","_","```,","`"] { t = t.replacingOccurrences(of: mark, with: "") }
        // Collapse whitespace
        t = t.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        return t.trimmingCharacters(in: .whitespacesAndNewlines)
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
                var title = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: Features.foundationModelsEnabled)
                if title.isEmpty {
                    // Fallback to tail if head chunk didn’t include first user text
                    strategy = "tail_fallback"
                    lines = try tailJSONLLines(url: url, maxBytes: 600_000, maxLines: 4000)
                    thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: url.path))
                    msgs = thread.events.compactMap { $0.message }.filter { $0.role == .user || $0.role == .assistant }
                    msgs.sort { $0.ts < $1.ts }
                    title = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: Features.foundationModelsEnabled)
                }
                if !title.isEmpty { OpenAgentsLog.ui.info("Titles \(row.source)::\(row.id) strategy=\(strategy) title=\(title, privacy: .public)") }
                guard !title.isEmpty else { return }
                let finalTitle = title
                await MainActor.run { self.titles[rowKey] = finalTitle }
                await TitleCache.shared.set(path: url.path, mtime: mtime, title: finalTitle)
            } catch {
                // ignore errors
            }
        }
    }

    // Provider badges removed per request.

    // relative() moved into HistorySessionRow component
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
