import Foundation

public enum ClaudeScanner {
    public struct Options {
        public var baseDir: URL?
        public var maxFiles: Int
        public init(baseDir: URL? = nil, maxFiles: Int = 200) {
            self.baseDir = baseDir
            self.maxFiles = maxFiles
        }
    }

    public static func defaultBaseDir() -> URL {
        if let env = ProcessInfo.processInfo.environment["CLAUDE_PROJECTS_DIR"], !env.isEmpty {
            return URL(fileURLWithPath: env).standardizedFileURL
        }
        #if os(macOS)
        let home = FileManager.default.homeDirectoryForCurrentUser
        let candidates: [URL] = [
            home.appendingPathComponent(".claude/projects", isDirectory: true),
            home.appendingPathComponent(".claude/local/claude/projects", isDirectory: true),
            home.appendingPathComponent(".claude/local/projects", isDirectory: true),
        ]
        for c in candidates where FileManager.default.fileExists(atPath: c.path) {
            return c
        }
        // Fallback: ~/.claude/**/projects (first found)
        let root = home.appendingPathComponent(".claude", isDirectory: true)
        if let enumerator = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
            for case let url as URL in enumerator {
                if url.lastPathComponent == "projects" { return url }
            }
        }
        return home.appendingPathComponent(".claude/projects", isDirectory: true)
        #else
        // iOS and other platforms: no direct access to desktop FS
        return URL(fileURLWithPath: "/nonexistent")
        #endif
    }

    public static func listJSONLFiles(at base: URL) -> [URL] {
        var out: [URL] = []
        guard let en = FileManager.default.enumerator(at: base, includingPropertiesForKeys: [.isDirectoryKey, .contentModificationDateKey], options: [.skipsHiddenFiles]) else { return out }
        for case let url as URL in en {
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true { continue }
            if url.pathExtension.lowercased() == "jsonl" { out.append(url) }
        }
        return out
    }

    static func sessionID(from url: URL, base: URL) -> String? {
        // Prefer a stable, unique id relative to the base projects dir
        // Example: ~/.claude/projects/myproj/sessions/2025-11-03/transcript.jsonl
        // → id: "myproj/sessions/2025-11-03/transcript"
        let stem = url.deletingPathExtension().lastPathComponent
        let parent = url.deletingLastPathComponent().lastPathComponent
        let combined = parent.isEmpty ? stem : parent + "/" + stem
        return combined.isEmpty ? nil : combined
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
        if let ts = json["ts"] as? Double { return Int64(ts) }
        if let ts = json["ts"] as? Int { return Int64(ts) }
        if let message = json["message"] as? [String: Any], let ts = message["ts"] as? Double { return Int64(ts) }
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
            guard let id = sessionID(from: url, base: base) else { continue }
            let updated = fileMTime(url)
            let lastTs = tailLastMessageTs(url) ?? updated
            let row = ThreadSummary(
                id: id,
                title: nil,
                source: "claude_code",
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
            guard let id = sessionID(from: url, base: base) else { continue }
            let updated = fileMTime(url)
            rows.append(ThreadSummary(id: id, title: nil, source: "claude_code", created_at: nil, updated_at: updated, last_message_ts: nil, message_count: nil))
        }
        return rows
    }
}
