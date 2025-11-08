// ExploreOrchestrator.swift — On-device Foundation Models orchestrator for workspace exploration
// Part of Phase 2: orchestrate.explore.start implementation

import Foundation

#if canImport(FoundationModels)
import FoundationModels

// MARK: - Guided Generation Types for Plan Creation

@available(iOS 26.0, macOS 26.0, *)
@Generable
struct ExplorationPlanResponse {
    @Guide(description: "EXACTLY 3 operations to explore the workspace.")
    let operations: [PlannedOperation]
}

@available(iOS 26.0, macOS 26.0, *)
@Generable
struct PlannedOperation {
    @Guide(description: "Operation type: sessionList, readSpan, grep, or listDir")
    let type: String

    @Guide(description: "Single parameter value. Examples: claude-code, /path/to/file, pattern")
    let param: String
}

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

    // MARK: - Persistent FM session
    #if canImport(FoundationModels)
    /// Persistent Foundation Models session (reused across exploration calls)
    /// The session maintains its own conversation history automatically
    private var fmSession: LanguageModelSession?
    /// Turn counter for context management
    private var sessionTurnCount: Int = 0
    #endif

    /// Current plan
    private var currentPlan: ExplorePlan?
    // Planning / streaming reducer
    private let planning: PlanningReducer

    /// Executed operations (for deduplication)
    private var executedOps: Set<AgentOp> = []

    /// Results from executed operations
    private var operationResults: [AgentOp: any Encodable] = [:]

    /// Stream handler for ACP updates
    private let streamHandler: ACPUpdateStreamHandler

    /// Tool executor (lazy so we can safely capture `self` for progress callbacks)
    private lazy var toolExecutor: ToolExecutor = {
        ToolExecutor(workspaceRoot: workspaceRoot, progress: { [weak self] op, fraction, note in
            await self?.streamProgress(op, fraction: fraction, note: note)
        })
    }()

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
        self.planning = PlanningReducer(stream: streamHandler)
        // toolExecutor is lazy and will be created on first use
    }

    /// Start exploration process
    public func startExploration() async throws -> ExploreSummary {
        #if canImport(FoundationModels)
        // Check Foundation Models availability
        let model = SystemLanguageModel.default
        switch model.availability {
        case .available:
            OpenAgentsLog.orchestration.info("Foundation Models available")
        case .unavailable(let reason):
            let reasonStr = String(describing: reason)
            OpenAgentsLog.orchestration.warning("Foundation Models unavailable: \(reasonStr)")

            // Send agent message explaining unavailability
            await streamUnavailabilityMessage(reason: reasonStr)

            // Fail with error
            throw OrchestrationError.modelUnavailable(reasonStr)
        }

        // Validate workspace
        try validateWorkspace()

        // Route based on policy flag
        if policy.use_native_tool_calling {
            // NEW PATH: Native FM tool calling loop
            OpenAgentsLog.orchestration.info("Using native FM tool calling loop (experimental)")
            let summary = try await executeNativeToolCallingLoop()
            return summary
        } else {
            // LEGACY PATH: Text-based plan generation and parsing
            OpenAgentsLog.orchestration.info("Using legacy text plan parsing")

            // Generate initial plan
            let plan = try await generateInitialPlan(using: model)
            currentPlan = plan

            // Stream plan as ACP
            await planning.streamPlan(plan)

            // Execute operations from plan
            try await executeOperations(plan.nextOps)

            // Generate summary (simplified for Phase 2)
            let summary = try await generateSummary()

            return summary
        }
        #else
        // Foundation Models not available at compile time
        throw OrchestrationError.modelUnavailable("FoundationModels framework not available")
        #endif
    }

    // MARK: - Session Management

    #if canImport(FoundationModels)
    /// Get or create persistent FM session with tools
    @available(iOS 26.0, macOS 26.0, *)
    private func getOrCreateSession() async throws -> LanguageModelSession {
        if let existing = fmSession {
            OpenAgentsLog.orchestration.debug("Reusing existing FM session (turn \(self.sessionTurnCount))")
            return existing
        }

        let tools = FMToolsRegistry.defaultTools(workspaceRoot: workspaceRoot)
        let instructions = Instructions("""
        You are a workspace exploration assistant. Use the available tools to explore the workspace and achieve the user's goals.

        Available tools:
        - session.list: List recent conversation sessions
        - session.search: Search sessions for patterns
        - session.read: Read session content
        - session.analyze: Analyze sessions for insights
        - content.get_span: Read file content
        - code.grep: Search code
        - fs.list_dir: List directory contents

        After using tools, summarize your findings and suggest next steps.
        """)

        let session = LanguageModelSession(
            model: SystemLanguageModel.default,
            tools: tools,
            instructions: instructions
        )

        session.prewarm(promptPrefix: nil)
        fmSession = session
        sessionTurnCount = 0
        OpenAgentsLog.orchestration.info("FM session created with \(tools.count) tools")
        return session
    }
    /// Execute native FM tool calling loop
    /// The session handles tool calling automatically - tools are invoked by FM as needed
    @available(iOS 26.0, macOS 26.0, *)
    private func executeNativeToolCallingLoop() async throws -> ExploreSummary {
        let session = try await getOrCreateSession()
        sessionTurnCount += 1

        let workspaceName = (workspaceRoot as NSString).lastPathComponent
        let goalsStr = goals.isEmpty ? "(explore the workspace)" : goals.joined(separator: "\n- ")

        let prompt = """
        Workspace: \(workspaceName)
        Goals:
        - \(goalsStr)

        Use the available tools to explore the workspace and achieve these goals. Start by analyzing recent sessions to understand the user's work patterns, then use other tools as needed.

        After using tools, provide a summary of your findings.
        """

        OpenAgentsLog.orchestration.info("Starting native tool calling (turn \(self.sessionTurnCount))")

        // Send prompt - FM session will automatically call tools as needed
        let t0 = Date()
        let response = try await session.respond(to: prompt)
        let elapsed = Date().timeIntervalSince(t0)

        OpenAgentsLog.orchestration.debug("FM response received in \(String(format: "%.2f", elapsed))s")
        OpenAgentsLog.orchestration.debug("Response: \(response.content.prefix(200))...")

        // Stream the response content as agent message chunk
        let chunk = ACP.Client.ContentChunk(
            content: .text(.init(text: response.content))
        )
        await streamHandler(.agentMessageChunk(chunk))

        // Generate summary from the response
        let summary = try await generateSummaryFromResponse(response.content)

        return summary
    }

    /// Generate summary from FM response content
    @available(iOS 26.0, macOS 26.0, *)
    private func generateSummaryFromResponse(_ content: String) async throws -> ExploreSummary {
        // Extract insights from the response
        let workspaceName = (workspaceRoot as NSString).lastPathComponent

        // For now, create a simple summary
        // In future iterations, we could use FM to structure this better
        let summary = ExploreSummary(
            repo_name: workspaceName,
            languages: [:],  // Could extract from response
            entrypoints: [],  // Could extract from response
            top_files: [],    // Could extract from response
            followups: [content]  // Use the FM response as a follow-up suggestion
        )

        return summary
    }
    #endif

    // MARK: - Plan Generation

    #if canImport(FoundationModels)
    @available(iOS 26.0, macOS 26.0, *)
    private func generateInitialPlan(using model: SystemLanguageModel) async throws -> ExplorePlan {
        // Minimal instructions to stay under 4096 token context limit
        let instructions = Instructions("""
        Plan a short workspace exploration (3–5 steps) using these ops: sessionList, sessionSearch, sessionRead, listDir, readSpan, grep, sessionAnalyze. Prefer sessionSearch + sessionRead for conversation-history goals; use listDir/readSpan/grep for code/file goals. Output only the steps, one per line, mentioning the op name and key parameter(s).

        Available operations:
        
        """)

        let tools: [any Tool] = {
            var t: [any Tool] = []
            #if canImport(FoundationModels)
            if #available(iOS 26.0, macOS 26.0, *) {
                t = FMToolsRegistry.defaultTools(workspaceRoot: workspaceRoot)
            }
            #endif
            return t
        }()
        let session = LanguageModelSession(model: model, tools: tools, instructions: instructions)
        session.prewarm(promptPrefix: nil)

        let workspaceName = (workspaceRoot as NSString).lastPathComponent
        let goalsStr = goals.isEmpty ? "(no goals provided)" : goals.joined(separator: "\n- ")

        // Minimal prompt - MUST stay well under 4096 tokens total (instructions + prompt)
        let prompt = """
        Workspace: \(workspaceName)
        Goals:
        - \(goalsStr)

        Draft a 3–5 step plan using the available ops (sessionList, sessionSearch, sessionRead, listDir, readSpan, grep, sessionAnalyze). Output only the steps, one per line.
        """

        // Validate context size before sending
        let estimatedTokens = PlanningReducer.estimateTokenCount(instructions: instructions, prompt: prompt)
        guard estimatedTokens < 3500 else {
            throw OrchestrationError.executionFailed("Prompt too large: \(estimatedTokens) tokens (limit: 4096). Truncate goals or workspace path.")
        }

        OpenAgentsLog.orchestration.debug("Estimated tokens: \(estimatedTokens)")
        let instrChars = String(describing: instructions).count
        OpenAgentsLog.orchestration.debug("FM preparing request: instructions=\(instrChars) chars, prompt=\(prompt.count) chars")

        do {
            let options = GenerationOptions(temperature: 0.5)
            let t0 = Date()
            let response = try await session.respond(to: prompt, options: options)
            OpenAgentsLog.orchestration.debug("FM response received in \(String(format: "%.2f", Date().timeIntervalSince(t0)))s")
            let raw = response.content
            let ops = try PlanningReducer.parseOperationsFromResponse(raw)

            OpenAgentsLog.orchestration.info("Generated plan with \(ops.count) operations")

            guard !ops.isEmpty else {
                throw OrchestrationError.executionFailed("FM generated empty plan")
            }

            // Post-process: add sessionAnalyze if we have session operations
            // This reads actual conversation content for deeper analysis
            let finalOps = PlanningReducer.addAnalysisIfNeeded(ops)

            return ExplorePlan(
                goals: goals,
                nextOps: finalOps
            )
        } catch {
            OpenAgentsLog.orchestration.error("Error generating plan: \(error)")
            throw OrchestrationError.executionFailed("Failed to generate exploration plan: \(error.localizedDescription)")
        }
    }

    // addAnalysisIfNeeded moved to PlanningReducer.addAnalysisIfNeeded

    /// Convert PlannedOperation (from guided generation) to AgentOp
    /// Hard-codes all numeric parameters to avoid confusing the model
    @available(iOS 26.0, macOS 26.0, *)
    private func convertPlannedOperation(_ op: PlannedOperation) -> AgentOp? {
        let type = op.type.lowercased()
        let param = op.param

        switch type {
        case "sessionlist", "session.list":
            // param = provider name (claude-code or codex)
            // Hard-code topK=20 (never ask model for numbers)
            let provider = param.lowercased()
            return AgentOp(kind: .sessionList(SessionListParams(provider: provider, topK: 20)))

        case "readspan", "read":
            // param = file path
            // Hard-code startLine=1, endLine=100
            return AgentOp(kind: .readSpan(ReadSpanParams(path: param, startLine: 1, endLine: 100)))

        case "grep", "search":
            // param = search pattern
            return AgentOp(kind: .grep(GrepParams(pattern: param, pathPrefix: nil)))

        case "listdir", "list":
            // param = directory path
            // Hard-code depth=0
            return AgentOp(kind: .listDir(ListDirParams(path: param, depth: 0)))

        default:
            OpenAgentsLog.orchestration.warning("Unknown operation type: \(type)")
            return nil
        }
    }
    #endif

    /// Parse operations from FM response
    private func parseOperationsFromResponse(_ response: String) throws -> [AgentOp] {
        var ops: [AgentOp] = []

        // Extract content from response description
        let content = extractContent(from: response) ?? response

        OpenAgentsLog.orchestration.debug("Parsing FM response (first 500 chars): \(String(content.prefix(500)))")

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

        // No fallback - if parsing failed, throw an error with the response for debugging
        if ops.isEmpty {
            throw OrchestrationError.executionFailed("Failed to parse any operations from FM response. Response was: \(String(content.prefix(1000)))")
        }

        return Array(ops.prefix(5)) // Limit to 5 operations
    }

    // MARK: - Operation Execution

    private func executeOperations(_ ops: [AgentOp]) async throws {
        // Deduplicate
        let pending = ops.filter { !executedOps.contains($0) }
        guard !pending.isEmpty else { return }
        let exec = ToolExecutionOrchestrator(toolExecutor: toolExecutor, stream: streamHandler, planner: planning)
        let results = try await exec.execute(pending)
        for (op, result) in results {
            operationResults[op] = result
            executedOps.insert(op)
        }
    }

    // MARK: - Summary Generation

    private func generateSummary() async throws -> ExploreSummary {
        let workspaceName = (workspaceRoot as NSString).lastPathComponent

        var topFiles: [String] = []
        var followups: [String] = []
        var fileFrequency: [String: Int] = [:]
        var allSessions: [(provider: String, sessions: [SessionMetadata])] = []

        // First pass: collect all sessions
        for (op, result) in operationResults {
            switch op.kind {
            case .sessionList(let params):
                if let listResult = result as? SessionListResult {
                    let provider = params.provider ?? "all"
                    allSessions.append((provider: provider, sessions: listResult.sessions))
                }
            default:
                break
            }
        }

        // Second pass: aggregate session insights
        if !allSessions.isEmpty {
            let totalCount = allSessions.reduce(0) { $0 + $1.sessions.count }
            followups.append("Found \(totalCount) total sessions across \(allSessions.count) providers")

            // Collect unique, interesting titles (skip "Warmup" and duplicates)
            var seenTitles = Set<String>()
            var interestingTitles: [String] = []

            for (_, sessions) in allSessions {
                for session in sessions {
                    guard let title = session.title else { continue }

                    // Skip generic titles
                    if title == "Warmup" || title.isEmpty { continue }

                    // Clean up title
                    let cleanTitle = title
                        .replacingOccurrences(of: "\n", with: " ")
                        .trimmingCharacters(in: CharacterSet.whitespacesAndNewlines)
                        .components(separatedBy: CharacterSet.whitespacesAndNewlines)
                        .filter { !$0.isEmpty }
                        .joined(separator: " ")

                    // Deduplicate
                    if seenTitles.contains(cleanTitle) { continue }
                    seenTitles.insert(cleanTitle)

                    // Truncate
                    let truncated = cleanTitle.count > 70
                        ? String(cleanTitle.prefix(70)) + "..."
                        : cleanTitle

                    interestingTitles.append(truncated)

                    if interestingTitles.count >= 8 { break }
                }
                if interestingTitles.count >= 8 { break }
            }

            if !interestingTitles.isEmpty {
                followups.append("**Recent work:**")
                for title in interestingTitles {
                    // Do not prefix bullets here; the server formats bullets.
                    followups.append(title)
                }
            }
        }

        // Third pass: process other operation types
        for (op, result) in operationResults {
            switch op.kind {
            case .sessionList:
                // Already processed above
                break

            case .sessionSearch(let params):
                if let searchResult = result as? SessionSearchResult {
                    followups.append("Search '\(params.pattern)' found \(searchResult.matches.count) matches")
                }

            case .sessionRead(let params):
                if let readResult = result as? SessionReadResult {
                    // Add file references from session to top files
                    topFiles.append(contentsOf: readResult.fileReferences.prefix(10))

                    // Track file frequency
                    for file in readResult.fileReferences {
                        fileFrequency[file, default: 0] += 1
                    }

                    followups.append("Session \(params.sessionId): \(readResult.events.count) events, \(readResult.fileReferences.count) files referenced")
                }

            case .sessionAnalyze:
                if let analyzeResult = result as? SessionAnalyzeResult {
                    // Use file frequency from analysis
                    if let freq = analyzeResult.fileFrequency {
                        for (file, count) in freq {
                            fileFrequency[file, default: 0] += count
                        }
                    }

                    // Add goal patterns as insights
                    if let goals = analyzeResult.goalPatterns {
                        followups.append("Common goals: \(goals.prefix(3).joined(separator: ", "))")
                    }

                    if let avgLength = analyzeResult.avgConversationLength {
                        followups.append("Average conversation: \(Int(avgLength)) events")
                    }
                }

            case .readSpan:
                if let spanResult = result as? ContentSpanResult {
                    topFiles.append(spanResult.path)
                }

            case .grep:
                if let grepResult = result as? GrepResult {
                    let uniqueFiles = Set(grepResult.matches.map { $0.path })
                    topFiles.append(contentsOf: uniqueFiles)
                    followups.append("Pattern '\(grepResult.pattern)' found in \(uniqueFiles.count) files")
                }

            default:
                break
            }
        }

        // Get most frequently referenced files
        let sortedByFrequency = fileFrequency.sorted { $0.value > $1.value }
        let mostFrequentFiles = sortedByFrequency.prefix(10).map { $0.key }

        // Combine with discovered files, prioritize frequent ones
        let finalTopFiles = mostFrequentFiles + topFiles.filter { !mostFrequentFiles.contains($0) }
        let uniqueTopFiles = Array(Set(finalTopFiles)).prefix(10)

        // If we found file frequency data, add it to followups
        if !sortedByFrequency.isEmpty {
            let top3 = sortedByFrequency.prefix(3).map { "\($0.key) (\($0.value)x)" }
            followups.insert("Most modified files: \(top3.joined(separator: ", "))", at: 0)
        }

        // Return actual data - no placeholders
        return ExploreSummary(
            repo_name: workspaceName,
            languages: [:],
            entrypoints: [],
            top_files: Array(uniqueTopFiles),
            followups: Array(followups.prefix(5))
        )
    }

    // MARK: - ACP Streaming

    // streamPlan is handled by PlanningReducer

    // updatePlanEntry is handled by PlanningReducer

    // streamToolCall moved to ToolExecutionOrchestrator

    // streamToolCallUpdate moved to ToolExecutionOrchestrator

    // MARK: - FM-Powered Insight Generation
    /// Use Foundation Models to turn computed metrics into actual analysis text
    @available(iOS 26.0, macOS 26.0, *)
    private func generateFMAnalysis() async -> FMAnalysisResult? {
        #if canImport(FoundationModels)
        // Extract latest session.analyze result
        var analyze: SessionAnalyzeResult?
        for (op, result) in operationResults {
            if case .sessionAnalyze = op.kind, let r = result as? SessionAnalyzeResult {
                analyze = r
            }
        }
        guard let analyze = analyze else { return nil }

        // Deterministic summary when we already have explicit userIntent/goalPatterns
        if let intent = analyze.userIntent?.trimmingCharacters(in: .whitespacesAndNewlines), !intent.isEmpty {
            // Normalize newlines and trim items
            let lines = intent
                .replacingOccurrences(of: "\r", with: "\n")
                .components(separatedBy: "\n")
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .filter { !$0.isEmpty }

            // If intent starts with a label like "Read:", convert bullets into a clear sentence.
            if let first = lines.first, first.hasSuffix(":") {
                let label = first.dropLast().trimmingCharacters(in: .whitespaces) // e.g., "Read"
                let items = lines.dropFirst().map { raw -> String in
                    // Strip any leading bullet markers and extra dashes
                    let stripped = raw.replacingOccurrences(of: "^[\\s]*[-*•]+\\s*", with: "", options: .regularExpression)
                    // If item looks like an absolute path under the workspace, make it repo-relative
                    let rel = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: stripped)
                    // Prefer short display for paths
                    if rel != "." && rel != stripped { return rel }
                    return stripped
                }
                let nonEmpty = items.filter { !$0.isEmpty }
                if nonEmpty.isEmpty {
                    // Fall back to raw intent if we somehow stripped everything
                    return FMAnalysisResult(text: String(label), source: .sessionAnalyze)
                }
                // Join items succinctly (max 4)
                let maxItems = Array(nonEmpty.prefix(4))
                let joined: String = {
                    if maxItems.count == 1 { return maxItems[0] }
                    if maxItems.count == 2 { return "\(maxItems[0]) and \(maxItems[1])" }
                    let head = maxItems.dropLast().joined(separator: ", ")
                    if let last = maxItems.last { return "\(head), and \(last)" }
                    return head
                }()
                let sentence = "User intends to \(label.lowercased()) \(joined)."
                // Build context from session.analyze metrics
                let topFilesFromAnalyze: [String] = {
                    let pairs = (analyze.fileFrequency ?? [:]).sorted { $0.value > $1.value }
                    return pairs.prefix(5).map { k, _ in
                        let rel = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: k)
                        return rel
                    }
                }()
                let goals: [String] = (analyze.goalPatterns ?? []).prefix(3).map { $0 }
                let avg = analyze.avgConversationLength
                return FMAnalysisResult(text: sentence, source: .sessionAnalyze, topFiles: topFilesFromAnalyze, goalPatterns: goals, avgConversationLength: avg)
            }
            // Otherwise join as a compact sentence
            let sentence = lines.joined(separator: " ")
            let topFilesFromAnalyze: [String] = {
                let pairs = (analyze.fileFrequency ?? [:]).sorted { $0.value > $1.value }
                return pairs.prefix(5).map { k, _ in
                    let rel = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: k)
                    return rel
                }
            }()
            let goals: [String] = (analyze.goalPatterns ?? []).prefix(3).map { $0 }
            let avg = analyze.avgConversationLength
            return FMAnalysisResult(text: sentence, source: .sessionAnalyze, topFiles: topFilesFromAnalyze, goalPatterns: goals, avgConversationLength: avg)
        }

        // Build compact JSON context
        struct Context: Codable {
            let avgConversationLength: Double?
            let topFiles: [String]
            let fileFrequency: [String:Int]?
            let toolFrequency: [String:Int]?
            let goalPatterns: [String]?
        }
        // Use the file frequency keys we aggregated into summary as hints too
        let topFilesHint: [String] = {
            var set = Set<String>()
            for (op, result) in operationResults {
                if case .grep = op.kind, let r = result as? GrepResult {
                    r.matches.prefix(20).forEach { set.insert($0.path) }
                }
                if case .readSpan = op.kind, let r = result as? ContentSpanResult { set.insert(r.path) }
            }
            return Array(set).prefix(10).map { $0 }
        }()

        let ctx = Context(
            avgConversationLength: analyze.avgConversationLength,
            topFiles: topFilesHint,
            fileFrequency: analyze.fileFrequency,
            toolFrequency: analyze.toolFrequency,
            goalPatterns: analyze.goalPatterns?.prefix(12).map { $0 }
        )

        guard let ctxData = try? JSONEncoder().encode(ctx),
              let ctxStr = String(data: ctxData, encoding: .utf8) else {
            return nil
        }

        // Prepare prompt
        let instructions = Instructions("""
        Infer the current user intent based on recent conversations.
        - Focus only on intent from the latest claude-code/codex sessions.
        - Ignore file frequency and tool trends unless directly relevant to the intent.
        - Output 1–2 sentences clearly stating the user intent (no bullets).
        """)

        let model = SystemLanguageModel.default
        let tools: [any Tool] = {
            var t: [any Tool] = []
            #if canImport(FoundationModels)
            if #available(iOS 26.0, macOS 26.0, *) {
                t = FMToolsRegistry.defaultTools(workspaceRoot: workspaceRoot)
            }
            #endif
            return t
        }()
        let session = LanguageModelSession(model: model, tools: tools, instructions: instructions)

        let prompt = """
        Recent conversation metrics (for context):
        \(ctxStr)

        Based on the latest sessions, state the user intent in 1–2 sentences.
        """

        // Log sizes and time
        OpenAgentsLog.orchestration.debug("FM analysis request: instructions=\(String(describing: instructions).count) chars, prompt=\(prompt.count) chars, ctxBytes=\(ctxData.count)")
        do {
            let t0 = Date()
            let resp = try await session.respond(to: prompt)
            let dt = Date().timeIntervalSince(t0)
            // LanguageModelSession.Response<String> exposes `content` (not `text`)
            OpenAgentsLog.orchestration.debug("FM analysis response in \(String(format: "%.2f", dt))s, text=\(resp.content.count) chars")
            let analysis = resp.content.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !analysis.isEmpty else { return nil }
            // Provide context from analyze metrics and grep hints
            let topFilesFromAnalyze: [String] = {
                let pairs = (analyze.fileFrequency ?? [:]).sorted { $0.value > $1.value }
                let top = pairs.prefix(3).map { k, _ in k }
                let merged = Array((top + topFilesHint).prefix(5))
                return merged.map { PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: $0) }
            }()
            let goals: [String] = (analyze.goalPatterns ?? []).prefix(3).map { $0 }
            let avg = analyze.avgConversationLength
            return FMAnalysisResult(text: analysis, source: .fm, topFiles: topFilesFromAnalyze, goalPatterns: goals, avgConversationLength: avg)
        } catch {
            OpenAgentsLog.orchestration.error("FM analysis error: \(error)")
            return nil
        }
        #else
        return nil
        #endif
    }

    /// Result type for FM analysis (text + source)
    public struct FMAnalysisResult: Sendable {
        public enum Source: String, Sendable { case sessionAnalyze = "session.analyze", fm = "fm" }
        public let text: String
        public let source: Source
        public let topFiles: [String]
        public let goalPatterns: [String]
        public let avgConversationLength: Double?
        public init(text: String, source: Source, topFiles: [String] = [], goalPatterns: [String] = [], avgConversationLength: Double? = nil) {
            self.text = text
            self.source = source
            self.topFiles = topFiles
            self.goalPatterns = goalPatterns
            self.avgConversationLength = avgConversationLength
        }
    }

    /// Public accessor to compute FM analysis for the last run
    @available(iOS 26.0, macOS 26.0, *)
    public func fmAnalysis() async -> FMAnalysisResult? {
        return await generateFMAnalysis()
    }

    /// Stream progress updates for long-running tools (e.g., session.analyze)
    private func streamProgress(_ op: AgentOp, fraction: Double, note: String?) async {
        let meta: [String: AnyEncodable] = {
            var m: [String: AnyEncodable] = ["progress": AnyEncodable(fraction)]
            if let s = note { m["note"] = AnyEncodable(s) }
            return m
        }()
        let update = ACPToolCallUpdateWire(
            call_id: op.opId.uuidString,
            status: .started,
            output: nil,
            error: nil,
            _meta: meta
        )
        await streamHandler(.toolCallUpdate(update))
        OpenAgentsLog.orchestration.debug("Progress: \(op.toolName) \(Int(fraction * 100))% \(note ?? "")")
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
            var pathPrefix = extractPathAfterIn(from: line)
            if let p = pathPrefix { pathPrefix = normalizeWorkspacePath(p) }
            // If normalized points to workspace root, treat as nil (root)
            if pathPrefix == "workspace" || pathPrefix == "." || pathPrefix == "/" || pathPrefix == "/workspace" { pathPrefix = nil }
            return AgentOp(kind: .grep(GrepParams(pattern: pattern, pathPrefix: pathPrefix)))
        }
        return nil
    }

    private func parseListDir(from line: String) -> AgentOp? {
        // Example: "listDir: src/" or "List src/"
        if let path = extractPath(from: line) {
            let normalized = normalizeWorkspacePath(path)
            return AgentOp(kind: .listDir(ListDirParams(path: normalized, depth: 0)))
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

    private func normalizeWorkspacePath(_ p: String) -> String {
        // Delegate to PathUtils for a single source of truth (handles aliases like /workspace, /<name>, and placeholders like /path/to)
        return PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: p)
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

    // Token estimation moved to PlanningReducer.estimateTokenCount
}
