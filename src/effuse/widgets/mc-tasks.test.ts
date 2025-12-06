/**
 * MC Tasks Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { MCTasksWidget, type MCTasksState, type MCTask } from "./mc-tasks.js"
import { mountWidget } from "../widget/mount.js"
import { makeCustomTestLayer, makeTestLayer } from "../layers/test.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"

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

  test("US-10.4 shows priority badges for tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: [
                {
                  id: "oa-p0",
                  title: "Critical bug",
                  description: "",
                  status: "open",
                  priority: 0,
                  type: "bug",
                  labels: [],
                  createdAt: "2024-12-01",
                  updatedAt: "2024-12-01",
                },
                {
                  id: "oa-p3",
                  title: "Small tweak",
                  description: "",
                  status: "open",
                  priority: 3,
                  type: "task",
                  labels: [],
                  createdAt: "2024-12-02",
                  updatedAt: "2024-12-02",
                },
              ],
              loading: false,
              error: null,
              collapsed: false,
              maxDisplay: 20,
              assigningId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("P0")
          expect(html).toContain("P3")
        })
      )
    )
  })

  test("US-10.6 displays task labels", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: [
                {
                  id: "oa-l1",
                  title: "Label task",
                  description: "",
                  status: "open",
                  priority: 2,
                  type: "feature",
                  labels: ["ui", "theme"],
                  createdAt: "2024-12-03",
                  updatedAt: "2024-12-03",
                },
              ],
              loading: false,
              error: null,
              collapsed: false,
              maxDisplay: 20,
              assigningId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("ui, theme")
          expect(html).toContain("feature")
          expect(html).toContain("oa-l1")
        })
      )
    )
  })

  test("US-10.5 shows task type badges", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "mc-tasks-test" } as Element

          const customWidget = {
            ...MCTasksWidget,
            initialState: (): MCTasksState => ({
              tasks: [
                {
                  id: "oa-bug",
                  title: "Bug task",
                  description: "",
                  status: "open",
                  priority: 1,
                  type: "bug",
                  labels: [],
                  createdAt: "2024-12-03",
                  updatedAt: "2024-12-03",
                },
                {
                  id: "oa-feature",
                  title: "Feature task",
                  description: "",
                  status: "open",
                  priority: 2,
                  type: "feature",
                  labels: [],
                  createdAt: "2024-12-03",
                  updatedAt: "2024-12-03",
                },
              ],
              loading: false,
              error: null,
              collapsed: false,
              maxDisplay: 20,
              assigningId: null,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("bug")
          expect(html).toContain("feature")
          expect(html).toContain("oa-bug")
          expect(html).toContain("oa-feature")
        })
      )
    )
  })

  test("US-10.8 refresh reloads ready tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          let requestedLimit: number | undefined
          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              loadReadyTasks: (limit = 50) => {
                requestedLimit = limit
                return Effect.succeed([
                  {
                    id: "oa-new",
                    title: "New task",
                    description: "",
                    status: "open",
                    priority: 1,
                    type: "task",
                    labels: [],
                    createdAt: "2024-12-04",
                    updatedAt: "2024-12-04",
                  },
                ])
              },
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "mc-tasks-test" } as Element
              const state = yield* stateService.cell(MCTasksWidget.initialState())
              const ctx = { state, emit: () => Effect.void, dom, container }

              yield* MCTasksWidget.handleEvent({ type: "load" }, ctx)

              const updated = yield* state.get
              expect(updated.loading).toBe(false)
              expect(updated.tasks).toHaveLength(1)
              expect(updated.tasks[0]?.id).toBe("oa-new")

              const html = (yield* MCTasksWidget.render(ctx)).toString()
              expect(html).toContain("oa-new")
              expect(html).toContain("Refresh")
            }),
            layer
          )

          expect(requestedLimit).toBe(50)
        })
      )
    )
  })

  test("US-10.1 loads ready tasks from socket and updates list", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const tasks: MCTask[] = [
            {
              id: "oa-1",
              title: "Fix bug",
              description: "desc",
              status: "open",
              priority: 0,
              type: "bug",
              labels: ["bugfix"],
              createdAt: "2024-12-01",
              updatedAt: "2024-12-01",
            },
            {
              id: "oa-2",
              title: "Add feature",
              description: "desc",
              status: "open",
              priority: 2,
              type: "feature",
              labels: ["feature"],
              createdAt: "2024-12-02",
              updatedAt: "2024-12-02",
            },
          ]

          let requestedLimit: number | undefined
          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              loadReadyTasks: (limit = 50) => {
                requestedLimit = limit
                return Effect.succeed(tasks)
              },
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "mc-tasks-test" } as Element
              const state = yield* stateService.cell(MCTasksWidget.initialState())
              const ctx = { state, emit: () => Effect.void, dom, container }

              yield* MCTasksWidget.handleEvent({ type: "load" }, ctx)

              const updated = yield* state.get
              expect(updated.loading).toBe(false)
              expect(updated.tasks).toHaveLength(2)

              const html = (yield* MCTasksWidget.render(ctx)).toString()
              expect(html).toContain("Ready Tasks (2)")
              expect(html).toContain("oa-1")
              expect(html).toContain("oa-2")
              expect(html).toContain("P0")
              expect(html).toContain("P2")
            }),
            layer
          )

          expect(requestedLimit).toBe(50)
        })
      )
    )
  })

  test("US-10.2 assigns a task to MechaCoder and removes it from list", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          let lastAssignTask: string | null = null
          let lastAssignSandbox: boolean | undefined

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              assignTaskToMC: (taskId, options) => {
                lastAssignTask = taskId
                lastAssignSandbox = options?.sandbox
                return Effect.succeed({ assigned: true })
              },
              loadReadyTasks: () => Effect.succeed([]),
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "mc-tasks-test" } as Element
              const state = yield* stateService.cell({
                ...MCTasksWidget.initialState(),
                tasks: [
                  {
                    id: "oa-assign",
                    title: "Assign me",
                    description: "",
                    status: "open",
                    priority: 1,
                    type: "task",
                    labels: [],
                    createdAt: "2024-12-01",
                    updatedAt: "2024-12-01",
                  },
                  {
                    id: "oa-stay",
                    title: "Keep me",
                    description: "",
                    status: "open",
                    priority: 2,
                    type: "task",
                    labels: [],
                    createdAt: "2024-12-02",
                    updatedAt: "2024-12-02",
                  },
                ],
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              yield* MCTasksWidget.handleEvent({ type: "assign", taskId: "oa-assign" }, ctx)

              const updated = yield* state.get
              expect(updated.assigningId).toBeNull()
              expect(updated.tasks.map((t) => t.id)).toEqual(["oa-stay"])

              const html = (yield* MCTasksWidget.render(ctx)).toString()
              expect(html).not.toContain("oa-assign")
              expect(html).toContain("oa-stay")
            }),
            layer
          )

          expect(lastAssignTask).toBe("oa-assign")
          expect(lastAssignSandbox).toBe(true)
        })
      )
    )
  })

  test("US-10.7 toggles collapse state to hide task list", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "mc-tasks-test" } as Element
              const state = yield* stateService.cell({
                ...MCTasksWidget.initialState(),
                tasks: [
                  {
                    id: "oa-1",
                    title: "Task",
                    description: "",
                    status: "open",
                    priority: 1,
                    type: "task",
                    labels: [],
                    createdAt: "2024-12-01",
                    updatedAt: "2024-12-01",
                  },
                ],
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              const expandedHtml = (yield* MCTasksWidget.render(ctx)).toString()
              expect(expandedHtml).toContain("Task")
              expect(expandedHtml).toContain("Ready Tasks (1)")

              yield* MCTasksWidget.handleEvent({ type: "toggleCollapse" }, ctx)

              const collapsed = yield* state.get
              expect(collapsed.collapsed).toBe(true)

              const collapsedHtml = (yield* MCTasksWidget.render(ctx)).toString()
              expect(collapsedHtml).toContain("Ready Tasks (1)")
              expect(collapsedHtml).toContain("▼")
              expect(collapsedHtml).not.toContain("oa-1")
            }),
            layer
          )
        })
      )
    )
  })
})
