import Foundation

public enum ClaudeCodeScanner {
    public struct Options {
        public var baseDir: URL?
        public var maxFiles: Int
        public init(baseDir: URL? = nil, maxFiles: Int = 200) {
            self.baseDir = baseDir
            self.maxFiles = maxFiles
        }
    }

    public static func defaultBaseDir() -> URL {
        #if os(macOS)
        let home = FileManager.default.homeDirectoryForCurrentUser
        return home.appendingPathComponent(".claude/projects", isDirectory: true)
        #else
        // iOS: no direct desktop FS. Return placeholder.
        return URL(fileURLWithPath: "/nonexistent")
        #endif
    }

    /// List all JSONL session files in Claude Code projects directory
    public static func listJSONLFiles(at base: URL) -> [URL] {
        var out: [URL] = []
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(
            at: base,
            includingPropertiesForKeys: [.isDirectoryKey, .contentModificationDateKey],
            options: [.skipsHiddenFiles]
        ) else { return out }

        for case let url as URL in enumerator {
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                continue
            }
            if url.pathExtension.lowercased() == "jsonl" {
                // Exclude backup files
                if url.lastPathComponent.contains(".backup") {
                    continue
                }
                out.append(url)
            }
        }
        return out
    }

    /// Get most recent N session files by modification time
    public static func listRecentTopN(at base: URL, topK: Int) -> [URL] {
        var all = listJSONLFiles(at: base)
        all.sort { fileMTime($0) > fileMTime($1) }
        if all.count > topK {
            all = Array(all.prefix(topK))
        }
        return all
    }

    public static func makeSummary(for url: URL, base: URL) -> ThreadSummary {
        let id = scanForSessionID(url) ?? relativeId(for: url, base: base)
        let updated = fileMTime(url)
        let lastTs = tailLastMessageTs(url) ?? updated
        let title = quickTitle(for: url)

        return ThreadSummary(
            id: id,
            title: title,
            source: "claude-code",
            created_at: nil,
            updated_at: updated,
            last_message_ts: lastTs,
            message_count: nil
        )
    }

    /// Scan first few lines for sessionId
    public static func scanForSessionID(_ url: URL) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }

        // Read first 50KB
        guard let data = try? handle.read(upToCount: 50_000),
              let text = String(data: data, encoding: .utf8) else { return nil }

        // Parse first line
        let lines = text.split(separator: "\n", maxSplits: 5)
        for line in lines {
            if let data = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let sessionId = obj["sessionId"] as? String {
                return sessionId
            }
        }
        return nil
    }

    /// Extract project path from URL or file contents
    private static func extractProjectPath(_ url: URL, base: URL) -> String? {
        // Try to read from first line
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }

        guard let data = try? handle.read(upToCount: 10_000),
              let text = String(data: data, encoding: .utf8) else { return nil }

        let lines = text.split(separator: "\n", maxSplits: 3)
        for line in lines {
            if let data = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                if let cwd = obj["cwd"] as? String {
                    return cwd
                }
                if let message = obj["message"] as? [String: Any],
                   let content = message["content"] as? String {
                    // Extract from first user message if available
                    return nil
                }
            }
        }
        return nil
    }

    /// Derive title from first user message
    public static func quickTitle(for url: URL, maxBytes: Int = 300_000) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }

        guard let data = try? handle.read(upToCount: maxBytes),
              let text = String(data: data, encoding: .utf8) else { return nil }

        // Look for first user message
        for line in text.split(separator: "\n") {
            if let data = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               obj["type"] as? String == "user",
               let message = obj["message"] as? [String: Any],
               let content = message["content"] as? String {
                // Take first ~60 chars
                let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
                if trimmed.isEmpty { continue }
                let words = trimmed.split(separator: " ", maxSplits: 10)
                let preview = words.prefix(8).joined(separator: " ")
                return preview.count > 60 ? String(preview.prefix(60)) + "..." : preview
            }
        }
        return nil
    }

    /// Get timestamp of last message (milliseconds since epoch)
    private static func tailLastMessageTs(_ url: URL) -> Int64? {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }

        // Seek to last 50KB
        let fileSize = (try? handle.seekToEnd()) ?? 0
        if fileSize > 50_000 {
            try? handle.seek(toOffset: fileSize - 50_000)
        } else {
            try? handle.seek(toOffset: 0)
        }

        guard let data = try? handle.readToEnd(),
              let text = String(data: data, encoding: .utf8) else { return nil }

        let lines = text.split(separator: "\n")
        for line in lines.reversed() {
            if let data = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let timestamp = obj["timestamp"] as? Double {
                return Int64(timestamp) // Claude Code timestamps are already in ms
            }
        }
        return nil
    }

    /// Generate relative ID from file path
    public static func relativeId(for url: URL, base: URL) -> String {
        let rel = url.path.replacingOccurrences(of: base.path, with: "")
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        if rel.hasSuffix(".jsonl") {
            return String(rel.dropLast(6))
        }
        return rel
    }

    /// Get file modification time as Int64 (milliseconds since epoch)
    private static func fileMTime(_ url: URL) -> Int64 {
        let values = try? url.resourceValues(forKeys: [.contentModificationDateKey])
        let timeInterval = values?.contentModificationDate?.timeIntervalSince1970 ?? 0
        return Int64(timeInterval * 1000) // Convert seconds to milliseconds
    }
}
