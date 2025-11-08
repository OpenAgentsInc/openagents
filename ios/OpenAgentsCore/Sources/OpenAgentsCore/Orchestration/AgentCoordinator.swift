#if os(macOS)
import Foundation

// MARK: - Cycle Result

/// Result of running one orchestration cycle
public enum CycleResult: Sendable {
    /// No agents were available to execute tasks
    case noAgentsAvailable

    /// Decision was made and task enqueued
    case decisionMade(taskId: TaskID)

    /// Task was executed and is now in progress
    case taskExecuted(taskId: TaskID, sessionId: String)

    /// Task execution failed
    case taskFailed(taskId: TaskID, error: String)

    /// No pending tasks and no new decision needed
    case idle
}

// MARK: - Coordinator Metrics

/// Metrics tracked by the AgentCoordinator
public struct CoordinatorMetrics: Sendable {
    /// Number of orchestration cycles run
    public let cyclesRun: Int

    /// Number of tasks executed (moved to in_progress)
    public let tasksExecuted: Int

    /// Number of tasks completed successfully
    public let tasksCompleted: Int

    /// Number of tasks that failed
    public let tasksFailed: Int

    /// Number of tasks cancelled
    public let tasksCancelled: Int

    /// Timestamp of last cycle
    public let lastCycleTimestamp: Date?

    public init(
        cyclesRun: Int = 0,
        tasksExecuted: Int = 0,
        tasksCompleted: Int = 0,
        tasksFailed: Int = 0,
        tasksCancelled: Int = 0,
        lastCycleTimestamp: Date? = nil
    ) {
        self.cyclesRun = cyclesRun
        self.tasksExecuted = tasksExecuted
        self.tasksCompleted = tasksCompleted
        self.tasksFailed = tasksFailed
        self.tasksCancelled = tasksCancelled
        self.lastCycleTimestamp = lastCycleTimestamp
    }
}

// MARK: - Agent Coordinator

