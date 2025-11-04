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
        #if os(macOS)
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".codex/sessions", isDirectory: true)
        #else
        return URL(fileURLWithPath: "/nonexistent")
        #endif
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

    /// Fast top-N file finder tailored for Codex sessions shape: base/YYYY/MM/DD/rollout-*.jsonl
    /// Falls back to a shallow enumerator if the shape isn't present.
    static func listRecentTopN(at base: URL, topK: Int) -> [URL] {
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

    static func scanFast(options: Options = .init()) -> [LocalThreadSummary] {
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
            rows.append(LocalThreadSummary(id: id, title: nil, source: "codex", created_at: nil, updated_at: updated, last_message_ts: nil, message_count: nil))
        }
        return rows
    }
}

enum LocalCodexDiscovery {
    struct Options { var preferEnvOnly: Bool = false }
    private static let overrideKey = "codex_base_override"
    private static let overrideBookmarkKey = "codex_base_override_bookmark"

    static func setUserOverride(_ url: URL?) {
        let d = UserDefaults.standard
        if let u = url {
            d.set(u.path, forKey: overrideKey)
            // Persist security-scoped bookmark for sandbox access (macOS sandboxed builds)
            #if os(macOS)
            if let data = try? u.bookmarkData(options: .withSecurityScope, includingResourceValuesForKeys: nil, relativeTo: nil) {
                d.set(data, forKey: overrideBookmarkKey)
            }
            #endif
        } else {
            d.removeObject(forKey: overrideKey)
            #if os(macOS)
            d.removeObject(forKey: overrideBookmarkKey)
            #endif
        }
    }

    static func userOverride() -> URL? {
        let d = UserDefaults.standard
        #if os(macOS)
        if let data = d.data(forKey: overrideBookmarkKey) {
            var isStale = false
            if let url = try? URL(resolvingBookmarkData: data, options: .withSecurityScope, relativeTo: nil, bookmarkDataIsStale: &isStale) {
                _ = url.startAccessingSecurityScopedResource()
                return url
            }
        }
        #endif
        if let p = d.string(forKey: overrideKey), !p.isEmpty {
            return URL(fileURLWithPath: p).standardizedFileURL
        }
        return nil
    }

    static func discoverBaseDirs(_ opts: Options = .init()) -> [URL] {
        var out: [URL] = []
        let fm = FileManager.default
        let env = ProcessInfo.processInfo.environment
        if let user = userOverride() {
            if fm.fileExists(atPath: user.path) { out.append(user) }
        }
        if let override = env["CODEXD_HISTORY_DIR"], !override.isEmpty {
            let u = URL(fileURLWithPath: override).standardizedFileURL
            if fm.fileExists(atPath: u.path) { out.append(u) }
        }
        if !opts.preferEnvOnly {
            // Derive three home bases only; avoid recursive enumeration for speed
            #if os(macOS)
            let home = fm.homeDirectoryForCurrentUser
            let home2 = URL(fileURLWithPath: NSHomeDirectory())
            let realHome = URL(fileURLWithPath: "/Users/\(NSUserName())", isDirectory: true)
            let defaultBase = home.appendingPathComponent(".codex/sessions", isDirectory: true)
            let defaultBase2 = home2.appendingPathComponent(".codex/sessions", isDirectory: true)
            let defaultBase3 = realHome.appendingPathComponent(".codex/sessions", isDirectory: true)
            out.append(defaultBase)
            if !out.contains(where: { $0.path == defaultBase2.path }) { out.append(defaultBase2) }
            if !out.contains(where: { $0.path == defaultBase3.path }) { out.append(defaultBase3) }
            #endif
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

    static func loadAllSummaries(topK: Int = 10) -> [LocalThreadSummary] {
        let bases = discoverBaseDirs()
        // Gather recent candidates per base without content reads
        var candidates: [URL] = []
        for b in bases {
            candidates.append(contentsOf: LocalCodexScanner.listRecentTopN(at: b, topK: topK))
        }
        // Merge and pick global topK by mtime
        candidates.sort { LocalCodexScanner.fileMTime($0) > LocalCodexScanner.fileMTime($1) }
        if candidates.count > topK { candidates = Array(candidates.prefix(topK)) }
        let rows: [LocalThreadSummary] = candidates.map { url in
            let base = bases.first { url.path.hasPrefix($0.path) } ?? url.deletingLastPathComponent()
            let id = LocalCodexScanner.scanForThreadID(url) ?? LocalCodexScanner.relativeId(for: url, base: base)
            let updated = LocalCodexScanner.fileMTime(url)
            return LocalThreadSummary(id: id, title: nil, source: "codex", created_at: nil, updated_at: updated, last_message_ts: nil, message_count: nil)
        }
        print("[History] Codex bases=\(bases.map{ $0.path }) items=\(rows.count)")
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
