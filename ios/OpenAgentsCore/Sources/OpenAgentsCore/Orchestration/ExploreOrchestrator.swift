// ExploreOrchestrator.swift — On-device Foundation Models orchestrator for workspace exploration
// Part of Phase 2: orchestrate.explore.start implementation

import Foundation

#if canImport(FoundationModels)
import FoundationModels
#endif

/// Stream handler for ACP updates during orchestration
public typealias ACPUpdateStreamHandler = @Sendable (ACP.Client.SessionUpdate) async -> Void

/// Orchestration errors
public enum OrchestrationError: Error, LocalizedError {
    case modelUnavailable(String)
    case workspaceInvalid(String)
    case pathDenied(String)
    case executionFailed(String)

    public var errorDescription: String? {
        switch self {
        case .modelUnavailable(let reason):
            return "Foundation Models unavailable: \(reason)"
        case .workspaceInvalid(let reason):
            return "Invalid workspace: \(reason)"
        case .pathDenied(let path):
            return "Access denied to path: \(path)"
        case .executionFailed(let reason):
            return "Execution failed: \(reason)"
        }
    }
}

/// Actor managing workspace exploration orchestration
@available(iOS 26.0, macOS 26.0, *)
public actor ExploreOrchestrator {
    /// Workspace root path
    private let workspaceRoot: String

    /// Goals for exploration
    private let goals: [String]

    /// Policy governing exploration
    private let policy: ExplorationPolicy

    /// Current plan
    private var currentPlan: ExplorePlan?

    /// Executed operations (for deduplication)
    private var executedOps: Set<AgentOp> = []

    /// Stream handler for ACP updates
    private let streamHandler: ACPUpdateStreamHandler

    /// Tool executor
    private let toolExecutor: ToolExecutor

    public init(
        workspaceRoot: String,
        goals: [String],
        policy: ExplorationPolicy,
        streamHandler: @escaping ACPUpdateStreamHandler
    ) {
        self.workspaceRoot = workspaceRoot
        self.goals = goals
        self.policy = policy
        self.streamHandler = streamHandler
        self.toolExecutor = ToolExecutor(workspaceRoot: workspaceRoot)
    }

    /// Start exploration process
    public func startExploration() async throws -> ExploreSummary {
        #if canImport(FoundationModels)
        // Check Foundation Models availability
        let model = SystemLanguageModel.default
        switch model.availability {
        case .available:
            print("[Orchestrator] Foundation Models available")
        case .unavailable(let reason):
            let reasonStr = String(describing: reason)
            print("[Orchestrator] Foundation Models unavailable: \(reasonStr)")

            // Send agent message explaining unavailability
            await streamUnavailabilityMessage(reason: reasonStr)

            // Fail with error
            throw OrchestrationError.modelUnavailable(reasonStr)
        }

        // Validate workspace
        try validateWorkspace()

        // Generate initial plan
        let plan = try await generateInitialPlan(using: model)
        currentPlan = plan

        // Stream plan as ACP
        await streamPlan(plan)

        // Execute operations from plan
        try await executeOperations(plan.nextOps)

        // Generate summary (simplified for Phase 2)
        let summary = try await generateSummary()

        return summary
        #else
        // Foundation Models not available at compile time
        throw OrchestrationError.modelUnavailable("FoundationModels framework not available")
        #endif
    }

    // MARK: - Plan Generation

    #if canImport(FoundationModels)
    private func generateInitialPlan(using model: SystemLanguageModel) async throws -> ExplorePlan {
        let instructions = Instructions("""
        You are a workspace exploration assistant. You analyze code repositories and conversation history to extract insights.

        Available operations:
        - readSpan: Read specific lines from a file (e.g., README.md, package.json, main entry files)
        - grep: Search for patterns across files (e.g., function definitions, imports, TODOs)
        - listDir: List directory contents
        - sessionList: List recent conversation sessions from Claude Code or Codex
        - sessionSearch: Search across conversation history for specific patterns or topics
        - sessionRead: Read detailed content from a specific conversation session
        - sessionAnalyze: Aggregate insights from multiple sessions (file frequency, tool usage, patterns)

        Guidelines:
        - Start with top-level README or documentation files for code exploration
        - Use session tools to understand development history and patterns
        - Chain operations: search sessions → read relevant ones → analyze files mentioned
        - Keep operations focused and bounded
        - Maximum 5 operations in initial plan

        Output a plan with 3-5 concrete operations based on the goals provided.
        """)

        let session = LanguageModelSession(model: model, tools: [], instructions: instructions)
        try? session.prewarm(promptPrefix: nil)

        let workspaceName = (workspaceRoot as NSString).lastPathComponent
        let goalsStr = goals.isEmpty ? "Understand the repository structure" : goals.joined(separator: ", ")

        let prompt = """
        Workspace: \(workspaceName)
        Path: \(workspaceRoot)
        Goals: \(goalsStr)

        Create a plan with 3-5 operations to explore this workspace. For each operation, specify:
        1. Type (readSpan, grep, listDir, sessionList, sessionSearch, sessionRead, or sessionAnalyze)
        2. Parameters (file path, search pattern, session filters, etc.)

        Format examples:
        - readSpan: README.md lines 1-50
        - grep: "import.*from" in src/
        - listDir: src/
        - sessionList: top 10 claude-code sessions
        - sessionSearch: "authentication" in all sessions
        - sessionRead: session abc123
        - sessionAnalyze: sessions [id1, id2] for file frequency
        """

        do {
            let options = GenerationOptions(temperature: 0.2)
            let resp = try await session.respond(to: prompt, options: options)

            // Parse response to extract operations
            let desc = String(describing: resp)
            let ops = parseOperationsFromResponse(desc)

            print("[Orchestrator] Generated plan with \(ops.count) operations")

            return ExplorePlan(
                goals: goals,
                nextOps: ops
            )
        } catch {
            print("[Orchestrator] Error generating plan: \(error)")
            // Fallback to basic plan
            return createFallbackPlan()
        }
    }
    #endif

    /// Parse operations from FM response
    private func parseOperationsFromResponse(_ response: String) -> [AgentOp] {
        var ops: [AgentOp] = []

        // Extract content from response description
        let content = extractContent(from: response) ?? response

        // Simple parsing: look for operation patterns
        let lines = content.components(separatedBy: "\n")

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)

            // Parse session operations (Phase 2.5)
            if trimmed.contains("sessionList") || (trimmed.contains("session") && trimmed.contains("list")) {
                if let op = parseSessionList(from: trimmed) {
                    ops.append(op)
                }
            }
            else if trimmed.contains("sessionSearch") || (trimmed.contains("session") && trimmed.contains("search")) {
                if let op = parseSessionSearch(from: trimmed) {
                    ops.append(op)
                }
            }
            else if trimmed.contains("sessionRead") || (trimmed.contains("session") && trimmed.contains("read")) {
                if let op = parseSessionRead(from: trimmed) {
                    ops.append(op)
                }
            }
            else if trimmed.contains("sessionAnalyze") || (trimmed.contains("session") && trimmed.contains("analyze")) {
                if let op = parseSessionAnalyze(from: trimmed) {
                    ops.append(op)
                }
            }
            // Parse readSpan operations
            else if trimmed.contains("readSpan") || trimmed.contains("Read") {
                if let op = parseReadSpan(from: trimmed) {
                    ops.append(op)
                }
            }
            // Parse grep operations
            else if trimmed.contains("grep") || trimmed.contains("Search") {
                if let op = parseGrep(from: trimmed) {
                    ops.append(op)
                }
            }
            // Parse listDir operations
            else if trimmed.contains("listDir") || trimmed.contains("List") {
                if let op = parseListDir(from: trimmed) {
                    ops.append(op)
                }
            }
        }

        // If parsing failed, return fallback operations
        if ops.isEmpty {
            ops = createFallbackPlan().nextOps
        }

        return Array(ops.prefix(5)) // Limit to 5 operations
    }

    /// Create fallback plan when FM is unavailable or fails
    private func createFallbackPlan() -> ExplorePlan {
        return ExplorePlan(
            goals: goals.isEmpty ? ["Understand workspace structure"] : goals,
            nextOps: [
                AgentOp(kind: .readSpan(ReadSpanParams(path: "README.md", startLine: 1, endLine: 50))),
                AgentOp(kind: .listDir(ListDirParams(path: ".", depth: 0)))
            ]
        )
    }

    // MARK: - Operation Execution

    private func executeOperations(_ ops: [AgentOp]) async throws {
        for op in ops {
            // Skip if already executed (deduplication)
            if executedOps.contains(op) {
                print("[Orchestrator] Skipping duplicate op: \(op.description)")
                continue
            }

            // Execute operation
            try await executeOperation(op)

            // Mark as executed
            executedOps.insert(op)
        }
    }

    private func executeOperation(_ op: AgentOp) async throws {
        print("[Orchestrator] Executing: \(op.description)")

        // Stream tool call (started)
        await streamToolCall(op, status: .started)

        do {
            // Execute via ToolExecutor
            let result = try await toolExecutor.execute(op)

            // Stream tool call update (completed)
            await streamToolCallUpdate(op, status: .completed, output: result)
        } catch {
            print("[Orchestrator] Operation failed: \(error)")

            // Stream tool call update (error)
            await streamToolCallUpdate(op, status: .error, error: error.localizedDescription)

            throw error
        }
    }

    // MARK: - Summary Generation

    private func generateSummary() async throws -> ExploreSummary {
        // Simplified summary for Phase 2
        // In Phase 3, this would aggregate results from executed operations
        let workspaceName = (workspaceRoot as NSString).lastPathComponent

        return ExploreSummary(
            repo_name: workspaceName,
            languages: ["Swift": 1000], // Placeholder
            entrypoints: ["README.md"],
            top_files: ["README.md"],
            followups: ["Index repository for semantic search", "Analyze dependencies"]
        )
    }

    // MARK: - ACP Streaming

    private func streamPlan(_ plan: ExplorePlan) async {
        let acpPlan = plan.toACPPlan()
        let update = ACP.Client.SessionUpdate.plan(acpPlan)
        await streamHandler(update)
    }

    private func streamToolCall(_ op: AgentOp, status: ACPToolCallUpdateWire.Status) async {
        let toolCall = ACPToolCallWire(
            call_id: op.opId.uuidString,
            name: op.toolName,
            arguments: nil, // Could add op parameters here
            _meta: ["op_hash": AnyEncodable(op.opHash)]
        )
        let update = ACP.Client.SessionUpdate.toolCall(toolCall)
        await streamHandler(update)
    }

    private func streamToolCallUpdate(
        _ op: AgentOp,
        status: ACPToolCallUpdateWire.Status,
        output: (any Encodable)? = nil,
        error: String? = nil
    ) async {
        let update = ACPToolCallUpdateWire(
            call_id: op.opId.uuidString,
            status: status,
            output: output.map { AnyEncodable($0) },
            error: error,
            _meta: nil
        )
        let sessionUpdate = ACP.Client.SessionUpdate.toolCallUpdate(update)
        await streamHandler(sessionUpdate)
    }

    private func streamUnavailabilityMessage(reason: String) async {
        let message = "Foundation Models are unavailable on this device (\(reason)). Unable to generate exploration plan."
        let chunk = ACP.Client.ContentChunk(
            content: .text(.init(text: message))
        )
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)
        await streamHandler(update)
    }

    // MARK: - Validation

    private func validateWorkspace() throws {
        let fm = FileManager.default

        // Check workspace exists
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: workspaceRoot, isDirectory: &isDir), isDir.boolValue else {
            throw OrchestrationError.workspaceInvalid("Path does not exist or is not a directory")
        }

        // Check readable
        guard fm.isReadableFile(atPath: workspaceRoot) else {
            throw OrchestrationError.pathDenied(workspaceRoot)
        }
    }

    // MARK: - Helpers

    private func extractContent(from description: String) -> String? {
        // Similar to FoundationModelSummarizer extraction
        if description.localizedCaseInsensitiveContains("Safety guardrails") { return nil }
        if let m = match(description, pattern: #"content:\s*'([^']*)'"#) { return m }
        if let m = match(description, pattern: #"content:\s*\"([^\"]*)\""#) { return m }
        if let m = match(description, pattern: #"rawContent:\s*'([^']*)'"#) { return m }
        if let m = match(description, pattern: #"rawContent:\s*\"([^\"]*)\""#) { return m }
        return nil
    }

    private func match(_ s: String, pattern: String) -> String? {
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

    // Simple operation parsers (can be enhanced)
    private func parseReadSpan(from line: String) -> AgentOp? {
        // Example: "readSpan: README.md lines 1-50" or "Read README.md lines 1-50"
        // Very basic parsing - production would use more robust extraction
        if let path = extractPath(from: line),
           let (start, end) = extractLineRange(from: line) {
            return AgentOp(kind: .readSpan(ReadSpanParams(path: path, startLine: start, endLine: end)))
        }
        return nil
    }

    private func parseGrep(from line: String) -> AgentOp? {
        // Example: "grep: 'import' in src/" or "Search 'import' in src/"
        if let pattern = extractQuotedString(from: line) {
            let pathPrefix = extractPathAfterIn(from: line)
            return AgentOp(kind: .grep(GrepParams(pattern: pattern, pathPrefix: pathPrefix)))
        }
        return nil
    }

    private func parseListDir(from line: String) -> AgentOp? {
        // Example: "listDir: src/" or "List src/"
        if let path = extractPath(from: line) {
            return AgentOp(kind: .listDir(ListDirParams(path: path, depth: 0)))
        }
        return nil
    }

    private func extractPath(from line: String) -> String? {
        // Extract filename-like patterns
        let pattern = #"([a-zA-Z0-9_\-./]+\.[a-z]+|[a-zA-Z0-9_\-./]+/)"#
        return match(line, pattern: pattern)
    }

    private func extractLineRange(from line: String) -> (Int, Int)? {
        // Extract "1-50" or "lines 1-50"
        if let m = match(line, pattern: #"(\d+)-(\d+)"#) {
            let parts = m.components(separatedBy: "-")
            if parts.count == 2, let start = Int(parts[0]), let end = Int(parts[1]) {
                return (start, end)
            }
        }
        return nil
    }

    private func extractQuotedString(from line: String) -> String? {
        // Extract text within quotes
        if let m = match(line, pattern: #"['"]([^'"]+)['"]"#) {
            return m
        }
        return nil
    }

    private func extractPathAfterIn(from line: String) -> String? {
        // Extract path after "in"
        if let m = match(line, pattern: #"in\s+([a-zA-Z0-9_\-./]+)"#) {
            return m
        }
        return nil
    }

    // MARK: - Session Operation Parsers (Phase 2.5)

    private func parseSessionList(from line: String) -> AgentOp? {
        // Example: "sessionList: top 10 claude-code sessions"
        let provider = extractProvider(from: line)
        let topK = extractNumber(from: line)
        return AgentOp(kind: .sessionList(SessionListParams(provider: provider, topK: topK)))
    }

    private func parseSessionSearch(from line: String) -> AgentOp? {
        // Example: "sessionSearch: \"authentication\" in all sessions"
        if let pattern = extractQuotedString(from: line) {
            let provider = extractProvider(from: line)
            return AgentOp(kind: .sessionSearch(SessionSearchParams(pattern: pattern, provider: provider)))
        }
        return nil
    }

    private func parseSessionRead(from line: String) -> AgentOp? {
        // Example: "sessionRead: session abc123"
        if let sessionId = extractSessionId(from: line) {
            let provider = extractProvider(from: line) ?? "claude-code"
            return AgentOp(kind: .sessionRead(SessionReadParams(sessionId: sessionId, provider: provider)))
        }
        return nil
    }

    private func parseSessionAnalyze(from line: String) -> AgentOp? {
        // Example: "sessionAnalyze: sessions [id1, id2] for file frequency"
        // Simplified: just create with empty session IDs, will be populated by FM context
        let provider = extractProvider(from: line)
        // For now, return a basic analyze op - FM should provide session IDs from prior sessionList/sessionSearch
        return AgentOp(kind: .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: provider)))
    }

    // MARK: - Session Extraction Helpers

    private func extractProvider(from line: String) -> String? {
        // Extract "claude-code" or "codex"
        if line.localizedCaseInsensitiveContains("claude-code") || line.localizedCaseInsensitiveContains("claude code") {
            return "claude-code"
        }
        if line.localizedCaseInsensitiveContains("codex") {
            return "codex"
        }
        return nil
    }

    private func extractNumber(from line: String) -> Int? {
        // Extract number like "10" from "top 10"
        if let m = match(line, pattern: #"top\s+(\d+)"#) {
            return Int(m)
        }
        if let m = match(line, pattern: #"(\d+)\s+sessions?"#) {
            return Int(m)
        }
        return nil
    }

    private func extractSessionId(from line: String) -> String? {
        // Extract session ID (alphanumeric + dashes)
        if let m = match(line, pattern: #"session\s+([a-zA-Z0-9\-_]+)"#) {
            return m
        }
        return nil
    }
}
