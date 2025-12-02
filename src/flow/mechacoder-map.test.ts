import { describe, it, expect } from "bun:test"
import {
  buildMechaCoderFlowTree,
  createMechaCoderState,
  generateNodeSizes,
  NODE_SIZES,
} from "./mechacoder-map.js"
import type { FlowNode } from "./model.js"

describe("buildMechaCoderFlowTree", () => {
  it("builds tree matching sample-data structure", () => {
    const state = createMechaCoderState({
      repos: [
        {
          name: "openagents",
          path: "/code/openagents",
          tasks: [
            { id: "oa-b78d3f", title: "HUD-1 Flow model", status: "in_progress", priority: 1, type: "task" },
            { id: "oa-138548", title: "HUD-2 layout", status: "open", priority: 1, type: "task" },
            { id: "oa-91a779", title: "HUD-3 path", status: "open", priority: 2, type: "task" },
          ],
        },
        {
          name: "nostr-effect",
          path: "/code/nostr-effect",
          tasks: [
            { id: "ne-task1", title: "nostr-effect task 1", status: "open", priority: 2, type: "task" },
          ],
        },
      ],
      currentPhase: "edit",
      activeTaskId: "oa-b78d3f",
    })

    const tree = buildMechaCoderFlowTree(state)

    // Root structure
    expect(tree.id).toBe("root")
    expect(tree.type).toBe("root")
    expect(tree.label).toBe("OpenAgents Desktop")
    expect(tree.direction).toBe("horizontal")
    expect(tree.children).toHaveLength(1)

    // MechaCoder agent
    const mechacoder = tree.children![0]
    expect(mechacoder.id).toBe("mechacoder")
    expect(mechacoder.type).toBe("agent")
    expect(mechacoder.direction).toBe("vertical")
    expect(mechacoder.children).toHaveLength(3) // 2 repos + internal loop

    // First repo
    const repo1 = mechacoder.children![0]
    expect(repo1.id).toBe("repo-openagents")
    expect(repo1.type).toBe("repo")
    expect(repo1.children).toHaveLength(3)

    // Tasks sorted: in_progress first
    expect(repo1.children![0].id).toBe("oa-b78d3f")
    expect(repo1.children![0].metadata?.status).toBe("busy")

    // Second repo
    const repo2 = mechacoder.children![1]
    expect(repo2.id).toBe("repo-nostr-effect")
    expect(repo2.children).toHaveLength(1)

    // Internal loop
    const loop = mechacoder.children![2]
    expect(loop.id).toBe("internal-loop")
    expect(loop.type).toBe("workflow")
    expect(loop.direction).toBe("horizontal")
    expect(loop.children).toHaveLength(5)

    // Phases
    const phases = loop.children!.map(c => c.label)
    expect(phases).toEqual(["read", "plan", "edit", "test", "commit"])

    // Current phase is "edit"
    const editPhase = loop.children!.find(c => c.label === "edit")
    expect(editPhase?.metadata?.status).toBe("busy")
  })

  it("handles empty repos", () => {
    const state = createMechaCoderState({
      repos: [],
    })

    const tree = buildMechaCoderFlowTree(state)
    const mechacoder = tree.children![0]
    
    // Only internal loop
    expect(mechacoder.children).toHaveLength(1)
    expect(mechacoder.children![0].id).toBe("internal-loop")
  })

  it("handles repos with no tasks", () => {
    const state = createMechaCoderState({
      repos: [
        { name: "empty-repo", path: "/code/empty", tasks: [] },
      ],
    })

    const tree = buildMechaCoderFlowTree(state)
    const repo = tree.children![0].children![0]
    
    expect(repo.children).toHaveLength(0)
  })

  it("sorts tasks by status then priority", () => {
    const state = createMechaCoderState({
      repos: [
        {
          name: "test",
          path: "/test",
          tasks: [
            { id: "t1", title: "Low priority open", status: "open", priority: 3, type: "task" },
            { id: "t2", title: "High priority open", status: "open", priority: 1, type: "task" },
            { id: "t3", title: "In progress", status: "in_progress", priority: 2, type: "task" },
            { id: "t4", title: "Closed", status: "closed", priority: 0, type: "task" },
            { id: "t5", title: "Blocked", status: "blocked", priority: 1, type: "task" },
          ],
        },
      ],
    })

    const tree = buildMechaCoderFlowTree(state)
    const tasks = tree.children![0].children![0].children!

    // Order: in_progress, open (by priority), blocked, closed
    expect(tasks.map(t => t.id)).toEqual(["t3", "t2", "t1", "t5", "t4"])
  })

  it("maps task statuses correctly", () => {
    const state = createMechaCoderState({
      repos: [
        {
          name: "test",
          path: "/test",
          tasks: [
            { id: "t1", title: "Open", status: "open", priority: 1, type: "task" },
            { id: "t2", title: "In progress", status: "in_progress", priority: 1, type: "task" },
            { id: "t3", title: "Blocked", status: "blocked", priority: 1, type: "task" },
            { id: "t4", title: "Closed", status: "closed", priority: 1, type: "task" },
          ],
        },
      ],
    })

    const tree = buildMechaCoderFlowTree(state)
    const tasks = tree.children![0].children![0].children!

    const statusMap: Record<string, string> = {}
    for (const task of tasks) {
      statusMap[task.id] = task.metadata?.status as string
    }

    // Note: sorted by status
    expect(statusMap["t1"]).toBe("idle")
    expect(statusMap["t2"]).toBe("busy")
    expect(statusMap["t3"]).toBe("blocked")
    expect(statusMap["t4"]).toBe("completed")
  })

  it("includes metadata on nodes", () => {
    const state = createMechaCoderState({
      repos: [
        {
          name: "test",
          path: "/code/test",
          tasks: [
            { id: "t1", title: "Task with labels", status: "open", priority: 1, type: "feature", labels: ["hud", "flow"] },
          ],
        },
      ],
      currentPhase: "test",
      activeTaskId: "t1",
    })

    const tree = buildMechaCoderFlowTree(state)
    
    // MechaCoder metadata
    const mechacoder = tree.children![0]
    expect(mechacoder.metadata?.activeTaskId).toBe("t1")
    expect(mechacoder.metadata?.currentPhase).toBe("test")
    expect(mechacoder.metadata?.status).toBe("busy")

    // Repo metadata
    const repo = mechacoder.children![0]
    expect(repo.metadata?.path).toBe("/code/test")
    expect(repo.metadata?.taskCount).toBe(1)
    expect(repo.metadata?.openCount).toBe(1)

    // Task metadata
    const task = repo.children![0]
    expect(task.metadata?.priority).toBe(1)
    expect(task.metadata?.taskType).toBe("feature")
    expect(task.metadata?.labels).toEqual(["hud", "flow"])
  })

  it("truncates long task titles", () => {
    const state = createMechaCoderState({
      repos: [
        {
          name: "test",
          path: "/test",
          tasks: [
            { id: "t1", title: "This is a very long task title that should be truncated", status: "open", priority: 1, type: "task" },
          ],
        },
      ],
    })

    const tree = buildMechaCoderFlowTree(state)
    const task = tree.children![0].children![0].children![0]
    
    expect(task.label).toBe("t1: This is a very long task title...")
    expect(task.metadata?.fullTitle).toBe("This is a very long task title that should be truncated")
  })
})

