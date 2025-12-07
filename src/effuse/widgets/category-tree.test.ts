/**
 * Category Tree Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { CategoryTreeWidget, type CategoryTreeState, type CategoryTreeEvent, type TBTaskData } from "./category-tree.js"
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
            name: "terminal-bench-v1",
            version: "1.0.0",
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

          return yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "category-tree-test" } as Element
              const tasks = new Map<string, TBTaskData>([
                ["task-001", { id: "task-001", name: "Task 1", difficulty: "easy", category: "basics", status: "pending" as const }],
                ["task-002", { id: "task-002", name: "Task 2", difficulty: "hard", category: "advanced", status: "pending" as const }],
              ])
              const state = yield* stateService.cell<CategoryTreeState>({
                tasks,
                collapsedCategories: new Set(["basics", "advanced"]),
                visible: true,
                selectedTaskId: null,
              })
              const ctx = { state, emit: (_event: CategoryTreeEvent) => Effect.succeed(undefined), dom, container }

              if (CategoryTreeWidget.handleEvent) {
                yield* CategoryTreeWidget.handleEvent({ type: "expandAll" }, ctx)
              }

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

          return yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "category-tree-test" } as Element
              const tasks = new Map<string, TBTaskData>([
                ["task-001", { id: "task-001", name: "Task 1", difficulty: "easy" as const, category: "basics", status: "pending" as const }],
              ])
              const state = yield* stateService.cell<CategoryTreeState>({
                tasks,
                collapsedCategories: new Set(),
                visible: true,
                selectedTaskId: null,
              })
              const ctx = { state, emit: (_event: CategoryTreeEvent) => Effect.succeed(undefined), dom, container }

              if (CategoryTreeWidget.handleEvent) {
                yield* CategoryTreeWidget.handleEvent({ type: "collapseAll" }, ctx)
              }

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

  test("US-2.2 toggles category collapse state", async () => {
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
              const state = yield* stateService.cell<CategoryTreeState>({
                tasks,
                collapsedCategories: new Set(["basics"]),
                visible: true,
                selectedTaskId: null,
              })
              const ctx = { state, emit: (_event: CategoryTreeEvent) => Effect.succeed(undefined), dom, container }

              // Expand basics
              if (CategoryTreeWidget.handleEvent) {
                yield* CategoryTreeWidget.handleEvent({ type: "toggleCategory", category: "basics" }, ctx)
              }
              const expanded = yield* state.get
              expect(expanded.collapsedCategories.has("basics")).toBe(false)

              // Collapse again
              if (CategoryTreeWidget.handleEvent) {
                yield* CategoryTreeWidget.handleEvent({ type: "toggleCategory", category: "basics" }, ctx)
              }
              const collapsed = yield* state.get
              expect(collapsed.collapsedCategories.has("basics")).toBe(true)
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.2 shows stats when category is collapsed", async () => {
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
                ["task-pass", { id: "task-pass", name: "Task Pass", difficulty: "easy", category: "core", status: "passed" }],
                ["task-fail", { id: "task-fail", name: "Task Fail", difficulty: "medium", category: "core", status: "failed" }],
              ])
              const state = yield* stateService.cell<CategoryTreeState>({
                tasks,
                collapsedCategories: new Set(["core"]),
                visible: true,
                selectedTaskId: null,
              })
              const ctx = { state, emit: (_event: CategoryTreeEvent) => Effect.succeed(undefined), dom, container }

              const html = (yield* Effect.provide(CategoryTreeWidget.render(ctx), layer)).toString()
              expect(html).toContain("✓1")
              expect(html).toContain("✗1")
              expect(html).not.toContain("Task Pass")
              expect(html).not.toContain("Task Fail")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.10 highlights selected task in the list", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { getRendered } = yield* makeTestLayer()

          const stateService = yield* StateServiceTag
          const dom = yield* DomServiceTag
          const container = { id: "category-tree-test" } as Element
          const tasks = new Map<string, TBTaskData>([
            ["task-1", { id: "task-1", name: "Task One", difficulty: "easy", category: "core", status: "pending" }],
            ["task-2", { id: "task-2", name: "Task Two", difficulty: "medium", category: "core", status: "pending" }],
          ])
          const state = yield* stateService.cell<CategoryTreeState>({
            tasks,
            collapsedCategories: new Set(),
            visible: true,
            selectedTaskId: null,
          } as CategoryTreeState)
          const ctx = { state, emit: (_event: CategoryTreeEvent) => Effect.succeed(undefined), dom, container }

          if (CategoryTreeWidget.handleEvent) {
            yield* CategoryTreeWidget.handleEvent({ type: "selectTask", taskId: "task-2" }, ctx)
          }
          const updated = yield* state.get
          expect(updated.selectedTaskId).toBe("task-2")

          const html = (yield* CategoryTreeWidget.render(ctx)).toString()
          expect(html).toContain("Task One")
          expect(html).toContain("Task Two")
          expect(html).toContain("bg-zinc-800/60") // Selected highlight class
        })
      )
    )
  })

  test("US-4.5 updates status icons on task start and completion", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [
              { id: "task-001", name: "First", difficulty: "easy", category: "basics" },
              { id: "task-002", name: "Second", difficulty: "hard", category: "advanced" },
            ],
          })

          yield* Effect.sleep(10)

          yield* injectMessage({
            type: "tb_task_start",
            runId: "run-1",
            taskId: "task-001",
            taskName: "First",
            difficulty: "easy",
            category: "basics",
            taskIndex: 1,
            totalTasks: 2,
          })

          yield* Effect.sleep(10)

          let html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("▶")

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-1",
            taskId: "task-001",
            outcome: "success",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })

          yield* Effect.sleep(10)

          html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("✓")
        })
      )
    )
  })

  test("US-14.6 marks timeout tasks with timeout icon", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [{ id: "task-timeout", name: "Timeout task", difficulty: "medium", category: "core" }],
          })

          yield* Effect.sleep(0)

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-timeout",
            taskId: "task-timeout",
            outcome: "timeout",
            durationMs: 5000,
            turns: 0,
            tokens: 0,
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("⏱")
          expect(html).toContain("Timeout task")
        })
      )
    )
  })

  test("US-4.5 shows failed icon when task finishes with failure", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [{ id: "task-fail", name: "Failing task", difficulty: "hard", category: "core" }],
          })

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-fail",
            taskId: "task-fail",
            outcome: "failure",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("✗")
          expect(html).toContain("Failing task")
        })
      )
    )
  })

  test("US-4.5 shows error icon when task completes with error outcome", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [{ id: "task-error", name: "Error task", difficulty: "medium", category: "core" }],
          })

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-error",
            taskId: "task-error",
            outcome: "error",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("⚠")
          expect(html).toContain("Error task")
        })
      )
    )
  })

  test("US-4.4 shows category pass/fail counts as tasks complete", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [
              { id: "task-pass", name: "Pass task", difficulty: "easy", category: "alpha" },
              { id: "task-fail", name: "Fail task", difficulty: "hard", category: "beta" },
            ],
          })

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-1",
            taskId: "task-pass",
            outcome: "success",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-1",
            taskId: "task-fail",
            outcome: "failure",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("alpha")
          expect(html).toContain("beta")
          expect(html).toContain("✓1")
          expect(html).toContain("✗1")
        })
      )
    )
  })

  test("US-4.3 updates pass/fail counts in real-time as tasks complete", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          // Load suite
          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [
              { id: "task-1", name: "Task 1", difficulty: "easy", category: "test" },
              { id: "task-2", name: "Task 2", difficulty: "easy", category: "test" },
              { id: "task-3", name: "Task 3", difficulty: "easy", category: "test" },
            ],
          })
          yield* Effect.sleep(0)

          // Initially all pending
          let html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("test")

          // First task passes
          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-1",
            taskId: "task-1",
            outcome: "success",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })
          yield* Effect.sleep(0)

          html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("✓1") // 1 passed

          // Second task fails
          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-1",
            taskId: "task-2",
            outcome: "failure",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })
          yield* Effect.sleep(0)

          html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("✓1") // Still 1 passed
          expect(html).toContain("✗1") // Now 1 failed

          // Third task passes
          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-1",
            taskId: "task-3",
            outcome: "success",
            durationMs: 1000,
            turns: 10,
            tokens: 100,
          })
          yield* Effect.sleep(0)

          html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("✓2") // Now 2 passed
          expect(html).toContain("✗1") // Still 1 failed
        })
      )
    )
  })

  test("US-6.2 shows per-task results with status icons and names", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          // Load suite with tasks in different categories
          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [
              { id: "pass-task", name: "Passed Task", difficulty: "easy", category: "alpha" },
              { id: "fail-task", name: "Failed Task", difficulty: "medium", category: "beta" },
              { id: "timeout-task", name: "Timeout Task", difficulty: "hard", category: "alpha" },
            ],
          })
          yield* Effect.sleep(0)

          // Complete all tasks with different outcomes
          yield* injectMessage({ type: "tb_task_complete", runId: "run-1", taskId: "pass-task", outcome: "success", durationMs: 1000, turns: 10, tokens: 100 })
          yield* injectMessage({ type: "tb_task_complete", runId: "run-1", taskId: "fail-task", outcome: "failure", durationMs: 1000, turns: 10, tokens: 100 })
          yield* injectMessage({ type: "tb_task_complete", runId: "run-1", taskId: "timeout-task", outcome: "timeout", durationMs: 5000, turns: 0, tokens: 0 })
          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""

          // Verify all tasks are visible with correct icons
          expect(html).toContain("Passed Task")
          expect(html).toContain("✓") // passed icon
          expect(html).toContain("Failed Task")
          expect(html).toContain("✗") // failed icon
          expect(html).toContain("Timeout Task")
          expect(html).toContain("⏱") // timeout icon

          // Verify categories are shown
          expect(html).toContain("alpha")
          expect(html).toContain("beta")
        })
      )
    )
  })

  test("US-2.11 shows difficulty badges for each task", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "category-tree-test" } as Element

          yield* mountWidget(CategoryTreeWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "tb_suite_info",
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [
              { id: "easy", name: "Easy task", difficulty: "easy", category: "alpha" },
              { id: "medium", name: "Medium task", difficulty: "medium", category: "alpha" },
              { id: "hard", name: "Hard task", difficulty: "hard", category: "beta" },
            ],
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("E</span>")
          expect(html).toContain("M</span>")
          expect(html).toContain("H</span>")
        })
      )
    )
  })
})
