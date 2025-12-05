/**
 * TB Output Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { TBOutputWidget, type TBOutputState, type TBOutputLine } from "./tb-output.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"

describe("TBOutputWidget", () => {
  test("renders hidden when not visible", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-output-test" } as Element

          yield* mountWidget(TBOutputWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("hidden")
        })
      )
    )
  })

  test("renders visible with output lines", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-output-test" } as Element

          const mockLines: TBOutputLine[] = [
            { text: "Starting test...", source: "system", timestamp: Date.now() },
            { text: "Running verification", source: "verification", timestamp: Date.now() },
            { text: "Agent response here", source: "agent", timestamp: Date.now() },
          ]

          const customWidget = {
            ...TBOutputWidget,
            initialState: (): TBOutputState => ({
              outputLines: mockLines,
              maxLines: 500,
              visible: true,
              runId: "run-abc12345",
              taskId: "task-001",
              autoScroll: true,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("TB Output")
          expect(html).toContain("abc12345") // Short run ID
          expect(html).toContain("task-001")
          expect(html).toContain("Starting test...")
          expect(html).toContain("Agent response here")
          expect(html).toContain("3 lines")
        })
      )
    )
  })

  test("renders empty output state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-output-test" } as Element

          const customWidget = {
            ...TBOutputWidget,
            initialState: (): TBOutputState => ({
              outputLines: [],
              maxLines: 500,
              visible: true,
              runId: "run-xyz",
              taskId: null,
              autoScroll: true,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("No output yet")
          expect(html).toContain("0 lines")
        })
      )
    )
  })

  test("renders source badges correctly", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-output-test" } as Element

          const mockLines: TBOutputLine[] = [
            { text: "Agent output", source: "agent", timestamp: Date.now() },
            { text: "Verification output", source: "verification", timestamp: Date.now() },
            { text: "System output", source: "system", timestamp: Date.now() },
          ]

          const customWidget = {
            ...TBOutputWidget,
            initialState: (): TBOutputState => ({
              outputLines: mockLines,
              maxLines: 500,
              visible: true,
              runId: null,
              taskId: null,
              autoScroll: true,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("AGT") // Agent label
          expect(html).toContain("VRF") // Verification label
          expect(html).toContain("SYS") // System label
        })
      )
    )
  })

  test("shows auto-scroll toggle state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-output-test" } as Element

          // Auto-scroll enabled
          const customWidget1 = {
            ...TBOutputWidget,
            initialState: (): TBOutputState => ({
              outputLines: [],
              maxLines: 500,
              visible: true,
              runId: null,
              taskId: null,
              autoScroll: true,
            }),
          }

          yield* mountWidget(customWidget1, container).pipe(Effect.provide(layer))
          const html1 = yield* getRendered(container)
          expect(html1).toContain('data-autoscroll="true"')

          // Auto-scroll disabled
          const customWidget2 = {
            ...TBOutputWidget,
            initialState: (): TBOutputState => ({
              outputLines: [],
              maxLines: 500,
              visible: true,
              runId: null,
              taskId: null,
              autoScroll: false,
            }),
          }

          yield* mountWidget(customWidget2, container).pipe(Effect.provide(layer))
          const html2 = yield* getRendered(container)
          expect(html2).toContain('data-autoscroll="false"')
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = TBOutputWidget.initialState()

    expect(state.outputLines).toEqual([])
    expect(state.maxLines).toBe(500)
    expect(state.visible).toBe(false)
    expect(state.runId).toBeNull()
    expect(state.taskId).toBeNull()
    expect(state.autoScroll).toBe(true)
  })
})
