import XCTest
@testable import OpenAgentsCore

#if os(macOS)

final class AgentCoordinatorTests: XCTestCase {

    // MARK: - Helpers

    private func createTestDB() throws -> TinyvexDbLayer {
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("coord_\(UUID().uuidString).sqlite")
        return try TinyvexDbLayer(path: tmp.path)
    }

    private func createTestDecision() -> DecisionOutput {
        return DecisionOutput(
            task: "Test task",
            agentMode: .claude_code,
            priority: .medium,
            estimatedDuration: 1800,
            rationale: "Test rationale",
            confidence: 0.8
        )
    }

    // MARK: - Initialization Tests

    func testInit() async throws {
        // Given: Dependencies
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        // When: Create coordinator
        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        // Then: Should initialize
        XCTAssertNotNil(coordinator)
    }

    // MARK: - Cycle Tests

    func testRunCycle_noAgentsAvailable_skipsDecision() async throws {
        // Given: Coordinator with no agents registered
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()
        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        // When: Run one cycle
        let result = await coordinator.runCycle(
            timeBudgetSeconds: 1800,
            workingDirectory: nil
        )

        // Then: Should return .noAgentsAvailable
        guard case .noAgentsAvailable = result else {
            XCTFail("Expected .noAgentsAvailable, got \(result)")
            return
        }
    }

    func testRunCycle_noTasksInQueue_makesDecisionAndEnqueues() async throws {
        // Given: Coordinator with mock agent but no tasks in queue
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        // Register mock agent
        let mockProvider = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true
        )
        await agentRegistry.register(mockProvider)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        // When: Run one cycle
        let result = await coordinator.runCycle(
            timeBudgetSeconds: 1800,
            workingDirectory: nil
        )

        // Then: Should make decision and enqueue
        guard case .decisionMade(let taskId) = result else {
            XCTFail("Expected .decisionMade, got \(result)")
            return
        }

