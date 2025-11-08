# Overnight Agents - Testing Plan

**Last Updated**: 2025-11-08
**Status**: Design Phase

## Testing Philosophy

1. **Test the happy path first**: Prove the core loop works end-to-end
2. **Fail fast on errors**: Comprehensive error handling tests
3. **Mock expensive operations**: FM calls, agent sessions for unit tests
4. **Real integration tests**: Actual agents, actual GitHub for E2E validation
5. **Observable state**: All components expose state for testing

---

## Test Pyramid

```
                    ┌─────────────┐
                    │   Manual    │  Demo scenarios, video recording
                    │   Testing   │  Real overnight runs
                    └─────────────┘
                   ┌───────────────┐
                   │   End-to-End  │  Compressed overnight run
                   │  Integration  │  Real agents, mock GitHub
                   └───────────────┘
              ┌──────────────────────┐
              │  Component           │  Multi-agent coordination
              │  Integration Tests   │  Task queue lifecycle
              └──────────────────────┘
        ┌──────────────────────────────┐
        │      Unit Tests              │  Individual actor methods
        │  (Fast, isolated, mocked)    │  State transitions
        └──────────────────────────────┘
```

---

## Unit Tests

### SchedulerService Tests

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/SchedulerServiceTests.swift`

```swift
final class SchedulerServiceTests: XCTestCase {
    func testCronExpressionParsing() async throws {
        let scheduler = SchedulerService()

        // Every 30 minutes, 1am-5am
        let nextWake = scheduler.nextWakeTime(
            from: Date(timeIntervalSince1970: 0), // Jan 1, 1970 00:00:00
            cron: "*/30 1-5 * * *",
            jitter: 0
        )

        // Should be 1:00 AM same day
        XCTAssertNotNil(nextWake)
        // Assert hour is 1
    }

    func testConstraintChecking_PluggedIn() async throws {
        let scheduler = SchedulerService()
        let constraints = ScheduleConstraints(pluggedIn: true, wifiOnly: false)

        // Mock IOKit (or use dependency injection)
        let satisfied = await scheduler.checkConstraints(constraints)

        // Assert based on test environment
        XCTAssertTrue(satisfied)
    }

    func testTimeWindowEnforcement() async throws {
        let scheduler = SchedulerService()

        // Current time: 3:00 AM (within window)
        let currentTime = Calendar.current.date(from: DateComponents(hour: 3, minute: 0))!
        let window = TimeWindow(start: "01:00", end: "05:00")

        XCTAssertTrue(scheduler.isWithinWindow(currentTime, window: window))

        // Current time: 8:00 AM (outside window)
        let outsideTime = Calendar.current.date(from: DateComponents(hour: 8, minute: 0))!
        XCTAssertFalse(scheduler.isWithinWindow(outsideTime, window: window))
    }

    func testJitterApplication() async throws {
        let scheduler = SchedulerService()

        // Run 100 times, ensure jitter is within 0-300 seconds
        for _ in 0..<100 {
            let nextWake = scheduler.nextWakeTime(
                from: Date(),
                cron: "0 * * * *",
                jitter: 300
            )!

            let baseTime = Calendar.current.nextDate(
                after: Date(),
                matching: DateComponents(minute: 0),
                matchingPolicy: .nextTime
            )!

            let diff = nextWake.timeIntervalSince(baseTime)
            XCTAssertGreaterThanOrEqual(diff, 0)
            XCTAssertLessThanOrEqual(diff, 300)
        }
    }
}
```

### DecisionOrchestrator Tests

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/DecisionOrchestratorTests.swift`

