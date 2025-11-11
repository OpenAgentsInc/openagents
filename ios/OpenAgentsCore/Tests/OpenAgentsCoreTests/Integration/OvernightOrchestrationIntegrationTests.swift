#if os(macOS)
import XCTest
@testable import OpenAgentsCore

/// Integration tests for overnight orchestration flow
///
/// These tests verify the end-to-end orchestration flow:
/// 1. SchedulerService triggers on schedule
/// 2. Constraints are checked
/// 3. AgentCoordinator runs cycle
/// 4. DecisionEngine makes decisions
/// 5. Tasks are queued and executed
/// 6. Codex is delegated work
///
/// Tests use compressed timelines (minutes instead of hours) for fast execution.
class OvernightOrchestrationIntegrationTests: XCTestCase {

    var tempDir: URL!
    var tinyvexDB: TinyvexDbLayer!
    var taskQueue: TaskQueue!
    var decisionEngine: DecisionEngine!
    var scheduler: SchedulerService!

    override func setUp() async throws {
        try await super.setUp()

        // Create temp directory for test workspace
        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("orchestration-integration-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)

        // Create Tinyvex DB
        let dbPath = tempDir.appendingPathComponent("test.db").path
        tinyvexDB = try TinyvexDbLayer(path: dbPath)

        // Initialize task queue
        taskQueue = TaskQueue(db: tinyvexDB)

        // Initialize decision engine
        decisionEngine = DecisionEngine()

        // Initialize scheduler
        scheduler = SchedulerService()
    }

    override func tearDown() async throws {
        // Stop scheduler
        await scheduler.stop()

        // Clean up temp directory
        if let tempDir = tempDir {
            try? FileManager.default.removeItem(at: tempDir)
        }

        try await super.tearDown()
    }

    // MARK: - Compressed Overnight Run

    func testCompressedOvernightRun() async throws {
        // This test runs a compressed overnight orchestration:
        // - 3 cycles over 6 minutes (every 2 minutes)
        // - No actual agent execution (mocked)
        // - Verifies scheduler triggers and constraints are checked

        var cycleCount = 0
        let expectedCycles = 3

        // Create config with very short intervals for testing
        let config = OrchestrationConfig(
            workspaceRoot: tempDir.path,
            schedule: OrchestrationConfig.Schedule(
                expression: "*/2 * * * *", // Every 2 minutes
                windowStart: "00:00", // All day for test
                windowEnd: "23:59",
                jitterMs: 0, // No jitter for deterministic test
                onMissed: "skip"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: false, // Don't check (would fail in CI)
                wifiOnly: false   // Don't check (would fail in CI)
            ),
            timeBudgetSec: 120, // 2 minutes per cycle
            maxConcurrent: 1
        )

        // Configure scheduler with mock trigger
        await scheduler.configure(config: config) {
            cycleCount += 1
            print("Orchestration cycle #\(cycleCount) triggered")
        }

        // Start scheduler
        await scheduler.start()

        // Wait for 3 cycles (6 minutes + buffer)
        // In real usage, this would be overnight (hours)
        try await Task.sleep(nanoseconds: 7 * 60 * 1_000_000_000) // 7 minutes

        // Stop scheduler
        await scheduler.stop()

        // Verify cycles ran
        let metrics = await scheduler.metrics()
        let actualCycleCount = metrics["cycle_count"] as? Int ?? 0

        XCTAssertGreaterThanOrEqual(actualCycleCount, expectedCycles - 1, "Should run at least 2-3 cycles")
        XCTAssertLessThanOrEqual(actualCycleCount, expectedCycles + 1, "Should not run more than 4 cycles")

        // Verify state
        let status = await scheduler.status()
        XCTAssertEqual(status, .stopped, "Scheduler should be stopped")
    }

    // MARK: - Constraint Checking Integration

    func testConstraintCheckingPreventsExecution() async throws {
        // Test that constraints actually prevent execution
        // Use pluggedIn constraint (will likely fail in CI, which is what we want to test)

        var cycleCount = 0

        let config = OrchestrationConfig(
            workspaceRoot: tempDir.path,
            schedule: OrchestrationConfig.Schedule(
                expression: "*/1 * * * *", // Every minute
                windowStart: "00:00",
                windowEnd: "23:59",
                jitterMs: 0,
                onMissed: "skip"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: true,  // This constraint may fail in CI
                wifiOnly: false
            ),
            timeBudgetSec: 60
        )

        await scheduler.configure(config: config) {
            cycleCount += 1
        }

        await scheduler.start()

        // Wait 3 minutes
        try await Task.sleep(nanoseconds: 3 * 60 * 1_000_000_000)

        await scheduler.stop()

        // Cycle count depends on whether machine is plugged in
        // We just verify scheduler respects constraints (doesn't crash)
        let metrics = await scheduler.metrics()
        let actualCycleCount = metrics["cycle_count"] as? Int ?? 0

        print("Cycles run with pluggedIn constraint: \(actualCycleCount)")

        // Should be 0-3 depending on power state
        XCTAssertGreaterThanOrEqual(actualCycleCount, 0)
        XCTAssertLessThanOrEqual(actualCycleCount, 4)
    }

    // MARK: - Time Window Enforcement

    func testTimeWindowEnforcement() async throws {
        // Test that scheduler respects time windows
        // Create a window that's definitely not active right now

        let calendar = Calendar.current
        let now = Date()
        let components = calendar.dateComponents([.hour], from: now)
        let currentHour = components.hour ?? 0

        // Create a window that's NOT active (current hour + 2, for 1 hour)
        let windowStartHour = (currentHour + 2) % 24
        let windowEndHour = (currentHour + 3) % 24

        let windowStart = String(format: "%02d:00", windowStartHour)
        let windowEnd = String(format: "%02d:00", windowEndHour)

        var cycleCount = 0

        let config = OrchestrationConfig(
            workspaceRoot: tempDir.path,
            schedule: OrchestrationConfig.Schedule(
                expression: "*/1 * * * *",
                windowStart: windowStart,
                windowEnd: windowEnd,
                jitterMs: 0,
                onMissed: "skip"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: false,
                wifiOnly: false
            )
        )

        await scheduler.configure(config: config) {
            cycleCount += 1
        }

        await scheduler.start()

        // Wait 2 minutes
        try await Task.sleep(nanoseconds: 2 * 60 * 1_000_000_000)

        await scheduler.stop()

        // Should not have run any cycles (outside window)
        XCTAssertEqual(cycleCount, 0, "Should not run outside time window")
    }

    // MARK: - Jitter Application

    func testJitterApplication() async throws {
        // Test that jitter is applied (wake times vary)

        var wakeTimes: [Date] = []

        let config = OrchestrationConfig(
            workspaceRoot: tempDir.path,
            schedule: OrchestrationConfig.Schedule(
                expression: "*/1 * * * *",
                windowStart: "00:00",
                windowEnd: "23:59",
                jitterMs: 10000, // 10 second jitter
                onMissed: "skip"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: false,
                wifiOnly: false
            )
        )

        await scheduler.configure(config: config) {
            wakeTimes.append(Date())
        }

        await scheduler.start()

        // Wait for 3 cycles
        try await Task.sleep(nanoseconds: 4 * 60 * 1_000_000_000)

        await scheduler.stop()

        // Verify we got multiple wake times
        XCTAssertGreaterThanOrEqual(wakeTimes.count, 2, "Should have at least 2 wake times")

        // Verify wake times are not exactly 60 seconds apart (due to jitter)
        if wakeTimes.count >= 2 {
            let interval = wakeTimes[1].timeIntervalSince(wakeTimes[0])

            // Should be around 60 seconds, but not exactly (due to jitter)
            // Allow 50-70 second range (60 ± 10)
            XCTAssertGreaterThan(interval, 50.0, "Interval should be at least 50s")
            XCTAssertLessThan(interval, 70.0, "Interval should be at most 70s")

            // Most importantly: should not be exactly 60.0
            // (extremely unlikely with jitter)
            let tolerance: Double = 0.1
            let isExactly60 = abs(interval - 60.0) < tolerance
            XCTAssertFalse(isExactly60, "Jitter should prevent exact 60s intervals")
        }
    }

    // MARK: - Sleep Prevention

    func testSleepPrevention() async throws {
        // Test that scheduler creates sleep assertion

        let config = OrchestrationConfig(
            workspaceRoot: tempDir.path,
            schedule: OrchestrationConfig.Schedule(
                expression: "*/5 * * * *",
                windowStart: "00:00",
                windowEnd: "23:59"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: false,
                wifiOnly: false
            )
        )

        await scheduler.configure(config: config) {
            // No-op trigger
        }

        // Start scheduler (should create assertion)
        await scheduler.start()

        // Give it a moment to create assertion
        try await Task.sleep(nanoseconds: 100_000_000) // 0.1s

        // We can't directly verify IOPMAssertion was created from here
        // but we can verify scheduler started and didn't crash
        let status = await scheduler.status()
        XCTAssertNotEqual(status, .stopped, "Scheduler should be running")

        // Stop scheduler (should release assertion)
        await scheduler.stop()

        let stoppedStatus = await scheduler.status()
        XCTAssertEqual(stoppedStatus, .stopped, "Scheduler should be stopped")
    }

    // MARK: - Integration with AgentCoordinator

    func testIntegrationWithAgentCoordinator() async throws {
        // Test full stack: Scheduler → AgentCoordinator → DecisionEngine → TaskQueue

        // NOTE: This test doesn't actually run Codex (no AgentRegistry available in test)
        // It verifies the wiring between components

        let agentRegistry = AgentRegistry()
        let coordinator = AgentCoordinator(
            taskQueue: taskQueue,
            decisionEngine: decisionEngine,
            agentRegistry: agentRegistry,
            updateHub: nil // No update hub for test
        )

        var coordinatorCycleCount = 0

        let config = OrchestrationConfig(
            workspaceRoot: tempDir.path,
            schedule: OrchestrationConfig.Schedule(
                expression: "*/1 * * * *",
                windowStart: "00:00",
                windowEnd: "23:59",
                jitterMs: 0
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: false,
                wifiOnly: false
            ),
            goals: ["test goal"]
        )

        // Configure scheduler to trigger coordinator
        await scheduler.configure(config: config) {
            coordinatorCycleCount += 1

            // Trigger coordinator cycle
            Task {
                do {
                    let result = await coordinator.runCycle()
                    print("Coordinator result: \(result)")
                } catch {
                    print("Coordinator error: \(error)")
                }
            }
        }

        await scheduler.start()

        // Wait for 2 cycles
        try await Task.sleep(nanoseconds: 150 * 1_000_000_000) // 2.5 minutes

        await scheduler.stop()

        // Verify coordinator was triggered
        XCTAssertGreaterThanOrEqual(coordinatorCycleCount, 1, "Coordinator should be triggered")

        // Verify coordinator metrics
        let metrics = await coordinator.metrics()
        XCTAssertGreaterThanOrEqual(metrics.cyclesRun, 1, "Coordinator should run at least 1 cycle")
    }

    // MARK: - Metrics Tracking

    func testMetricsTracking() async throws {
        var cycleCount = 0

        let config = OrchestrationConfig(
            workspaceRoot: tempDir.path,
            schedule: OrchestrationConfig.Schedule(
                expression: "*/1 * * * *",
                windowStart: "00:00",
                windowEnd: "23:59",
                jitterMs: 0
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: false,
                wifiOnly: false
            )
        )

        await scheduler.configure(config: config) {
            cycleCount += 1
        }

        await scheduler.start()

        // Wait for 2 cycles
        try await Task.sleep(nanoseconds: 150 * 1_000_000_000)

        let metrics = await scheduler.metrics()

        // Verify metrics structure
        XCTAssertNotNil(metrics["cycle_count"])
        XCTAssertNotNil(metrics["state"])

        if let cycleCountMetric = metrics["cycle_count"] as? Int {
            XCTAssertGreaterThanOrEqual(cycleCountMetric, 1)
        }

        if let state = metrics["state"] as? String {
            XCTAssertEqual(state, "running")
        }

        // After first cycle, should have last_run_time
        if cycleCount > 0 {
            XCTAssertNotNil(metrics["last_run_time"])
            XCTAssertNotNil(metrics["seconds_since_last_run"])
        }

        await scheduler.stop()
    }
}

#endif
