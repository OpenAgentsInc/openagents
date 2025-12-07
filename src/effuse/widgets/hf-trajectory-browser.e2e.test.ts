/**
 * HuggingFace Trajectory Browser E2E Tests
 *
 * Tests the full integration of list + detail widgets with Happy-DOM.
 */

import { describe, test, expect } from "bun:test"
import { Effect, Layer } from "effect"
import * as FileSystem from "@effect/platform/FileSystem"
import { makeHappyDomLayer } from "../testing/layers/happy-dom.js"
import { TestHarnessTag } from "../testing/index.js"
import { HFTrajectoryListWidget } from "./hf-trajectory-list.js"
import { HFTrajectoryDetailWidget } from "./hf-trajectory-detail.js"
import { OpenThoughtsService, type IOpenThoughtsService } from "../../huggingface/openthoughts.js"
import type { Trajectory } from "../../atif/schema.js"
import { HFDatasetError } from "../../huggingface/schema.js"

// ============================================================================
// Mock Service
// ============================================================================

const createMockTrajectory = (index: number): Trajectory => ({
  schema_version: "ATIF-v1.4",
  session_id: `session-${index}`,
  agent: {
    name: `agent-${index % 3}`,
    version: "1.0",
    model_name: `model-${index % 2}`,
  },
  steps: [
    {
      step_id: 1,
      timestamp: new Date(2024, 0, 1 + index).toISOString(),
      source: "user",
      message: "User message",
    },
    {
      step_id: 2,
      timestamp: new Date(2024, 0, 1 + index).toISOString(),
      source: "agent",
      message: "Agent response",
      tool_calls: [
        {
          tool_call_id: "call-1",
          function_name: "testTool",
          arguments: { arg: "value" },
        },
      ],
    },
  ],
  extra: {
    task: `task-${index % 5}`,
    episode: `episode-${index}`,
    date: new Date(2024, 0, 1 + index).toISOString(),
  },
})

const makeMockOpenThoughtsService = (totalCount: number): IOpenThoughtsService => ({
  ensureDownloaded: () => Effect.succeed("/mock/path"),
  count: () => Effect.succeed(totalCount),
  getTrajectory: (index: number) =>
    index >= 0 && index < totalCount
      ? Effect.succeed(createMockTrajectory(index))
      : Effect.succeed(null),
  getTrajectoryByRunId: (_runId: string) => Effect.succeed(null),
  getTrajectories: (offset = 0, limit = 100) => {
    const start = Math.max(0, offset)
    const end = Math.min(totalCount, start + limit)
    const trajectories: Trajectory[] = []
    for (let i = start; i < end; i++) {
      trajectories.push(createMockTrajectory(i))
    }
    return Effect.succeed(trajectories)
  },
  streamTrajectories: () =>
    Effect.fail(new HFDatasetError("invalid_config", "Mock: streaming not implemented")),
  getParquetPath: () => Effect.succeed("/mock/path.parquet"),
})

// ============================================================================
// E2E Tests
// ============================================================================