```swift
final class DecisionOrchestratorTests: XCTestCase {
    func testDecideNextTask_MockFM() async throws {
        let orchestrator = DecisionOrchestrator(
            fmOrchestrator: MockFMOrchestrator()
        )

        let context = OrchestrationContext(
            sessionInsights: [
                SessionInsight(
                    provider: .claude_code,
                    fileFrequency: ["BridgeManager.swift": 10],
                    toolUsage: ["edit_file": 50],
                    userIntents: ["refactor", "error handling"],
                    avgSessionDuration: 600
                )
            ],
            repoStatus: RepoStatus(
                branch: "main",
                ahead: 0,
                behind: 0,
                modifiedFiles: [],
                untrackedFiles: [],
                recentCommits: [],
                testCoverage: 0.65
            ),
            recentTasks: [],
            availableAgents: [.claude_code, .codex],
            timeBudget: 1800,
            userPreferences: [:]
        )

        let decision = try await orchestrator.decideNextTask(context: context)

        XCTAssertEqual(decision.agent, .claude_code)
        XCTAssertTrue(decision.task.contains("BridgeManager"))
        XCTAssertGreaterThan(decision.confidence, 0.5)
    }

    func testSessionHistoryAnalysis() async throws {
        let orchestrator = DecisionOrchestrator()

        // Assumes test .jsonl files in test resources
        let insights = try await orchestrator.analyzeSessionHistory(
            providers: [.claude_code],
            topK: 10
        )

        XCTAssertGreaterThan(insights.count, 0)
        XCTAssertGreaterThan(insights[0].fileFrequency.count, 0)
    }

    func testFallbackWhenFMUnavailable() async throws {
        let orchestrator = DecisionOrchestrator(
            fmOrchestrator: nil // FM unavailable
        )

        let context = OrchestrationContext(/* ... */)

        let decision = try await orchestrator.decideNextTask(context: context)

        // Should use heuristic fallback
        XCTAssertNotNil(decision)
        XCTAssertEqual(decision.rationale, "Heuristic: Most frequently touched file")
    }
}
```

### TaskQueue Tests

**File**: `ios/OpenAgensCore/Tests/OpenAgentsCoreTests/Orchestration/TaskQueueTests.swift`

```swift
final class TaskQueueTests: XCTestCase {
    var taskQueue: TaskQueue!
    var tempDB: URL!

    override func setUp() async throws {
        // Create temp SQLite DB
        tempDB = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension("db")

        taskQueue = try await TaskQueue(databaseURL: tempDB)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: tempDB)
    }

    func testEnqueueDequeue() async throws {
        let task = OvernightTask(
            id: UUID().uuidString,
            opHash: "test-hash",
            status: .pending,
            decision: TaskDecision(/* ... */),
            sessionId: nil,
            createdAt: Date(),
            startedAt: nil,
            completedAt: nil,
            error: nil,
            metadata: [:]
        )

        let taskId = try await taskQueue.enqueue(task)
        XCTAssertEqual(taskId, task.id)

        let dequeued = try await taskQueue.dequeue()
        XCTAssertNotNil(dequeued)
        XCTAssertEqual(dequeued?.id, task.id)
    }

    func testDeduplication() async throws {
        let decision = TaskDecision(
            task: "Refactor X",
            agent: .claude_code,
            priority: .high,
            estimatedDuration: 600,
            rationale: "Test",
            confidence: 0.9,
            metadata: [:]
        )

        let task1 = OvernightTask(
            id: UUID().uuidString,
            opHash: "same-hash",
            status: .pending,
            decision: decision,
            /* ... */
        )

        let task2 = OvernightTask(
            id: UUID().uuidString,
            opHash: "same-hash",  // Same hash!
            status: .pending,
            decision: decision,
            /* ... */
        )

        let id1 = try await taskQueue.enqueue(task1)
        let id2 = try await taskQueue.enqueue(task2)

        // Should return existing task ID
        XCTAssertEqual(id1, id2)
    }

    func testStatusUpdate() async throws {
        let task = OvernightTask(/* ... */)
        let taskId = try await taskQueue.enqueue(task)

        try await taskQueue.updateStatus(taskId, status: .in_progress)

        let updated = try await taskQueue.all().first { $0.id == taskId }
        XCTAssertEqual(updated?.status, .in_progress)
    }

    func testObservableUpdates() async throws {
        let expectation = XCTestExpectation(description: "Queue update received")

        Task {
            for await update in taskQueue.updates {
                if case .enqueued(let taskId) = update {
                    expectation.fulfill()
                }
            }
        }

        let task = OvernightTask(/* ... */)
        _ = try await taskQueue.enqueue(task)

        await fulfillment(of: [expectation], timeout: 1.0)
    }
}
```

