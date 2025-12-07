/**
 * TB Controls Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect, Stream } from "effect"
import { TBControlsWidget, initialTBControlsState, type TBControlsState, type TBControlsEvent, type TBSuiteInfo } from "./tb-controls.js"
import { mountWidget } from "../widget/mount.js"
import { makeCustomTestLayer, makeTestLayer } from "../layers/test.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"
import { SocketError } from "../services/socket.js"
import type { StartTBRunOptions } from "../services/socket.js"

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

  test("US-1.1 pre-populates default suite path input", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-controls-test" } as Element

          yield* mountWidget(TBControlsWidget, container).pipe(Effect.provide(layer))

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain('value="./tasks/terminal-bench-2.json"')
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
              statusType: "idle" as const,
              isRunning: false,
              runId: null,
              collapsed: false,
              totalTasks: 0,
              completedTasks: 0,
              passedTasks: 0,
              failedTasks: 0,
              startedAt: null,
              duration: null,
              difficultyFilter: null,
              searchFilter: "",
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
              statusType: "running" as const,
              isRunning: true,
              runId: "run-12345",
              collapsed: false,
              totalTasks: 0,
              completedTasks: 0,
              passedTasks: 0,
              failedTasks: 0,
              startedAt: null,
              duration: null,
              difficultyFilter: null,
              searchFilter: "",
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
              statusType: "idle" as const,
              isRunning: false,
              runId: null,
              collapsed: true,
              totalTasks: 0,
              completedTasks: 0,
              passedTasks: 0,
              failedTasks: 0,
              startedAt: null,
              duration: null,
              difficultyFilter: null,
              searchFilter: "",
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
              statusType: "error" as const,
              isRunning: false,
              runId: null,
              collapsed: false,
              totalTasks: 0,
              completedTasks: 0,
              passedTasks: 0,
              failedTasks: 0,
              startedAt: null,
              duration: null,
              difficultyFilter: null,
              searchFilter: "",
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
              statusType: "idle" as const,
              isRunning: false,
              runId: null,
              collapsed: false,
              totalTasks: 0,
              completedTasks: 0,
              passedTasks: 0,
              failedTasks: 0,
              startedAt: null,
              duration: null,
              difficultyFilter: null,
              searchFilter: "",
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

    expect(state.suitePath).toBe("./tasks/terminal-bench-2.json")
    expect(state.suite).toBeNull()
    expect(state.selectedTaskIds.size).toBe(0)
    expect(state.loading).toBe(false)
    expect(state.status).toBe("Ready")
    expect(state.statusType).toBe("idle")
    expect(state.isRunning).toBe(false)
    expect(state.runId).toBeNull()
    expect(state.collapsed).toBe(false)
  })

  test("US-1.1 loads suite and selects all tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const mockSuite: TBSuiteInfo = {
            name: "terminal-bench-v1",
            version: "1.2.3",
            tasks: [
              { id: "task-001", name: "Easy task", difficulty: "easy", category: "basics" },
              { id: "task-002", name: "Hard task", difficulty: "hard", category: "advanced" },
            ],
          }

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              loadTBSuite: (_suitePath) => Effect.succeed(mockSuite),
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell(TBControlsWidget.initialState())
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)

              const updated = yield* state.get
              expect(updated.suite?.name).toBe("terminal-bench-v1")
              expect(updated.selectedTaskIds.size).toBe(mockSuite.tasks.length)
              expect(updated.status).toBe("Ready")
              expect(updated.statusType).toBe("idle")

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("terminal-bench-v1")
              expect(html).toContain("v1.2.3")
              expect(html).toContain("2/2 selected")
              expect(html).toContain("Easy task")
              expect(html).toContain("Hard task")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-3.1 starts a run for the selected tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const mockSuite: TBSuiteInfo = {
            name: "terminal-bench-v1",
            version: "1.2.3",
            tasks: [
              { id: "task-001", name: "First", difficulty: "easy", category: "basics" },
              { id: "task-002", name: "Second", difficulty: "medium", category: "core" },
              { id: "task-003", name: "Third", difficulty: "hard", category: "advanced" },
            ],
          }

          let receivedOptions: StartTBRunOptions | null = null

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              loadTBSuite: (_suitePath) => Effect.succeed(mockSuite),
              startTBRun: (options) => {
                receivedOptions = options
                return Effect.succeed({ runId: "run-42" })
              },
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell(TBControlsWidget.initialState())
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)
              yield* TBControlsWidget.handleEvent!({ type: "startRun" }, ctx)

              const updated = yield* state.get
              expect(receivedOptions?.taskIds).toEqual(mockSuite.tasks.map((task) => task.id))
              expect(updated.isRunning).toBe(true)
              expect(updated.runId).toBe("run-42")
              expect(updated.status).toBe("Running...")
              expect(updated.statusType).toBe("running")

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("Running...")
              expect(html).toContain("Stop")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-3.2 starts a run for only selected tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const mockSuite: TBSuiteInfo = {
            name: "terminal-bench-v1",
            version: "1.2.3",
            tasks: [
              { id: "task-001", name: "First", difficulty: "easy", category: "basics" },
              { id: "task-002", name: "Second", difficulty: "medium", category: "core" },
              { id: "task-003", name: "Third", difficulty: "hard", category: "advanced" },
            ],
          }

          let receivedOptions: StartTBRunOptions | null = null

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              loadTBSuite: (_suitePath) => Effect.succeed(mockSuite),
              startTBRun: (options) => {
                receivedOptions = options
                return Effect.succeed({ runId: "run-99" })
              },
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell(TBControlsWidget.initialState())
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)
              yield* state.update((s) => ({
                ...s,
                selectedTaskIds: new Set(["task-002"]),
              }))

              yield* TBControlsWidget.handleEvent!({ type: "startRun" }, ctx)

              const updated = yield* state.get
              expect(receivedOptions?.taskIds).toEqual(["task-002"])
              expect(updated.isRunning).toBe(true)
              expect(updated.runId).toBe("run-99")
              expect(updated.status).toBe("Running...")
              expect(updated.statusType).toBe("running")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-3.3 starts a single random task", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const mockSuite: TBSuiteInfo = {
            name: "terminal-bench-v1",
            version: "1.2.3",
            tasks: [
              { id: "task-001", name: "First", difficulty: "easy", category: "basics" },
              { id: "task-002", name: "Second", difficulty: "medium", category: "core" },
            ],
          }

          let receivedOptions: StartTBRunOptions | null = null

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              loadTBSuite: (_suitePath) => Effect.succeed(mockSuite),
              startTBRun: (options) => {
                receivedOptions = options
                return Effect.succeed({ runId: "run-random" })
              },
            },
          })

          const originalRandom = Math.random
          Math.random = () => 0 // force first index

          try {
            yield* Effect.provide(
              Effect.gen(function* () {
                const stateService = yield* StateServiceTag
                const dom = yield* DomServiceTag
                const container = { id: "tb-controls-test" } as Element
                const state = yield* stateService.cell(TBControlsWidget.initialState())
                const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

                yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)
                yield* TBControlsWidget.handleEvent!({ type: "startRandomTask" }, ctx)

                const updated = yield* state.get
                expect(receivedOptions?.taskIds).toEqual(["task-001"])
                expect(updated.isRunning).toBe(true)
                expect(updated.runId).toBe("run-random")
                expect(updated.statusType).toBe("running")
              }),
              layer
            )
          } finally {
            Math.random = originalRandom
          }
        })
      )
    )
  })

  test("US-3.4 stops an active run and resets controls", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          let stopCalls = 0

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              stopTBRun: () => {
                stopCalls += 1
                return Effect.succeed({ stopped: true })
              },
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                isRunning: true,
                runId: "run-stop-me",
                status: "Running...",
                statusType: "running" as const,
                suite: {
                  name: "terminal-bench-v1",
                  version: "1.0.0",
                  tasks: [{ id: "task-001", name: "Task 1", difficulty: "easy", category: "basics" }],
                },
                selectedTaskIds: new Set(["task-001"]),
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "stopRun" }, ctx)

              const updated = yield* state.get
              expect(stopCalls).toBe(1)
              expect(updated.isRunning).toBe(false)
              expect(updated.runId).toBeNull()
              expect(updated.status).toBe("Stopped")
              expect(updated.statusType).toBe("idle")

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("Stopped")
              expect(html).toContain("Start")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-1.2 shows error when loading with empty path", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suitePath: "   ",
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)

              const updated = yield* state.get
              expect(updated.status).toBe("No path")
              expect(updated.statusType).toBe("error")

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("No path")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-1.3 shows suite metadata after loading", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const mockSuite: TBSuiteInfo = {
            name: "terminal-bench-v2",
            version: "2.0.0",
            tasks: [
              { id: "task-alpha", name: "Alpha", difficulty: "easy", category: "basics" },
              { id: "task-beta", name: "Beta", difficulty: "medium", category: "core" },
            ],
          }

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              loadTBSuite: (_suitePath) => Effect.succeed(mockSuite),
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell(TBControlsWidget.initialState())
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)

              const updated = yield* state.get
              expect(updated.suite?.name).toBe("terminal-bench-v2")
              expect(updated.suite?.version).toBe("2.0.0")
              expect(updated.selectedTaskIds.size).toBe(2)

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("terminal-bench-v2")
              expect(html).toContain("v2.0.0")
              expect(html).toContain("2/2 selected")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.8 toggles individual task selection", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const suite: TBSuiteInfo = {
                name: "suite",
                version: "1",
                tasks: [
                  { id: "a", name: "A", difficulty: "easy", category: "c" },
                  { id: "b", name: "B", difficulty: "hard", category: "c" },
                ],
              }
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suite,
                selectedTaskIds: new Set(["a"]),
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "toggleTask", taskId: "b" }, ctx)
              yield* TBControlsWidget.handleEvent!({ type: "toggleTask", taskId: "a" }, ctx)

              const updated = yield* state.get
              expect(updated.selectedTaskIds.has("b")).toBe(true)
              expect(updated.selectedTaskIds.has("a")).toBe(false)

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("1/2 selected")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.9 select all and clear selection buttons work", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const suite: TBSuiteInfo = {
                name: "suite",
                version: "1",
                tasks: [
                  { id: "a", name: "A", difficulty: "easy", category: "c" },
                  { id: "b", name: "B", difficulty: "hard", category: "c" },
                ],
              }
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suite,
                selectedTaskIds: new Set(["a", "b"]),
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "selectNone" }, ctx)
              let updated = yield* state.get
              expect(updated.selectedTaskIds.size).toBe(0)

              yield* TBControlsWidget.handleEvent!({ type: "selectAll" }, ctx)
              updated = yield* state.get
              expect(updated.selectedTaskIds.size).toBe(2)

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("2/2 selected")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.8 shows selection count as tasks are toggled", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const suite: TBSuiteInfo = {
                name: "terminal-bench-v1",
                version: "1.0.0",
                tasks: [
                  { id: "task-1", name: "Task 1", difficulty: "easy", category: "alpha" },
                  { id: "task-2", name: "Task 2", difficulty: "hard", category: "beta" },
                ],
              }

              const state = yield* stateService.cell({
                ...initialTBControlsState,
                suitePath: "/tmp/suite.json",
                suite,
                selectedTaskIds: new Set(["task-1"]),
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              // Initial render reflects 1/2 selected
              let html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("1/2 selected")

              // Toggle second task on
              yield* TBControlsWidget.handleEvent!({ type: "toggleTask", taskId: "task-2" }, ctx)

              html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("2/2 selected")

              // Toggle first task off
              yield* TBControlsWidget.handleEvent!({ type: "toggleTask", taskId: "task-1" }, ctx)
              html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("1/2 selected")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-14.3 surfaces suite load errors and clears selection", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              loadTBSuite: (_suitePath) => Effect.fail(new SocketError("request_failed", "file missing")),
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suite: {
                  name: "previous-suite",
                  version: "1.0.0",
                  tasks: [{ id: "task-keep", name: "Keep", difficulty: "easy", category: "basics" }],
                },
                selectedTaskIds: new Set(["task-keep"]),
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)

              const updated = yield* state.get
              expect(updated.suite).toBeNull()
              expect(updated.selectedTaskIds.size).toBe(0)
              expect(updated.status).toBe("Load failed")
              expect(updated.statusType).toBe("error")

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("Load failed")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-14.4 shows run error when startTBRun fails", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          let startCalls = 0

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              startTBRun: (_options) => {
                startCalls++
                return Effect.fail(new SocketError("request_failed", "runner exploded"))
              },
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suite: {
                  name: "terminal-bench-v1",
                  version: "1.0.0",
                  tasks: [{ id: "task-1", name: "Task 1", difficulty: "easy", category: "basics" }],
                },
                selectedTaskIds: new Set(["task-1"]),
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "startRun" }, ctx)

              const updated = yield* state.get
              expect(startCalls).toBe(1)
              expect(updated.isRunning).toBe(false)
              expect(updated.runId).toBeNull()
              expect(updated.status).toBe("Start failed")
              expect(updated.statusType).toBe("error")

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("Start failed")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-4.1 updates status when tb_run_start arrives", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-controls-test" } as Element

          yield* mountWidget(TBControlsWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({ 
              type: "tb_run_start", 
              runId: "run-socket",
              suiteName: "test-suite",
              suiteVersion: "1.0.0",
              totalTasks: 1,
              taskIds: ["task-1"],
              timestamp: new Date().toISOString(),
            })
          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("Running...")
        })
      )
    )
  })

  test("US-4.1 marks run complete on tb_run_complete for active run", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-controls-test" } as Element

          yield* mountWidget(TBControlsWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({ 
              type: "tb_run_start", 
              runId: "run-complete",
              suiteName: "test-suite",
              suiteVersion: "1.0.0",
              totalTasks: 1,
              taskIds: ["task-1"],
              timestamp: new Date().toISOString(),
            })
          yield* Effect.sleep(0)

          yield* injectMessage({ 
            type: "tb_run_complete", 
            runId: "run-complete",
            passRate: 1.0,
            passed: 1,
            failed: 0,
            timeout: 0,
            error: 0,
            totalDurationMs: 1000,
          })
          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("Complete")
          expect(html).not.toContain("Running...")
        })
      )
    )
  })

  test("US-4.1 ignores completion for a different runId", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-controls-test" } as Element

          yield* mountWidget(TBControlsWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({ 
              type: "tb_run_start", 
              runId: "run-active",
              suiteName: "test-suite",
              suiteVersion: "1.0.0",
              totalTasks: 1,
              taskIds: ["task-1"],
              timestamp: new Date().toISOString(),
            })
          yield* Effect.sleep(0)

          yield* injectMessage({ 
            type: "tb_run_complete", 
            runId: "run-other",
            passRate: 1.0,
            passed: 1,
            failed: 0,
            timeout: 0,
            error: 0,
            totalDurationMs: 1000,
          })
          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("Running...")
        })
      )
    )
  })

  test("US-1.4 reloads suite and reselects all tasks", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const suite: TBSuiteInfo = {
            name: "terminal-bench-v1",
            version: "1.0.0",
            tasks: [
              { id: "task-a", name: "Task A", difficulty: "easy", category: "alpha" },
              { id: "task-b", name: "Task B", difficulty: "hard", category: "beta" },
            ],
          }

          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              getMessages: () => Stream.empty,
              loadTBSuite: () => Effect.succeed(suite),
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suite,
                selectedTaskIds: new Set(["task-a"]),
                status: "Dirty",
                statusType: "error" as const,
              } as TBControlsState)
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              yield* TBControlsWidget.handleEvent!({ type: "loadSuite" }, ctx)

              const updated = yield* state.get
              expect(updated.selectedTaskIds.size).toBe(2)
              expect(updated.status).toBe("Ready")

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("2/2 selected")
              expect(html).toContain("terminal-bench-v1")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-1.5 shows suite file path in the UI", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suitePath: "/abs/path/to/suite.json",
                suite: {
                  name: "terminal-bench-v1",
                  version: "1.0.0",
                  tasks: [{ id: "t1", name: "Task 1", difficulty: "easy", category: "alpha" }],
                },
              })
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              const html = (yield* TBControlsWidget.render(ctx)).toString()
              expect(html).toContain("/abs/path/to/suite.json")
              expect(html).toContain("terminal-bench-v1")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-4.2 displays progress bar during run with pass/fail counts", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                isRunning: true,
                totalTasks: 10,
                completedTasks: 7,
                passedTasks: 5,
                failedTasks: 2,
                status: "7/10 tasks",
                statusType: "running",
              })
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              const html = (yield* TBControlsWidget.render(ctx)).toString()

              // Verify progress bar is shown
              expect(html).toContain('data-testid="progress-bar"')

              // Verify progress percentage
              expect(html).toContain("Progress: 7/10 (70%)")

              // Verify pass/fail counts
              expect(html).toContain("✓5")
              expect(html).toContain("✗2")

              // Verify progress bar segments
              expect(html).toContain('data-testid="progress-passed"')
              expect(html).toContain('data-testid="progress-failed"')
              expect(html).toContain("width: 50%") // 5/10 passed = 50%
              expect(html).toContain("width: 20%") // 2/10 failed = 20%
            }),
            layer
          )
        })
      )
    )
  })

  test("US-4.6 displays run duration timer in HH:MM:SS format", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element

              // Test 1: Timer during active run
              const startTime = Date.now() - (3 * 60 + 45) * 1000 // 3:45 ago
              const runningState = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                isRunning: true,
                totalTasks: 10,
                completedTasks: 5,
                passedTasks: 3,
                failedTasks: 2,
                startedAt: startTime,
                duration: null,
                status: "5/10 tasks",
                statusType: "running",
              })
              const runningCtx = { state: runningState, emit: () => Effect.void, dom, container }

              const runningHtml = (yield* TBControlsWidget.render(runningCtx)).toString()
              expect(runningHtml).toContain('data-testid="run-duration"')
              expect(runningHtml).toContain("00:03:") // Should show ~3 minutes
              expect(runningHtml).toContain("⏱") // Timer icon

              // Test 2: Duration after run completes
              const completedState = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                isRunning: false,
                totalTasks: 10,
                completedTasks: 10,
                passedTasks: 7,
                failedTasks: 3,
                startedAt: null,
                duration: 125000, // 2 minutes 5 seconds
                status: "Complete",
                statusType: "success",
              })
              const completedCtx = { state: completedState, emit: () => Effect.void, dom, container }

              const completedHtml = (yield* TBControlsWidget.render(completedCtx)).toString()
              expect(completedHtml).toContain('data-testid="run-duration"')
              expect(completedHtml).toContain("00:02:05") // Exact duration
              expect(completedHtml).toContain("⏱") // Timer icon

              // Test 3: No timer when not running and no duration
              const idleState = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                isRunning: false,
                startedAt: null,
                duration: null,
              })
              const idleCtx = { state: idleState, emit: () => Effect.void, dom, container }

              const idleHtml = (yield* TBControlsWidget.render(idleCtx)).toString()
              expect(idleHtml).not.toContain('data-testid="run-duration"')
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.5 filters tasks by difficulty", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const mockSuite: TBSuiteInfo = {
                name: "test-suite",
                version: "1.0.0",
                tasks: [
                  { id: "task-001", name: "Easy task 1", difficulty: "easy", category: "basics" },
                  { id: "task-002", name: "Easy task 2", difficulty: "easy", category: "basics" },
                  { id: "task-003", name: "Medium task", difficulty: "medium", category: "core" },
                  { id: "task-004", name: "Hard task", difficulty: "hard", category: "advanced" },
                ],
              }
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suite: mockSuite,
                difficultyFilter: "easy",
              })
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              const html = (yield* TBControlsWidget.render(ctx)).toString()

              // Should show easy tasks
              expect(html).toContain("Easy task 1")
              expect(html).toContain("Easy task 2")
              // Should not show medium/hard tasks
              expect(html).not.toContain("Medium task")
              expect(html).not.toContain("Hard task")
              // Should show filter dropdown with "easy" selected
              expect(html).toContain('value="easy"')
              expect(html).toContain("selected")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-2.7 searches tasks by name or ID", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-controls-test" } as Element
              const mockSuite: TBSuiteInfo = {
                name: "test-suite",
                version: "1.0.0",
                tasks: [
                  { id: "task-001", name: "Create file", difficulty: "easy", category: "basics" },
                  { id: "task-002", name: "Delete file", difficulty: "easy", category: "basics" },
                  { id: "task-003", name: "List directory", difficulty: "medium", category: "core" },
                ],
              }
              const state = yield* stateService.cell({
                ...TBControlsWidget.initialState(),
                suite: mockSuite,
                searchFilter: "file",
              })
              const ctx = { state, emit: (_event: TBControlsEvent) => Effect.succeed(undefined), dom, container }

              const html = (yield* TBControlsWidget.render(ctx)).toString()

              // Should show tasks with "file" in name
              expect(html).toContain("Create file")
              expect(html).toContain("Delete file")
              // Should not show unmatched task
              expect(html).not.toContain("List directory")
              // Should show search input with value
              expect(html).toContain('placeholder="Search tasks..."')
              expect(html).toContain('value="file"')
            }),
            layer
          )
        })
      )
    )
  })
})
