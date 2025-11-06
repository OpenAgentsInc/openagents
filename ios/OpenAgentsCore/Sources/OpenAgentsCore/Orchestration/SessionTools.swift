// SessionTools.swift — Session history exploration tools for Phase 2.5
// Enables deep traversal of Claude Code and Codex conversation history

import Foundation

/// Tool for listing and discovering conversation sessions
public struct SessionListTool: Sendable {
    public init() {}

    public func list(
        provider: String? = nil,
        topK: Int? = nil,
        since: Int64? = nil
    ) async throws -> SessionListResult {
        let k = min(topK ?? 20, 200) // Bound to 200 max
        var allSessions: [SessionMetadata] = []

        // Collect from Claude Code if requested
        if provider == nil || provider == "claude-code" {
            let claudeBase = ClaudeCodeScanner.defaultBaseDir()
            if FileManager.default.fileExists(atPath: claudeBase.path) {
                let files = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: k * 2)
                for url in files {
                    let summary = ClaudeCodeScanner.makeSummary(for: url, base: claudeBase)

                    // Filter by timestamp if requested
                    if let since = since, summary.updated_at < since {
                        continue
                    }

                    allSessions.append(SessionMetadata(
                        id: summary.id,
                        title: summary.title,
                        provider: "claude-code",
                        updated_at: summary.updated_at,
                        file_path: url.path
                    ))
                }
            }
        }

        // Collect from Codex if requested
        if provider == nil || provider == "codex" {
            let codexBase = CodexScanner.defaultBaseDir()
            if FileManager.default.fileExists(atPath: codexBase.path) {
                let files = CodexScanner.listRecentTopN(at: codexBase, topK: k * 2)
                for url in files {
                    let summary = CodexScanner.makeSummary(for: url, base: codexBase)

                    // Filter by timestamp
                    if let since = since, summary.updated_at < since {
                        continue
                    }

                    allSessions.append(SessionMetadata(
                        id: summary.id,
                        title: summary.title,
                        provider: "codex",
                        updated_at: summary.updated_at,
                        file_path: url.path
                    ))
                }
            }
        }

        // Sort by timestamp descending
        allSessions.sort { $0.updated_at > $1.updated_at }

        let totalCount = allSessions.count
        let truncated = allSessions.count > k
        if truncated {
            allSessions = Array(allSessions.prefix(k))
        }

        return SessionListResult(
            sessions: allSessions,
            truncated: truncated,
            total_count: totalCount
        )
    }
}

/// Tool for searching across session history
public struct SessionSearchTool: Sendable {
    public init() {}

    public func search(
        pattern: String,
        provider: String? = nil,
        sessionIds: [String]? = nil,
        maxResults: Int? = nil,
        contextLines: Int? = nil
    ) async throws -> SessionSearchResult {
        let maxRes = min(maxResults ?? 100, 200)
        let context = contextLines ?? 2
        var matches: [SessionSearchMatch] = []
        var totalMatches = 0

        // Compile regex
        guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else {
            throw ToolExecutionError.invalidParameters("Invalid regex pattern: \(pattern)")
        }

        // Get sessions to search
        var sessions: [SessionMetadata] = []
        if let sessionIds = sessionIds, !sessionIds.isEmpty {
            // Search specific sessions
            let listTool = SessionListTool()
            let allSessions = try await listTool.list(provider: provider, topK: 200)
            sessions = allSessions.sessions.filter { sessionIds.contains($0.id) }
        } else {
            // Search all recent sessions
            let listTool = SessionListTool()
            sessions = try await listTool.list(provider: provider, topK: 100).sessions
        }

        // Search each session file
        for session in sessions {
            guard matches.count < maxRes else { break }

            let url = URL(fileURLWithPath: session.file_path)
            guard FileManager.default.fileExists(atPath: url.path) else { continue }

            // Read file lines
            guard let content = try? String(contentsOf: url, encoding: .utf8) else { continue }
            let lines = content.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)

            // Search each line
            for (lineIdx, line) in lines.enumerated() {
                let nsLine = line as NSString
                let range = NSRange(location: 0, length: nsLine.length)

                if regex.firstMatch(in: line, range: range) != nil {
                    totalMatches += 1
                    guard matches.count < maxRes else { break }

                    // Extract context
                    let beforeStart = max(0, lineIdx - context)
                    let afterEnd = min(lines.count, lineIdx + context + 1)
                    let contextBefore = lineIdx > beforeStart ? Array(lines[beforeStart..<lineIdx]) : nil
                    let contextAfter = afterEnd > lineIdx + 1 ? Array(lines[(lineIdx+1)..<afterEnd]) : nil

                    matches.append(SessionSearchMatch(
                        sessionId: session.id,
                        provider: session.provider,
                        lineNumber: lineIdx + 1,
                        line: line,
                        contextBefore: contextBefore,
                        contextAfter: contextAfter
                    ))
                }
            }
        }

        return SessionSearchResult(
            pattern: pattern,
            matches: matches,
            truncated: totalMatches > maxRes,
            totalMatches: totalMatches
        )
    }
}

/// Tool for reading session content
public struct SessionReadTool: Sendable {
    public init() {}

    public func read(
        sessionId: String,
        provider: String,
        startLine: Int? = nil,
        endLine: Int? = nil,
        maxEvents: Int? = nil
    ) async throws -> SessionReadResult {
        let maxEvts = min(maxEvents ?? 100, 200)

        // Find session file
        let listTool = SessionListTool()
        let sessions = try await listTool.list(provider: provider, topK: 200)
        guard let session = sessions.sessions.first(where: { $0.id == sessionId }) else {
            throw ToolExecutionError.fileNotFound("Session not found: \(sessionId)")
        }

        let url = URL(fileURLWithPath: session.file_path)
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw ToolExecutionError.fileNotFound("Session file not found: \(url.path)")
        }

