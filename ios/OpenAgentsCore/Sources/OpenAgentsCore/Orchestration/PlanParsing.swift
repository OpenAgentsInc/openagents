import Foundation

/// Parsing utilities for converting FM responses into AgentOp sequences.
enum PlanParsing {
    static func parseOperationsFromResponse(_ response: String, workspaceRoot: String) -> [AgentOp] {
        let content = extractContent(from: response) ?? response
        var ops: [AgentOp] = []
        let lines = content.components(separatedBy: "\n")
        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty { continue }
            if trimmed.contains("sessionList") || (trimmed.contains("session") && trimmed.contains("list")) {
                if let op = parseSessionList(from: trimmed) { ops.append(op) }
            } else if trimmed.contains("sessionSearch") || (trimmed.contains("session") && trimmed.contains("search")) {
                if let op = parseSessionSearch(from: trimmed) { ops.append(op) }
            } else if trimmed.contains("sessionRead") || (trimmed.contains("session") && trimmed.contains("read")) {
                if let op = parseSessionRead(from: trimmed) { ops.append(op) }
            } else if trimmed.contains("sessionAnalyze") || (trimmed.contains("session") && trimmed.contains("analyze")) {
                if let op = parseSessionAnalyze(from: trimmed) { ops.append(op) }
            } else if trimmed.contains("readSpan") || trimmed.contains("Read") {
                if let op = parseReadSpan(from: trimmed, workspaceRoot: workspaceRoot) { ops.append(op) }
            } else if trimmed.contains("grep") || trimmed.contains("Search") {
                if let op = parseGrep(from: trimmed, workspaceRoot: workspaceRoot) { ops.append(op) }
            } else if trimmed.contains("listDir") || trimmed.contains("List") {
                if let op = parseListDir(from: trimmed, workspaceRoot: workspaceRoot) { ops.append(op) }
            }
        }
        return Array(ops.prefix(5))
    }

    // MARK: - Extraction helpers
    static func extractContent(from description: String) -> String? {
        if description.localizedCaseInsensitiveContains("Safety guardrails") { return nil }
        if let m = match(description, pattern: #"content:\s*'([^']*)'"#) { return m }
        if let m = match(description, pattern: #"content:\s*\"([^\"]*)\""#) { return m }
        if let m = match(description, pattern: #"rawContent:\s*'([^']*)'"#) { return m }
        if let m = match(description, pattern: #"rawContent:\s*\"([^\"]*)\""#) { return m }
        return nil
    }

    static func match(_ s: String, pattern: String) -> String? {
        do {
            let re = try NSRegularExpression(pattern: pattern, options: [])
            let range = NSRange(s.startIndex..<s.endIndex, in: s)
            if let m = re.firstMatch(in: s, options: [], range: range), m.numberOfRanges >= 2,
               let r = Range(m.range(at: 1), in: s) {
                return String(s[r])
            }
        } catch {}
        return nil
    }

    // MARK: - Op parsers
    static func parseReadSpan(from line: String, workspaceRoot: String) -> AgentOp? {
        if let path = extractPath(from: line), let (start, end) = extractLineRange(from: line) {
            let norm = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: path)
            return AgentOp(kind: .readSpan(ReadSpanParams(path: norm, startLine: start, endLine: end)))
        }
        return nil
    }

    static func parseGrep(from line: String, workspaceRoot: String) -> AgentOp? {
        if let pattern = extractQuotedString(from: line) {
            var pathPrefix = extractPathAfterIn(from: line)
            if let p = pathPrefix { pathPrefix = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: p) }
            if pathPrefix == "workspace" || pathPrefix == "." || pathPrefix == "/" || pathPrefix == "/workspace" { pathPrefix = nil }
            return AgentOp(kind: .grep(GrepParams(pattern: pattern, pathPrefix: pathPrefix)))
        }
        return nil
    }

    static func parseListDir(from line: String, workspaceRoot: String) -> AgentOp? {
        if let path = extractPath(from: line) {
            let normalized = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: path)
            return AgentOp(kind: .listDir(ListDirParams(path: normalized, depth: 0)))
        }
        return nil
    }

    static func extractPath(from line: String) -> String? {
        let pattern = #"([a-zA-Z0-9_\-./]+\.[a-z]+|[a-zA-Z0-9_\-./]+/)"#
        return match(line, pattern: pattern)
    }
    static func extractLineRange(from line: String) -> (Int, Int)? {
        if let m = match(line, pattern: #"(\d+)-(\d+)"#) {
            let parts = m.components(separatedBy: "-")
            if parts.count == 2, let start = Int(parts[0]), let end = Int(parts[1]) { return (start, end) }
        }
        return nil
    }
    static func extractQuotedString(from line: String) -> String? {
        if let m = match(line, pattern: #"['"]([^'"]+)['"]"#) { return m }
        return nil
    }
    static func extractPathAfterIn(from line: String) -> String? {
        if let m = match(line, pattern: #"in\s+([a-zA-Z0-9_\-./]+)"#) { return m }
        return nil
    }

    // Session ops
    static func parseSessionList(from line: String) -> AgentOp? {
        let provider = extractProvider(from: line)
        let topK = extractNumber(from: line)
        return AgentOp(kind: .sessionList(SessionListParams(provider: provider, topK: topK)))
    }
    static func parseSessionSearch(from line: String) -> AgentOp? {
        if let pattern = extractQuotedString(from: line) {
            let provider = extractProvider(from: line)
            return AgentOp(kind: .sessionSearch(SessionSearchParams(pattern: pattern, provider: provider)))
        }
        return nil
    }
    static func parseSessionRead(from line: String) -> AgentOp? {
        if let sessionId = extractSessionId(from: line) {
            let provider = extractProvider(from: line) ?? "claude-code"
            return AgentOp(kind: .sessionRead(SessionReadParams(sessionId: sessionId, provider: provider)))
        }
        return nil
    }
    static func parseSessionAnalyze(from line: String) -> AgentOp? {
        let provider = extractProvider(from: line)
        return AgentOp(kind: .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: provider)))
    }
    static func extractProvider(from line: String) -> String? {
        if line.localizedCaseInsensitiveContains("claude-code") || line.localizedCaseInsensitiveContains("claude code") { return "claude-code" }
        if line.localizedCaseInsensitiveContains("codex") { return "codex" }
        return nil
    }
    static func extractNumber(from line: String) -> Int? {
        if let m = match(line, pattern: #"top\s+(\d+)"#) { return Int(m) }
        if let m = match(line, pattern: #"(\d+)\s+sessions?"#) { return Int(m) }
        return nil
    }
    static func extractSessionId(from line: String) -> String? {
        if let m = match(line, pattern: #"session\s+([a-zA-Z0-9\-_]+)"#) { return m }
        return nil
    }
}

