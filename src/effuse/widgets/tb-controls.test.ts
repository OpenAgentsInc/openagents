/**
 * TB Controls Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { TBControlsWidget, type TBControlsState, type TBSuiteInfo } from "./tb-controls.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"

describe("TBControlsWidget", () => {
  test("renders initial state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-controls-test" } as Element

          yield* mountWidget(TBControlsWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Terminal-Bench")
          expect(html).toContain("Ready")
          expect(html).toContain("Load")
          expect(html).toContain("Start")
          expect(html).toContain("Random")
          expect(html).toContain("Stop")
        })
      )
    )
  })

  test("renders with loaded suite", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-controls-test" } as Element

          const mockSuite: TBSuiteInfo = {
            name: "test-suite",
            version: "1.0.0",
            tasks: [
              { id: "task-001", name: "Easy task", difficulty: "easy", category: "basics" },
              { id: "task-002", name: "Medium task", difficulty: "medium", category: "core" },
              { id: "task-003", name: "Hard task", difficulty: "hard", category: "advanced" },
            ],
          }

          const customWidget = {
            ...TBControlsWidget,
            initialState: (): TBControlsState => ({
              suitePath: "/path/to/suite",
              suite: mockSuite,
              selectedTaskIds: new Set(["task-001", "task-002", "task-003"]),
              loading: false,
              status: "Ready",
              statusType: "idle",
              isRunning: false,
              runId: null,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("test-suite")
          expect(html).toContain("v1.0.0")
          expect(html).toContain("Easy task")
          expect(html).toContain("Medium task")
          expect(html).toContain("Hard task")
          expect(html).toContain("3/3 selected")
        })
      )
    )
  })

  test("renders running state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-controls-test" } as Element

          const mockSuite: TBSuiteInfo = {
            name: "test-suite",
            version: "1.0.0",
            tasks: [{ id: "task-001", name: "Test task", difficulty: "easy", category: "test" }],
          }

          const customWidget = {
            ...TBControlsWidget,
            initialState: (): TBControlsState => ({
              suitePath: "/path/to/suite",
              suite: mockSuite,
              selectedTaskIds: new Set(["task-001"]),
              loading: false,
              status: "Running...",
              statusType: "running",
              isRunning: true,
              runId: "run-12345",
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Running...")
          expect(html).toContain("text-blue-400") // Running status color
          // Stop button should be enabled
          expect(html).toContain('data-action="stopRun"')
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-controls-test" } as Element

          const customWidget = {
            ...TBControlsWidget,
            initialState: (): TBControlsState => ({
              suitePath: "",
              suite: null,
              selectedTaskIds: new Set(),
              loading: false,
              status: "Ready",
              statusType: "idle",
              isRunning: false,
              runId: null,
              collapsed: true,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Terminal-Bench")
          expect(html).toContain("▼") // Collapsed indicator
          // Should not contain controls when collapsed
          expect(html).not.toContain("Path to suite")
        })
      )
    )
  })

  test("renders error state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-controls-test" } as Element

          const customWidget = {
            ...TBControlsWidget,
            initialState: (): TBControlsState => ({
              suitePath: "/invalid/path",
              suite: null,
              selectedTaskIds: new Set(),
              loading: false,
              status: "Load failed",
              statusType: "error",
              isRunning: false,
              runId: null,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Load failed")
          expect(html).toContain("text-red-400") // Error status color
        })
      )
    )
  })

  test("renders difficulty badges correctly", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-controls-test" } as Element

          const mockSuite: TBSuiteInfo = {
            name: "test-suite",
            version: "1.0.0",
            tasks: [
              { id: "task-001", name: "Easy", difficulty: "easy", category: "test" },
              { id: "task-002", name: "Medium", difficulty: "medium", category: "test" },
              { id: "task-003", name: "Hard", difficulty: "hard", category: "test" },
            ],
          }

          const customWidget = {
            ...TBControlsWidget,
            initialState: (): TBControlsState => ({
              suitePath: "/path/to/suite",
              suite: mockSuite,
              selectedTaskIds: new Set(),
              loading: false,
              status: "Ready",
              statusType: "idle",
              isRunning: false,
              runId: null,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("emerald") // Easy
          expect(html).toContain("amber") // Medium
          expect(html).toContain("red") // Hard
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = TBControlsWidget.initialState()

    expect(state.suitePath).toBe("")
    expect(state.suite).toBeNull()
    expect(state.selectedTaskIds.size).toBe(0)
    expect(state.loading).toBe(false)
    expect(state.status).toBe("Ready")
    expect(state.statusType).toBe("idle")
    expect(state.isRunning).toBe(false)
    expect(state.runId).toBeNull()
    expect(state.collapsed).toBe(false)
  })
})