### AgentCoordinator Tests

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/AgentCoordinatorTests.swift`

```swift
final class AgentCoordinatorTests: XCTestCase {
    func testDelegateTask_MockAgent() async throws {
        let coordinator = AgentCoordinator(
            agentRegistry: MockAgentRegistry()
        )

        let task = OvernightTask(
            decision: TaskDecision(
                task: "Write tests for X",
                agent: .codex,
                /* ... */
            ),
            /* ... */
        )

        let result = try await coordinator.delegate(task)

        XCTAssertTrue(result.success)
        XCTAssertGreaterThan(result.toolCalls.count, 0)
    }

    func testTimeBudgetEnforcement() async throws {
        let coordinator = AgentCoordinator()

        let task = OvernightTask(
            decision: TaskDecision(
                task: "Long task",
                agent: .claude_code,
                estimatedDuration: 60,  // 1 minute budget
                /* ... */
            ),
            /* ... */
        )

        // Mock agent that runs for 90 seconds
        let result = try await coordinator.delegate(task)

        // Should be cancelled due to time budget
        XCTAssertFalse(result.success)
        XCTAssertEqual(result.error, "Time budget exceeded")
    }

    func testConcurrentSessions() async throws {
        let coordinator = AgentCoordinator()

        let task1 = OvernightTask(decision: TaskDecision(agent: .claude_code, /* ... */))
        let task2 = OvernightTask(decision: TaskDecision(agent: .codex, /* ... */))

        // Start both concurrently
        async let result1 = coordinator.delegate(task1)
        async let result2 = coordinator.delegate(task2)

        let (r1, r2) = try await (result1, result2)

        XCTAssertTrue(r1.success)
        XCTAssertTrue(r2.success)
        XCTAssertNotEqual(r1.sessionId, r2.sessionId)
    }
}
```

### PRAutomationService Tests

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/GitHubIntegration/PRAutomationServiceTests.swift`

```swift
final class PRAutomationServiceTests: XCTestCase {
    func testCreateBranch() async throws {
        let service = PRAutomationService()
        let branchName = try await service.createBranch(
            baseBranch: "main",
            sessionId: "test-session-123"
        )

        XCTAssertEqual(branchName, "agent/test-session-123")

        // Verify branch exists in git
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["branch", "--list", branchName]

        let pipe = Pipe()
        process.standardOutput = pipe
        try process.run()
        process.waitUntilExit()

        let output = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)!
        XCTAssertTrue(output.contains(branchName))
    }

    func testGeneratePRBody() async throws {
        let service = PRAutomationService()

        let task = OvernightTask(/* ... */)
        let result = AgentSessionResult(/* ... */)

        let body = try await service.generatePRBody(task: task, result: result)

        XCTAssertTrue(body.contains("Autonomous Agent Work"))
        XCTAssertTrue(body.contains(task.decision.task))
        XCTAssertTrue(body.contains(result.sessionId))
    }

    func testCommitFromToolCalls() async throws {
        // Skip if not in git repo
        guard FileManager.default.fileExists(atPath: ".git") else {
            throw XCTSkip("Not in git repo")
        }

        let service = PRAutomationService()

        let toolCalls: [ACPToolCallWire] = [
            ACPToolCallWire(
                call_id: "1",
                name: "edit_file",
                arguments: ["path": "test.txt", "content": "Hello"]
            )
        ]

        try await service.commitFromToolCalls("test-session", toolCalls: toolCalls)

        // Verify commit exists
        // git log -1 --oneline
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["log", "-1", "--oneline"]

        let pipe = Pipe()
        process.standardOutput = pipe
        try process.run()
        process.waitUntilExit()

        let output = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)!
        XCTAssertTrue(output.contains("Agent work:"))
    }
}
```

