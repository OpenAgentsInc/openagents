import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"

import { dispatchEligibility, dispatchLiveness, dispatchReadySupervisorTasks } from "./coordinator.js"
import { resolveOrchestrationGroup } from "./groups.js"
import { createPylonOrchestrationStore } from "./store.js"

const baseTaskSpec = {
  title: "Issue #6405",
  prompt: "Implement public issue #6405.",
  verifyCommand: "bun test",
  repo: "OpenAgentsInc/openagents",
  branch: "main",
  baseCommit: "abc123",
}

describe("Pylon supervisor orchestration store", () => {
  test("persists a dependency DAG and promotes dependents when prerequisites complete", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))

    store.createTask({ id: "task.root", spec: baseTaskSpec })
    store.createTask({
      id: "task.child",
      parentId: "task.root",
      spec: { ...baseTaskSpec, title: "child" },
      deps: ["task.root"],
    })

    expect(store.getTask("task.root")?.status).toBe("ready")
    expect(store.getTask("task.child")?.status).toBe("pending")

    store.completeTask("task.root", JSON.stringify({ ok: true }))

    expect(store.getTask("task.child")?.status).toBe("ready")
  })

  test("records dispatch failures as a three-strike circuit breaker", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createDispatchContext({
      id: "ctx.codex.1",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: new Date("2026-06-27T12:00:00.000Z"),
    })

    expect(store.recordDispatchFailure("ctx.codex.1").status).toBe("idle")
    expect(store.recordDispatchFailure("ctx.codex.1").status).toBe("idle")
    const broken = store.recordDispatchFailure("ctx.codex.1")

    expect(broken.failureCount).toBe(3)
    expect(broken.status).toBe("circuit_broken")
    expect(dispatchEligibility(broken, { now: new Date("2026-06-27T12:01:00.000Z") })).toEqual({
      ok: false,
      reason: "circuit_broken",
    })
  })

  test("plans dispatch only for fresh, non-drifted idle contexts and persists assignment", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({ id: "task.a", spec: baseTaskSpec, now })
    store.createTask({ id: "task.b", spec: { ...baseTaskSpec, title: "b" }, now })
    store.createDispatchContext({
      id: "ctx.fresh",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      worktreeId: "wt-a",
      lastHeartbeatAt: new Date("2026-06-27T11:57:00.000Z"),
      now,
    })
    store.createDispatchContext({
      id: "ctx.stale",
      assigneeHandle: "codex-2",
      runnerKind: "codex",
      lastHeartbeatAt: new Date("2026-06-27T11:52:00.000Z"),
      now,
    })
    store.createDispatchContext({
      id: "ctx.drifted",
      assigneeHandle: "codex-3",
      runnerKind: "codex",
      lastHeartbeatAt: new Date("2026-06-27T11:59:00.000Z"),
      baseBehindBy: 25,
      now,
    })

    const result = dispatchReadySupervisorTasks(store, { now, maxBaseBehindBy: 20, maxConcurrentSlots: 2 })

    expect(result.planned.map((entry) => [entry.task.id, entry.context.id])).toEqual([["task.a", "ctx.fresh"]])
    expect(result.refused.map((entry) => [entry.context.id, entry.eligibility.ok ? "ok" : entry.eligibility.reason])).toEqual([
      ["ctx.stale", "heartbeat_stale"],
      ["ctx.drifted", "base_drift"],
    ])
    expect(store.getTask("task.a")?.status).toBe("dispatched")
    expect(store.getDispatchContext("ctx.fresh")?.currentTaskId).toBe("task.a")
  })

  test("tracks a virtual HEAD chain for concurrently dispatched git tasks", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({ id: "task.a", spec: baseTaskSpec, now })
    store.createTask({ id: "task.b", spec: { ...baseTaskSpec, title: "Issue #6406" }, now })
    store.createDispatchContext({
      id: "ctx.a",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })
    store.createDispatchContext({
      id: "ctx.b",
      assigneeHandle: "codex-2",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })

    const result = dispatchReadySupervisorTasks(store, { now, maxConcurrentSlots: 2 })
    const taskA = store.getTask("task.a")
    const taskB = store.getTask("task.b")
    const virtualHead = store.getVirtualHead("OpenAgentsInc/openagents", "main")

    expect(result.planned.map((entry) => entry.task.id)).toEqual(["task.a", "task.b"])
    expect(taskA?.spec.baseCommit).toBe("abc123")
    expect(taskB?.spec.baseCommit).toStartWith("virtual-head.")
    expect(taskB?.spec.baseCommit).not.toBe("abc123")
    expect(virtualHead?.projectedHead).toStartWith("virtual-head.")
    expect(virtualHead?.pendingTaskIds).toEqual(["task.a", "task.b"])

    store.completeTask("task.a", JSON.stringify({ ok: true }), now)

    expect(store.getVirtualHead("OpenAgentsInc/openagents", "main")?.pendingTaskIds).toEqual(["task.b"])
  })

  test("classifies heartbeat liveness with fresh, stale, hung, and missing states", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const fresh = store.createDispatchContext({
      id: "ctx.fresh",
      assigneeHandle: "codex-1",
      lastHeartbeatAt: new Date("2026-06-27T11:56:00.000Z"),
      now,
    })
    const stale = store.createDispatchContext({
      id: "ctx.stale",
      assigneeHandle: "codex-2",
      lastHeartbeatAt: new Date("2026-06-27T11:53:00.000Z"),
      now,
    })
    const hung = store.createDispatchContext({
      id: "ctx.hung",
      assigneeHandle: "codex-3",
      lastHeartbeatAt: new Date("2026-06-27T11:48:00.000Z"),
      now,
    })
    const missing = store.createDispatchContext({ id: "ctx.missing", assigneeHandle: "codex-4", now })

    expect(dispatchLiveness(fresh, { now })).toBe("fresh")
    expect(dispatchLiveness(stale, { now })).toBe("stale")
    expect(dispatchLiveness(hung, { now })).toBe("hung")
    expect(dispatchLiveness(missing, { now })).toBe("missing")
  })
})

describe("Pylon supervisor group addressing", () => {
  test("resolves @all, @idle, @worktree:<id>, and assignee addresses", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createDispatchContext({
      id: "ctx.a",
      assigneeHandle: "codex-1",
      worktreeId: "wt-a",
      lastHeartbeatAt: now,
      now,
    })
    store.createDispatchContext({
      id: "ctx.b",
      assigneeHandle: "claude-1",
      runnerKind: "claude",
      worktreeId: "wt-b",
      lastHeartbeatAt: now,
      now,
    })
    store.releaseDispatchContext("ctx.b", "blocked", now)

    const contexts = store.listDispatchContexts()

    expect(resolveOrchestrationGroup("@all", contexts).map((context) => context.id)).toEqual(["ctx.a", "ctx.b"])
    expect(resolveOrchestrationGroup("@idle", contexts).map((context) => context.id)).toEqual(["ctx.a"])
    expect(resolveOrchestrationGroup("@worktree:wt-b", contexts).map((context) => context.id)).toEqual(["ctx.b"])
    expect(resolveOrchestrationGroup("@codex-1", contexts).map((context) => context.id)).toEqual(["ctx.a"])
  })
})
