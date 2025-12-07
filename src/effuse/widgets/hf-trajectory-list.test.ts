/**
 * HFTrajectoryListWidget Tests
 *
 * Tests the HuggingFace trajectory list widget with mock data.
 */

import { describe, test, expect } from "bun:test"
import { Effect, Layer, Ref } from "effect"
import {
  HFTrajectoryListWidget,
  type HFTrajectoryListState,
  type TrajectoryMetadata,
} from "./hf-trajectory-list.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"
import { OpenThoughtsService, type IOpenThoughtsService } from "../../huggingface/openthoughts.js"
import type { Trajectory } from "../../atif/schema.js"
import { HFDatasetError } from "../../huggingface/schema.js"

// ============================================================================
// Mock Data
// ============================================================================

/**
 * Create a mock trajectory for testing
 */
const createMockTrajectory = (index: number): Trajectory => ({
  schema_version: "1.0",
  session_id: `session-${index}`,
  agent: {
    name: `agent-${index % 3}`, // 3 different agents
    version: "1.0",
    model_name: `model-${index % 2}`, // 2 different models
  },
  steps: Array.from({ length: 5 + (index % 10) }, (_, i) => ({
    step_id: i + 1,
    timestamp: new Date(2024, 0, 1 + index).toISOString(),
    source: i % 2 === 0 ? "user" : "agent",
    message: `Step ${i + 1} message`,
  })),
  extra: {
    task: `task-${index % 5}`, // 5 different tasks
    episode: `episode-${index}`,
    date: new Date(2024, 0, 1 + index).toISOString(),
  },
})

/**
 * Create a mock OpenThoughtsService for testing
 */
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
    Effect.fail(new HFDatasetError("not_implemented", "Mock: streaming not implemented")),
  getParquetPath: () => Effect.succeed("/mock/path.parquet"),
})

// ============================================================================
// Tests
// ============================================================================

describe("HFTrajectoryListWidget", () => {
  test("renders initial loading state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(150)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          // Mount widget
          yield* mountWidget(HFTrajectoryListWidget, container).pipe(Effect.provide(layer))

          // Initial state should show loading
          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("HuggingFace Trajectories")
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = HFTrajectoryListWidget.initialState()

    expect(state.trajectories).toEqual([])
    expect(state.filteredTrajectories).toEqual([])
    expect(state.selectedSessionId).toBe(null)
    expect(state.searchQuery).toBe("")
    expect(state.currentPage).toBe(0)
    expect(state.pageSize).toBe(100)
    expect(state.totalCount).toBe(0)
    expect(state.loading).toBe(true)
    expect(state.error).toBe(null)
    expect(state.collapsed).toBe(false)
  })

  test("loads first page on mount", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(250)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          yield* mountWidget(HFTrajectoryListWidget, container).pipe(Effect.provide(layer))

          // Give subscription time to load
          yield* Effect.sleep(100)

          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          // Should show total count
          expect(html).toContain("250")
          // Should show pagination info
          expect(html).toContain("Showing 1-100 of 250")
        })
      )
    )
  })

  test("renders trajectory items with metadata", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(10)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          yield* mountWidget(HFTrajectoryListWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(100)

          const html = yield* getRendered(container)
          // Should show agent names
          expect(html).toContain("agent-0")
          // Should show task info
          expect(html).toContain("task-0")
          // Should show episode info
          expect(html).toContain("episode-0")
          // Should show step count
          expect(html).toContain("steps")
        })
      )
    )
  })

  test("handles pagination controls", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(250)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          yield* mountWidget(HFTrajectoryListWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(100)

          const html = yield* getRendered(container)
          // Page 1 should show prev disabled, next enabled
          expect(html).toContain("Prev")
          expect(html).toContain("Next")
          expect(html).toContain("Page 1")
        })
      )
    )
  })

  test("renders empty state when no trajectories", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(0)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          yield* mountWidget(HFTrajectoryListWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(100)

          const html = yield* getRendered(container)
          expect(html).toContain("No trajectories found")
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(100)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          // Create widget with collapsed initial state
          const collapsedWidget = {
            ...HFTrajectoryListWidget,
            initialState: (): HFTrajectoryListState => ({
              ...HFTrajectoryListWidget.initialState(),
              collapsed: true,
              loading: false,
            }),
          }

          yield* mountWidget(collapsedWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          expect(html).toContain("HuggingFace Trajectories")
          expect(html).toContain("▼") // Collapsed indicator
        })
      )
    )
  })

  test("renders search input", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(100)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          yield* mountWidget(HFTrajectoryListWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(100)

          const html = yield* getRendered(container)
          expect(html).toContain("Search by agent, task, episode")
        })
      )
    )
  })

  test("shows correct page boundaries", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer: baseLayer, getRendered } = yield* makeTestLayer()
          const mockService = makeMockOpenThoughtsService(350)
          const serviceLayer = Layer.succeed(OpenThoughtsService, mockService)
          const layer = Layer.merge(baseLayer, serviceLayer)

          const container = { id: "hf-list-test" } as Element

          yield* mountWidget(HFTrajectoryListWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(100)

          const html = yield* getRendered(container)
          // First page: 1-100 of 350
          expect(html).toContain("Showing 1-100 of 350")
        })
      )
    )
  })
})
