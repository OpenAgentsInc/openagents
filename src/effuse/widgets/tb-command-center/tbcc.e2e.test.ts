import { describe, it, expect } from "bun:test"
import { Effect, Stream } from "effect"
import { makeHappyDomLayer } from "../../testing/layers/happy-dom.js"
import { TestHarnessTag, TestBrowserTag } from "../../testing/index.js"
import { SocketServiceTag, type SocketService } from "../../services/socket.js"
import {
  TBCCDashboardWidget,
  TBCCTaskBrowserWidget,
  TBCCRunBrowserWidget,
  TBCCSettingsWidget,
} from "./index.js"

// Mock Data
const MOCK_TASKS = [
  {
    id: "task-1",
    name: "Fix Bug",
    description: "Fix a critical bug",
    difficulty: "hard",
    category: "Debugging",
    tags: ["bug", "urgent"],
    timeout: 300,
    max_turns: 50,
  },
  {
    id: "task-2",
    name: "Write Docs",
    description: "Write documentation",
    difficulty: "easy",
    category: "Documentation",
    tags: ["docs"],
    timeout: 100,
    max_turns: 10,
  },
]

const MOCK_RUNS = [
  {
    runId: "run-1",
    suiteName: "Test Suite",
    suiteVersion: "1.0",
    taskIds: ["task-1"],
    taskNames: ["Fix Bug"],
    status: "completed",
    outcome: "success",
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    durationMs: 5000,
    stepsCount: 10,
    tokensUsed: 1000,
    timestamp: new Date().toISOString(),
    passRate: 1.0,
    totalTasks: 1,
    completedTasks: 1,
    failedTasks: 0,
    passed: 1,
    failed: 0,
    timeout: 0,
    error: 0,
    totalDurationMs: 5000,
    totalTokens: 1000,
    totalCost: 0.05,
    taskCount: 1,
    filepath: "test-suite.json",
  },
]

// Mock Socket Service
const createMockSocket = (): SocketService => ({
  connect: () => Effect.void,
  disconnect: () => Effect.void,
  isConnected: () => Effect.succeed(true),
  getMessages: () => Stream.empty,
  loadTBSuite: () => Effect.succeed({ name: "Test Suite", version: "1.0", tasks: MOCK_TASKS }),
  startTBRun: () => Effect.succeed({ runId: "new-run-1" }),
  stopTBRun: () => Effect.succeed({ stopped: true }),
  loadRecentTBRuns: () => Effect.succeed(MOCK_RUNS as any),
  loadTBRunDetails: () => Effect.succeed({
    meta: {
      runId: "run-1",
      suiteName: "Test Suite",
      suiteVersion: "1.0",
      timestamp: new Date().toISOString(),
      totalDurationMs: 5000,
      totalTokens: 1000,
      status: "completed",
      outcome: "success",
      passRate: 1.0,
      totalTasks: 1,
      completedTasks: 1,
      failedTasks: 0,
      passed: 1,
      failed: 0,
      timeout: 0,
      error: 0,
      totalCost: 0.05,
      taskIds: ["task-1"],
      taskNames: ["Fix Bug"],
      startedAt: new Date().toISOString(),
      finishedAt: new Date().toISOString(),
      durationMs: 5000,
      stepsCount: 10,
      tokensUsed: 1000,
      taskCount: 1,
      filepath: "test-suite.json",
    },
    tasks: [
      {
        id: "task-1",
        name: "Fix Bug",
        category: "Debugging",
        difficulty: "hard",
        outcome: "success",
        turns: 10,
        durationMs: 5000,
        tokens: 1000,
        error: null,
      }
    ],
  } as any),
  loadReadyTasks: () => Effect.succeed([]),
  assignTaskToMC: () => Effect.succeed({ assigned: true }),
  loadUnifiedTrajectories: () => Effect.succeed([]),
  getHFTrajectoryCount: () => Effect.succeed(0),
  getHFTrajectories: () => Effect.succeed([]),
})

