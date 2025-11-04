import Foundation

public enum CodexScanner {
    public struct Options {
        public var baseDir: URL?
        public var maxFiles: Int
        public init(baseDir: URL? = nil, maxFiles: Int = 200) {
            self.baseDir = baseDir
            self.maxFiles = maxFiles
        }
    }

    public static func defaultBaseDir() -> URL {
        if let env = ProcessInfo.processInfo.environment["CODEXD_HISTORY_DIR"], !env.isEmpty {
            return URL(fileURLWithPath: env).standardizedFileURL
        }
        #if os(macOS)
        let home = FileManager.default.homeDirectoryForCurrentUser
        return home.appendingPathComponent(".codex/sessions", isDirectory: true)
        #else
        // iOS and other platforms: no direct desktop FS. Return a non-existent placeholder.
        return URL(fileURLWithPath: "/nonexistent")
        #endif
    }

    public static func listJSONLFiles(at base: URL) -> [URL] {
        var out: [URL] = []
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(at: base, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) else { return out }
        for case let url as URL in enumerator {
            if url.lastPathComponent == "openagents" { enumerator.skipDescendants(); continue }
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                continue
            }
            if url.pathExtension.lowercased() == "jsonl" {
                // Quick new-format check: presence of '"type"' near head
                if isNewFormatJSONL(url) {
                    out.append(url)
                }
            }
        }
        return out
    }

    /// Fast top-N file finder tailored for Codex sessions shape: base/YYYY/MM/DD/*.jsonl
    /// Falls back to a shallow enumerator if the shape isn't present.
    public static func listRecentTopN(at base: URL, topK: Int) -> [URL] {
        var picked: [URL] = []
        let fm = FileManager.default
        // Try shaped walk first
        let years = (try? fm.contentsOfDirectory(at: base, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]))?.filter { (try? $0.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true && $0.lastPathComponent.count == 4 } ?? []
        if !years.isEmpty {
            let ySorted = years.sorted { $0.lastPathComponent > $1.lastPathComponent }
            for y in ySorted {
                let months = (try? fm.contentsOfDirectory(at: y, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]))?.filter { (try? $0.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true } ?? []
                let mSorted = months.sorted { $0.lastPathComponent > $1.lastPathComponent }
                for m in mSorted {
                    let days = (try? fm.contentsOfDirectory(at: m, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]))?.filter { (try? $0.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true } ?? []
                    let dSorted = days.sorted { $0.lastPathComponent > $1.lastPathComponent }
                    for d in dSorted {
                        var files = (try? fm.contentsOfDirectory(at: d, includingPropertiesForKeys: [.contentModificationDateKey], options: [.skipsHiddenFiles]))?.filter { $0.pathExtension.lowercased() == "jsonl" } ?? []
                        files.sort { fileMTime($0) > fileMTime($1) }
                        for f in files { picked.append(f); if picked.count >= topK { return picked } }
                    }
                }
            }
        }
        // Fallback: shallow enumerator, collect topK by mtime without content checks
        var all: [URL] = []
        if let en = fm.enumerator(at: base, includingPropertiesForKeys: [.isDirectoryKey, .contentModificationDateKey], options: [.skipsHiddenFiles]) {
            for case let url as URL in en {
                if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true { continue }
                if url.pathExtension.lowercased() == "jsonl" { all.append(url) }
            }
        }
        all.sort { fileMTime($0) > fileMTime($1) }
        if all.count > topK { all = Array(all.prefix(topK)) }
        return all
    }

    public static func makeSummary(for url: URL, base: URL) -> ThreadSummary {
        let id = scanForThreadID(url) ?? relativeId(for: url, base: base)
        let updated = fileMTime(url)
        let lastTs = tailLastMessageTs(url) ?? updated
        let title = quickTitle(for: url)
        return ThreadSummary(
            id: id,
            title: title,
            source: "codex",
            created_at: nil,
            updated_at: updated,
            last_message_ts: lastTs,
            message_count: nil
        )
    }

    /// Quickly derive a short title from a JSONL session by taking the first
    /// non-preface user message (~5 words). Keeps this lightweight for responsiveness.
    /// Tries a small head read first, then (if needed) a small tail read fallback.
    public static func quickTitle(for url: URL, maxBytes: Int = 300_000, maxLines: Int = 2000) -> String? {
        // Helper to extract a candidate title from translated messages
        func firstUserSnippet(from thread: CodexAcpTranslator.Thread) -> String? {
            var msgs = thread.events.compactMap { $0.message }.filter { $0.role == .user }
            msgs.sort { $0.ts < $1.ts }
            for m in msgs {
                let text = m.parts.compactMap { part -> String? in
                    if case let .text(t) = part { return t.text } else { return nil }
                }.joined(separator: " ")
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !trimmed.isEmpty else { continue }
                if ConversationSummarizer.isSystemPreface(trimmed) { continue }
                let words = trimmed.split(whereSeparator: { $0.isWhitespace })
                if words.isEmpty { continue }
                return words.prefix(5).joined(separator: " ")
            }
            return nil
        }

        // Head pass (fast path)
        if let fh = try? FileHandle(forReadingFrom: url) {
            defer { try? fh.close() }
            var lines: [String] = []
            lines.reserveCapacity(256)
            var read = 0
            var count = 0
            while let line = fh.readLine() {
                lines.append(line)
                read += line.utf8.count + 1
                count += 1
                if read >= maxBytes || count >= maxLines { break }
            }
            if !lines.isEmpty {
                let thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: url.path))
                if let title = firstUserSnippet(from: thread) { return title }
            }
        }

        // Tail fallback (smaller window to stay lightweight)
        do {
            let tailBytes = min(200_000, maxBytes)
            let tailLines = min(1500, maxLines)
            let lines = try tailJSONLLines(url: url, maxBytes: tailBytes, maxLines: tailLines)
            if !lines.isEmpty {
                let thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: url.path))
                if let title = firstUserSnippet(from: thread) { return title }
            }
        } catch {
            // Ignore tail read errors and fall through
        }
        return nil
    }

    /// Efficient JSONL tail reader to avoid loading entire files.
    private static func tailJSONLLines(url: URL, maxBytes: Int, maxLines: Int) throws -> [String] {
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

    static func isNewFormatJSONL(_ url: URL) -> Bool {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return false }
        defer { try? handle.close() }
        var linesChecked = 0
        while let line = handle.readLine(), linesChecked < 50 {
            linesChecked += 1
            if line.contains("\"type\"") { return true }
        }
        return false
    }

    static func scanForThreadID(_ url: URL) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }
        var i = 0
        while let line = handle.readLine(), i < 500 {
            i += 1
            if let tid = extractThreadID(fromJSONLine: line) { return tid }
        }
        return extractUUIDLike(fromFilename: url)
    }

    static func relativeId(for url: URL, base: URL) -> String {
        let u = url.deletingPathExtension().resolvingSymlinksInPath().standardizedFileURL.path
        let b = base.resolvingSymlinksInPath().standardizedFileURL.path
        if u.hasPrefix(b + "/") {
            let rel = String(u.dropFirst(b.count + 1))
            return rel
        }
        // fallback to filename stem
        return url.deletingPathExtension().lastPathComponent
    }

    static func extractThreadID(fromJSONLine line: String) -> String? {
        guard let data = line.data(using: .utf8), let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let type = json["type"] as? String ?? ""
        if type == "thread.started" {
            if let tid = json["thread_id"] as? String { return tid }
        }
        if type == "session_meta" {
            if let payload = json["payload"] as? [String: Any], let id = payload["id"] as? String { return id }
        }
        if let payload = json["payload"] as? [String: Any], let session = payload["session"] as? [String: Any], let id = session["id"] as? String {
            return id
        }
        if let msg = json["msg"] as? [String: Any], let tid = msg["thread_id"] as? String {
            return tid
        }
        return nil
    }

    static func extractUUIDLike(fromFilename url: URL) -> String? {
        let name = url.lastPathComponent
        // Matches hyphenated 32-char uuid-like strings
        let pattern = "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"
        if let range = name.range(of: pattern, options: .regularExpression) {
            return String(name[range])
        }
        return nil
    }

    static func fileMTime(_ url: URL) -> Int64 {
        let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
        if let m = attrs?[.modificationDate] as? Date { return Int64(m.timeIntervalSince1970 * 1000) }
        return Int64(Date().timeIntervalSince1970 * 1000)
    }

    static func tailLastMessageTs(_ url: URL, limitLines: Int = 200) -> Int64? {
        guard let data = try? Data(contentsOf: url), let s = String(data: data, encoding: .utf8) else { return nil }
        let lines = s.split(separator: "\n", omittingEmptySubsequences: true)
        for line in lines.suffix(limitLines).reversed() {
            if let ts = extractTimestamp(fromJSONLine: String(line)) { return ts }
        }
        return nil
    }

    static func extractTimestamp(fromJSONLine line: String) -> Int64? {
        guard let data = line.data(using: .utf8), let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        // Common fields we might see; fall back if absent
        if let ts = json["ts"] as? Double { return Int64(ts) }
        if let ts = json["ts"] as? Int { return Int64(ts) }
        if let payload = json["payload"] as? [String: Any] {
            if let ts = payload["ts"] as? Double { return Int64(ts) }
            if let ts = payload["ts"] as? Int { return Int64(ts) }
        }
        return nil
    }

    public static func scan(options: Options = .init()) -> [ThreadSummary] {
        let base = options.baseDir ?? defaultBaseDir()
        guard FileManager.default.fileExists(atPath: base.path) else { return [] }
        var files = listJSONLFiles(at: base)
        files.sort { fileMTime($0) > fileMTime($1) }
        if files.count > options.maxFiles { files = Array(files.prefix(options.maxFiles)) }
        var rows: [ThreadSummary] = []
        rows.reserveCapacity(files.count)
        for url in files {
            let id = scanForThreadID(url) ?? relativeId(for: url, base: base)
            let updated = fileMTime(url)
            let lastTs = tailLastMessageTs(url) ?? updated
            let row = ThreadSummary(
                id: id,
                title: nil,
                source: "codex",
                created_at: nil,
                updated_at: updated,
                last_message_ts: lastTs,
                message_count: nil
            )
            rows.append(row)
        }
        return rows
    }

    public static func scanTopK(options: Options = .init(), topK: Int = 10) -> [ThreadSummary] {
        let base = options.baseDir ?? defaultBaseDir()
        guard FileManager.default.fileExists(atPath: base.path) else { return [] }
        var files = listJSONLFiles(at: base)
        files.sort { fileMTime($0) > fileMTime($1) }
        if files.count > topK { files = Array(files.prefix(topK)) }
        var rows: [ThreadSummary] = []
        rows.reserveCapacity(files.count)
        for url in files {
            let id = scanForThreadID(url) ?? relativeId(for: url, base: base)
            let updated = fileMTime(url)
            rows.append(ThreadSummary(id: id, title: nil, source: "codex", created_at: nil, updated_at: updated, last_message_ts: nil, message_count: nil))
        }
        return rows
    }
}

fileprivate extension FileHandle {
    func readLine() -> String? {
        var data = Data()
        while true {
            do {
                let chunk = try self.read(upToCount: 1) ?? Data()
                if chunk.isEmpty { break }
                if chunk[0] == 0x0A { break }
                data.append(chunk)
            } catch { break }
        }
        if data.isEmpty { return nil }
        return String(data: data, encoding: .utf8)
    }
}
