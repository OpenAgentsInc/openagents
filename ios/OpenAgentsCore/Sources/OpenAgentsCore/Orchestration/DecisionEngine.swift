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

    /// Decide the next task to execute based on session insights AND orchestration config
    ///
    /// This method applies config-based bias to the heuristic decision logic:
    /// - Goals keywords influence path selection (refactor vs tests)
    /// - Agent preferences filter and override agent selection
    /// - Time budget affects estimated duration
    ///
    /// Bias rules (per review feedback):
    /// 1. If goals contain ["refactor", "cleanup", "restructure", "error handling"], prefer refactor path (+0.1 confidence)
    /// 2. If goals contain ["test", "coverage"], prefer tests path (+0.1 confidence)
    /// 3. If both present, use agentPreferences.prefer to break ties; else fall back to file frequency
    /// 4. Agent selection filtered by agentPreferences.allow (if non-empty)
    ///
    /// - Parameters:
    ///   - insights: Session analysis result (file frequency, tool patterns, user intent)
    ///   - config: Orchestration configuration with goals, agent preferences, time budget
    /// - Returns: Decision output with task, agent mode, priority, duration, rationale, confidence
    public func decideNextTask(
        from insights: SessionAnalyzeResult,
        config: OrchestrationConfig
    ) async throws -> DecisionOutput {
        // 1. Analyze goals for keywords
        let goalsLower = config.goals.map { $0.lowercased() }
        let refactorKeywords = ["refactor", "cleanup", "restructure", "error handling"]
        let testsKeywords = ["test", "coverage"]

        let prefersRefactor = goalsLower.contains(where: { goal in
            refactorKeywords.contains(where: { goal.contains($0) })
        })
        let prefersTests = goalsLower.contains(where: { goal in
            testsKeywords.contains(where: { goal.contains($0) })
        })

        // 2. Extract user intent and file frequency (same as original heuristic)
        let userIntent = insights.userIntent ?? "Unknown"
        let goals = insights.goalPatterns ?? []
        let fileFreq = insights.fileFrequency ?? [:]
        let sortedFiles = fileFreq.sorted { $0.value > $1.value }

        // 3. Clamp time budget
        let clampedBudget = max(1800, min(config.timeBudgetSec, 7200))

        // 4. Determine base path (refactor vs tests) with config bias
        var baseDecision: (path: String, baseAgentMode: ACPSessionModeId, baseConfidence: Double)?

        // REFACTOR PATH: Top file touched >20× AND (user mentioned refactor OR config goals prefer refactor)
        if let topFile = sortedFiles.first, topFile.value > 20 {
            let userMentionsRefactor = userIntent.lowercased().contains("refactor") || goals.contains("refactor")
            if userMentionsRefactor || prefersRefactor {
                let fileName = URL(fileURLWithPath: topFile.key).lastPathComponent
                var confidence = min(0.6 + (Double(topFile.value) / 100.0), 0.95)

                // Apply config bias
                if prefersRefactor {
                    confidence = min(confidence + 0.1, 1.0)
                }

                baseDecision = (
                    path: "refactor",
                    baseAgentMode: .claude_code,
                    baseConfidence: confidence
                )
            }
        }

        // TESTS PATH: No strong refactor signal OR config prefers tests
        if baseDecision == nil || prefersTests {
            // If both refactor and tests preferred, use agent preference to break tie
            if prefersRefactor && prefersTests {
                // Both preferred - use agent preference or file frequency
                if let prefer = config.agentPreferences.effectivePrefer() {
                    // Use agent preference to decide
                    if prefer == .claude_code {
                        // Keep refactor if we have one
                        if baseDecision == nil {
                            baseDecision = ("refactor", .claude_code, 0.75)
                        }
                    } else {
                        // Override with tests
                        baseDecision = ("tests", .codex, 0.75)
                    }
                } else {
                    // No preference, use file frequency (keep refactor if we have it)
                    if baseDecision == nil {
                        baseDecision = ("tests", .codex, 0.75)
                    }
                }
            } else if prefersTests || baseDecision == nil {
                // Only tests preferred, or no preference at all
                var confidence: Double = 0.75
                if prefersTests {
                    confidence = min(confidence + 0.1, 1.0)
                }
                baseDecision = ("tests", .codex, confidence)
            }
        }

        // 5. Apply agent preferences
        guard let decision = baseDecision else {
            // Fallback path
            let agentMode = selectAgentMode(.claude_code, config: config)
            return DecisionOutput(
                task: "Explore the codebase and identify areas for improvement. Goals: \(config.goals.joined(separator: ", "))",
                agentMode: agentMode,
                priority: .low,
                estimatedDuration: TimeInterval(clampedBudget) * 0.5,
                rationale: """
                No strong file frequency signals detected. Defaulting to exploratory analysis. \
                User goals: \(config.goals.joined(separator: ", ")).
                """,
                confidence: 0.4,
                metadata: [
                    "decision_path": "fallback_exploration",
                    "config_id": config.id,
                    "goals_hash": config.goalsHash()
                ]
            )
        }

        // 6. Build final decision with agent preference override
        let selectedAgentMode = selectAgentMode(decision.baseAgentMode, config: config)

        if decision.path == "refactor" {
            guard let topFile = sortedFiles.first else {
                // Should not happen, but handle gracefully
                let agentMode = selectAgentMode(.claude_code, config: config)
                return DecisionOutput(
                    task: "Refactor code to improve quality based on goals: \(config.goals.joined(separator: ", "))",
                    agentMode: agentMode,
                    priority: .high,
                    estimatedDuration: TimeInterval(clampedBudget),
                    rationale: "Config goals indicate refactor focus: \(config.goals.joined(separator: ", "))",
                    confidence: decision.baseConfidence,
                    metadata: [
                        "decision_path": "refactor",
                        "config_id": config.id,
                        "goals_hash": config.goalsHash()
                    ]
                )
            }

            let fileName = URL(fileURLWithPath: topFile.key).lastPathComponent

            var rationale = """
            \(fileName) has been modified \(topFile.value) times recently. Refactoring will improve \
            code quality and maintainability.
            """
            if !config.goals.isEmpty {
                rationale += " User goals: \(config.goals.joined(separator: ", ")). "
            }
            rationale += "Agent: \(selectedAgentMode.rawValue) selected based on config preferences."

            return DecisionOutput(
                task: "Refactor \(fileName) to improve error handling, reduce complexity, and enhance readability",
                agentMode: selectedAgentMode,
                priority: .high,
                estimatedDuration: TimeInterval(clampedBudget),
                rationale: rationale,
                confidence: decision.baseConfidence,
                metadata: [
                    "top_file": topFile.key,
                    "file_touches": "\(topFile.value)",
                    "decision_path": "refactor",
                    "config_id": config.id,
                    "goals_hash": config.goalsHash()
                ]
            )
        } else {
            // TESTS PATH
            let fileName = sortedFiles.first.map { URL(fileURLWithPath: $0.key).lastPathComponent } ?? "codebase"

            var rationale = ""
            if let topFile = sortedFiles.first {
                rationale = "\(fileName) has been modified \(topFile.value) times recently. "
            }
            rationale += "Generating tests will improve code coverage and reliability. "
            if !config.goals.isEmpty {
                rationale += "User goals: \(config.goals.joined(separator: ", ")). "
            }
            rationale += "Agent: \(selectedAgentMode.rawValue) selected based on config preferences."

            return DecisionOutput(
                task: "Generate comprehensive tests for \(fileName) covering all public methods and edge cases",
                agentMode: selectedAgentMode,
                priority: .medium,
                estimatedDuration: TimeInterval(clampedBudget) * 0.7,
                rationale: rationale,
                confidence: decision.baseConfidence,
                metadata: sortedFiles.first.map { [
                    "top_file": $0.key,
                    "file_touches": "\($0.value)",
                    "decision_path": "tests",
                    "config_id": config.id,
                    "goals_hash": config.goalsHash()
                ] } ?? [
                    "decision_path": "tests",
                    "config_id": config.id,
                    "goals_hash": config.goalsHash()
                ]
            )
        }
    }

    // MARK: - Private Helpers

    /// Select agent mode based on config preferences
    private func selectAgentMode(_ defaultMode: ACPSessionModeId, config: OrchestrationConfig) -> ACPSessionModeId {
        // Get effective prefer (nil if not in allow list)
        if let preferred = config.agentPreferences.effectivePrefer() {
            return preferred
        }

        // Check if default mode is in allow list
        let allowList = config.agentPreferences.allow
        if allowList.isEmpty {
            // No restrictions, use default
            return defaultMode
        }

        // If default is in allow list, use it
        if allowList.contains(defaultMode) {
            return defaultMode
        }

        // Default not allowed, use first allowed agent
        return allowList.first ?? defaultMode
    }
}
