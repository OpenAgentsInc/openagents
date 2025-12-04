import { describe, it, expect } from "bun:test"
import {
  buildTBFlowTree,
  createEmptyTBFlowState,
  createTBFlowState,
  toggleRunExpanded,
  generateTBNodeSizes,
  type TBFlowState,
  type TBRunDetails,
} from "./tb-map.js"
import type { TBRunWithPath, TBTaskResult } from "../tbench-hud/persistence.js"

describe("TB Flow Map", () => {
  describe("createEmptyTBFlowState", () => {
    it("creates state with no runs and no current run", () => {
      const state = createEmptyTBFlowState()
      expect(state.runs).toEqual([])
      expect(state.currentRunId).toBeNull()
      expect(state.currentTaskId).toBeNull()
      expect(state.expandedRunIds.size).toBe(0)
    })
  })

  describe("buildTBFlowTree", () => {
    it("builds tree with controls and timeline nodes when no runs", () => {
      const state = createEmptyTBFlowState()
      const tree = buildTBFlowTree(state)

      expect(tree.id).toBe("tb-root")
      expect(tree.type).toBe("tb-root")
      expect(tree.children).toHaveLength(2)
      expect(tree.children![0].type).toBe("tb-controls")
      expect(tree.children![1].type).toBe("tb-timeline")
      expect(tree.metadata?.isRunning).toBe(false)
    })

    it("sets isRunning=true in metadata when currentRunId is set", () => {
      const state = createTBFlowState({
        runs: [],
        currentRunId: "run-123",
      })
      const tree = buildTBFlowTree(state)

      expect(tree.metadata?.isRunning).toBe(true)
      expect(tree.metadata?.currentRunId).toBe("run-123")
    })

    it("includes run summary nodes for past runs", () => {
      const mockRun: TBRunWithPath = {
        runId: "run-abc",
        suiteName: "Test Suite",
        suiteVersion: "1.0.0",
        timestamp: "2024-01-01T00:00:00Z",
        passRate: 0.75,
        passed: 3,
        failed: 1,
        timeout: 0,
        error: 0,
        totalDurationMs: 60000,
        totalTokens: 1000,
        taskCount: 4,
        filepath: "/tmp/run.json",
      }

      const state = createTBFlowState({
        runs: [mockRun],
      })
      const tree = buildTBFlowTree(state)

      // Timeline node should have the run as a child
      const timeline = tree.children![1]
      expect(timeline.children).toHaveLength(1)
      expect(timeline.children![0].type).toBe("tb-run-summary")
      expect(timeline.children![0].metadata?.runId).toBe("run-abc")
      expect(timeline.children![0].metadata?.passRate).toBe(0.75)
    })

    it("marks current run as busy status", () => {
      const mockRun: TBRunWithPath = {
        runId: "run-live",
        suiteName: "Live Suite",
        suiteVersion: "1.0.0",
        timestamp: "2024-01-01T00:00:00Z",
        passRate: 0.5,
        passed: 2,
        failed: 2,
        timeout: 0,
        error: 0,
        totalDurationMs: 30000,
        totalTokens: 500,
        taskCount: 4,
        filepath: "/tmp/run.json",
      }

      const state = createTBFlowState({
        runs: [mockRun],
        currentRunId: "run-live",
      })
      const tree = buildTBFlowTree(state)

      const timeline = tree.children![1]
      const runNode = timeline.children![0]
      expect(runNode.metadata?.isCurrentRun).toBe(true)
      expect(runNode.metadata?.status).toBe("busy")
    })

    it("expands run to show tasks when in expandedRunIds", () => {
      const mockRun: TBRunWithPath = {
        runId: "run-expanded",
        suiteName: "Expanded Suite",
        suiteVersion: "1.0.0",
        timestamp: "2024-01-01T00:00:00Z",
        passRate: 1.0,
        passed: 2,
        failed: 0,
        timeout: 0,
        error: 0,
        totalDurationMs: 10000,
        totalTokens: 200,
        taskCount: 2,
        filepath: "/tmp/run.json",
      }

      const mockTasks: TBTaskResult[] = [
        {
          id: "task-1",
          name: "Task One",
          category: "test",
          difficulty: "easy",
          outcome: "success",
          durationMs: 5000,
          turns: 3,
          tokens: 100,
        },
        {
          id: "task-2",
          name: "Task Two",
          category: "test",
          difficulty: "medium",
          outcome: "success",
          durationMs: 5000,
          turns: 5,
          tokens: 100,
        },
      ]

      const state = createTBFlowState({
        runs: [mockRun],
        expandedRunIds: new Set(["run-expanded"]),
      })

      const runDetails: Map<string, TBRunDetails> = new Map([
        ["run-expanded", { meta: mockRun, tasks: mockTasks }],
      ])

      const tree = buildTBFlowTree(state, runDetails)

      const timeline = tree.children![1]
      const runNode = timeline.children![0]

      // Should be expanded type with task children
      expect(runNode.type).toBe("tb-run-expanded")
      expect(runNode.children).toHaveLength(2)
      expect(runNode.children![0].type).toBe("tb-task")
      expect(runNode.children![0].metadata?.taskId).toBe("task-1")
    })
  })

  describe("toggleRunExpanded", () => {
    it("adds runId to expandedRunIds when not present", () => {
      const state = createEmptyTBFlowState()
      const newState = toggleRunExpanded(state, "run-123")

      expect(newState.expandedRunIds.has("run-123")).toBe(true)
    })

    it("removes runId from expandedRunIds when present", () => {
      const state = createTBFlowState({
        runs: [],
        expandedRunIds: new Set(["run-123"]),
      })
      const newState = toggleRunExpanded(state, "run-123")

      expect(newState.expandedRunIds.has("run-123")).toBe(false)
    })

    it("preserves other state when toggling", () => {
      const state = createTBFlowState({
        runs: [],
        currentRunId: "current-run",
        currentTaskId: "current-task",
        expandedRunIds: new Set(["other-run"]),
      })
      const newState = toggleRunExpanded(state, "new-run")

      expect(newState.currentRunId).toBe("current-run")
      expect(newState.currentTaskId).toBe("current-task")
      expect(newState.expandedRunIds.has("other-run")).toBe(true)
      expect(newState.expandedRunIds.has("new-run")).toBe(true)
    })
  })

  describe("generateTBNodeSizes", () => {
    it("generates sizes for all nodes in tree", () => {
      const state = createEmptyTBFlowState()
      const tree = buildTBFlowTree(state)
      const sizes = generateTBNodeSizes(tree)

      // Should have sizes for root, controls, and timeline
      expect(sizes["tb-root"]).toBeDefined()
      expect(sizes["tb-controls-node"]).toBeDefined()
      expect(sizes["tb-run-timeline"]).toBeDefined()
    })

    it("uses correct default sizes for node types", () => {
      const state = createEmptyTBFlowState()
      const tree = buildTBFlowTree(state)
      const sizes = generateTBNodeSizes(tree)

      expect(sizes["tb-root"].width).toBe(280)
      expect(sizes["tb-root"].height).toBe(80)
      expect(sizes["tb-controls-node"].width).toBe(260)
      expect(sizes["tb-controls-node"].height).toBe(100)
    })
  })

  describe("flow state sync scenarios", () => {
    // These tests ensure TB events properly update flow state

    it("currentRunId changes flow tree metadata", () => {
      // Simulate: tbState.isRunning = true, tbState.runId = "run-X"
      // Then sync: tbFlowState.currentRunId = "run-X"
      // Result: flow tree shows isRunning: true

      const stateIdle = createEmptyTBFlowState()
      const treeIdle = buildTBFlowTree(stateIdle)
      expect(treeIdle.metadata?.isRunning).toBe(false)

      // After run starts
      const stateRunning = createTBFlowState({
        runs: [],
        currentRunId: "run-X",
      })
      const treeRunning = buildTBFlowTree(stateRunning)
      expect(treeRunning.metadata?.isRunning).toBe(true)
      expect(treeRunning.metadata?.currentRunId).toBe("run-X")
    })

    it("currentTaskId affects task node status when expanded", () => {
      const mockRun: TBRunWithPath = {
        runId: "run-live",
        suiteName: "Live Suite",
        suiteVersion: "1.0.0",
        timestamp: "2024-01-01T00:00:00Z",
        passRate: 0,
        passed: 0,
        failed: 0,
        timeout: 0,
        error: 0,
        totalDurationMs: 0,
        totalTokens: 0,
        taskCount: 2,
        filepath: "/tmp/run.json",
      }

      const mockTasks: TBTaskResult[] = [
        {
          id: "task-1",
          name: "Task One",
          category: "test",
          difficulty: "easy",
          outcome: "success",
          durationMs: 0,
          turns: 0,
          tokens: 0,
        },
        {
          id: "task-2",
          name: "Task Two",
          category: "test",
          difficulty: "medium",
          outcome: "success",
          durationMs: 0,
          turns: 0,
          tokens: 0,
        },
      ]

      const state = createTBFlowState({
        runs: [mockRun],
        currentRunId: "run-live",
        currentTaskId: "task-2",
        expandedRunIds: new Set(["run-live"]),
      })

      const runDetails: Map<string, TBRunDetails> = new Map([
        ["run-live", { meta: mockRun, tasks: mockTasks }],
      ])

      const tree = buildTBFlowTree(state, runDetails)
      const timeline = tree.children![1]
      const runNode = timeline.children![0]

      // Find task-2 node - it should be busy since it's currentTaskId
      const task2 = runNode.children?.find((c) => c.metadata?.taskId === "task-2")
      expect(task2?.metadata?.status).toBe("busy")

      // task-1 should not be busy
      const task1 = runNode.children?.find((c) => c.metadata?.taskId === "task-1")
      expect(task1?.metadata?.status).not.toBe("busy")
    })
  })
})
