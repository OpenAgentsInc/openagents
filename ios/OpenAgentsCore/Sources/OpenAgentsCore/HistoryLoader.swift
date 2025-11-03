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
        var rows: [ThreadSummary] = []
        if opts.includeCodex {
            let c = CodexScanner.scan(options: .init(baseDir: opts.codexBase, maxFiles: opts.maxFilesPerProvider))
            rows.append(contentsOf: c)
        }
        if opts.includeClaude {
            let c = ClaudeScanner.scan(options: .init(baseDir: opts.claudeBase, maxFiles: opts.maxFilesPerProvider))
            rows.append(contentsOf: c)
        }
        rows.sort { (a, b) in
            let at = a.updated_at
            let bt = b.updated_at
            if at == bt { return (a.last_message_ts ?? at) > (b.last_message_ts ?? bt) }
            return at > bt
        }
        if rows.count > opts.maxResults {
            return Array(rows.prefix(opts.maxResults))
        }
        return rows
    }
}
