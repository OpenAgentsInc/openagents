/**
 * APM Widget Tests
 *
 * Tests the APM widget using the Effuse test layer.
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { APMWidget, type APMState } from "./apm-widget.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"

describe("APMWidget", () => {
  test("renders initial state with 0 APM", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()

          // Create mock container
          const container = { id: "apm-test" } as Element

          // Mount widget with test layer
          yield* mountWidget(APMWidget, container).pipe(Effect.provide(layer))

          // Check rendered content
          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("0.0") // Initial APM
          expect(html).toContain("APM")
        })
      )
    )
  })

  test("renders with custom initial state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "apm-test" } as Element

          // Create widget with custom initial state
          const customWidget = {
            ...APMWidget,
            initialState: (): APMState => ({
              sessionAPM: 25.5,
              recentAPM: 20.0,
              totalActions: 100,
              durationMinutes: 30,
              apm1h: 22.0,
              apm6h: 18.0,
              apm1d: 15.0,
              apmLifetime: 12.0,
              claudeCodeAPM: 10.0,
              mechaCoderAPM: 25.0,
              efficiencyRatio: 2.5,
              expanded: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("25.5") // Session APM
          expect(html).toContain("100 actions") // Total actions
        })
      )
    )
  })

  test("updates state on APM update message", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "apm-test" } as Element

          yield* mountWidget(APMWidget, container).pipe(Effect.provide(layer))

          // Initial render should show 0
          let html = yield* getRendered(container)
          expect(html).toContain("0.0")

          // Inject APM update message
          yield* injectMessage({
            type: "apm_update",
            sessionId: "test-session",
            sessionAPM: 15.5,
            recentAPM: 12.0,
            totalActions: 50,
            durationMinutes: 10,
          })

          // Give stream time to process
          yield* Effect.sleep(50)

          // Check updated content
          html = yield* getRendered(container)
          expect(html).toContain("15.5")
          expect(html).toContain("50 actions")
        })
      )
    )
  })

  test("US-4.8 updates live APM metrics from socket", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "apm-test" } as Element

          yield* mountWidget(APMWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "apm_update",
            sessionId: "session-live",
            sessionAPM: 22.4,
            recentAPM: 18.3,
            totalActions: 75,
            durationMinutes: 15,
          })

          yield* Effect.sleep(30)

          const html = yield* getRendered(container)
          expect(html).toContain("22.4")
          expect(html).toContain("75 actions")
        })
      )
    )
  })

  test("US-4.8 renders snapshot metrics and comparison when expanded", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "apm-test" } as Element

          const expandedWidget = {
            ...APMWidget,
            initialState: (): APMState => ({ ...APMWidget.initialState(), expanded: true }),
          }

          yield* mountWidget(expandedWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({
            type: "apm_snapshot",
            sessionId: "session-snap",
            combined: {
              apm1h: 12.5,
              apm6h: 10.1,
              apm1d: 8.5,
              apm1w: 7.2,
              apm1m: 6.5,
              apmLifetime: 9.9,
              totalSessions: 150,
              totalActions: 1337
            },
            comparison: { claudeCodeAPM: 14.2, mechaCoderAPM: 18.4, efficiencyRatio: 1.8 },
          })

          yield* Effect.sleep(30)

          const html = yield* getRendered(container)
          expect(html).toContain("12.5")
          expect(html).toContain("10.1")
          expect(html).toContain("1.8x")
          expect(html).toContain("Claude")
          expect(html).toContain("MC")
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = APMWidget.initialState()

    expect(state.sessionAPM).toBe(0)
    expect(state.recentAPM).toBe(0)
    expect(state.totalActions).toBe(0)
    expect(state.durationMinutes).toBe(0)
    expect(state.expanded).toBe(false)
  })
})
