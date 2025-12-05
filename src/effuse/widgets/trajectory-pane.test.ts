/**
 * Trajectory Pane Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { TrajectoryPaneWidget, type TrajectoryPaneState } from "./trajectory-pane.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"
import type { UnifiedTrajectory } from "../../desktop/protocol.js"

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
})