---

## Integration Tests

### End-to-End Overnight Run (Compressed)

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/OvernightRunIntegrationTests.swift`

```swift
final class OvernightRunIntegrationTests: XCTestCase {
    func testCompressedOvernightRun() async throws {
        // This test runs the full overnight loop with compressed timeline:
        // - 1-minute intervals instead of 30-minute
        // - Mock FM for speed
        // - Mock GitHub PR creation
        // - Real agents (Claude Code / Codex)

        let manifest = UpgradeManifest(
            id: "test-overnight",
            schedule: UpgradeSchedule(
                type: .cron,
                expression: "* * * * *",  // Every minute
                window: nil,  // No time window for test
                constraints: ScheduleConstraints(
                    pluggedIn: false,  // Don't require plugged in
                    wifiOnly: false,
                    cpuMaxPercentage: 100,
                    respectDnd: false,
                    suspendIfActive: false
                ),
                jitter: 0,
                onMissed: .skip
            ),
            pipeline: [
                UpgradeOperation(op: "session.analyze", params: ["topK": 5]),
                UpgradeOperation(op: "orchestrate.decide", backend: "mock"),
                UpgradeOperation(op: "agent.execute", params: nil),
                UpgradeOperation(op: "pr.create", params: ["auto_push": false])
            ],
            /* ... */
        )

        let scheduler = SchedulerService()
        try await scheduler.start(upgrade: manifest)

        // Wait for 3 cycles (3 minutes)
        try await Task.sleep(nanoseconds: 180_000_000_000)

        try await scheduler.stop()

        // Verify results
        let taskQueue = TaskQueue.shared
        let completedTasks = try await taskQueue.all(filter: TaskFilter(status: .completed))

        XCTAssertGreaterThanOrEqual(completedTasks.count, 2)

        for task in completedTasks {
            XCTAssertNotNil(task.sessionId)
            XCTAssertNotNil(task.completedAt)
            XCTAssertNil(task.error)
        }
    }
}
```

### Multi-Agent Coordination

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/MultiAgentCoordinationTests.swift`

```swift
final class MultiAgentCoordinationTests: XCTestCase {
    func testClaudeAndCodexInParallel() async throws {
        let coordinator = AgentCoordinator()

        let claudeTask = OvernightTask(
            decision: TaskDecision(
                task: "Refactor error handling in BridgeManager",
                agent: .claude_code,
                /* ... */
            )
        )

        let codexTask = OvernightTask(
            decision: TaskDecision(
                task: "Generate tests for WebSocketServer",
                agent: .codex,
                /* ... */
            )
        )

        // Run both in parallel
        async let claudeResult = coordinator.delegate(claudeTask)
        async let codexResult = coordinator.delegate(codexTask)

        let (r1, r2) = try await (claudeResult, codexResult)

        // Both should succeed
        XCTAssertTrue(r1.success)
        XCTAssertTrue(r2.success)

        // Different session IDs
        XCTAssertNotEqual(r1.sessionId, r2.sessionId)

        // Both should have tool calls
        XCTAssertGreaterThan(r1.toolCalls.count, 0)
        XCTAssertGreaterThan(r2.toolCalls.count, 0)
    }
}
```

### FM Decision Quality

