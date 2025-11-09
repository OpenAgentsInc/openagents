import XCTest
@testable import OpenAgentsCore

final class DecisionEngineTests: XCTestCase {

    func testDecideRefactor_whenTopFileFrequentAndUserMentionsRefactor() async throws {
        // Given: High file frequency + "refactor" in user intent
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: [
                "BridgeManager.swift": 25,
                "AgentProvider.swift": 10
            ],
            toolFrequency: ["edit_file": 30],
            goalPatterns: ["refactor", "improve"],
            avgConversationLength: 15.0,
            userIntent: "refactor error handling in BridgeManager"
        )

        // When: Decide next task
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600  // 1 hour
        )

        // Then: Should choose refactor task
        XCTAssertTrue(decision.task.contains("Refactor"), "Task should be a refactor")
        XCTAssertTrue(decision.task.contains("BridgeManager"), "Should mention top file")
        XCTAssertEqual(decision.agentMode, .claude_code, "Claude Code excels at refactoring")
        XCTAssertEqual(decision.priority, .high, "Refactor with strong signal is high priority")
        XCTAssertGreaterThanOrEqual(decision.confidence, 0.6, "Should have good confidence")
        XCTAssertLessThanOrEqual(decision.confidence, 1.0, "Confidence must be <= 1.0")
        XCTAssertEqual(decision.metadata?["decision_path"], "refactor")
    }

    func testDecideTests_whenNoStrongRefactorSignal() async throws {
        // Given: File frequency without "refactor" intent
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: [
                "SessionUpdateHub.swift": 15
            ],
            toolFrequency: ["edit_file": 20],
            goalPatterns: ["improve", "fix"],
            avgConversationLength: 12.0,
            userIntent: "improve session handling"
        )

        // When: Decide next task
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600
        )

        // Then: Should choose test generation
        XCTAssertTrue(decision.task.contains("tests") || decision.task.contains("test"), "Should generate tests")
        XCTAssertTrue(decision.task.contains("SessionUpdateHub"), "Should mention the file")
        XCTAssertEqual(decision.agentMode, .codex, "Codex excels at test generation")
        XCTAssertEqual(decision.priority, .medium, "Tests are medium priority")
        XCTAssertEqual(decision.metadata?["decision_path"], "tests")
    }

    func testTimeBudgetClamping_minimum() async throws {
        // Given: Time budget below 30 minutes
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 5],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: nil
        )

        // When: Decide with 10 minute budget
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 600  // 10 minutes
        )

        // Then: Should clamp to 30 minutes minimum
        XCTAssertGreaterThanOrEqual(decision.estimatedDuration, 1800, "Should clamp to 30 min minimum")
    }

    func testTimeBudgetClamping_maximum() async throws {
        // Given: Time budget above 2 hours
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 30],
            toolFrequency: [:],
            goalPatterns: ["refactor"],
            avgConversationLength: 20.0,
            userIntent: "refactor everything"
        )

        // When: Decide with 5 hour budget
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 18000  // 5 hours
        )

        // Then: Should clamp to 2 hours maximum
        XCTAssertLessThanOrEqual(decision.estimatedDuration, 7200, "Should clamp to 2h maximum")
    }

    func testConfidenceBounds_neverExceedsOne() async throws {
        // Given: Very high file frequency (would calculate >1.0 confidence)
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: [
                "VeryActiveFile.swift": 150  // Would give confidence >1.0 in formula
            ],
            toolFrequency: [:],
            goalPatterns: ["refactor"],
            avgConversationLength: 30.0,
            userIntent: "refactor VeryActiveFile"
        )

        // When: Decide
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600
        )

        // Then: Confidence should be clamped to 1.0
        XCTAssertLessThanOrEqual(decision.confidence, 1.0, "Confidence must never exceed 1.0")
        XCTAssertGreaterThanOrEqual(decision.confidence, 0.0, "Confidence must never be negative")
    }

    func testFallbackExploration_whenNoFileFrequency() async throws {
        // Given: Empty insights
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: [:],  // No files
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 5.0,
            userIntent: "general improvements"
        )

        // When: Decide
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600
        )

        // Then: Should fallback to exploration
        XCTAssertTrue(decision.task.contains("Explore") || decision.task.contains("explore"), "Should be exploration task")
        XCTAssertEqual(decision.agentMode, .claude_code, "Claude Code for general exploration")
        XCTAssertEqual(decision.priority, .low, "Fallback is low priority")
        XCTAssertLessThanOrEqual(decision.confidence, 0.5, "Low confidence for fallback")
        XCTAssertEqual(decision.metadata?["decision_path"], "fallback_exploration")
    }

    func testRationaleIncludesFileAndUserIntent() async throws {
        // Given: Specific insights
        let engine = DecisionEngine()
        let userIntent = "improve error handling and add better logging"
        let insights = SessionAnalyzeResult(
            fileFrequency: ["ErrorHandler.swift": 22],
            toolFrequency: [:],
            goalPatterns: ["refactor"],
            avgConversationLength: 12.0,
            userIntent: userIntent
        )

        // When: Decide
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600
        )

        // Then: Rationale should mention both file and user intent
        XCTAssertTrue(decision.rationale.contains("ErrorHandler"), "Rationale should mention the file")
        XCTAssertTrue(decision.rationale.contains("22"), "Rationale should mention frequency count")
    }

    func testMetadataIncludesTopFile() async throws {
        // Given: Insights with top file
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: ["TopFile.swift": 30],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: nil
        )

        // When: Decide
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600
        )

        // Then: Metadata should include top file info
        XCTAssertNotNil(decision.metadata?["top_file"], "Should have top_file in metadata")
        XCTAssertNotNil(decision.metadata?["file_touches"], "Should have file_touches in metadata")
        XCTAssertNotNil(decision.metadata?["decision_path"], "Should have decision_path in metadata")
    }

    func testAgentSelection_claudeCodeForRefactoring() async throws {
        // Given: Refactor signal
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: ["Code.swift": 25],
            toolFrequency: [:],
            goalPatterns: ["refactor"],
            avgConversationLength: 15.0,
            userIntent: "refactor this code"
        )

        // When: Decide
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600
        )

        // Then: Should select Claude Code
        XCTAssertEqual(decision.agentMode, .claude_code, "Claude Code for refactoring tasks")
    }

    func testAgentSelection_codexForTests() async throws {
        // Given: No refactor signal, defaults to tests
        let engine = DecisionEngine()
        let insights = SessionAnalyzeResult(
            fileFrequency: ["Service.swift": 15],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: "improve code quality"
        )

        // When: Decide
        let decision = try await engine.decideNextTask(
            from: insights,
            timeBudgetSeconds: 3600
        )

        // Then: Should select Codex
        XCTAssertEqual(decision.agentMode, .codex, "Codex for test generation")
    }

    // MARK: - Config-Aware Decision Tests

    func testConfigBias_refactorKeywordsIncreaseConfidence() async throws {
        // Given: Config with refactor goal keyword + insights with refactor signal
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["refactor error handling in BridgeManager"]
        config.timeBudgetSec = 1800

        let insights = SessionAnalyzeResult(
            fileFrequency: ["BridgeManager.swift": 25],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: "improve code"
        )

        // When: Decide with config
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Should choose refactor with boosted confidence
        XCTAssertEqual(decision.metadata?["decision_path"], "refactor")
        XCTAssertGreaterThanOrEqual(decision.confidence, 0.7, "Confidence should be boosted by config bias")
        XCTAssertEqual(decision.metadata?["config_id"], config.id)
        XCTAssertEqual(decision.metadata?["goals_hash"], config.goalsHash())
    }

    func testConfigBias_testsKeywordsIncreaseConfidence() async throws {
        // Given: Config with test keyword
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["increase test coverage"]
        config.timeBudgetSec = 1800

        let insights = SessionAnalyzeResult(
            fileFrequency: ["Service.swift": 15],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: nil
        )

        // When: Decide with config
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Should choose tests with boosted confidence
        XCTAssertEqual(decision.metadata?["decision_path"], "tests")
        XCTAssertGreaterThanOrEqual(decision.confidence, 0.8, "Tests bias should boost confidence")
        XCTAssertEqual(decision.metadata?["config_id"], config.id)
    }

    func testConfigAgentPreference_preferIsRespected() async throws {
        // Given: Config with prefer = codex
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["refactor code"]
        config.agentPreferences.prefer = .codex
        config.agentPreferences.allow = [.codex, .claude_code]
        config.timeBudgetSec = 1800

        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 25],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: "refactor"
        )

        // When: Decide with config
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Should use preferred agent
        XCTAssertEqual(decision.agentMode, .codex, "Should respect prefer setting")
    }

    func testConfigAgentPreference_allowListIsRespected() async throws {
        // Given: Config with only Codex in allow list
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["refactor code"]
        config.agentPreferences.allow = [.codex]  // Only Codex allowed
        config.timeBudgetSec = 1800

        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 25],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: "refactor"
        )

        // When: Decide with config (normally would choose Claude Code for refactor)
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Should use allowed agent
        XCTAssertEqual(decision.agentMode, .codex, "Should respect allow list restriction")
    }

    func testConfigAgentPreference_preferNotInAllowIsIgnored() async throws {
        // Given: Config with prefer not in allow list
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["generate tests"]
        config.agentPreferences.prefer = .claude_code
        config.agentPreferences.allow = [.codex]  // Claude Code not allowed
        config.timeBudgetSec = 1800

        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 15],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: nil
        )

        // When: Decide with config
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Should ignore invalid prefer and use allowed agent
        XCTAssertEqual(decision.agentMode, .codex, "Should ignore prefer if not in allow list")
    }

    func testConfigBias_bothRefactorAndTests_prefersRefactor() async throws {
        // Given: Config with both refactor and test keywords, prefer Claude Code
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["refactor error handling", "add test coverage"]
        config.agentPreferences.prefer = .claude_code
        config.agentPreferences.allow = [.claude_code, .codex]
        config.timeBudgetSec = 1800

        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 25],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: "improve code"
        )

        // When: Decide with config
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Should choose refactor (file frequency >20 + prefer Claude Code)
        XCTAssertEqual(decision.metadata?["decision_path"], "refactor")
        XCTAssertEqual(decision.agentMode, .claude_code)
    }

    func testConfigMetadata_includesConfigIdAndGoalsHash() async throws {
        // Given: Config with goals
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.id = "test-config-123"
        config.goals = ["refactor", "add tests"]
        config.timeBudgetSec = 1800

        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 15],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: nil
        )

        // When: Decide with config
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Metadata should include config_id and goals_hash
        XCTAssertEqual(decision.metadata?["config_id"], "test-config-123")
        XCTAssertEqual(decision.metadata?["goals_hash"], config.goalsHash())
        XCTAssertNotNil(decision.metadata?["decision_path"])
    }

    func testConfigTimeBudget_usedForEstimatedDuration() async throws {
        // Given: Config with specific time budget
        let engine = DecisionEngine()
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["refactor code"]
        config.timeBudgetSec = 3600  // 1 hour

        let insights = SessionAnalyzeResult(
            fileFrequency: ["File.swift": 25],
            toolFrequency: [:],
            goalPatterns: [],
            avgConversationLength: 10.0,
            userIntent: "refactor"
        )

        // When: Decide with config
        let decision = try await engine.decideNextTask(from: insights, config: config)

        // Then: Should use config time budget (clamped to valid range)
        XCTAssertEqual(decision.estimatedDuration, TimeInterval(config.timeBudgetSec))
    }
}
