/**
 * TB Output Component Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { mountComponent } from "../component/mount.js"
import { makeCustomTestLayer, makeTestLayer } from "../layers/test.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"
import {
  TBOutputComponent,
  type TBOutputState,
  type TBOutputEvent,
  type TBOutputLine,
} from "./tb-output.js"

describe("TBOutputComponent", () => {
  test("renders hidden when not visible", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-output-test" } as Element

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

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
            { text: "Starting test...", source: "system" as const, timestamp: Date.now() },
            { text: "Running verification", source: "verification" as const, timestamp: Date.now() },
            { text: "Agent response here", source: "agent" as const, timestamp: Date.now() },
          ]

          const customComponent = {
            ...TBOutputComponent,
            initialState: (): TBOutputState => ({
              outputLines: mockLines,
              maxLines: 500,
              visible: true,
              runId: "run-abc12345",
              taskId: "task-001",
              autoScroll: true,
              showLineNumbers: true,
              selectedLine: null,
              visibleSources: { agent: true, verification: true, system: true, tool: true },
            }),
          }

          yield* mountComponent(customComponent, container).pipe(Effect.provide(layer))

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

          const customComponent = {
            ...TBOutputComponent,
            initialState: (): TBOutputState => ({
              outputLines: [],
              maxLines: 500,
              visible: true,
              runId: "run-xyz",
              taskId: null,
              autoScroll: true,
              showLineNumbers: true,
              selectedLine: null,
              visibleSources: { agent: true, verification: true, system: true, tool: true },
            }),
          }

          yield* mountComponent(customComponent, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("No output yet")
          expect(html).toContain("0 lines")
        })
      )
    )
  })

  test("US-5.9 close button hides the output viewer", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()
          // const { getRendered } = yield* makeTestLayer()
          const container = { id: "tb-output-test" } as Element

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag

              const state = yield* stateService.cell<TBOutputState>({
                outputLines: [{ text: "line 1", source: "system" as const, timestamp: Date.now() }],
                maxLines: 500,
                visible: true,
                runId: "run-123",
                taskId: "task-1",
                autoScroll: true,
                showLineNumbers: true,
                selectedLine: null,
                visibleSources: { agent: true, verification: true, system: true, tool: true },
              } as TBOutputState)
              const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

              const htmlVisible = (yield* TBOutputComponent.render(ctx)).toString()
              expect(htmlVisible).toContain("line 1")
              expect(htmlVisible).toContain("TB Output")

              yield* TBOutputComponent.handleEvent!({ type: "close" }, ctx)

              const htmlHidden = (yield* TBOutputComponent.render(ctx)).toString()
              expect(htmlHidden).toContain("hidden")
            }),
            layer
          )
        })
      )
    )
  })

  test("US-5.1 open event shows viewer when hidden", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-output-test" } as Element
              const state = yield* stateService.cell<TBOutputState>({
                outputLines: [],
              maxLines: 500,
              visible: false,
              runId: null,
              taskId: null,
              autoScroll: true,
              showLineNumbers: true,
              selectedLine: null,
              visibleSources: { agent: true, verification: true, system: true, tool: true },
            })
            const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

              let html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).toContain("hidden")

              yield* TBOutputComponent.handleEvent!({ type: "open" }, ctx)
              html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).toContain("TB Output")
            }),
            layer
          )
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

          const customComponent = {
            ...TBOutputComponent,
            initialState: (): TBOutputState => ({
              outputLines: mockLines,
              maxLines: 500,
              visible: true,
              runId: null,
              taskId: null,
              autoScroll: true,
              showLineNumbers: true,
              selectedLine: null,
              visibleSources: { agent: true, verification: true, system: true, tool: true },
            }),
          }

          yield* mountComponent(customComponent, container).pipe(Effect.provide(layer))

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
          const customComponent1 = {
            ...TBOutputComponent,
            initialState: (): TBOutputState => ({
              outputLines: [],
              maxLines: 500,
              visible: true,
              runId: null,
              taskId: null,
              autoScroll: true,
              showLineNumbers: true,
              selectedLine: null,
              visibleSources: { agent: true, verification: true, system: true, tool: true },
            }),
          }

          yield* mountComponent(customComponent1, container).pipe(Effect.provide(layer))
          const html1 = yield* getRendered(container)
          expect(html1).toContain('data-autoscroll="true"')

          // Auto-scroll disabled
          const customComponent2 = {
            ...TBOutputComponent,
            initialState: (): TBOutputState => ({
              outputLines: [],
              maxLines: 500,
              visible: true,
              runId: null,
              taskId: null,
              autoScroll: false,
              showLineNumbers: true,
              selectedLine: null,
              visibleSources: { agent: true, verification: true, system: true, tool: true },
            }),
          }

          yield* mountComponent(customComponent2, container).pipe(Effect.provide(layer))
          const html2 = yield* getRendered(container)
          expect(html2).toContain('data-autoscroll="false"')
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = TBOutputComponent.initialState()

    expect(state.outputLines).toEqual([])
    expect(state.maxLines).toBe(500)
    expect(state.visible).toBe(false)
    expect(state.runId).toBeNull()
    expect(state.taskId).toBeNull()
    expect(state.autoScroll).toBe(true)
    expect(state.showLineNumbers).toBe(true)
    expect(state.selectedLine).toBeNull()
    expect(state.visibleSources).toEqual({ agent: true, verification: true, system: true, tool: true })
  })

  test("US-5.1 streams live output on tb_task_output", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({
              type: "tb_run_start",
              runId: "run-live",
              suiteName: "test-suite",
              suiteVersion: "1.0.0",
              totalTasks: 1,
              taskIds: ["task-001"],
              timestamp: new Date().toISOString(),
            })
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

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({
              type: "tb_run_start",
              runId: "run-12345678",
              suiteName: "test-suite",
              suiteVersion: "1.0.0",
              totalTasks: 1,
              taskIds: ["task-xyz"],
              timestamp: new Date().toISOString(),
            })
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

  test("US-5.2 ignores output from other runs", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({
              type: "tb_run_start",
              runId: "run-active",
              suiteName: "test-suite",
              suiteVersion: "1.0.0",
              totalTasks: 1,
              taskIds: ["task-1"],
              timestamp: new Date().toISOString(),
            })
          yield* injectMessage({
            type: "tb_task_output",
            runId: "run-other",
            taskId: "task-x",
            text: "Should be ignored",
            source: "agent",
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).not.toContain("Should be ignored")
          expect(html).toContain("run-active".slice(-8))
        })
      )
    )
  })

  test("US-5.1 opens output and clears old lines on run start", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          const prefilledComponent = {
            ...TBOutputComponent,
            initialState: (): TBOutputState => ({
              outputLines: [{ text: "Old line", source: "agent" as const, timestamp: Date.now() }],
              maxLines: 500,
              visible: false,
              runId: "run-old",
              taskId: "task-old",
              autoScroll: true,
              showLineNumbers: true,
              selectedLine: null,
              visibleSources: { agent: true, verification: true, system: true, tool: true },
            }),
          }

          yield* mountComponent(prefilledComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({
              type: "tb_run_start",
              runId: "run-fresh",
              suiteName: "test-suite",
              suiteVersion: "1.0.0",
              totalTasks: 1,
              taskIds: ["task-1"],
              timestamp: new Date().toISOString(),
            })
          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("run-fresh".slice(-8))
          expect(html).not.toContain("Old line")
          expect(html).toContain("TB Output")
        })
      )
    )
  })

  test("US-5.1 keeps output visible after run completes and clears runId", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-complete", suiteName: "test-suite", suiteVersion: "1.0.0", totalTasks: 1, taskIds: ["task-1"], timestamp: "2025-01-01T00:00:00Z" })
          yield* injectMessage({
            type: "tb_task_output",
            runId: "run-complete",
            taskId: "task-1",
            text: "Finished line",
            source: "system",
          })
          yield* injectMessage({ type: "tb_run_complete", runId: "run-complete", passRate: 100, passed: 1, failed: 0, timeout: 0, error: 0, totalDurationMs: 1000 })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("Finished line")
          expect(html).not.toContain("run-complete") // runId cleared
          expect(html).toContain("TB Output")
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

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-verify", suiteName: "verification-suite", suiteVersion: "1.0.0", totalTasks: 1, taskIds: ["task-verify"], timestamp: "2025-01-01T00:00:00Z" })
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
              const state = yield* stateService.cell<TBOutputState>({
                outputLines: [
                  { text: "keep?", source: "agent", timestamp: Date.now() },
                  { text: "another", source: "system", timestamp: Date.now() },
                ],
                maxLines: 500,
                visible: true,
                runId: "run-clear",
                taskId: "task-clear",
                autoScroll: true,
                showLineNumbers: true,
                selectedLine: null,
                visibleSources: { agent: true, verification: true, system: true, tool: true },
              })
              const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

              yield* TBOutputComponent.handleEvent!({ type: "clear" }, ctx)

              const updated = yield* state.get
              expect(updated.outputLines.length).toBe(0)

              const html = (yield* TBOutputComponent.render(ctx)).toString()
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
                const state = yield* stateService.cell<TBOutputState>({
                  outputLines: [
                    { text: "line one", source: "agent", timestamp: Date.now() },
                    { text: "line two", source: "verification", timestamp: Date.now() },
                  ],
                  maxLines: 500,
                  visible: true,
                  runId: "run-copy",
                  taskId: "task-copy",
                  autoScroll: true,
                  showLineNumbers: true,
                  selectedLine: null,
                  visibleSources: { agent: true, verification: true, system: true, tool: true },
                })
                const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

                yield* TBOutputComponent.handleEvent!({ type: "copy" }, ctx)

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

  test("US-5.7 toggles output sources to show/hide streams", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-output-test" } as Element
              const state = yield* stateService.cell<TBOutputState>({
                outputLines: [
                  { text: "agent only", source: "agent", timestamp: Date.now() },
                  { text: "verify line", source: "verification", timestamp: Date.now() },
                  { text: "system note", source: "system", timestamp: Date.now() },
                ],
                maxLines: 500,
                visible: true,
                runId: "run-sources",
                taskId: "task-sources",
                autoScroll: true,
                showLineNumbers: true,
                selectedLine: null,
                visibleSources: { agent: true, verification: true, system: true, tool: true },
              })
              const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

              let html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).toContain("AGT")
              expect(html).toContain("VRF")
              expect(html).toContain("SYS")
              expect(html).toContain("3 lines")

              yield* TBOutputComponent.handleEvent!({ type: "toggleSource", source: "agent" }, ctx)
              html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).not.toContain("agent only")
              expect(html).toContain("VRF")
              expect(html).toContain("SYS")
              expect(html).toContain("2 lines")

              const updated = yield* state.get
              expect(updated.visibleSources.agent).toBe(false)
            }),
            layer
          )
        })
      )
    )
  })

  test("US-5.8 toggles line numbers and highlights selected line", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-output-test" } as Element
              const state = yield* stateService.cell<TBOutputState>({
                outputLines: [
                  { text: "first line", source: "agent", timestamp: Date.now() },
                  { text: "second line", source: "system", timestamp: Date.now() },
                ],
                maxLines: 500,
                visible: true,
                runId: "run-lines",
                taskId: "task-lines",
                autoScroll: true,
                showLineNumbers: true,
                selectedLine: null,
                visibleSources: { agent: true, verification: true, system: true, tool: true },
              })
              const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

              let html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).toContain('data-line="1"')
              expect(html).toContain('data-line="2"')

              yield* TBOutputComponent.handleEvent!({ type: "selectLine", lineNumber: 2 }, ctx)
              html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).toContain('data-line="2"')
              expect(html).toContain("bg-zinc-800/80")

              yield* TBOutputComponent.handleEvent!({ type: "toggleLineNumbers" }, ctx)
              const updated = yield* state.get
              expect(updated.showLineNumbers).toBe(false)
              expect(updated.selectedLine).toBeNull()

              html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).not.toContain('data-line="1"')
              expect(html).not.toContain('data-line="2"')
            }),
            layer
          )
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
              const state = yield* stateService.cell<TBOutputState>({
                outputLines: [{ text: "line one", source: "agent", timestamp: Date.now() }],
                maxLines: 500,
                visible: true,
                runId: "run-auto",
                taskId: "task-auto",
                autoScroll: true,
                showLineNumbers: true,
                selectedLine: null,
                visibleSources: { agent: true, verification: true, system: true, tool: true },
              })
              const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

              const initialHtml = (yield* TBOutputComponent.render(ctx)).toString()
              expect(initialHtml).toContain('data-autoscroll="true"')

              yield* TBOutputComponent.handleEvent!({ type: "toggleAutoScroll" }, ctx)

              const updated = yield* state.get
              expect(updated.autoScroll).toBe(false)

              const toggledHtml = (yield* TBOutputComponent.render(ctx)).toString()
              expect(toggledHtml).toContain('data-autoscroll="false"')
            }),
            layer
          )
        })
      )
    )
  })

  test("displays ATIF step with tool calls", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-atif", suiteName: "atif-suite", suiteVersion: "1.0.0", totalTasks: 1, taskIds: ["task-atif"], timestamp: "2025-01-01T00:00:00Z" })
          yield* injectMessage({
            type: "atif_step",
            runId: "run-atif",
            sessionId: "session-123",
            step: {
              step_id: 1,
              timestamp: new Date().toISOString(),
              source: "agent",
              message: "Let me read that file",
              tool_calls: [
                {
                  tool_call_id: "tc-001",
                  function_name: "read_file",
                  arguments: { file_path: "/src/test.ts" },
                },
              ],
            },
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("TL") // Tool label
          expect(html).toContain("read_file")
          expect(html).toContain("file_path")
        })
      )
    )
  })

  test("displays ATIF step with observation results", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-obs", suiteName: "obs-suite", suiteVersion: "1.0.0", totalTasks: 1, taskIds: ["task-obs"], timestamp: "2025-01-01T00:00:00Z" })
          yield* injectMessage({
            type: "atif_step",
            runId: "run-obs",
            sessionId: "session-456",
            step: {
              step_id: 2,
              timestamp: new Date().toISOString(),
              source: "system",
              message: "Tool execution results",
              observation: {
                results: [
                  {
                    source_call_id: "tc-001",
                    content: "File contents here",
                  },
                ],
              },
            },
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).toContain("TL") // Tool label
          expect(html).toContain("File contents here")
        })
      )
    )
  })

  test("ignores ATIF steps from other runs", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeCustomTestLayer({})
          const container = { id: "tb-output-test" } as Element

          yield* mountComponent(TBOutputComponent, container).pipe(Effect.provide(layer))

          yield* injectMessage({ type: "tb_run_start", runId: "run-current", suiteName: "current-suite", suiteVersion: "1.0.0", totalTasks: 1, taskIds: ["task-current"], timestamp: "2025-01-01T00:00:00Z" })
          yield* injectMessage({
            type: "atif_step",
            runId: "run-other",
            sessionId: "session-other",
            step: {
              step_id: 1,
              timestamp: new Date().toISOString(),
              source: "agent",
              message: "Should be ignored",
              tool_calls: [
                {
                  tool_call_id: "tc-ignored",
                  function_name: "ignored_tool",
                  arguments: {},
                },
              ],
            },
          })

          yield* Effect.sleep(0)

          const html = (yield* getRendered(container)) ?? ""
          expect(html).not.toContain("ignored_tool")
        })
      )
    )
  })

  test("toggles tool source visibility", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-output-test" } as Element
              const state = yield* stateService.cell<TBOutputState>({
                outputLines: [
                  { text: "→ read_file(path=\"/test.ts\")", source: "tool", timestamp: Date.now() },
                  { text: "Agent message", source: "agent", timestamp: Date.now() },
                ],
                maxLines: 500,
                visible: true,
                runId: "run-toggle",
                taskId: "task-toggle",
                autoScroll: true,
                showLineNumbers: true,
                selectedLine: null,
                visibleSources: { agent: true, verification: true, system: true, tool: true },
              })
              const ctx = { state, emit: (_event: TBOutputEvent) => Effect.succeed(undefined), dom, container }

              let html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).toContain("TL")
              expect(html).toContain("read_file")
              expect(html).toContain("2 lines")

              yield* TBOutputComponent.handleEvent!({ type: "toggleSource", source: "tool" }, ctx)
              html = (yield* TBOutputComponent.render(ctx)).toString()
              expect(html).not.toContain("read_file")
              expect(html).toContain("1 lines")

              const updated = yield* state.get
              expect(updated.visibleSources.tool).toBe(false)
            }),
            layer
          )
        })
      )
    )
  })
})