**File**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/FMDecisionQualityTests.swift`

```swift
final class FMDecisionQualityTests: XCTestCase {
    func testFMSelectsRelevantTask() async throws {
        // Skip if FM not available
        guard SystemLanguageModel.default.availability.status == .available else {
            throw XCTSkip("Foundation Models not available on this device")
        }

        let orchestrator = DecisionOrchestrator()

        let context = OrchestrationContext(
            sessionInsights: [
                SessionInsight(
                    provider: .claude_code,
                    fileFrequency: [
                        "BridgeManager.swift": 25,
                        "AgentProvider.swift": 10,
                        "SessionUpdateHub.swift": 5
                    ],
                    toolUsage: ["edit_file": 100, "run_bash": 20],
                    userIntents: ["refactor", "improve error handling", "add logging"],
                    avgSessionDuration: 900
                )
            ],
            repoStatus: RepoStatus(
                branch: "main",
                testCoverage: 0.65,
                /* ... */
            ),
            /* ... */
        )

        let decision = try await orchestrator.decideNextTask(context: context)

        // Should mention BridgeManager (most touched file)
        XCTAssertTrue(decision.task.lowercased().contains("bridge"))

        // Should be high priority (user frequently works on this)
        XCTAssertEqual(decision.priority, .high)

        // Should select Claude Code (complex refactoring)
        XCTAssertEqual(decision.agent, .claude_code)

        // Should have reasonable confidence
        XCTAssertGreaterThan(decision.confidence, 0.7)

        // Should have explanation
        XCTAssertGreaterThan(decision.rationale.count, 50)
    }

