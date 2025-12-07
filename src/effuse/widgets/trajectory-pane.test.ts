/**
 * Trajectory Pane Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { TrajectoryPaneWidget, type TrajectoryPaneState } from "./trajectory-pane.js"
import { mountWidget } from "../widget/mount.js"
import { makeCustomTestLayer, makeTestLayer } from "../layers/test.js"
import type { UnifiedTrajectory } from "../../desktop/protocol.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"

describe("TrajectoryPaneWidget", () => {
  test("renders empty state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "trajectory-test" } as Element

          yield* mountWidget(TrajectoryPaneWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("No trajectories found")
          expect(html).toContain("Trajectories")
        })
      )
    )
  })

  test("renders with trajectories", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "trajectory-test" } as Element

          // Create widget with pre-populated trajectories
          const mockTrajectories: UnifiedTrajectory[] = [
            {
              id: "run-12345678",
              type: "tb-run",
              timestamp: "2024-12-05T10:00:00Z",
              label: "TB: 85% (34/40)",
              suiteName: "test-suite",
              passRate: 85,
              passed: 34,
              failed: 6,
              taskCount: 40,
            },
            {
              id: "atif-87654321",
              type: "atif",
              timestamp: "2024-12-05T09:00:00Z",
              label: "MC: 45 steps",
              agentName: "mechacoder",
              totalSteps: 45,
            },
          ]

          const customWidget = {
            ...TrajectoryPaneWidget,
            initialState: (): TrajectoryPaneState => ({
              trajectories: mockTrajectories,
              selectedId: null,
              loading: false,
              error: null,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("12345678") // Short ID
          expect(html).toContain("87654321")
          expect(html).toContain("TB: 85%")
          expect(html).toContain("MC: 45 steps")
          expect(html).toContain("TB") // Type badge
          expect(html).toContain("ATIF") // Type badge
        })
      )
    )
  })

  test("renders loading state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "trajectory-test" } as Element

          const customWidget = {
            ...TrajectoryPaneWidget,
            initialState: (): TrajectoryPaneState => ({
              trajectories: [],
              selectedId: null,
              loading: true,
              error: null,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Loading trajectories")
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "trajectory-test" } as Element

          const customWidget = {
            ...TrajectoryPaneWidget,
            initialState: (): TrajectoryPaneState => ({
              trajectories: [
                {
                  id: "run-12345678",
                  type: "tb-run",
                  timestamp: "2024-12-05T10:00:00Z",
                  label: "TB: 85%",
                },
              ],
              selectedId: null,
              loading: false,
              error: null,
              collapsed: true,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          // When collapsed, should show header but not trajectory list
          expect(html).toContain("+ Trajectories") // Collapsed indicator
          expect(html).not.toContain("TB: 85%") // Content hidden
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = TrajectoryPaneWidget.initialState()

    expect(state.trajectories).toEqual([])
    expect(state.selectedId).toBeNull()
    expect(state.loading).toBe(false)
    expect(state.error).toBeNull()
    expect(state.collapsed).toBe(false)
  })

  test("US-7.1 loads recent trajectories on load event", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const mockTrajectories: UnifiedTrajectory[] = [
            {
              id: "run-aaaa",
              type: "tb-run",
              timestamp: "2024-12-05T10:00:00Z",
              label: "TB: 90% (9/10)",
              suiteName: "suite",
              passRate: 90,
              passed: 9,
              failed: 1,
              taskCount: 10,
            },
          ]

          let requestedLimit: number | undefined
          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              loadUnifiedTrajectories: (limit = 50) => {
                requestedLimit = limit
                return Effect.succeed(mockTrajectories)
              },
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "trajectory-test" } as Element
              const state = yield* stateService.cell<TrajectoryPaneState>(TrajectoryPaneWidget.initialState())
              const ctx = { state, emit: () => Effect.void, dom, container }

              if (TrajectoryPaneWidget.handleEvent) {
                yield* TrajectoryPaneWidget.handleEvent({ type: "load" }, ctx)
              }

              const html = (yield* TrajectoryPaneWidget.render(ctx)).toString()
              expect(html).toContain("run-aaaa")
              expect(html).toContain("TB: 90%")
              const updated = yield* state.get
              expect(updated.loading).toBe(false)
              expect(updated.trajectories).toHaveLength(1)
            }),
            layer
          )

          expect(requestedLimit).toBe(50)
        })
      )
    )
  })

  test("US-7.2 selects a trajectory for details", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "trajectory-test" } as Element
              const trajectories: UnifiedTrajectory[] = [
                { id: "run-1", type: "tb-run", timestamp: "2024-01-01", label: "Run 1" },
                { id: "run-2", type: "tb-run", timestamp: "2024-01-02", label: "Run 2" },
              ]
              const state = yield* stateService.cell<TrajectoryPaneState>({
                trajectories,
                selectedId: null,
                loading: false,
                error: null,
                collapsed: false,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              if (TrajectoryPaneWidget.handleEvent) {
                yield* TrajectoryPaneWidget.handleEvent({ type: "select", trajectoryId: "run-2" }, ctx)
              }

              const updated = yield* state.get
              expect(updated.selectedId).toBe("run-2")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-7.6 refresh reloads the trajectory list", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeCustomTestLayer({
            socketService: {
              loadUnifiedTrajectories: (_limit = 50) =>
                Effect.succeed([
                  {
                    id: "run-new",
                    type: "tb-run",
                    timestamp: "2024-12-06T00:00:00Z",
                    label: "New TB run",
                  },
                ]),
            },
          })

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "trajectory-test" } as Element
              const state = yield* stateService.cell<TrajectoryPaneState>({
                trajectories: [] as UnifiedTrajectory[],
                selectedId: null,
                loading: false,
                error: null,
                collapsed: false,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              if (TrajectoryPaneWidget.handleEvent) {
                yield* TrajectoryPaneWidget.handleEvent({ type: "load" }, ctx)
              }

              const updated = yield* state.get
              expect(updated.loading).toBe(false)
              expect(updated.error).toBeNull()
              expect(updated.trajectories).toHaveLength(1)
              expect(updated.trajectories[0]?.id).toBe("run-new")

              const html = (yield* TrajectoryPaneWidget.render(ctx)).toString()
              expect(html).toContain("run-new")
              expect(html).toContain("Refresh")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-7.3 shows unified TB and ATIF trajectories with type badges", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "trajectory-test" } as Element

          const customWidget = {
            ...TrajectoryPaneWidget,
            initialState: (): TrajectoryPaneState => ({
              trajectories: [
                {
                  id: "run-tb-1",
                  type: "tb-run",
                  timestamp: "2024-12-06T01:00:00Z",
                  label: "TB run",
                },
                {
                  id: "run-atif-1",
                  type: "atif",
                  timestamp: "2024-12-05T23:00:00Z",
                  label: "ATIF trace",
                },
              ],
              selectedId: null,
              loading: false,
              error: null,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("run-tb-1")
          expect(html).toContain("run-atif-1")
          expect(html).toContain("TB")
          expect(html).toContain("ATIF")
        })
      )
    )
  })

  test("US-7.7 toggles collapse to hide and show trajectories", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "trajectory-test" } as Element
              const state = yield* stateService.cell<TrajectoryPaneState>({
                trajectories: [
                  {
                    id: "run-1",
                    type: "tb-run",
                    timestamp: "2024-12-05T11:00:00Z",
                    label: "Run 1",
                  },
                ],
                selectedId: null,
                loading: false,
                error: null,
                collapsed: false,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              const expanded = (yield* TrajectoryPaneWidget.render(ctx)).toString()
              expect(expanded).toContain("Run 1")
              expect(expanded).toContain("- Trajectories")

              if (TrajectoryPaneWidget.handleEvent) {
                yield* TrajectoryPaneWidget.handleEvent({ type: "toggleCollapse" }, ctx)
              }

              const collapsed = yield* state.get
              expect(collapsed.collapsed).toBe(true)

              const collapsedHtml = (yield* TrajectoryPaneWidget.render(ctx)).toString()
              expect(collapsedHtml).toContain("+ Trajectories")
              expect(collapsedHtml).not.toContain("Run 1")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-6.1 displays run summary with pass rate and counts", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "trajectory-test" } as Element

          // Create widget with a completed run showing summary stats
          const customWidget = {
            ...TrajectoryPaneWidget,
            initialState: (): TrajectoryPaneState => ({
              trajectories: [
                {
                  id: "run-summary-test",
                  type: "tb-run",
                  timestamp: "2024-12-06T12:00:00Z",
                  label: "TB: 75% (15/20)",
                  suiteName: "terminal-bench-v1",
                  passRate: 75,
                  passed: 15,
                  failed: 5,
                  taskCount: 20,
                },
              ],
              selectedId: null,
              loading: false,
              error: null,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)

          // Verify run summary shows pass rate
          expect(html).toContain("75%")
          // Verify passed/failed counts shown
          expect(html).toContain("15/20")
          // Verify run ID visible
          expect(html).toContain("summary-test")
          // Verify type badge
          expect(html).toContain("TB")
        })
      )
    )
  })

  test("US-7.5 clears run history and resets selection", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "trajectory-test" } as Element
              const state = yield* stateService.cell<TrajectoryPaneState>({
                trajectories: [
                  { id: "run-1", type: "tb-run", timestamp: "2024-12-05T11:00:00Z", label: "Run 1" },
                  { id: "run-2", type: "atif", timestamp: "2024-12-05T12:00:00Z", label: "ATIF 2" },
                ],
                selectedId: "run-2",
                loading: false,
                error: "some-error",
                collapsed: false,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              if (TrajectoryPaneWidget.handleEvent) {
                yield* TrajectoryPaneWidget.handleEvent({ type: "clear" }, ctx)
              }

              const updated = yield* state.get
              expect(updated.trajectories).toHaveLength(0)
              expect(updated.selectedId).toBeNull()
              expect(updated.error).toBeNull()

              const html = (yield* TrajectoryPaneWidget.render(ctx)).toString()
              expect(html).toContain("No trajectories found")
            }),
            layer
          )
        })
      )
    )
  })
})
