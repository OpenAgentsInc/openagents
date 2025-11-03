import Foundation

// Local copy for app target (no SPM required)
struct LocalThreadSummary: Codable, Equatable, Hashable {
    let id: String
    let title: String?
    let source: String // "codex" | "claude_code"
    let created_at: Int64?
    let updated_at: Int64
    let last_message_ts: Int64?
    let message_count: Int?
    // Unique UI key to avoid collapsing duplicates
    var uniqueKey: String { source + "::" + id }
}

enum LocalCodexScanner {
    struct Options { var baseDir: URL?; var maxFiles: Int = 1000 }

    static func defaultBaseDir() -> URL {
        if let env = ProcessInfo.processInfo.environment["CODEXD_HISTORY_DIR"], !env.isEmpty {
            return URL(fileURLWithPath: env).standardizedFileURL
        }
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".codex/sessions", isDirectory: true)
    }

    static func isNewFormatJSONL(_ url: URL) -> Bool {
        guard let h = try? FileHandle(forReadingFrom: url) else { return false }
        defer { try? h.close() }
        var checked = 0
        while let line = h.readLine(), checked < 50 { checked += 1; if line.contains("\"type\"") { return true } }
        return false
    }

    static func listJSONLFiles(at base: URL) -> [URL] {
        var out: [URL] = []
        let fm = FileManager.default
        guard let en = fm.enumerator(at: base, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) else { return out }
        for case let url as URL in en {
            if url.lastPathComponent == "openagents" { en.skipDescendants(); continue }
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true { continue }
            if url.pathExtension.lowercased() == "jsonl" && isNewFormatJSONL(url) { out.append(url) }
        }
        return out
    }

    static func extractThreadID(fromJSONLine line: String) -> String? {
        guard let data = line.data(using: .utf8), let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let type = json["type"] as? String ?? ""
        if type == "thread.started" { if let tid = json["thread_id"] as? String { return tid } }
        if type == "session_meta" { if let payload = json["payload"] as? [String: Any], let id = payload["id"] as? String { return id } }
        if let payload = json["payload"] as? [String: Any], let session = payload["session"] as? [String: Any], let id = session["id"] as? String { return id }
        if let msg = json["msg"] as? [String: Any], let tid = msg["thread_id"] as? String { return tid }
        return nil
    }

    static func scanForThreadID(_ url: URL) -> String? {
        guard let h = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? h.close() }
        var i = 0
        while let line = h.readLine(), i < 500 { i += 1; if let tid = extractThreadID(fromJSONLine: line) { return tid } }
        return nil
    }

    static func relativeId(for url: URL, base: URL) -> String {
        let u = url.deletingPathExtension().resolvingSymlinksInPath().standardizedFileURL.path
        let b = base.resolvingSymlinksInPath().standardizedFileURL.path
        if u.hasPrefix(b + "/") { return String(u.dropFirst(b.count + 1)) }
        return url.deletingPathExtension().lastPathComponent
    }

    static func fileMTime(_ url: URL) -> Int64 {
        if let m = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date { return Int64(m.timeIntervalSince1970 * 1000) }
        return Int64(Date().timeIntervalSince1970 * 1000)
    }

    static func tailLastMessageTs(_ url: URL, limit: Int = 200) -> Int64? {
        guard let s = try? String(contentsOf: url) else { return nil }
        for line in s.split(separator: "\n", omittingEmptySubsequences: true).suffix(limit).reversed() {
            if let ts = extractTimestamp(fromJSONLine: String(line)) { return ts }
        }
        return nil
    }

    static func extractTimestamp(fromJSONLine line: String) -> Int64? {
        guard let data = line.data(using: .utf8), let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        if let ts = json["ts"] as? Double { return Int64(ts) }
        if let ts = json["ts"] as? Int { return Int64(ts) }
        if let payload = json["payload"] as? [String: Any] {
            if let ts = payload["ts"] as? Double { return Int64(ts) }
            if let ts = payload["ts"] as? Int { return Int64(ts) }
        }
        return nil
    }

    static func scan(options: Options = .init()) -> [LocalThreadSummary] {
        let base = options.baseDir ?? defaultBaseDir()
        guard FileManager.default.fileExists(atPath: base.path) else { return [] }
        var files = listJSONLFiles(at: base)
        files.sort { fileMTime($0) > fileMTime($1) }
        if files.count > options.maxFiles { files = Array(files.prefix(options.maxFiles)) }
        var rows: [LocalThreadSummary] = []
        rows.reserveCapacity(files.count)
        for url in files {
            let id = scanForThreadID(url) ?? relativeId(for: url, base: base)
            let updated = fileMTime(url)
            let lastTs = tailLastMessageTs(url) ?? updated
            rows.append(LocalThreadSummary(id: id, title: nil, source: "codex", created_at: nil, updated_at: updated, last_message_ts: lastTs, message_count: nil))
        }
        return rows
    }
}

enum LocalCodexDiscovery {
    struct Options { var preferEnvOnly: Bool = false }

    static func discoverBaseDirs(_ opts: Options = .init()) -> [URL] {
        var out: [URL] = []
        let fm = FileManager.default
        let env = ProcessInfo.processInfo.environment
        if let override = env["CODEXD_HISTORY_DIR"], !override.isEmpty {
            let u = URL(fileURLWithPath: override).standardizedFileURL
            if fm.fileExists(atPath: u.path) { out.append(u) }
        }
        if !opts.preferEnvOnly {
            let home = fm.homeDirectoryForCurrentUser
            let defaultBase = home.appendingPathComponent(".codex/sessions", isDirectory: true)
            if fm.fileExists(atPath: defaultBase.path) { out.append(defaultBase) }
            let codexRoot = home.appendingPathComponent(".codex", isDirectory: true)
            if let en = fm.enumerator(at: codexRoot, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
                var seen = Set(out.map { $0.path })
                for case let p as URL in en {
                    if (try? p.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                        if LocalCodexScanner.listJSONLFiles(at: p).count > 0 {
                            if !seen.contains(p.path) { out.append(p); seen.insert(p.path) }
                        }
                    }
                }
            }
        }
        if let extra = env["CODEX_EXTRA_DIRS"], !extra.isEmpty {
            for seg in extra.split(separator: ":") {
                let u = URL(fileURLWithPath: String(seg)).standardizedFileURL
                if fm.fileExists(atPath: u.path) { out.append(u) }
            }
        }
        // dedupe
        var uniq: [URL] = []
        var seen: Set<String> = []
        for u in out { if !seen.contains(u.path) { uniq.append(u); seen.insert(u.path) } }
        return uniq
    }

    static func loadAllSummaries(maxFilesPerBase: Int = 1000, maxResults: Int = 500) -> [LocalThreadSummary] {
        var rows: [LocalThreadSummary] = []
        for base in discoverBaseDirs() {
            let r = LocalCodexScanner.scan(options: .init(baseDir: base, maxFiles: maxFilesPerBase))
            rows.append(contentsOf: r)
        }
        // sort newest first
        rows.sort { $0.updated_at > $1.updated_at }
        if rows.count > maxResults { rows = Array(rows.prefix(maxResults)) }
        return rows
    }
}

fileprivate extension FileHandle {
    func readLine() -> String? {
        var data = Data(); while true {
            let chunk = try? self.read(upToCount: 1) ?? Data(); if let c = chunk, c.count > 0 {
                if c[0] == 0x0A { break }; data.append(c)
            } else { break }
        }
        return data.isEmpty ? nil : String(data: data, encoding: .utf8)
    }
}