        // Read file content
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            throw ToolExecutionError.executionFailed("Failed to read session file")
        }

        let lines = content.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)

        // Apply line range if specified
        let start = (startLine ?? 1) - 1
        let end = min(endLine ?? lines.count, lines.count)
        guard start >= 0 && start < lines.count else {
            throw ToolExecutionError.invalidParameters("Invalid line range")
        }

        let selectedLines = Array(lines[start..<end])

        // Parse events
        var events: [SessionEvent] = []
        var fileReferences: Set<String> = []

        for (idx, line) in selectedLines.enumerated() {
            guard events.count < maxEvts else { break }
            guard let data = line.data(using: .utf8),
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                continue
            }

            // Extract event type and content
            let type = json["type"] as? String ?? "unknown"
            var content = ""
            var timestamp: Int64? = nil

            // Extract timestamp
            if let ts = json["timestamp"] as? Double {
                timestamp = Int64(ts)
            } else if let ts = json["ts"] as? Int64 {
                timestamp = ts
            }

            // Extract content based on type
            switch type {
            case "user":
                if let message = json["message"] as? [String: Any],
                   let text = message["content"] as? String {
                    content = String(text.prefix(200)) // Preview only
                }
            case "assistant", "agent":
                if let message = json["message"] as? [String: Any],
                   let text = message["content"] as? String {
                    content = String(text.prefix(200))
                }
            case "tool_use", "tool_call":
                if let name = json["name"] as? String {
                    content = "Tool: \(name)"

                    // Extract file references from Read/Edit tools
                    if name.contains("Read") || name.contains("read") ||
                       name.contains("Edit") || name.contains("edit") ||
                       name.contains("content.get_span") {
                        if let args = json["arguments"] as? [String: Any],
                           let path = args["file_path"] as? String ?? args["path"] as? String {
                            fileReferences.insert(path)
                        }
                    }
                }
            case "thinking":
                if let text = json["content"] as? String {
                    content = String(text.prefix(100))
                }
            default:
                content = type
            }

            if !content.isEmpty {
                events.append(SessionEvent(
                    type: type,
                    lineNumber: start + idx + 1,
                    content: content,
                    timestamp: timestamp
                ))
            }
        }

        return SessionReadResult(
            sessionId: sessionId,
            events: events,
            truncated: events.count >= maxEvts,
            totalEvents: selectedLines.count,
            fileReferences: Array(fileReferences).sorted()
        )
    }
}

/// Tool for analyzing sessions
public struct SessionAnalyzeTool: Sendable {
    public init() {}

    public func analyze(
        sessionIds: [String],
        provider: String? = nil,
        metrics: [String]? = nil
    ) async throws -> SessionAnalyzeResult {
        let computeAll = metrics == nil
        let computeFiles = computeAll || metrics?.contains("files") == true
        let computeTools = computeAll || metrics?.contains("tools") == true
        let computeGoals = computeAll || metrics?.contains("goals") == true

        var fileFreq: [String: Int] = [:]
        var toolFreq: [String: Int] = [:]
        var goalPatterns: [String] = []
        var totalEvents = 0
        var sessionCount = 0

        // If no session IDs provided, discover recent sessions from provider(s)
        var effectiveSessionIds = sessionIds
        if effectiveSessionIds.isEmpty {
            let listTool = SessionListTool()

            if let specificProvider = provider {
                // Single provider
                let listResult = try await listTool.list(provider: specificProvider, topK: 10)
                effectiveSessionIds = listResult.sessions.map { $0.id }
            } else {
                // All providers - get 5 from each
                var allSessions: [(id: String, provider: String)] = []

                for prov in ["claude-code", "codex"] {
                    let listResult = try await listTool.list(provider: prov, topK: 5)
                    allSessions.append(contentsOf: listResult.sessions.map { (id: $0.id, provider: prov) })
                }

                effectiveSessionIds = allSessions.map { $0.id }
            }
        }

        // Read each session
        let readTool = SessionReadTool()
        for sessionId in effectiveSessionIds.prefix(10) { // Limit to 10 sessions for analysis
            do {
                let prov = provider ?? "claude-code" // Default provider
                let result = try await readTool.read(sessionId: sessionId, provider: prov, maxEvents: 200)
                sessionCount += 1
                totalEvents += result.events.count

                // Count file references
                if computeFiles {
                    for file in result.fileReferences {
                        fileFreq[file, default: 0] += 1
                    }
                }

                // Analyze events for tools and goals
                for event in result.events {
                    if computeTools && event.type.contains("tool") {
                        let toolName = event.content.replacingOccurrences(of: "Tool: ", with: "")
                        toolFreq[toolName, default: 0] += 1
                    }

                    if computeGoals && event.type == "user" && goalPatterns.count < 20 {
                        // Extract first user message as goal pattern
                        if !event.content.isEmpty {
                            goalPatterns.append(event.content)
                        }
                    }
                }
            } catch {
                // Skip sessions that can't be read
                continue
            }
        }

        let avgLength = sessionCount > 0 ? Double(totalEvents) / Double(sessionCount) : 0.0

        return SessionAnalyzeResult(
            fileFrequency: computeFiles ? fileFreq : nil,
            toolFrequency: computeTools ? toolFreq : nil,
            goalPatterns: computeGoals ? goalPatterns : nil,
            avgConversationLength: avgLength
        )
    }
}