describe("generateNodeSizes", () => {
  it("generates sizes for all nodes using defaults", () => {
    const tree: FlowNode = {
      id: "root",
      type: "root",
      label: "Root",
      children: [
        {
          id: "agent",
          type: "agent",
          label: "Agent",
          children: [
            { id: "task1", type: "task", label: "Task 1" },
            { id: "task2", type: "task", label: "Task 2" },
          ],
        },
      ],
    }

    const sizes = generateNodeSizes(tree)

    expect(sizes["root"]).toEqual(NODE_SIZES["root"])
    expect(sizes["agent"]).toEqual(NODE_SIZES["agent"])
    expect(sizes["task1"]).toEqual(NODE_SIZES["task"])
    expect(sizes["task2"]).toEqual(NODE_SIZES["task"])
  })

  it("applies overrides", () => {
    const tree: FlowNode = {
      id: "root",
      type: "root",
      label: "Root",
      children: [{ id: "task", type: "task", label: "Task" }],
    }

    const sizes = generateNodeSizes(tree, {
      task: { width: 300, height: 100 },
    })

    expect(sizes["root"]).toEqual(NODE_SIZES["root"])
    expect(sizes["task"]).toEqual({ width: 300, height: 100 })
  })

  it("uses default size for unknown types", () => {
    const tree: FlowNode = {
      id: "custom",
      type: "unknown-type",
      label: "Custom",
    }

    const sizes = generateNodeSizes(tree)

    expect(sizes["custom"]).toEqual({ width: 200, height: 60 })
  })
})

describe("createMechaCoderState", () => {
  it("creates state with defaults", () => {
    const state = createMechaCoderState({
      repos: [],
    })

    expect(state.repos).toEqual([])
    expect(state.currentPhase).toBe("idle")
    expect(state.activeTaskId).toBeNull()
    expect(state.recentRuns).toEqual([])
  })

  it("creates state with all options", () => {
    const state = createMechaCoderState({
      repos: [
        {
          name: "test",
          path: "/test",
          tasks: [{ id: "t1", title: "Task", status: "open", priority: 1, type: "task" }],
        },
      ],
      currentPhase: "edit",
      activeTaskId: "t1",
      recentRuns: [
        {
          id: "run-1",
          taskId: "t1",
          status: "success",
          startedAt: "2024-01-01T00:00:00Z",
          finishedAt: "2024-01-01T00:01:00Z",
          totalTurns: 5,
        },
      ],
    })

    expect(state.repos).toHaveLength(1)
    expect(state.currentPhase).toBe("edit")
    expect(state.activeTaskId).toBe("t1")
    expect(state.recentRuns).toHaveLength(1)
  })
})
