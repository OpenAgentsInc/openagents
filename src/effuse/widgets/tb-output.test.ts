/**
 * TB Output Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { TBOutputWidget, type TBOutputState, type TBOutputLine } from "./tb-output.js"
import { mountWidget } from "../widget/mount.js"
import { makeCustomTestLayer, makeTestLayer } from "../layers/test.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"

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

  test("US-5.1 streams live output on tb_task_output", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountWidget(TBOutputWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-live" })
          yield* injectMessage({
            type: "tb_task_output",
            runId: "run-live",
            taskId: "task-001",
            text: "Agent is working",
            source: "agent",
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("Agent is working")
          expect(html).toContain("AGT")
        })
      )
    )
  })

  test("US-5.2 shows run/task context for output", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountWidget(TBOutputWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-12345678" })
          yield* injectMessage({
            type: "tb_task_output",
            runId: "run-12345678",
            taskId: "task-xyz",
            text: "Task-specific output",
            source: "verification",
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("12345678")
          expect(html).toContain("task-xyz")
          expect(html).toContain("Task-specific output")
        })
      )
    )
  })

  test("US-5.3 renders verification output lines", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountWidget(TBOutputWidget, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-verify" })
          yield* injectMessage({
            type: "tb_task_output",
            runId: "run-verify",
            taskId: "task-verify",
            text: "Verification is checking assertions",
            source: "verification",
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("VRF")
          expect(html).toContain("Verification is checking assertions")
        })
      )
    )
  })

  test("US-5.5 clear output removes lines", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-output-test" } as Element
              const state = yield* stateService.cell({
                outputLines: [
                  { text: "keep?", source: "agent", timestamp: Date.now() },
                  { text: "another", source: "system", timestamp: Date.now() },
                ],
                maxLines: 500,
                visible: true,
                runId: "run-clear",
                taskId: "task-clear",
                autoScroll: true,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              yield* TBOutputWidget.handleEvent({ type: "clear" }, ctx)

              const updated = yield* state.get
              expect(updated.outputLines.length).toBe(0)

              const html = (yield* TBOutputWidget.render(ctx)).toString()
              expect(html).toContain("No output yet")
              expect(html).toContain("0 lines")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-5.6 copy output collects rendered text", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          let copied = ""
          const originalNavigator = globalThis.navigator
          ;(globalThis as any).navigator = {
            clipboard: {
              writeText: (text: string) => {
                copied = text
                return Promise.resolve()
              },
            },
          }

          try {
            yield* Effect.provide(
              Effect.gen(function* () {
                const stateService = yield* StateServiceTag
                const dom = yield* DomServiceTag
                const container = { id: "tb-output-test" } as Element
                const state = yield* stateService.cell({
                  outputLines: [
                    { text: "line one", source: "agent", timestamp: Date.now() },
                    { text: "line two", source: "verification", timestamp: Date.now() },
                  ],
                  maxLines: 500,
                  visible: true,
                  runId: "run-copy",
                  taskId: "task-copy",
                  autoScroll: true,
                })
                const ctx = { state, emit: () => Effect.void, dom, container }

                yield* TBOutputWidget.handleEvent({ type: "copy" }, ctx)

                expect(copied).toContain("[agent] line one")
                expect(copied).toContain("[verification] line two")
              }),
              layer
            )
          } finally {
            if (originalNavigator) {
              ;(globalThis as any).navigator = originalNavigator
            } else {
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              delete (globalThis as any).navigator
            }
          }
        })
      )
    )
  })

  test("US-5.1 toggles auto-scroll state and attribute", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-output-test" } as Element
              const state = yield* stateService.cell({
                outputLines: [{ text: "line one", source: "agent", timestamp: Date.now() }],
                maxLines: 500,
                visible: true,
                runId: "run-auto",
                taskId: "task-auto",
                autoScroll: true,
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              const initialHtml = (yield* TBOutputWidget.render(ctx)).toString()
              expect(initialHtml).toContain('data-autoscroll="true"')

              yield* TBOutputWidget.handleEvent({ type: "toggleAutoScroll" }, ctx)

              const updated = yield* state.get
              expect(updated.autoScroll).toBe(false)

              const toggledHtml = (yield* TBOutputWidget.render(ctx)).toString()
              expect(toggledHtml).toContain('data-autoscroll="false"')
            }),
            layer
          )
        })
      )
    )
  })
})
