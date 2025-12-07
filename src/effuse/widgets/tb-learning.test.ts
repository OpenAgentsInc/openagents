/**
 * TB Learning Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { TBLearningWidget } from "./tb-learning.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"

describe("TBLearningWidget", () => {
  test("renders empty state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-learning-test" } as Element

          yield* mountWidget(TBLearningWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("Learning Metrics")
          expect(html).toContain("No learning metrics available")
        })
      )
    )
  })

  test("displays learning metrics from tb_learning_metrics message", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "tb-learning-test" } as Element

          yield* mountWidget(TBLearningWidget, container).pipe(Effect.provide(layer))

          // Start run
          yield* injectMessage({
            type: "tb_run_start",
            runId: "run-learning-123",
            suiteName: "test-suite",
            suiteVersion: "1.0.0",
            totalTasks: 5,
            taskIds: ["t1", "t2", "t3", "t4", "t5"],
            timestamp: "2024-12-06T10:00:00Z",
          })

          // Send learning metrics
          yield* injectMessage({
            type: "tb_learning_metrics",
            runId: "run-learning-123",
            taskId: "t1",
            model: "fm",
            skillsUsed: 3,
            skillIds: ["skill-1", "skill-2", "skill-3"],
            memoriesUsed: 2,
            reflexionEnabled: true,
            reflectionsGenerated: 1,
            newSkillsLearned: 0,
          })

          const html = yield* getRendered(container)

          // Verify model badge
          expect(html).toContain("fm")

          // Verify metrics displayed
          expect(html).toContain("Skills Used")
          expect(html).toContain("3") // skills count

          expect(html).toContain("Memories Used")
          expect(html).toContain("2") // memories count

          expect(html).toContain("Reflections")
          expect(html).toContain("1") // reflections count
          expect(html).toContain("✓ Enabled") // reflexion enabled

          expect(html).toContain("Skills Learned")
          expect(html).toContain("0") // new skills learned

          // Verify skill IDs shown
          expect(html).toContain("skill-1")
        })
      )
    )
  })

  test("accumulates metrics across multiple messages", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "tb-learning-test" } as Element

          yield* mountWidget(TBLearningWidget, container).pipe(Effect.provide(layer))

          // Start run
          yield* injectMessage({
            type: "tb_run_start",
            runId: "run-accum",
            suiteName: "test",
            suiteVersion: "1.0.0",
            totalTasks: 3,
            taskIds: ["t1", "t2", "t3"],
            timestamp: "2024-12-06T10:00:00Z",
          })

          // First task metrics
          yield* injectMessage({
            type: "tb_learning_metrics",
            runId: "run-accum",
            taskId: "t1",
            model: "fm",
            skillsUsed: 2,
            skillIds: ["skill-a", "skill-b"],
            memoriesUsed: 1,
            reflexionEnabled: true,
            reflectionsGenerated: 1,
            newSkillsLearned: 0,
          })

          // Second task metrics
          yield* injectMessage({
            type: "tb_learning_metrics",
            runId: "run-accum",
            taskId: "t2",
            model: "fm",
            skillsUsed: 3,
            skillIds: ["skill-b", "skill-c", "skill-d"],
            memoriesUsed: 2,
            reflexionEnabled: true,
            reflectionsGenerated: 2,
            newSkillsLearned: 1,
          })

          const html = yield* getRendered(container)

          // Verify accumulated totals
          expect(html).toContain("5") // 2 + 3 skills used
          expect(html).toContain("3") // 1 + 2 memories used
          expect(html).toContain("3") // 1 + 2 reflections
          expect(html).toContain("1") // 0 + 1 new skills learned

          // Verify unique skill IDs (should dedupe)
          expect(html).toContain("skill-a")
          expect(html).toContain("skill-b")
          expect(html).toContain("skill-c")
        })
      )
    )
  })

  test("displays learning summary from tb_learning_summary message", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "tb-learning-test" } as Element

          yield* mountWidget(TBLearningWidget, container).pipe(Effect.provide(layer))

          // Start run
          yield* injectMessage({
            type: "tb_run_start",
            runId: "run-summary",
            suiteName: "test",
            suiteVersion: "1.0.0",
            totalTasks: 10,
            taskIds: ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10"],
            timestamp: "2024-12-06T10:00:00Z",
          })

          // Send learning summary
          yield* injectMessage({
            type: "tb_learning_summary",
            runId: "run-summary",
            totalTasks: 10,
            passed: 8,
            passRate: 0.8,
            model: "fm",
            learningFlags: {
              skills: true,
              memory: true,
              reflexion: false,
              learn: true,
            },
            totalSkillsUsed: 25,
            totalMemoriesUsed: 15,
            totalReflectionsGenerated: 0,
            newSkillsLearned: 3,
            skillLibrarySize: 42,
          })

          const html = yield* getRendered(container)

          // Verify learning flags displayed
          expect(html).toContain("✓ Skills")
          expect(html).toContain("✓ Memory")
          expect(html).toContain("✓ Learn")
          // Reflexion should be shown as disabled (no checkmark)
          expect(html).toContain("Reflexion")

          // Verify summary metrics
          expect(html).toContain("25") // total skills used
          expect(html).toContain("15") // total memories used
          expect(html).toContain("3") // new skills learned
          expect(html).toContain("Library: 42") // skill library size

          // Verify run summary section
          expect(html).toContain("Run Summary")
          expect(html).toContain("10") // total tasks
          expect(html).toContain("8") // passed
          expect(html).toContain("80%") // pass rate
        })
      )
    )
  })

  test("shows enabled/disabled features correctly", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "tb-learning-test" } as Element

          yield* mountWidget(TBLearningWidget, container).pipe(Effect.provide(layer))

          // Start run
          yield* injectMessage({
            type: "tb_run_start",
            runId: "run-flags",
            suiteName: "test",
            suiteVersion: "1.0.0",
            totalTasks: 5,
            taskIds: ["t1"],
            timestamp: "2024-12-06T10:00:00Z",
          })

          // Send summary with mixed flags
          yield* injectMessage({
            type: "tb_learning_summary",
            runId: "run-flags",
            totalTasks: 5,
            passed: 3,
            passRate: 0.6,
            model: "fm",
            learningFlags: {
              skills: true,
              memory: false,
              reflexion: true,
              learn: false,
            },
            totalSkillsUsed: 10,
            totalMemoriesUsed: 0,
            totalReflectionsGenerated: 5,
            newSkillsLearned: 0,
          })

          const html = yield* getRendered(container)

          // Skills enabled (has checkmark)
          expect(html).toContain("✓ Skills")
          // Memory disabled (no checkmark in badge)
          expect(html).toMatch(/Memory<\/span>/)
          // Reflexion enabled
          expect(html).toContain("✓ Reflexion")
          // Learn disabled
          expect(html).toMatch(/Learn<\/span>/)
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "tb-learning-test" } as Element

          const customWidget = {
            ...TBLearningWidget,
            initialState: () => ({
              ...TBLearningWidget.initialState(),
              collapsed: true,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("Learning Metrics")
          expect(html).toContain("▼") // Collapsed indicator
          // Should not show content when collapsed
          expect(html).not.toContain("Skills Used")
        })
      )
    )
  })

  test("ignores messages from other runs", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered, injectMessage } = yield* makeTestLayer()
          const container = { id: "tb-learning-test" } as Element

          yield* mountWidget(TBLearningWidget, container).pipe(Effect.provide(layer))

          // Start run
          yield* injectMessage({
            type: "tb_run_start",
            runId: "run-correct",
            suiteName: "test",
            suiteVersion: "1.0.0",
            totalTasks: 5,
            taskIds: ["t1"],
            timestamp: "2024-12-06T10:00:00Z",
          })

          // Send metrics for wrong run
          yield* injectMessage({
            type: "tb_learning_metrics",
            runId: "run-wrong",
            taskId: "t1",
            model: "fm",
            skillsUsed: 999,
            skillIds: ["wrong-skill"],
            memoriesUsed: 999,
            reflexionEnabled: true,
            reflectionsGenerated: 999,
            newSkillsLearned: 999,
          })

          const html = yield* getRendered(container)

          // Should still show zeros (not the wrong run's data)
          expect(html).toContain("Skills Used")
          expect(html).toContain("0") // Should not be 999
          expect(html).not.toContain("999")
          expect(html).not.toContain("wrong-skill")
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = TBLearningWidget.initialState()

    expect(state.runId).toBeNull()
    expect(state.model).toBeNull()
    expect(state.learningFlags).toBeNull()
    expect(state.skillsUsed).toBe(0)
    expect(state.skillIds).toEqual([])
    expect(state.memoriesUsed).toBe(0)
    expect(state.reflexionEnabled).toBe(false)
    expect(state.reflectionsGenerated).toBe(0)
    expect(state.newSkillsLearned).toBe(0)
    expect(state.skillLibrarySize).toBeNull()
    expect(state.summary).toBeNull()
    expect(state.collapsed).toBe(false)
    expect(state.loading).toBe(false)
  })
})
