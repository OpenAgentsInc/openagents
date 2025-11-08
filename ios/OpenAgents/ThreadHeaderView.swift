import SwiftUI
import OpenAgentsCore

struct ThreadHeaderView: View {
    let row: LocalThreadSummary
    let url: URL

    @State private var title: String = ""
    @State private var isWorking = false

    var body: some View {
        GlassBar {
            HStack(spacing: 8) {
                Image(systemName: "text.bubble")
                    .foregroundStyle(OATheme.Colors.textSecondary)
                Text(title.isEmpty ? "Thread" : title)
                            .font(InterFont.font(relativeTo: .subheadline, size: 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .lineLimit(1)
                Spacer()
                Button(action: regenerate) {
                    if isWorking {
                        ProgressView().controlSize(.small)
                    } else {
                        Image(systemName: "arrow.clockwise")
                    }
                }
                .buttonStyle(.plain)
                .foregroundStyle(OATheme.Colors.textSecondary)
                .help("Regenerate title")
            }
        }
        .onAppear(perform: loadCachedOrGenerate)
    }

    private func loadCachedOrGenerate() {
        Task {
            let mtime = fileMTime(url)
            if let cached = await TitleCache.shared.get(path: url.path, mtime: mtime) {
                await MainActor.run { self.title = cached }
            } else {
                regenerate()
            }
        }
    }

    private func regenerate() {
        if isWorking { return }
        isWorking = true
        Task.detached(priority: .userInitiated) {
            do {
                var lines = try headJSONLLines(url: url, maxBytes: 600_000, maxLines: 4000)
                var thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: url.path))
                var msgs = thread.events.compactMap { $0.message }.filter { $0.role == .user || $0.role == .assistant }
                // skip system/preface
                msgs = msgs.filter { m in
                    let text = m.parts.compactMap { part -> String? in
                        if case let .text(t) = part { return t.text } else { return nil }
                    }.joined(separator: " ")
                    return !ConversationSummarizer.isSystemPreface(text)
                }
                msgs.sort { $0.ts < $1.ts }
                var newTitle = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: Features.foundationModelsEnabled)
                if newTitle.isEmpty {
                    lines = try tailJSONLLines(url: url, maxBytes: 600_000, maxLines: 4000)
                    thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: url.path))
                    msgs = thread.events.compactMap { $0.message }.filter { $0.role == .user || $0.role == .assistant }
                    msgs = msgs.filter { m in
                        let text = m.parts.compactMap { part -> String? in
                            if case let .text(t) = part { return t.text } else { return nil }
                        }.joined(separator: " ")
                        return !ConversationSummarizer.isSystemPreface(text)
                    }
                    msgs.sort { $0.ts < $1.ts }
                    newTitle = await ConversationSummarizer.summarizeTitle(messages: msgs, preferOnDeviceModel: Features.foundationModelsEnabled)
                }
                let mtime = fileMTime(url)
                if !newTitle.isEmpty {
                    let finalTitle = newTitle
                    OpenAgentsLog.ui.info("TitleHeader regenerated \(row.id) title=\(finalTitle, privacy: .public)")
                    await TitleCache.shared.set(path: url.path, mtime: mtime, title: finalTitle)
                    await MainActor.run { self.title = finalTitle }
                }
            } catch {
                // ignore errors
            }
            await MainActor.run { self.isWorking = false }
        }
    }
}

// Local helpers (duplicated for minimal coupling)
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
        buffer.insert(contentsOf: data, at: 0)
        totalRead += data.count
        if buffer.count >= maxBytes { break }
    }
    var text = String(data: buffer, encoding: .utf8) ?? String(decoding: buffer, as: UTF8.self)
    if !text.hasSuffix("\n") { text.append("\n") }
    var lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
    if lines.count > maxLines { lines = Array(lines.suffix(maxLines)) }
    return lines
}

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

nonisolated private func fileMTime(_ url: URL) -> Int64 {
    let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
    if let m = attrs?[.modificationDate] as? Date { return Int64(m.timeIntervalSince1970 * 1000) }
    return Int64(Date().timeIntervalSince1970 * 1000)
}
