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
    @available(iOS 26.0, macOS 26.0, *)
    private var fm: NativeFMOrchestrator?
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
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *) {
            self.fm = NativeFMOrchestrator(workspaceRoot: workspaceRoot, goals: goals, stream: streamHandler)
        }
        #endif
    }

    /// Start exploration process
    public func startExploration() async throws -> ExploreSummary {
        #if canImport(FoundationModels)
        // Validate workspace
        try validateWorkspace()
        if policy.use_native_tool_calling {
            if #available(iOS 26.0, macOS 26.0, *) {
                guard let fm = fm else { throw OrchestrationError.modelUnavailable("FM orchestrator not available") }
                return try await fm.executeNativeToolCallingLoop()
            } else {
                throw OrchestrationError.modelUnavailable("Requires iOS/macOS 26+")
            }
        } else {
            // Legacy path: plan + execute
            let model = SystemLanguageModel.default
            // Generate initial plan
            let plan = try await generateInitialPlan(using: model)
            currentPlan = plan
            await planning.streamPlan(plan)
            try await executeOperations(plan.nextOps)
            return try await generateSummary()
        }
        #else
        // Foundation Models not available at compile time
        throw OrchestrationError.modelUnavailable("FoundationModels framework not available")
        #endif
    }

    // MARK: - Session Management

    #if canImport(FoundationModels)
    // Native FM loop is implemented in NativeFMOrchestrator and invoked via `fm` when available.

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
            let ops = PlanParsing.parseOperationsFromResponse(raw, workspaceRoot: workspaceRoot)

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
        return try PlanningReducer.parseOperationsFromResponse(response)
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
        return await SummaryBuilder.generate(workspaceRoot: workspaceRoot, operationResults: operationResults)
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
        if let r = await FMAnalysis.compute(workspaceRoot: workspaceRoot, operationResults: operationResults) {
            return FMAnalysisResult(text: r.text, source: r.source == .fm ? .fm : .sessionAnalyze, topFiles: r.topFiles, goalPatterns: r.goalPatterns, avgConversationLength: r.avgConversationLength)
        }
        return nil
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

    // Helper parsing moved to PlanParsing

    // Token estimation moved to PlanningReducer.estimateTokenCount
}