    func testFMSuggestsTestsForLowCoverage() async throws {
        guard SystemLanguageModel.default.availability.status == .available else {
            throw XCTSkip("Foundation Models not available")
        }

        let orchestrator = DecisionOrchestrator()

        let context = OrchestrationContext(
            sessionInsights: [],
            repoStatus: RepoStatus(
                branch: "main",
                testCoverage: 0.35,  // Low coverage!
                /* ... */
            ),
            /* ... */
        )

        let decision = try await orchestrator.decideNextTask(context: context)

        // Should suggest writing tests
        XCTAssertTrue(decision.task.lowercased().contains("test"))

        // Should use Codex (better for test generation)
        XCTAssertEqual(decision.agent, .codex)
    }
}
```

---

## Manual Testing Scenarios

### Scenario 1: Single Overnight Cycle

**Goal**: Verify one full orchestration cycle works end-to-end

**Steps**:
1. Load upgrade manifest: `examples/nightly-refactor.json`
2. Start scheduler with compressed timeline (5 min interval)
3. Monitor iOS app for real-time updates
4. Verify:
   - DecisionOrchestrator makes decision
   - Agent starts and completes work
   - PR is created on GitHub
   - iOS app shows correct status

**Expected Results**:
- ✅ Task appears in iOS queue
- ✅ Agent session starts
- ✅ Tool calls stream to iOS
- ✅ PR created with correct title/body
- ✅ Task marked completed
- ✅ No errors in logs

**Cleanup**:
- Delete test PR
- Delete agent branch
- Clear task queue

---

### Scenario 2: Real Overnight Run (8 Hours)

**Goal**: Validate production-ready overnight run

**Steps**:
1. Setup:
   - Clean main branch (no uncommitted changes)
   - Verify gh CLI authenticated
   - Verify Foundation Models available
   - Load production upgrade manifest
2. Start scheduler at 10:00 PM
3. Let run overnight
4. Check results at 6:00 AM

**Expected Results**:
- ✅ 8-12 orchestration cycles completed
- ✅ 5-10 PRs created
- ✅ All PRs have passing tests
- ✅ No duplicate work
- ✅ Reasonable task selection (based on repo state)
- ✅ No crashes or hangs

**Metrics to Collect**:
- Total orchestration cycles: N
- Total tasks enqueued: N
- Total tasks completed: N
- Total tasks failed: N
- Total PRs created: N
- Average task duration: N seconds
- FM decision quality (manual review)

---

### Scenario 3: Constraint Enforcement

**Goal**: Verify constraints are respected

**Steps**:
1. Set upgrade manifest constraints:
   ```json
   {
     "constraints": {
       "plugged_in": true,
       "wifi_only": true,
       "cpu_max_percentage": 50,
       "respect_dnd": true,
       "suspend_if_active": true
     }
   }
   ```
2. Start scheduler
3. Test each constraint:
   - Unplug laptop → should pause
   - Switch to cellular → should pause
   - Run CPU-intensive task → should pause
   - Enable Do Not Disturb → should pause
   - Open app in foreground → should pause

**Expected Results**:
- ✅ Scheduler pauses when constraints violated
- ✅ Scheduler resumes when constraints satisfied
- ✅ No tasks started during constraint violations
- ✅ Logs show constraint check results

---

### Scenario 4: Error Recovery

**Goal**: Verify graceful error handling

**Steps**:
1. Force various errors:
   - Kill agent process mid-execution
   - Disconnect network during PR creation
   - Make git conflict
   - Exceed time budget
   - Trigger AUP violation
2. Verify recovery behavior

**Expected Results**:
- ✅ Agent crash → task marked failed, next cycle continues
- ✅ Network error → retry with backoff, eventually fail gracefully
- ✅ Git conflict → alert user, don't create PR
- ✅ Time budget exceeded → cancel agent, mark task failed
- ✅ AUP violation → immediate stop, log violation, alert user

---

### Scenario 5: iOS Monitoring

**Goal**: Verify iOS app shows correct real-time state

**Steps**:
1. Start overnight run on macOS
2. Open iOS app
3. Verify real-time updates:
   - Task queue counts (pending/active/done)
   - Agent session progress
   - Tool call streaming
   - Decision rationale
   - PR preview

**Expected Results**:
- ✅ Task queue updates in real-time
- ✅ Session progress shows tool calls as they happen
- ✅ Decision cards show FM rationale
- ✅ PR preview shows correct title/body
- ✅ Manual approve/cancel works
- ✅ No UI lag or crashes

---

## Performance Testing

### Load Testing

**Goal**: Verify system handles multiple cycles without degradation

**Test**:
```swift
func testNoConcurrentTaskLimit() async throws {
    let coordinator = AgentCoordinator()

    // Enqueue 20 tasks
    var tasks: [OvernightTask] = []
    for i in 0..<20 {
        tasks.append(OvernightTask(
            decision: TaskDecision(task: "Task \(i)", agent: .claude_code)
        ))
    }

    let startTime = Date()

    // Execute all (should throttle to max 2 concurrent)
    for task in tasks {
        _ = try await coordinator.delegate(task)
    }

    let duration = Date().timeIntervalSince(startTime)

    // Should take roughly: 20 tasks / 2 concurrent * avg_duration
    // With avg_duration ~5 min = ~50 minutes total
    XCTAssertLessThan(duration, 60 * 60)  // Under 1 hour
}
```

### Memory Leak Testing

**Test**:
```swift
func testNoMemoryLeaksInLongRun() async throws {
    let scheduler = SchedulerService()
    let manifest = UpgradeManifest(/* ... */)

    let initialMemory = getMemoryUsage()

    try await scheduler.start(upgrade: manifest)

    // Run for 1 hour (simulated)
    for _ in 0..<60 {
        try await Task.sleep(nanoseconds: 60_000_000_000)
    }

    try await scheduler.stop()

    let finalMemory = getMemoryUsage()

    // Memory should not grow more than 50 MB
    XCTAssertLessThan(finalMemory - initialMemory, 50 * 1024 * 1024)
}
```

---

## Continuous Testing

### CI/CD Pipeline

**GitHub Actions** (`.github/workflows/overnight-tests.yml`):
```yaml
name: Overnight Agent Tests

on:
  push:
    branches: [main]
    paths:
      - 'ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/**'
      - 'ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/**'

jobs:
  unit-tests:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - name: Run unit tests
        run: |
          cd ios
          xcodebuild test \
            -workspace OpenAgents.xcworkspace \
            -scheme OpenAgents \
            -sdk macosx \
            -testPlan UnitTests

  integration-tests:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - name: Run integration tests
        run: |
          cd ios
          xcodebuild test \
            -workspace OpenAgents.xcworkspace \
            -scheme OpenAgents \
            -sdk macosx \
            -testPlan IntegrationTests
        timeout-minutes: 30
