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

    /// Current plan
    private var currentPlan: ExplorePlan?
    /// Current ACP plan (for status updates)
    private var currentACPPlan: ACPPlan?
    /// Map op_id -> entry index for fast status updates
    private var planIndexByOpId: [String: Int] = [:]

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
        // toolExecutor is lazy and will be created on first use
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
        try? session.prewarm(promptPrefix: nil)

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
        let estimatedTokens = estimateTokenCount(instructions: instructions, prompt: prompt)
        guard estimatedTokens < 3500 else {
            throw OrchestrationError.executionFailed("Prompt too large: \(estimatedTokens) tokens (limit: 4096). Truncate goals or workspace path.")
        }

        print("[Orchestrator] Estimated tokens: \(estimatedTokens)")
        let instrChars = String(describing: instructions).count
        print("[FM] preparing request: instructions=\(instrChars) chars, prompt=\(prompt.count) chars")

        do {
            let options = GenerationOptions(temperature: 0.5)
            let t0 = Date()
            let response = try await session.respond(to: prompt, options: options)
            print("[FM] response received in \(String(format: "%.2f", Date().timeIntervalSince(t0)))s")
            let raw = response.content
            let ops = try parseOperationsFromResponse(raw)

            print("[Orchestrator] Generated plan with \(ops.count) operations")

            guard !ops.isEmpty else {
                throw OrchestrationError.executionFailed("FM generated empty plan")
            }

            // Post-process: add sessionAnalyze if we have session operations
            // This reads actual conversation content for deeper analysis
            let finalOps = addAnalysisIfNeeded(ops)

            return ExplorePlan(
                goals: goals,
                nextOps: finalOps
            )
        } catch {
            print("[Orchestrator] Error generating plan: \(error)")
            throw OrchestrationError.executionFailed("Failed to generate exploration plan: \(error.localizedDescription)")
        }
    }

    /// Add sessionAnalyze if plan contains session operations
    /// This ensures we actually read conversation content, not just list titles
    private func addAnalysisIfNeeded(_ ops: [AgentOp]) -> [AgentOp] {
        let hasSessionOps = ops.contains { op in
            switch op.kind {
            case .sessionList, .sessionSearch:
                return true
            default:
                return false
            }
        }

        guard hasSessionOps else {
            return ops // No session operations, return as-is
        }

        // Add sessionAnalyze to read and analyze conversation content
        var expanded = ops
        expanded.append(AgentOp(kind: .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: nil))))
        return expanded
    }

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
            print("[Orchestrator] Unknown operation type: \(type)")
            return nil
        }
    }
    #endif

    /// Parse operations from FM response
    private func parseOperationsFromResponse(_ response: String) throws -> [AgentOp] {
        var ops: [AgentOp] = []

        // Extract content from response description
        let content = extractContent(from: response) ?? response

        print("[Orchestrator] Parsing FM response (first 500 chars): \(String(content.prefix(500)))")

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
        print("[Orchestrator] Executing: \(op.description) [tool=\(op.toolName) id=\(op.opId)]")

        // Stream tool call (started)
        await streamToolCall(op, status: .started)

        do {
            // Execute via ToolExecutor
            let result = try await toolExecutor.execute(op)

            // Store result for summary generation
            operationResults[op] = result

            // Stream tool call update (completed)
            await streamToolCallUpdate(op, status: .completed, output: result)
            print("[Orchestrator] Completed: \(op.toolName) [id=\(op.opId)]")
        } catch {
            print("[Orchestrator] Operation failed: \(error)")

            // Stream tool call update (error)
            await streamToolCallUpdate(op, status: .error, error: error.localizedDescription)

            throw error
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

            for (provider, sessions) in allSessions {
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

    private func streamPlan(_ plan: ExplorePlan) async {
        let acpPlan = plan.toACPPlan()
        // Build index map for entry updates
        planIndexByOpId.removeAll()
        for (i, e) in acpPlan.entries.enumerated() {
            if let meta = e._meta, let any = meta["op_id"], case let .string(opId) = any.toJSONValue() {
                planIndexByOpId[opId] = i
            }
        }
        currentACPPlan = acpPlan
        let update = ACP.Client.SessionUpdate.plan(acpPlan)
        await streamHandler(update)
    }

    private func updatePlanEntry(opId: String, to status: ACPPlanEntryStatus, error: String? = nil) async {
        guard var plan = currentACPPlan, let idx = planIndexByOpId[opId], idx < plan.entries.count else { return }
        var entry = plan.entries[idx]
        entry.status = status
        if var meta = entry._meta {
            if let err = error { meta["error"] = AnyEncodable(err) }
            entry._meta = meta
        } else if let err = error {
            entry._meta = ["error": AnyEncodable(err)]
        }
        plan.entries[idx] = entry
        currentACPPlan = plan
        await streamHandler(.plan(plan))
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

        // Also emit a started update for clearer progress in UI
        if status == .started {
            switch op.kind {
            case .sessionAnalyze, .grep, .sessionSearch:
                let started = ACPToolCallUpdateWire(
                    call_id: op.opId.uuidString,
                    status: .started,
                    output: nil,
                    error: nil,
                    _meta: ["progress": AnyEncodable(0.0)]
                )
                let startedUpdate = ACP.Client.SessionUpdate.toolCallUpdate(started)
                await streamHandler(startedUpdate)
            default:
                break
            }
        }
        // Update plan entry to in_progress when a tool starts
        await updatePlanEntry(opId: op.opId.uuidString, to: .in_progress)
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
        // Mark completed (or completed with error) in plan
        if status == .completed {
            await updatePlanEntry(opId: op.opId.uuidString, to: .completed)
        } else if status == .error {
            await updatePlanEntry(opId: op.opId.uuidString, to: .completed, error: error)
        }
    }

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
                    return "\(head), and \(maxItems.last!)"
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
        print("[FM] analysis request: instructions=\(String(describing: instructions).count) chars, prompt=\(prompt.count) chars, ctxBytes=\(ctxData.count)")
        do {
            let t0 = Date()
            let resp = try await session.respond(to: prompt)
            let dt = Date().timeIntervalSince(t0)
            // LanguageModelSession.Response<String> exposes `content` (not `text`)
            print("[FM] analysis response in \(String(format: "%.2f", dt))s, text=\(resp.content.count) chars")
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
            print("[FM] analysis error: \(error)")
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
        print("[Orchestrator] Progress: \(op.toolName) \(Int(fraction * 100))% \(note ?? "")")
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
        var path = p.trimmingCharacters(in: .whitespacesAndNewlines)
        let name = (workspaceRoot as NSString).lastPathComponent
        let rootStd = (workspaceRoot as NSString).standardizingPath

        // Common aliases → workspace root
        if path == "." || path == "/" || path.lowercased() == "workspace" || path == "/workspace" {
            return "."
        }

        // Strip "/workspace/" prefix if present
        if path.hasPrefix("/workspace/") {
            path.removeFirst("/workspace/".count)
        }

        // Absolute path inside workspace → make relative
        if path.hasPrefix("/") {
            let std = (path as NSString).standardizingPath
            if std.hasPrefix(rootStd) {
                var rel = String(std.dropFirst(rootStd.count))
                if rel.hasPrefix("/") { rel.removeFirst() }
                path = rel
            } else {
                // Handle "/<workspaceName>[/...]" shorthand
                let nameWithSlash = "/" + name
                if std == nameWithSlash {
                    path = "."
                } else if std.hasPrefix(nameWithSlash + "/") {
                    var rel = String(std.dropFirst(nameWithSlash.count + 1))
                    if rel.hasPrefix("/") { rel.removeFirst() }
                    path = rel
                }
            }
        }

        // Remove leading workspace name to avoid duplication (e.g., "openagents/..." → "...")
        if path == name { return "." }
        if path.hasPrefix(name + "/") { path.removeFirst(name.count + 1) }

        // Clean leading/trailing tokens
        if path.hasPrefix("./") { path.removeFirst(2) }
        if path.hasSuffix("/") { path.removeLast() }
        return path.isEmpty ? "." : path
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

    // MARK: - Token Estimation

    /// Estimate token count for Foundation Models context validation
    /// Foundation Models have a 4096 token context window limit
    private func estimateTokenCount(instructions: Instructions, prompt: String) -> Int {
        // Conservative estimate: ~4 characters per token (OpenAI rule of thumb)
        // This is approximate but provides a safe upper bound
        let instructionsText = String(describing: instructions)
        let totalChars = instructionsText.count + prompt.count
        return totalChars / 4
    }
}
