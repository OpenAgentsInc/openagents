import { describe, it, expect, mock } from "bun:test"
import { Effect, Layer, Stream } from "effect"
import { makeHappyDomLayer } from "../../testing/layers/happy-dom.js"
import { TestHarnessTag, TestBrowserTag } from "../../testing/index.js"
import { SocketServiceTag, SocketError, type SocketService } from "../../services/socket.js"
import {
  TBCCShellWidget,
  TBCCDashboardWidget,
  TBCCTaskBrowserWidget,
  TBCCRunBrowserWidget,
  TBCCSettingsWidget,
} from "./index.js"

// Mock Socket Service
const createMockSocket = (): SocketService => ({
  connect: () => Effect.void,
  disconnect: () => Effect.void,
  isConnected: () => Effect.succeed(true),
  getMessages: () => Stream.empty,
  loadTBSuite: () => Effect.succeed({ name: "Test Suite", version: "1.0", tasks: [] }),
  startTBRun: () => Effect.succeed({ runId: "test-run" }),
  stopTBRun: () => Effect.succeed({ stopped: true }),
  loadRecentTBRuns: () => Effect.succeed([]),
  loadTBRunDetails: () => Effect.succeed(null),
  loadReadyTasks: () => Effect.succeed([]),
  assignTaskToMC: () => Effect.succeed({ assigned: true }),
  loadUnifiedTrajectories: () => Effect.succeed([]),
  getHFTrajectoryCount: () => Effect.succeed(0),
  getHFTrajectories: () => Effect.succeed([]),
})

describe("TB Command Center E2E", () => {
  it("should mount the shell widget and all tabs", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag
            const browser = yield* TestBrowserTag

            // Mount Shell
            const shellHandle = yield* harness.mount(TBCCShellWidget, {
              containerId: "tbcc-shell-widget",
            })

            // Check Shell Render
            const shellHTML = yield* shellHandle.getHTML
            expect(shellHTML).toContain("Dashboard")
            expect(shellHTML).toContain("Tasks")
            expect(shellHTML).toContain("Runs")
            expect(shellHTML).toContain("Settings")

            // Mount Dashboard (simulating effuse-main.ts mounting it)
            const dashboardHandle = yield* harness.mount(TBCCDashboardWidget, {
              containerId: "tbcc-tab-dashboard",
            })
            // Wait for loading to finish
            yield* dashboardHandle.waitForState((s) => !s.loading)
            const dashboardHTML = yield* dashboardHandle.getHTML
            expect(dashboardHTML).toContain("Dashboard")

            // Mount Task Browser
            const taskHandle = yield* harness.mount(TBCCTaskBrowserWidget, {
              containerId: "tbcc-tab-tasks",
            })
            // Wait for loading to finish
            yield* taskHandle.waitForState((s) => !s.loading)
            const taskHTML = yield* taskHandle.getHTML
            expect(taskHTML).toContain("Search tasks...")

            // Mount Run Browser
            const runHandle = yield* harness.mount(TBCCRunBrowserWidget, {
              containerId: "tbcc-tab-runs",
            })
            // Wait for loading to finish
            yield* runHandle.waitForState((s) => !s.loading)
            const runHTML = yield* runHandle.getHTML
            expect(runHTML).toContain("Run History")

            // Mount Settings
            const settingsHandle = yield* harness.mount(TBCCSettingsWidget, {
              containerId: "tbcc-tab-settings",
            })
            const settingsHTML = yield* settingsHandle.getHTML
            expect(settingsHTML).toContain("Settings")

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocket()),
            Effect.provide(layer)
          )
        })
      )
    )
  })
})
