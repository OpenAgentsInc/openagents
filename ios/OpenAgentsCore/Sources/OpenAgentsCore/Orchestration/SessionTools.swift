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
        let deadline = Date().addingTimeInterval(1.0) // hard budget for titles

        // Collect from Claude Code if requested
        if provider == nil || provider == "claude-code" {
            let claudeBase = ClaudeCodeScanner.defaultBaseDir()
            if FileManager.default.fileExists(atPath: claudeBase.path) {
                let files = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: k)
                for url in files {
                    let summary: ThreadSummary
                    if Date() > deadline {
                        summary = ClaudeCodeScanner.makeSummaryFast(for: url, base: claudeBase)
                    } else {
                        summary = ClaudeCodeScanner.makeSummary(for: url, base: claudeBase)
                    }

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
                let files = CodexScanner.listRecentTopN(at: codexBase, topK: k)
                for url in files {
                    let summary: ThreadSummary
                    if Date() > deadline {
                        summary = CodexScanner.makeSummaryFast(for: url, base: codexBase)
                    } else {
                        summary = CodexScanner.makeSummary(for: url, base: codexBase)
                    }

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
        // Cap context lines to avoid huge payloads
        let context = min(contextLines ?? 2, 4)
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

                    func trim(_ s: String) -> String {
                        if s.count > 240 { return String(s.prefix(240)) + "…" }
                        return s
                    }
                    let trimmedLine = trim(line)
                    let trimmedBefore = contextBefore?.map { trim($0) }
                    let trimmedAfter = contextAfter?.map { trim($0) }
                    matches.append(SessionSearchMatch(
                        sessionId: session.id,
                        provider: session.provider,
                        lineNumber: lineIdx + 1,
                        line: trimmedLine,
                        contextBefore: trimmedBefore,
                        contextAfter: trimmedAfter
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
        let startTime = Date()
        let maxSeconds: TimeInterval = 1.5
        let maxBytes: Int = 1 * 1024 * 1024 // 1MB safety cap

        // Find session file
        let listTool = SessionListTool()
        // Prefer small topK for responsiveness; fallback to larger if not found
        var sessions = try await listTool.list(provider: provider, topK: 60).sessions
        if !sessions.contains(where: { $0.id == sessionId }) {
            sessions = try await listTool.list(provider: provider, topK: 200).sessions
        }
        guard let session = sessions.first(where: { $0.id == sessionId }) else {
            throw ToolExecutionError.fileNotFound("Session not found: \(sessionId)")
        }

        let url = URL(fileURLWithPath: session.file_path)
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw ToolExecutionError.fileNotFound("Session file not found: \(url.path)")
        }

        // Incremental read to avoid loading entire file (large JSONL)
        let startIdx = max((startLine ?? 1), 1)
        let endIdx = endLine
        var currentLine = 0
        var selectedCount = 0
        var events: [SessionEvent] = []
        var fileReferences: Set<String> = []
        var truncated = false
        var bytesProcessed = 0

        guard let fh = try? FileHandle(forReadingFrom: url) else {
            throw ToolExecutionError.executionFailed("Failed to open session file")
        }
        defer { try? fh.close() }

        let chunkSize = 64 * 1024
        var buffer = Data()
        func processLine(_ line: String) {
            currentLine += 1
            guard currentLine >= startIdx else { return }
            if let endIdx = endIdx, currentLine > endIdx { return }
            selectedCount += 1
            guard events.count < maxEvts else { truncated = true; return }
            guard let data = line.data(using: .utf8) else { return }
            // Skip extremely long single-line events
            if data.count > 256 * 1024 { return }
            guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

            let type = json["type"] as? String ?? "unknown"
            var content = ""
            var timestamp: Int64? = nil
            if let ts = json["timestamp"] as? Double { timestamp = Int64(ts) }
            else if let ts = json["ts"] as? Int64 { timestamp = ts }

            switch type {
            case "user":
                if let message = json["message"] as? [String: Any], let text = message["content"] as? String {
                    content = String(text.prefix(200))
                }
            case "assistant", "agent":
                if let message = json["message"] as? [String: Any], let text = message["content"] as? String {
                    content = String(text.prefix(200))
                }
            case "tool_use", "tool_call":
                if let name = json["name"] as? String {
                    content = "Tool: \(name)"
                    if name.localizedCaseInsensitiveContains("read") ||
                       name.localizedCaseInsensitiveContains("edit") ||
                       name.contains("content.get_span") {
                        if let args = json["arguments"] as? [String: Any],
                           let path = args["file_path"] as? String ?? args["path"] as? String {
                            fileReferences.insert(path)
                        }
                    }
                }
            case "thinking":
                if let text = json["content"] as? String { content = String(text.prefix(100)) }
            default:
                content = type
            }

            if !content.isEmpty {
                events.append(SessionEvent(
                    type: type,
                    lineNumber: currentLine,
                    content: content,
                    timestamp: timestamp
                ))
            }
        }

        var stop = false
        while true {
            guard let chunk = try? fh.read(upToCount: chunkSize), !chunk.isEmpty else { break }
            bytesProcessed += chunk.count
            buffer.append(chunk)
            while let nlIndex = buffer.firstIndex(of: 0x0A) { // '\n'
                let lineData = buffer[..<nlIndex]
                let removeEnd = buffer.index(after: nlIndex)
                buffer.removeSubrange(..<removeEnd)
                if !lineData.isEmpty {
                    let line = String(decoding: lineData, as: UTF8.self)
                    processLine(line)
                }
                if let endIdx = endIdx, currentLine >= endIdx { stop = true; break }
                if events.count >= maxEvts { truncated = true; stop = true; break }
                // Yield occasionally
                await Task.yield()
            }
            if stop { break }
            // Safety caps to avoid stalls on huge files
            if Date().timeIntervalSince(startTime) > maxSeconds { print("[SessionRead] time cap hit for id=\(sessionId)"); truncated = true; break }
            if bytesProcessed > maxBytes { print("[SessionRead] byte cap hit for id=\(sessionId) bytes=\(bytesProcessed)"); truncated = true; break }
        }
        // Process any remaining data as last line (if not empty)
        if !buffer.isEmpty, events.count < maxEvts {
            let line = String(decoding: buffer, as: UTF8.self)
            processLine(line)
        }
        print("[SessionRead] done id=\(sessionId) events=\(events.count) processedBytes=\(bytesProcessed) in \(String(format: "%.2f", Date().timeIntervalSince(startTime)))s")

        return SessionReadResult(
            sessionId: sessionId,
            events: events,
            truncated: truncated || events.count >= maxEvts,
            totalEvents: selectedCount,
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
        metrics: [String]? = nil,
        progress: ((Int, Int) -> Void)? = nil
    ) async throws -> SessionAnalyzeResult {
        print("[SessionAnalyze] start provider=\(provider ?? "<any>") explicit_ids=\(sessionIds.count) metrics=\(metrics ?? ["files","tools","goals"]))")
        let computeAll = metrics == nil
        let computeFiles = computeAll || metrics?.contains("files") == true
        let computeTools = computeAll || metrics?.contains("tools") == true
        let computeGoals = computeAll || metrics?.contains("goals") == true

        var fileFreq: [String: Int] = [:]
        var toolFreq: [String: Int] = [:]
        var goalPatterns: [String] = []
        var totalEvents = 0
        var sessionCount = 0

        // Build targets (sessionId + provider if known)
        var targets: [(id: String, provider: String?)] = []
        if sessionIds.isEmpty {
            let listTool = SessionListTool()
            if let specificProvider = provider {
                // Only the most recent session for the specified provider
                let listResult = try await listTool.list(provider: specificProvider, topK: 1)
                targets = listResult.sessions.map { ($0.id, specificProvider) }
            } else {
                // Most recent one per provider for intent inference
                for prov in ["claude-code", "codex"] {
                    let listResult = try await listTool.list(provider: prov, topK: 1)
                    targets.append(contentsOf: listResult.sessions.map { ($0.id, prov) })
                }
            }
        } else {
            // Caller provided explicit session IDs. Respect provided provider if present; otherwise, we'll auto-detect per session.
            targets = sessionIds.map { ($0, provider) }
        }
        print("[SessionAnalyze] targets=\(targets.count) (limit=10)")

        // Read each target (most recent per provider by default)
        let readTool = SessionReadTool()
        let total = min(targets.count, 10)
        for (idx, target) in targets.prefix(10).enumerated() {
            do {
                // Determine provider for this session; if unknown, try both providers.
                let result: SessionReadResult
                if let prov = target.provider {
                    print("[SessionAnalyze] read session id=\(target.id) provider=\(prov) …")
                    result = try await readTool.read(sessionId: target.id, provider: prov, maxEvents: 200)
                } else {
                    // Try claude-code first, then codex
                    print("[SessionAnalyze] read session id=\(target.id) provider=<auto> try claude-code …")
                    if let r1 = try? await readTool.read(sessionId: target.id, provider: "claude-code", maxEvents: 200) {
                        result = r1
                        print("[SessionAnalyze] id=\(target.id) resolved provider=claude-code events=\(r1.events.count)")
                    } else if let r2 = try? await readTool.read(sessionId: target.id, provider: "codex", maxEvents: 200) {
                        result = r2
                        print("[SessionAnalyze] id=\(target.id) resolved provider=codex events=\(r2.events.count)")
                    } else {
                        print("[SessionAnalyze] skip id=\(target.id) not found in either provider")
                        continue // Skip if not found under either provider
                    }
                }
                sessionCount += 1
                totalEvents += result.events.count
                if (idx + 1) % 1 == 0 { // log every session
                    print("[SessionAnalyze] progress \(idx+1)/\(min(targets.count, 10)) sessions, totalEvents=\(totalEvents)")
                }
                // Emit progress callback
                progress?(idx + 1, total)

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
                print("[SessionAnalyze] error reading id=\(target.id): \(error.localizedDescription)")
                continue
            }
        }

        let avgLength = sessionCount > 0 ? Double(totalEvents) / Double(sessionCount) : 0.0
        // Infer a single user intent: pick the first user message from the most recent sessions
        let inferredIntent: String? = {
            if let first = goalPatterns.first { return first }
            return nil
        }()

        let result = SessionAnalyzeResult(
            fileFrequency: computeFiles ? fileFreq : nil,
            toolFrequency: computeTools ? toolFreq : nil,
            goalPatterns: computeGoals ? goalPatterns : nil,
            avgConversationLength: avgLength,
            userIntent: inferredIntent
        )
        print("[SessionAnalyze] done sessions=\(sessionCount) avgLen=\(Int(avgLength)) files=\(fileFreq.count) tools=\(toolFreq.count) goals=\(goalPatterns.count)")
        return result
    }
}