```

---

## Test Coverage Goals

| Component | Target Coverage | Current |
|-----------|----------------|---------|
| SchedulerService | 90% | 0% |
| DecisionOrchestrator | 85% | 0% |
| TaskQueue | 95% | 0% |
| AgentCoordinator | 85% | 0% |
| PRAutomationService | 80% | 0% |
| UpgradeExecutor | 90% | 0% |
| PolicyEnforcer | 95% | 0% |

**Overall Target**: 85%+ coverage on new code

---

## Testing Tools

### Mocks & Stubs

**MockFMOrchestrator**:
```swift
class MockFMOrchestrator: FMOrchestratorProtocol {
    var mockDecision: TaskDecision?

    func decideNextTask(context: OrchestrationContext) async throws -> TaskDecision {
        return mockDecision ?? TaskDecision(
            task: "Mock task",
            agent: .claude_code,
            priority: .medium,
            estimatedDuration: 600,
            rationale: "Mock rationale",
            confidence: 0.8,
            metadata: [:]
        )
    }
}
```

**MockAgentProvider**:
```swift
class MockAgentProvider: AgentProvider {
    var mockSessionUpdates: [ACPSessionUpdate] = []

    func start(sessionId: String, prompt: String, workingDirectory: URL) async throws {
        // Simulate agent work
        for update in mockSessionUpdates {
            await SessionUpdateHub.shared.broadcast(sessionId: sessionId, update: update)
        }
    }
}
```

### Test Fixtures

**Test Session Files** (`ios/OpenAgentsCore/Tests/Fixtures/`):
- `claude-code-session-001.jsonl` - Typical refactoring session
- `codex-session-001.jsonl` - Test generation session
- `failed-session-001.jsonl` - Session with errors

**Test Upgrade Manifests** (`ios/OpenAgentsCore/Tests/Fixtures/`):
- `valid-manifest.json` - Properly formed manifest
- `invalid-manifest-missing-fields.json` - Missing required fields
- `invalid-manifest-bad-cron.json` - Invalid cron expression

---

## Test Execution Order

### Phase 1: Unit Tests (Week 1)
1. SchedulerService
2. DecisionOrchestrator (with mocks)
3. TaskQueue
4. PolicyEnforcer

### Phase 2: Integration Tests (Week 2)
1. AgentCoordinator (real agents, mock GitHub)
2. PRAutomationService (real git, mock gh CLI)
3. UpgradeExecutor (end-to-end pipeline)

### Phase 3: E2E Tests (Week 3)
1. Compressed overnight run (1 hour)
2. Multi-agent coordination
3. FM decision quality

### Phase 4: Manual Tests (Before Demo)
1. Real overnight run (8 hours)
2. Constraint enforcement
3. Error recovery
4. iOS monitoring
5. Final video recording dry run

---

## Success Criteria

**Before Merge**:
- ✅ All unit tests pass
- ✅ All integration tests pass
- ✅ Code coverage ≥ 85%
- ✅ No memory leaks
- ✅ No race conditions (Thread Sanitizer clean)

**Before Demo**:
- ✅ Real overnight run produces 5+ quality PRs
- ✅ No crashes or hangs
- ✅ FM decisions make sense (manual review)
- ✅ iOS app updates in real-time
- ✅ All constraints respected

**Demo Day**:
- ✅ Video shows working system end-to-end
- ✅ PRs are real and valuable
- ✅ Upgrade JSON is clean and portable
- ✅ Future vision (Nostr, Bitcoin) is clear

---

## Next Steps

1. Implement SchedulerService + tests
2. Implement DecisionOrchestrator + tests
3. Implement TaskQueue + tests
4. Run first integration test (compressed overnight)
5. Iterate based on results
6. Schedule real overnight run
7. Record demo video
