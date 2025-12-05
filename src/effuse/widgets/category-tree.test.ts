/**
 * Category Tree Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { CategoryTreeWidget, type CategoryTreeState, type TBTaskData } from "./category-tree.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"

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
})
