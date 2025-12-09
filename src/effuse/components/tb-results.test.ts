/**
 * TB Results Component Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { mountComponent } from "../component/mount.js"
import {
  TBResultsComponent,
} from "./tb-results.js"
import { makeTestLayer } from "../layers/test.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"

describe("TBResultsComponent", () => {
  test("renders empty state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-results-test" } as Element

          yield* mountComponent(TBResultsComponent, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("Run Results")
          expect(html).toContain("No results to display")
        })
      )
    )
  })

  test("US-6.1 displays run summary with pass rate and counts", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "tb-results-test" } as Element

          yield* mountComponent(TBResultsComponent, container).pipe(Effect.provide(layer))

          // Simulate run start
          yield* injectMessage({
            type: "tb_run_start",
            runId: "run-123",
            suiteName: "terminal-bench-v1",
            suiteVersion: "1.0.0",
            totalTasks: 5,
            taskIds: ["task1", "task2", "task3", "task4", "task5"],
            timestamp: "2024-12-06T10:00:00Z",
          })

          // Simulate tasks starting with metadata
          yield* injectMessage({
            type: "tb_task_start",
            runId: "run-123",
            taskId: "task1",
            taskName: "Create file",
            category: "file-ops",
            difficulty: "easy" as const,
            taskIndex: 0,
            totalTasks: 5,
          })

          yield* injectMessage({
            type: "tb_task_start",
            runId: "run-123",
            taskId: "task2",
            taskName: "List files",
            category: "file-ops",
            difficulty: "medium" as const,
            taskIndex: 1,
            totalTasks: 5,
          })

          // Simulate task completions
          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-123",
            taskId: "task1",
            outcome: "success" as const,
            durationMs: 5000,
            turns: 3,
            tokens: 1500,
          })

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-123",
            taskId: "task2",
            outcome: "failure" as const,
            durationMs: 8000,
            turns: 5,
            tokens: 2500,
          })

          // Simulate run complete
          yield* injectMessage({
            type: "tb_run_complete",
            runId: "run-123",
            passRate: 0.6,
            passed: 3,
            failed: 1,
            timeout: 1,
            error: 0,
            totalDurationMs: 45000,
          })

          const html = yield* getRendered(container)

          // Verify summary shows pass rate
          expect(html).toContain("60%") // Pass rate
          expect(html).toContain("✓3") // Passed count
          expect(html).toContain("✗1") // Failed count
          expect(html).toContain("⏱1") // Timeout count

          // Verify duration displayed
          expect(html).toContain("45s") // Total duration

          // Verify run ID visible
          expect(html).toContain(expect.stringContaining("run-123".slice(-8)))
        })
      )
    )
  })

  test("US-6.2 displays per-task results table with sortable columns", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "tb-results-test" } as Element

          yield* mountComponent(TBResultsComponent, container).pipe(Effect.provide(layer))

          // Start run
          yield* injectMessage({
            type: "tb_run_start",
            runId: "run-456",
            suiteName: "test-suite",
            suiteVersion: "1.0.0",
            totalTasks: 3,
            taskIds: ["t1", "t2", "t3"],
            timestamp: "2024-12-06T10:00:00Z",
          })

          // Task starts
          yield* injectMessage({
            type: "tb_task_start",
            runId: "run-456",
            taskId: "t1",
            taskName: "Task A",
            category: "cat1",
            difficulty: "easy" as const,
            taskIndex: 0,
            totalTasks: 3,
          })

          yield* injectMessage({
            type: "tb_task_start",
            runId: "run-456",
            taskId: "t2",
            taskName: "Task B",
            category: "cat2",
            difficulty: "hard" as const,
            taskIndex: 1,
            totalTasks: 3,
          })

          yield* injectMessage({
            type: "tb_task_start",
            runId: "run-456",
            taskId: "t3",
            taskName: "Task C",
            category: "cat1",
            difficulty: "medium" as const,
            taskIndex: 2,
            totalTasks: 3,
          })

          // Task completions
          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-456",
            taskId: "t1",
            outcome: "success" as const,
            durationMs: 3000,
            turns: 2,
            tokens: 1000,
          })

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-456",
            taskId: "t2",
            outcome: "timeout" as const,
            durationMs: 10000,
            turns: 8,
            tokens: 5000,
          })

          yield* injectMessage({
            type: "tb_task_complete",
            runId: "run-456",
            taskId: "t3",
            outcome: "failure" as const,
            durationMs: 6000,
            turns: 4,
            tokens: 2000,
          })

          // Complete run
          yield* injectMessage({
            type: "tb_run_complete",
            runId: "run-456",
            passRate: 0.33,
            passed: 1,
            failed: 1,
            timeout: 1,
            error: 0,
            totalDurationMs: 19000,
          })

          const html = yield* getRendered(container)

          // Verify table headers present
          expect(html).toContain("Task")
          expect(html).toContain("Diff")
          expect(html).toContain("Status")
          expect(html).toContain("Duration")
          expect(html).toContain("Turns")
          expect(html).toContain("Tokens")

          // Verify task names displayed
          expect(html).toContain("Task A")
          expect(html).toContain("Task B")
          expect(html).toContain("Task C")

          // Verify task IDs as subtitles
          expect(html).toContain("t1")
          expect(html).toContain("t2")
          expect(html).toContain("t3")

          // Verify outcome icons and labels
          expect(html).toContain("✓") // success icon
          expect(html).toContain("⏱") // timeout icon
          expect(html).toContain("✗") // failure icon
          expect(html).toContain("success")
          expect(html).toContain("timeout")
          expect(html).toContain("failure")

          // Verify difficulty badges
          expect(html).toContain("E") // easy
          expect(html).toContain("M") // medium
          expect(html).toContain("H") // hard

          // Verify metrics displayed
          expect(html).toContain("3s") // 3000ms
          expect(html).toContain("10s") // 10000ms
          expect(html).toContain("6s") // 6000ms
          expect(html).toContain("1,000") // tokens with comma separator
          expect(html).toContain("5,000")
          expect(html).toContain("2,000")
        })
      )
    )
  })

  test("filters tasks by outcome", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-results-test" } as Element
              const state = yield* stateService.cell(TBResultsComponent.initialState())

              // Pre-populate state with filter and result
              yield* state.update(() => ({
                ...TBResultsComponent.initialState(),
                currentResult: {
                  runId: "run-789",
                  suiteName: "test",
                  suiteVersion: "1.0.0",
                  passRate: 0.5,
                  passed: 1,
                  failed: 1,
                  timeout: 0,
                  error: 0,
                  totalDurationMs: 10000,
                  totalTasks: 2,
                  totalTokens: 3000,
                  taskResults: [
                    {
                      taskId: "t1",
                      taskName: "Success Task",
                      category: "cat1",
                      difficulty: "easy" as const,
                      outcome: "success" as const,
                      durationMs: 5000,
                      turns: 3,
                      tokens: 1500,
                    },
                    {
                      taskId: "t2",
                      taskName: "Failed Task",
                      category: "cat2",
                      difficulty: "hard" as const,
                      outcome: "failure" as const,
                      durationMs: 5000,
                      turns: 3,
                      tokens: 1500,
                    },
                  ],
                  timestamp: "2024-12-06T10:00:00Z",
                },
                outcomeFilter: "success" as const,
              }))

              const ctx = { state, emit: () => Effect.void, dom, container }
              yield* mountComponent(TBResultsComponent, container).pipe(Effect.provide(layer))
              const html = (yield* TBResultsComponent.render(ctx)).toString()

              // Should show success task
              expect(html).toContain("Success Task")
              // Should not show failed task when filtered to success
              expect(html).not.toContain("Failed Task")
              // Should show filter dropdown
              expect(html).toContain("All Outcomes")
            }),
            layer
          )
        })
      )
    )
  })

  test("sorts tasks by column", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "tb-results-test" } as Element
              const state = yield* stateService.cell(TBResultsComponent.initialState())

              // Pre-populate state with sort settings and result
              yield* state.update(() => ({
                ...TBResultsComponent.initialState(),
                currentResult: {
                  runId: "run-sort",
                  suiteName: "test",
                  suiteVersion: "1.0.0",
                  passRate: 0.5,
                  passed: 1,
                  failed: 1,
                  timeout: 0,
                  error: 0,
                  totalDurationMs: 10000,
                  totalTasks: 2,
                  totalTokens: 5000,
                  taskResults: [
                    {
                      taskId: "t1",
                      taskName: "Fast Task",
                      category: "cat1",
                      difficulty: "easy" as const,
                      outcome: "success" as const,
                      durationMs: 2000,
                      turns: 2,
                      tokens: 1000,
                    },
                    {
                      taskId: "t2",
                      taskName: "Slow Task",
                      category: "cat2",
                      difficulty: "hard" as const,
                      outcome: "failure" as const,
                      durationMs: 8000,
                      turns: 5,
                      tokens: 4000,
                    },
                  ],
                  timestamp: "2024-12-06T10:00:00Z",
                },
                sortBy: "duration" as const,
                sortDir: "desc" as const,
              }))

              const ctx = { state, emit: () => Effect.void, dom, container }
              yield* mountComponent(TBResultsComponent, container).pipe(Effect.provide(layer))
              const html = (yield* TBResultsComponent.render(ctx)).toString()

              // Verify sort indicator shown on duration column
              expect(html).toContain("Duration ▼")
              // Tasks should be present
              expect(html).toContain("Fast Task")
              expect(html).toContain("Slow Task")
            }),
            layer
          )
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-results-test" } as Element

          const customComponent = {
            ...TBResultsComponent,
            initialState: () => ({
              ...TBResultsComponent.initialState(),
              collapsed: true,
            }),
          }

          yield* mountComponent(customComponent, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Run Results")
          expect(html).toContain("▼") // Collapsed indicator
          // Should not show content when collapsed
          expect(html).not.toContain("Pass Rate")
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = TBResultsComponent.initialState()

    expect(state.currentResult).toBeNull()
    expect(state.activeTasks.size).toBe(0)
    expect(state.activeRunId).toBeNull()
    expect(state.collapsed).toBe(false)
    expect(state.sortBy).toBe("taskId")
    expect(state.sortDir).toBe("asc")
    expect(state.outcomeFilter).toBeNull()
  })
})