describe("HF Trajectory Browser E2E", () => {
  test("mounts list widget and shows trajectories", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()
          const mockService = makeMockOpenThoughtsService(150)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const fsLayer = Layer.succeed(FileSystem.FileSystem, {} as any)
          const fullLayer = Layer.mergeAll(layer, serviceLayer, fsLayer)

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag
            // const browser = yield* TestBrowserTag

            console.log("[E2E] Mounting list widget...")
            const listHandle = yield* harness.mount(HFTrajectoryListWidget, {
              containerId: "hf-list",
            })

            // Wait for initial load
            console.log("[E2E] Waiting for list to load...")
            yield* Effect.sleep(200)

            // Check that list rendered
            const listHTML = yield* listHandle.getHTML
            console.log("[E2E] List HTML length:", listHTML.length)
            console.log("[E2E] List HTML sample:", listHTML.slice(0, 500))

            expect(listHTML).toContain("Trajectories")

            // Check that trajectories are showing
            const state = yield* listHandle.getState
            console.log("[E2E] List state:", {
              totalCount: state.totalCount,
              trajectoriesLength: state.trajectories.length,
              loading: state.loading,
              error: state.error,
            })

            expect(state.loading).toBe(false)
            expect(state.totalCount).toBe(150)
            expect(state.trajectories.length).toBeGreaterThan(0)
          }).pipe(Effect.provide(fullLayer))
        })
      )
    )
  })

  test("mounts detail widget and loads trajectory on selection", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()
          const mockService = makeMockOpenThoughtsService(150)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const fsLayer = Layer.succeed(FileSystem.FileSystem, {} as any)
          const fullLayer = Layer.mergeAll(layer, serviceLayer, fsLayer)

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag

            console.log("[E2E] Mounting list and detail widgets...")
            const listHandle = yield* harness.mount(HFTrajectoryListWidget, {
              containerId: "hf-list",
            })

            const detailHandle = yield* harness.mount(HFTrajectoryDetailWidget, {
              containerId: "hf-detail",
            })

            // Wait for list to load
            yield* Effect.sleep(200)

            // Emit a selection event from list
            console.log("[E2E] Emitting selection event...")
            yield* listHandle.emit({
              type: "select",
              sessionId: "session-5",
              index: 5,
            })

            // Wait for detail to process
            yield* Effect.sleep(100)

            // Check detail state
            const detailState = yield* detailHandle.getState
            console.log("[E2E] Detail state:", {
              sessionId: detailState.sessionId,
              hasTrajectory: !!detailState.trajectory,
              loading: detailState.loading,
              error: detailState.error,
            })

            // Detail widget doesn't automatically load - that's handled by effuse-main
            // So we need to emit a load event directly
            const trajectory = createMockTrajectory(5)
            yield* detailHandle.emit({
              type: "load",
              sessionId: "session-5",
              trajectory,
            })

            yield* Effect.sleep(50)

            const updatedDetailState = yield* detailHandle.getState
            console.log("[E2E] Updated detail state:", {
              sessionId: updatedDetailState.sessionId,
              hasTrajectory: !!updatedDetailState.trajectory,
            })

            expect(updatedDetailState.sessionId).toBe("session-5")
            expect(updatedDetailState.trajectory).toBeDefined()
            expect(updatedDetailState.trajectory?.session_id).toBe("session-5")
          }).pipe(Effect.provide(fullLayer))
        })
      )
    )
  })

  test("full integration: list selection → detail load", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeHappyDomLayer()
          const mockService = makeMockOpenThoughtsService(150)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const fullLayer = Layer.mergeAll(
            layer,
            serviceLayer,
            Layer.succeed(FileSystem.FileSystem, {} as unknown as FileSystem.FileSystem)
          )

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag
            const openThoughtsService = yield* OpenThoughtsService

            console.log("[E2E] Full integration test starting...")

            // Mount both widgets
            const listHandle = yield* harness.mount(HFTrajectoryListWidget, {
              containerId: "hf-list",
            })

            const detailHandle = yield* harness.mount(HFTrajectoryDetailWidget, {
              containerId: "hf-detail",
            })

            // Wait for list to load
            yield* Effect.sleep(200)

            // Simulate the event forwarding from effuse-main.ts
            // When list emits select, we fetch trajectory and send to detail
            const selectedIndex = 10

            console.log("[E2E] Simulating list selection...")
            yield* listHandle.emit({
              type: "select",
              sessionId: `session-${selectedIndex}`,
              index: selectedIndex,
            })

            // Fetch trajectory (simulating effuse-main.ts behavior)
            console.log("[E2E] Fetching trajectory...")
            const trajectory = yield* openThoughtsService.getTrajectory(selectedIndex)

            // Send to detail widget
            console.log("[E2E] Loading trajectory in detail widget...")
            yield* detailHandle.emit({
              type: "load",
              sessionId: `session-${selectedIndex}`,
              trajectory: trajectory!,
            })

            yield* Effect.sleep(100)

            // Verify detail widget loaded the trajectory
            const detailState = yield* detailHandle.getState
            console.log("[E2E] Final detail state:", {
              sessionId: detailState.sessionId,
              stepCount: detailState.trajectory?.steps.length,
            })

            expect(detailState.sessionId).toBe(`session-${selectedIndex}`)
            expect(detailState.trajectory).toBeDefined()
            expect(detailState.trajectory?.steps.length).toBe(2)

            // Check rendered HTML
            const detailHTML = yield* detailHandle.getHTML
            console.log("[E2E] Detail HTML includes session:", detailHTML.includes(`session-${selectedIndex}`))
            expect(detailHTML).toContain(`session-${selectedIndex}`)
          }).pipe(Effect.provide(fullLayer))
        })
      )
    )
  })
})