        // Verify task was enqueued
        let task = try await taskQueue.get(taskId)
        XCTAssertNotNil(task, "Task should be in queue")
        XCTAssertEqual(task?.status, .pending)
    }

    func testRunCycle_pendingTaskExists_executesTask() async throws {
        // Given: Queue with pending task
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        let mockProvider = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true
        )
        await agentRegistry.register(mockProvider)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        // Enqueue a pending task manually
        let decision = createTestDecision()
        let task = OvernightTask(opHash: "test-hash", decision: decision)
        let taskId = try await taskQueue.enqueue(task)

        // When: Run cycle
        let result = await coordinator.runCycle(
            timeBudgetSeconds: 1800,
            workingDirectory: nil
        )

        // Then: Should execute task
        guard case .taskExecuted(let executedId, _) = result else {
            XCTFail("Expected .taskExecuted, got \(result)")
            return
        }

        XCTAssertEqual(executedId, taskId)

        // Verify task status changed to in_progress
        let updatedTask = try await taskQueue.get(taskId)
        XCTAssertEqual(updatedTask?.status, .in_progress)
        XCTAssertNotNil(updatedTask?.sessionId, "Session ID should be set")
    }

    func testRunCycle_taskExecutionFails_marksTaskAsFailed() async throws {
        // Given: Queue with pending task and mock provider that fails
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        let mockProvider = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true,
            shouldFailStart: true
        )
        await agentRegistry.register(mockProvider)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        let decision = createTestDecision()
        let task = OvernightTask(opHash: "test-hash", decision: decision)
        let taskId = try await taskQueue.enqueue(task)

        // When: Run cycle
        let result = await coordinator.runCycle(
            timeBudgetSeconds: 1800,
            workingDirectory: nil
        )

        // Then: Should report task failed
        guard case .taskFailed(let failedId, let error) = result else {
            XCTFail("Expected .taskFailed, got \(result)")
            return
        }

        XCTAssertEqual(failedId, taskId)
        XCTAssertNotNil(error)

        // Verify task status changed to failed
        let updatedTask = try await taskQueue.get(taskId)
        XCTAssertEqual(updatedTask?.status, .failed)
        XCTAssertNotNil(updatedTask?.error)
    }

    // MARK: - Task Completion Tests

    func testCompleteTask_success() async throws {
        // Given: Task in queue with in_progress status
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()
        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        let decision = createTestDecision()
        var task = OvernightTask(opHash: "test-hash", decision: decision)
        task.status = .in_progress
        task.sessionId = "test-session-123"
        let taskId = try await taskQueue.enqueue(task)

        // When: Complete the task successfully
        try await coordinator.completeTask(taskId, success: true)

        // Then: Task should be marked completed
        let updatedTask = try await taskQueue.get(taskId)
        XCTAssertEqual(updatedTask?.status, .completed)
        XCTAssertNotNil(updatedTask?.completedAt)
    }

    func testCompleteTask_failure() async throws {
        // Given: Task in queue with in_progress status
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()
        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        let decision = createTestDecision()
        var task = OvernightTask(opHash: "test-hash", decision: decision)
        task.status = .in_progress
        task.sessionId = "test-session-123"
        let taskId = try await taskQueue.enqueue(task)

        // When: Complete the task with failure
        try await coordinator.completeTask(taskId, success: false, error: "Build failed")

        // Then: Task should be marked failed with error
        let updatedTask = try await taskQueue.get(taskId)
        XCTAssertEqual(updatedTask?.status, .failed)
        XCTAssertNotNil(updatedTask?.completedAt)
        XCTAssertEqual(updatedTask?.error, "Build failed")
    }

    // MARK: - Cancel Tests

    func testCancelTask() async throws {
        // Given: Task in queue with in_progress status
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        let mockProvider = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true
        )
        await agentRegistry.register(mockProvider)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        let decision = createTestDecision()
        var task = OvernightTask(opHash: "test-hash", decision: decision)
        task.status = .in_progress
        task.sessionId = "test-session-123"
        let taskId = try await taskQueue.enqueue(task)

        // When: Cancel the task
        try await coordinator.cancelTask(taskId)

        // Then: Task should be marked cancelled
        let updatedTask = try await taskQueue.get(taskId)
        XCTAssertEqual(updatedTask?.status, .cancelled)
    }

    // MARK: - Config-Aware Cycle Tests

    func testRunCycleWithConfig_makesDecisionWithConfig() async throws {
        // Given: Coordinator with mock agent and config with goals
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        let mockProvider = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true
        )
        await agentRegistry.register(mockProvider)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["refactor error handling"]
        config.timeBudgetSec = 1800

        // When: Run cycle with config
        let result = await coordinator.runCycle(config: config, workingDirectory: nil)

        // Then: Should make decision and enqueue
        guard case .decisionMade(let taskId) = result else {
            XCTFail("Expected .decisionMade, got \(result)")
            return
        }

        // Verify task was enqueued with config metadata
        let task = try await taskQueue.get(taskId)
        XCTAssertNotNil(task)
        XCTAssertEqual(task?.metadata["config_id"], config.id)
        XCTAssertEqual(task?.metadata["goals_hash"], config.goalsHash())
    }

    func testRunCycleWithConfig_executesTaskWithConfigTimeout() async throws {
        // Given: Queue with pending task and config with specific time budget
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        let mockProvider = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true
        )
        await agentRegistry.register(mockProvider)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.timeBudgetSec = 3600  // 1 hour

        // Enqueue a pending task manually
        let decision = createTestDecision()
        let task = OvernightTask(opHash: "test-hash", decision: decision)
        let taskId = try await taskQueue.enqueue(task)

        // When: Run cycle with config
        let result = await coordinator.runCycle(config: config, workingDirectory: nil)

        // Then: Should execute task (timeout uses config.timeBudgetSec)
        guard case .taskExecuted(let executedId, _) = result else {
            XCTFail("Expected .taskExecuted, got \(result)")
            return
        }

        XCTAssertEqual(executedId, taskId)

        // Verify task status changed to in_progress
        let updatedTask = try await taskQueue.get(taskId)
        XCTAssertEqual(updatedTask?.status, .in_progress)
    }

    func testRunCycleWithConfig_respectsAgentPreferences() async throws {
        // Given: Coordinator with both agents registered, config prefers Codex
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        let mockClaudeCode = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true
        )
        let mockCodex = MockAgentProvider(
            id: .codex,
            displayName: "Mock Codex",
            available: true
        )
        await agentRegistry.register(mockClaudeCode)
        await agentRegistry.register(mockCodex)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.goals = ["generate tests"]
        config.agentPreferences.prefer = .codex
        config.agentPreferences.allow = [.codex, .claude_code]
        config.timeBudgetSec = 1800

        // When: Run cycle with config
        let result = await coordinator.runCycle(config: config, workingDirectory: nil)

        // Then: Should make decision (agent preference applied in DecisionEngine)
        guard case .decisionMade(let taskId) = result else {
            XCTFail("Expected .decisionMade, got \(result)")
            return
        }

        // Verify task exists with config metadata
        let task = try await taskQueue.get(taskId)
        XCTAssertNotNil(task)
        XCTAssertEqual(task?.metadata["config_id"], config.id)
    }

    func testRunCycleWithConfig_storesConfigMetadata() async throws {
        // Given: Coordinator with config
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()

        let mockProvider = MockAgentProvider(
            id: .claude_code,
            displayName: "Mock Claude Code",
            available: true
        )
        await agentRegistry.register(mockProvider)

        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        var config = OrchestrationConfig.createDefault(workspaceRoot: "/test")
        config.id = "test-config-123"
        config.goals = ["refactor", "add tests"]
        config.timeBudgetSec = 1800

        // When: Run cycle with config
        let result = await coordinator.runCycle(config: config, workingDirectory: nil)

        // Then: Should store config_id and goals_hash in task metadata
        guard case .decisionMade(let taskId) = result else {
            XCTFail("Expected .decisionMade, got \(result)")
            return
        }

        let task = try await taskQueue.get(taskId)
        XCTAssertNotNil(task)
        XCTAssertEqual(task?.metadata["config_id"], "test-config-123")
        XCTAssertEqual(task?.metadata["goals_hash"], config.goalsHash())
    }

    // MARK: - Metrics Tests

    func testMetrics() async throws {
        // Given: Coordinator
        let db = try createTestDB()
        let taskQueue = try await TaskQueue(db: db)
        let decisionEngine = DecisionEngine()
        let agentRegistry = AgentRegistry()
        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry
        )

        // When: Get metrics
        let metrics = await coordinator.metrics()

        // Then: Should have initial metrics
        XCTAssertEqual(metrics.cyclesRun, 0)
        XCTAssertEqual(metrics.tasksExecuted, 0)
        XCTAssertEqual(metrics.tasksCompleted, 0)
        XCTAssertEqual(metrics.tasksFailed, 0)
    }
}

// MARK: - Mock Agent Provider

class MockAgentProvider: AgentProvider {
    let id: ACPSessionModeId
    let displayName: String
    let capabilities: AgentCapabilities
    let available: Bool
    let shouldFailStart: Bool

    init(
        id: ACPSessionModeId,
        displayName: String,
        available: Bool = true,
        shouldFailStart: Bool = false
    ) {
        self.id = id
        self.displayName = displayName
        self.available = available
        self.shouldFailStart = shouldFailStart
        self.capabilities = AgentCapabilities(
            executionMode: .native,
            streamingMode: .acp,
            supportsResume: true,
            supportsWorkingDirectory: true
        )
    }

    func isAvailable() async -> Bool {
        return available
    }

    func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        if shouldFailStart {
            throw AgentProviderError.startFailed("Mock start failure")
        }

        return AgentHandle(
            sessionId: sessionId,
            mode: id,
            isStarted: true
        )
    }

    func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws {
        // Mock implementation
    }

    func cancel(sessionId: ACPSessionId, handle: AgentHandle) async {
        // Mock implementation
    }
}

#endif
