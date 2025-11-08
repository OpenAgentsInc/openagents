# Issue #010: End-to-End Integration Testing

**Component**: Testing Infrastructure
**Priority**: P1 (High)
**Estimated Effort**: 3-4 days
**Dependencies**: #001-#009 (all components)
**Assignee**: TBD

---

## Overview

Comprehensive integration tests validating the full overnight orchestration system: compressed overnight run, multi-agent coordination, FM decision quality, error recovery.

**Location**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/`

---

## Test Suites

### 1. Compressed Overnight Run

**File**: `OvernightRunIntegrationTests.swift`

```swift
func testCompressedOvernightRun() async throws {
    // Run full cycle with 1-min intervals instead of 30-min
    let manifest = loadManifest("examples/nightly-refactor.json")
    manifest.schedule.expression = "* * * * *"  // Every minute

    let scheduler = SchedulerService()
    try await scheduler.start(upgrade: manifest)

    // Wait for 3 cycles (3 minutes)
    try await Task.sleep(nanoseconds: 180_000_000_000)

    try await scheduler.stop()

    // Verify results
    let taskQueue = TaskQueue.shared
    let completed = try await taskQueue.all(filter: TaskFilter(status: .completed))

    XCTAssertGreaterThanOrEqual(completed.count, 2)

    for task in completed {
        XCTAssertNotNil(task.sessionId)
        XCTAssertNotNil(task.completedAt)
        XCTAssertNil(task.error)
    }
}
```

### 2. Multi-Agent Coordination

**File**: `MultiAgentCoordinationTests.swift`

```swift
func testClaudeAndCodexInParallel() async throws {
    let coordinator = AgentCoordinator()

    let claudeTask = OvernightTask(decision: TaskDecision(agent: .claude_code, ...))
    let codexTask = OvernightTask(decision: TaskDecision(agent: .codex, ...))

    async let r1 = coordinator.delegate(claudeTask)
    async let r2 = coordinator.delegate(codexTask)

    let (result1, result2) = try await (r1, r2)

    XCTAssertTrue(result1.success)
    XCTAssertTrue(result2.success)
    XCTAssertNotEqual(result1.sessionId, result2.sessionId)
}
```

### 3. FM Decision Quality

**File**: `FMDecisionQualityTests.swift`

```swift
func testFMSelectsRelevantTask() async throws {
    guard SystemLanguageModel.default.availability.status == .available else {
        throw XCTSkip("FM not available")
    }

    let orchestrator = DecisionOrchestrator()

    let context = OrchestrationContext(
        sessionInsights: [/* realistic session data */],
        repoStatus: RepoStatus(/* current repo state */),
        ...
    )

    let decision = try await orchestrator.decideNextTask(context: context)

    // Manual assertions based on context
    XCTAssertGreaterThan(decision.confidence, 0.7)
    XCTAssertGreaterThan(decision.rationale.count, 50)
    XCTAssertTrue(["claude-code", "codex"].contains(decision.agent.rawValue))
}
```

### 4. Error Recovery

**File**: `ErrorRecoveryIntegrationTests.swift`

```swift
func testRecoveryFromAgentCrash() async throws {
    // Mock agent that crashes mid-execution
    let coordinator = AgentCoordinator()
    let task = OvernightTask(/* ... */)

    // Agent will crash after 10 seconds
    do {
        _ = try await coordinator.delegate(task)
        XCTFail("Should have thrown")
    } catch {
        // Expected error
    }

    // Verify task marked as failed
    let taskQueue = TaskQueue.shared
    let updated = try await taskQueue.find(task.id)
    XCTAssertEqual(updated.status, .failed)

    // Verify scheduler continues to next task
    let nextTask = try await taskQueue.dequeue()
    XCTAssertNotNil(nextTask)
}
```

### 5. PR Creation Pipeline

**File**: `PRAutomationIntegrationTests.swift`

```swift
func testFullPRPipeline() async throws {
    // Requires gh CLI authentication
    guard isGHAuthenticated() else {
        throw XCTSkip("gh CLI not authenticated")
    }

    let prService = PRAutomationService()
    let sessionId = UUID().uuidString

    // Create branch
    let branch = try await prService.createBranch(baseBranch: "main", sessionId: sessionId)

    // Make some changes (mock tool calls)
    let toolCalls = [/* ... */]
    try await prService.commitFromToolCalls(sessionId, toolCalls: toolCalls)

    // Push
    try await prService.push(branch: branch, remote: "origin")

    // Create PR
    let prNumber = try await prService.createPR(
        title: "Test PR",
        body: "Test body",
        branch: branch,
        baseBranch: "main",
        draft: true
    )

    XCTAssertGreaterThan(prNumber, 0)

    // Cleanup: close PR, delete branch
    try await cleanup(prNumber: prNumber, branch: branch)
}
```

---

## Manual Test Scenarios

### Scenario 1: Real Overnight Run

**Duration**: 8 hours
**Goal**: Validate production-ready overnight orchestration

**Steps**:
1. Clean repo state (no uncommitted changes)
2. Load `examples/nightly-refactor.json`
3. Start scheduler at 10:00 PM
4. Let run overnight
5. Check results at 6:00 AM

**Expected Results**:
- 8-12 orchestration cycles
- 5-10 PRs created
- All PRs have passing tests
- FM decisions are reasonable
- No crashes or hangs

### Scenario 2: Constraint Enforcement

Test each constraint separately:
- Unplug laptop → should pause
- Switch to cellular → should pause
- Run CPU-intensive task → should pause
- Enable Do Not Disturb → should pause
- Use app actively → should pause

---

## Performance Benchmarks

```swift
func testNoMemoryLeaks() async throws {
    let initialMemory = getMemoryUsage()

    let scheduler = SchedulerService()
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

## Acceptance Criteria

- [ ] Compressed overnight run succeeds (3+ cycles in 3 min)
- [ ] Multi-agent coordination works (Claude + Codex parallel)
- [ ] FM decision quality validated (confidence > 0.7)
- [ ] Error recovery works (agent crash, network error, timeout)
- [ ] PR pipeline end-to-end works
- [ ] No memory leaks over 1-hour run
- [ ] All manual scenarios pass

---

## References

- testing-plan.md