/// Actor responsible for orchestrating overnight agent tasks.
///
/// The AgentCoordinator is the core orchestration loop:
/// 1. Check for pending tasks in queue
/// 2. If no pending tasks, use DecisionEngine to decide next task
/// 3. Execute tasks using appropriate agent from AgentRegistry
/// 4. Monitor task completion and update TaskQueue status
/// 5. Report metrics and progress
///
/// Thread-safe via actor isolation.
public actor AgentCoordinator {
    // MARK: - Dependencies

    private let taskQueue: TaskQueue
    private let decisionEngine: DecisionEngine
    private let agentRegistry: AgentRegistry

    // MARK: - Metrics

    private var cyclesRun: Int = 0
    private var tasksExecuted: Int = 0
    private var tasksCompleted: Int = 0
    private var tasksFailed: Int = 0
    private var tasksCancelled: Int = 0
    private var lastCycleTimestamp: Date?

    // MARK: - Initialization

    /// Initialize the AgentCoordinator
    /// - Parameters:
    ///   - taskQueue: Task queue for persistent task storage
    ///   - decisionEngine: Engine for making task decisions
    ///   - agentRegistry: Registry of available agent providers
    public init(
        taskQueue: TaskQueue,
        decisionEngine: DecisionEngine,
        agentRegistry: AgentRegistry
    ) {
        self.taskQueue = taskQueue
        self.decisionEngine = decisionEngine
        self.agentRegistry = agentRegistry
    }

    // MARK: - Public API

    /// Run one orchestration cycle
    ///
    /// Cycle logic:
    /// 1. Check if any agents are available
    /// 2. Check for pending tasks in queue
    /// 3. If pending task exists, execute it
    /// 4. If no pending tasks, make new decision and enqueue
    /// 5. Return result of cycle
    ///
    /// - Parameters:
    ///   - timeBudgetSeconds: Time budget for new tasks (if deciding)
    ///   - workingDirectory: Working directory for agent execution
    /// - Returns: Result of the cycle
    public func runCycle(
        timeBudgetSeconds: TimeInterval,
        workingDirectory: URL?
    ) async -> CycleResult {
        cyclesRun += 1
        lastCycleTimestamp = Date()

        // 1. Check if any agents are available
        let availableProviders = await agentRegistry.availableProviders()
        guard !availableProviders.isEmpty else {
            OpenAgentsLog.orchestration.warning("AgentCoordinator No agents available for orchestration")
            return .noAgentsAvailable
        }

        // 2. Check for pending tasks
        if let pendingTask = try? await taskQueue.dequeue() {
            OpenAgentsLog.orchestration.info("AgentCoordinator Found pending task: \(pendingTask.id)")
            return await executeTask(pendingTask, workingDirectory: workingDirectory)
        }

        // 3. No pending tasks - make new decision
        OpenAgentsLog.orchestration.info("AgentCoordinator No pending tasks, making new decision")
        return await makeDecision(timeBudgetSeconds: timeBudgetSeconds)
    }

    /// Complete a task with success or failure
    /// - Parameters:
    ///   - taskId: The task ID to complete
    ///   - success: Whether the task succeeded
    ///   - error: Optional error message if failed
    public func completeTask(
        _ taskId: TaskID,
        success: Bool,
        error: String? = nil
    ) async throws {
        guard var task = try await taskQueue.get(taskId) else {
            throw TaskQueueError.taskNotFound(taskId)
        }

        if success {
            task.status = .completed
            tasksCompleted += 1
            OpenAgentsLog.orchestration.info("AgentCoordinator Task completed: \(taskId)")
        } else {
            task.status = .failed
            task.error = error
            tasksFailed += 1
            OpenAgentsLog.orchestration.error("AgentCoordinator Task failed: \(taskId) - \(error ?? "unknown")")
        }

        task.completedAt = Date()
        try await taskQueue.update(task)
    }

    /// Cancel a running task
    /// - Parameter taskId: The task ID to cancel
    public func cancelTask(_ taskId: TaskID) async throws {
        guard var task = try await taskQueue.get(taskId) else {
            throw TaskQueueError.taskNotFound(taskId)
        }

        // Cancel the agent if it's running
        if let sessionId = task.sessionId,
           let handle = await agentRegistry.handle(for: ACPSessionId(sessionId)) {
            if let provider = await agentRegistry.provider(for: handle.mode) {
                await provider.cancel(sessionId: ACPSessionId(sessionId), handle: handle)
            }
            await agentRegistry.removeHandle(for: ACPSessionId(sessionId))
        }

        task.status = .cancelled
        task.completedAt = Date()
        try await taskQueue.update(task)

        tasksCancelled += 1
        OpenAgentsLog.orchestration.info("AgentCoordinator Task cancelled: \(taskId)")
    }

    /// Get current coordinator metrics
    /// - Returns: Current metrics snapshot
    public func metrics() -> CoordinatorMetrics {
        return CoordinatorMetrics(
            cyclesRun: cyclesRun,
            tasksExecuted: tasksExecuted,
            tasksCompleted: tasksCompleted,
            tasksFailed: tasksFailed,
            tasksCancelled: tasksCancelled,
            lastCycleTimestamp: lastCycleTimestamp
        )
    }

    // MARK: - Private Helpers

    /// Make a decision for the next task and enqueue it
    private func makeDecision(timeBudgetSeconds: TimeInterval) async -> CycleResult {
        do {
            // Analyze recent sessions to get insights
            let insights = try await decisionEngine.analyzeSessions()

            // Decide next task based on insights
            let decision = try await decisionEngine.decideNextTask(
                from: insights,
                timeBudgetSeconds: timeBudgetSeconds
            )

            // Create task from decision
            let opHash = computeOpHash(decision: decision)
            let task = OvernightTask(
                opHash: opHash,
                decision: decision,
                metadata: decision.metadata ?? [:]
            )

            // Enqueue task
            let taskId = try await taskQueue.enqueue(task)
            OpenAgentsLog.orchestration.info("AgentCoordinator Decision made, task enqueued: \(taskId)")

            return .decisionMade(taskId: taskId)
        } catch {
            OpenAgentsLog.orchestration.error("AgentCoordinator Decision failed: \(error)")
            return .idle
        }
    }

    /// Execute a pending task
    private func executeTask(
        _ task: OvernightTask,
        workingDirectory: URL?
    ) async -> CycleResult {
        do {
            // Get provider for the agent mode
            guard let provider = await agentRegistry.provider(for: task.decision.agentMode) else {
                throw AgentProviderError.notAvailable("Agent \(task.decision.agentMode.rawValue) not registered")
            }

            // Check if agent is available
            guard await provider.isAvailable() else {
                throw AgentProviderError.notAvailable("Agent \(task.decision.agentMode.rawValue) not available")
            }

            // Create session ID for this task
            let sessionId = "overnight_\(task.id)_\(UUID().uuidString)"

            // Create agent context
            let context = AgentContext(
                workingDirectory: workingDirectory,
                metadata: task.metadata
            )

            // Create update hub (no-op broadcast for now)
            let updateHub = SessionUpdateHub(
                tinyvexDb: nil,
                broadcastCallback: { _ in }
            )

            // Start the agent
            let handle = try await provider.start(
                sessionId: ACPSessionId(sessionId),
                prompt: task.decision.task,
                context: context,
                updateHub: updateHub
            )

            // Update task status to in_progress
            var updatedTask = task
            updatedTask.status = .in_progress
            updatedTask.sessionId = sessionId
            updatedTask.startedAt = Date()
            try await taskQueue.update(updatedTask)

            // Store handle in registry
            await agentRegistry.setHandle(handle, for: ACPSessionId(sessionId))

            tasksExecuted += 1
            OpenAgentsLog.orchestration.info("AgentCoordinator Task executing: \(task.id) session=\(sessionId)")

            return .taskExecuted(taskId: task.id, sessionId: sessionId)
        } catch {
            // Mark task as failed
            var failedTask = task
            failedTask.status = .failed
            failedTask.error = error.localizedDescription
            failedTask.completedAt = Date()
            try? await taskQueue.update(failedTask)

            tasksFailed += 1
            OpenAgentsLog.orchestration.error("AgentCoordinator Task execution failed: \(task.id) - \(error)")

            return .taskFailed(taskId: task.id, error: error.localizedDescription)
        }
    }

    /// Compute operation hash for deduplication
    private func computeOpHash(decision: DecisionOutput) -> String {
        // Simple hash: combine task description + agent mode
        let combined = "\(decision.task)|\(decision.agentMode.rawValue)"
        return String(combined.hash)
    }
}

#endif
