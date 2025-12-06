/**
 * Category Tree Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { CategoryTreeWidget, type CategoryTreeState, type TBTaskData } from "./category-tree.js"
import { mountWidget } from "../widget/mount.js"
import { makeCustomTestLayer, makeTestLayer } from "../layers/test.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"

describe("CategoryTreeWidget", () => {
  test("renders hidden when not visible", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("hidden")
        })
      )
    )
  })

  test("renders empty state when visible with no tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "category-tree-test" } as Element

          const customWidget = {
            ...CategoryTreeWidget,
            initialState: (): CategoryTreeState => ({
              tasks: new Map(),
              collapsedCategories: new Set(),
              visible: true,
              selectedTaskId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Categories")
          expect(html).toContain("No tasks loaded")
        })
      )
    )
  })

  test("renders tasks grouped by category", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "category-tree-test" } as Element

          const mockTasks = new Map<string, TBTaskData>([
            ["task-001", { id: "task-001", name: "Easy task", difficulty: "easy", category: "basics", status: "passed" }],
            ["task-002", { id: "task-002", name: "Medium task", difficulty: "medium", category: "basics", status: "failed" }],
            ["task-003", { id: "task-003", name: "Hard task", difficulty: "hard", category: "advanced", status: "pending" }],
          ])

          const customWidget = {
            ...CategoryTreeWidget,
            initialState: (): CategoryTreeState => ({
              tasks: mockTasks,
              collapsedCategories: new Set(),
              visible: true,
              selectedTaskId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("basics")
          expect(html).toContain("advanced")
          expect(html).toContain("Easy task")
          expect(html).toContain("Medium task")
          expect(html).toContain("Hard task")
        })
      )
    )
  })

  test("renders status icons correctly", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "category-tree-test" } as Element

          const mockTasks = new Map<string, TBTaskData>([
            ["task-001", { id: "task-001", name: "Passed", difficulty: "easy", category: "test", status: "passed" }],
            ["task-002", { id: "task-002", name: "Failed", difficulty: "easy", category: "test", status: "failed" }],
            ["task-003", { id: "task-003", name: "Running", difficulty: "easy", category: "test", status: "running" }],
            ["task-004", { id: "task-004", name: "Pending", difficulty: "easy", category: "test", status: "pending" }],
          ])

          const customWidget = {
            ...CategoryTreeWidget,
            initialState: (): CategoryTreeState => ({
              tasks: mockTasks,
              collapsedCategories: new Set(),
              visible: true,
              selectedTaskId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("✓") // Passed
          expect(html).toContain("✗") // Failed
          expect(html).toContain("▶") // Running
          expect(html).toContain("○") // Pending
        })
      )
    )
  })

  test("renders category stats", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "category-tree-test" } as Element

          const mockTasks = new Map<string, TBTaskData>([
            ["task-001", { id: "task-001", name: "Task 1", difficulty: "easy", category: "test", status: "passed" }],
            ["task-002", { id: "task-002", name: "Task 2", difficulty: "easy", category: "test", status: "passed" }],
            ["task-003", { id: "task-003", name: "Task 3", difficulty: "easy", category: "test", status: "failed" }],
          ])

          const customWidget = {
            ...CategoryTreeWidget,
            initialState: (): CategoryTreeState => ({
              tasks: mockTasks,
              collapsedCategories: new Set(),
              visible: true,
              selectedTaskId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("✓2") // 2 passed
          expect(html).toContain("✗1") // 1 failed
        })
      )
    )
  })

  test("hides tasks when category is collapsed", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "category-tree-test" } as Element

          const mockTasks = new Map<string, TBTaskData>([
            ["task-001", { id: "task-001", name: "Hidden task", difficulty: "easy", category: "collapsed-cat", status: "pending" }],
          ])

          const customWidget = {
            ...CategoryTreeWidget,
            initialState: (): CategoryTreeState => ({
              tasks: mockTasks,
              collapsedCategories: new Set(["collapsed-cat"]),
              visible: true,
              selectedTaskId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("collapsed-cat")
          expect(html).toContain("▶") // Collapsed chevron
          expect(html).not.toContain("Hidden task") // Task should be hidden
        })
      )
    )
  })

  test("renders difficulty badges", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "category-tree-test" } as Element

          const mockTasks = new Map<string, TBTaskData>([
            ["task-001", { id: "task-001", name: "Easy", difficulty: "easy", category: "test", status: "pending" }],
            ["task-002", { id: "task-002", name: "Medium", difficulty: "medium", category: "test", status: "pending" }],
            ["task-003", { id: "task-003", name: "Hard", difficulty: "hard", category: "test", status: "pending" }],
          ])

          const customWidget = {
            ...CategoryTreeWidget,
            initialState: (): CategoryTreeState => ({
              tasks: mockTasks,
              collapsedCategories: new Set(),
              visible: true,
              selectedTaskId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("E") // Easy
          expect(html).toContain("M") // Medium
          expect(html).toContain("H") // Hard
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = CategoryTreeWidget.initialState()

    expect(state.tasks.size).toBe(0)
    expect(state.collapsedCategories.size).toBe(0)
    expect(state.visible).toBe(false)
    expect(state.selectedTaskId).toBeNull()
  })

  test("US-2.1 shows suite tasks when tb_suite_info arrives", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          // Mount widget with live subscriptions
          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          // Inject suite info
          yield* injectMessage({
            type: "tb_suite_info",
            suiteName: "terminal-bench-v1",
            suiteVersion: "1.0.0",
            tasks: [
              { id: "task-001", name: "Task One", difficulty: "easy", category: "basics" },
              { id: "task-002", name: "Task Two", difficulty: "hard", category: "advanced" },
            ],
          })

          // Allow subscription to process
          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("Task One")
          expect(html).toContain("Task Two")
          expect(html).toContain("Categories")
        })
      )
    )
  })

  test("US-2.3 expand all clears collapsed categories", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "category-tree-test" } as Element
              const tasks = new Map<string, TBTaskData>([
                ["task-001", { id: "task-001", name: "Task 1", difficulty: "easy", category: "basics", status: "pending" }],
                ["task-002", { id: "task-002", name: "Task 2", difficulty: "hard", category: "advanced", status: "pending" }],
              ])
              const state = yield* stateService.cell({
                tasks,
                collapsedCategories: new Set(["basics", "advanced"]),
                visible: true,
                selectedTaskId: null,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              yield* CategoryTreeWidget.handleEvent({ type: "expandAll" }, ctx)

              const updated = yield* state.get
              expect(updated.collapsedCategories.size).toBe(0)
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.4 collapse all hides all categories", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "category-tree-test" } as Element
              const tasks = new Map<string, TBTaskData>([
                ["task-001", { id: "task-001", name: "Task 1", difficulty: "easy", category: "basics", status: "pending" }],
              ])
              const state = yield* stateService.cell({
                tasks,
                collapsedCategories: new Set(),
                visible: true,
                selectedTaskId: null,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              yield* CategoryTreeWidget.handleEvent({ type: "collapseAll" }, ctx)

              const updated = yield* state.get
              expect(updated.collapsedCategories.has("basics")).toBe(true)
              const html = (yield* CategoryTreeWidget.render(ctx)).toString()
              expect(html).not.toContain("Task 1")
            }),
            layer
          )
        })
      )
    )
  })
})