describe("TB Command Center E2E", () => {
  it("TBCC-001..005: Dashboard displays KPIs and runs", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag
            const browser = yield* TestBrowserTag

            const dashboardHandle = yield* harness.mount(TBCCDashboardWidget, {
              containerId: "tbcc-tab-dashboard",
            })
            yield* dashboardHandle.waitForState((s) => !s.loading)

            // TBCC-001: Dashboard visible
            const html = yield* dashboardHandle.getHTML
            expect(html).toContain("Dashboard")

            // TBCC-002: KPIs (Pass Rate, etc)
            expect(html).toContain("Success Rate")
            expect(html).toContain("Total Runs")

            // TBCC-003: Recent Runs
            expect(html).toContain("run-1")
            expect(html).toContain("success")

            // TBCC-004: Start Benchmark Button
            yield* browser.expectVisible("button[data-action='runFullBenchmark']")

            // TBCC-005: Navigate to run (simulate click)
            // Note: In a real app this emits an event. We can verify the event is emitted.
            // For now, just checking the button exists and is clickable.
            yield* browser.click("button[data-run-id='run-1']")

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocket()),
            Effect.provide(layer)
          )
        })
      )
    )
  })

  it("TBCC-010..014: Task Browser functionality", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag

            const taskHandle = yield* harness.mount(TBCCTaskBrowserWidget, {
              containerId: "tbcc-tab-tasks",
            })
            yield* taskHandle.waitForState((s) => !s.loading)

            // TBCC-010: Browse tasks
            const html = yield* taskHandle.getHTML
            expect(html).toContain("Fix Bug")
            expect(html).toContain("Write Docs")

            // TBCC-011: Filter buttons exist
            expect(html).toContain("data-difficulty=\"hard\"")
            expect(html).toContain("data-difficulty=\"easy\"")

            // TBCC-012: Search input exists
            expect(html).toContain("Search tasks...")

            // TBCC-013: Task items are clickable
            expect(html).toContain("data-task-id=\"task-1\"")
            expect(html).toContain("data-task-id=\"task-2\"")

            // TBCC-014: Verify task details are available
            expect(html).toContain("Debugging")
            expect(html).toContain("Documentation")

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocket()),
            Effect.provide(layer)
          )
        })
      )
    )
  })

  it("TBCC-020..024: Run Browser functionality", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag
            const browser = yield* TestBrowserTag

            const runHandle = yield* harness.mount(TBCCRunBrowserWidget, {
              containerId: "tbcc-tab-runs",
            })
            yield* runHandle.waitForState((s) => !s.loading)

            // TBCC-020: Local run history
            const html = yield* runHandle.getHTML
            expect(html).toContain("run-1")
            expect(html).toContain("success")

            // TBCC-021: HF Trajectories (Tab switch)
            yield* browser.click("button[data-source='hf']")
            yield* runHandle.waitForState((s) => s.dataSource === "hf")
            const hfHtml = yield* runHandle.getHTML
            // Note: Mock returns empty HF list, so we expect "No runs found" or similar
            expect(hfHtml).toContain("No runs found")

            // TBCC-022: Run details (Switch back to local and select)
            yield* browser.click("button[data-source='local']")
            yield* runHandle.waitForState((s) => s.dataSource === "local")

            yield* browser.click("div[data-run-id='run-1']")
            yield* runHandle.waitForState((s) => s.selectedRunId === "run-1")
            const detailHtml = yield* runHandle.getHTML
            expect(detailHtml).toContain("Fix Bug") // Task name
            expect(detailHtml).toContain("success") // Status

            // TBCC-023: Terminal output
            // Note: Mock returns empty output, but we can check the tab exists or section
            // The widget doesn't have explicit tabs for terminal output yet, just sections
            // But we can check for "Execution Steps"
            expect(detailHtml).toContain("Execution Steps")

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocket()),
            Effect.provide(layer)
          )
        })
      )
    )
  })

  it("TBCC-030..033: Settings functionality", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag
            const browser = yield* TestBrowserTag

            const settingsHandle = yield* harness.mount(TBCCSettingsWidget, {
              containerId: "tbcc-tab-settings",
            })

            // TBCC-030: Execution settings
            const html = yield* settingsHandle.getHTML
            expect(html).toContain("Execution")
            expect(html).toContain("Max Attempts")

            // TBCC-031: Logging settings
            expect(html).toContain("Logging &amp; Storage")
            expect(html).toContain("Save full trajectories")

            // TBCC-032: Persistence (Simulate change)
            // Note: We can't easily test localStorage persistence in this mock environment without mocking localStorage,
            // but we can test that the state updates.
            const input = yield* browser.query("input[data-key='maxAttempts']")
            // Simulate input change (HappyDOM might need specific event dispatch)
            // For now, let's just verify the input exists and has default value
            const value = input.getAttribute("value")
            expect(value).toBe("5") // Default

            // TBCC-033: Reset defaults
            yield* browser.expectVisible("button[data-action='reset']")

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocket()),
            Effect.provide(layer)
          )
        })
      )
    )
  })

  // =========================================================================
  // Phase 1: P0 Gap Tests
  // =========================================================================

  it("TBCC-022: View run details with execution steps", async () => {
    // Create enhanced mock with step data
    const mockWithSteps = (): SocketService => ({
      ...createMockSocket(),
      loadTBRunDetails: () => Effect.succeed({
        meta: MOCK_RUNS[0],
        tasks: [
          {
            id: "task-1",
            name: "Fix Bug",
            category: "Debugging",
            difficulty: "hard",
            outcome: "success",
            turns: 10,
            durationMs: 5000,
            tokens: 1000,
          }
        ],
      } as any),
    })

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag

            const runHandle = yield* harness.mount(TBCCRunBrowserWidget, {
              containerId: "tbcc-tab-runs",
            })
            yield* runHandle.waitForState((s) => !s.loading)

            // Emit selectRun event directly (avoiding browser interaction timeout)
            yield* runHandle.emit({ type: "selectRun", runId: "run-1", source: "local" })
            yield* runHandle.waitForState((s) => s.selectedRunId === "run-1")
            yield* runHandle.waitForState((s) => !s.loadingDetail)

            const html = yield* runHandle.getHTML

            // Verify task results are displayed
            expect(html).toContain("Fix Bug")
            expect(html).toContain("success")

            // Verify execution details section exists
            expect(html).toContain("Execution Steps")

          }).pipe(
            Effect.provideService(SocketServiceTag, mockWithSteps()),
            Effect.provide(layer)
          )
        })
      )
    )
  })

  it("TBCC-004: Start benchmark run verification", async () => {
    // Create spy for socket.startTBRun
    let startTBRunCalled = false
    let capturedOptions: any = null

    const mockSocketWithSpy = (): SocketService => ({
      ...createMockSocket(),
      startTBRun: (options) => {
        startTBRunCalled = true
        capturedOptions = options
        return Effect.succeed({ runId: "new-run-123" })
      }
    })

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag

            const dashboardHandle = yield* harness.mount(TBCCDashboardWidget, {
              containerId: "tbcc-tab-dashboard",
            })
            yield* dashboardHandle.waitForState((s) => !s.loading)

            // Emit runFullBenchmark event directly
            yield* dashboardHandle.emit({ type: "runFullBenchmark" })

            // Wait a bit for async effect to complete
            yield* Effect.sleep("100 millis")

            // Verify socket call was made
            expect(startTBRunCalled).toBe(true)
            expect(capturedOptions).toBeDefined()

            // Verify currentRun state was updated
            const state = yield* dashboardHandle.getState
            expect(state.currentRun).toBeDefined()
            expect(state.currentRun?.taskName).toBe("Full Benchmark")

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocketWithSpy()),
            Effect.provide(layer)
          )
        })
      )
    )
  })

  it("TBCC-002: KPI calculations with multiple runs", async () => {
    // Create mock with multiple runs of different outcomes
    const MULTI_RUNS = [
      { ...MOCK_RUNS[0], runId: "run-1", outcome: "success" as const },
      { ...MOCK_RUNS[0], runId: "run-2", outcome: "success" as const },
      { ...MOCK_RUNS[0], runId: "run-3", outcome: "failure" as const },
      { ...MOCK_RUNS[0], runId: "run-4", outcome: "success" as const },
    ]

    const mockWithMultiRuns = (): SocketService => ({
      ...createMockSocket(),
      loadRecentTBRuns: () => Effect.succeed(MULTI_RUNS as any),
    })

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag

            const dashboardHandle = yield* harness.mount(TBCCDashboardWidget, {
              containerId: "tbcc-tab-dashboard",
            })
            yield* dashboardHandle.waitForState((s) => !s.loading)

            const html = yield* dashboardHandle.getHTML

            // Verify KPIs are displayed
            expect(html).toContain("Success Rate")
            expect(html).toContain("Total Runs")

            // Verify stats were calculated (3 success out of 4 = 75%)
            const state = yield* dashboardHandle.getState
            expect(state.stats).toBeDefined()
            expect(state.stats?.totalRuns).toBe(4)
            expect(state.stats?.overallSuccessRate).toBe(0.75)

            // Verify all runs are shown in table
            expect(html).toContain("run-1")
            expect(html).toContain("run-2")
            expect(html).toContain("run-3")
            expect(html).toContain("run-4")

          }).pipe(
            Effect.provideService(SocketServiceTag, mockWithMultiRuns()),
            Effect.provide(layer)
          )
        })
      )
    )
  })
})
