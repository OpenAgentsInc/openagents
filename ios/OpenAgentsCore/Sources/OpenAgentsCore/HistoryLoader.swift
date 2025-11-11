import Foundation

public enum HistoryLoader {
    public struct Options {
        public var includeCodex: Bool
        public var includeClaude: Bool
        public var codexBase: URL?
        public var claudeBase: URL?
        public var maxFilesPerProvider: Int
        public var maxResults: Int
        public init(includeCodex: Bool = true,
                    includeClaude: Bool = true,
                    codexBase: URL? = nil,
                    claudeBase: URL? = nil,
                    maxFilesPerProvider: Int = 200,
                    maxResults: Int = 100) {
            self.includeCodex = includeCodex
            self.includeClaude = includeClaude
            self.codexBase = codexBase
            self.claudeBase = claudeBase
            self.maxFilesPerProvider = maxFilesPerProvider
            self.maxResults = maxResults
        }
    }

    public static func loadSummaries(_ opts: Options = .init()) -> [ThreadSummary] {
        let codexSummaries: [ThreadSummary] = opts.includeCodex
            ? CodexScanner.scan(options: .init(baseDir: opts.codexBase, maxFiles: opts.maxFilesPerProvider))
            : []
        let claudeSummaries: [ThreadSummary] = opts.includeClaude
            ? ClaudeScanner.scan(options: .init(baseDir: opts.claudeBase, maxFiles: opts.maxFilesPerProvider))
            : []
        return mergeSummaries(
            opts: opts,
            codexSummaries: codexSummaries,
            claudeSummaries: claudeSummaries
        )
    }

    /// Internal helper so tests can feed synthetic summaries without touching the filesystem.
    static func mergeSummaries(
        opts: Options,
        codexSummaries: [ThreadSummary],
        claudeSummaries: [ThreadSummary]
    ) -> [ThreadSummary] {
        var rows: [ThreadSummary] = []
        if opts.includeCodex {
            rows.append(contentsOf: codexSummaries)
        }
        if opts.includeClaude {
            rows.append(contentsOf: claudeSummaries)
        }

        // Deduplicate by (source,id), keeping the most recently updated
        var uniq: [String: ThreadSummary] = [:]
        for r in rows {
            let key = "\(r.source)::\(r.id)"
            if let prev = uniq[key] {
                if r.updated_at > prev.updated_at { uniq[key] = r }
            } else {
                uniq[key] = r
            }
        }

        var deduped = Array(uniq.values)
        deduped.sort { (a, b) in
            let at = a.updated_at
            let bt = b.updated_at
            if at == bt { return (a.last_message_ts ?? at) > (b.last_message_ts ?? bt) }
            return at > bt
        }
        if deduped.count > opts.maxResults {
            deduped = Array(deduped.prefix(opts.maxResults))
        }
        return deduped
    }
}
