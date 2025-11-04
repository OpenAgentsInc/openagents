import Foundation

public enum CodexDiscovery {
    public struct Options {
        public var preferEnvOnly: Bool
        public init(preferEnvOnly: Bool = false) { self.preferEnvOnly = preferEnvOnly }
    }
    /// Return a list of plausible Codex sessions base directories.
    /// Order of precedence:
    /// - `CODEXD_HISTORY_DIR` env (if exists)
    /// - `~/.codex/sessions` (if exists)
    /// - any subdirectory under `~/.codex` that contains new-format `*.jsonl`
    /// - extra dirs from `CODEX_EXTRA_DIRS` (":" separated), if they exist
    public static func discoverBaseDirs(options: Options = .init()) -> [URL] {
        #if os(macOS)
        var out: [URL] = []
        let fm = FileManager.default
        let env = ProcessInfo.processInfo.environment
        if let override = env["CODEXD_HISTORY_DIR"], !override.isEmpty {
            let u = URL(fileURLWithPath: override).standardizedFileURL
            if fm.fileExists(atPath: u.path) { out.append(u) }
        }
        if options.preferEnvOnly {
            // Include extras but skip default scanning for tests
            if let extra = env["CODEX_EXTRA_DIRS"], !extra.isEmpty {
                for seg in extra.split(separator: ":") {
                    let u = URL(fileURLWithPath: String(seg)).standardizedFileURL
                    if fm.fileExists(atPath: u.path) { out.append(u) }
                }
            }
            return dedupe(out)
        }
        let home = fm.homeDirectoryForCurrentUser
        let defaultBase = home.appendingPathComponent(".codex/sessions", isDirectory: true)
        if fm.fileExists(atPath: defaultBase.path) { out.append(defaultBase) }
        // Scan ~/.codex for any directory that contains new-format jsonl
        let codexRoot = home.appendingPathComponent(".codex", isDirectory: true)
        if let en = fm.enumerator(at: codexRoot, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
            var seen: Set<String> = Set(out.map { $0.path })
            for case let p as URL in en {
                if (try? p.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                    // quick probe: any new-format jsonl directly inside?
                    if containsNewFormatJSONL(in: p) {
                        if !seen.contains(p.path) {
                            out.append(p)
                            seen.insert(p.path)
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
        return dedupe(out)
        #else
        // iOS and other platforms: avoid using homeDirectoryForCurrentUser.
        // Respect environment overrides if present (useful for simulators/tests),
        // otherwise return an empty list to prevent desktop-specific scanning.
        var out: [URL] = []
        let fm = FileManager.default
        let env = ProcessInfo.processInfo.environment
        if let override = env["CODEXD_HISTORY_DIR"], !override.isEmpty {
            let u = URL(fileURLWithPath: override).standardizedFileURL
            if fm.fileExists(atPath: u.path) { out.append(u) }
        }
        if let extra = env["CODEX_EXTRA_DIRS"], !extra.isEmpty {
            for seg in extra.split(separator: ":") {
                let u = URL(fileURLWithPath: String(seg)).standardizedFileURL
                if fm.fileExists(atPath: u.path) { out.append(u) }
            }
        }
        return dedupe(out)
        #endif
    }

    private static func dedupe(_ urls: [URL]) -> [URL] {
        var uniq: [URL] = []
        var seen: Set<String> = []
        for u in urls {
            if !seen.contains(u.path) { uniq.append(u); seen.insert(u.path) }
        }
        return uniq
    }

    public static func containsNewFormatJSONL(in dir: URL) -> Bool {
        let fm = FileManager.default
        if let items = try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil) {
            for url in items where url.pathExtension.lowercased() == "jsonl" {
                if CodexScanner.isNewFormatJSONL(url) { return true }
            }
        }
        return false
    }

    /// Recursively list all new-format jsonl files across discovered bases.
    public static func listAllJSONLFiles(maxFilesPerBase: Int = 1000, options: Options = .init()) -> [URL] {
        var out: [URL] = []
        for base in discoverBaseDirs(options: options) {
            var files = CodexScanner.listJSONLFiles(at: base)
            // Sort newest first per base to cap effectively
            files.sort { CodexScanner.fileMTime($0) > CodexScanner.fileMTime($1) }
            if files.count > maxFilesPerBase { files = Array(files.prefix(maxFilesPerBase)) }
            out.append(contentsOf: files)
        }
        return out
    }

    /// Load all summaries across discovered bases.
    public static func loadAllSummaries(maxFilesPerBase: Int = 1000, maxResults: Int = 1000, options: Options = .init()) -> [ThreadSummary] {
        let bases = discoverBaseDirs(options: options)
        var rows: [ThreadSummary] = []
        for base in bases {
            let r = CodexScanner.scan(options: .init(baseDir: base, maxFiles: maxFilesPerBase))
            rows.append(contentsOf: r)
        }
        // de-dupe by id (Codex-only here) and sort
        var uniq: [String: ThreadSummary] = [:]
        for r in rows {
            if let prev = uniq[r.id] {
                if r.updated_at > prev.updated_at { uniq[r.id] = r }
            } else {
                uniq[r.id] = r
            }
        }
        var merged = Array(uniq.values)
        merged.sort { $0.updated_at > $1.updated_at }
        if merged.count > maxResults { merged = Array(merged.prefix(maxResults)) }
        return merged
    }
}

