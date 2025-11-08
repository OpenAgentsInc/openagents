import Foundation

// MARK: - Decision Output

/// The output of the decision engine, specifying what task to execute next
public struct DecisionOutput: Codable, Sendable {
    /// The task description/prompt to give to the agent
    public let task: String

    /// Which agent mode should handle this task
    public let agentMode: ACPSessionModeId

    /// Priority level for task scheduling
    public let priority: TaskPriority

    /// Estimated duration in seconds
    public let estimatedDuration: TimeInterval

    /// Human-readable explanation of why this task was chosen
    public let rationale: String

    /// Confidence score (0.0 - 1.0) in this decision
    public let confidence: Double

    /// Optional metadata for tracking/debugging
    public let metadata: [String: String]?

    public init(
        task: String,
        agentMode: ACPSessionModeId,
        priority: TaskPriority,
        estimatedDuration: TimeInterval,
        rationale: String,
        confidence: Double,
        metadata: [String: String]? = nil
    ) {
        self.task = task
        self.agentMode = agentMode
        self.priority = priority
        self.estimatedDuration = estimatedDuration
        self.rationale = rationale
        self.confidence = min(max(confidence, 0.0), 1.0) // Clamp to [0.0, 1.0]
        self.metadata = metadata
    }
}

// MARK: - Task Priority

public enum TaskPriority: String, Codable, Sendable, CaseIterable {
    case high
    case medium
    case low
}

// MARK: - Decision Engine

/// Actor responsible for analyzing session history and deciding next tasks
public actor DecisionEngine {

    /// Initialize the decision engine
    public init() {}

    /// Analyze recent sessions to extract insights
    ///
    /// This method delegates to SessionAnalyzeTool which aggregates:
    /// - File frequency (which files are touched most)
    /// - Tool usage patterns
    /// - User intent extraction
    /// - Goal patterns
    ///
    /// - Parameters:
    ///   - sessionIds: Optional list of session IDs to analyze. If nil, analyzes recent sessions.
    ///   - provider: Optional agent provider name to filter sessions
    ///   - maxSessions: Maximum number of sessions to analyze (default: 10)
    /// - Returns: Session analysis result with file frequency, tool patterns, and user intent
    /// - Throws: If analysis fails
    public func analyzeSessions(
        sessionIds: [String]? = nil,
        provider: String? = nil,
        maxSessions: Int = 10
    ) async throws -> SessionAnalyzeResult {
        let tool = SessionAnalyzeTool()

        // If no sessionIds provided, use empty array (SessionAnalyzeTool will fetch recent sessions)
        let ids = sessionIds ?? []
        let params = SessionAnalyzeParams(
            sessionIds: ids,
            provider: provider,
            metrics: nil  // nil = compute all metrics
        )

        return try await tool.analyze(
            sessionIds: params.sessionIds,
            provider: params.provider,
            metrics: params.metrics
        )
    }

    /// Decide the next task to execute based on session insights
    ///
    /// Implements the heuristic defined in docs/overnight/issues/002-decision-orchestrator.md:
    /// - Refactor: If top file touched >20 times AND user mentioned "refactor"
    /// - Tests: If no strong refactor signal
    /// - Claude Code: For refactoring and architectural work
    /// - Codex: For test generation and boilerplate
    ///
    /// - Parameters:
    ///   - insights: Analysis result from analyzeSessions()
    ///   - timeBudgetSeconds: Available time budget for the task (clamped to 30m-2h)
    /// - Returns: Decision output specifying task, agent, and confidence
    /// - Throws: If decision cannot be made
    public func decideNextTask(
        from insights: SessionAnalyzeResult,
        timeBudgetSeconds: TimeInterval
    ) async throws -> DecisionOutput {
        // Clamp time budget to reasonable range (30 minutes to 2 hours)
        let clampedBudget = min(max(timeBudgetSeconds, 1800), 7200)

        // Extract signals from insights
        let fileFreq = insights.fileFrequency ?? [:]
        let userIntent = insights.userIntent ?? ""
        let goals = insights.goalPatterns ?? []

        // Sort files by frequency
        let sortedFiles = fileFreq.sorted { $0.value > $1.value }

        // Decision heuristic (from docs/overnight/decision-logic.md)

        // REFACTOR PATH: Top file touched >20 times AND user mentioned "refactor"
        if let topFile = sortedFiles.first,
           topFile.value > 20,
           (userIntent.lowercased().contains("refactor") || goals.contains("refactor")) {

            let fileName = URL(fileURLWithPath: topFile.key).lastPathComponent
            let confidence = min(0.6 + (Double(topFile.value) / 100.0), 0.95)

            return DecisionOutput(
                task: "Refactor \(fileName) to improve code quality and maintainability based on recent changes",
                agentMode: .claude_code,  // Claude Code excels at refactoring
                priority: .high,
                estimatedDuration: clampedBudget,
                rationale: """
                \(fileName) has been modified \(topFile.value) times in recent sessions, indicating \
                active development. User intent suggests refactoring focus: "\(userIntent)". \
                Claude Code selected for architectural improvements.
                """,
                confidence: confidence,
                metadata: [
                    "top_file": topFile.key,
                    "file_touches": "\(topFile.value)",
                    "decision_path": "refactor"
                ]
            )
        }

        // TESTS PATH: No strong refactor signal, generate tests
        if let topFile = sortedFiles.first {
            let fileName = URL(fileURLWithPath: topFile.key).lastPathComponent
            let confidence = 0.75 // Medium confidence for test generation

            return DecisionOutput(
                task: "Generate comprehensive tests for \(fileName) covering all public methods and edge cases",
                agentMode: .codex,  // Codex excels at test generation
                priority: .medium,
                estimatedDuration: clampedBudget * 0.7,  // Tests typically faster
                rationale: """
                \(fileName) has been modified \(topFile.value) times recently but no strong refactor \
                signal detected. Generating tests will improve code coverage and reliability. \
                Codex selected for its test generation capabilities.
                """,
                confidence: confidence,
                metadata: [
                    "top_file": topFile.key,
                    "file_touches": "\(topFile.value)",
                    "decision_path": "tests"
                ]
            )
        }

        // FALLBACK: No clear signal, generic exploration
        return DecisionOutput(
            task: "Explore the codebase and identify areas for improvement based on user intent: \(userIntent)",
            agentMode: .claude_code,
            priority: .low,
            estimatedDuration: clampedBudget * 0.5,
            rationale: """
            No strong file frequency or user intent signals detected. Defaulting to exploratory \
            analysis. User intent: "\(userIntent)". Claude Code selected for general exploration.
            """,
            confidence: 0.4,  // Low confidence for fallback
            metadata: [
                "decision_path": "fallback_exploration"
            ]
        )
    }
}
