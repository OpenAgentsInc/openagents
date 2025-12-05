/**
 * MC Tasks Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { MCTasksWidget, type MCTasksState, type MCTask } from "./mc-tasks.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"

describe("MCTasksWidget", () => {
  test("renders empty state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          yield* mountWidget(MCTasksWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("No ready tasks found")
          expect(html).toContain("Ready Tasks (0)")
        })
      )
    )
  })

  test("renders loading state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: [],
              loading: true,
              error: null,
              collapsed: false,
              maxDisplay: 20,
              assigningId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Loading ready tasks...")
        })
      )
    )
  })

  test("renders with tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const mockTasks: MCTask[] = [
            {
              id: "oa-abc123",
              title: "Fix authentication bug",
              description: "Users cannot login",
              status: "open",
              priority: 0,
              type: "bug",
              labels: ["auth", "critical"],
              createdAt: "2024-12-05T10:00:00Z",
              updatedAt: "2024-12-05T10:00:00Z",
            },
            {
              id: "oa-def456",
              title: "Add dark mode support",
              description: "Implement theme switching",
              status: "open",
              priority: 2,
              type: "feature",
              labels: ["ui", "theme"],
              createdAt: "2024-12-05T09:00:00Z",
              updatedAt: "2024-12-05T09:00:00Z",
            },
          ]

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: mockTasks,
              loading: false,
              error: null,
              collapsed: false,
              maxDisplay: 20,
              assigningId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Ready Tasks (2)")
          expect(html).toContain("oa-abc123")
          expect(html).toContain("oa-def456")
          expect(html).toContain("Fix authentication bug")
          expect(html).toContain("Add dark mode support")
          expect(html).toContain("P0") // Priority 0
          expect(html).toContain("P2") // Priority 2
          expect(html).toContain("bug")
          expect(html).toContain("feature")
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const mockTasks: MCTask[] = [
            {
              id: "oa-xyz789",
              title: "Some task",
              description: "Description",
              status: "open",
              priority: 1,
              type: "task",
              labels: [],
              createdAt: "2024-12-05T10:00:00Z",
              updatedAt: "2024-12-05T10:00:00Z",
            },
          ]

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: mockTasks,
              loading: false,
              error: null,
              collapsed: true,
              maxDisplay: 20,
              assigningId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          // Should show count but not task details
          expect(html).toContain("Ready Tasks (1)")
          expect(html).toContain("▼") // Collapsed indicator
          expect(html).not.toContain("Some task") // Content hidden
        })
      )
    )
  })

  test("shows assigning state on button", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const mockTasks: MCTask[] = [
            {
              id: "oa-assigning",
              title: "Task being assigned",
              description: "Description",
              status: "open",
              priority: 1,
              type: "task",
              labels: [],
              createdAt: "2024-12-05T10:00:00Z",
              updatedAt: "2024-12-05T10:00:00Z",
            },
          ]

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: mockTasks,
              loading: false,
              error: null,
              collapsed: false,
              maxDisplay: 20,
              assigningId: "oa-assigning",
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Starting...")
          expect(html).toContain("cursor-not-allowed")
        })
      )
    )
  })

  test("renders error state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: [],
              loading: false,
              error: "Failed to load tasks",
              collapsed: false,
              maxDisplay: 20,
              assigningId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Failed to load tasks")
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = MCTasksWidget.initialState()

    expect(state.tasks).toEqual([])
    expect(state.loading).toBe(false)
    expect(state.error).toBeNull()
    expect(state.collapsed).toBe(false)
    expect(state.maxDisplay).toBe(20)
    expect(state.assigningId).toBeNull()
  })
})
