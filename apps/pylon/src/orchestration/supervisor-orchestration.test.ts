import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"

import { dispatchEligibility, dispatchLiveness, dispatchReadySupervisorTasks } from "./coordinator.js"
import { parseOrchestrationGroupAddress, resolveOrchestrationGroup } from "./groups.js"
import { encodeAgentRunnerStatusEventForDispatchContext } from "./status-control.js"
import {
  createPylonOrchestrationStore,
  isStoredOrchestrationRunnerKind,
  normalizeOrchestrationRunnerKind,
} from "./store.js"

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
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({ id: "task.flaky", spec: baseTaskSpec, now })
    store.createDispatchContext({
      id: "ctx.codex.1",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })
    store.markDispatched("task.flaky", "ctx.codex.1", now)

    expect(store.recordDispatchFailure("ctx.codex.1", 3, now).status).toBe("idle")
    expect(store.getTask("task.flaky")?.status).toBe("failed")
    expect(store.recordDispatchFailure("ctx.codex.1").status).toBe("idle")
    const broken = store.recordDispatchFailure("ctx.codex.1")

    expect(broken.failureCount).toBe(3)
    expect(broken.status).toBe("circuit_broken")
    expect(dispatchEligibility(broken, { now: new Date("2026-06-27T12:01:00.000Z") })).toEqual({
      ok: false,
      reason: "circuit_broken",
    })
  })

  test("keeps circuit-broken dispatch contexts quarantined across heartbeats", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createDispatchContext({
      id: "ctx.flapping",
      assigneeHandle: "codex-flap",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })

    store.recordDispatchFailure("ctx.flapping", 1, now)
    const heartbeat = store.recordHeartbeat("ctx.flapping", {
      at: new Date("2026-06-27T12:01:00.000Z"),
      status: "idle",
    })

    expect(heartbeat.status).toBe("circuit_broken")
    expect(heartbeat.lastHeartbeatAt).toBe("2026-06-27T12:01:00.000Z")
    expect(dispatchEligibility(heartbeat, { now: new Date("2026-06-27T12:01:30.000Z") })).toEqual({
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

  test("uses registry runner kinds to match typed tasks to compatible contexts", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({
      id: "task.claude",
      spec: { ...baseTaskSpec, title: "Claude task", runnerKind: "claude_agent" },
      now,
    })
    store.createTask({
      id: "task.codex",
      spec: { ...baseTaskSpec, title: "Codex task", runnerKind: "codex" },
      now,
    })
    store.createDispatchContext({
      id: "ctx.codex",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })
    store.createDispatchContext({
      id: "ctx.claude",
      assigneeHandle: "claude-1",
      runnerKind: "claude_agent",
      lastHeartbeatAt: now,
      now,
    })

    const result = dispatchReadySupervisorTasks(store, { now, maxConcurrentSlots: 2 })

    expect(result.planned.map((entry) => [entry.task.id, entry.context.id])).toEqual([
      ["task.codex", "ctx.codex"],
      ["task.claude", "ctx.claude"],
    ])
    expect(store.getTask("task.codex")?.status).toBe("dispatched")
    expect(store.getTask("task.claude")?.status).toBe("dispatched")
  })

  test("projects dispatch contexts as runner-neutral fleet status events", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const task = store.createTask({
      id: "task.7809",
      spec: { ...baseTaskSpec, title: "Issue #7809", runnerKind: "codex" },
      now,
    })
    store.createDispatchContext({
      id: "ctx.codex.7809",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      worktreeId: "wt-7809",
      worktreePath: "/Users/private/worktree",
      lastHeartbeatAt: now,
      now,
    })
    store.markDispatched(task.id, "ctx.codex.7809", now)
    const context = store.getDispatchContext("ctx.codex.7809")

    expect(context).not.toBeNull()
    const event = encodeAgentRunnerStatusEventForDispatchContext({
      context: context!,
      task: store.getTask(task.id),
      pylonRef: "pylon.public.runner-status",
      now: new Date("2026-07-01T12:01:00.000Z"),
    })

    expect(event).toMatchObject({
      runnerKind: "codex",
      state: "working",
      taskId: "task.7809",
      dispatchContextId: "ctx.codex.7809",
      pylonRef: "pylon.public.runner-status",
      supportedControlVerbs: ["status.list", "task.list", "task.update", "task.dispatch", "dispatch.cancel"],
    })
    expect(String(event.worktreeRef)).toStartWith("worktree.public.pylon.")
    expect(JSON.stringify(event)).not.toContain("/Users/private/worktree")
  })

  test("normalizes legacy runner aliases through the AgentRunner registry vocabulary", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const task = store.createTask({
      id: "task.legacy-claude",
      spec: { ...baseTaskSpec, runnerKind: "claude_agent" },
    })
    const context = store.createDispatchContext({
      id: "ctx.legacy-claude",
      assigneeHandle: "claude-legacy",
      runnerKind: "claude",
      lastHeartbeatAt: new Date("2026-06-27T12:00:00.000Z"),
    })

    expect(task.spec.runnerKind).toBe("claude_agent")
    expect(context.runnerKind).toBe("claude_agent")
    expect(normalizeOrchestrationRunnerKind("claude")).toBe("claude_agent")
    expect(isStoredOrchestrationRunnerKind("claude_agent")).toBe(true)
    expect(isStoredOrchestrationRunnerKind("claude")).toBe(true)
    expect(isStoredOrchestrationRunnerKind("opencode")).toBe(false)
  })

  test("projects public-safe task and dispatch-context state without prompts or worktree paths", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({
      id: "task.public-safe",
      spec: {
        ...baseTaskSpec,
        prompt: "Public issue prompt with operational detail.",
        issueRef: "issue.7808",
        runnerKind: "codex",
      },
      now,
    })
    store.createDispatchContext({
      id: "ctx.public-safe",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      worktreeId: "wt-public-safe",
      worktreePath: "/private/local/worktree/path",
      lastHeartbeatAt: now,
      now,
    })

    const snapshot = store.publicSnapshot()

    expect(snapshot.tasks).toEqual([
      {
        id: "task.public-safe",
        parentId: null,
        threadId: "task.public-safe",
        status: "ready",
        deps: [],
        runnerKind: "codex",
        repo: "OpenAgentsInc/openagents",
        branch: "main",
        baseCommit: "abc123",
        issueRef: "issue.7808",
        createdAt: "2026-06-27T12:00:00.000Z",
        updatedAt: "2026-06-27T12:00:00.000Z",
      },
    ])
    expect(JSON.stringify(snapshot)).not.toContain("Public issue prompt")
    expect(JSON.stringify(snapshot)).not.toContain("/private/local/worktree/path")
    expect(snapshot.dispatchContexts[0]?.worktreePath).toBeNull()
  })

  test("refuses an otherwise healthy idle context when the ready task requires another runner", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({
      id: "task.claude",
      spec: { ...baseTaskSpec, title: "Claude task", runnerKind: "claude_agent" },
      now,
    })
    store.createDispatchContext({
      id: "ctx.codex",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })

    const result = dispatchReadySupervisorTasks(store, { now })

    expect(result.planned).toEqual([])
    expect(result.refused.map((entry) => [entry.context.id, entry.eligibility.ok ? "ok" : entry.eligibility.reason])).toEqual([
      ["ctx.codex", "runner_mismatch"],
    ])
    expect(store.getTask("task.claude")?.status).toBe("ready")
    expect(store.getDispatchContext("ctx.codex")?.status).toBe("idle")
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
      runnerKind: "codex",
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
    expect(resolveOrchestrationGroup("@runner:codex", contexts).map((context) => context.id)).toEqual(["ctx.a"])
    expect(resolveOrchestrationGroup("@runner:claude", contexts).map((context) => context.id)).toEqual(["ctx.b"])
    expect(resolveOrchestrationGroup("@runner:claude_agent", contexts).map((context) => context.id)).toEqual(["ctx.b"])
    expect(resolveOrchestrationGroup("@worktree:wt-b", contexts).map((context) => context.id)).toEqual(["ctx.b"])
    expect(resolveOrchestrationGroup("@codex-1", contexts).map((context) => context.id)).toEqual(["ctx.a"])
    expect(parseOrchestrationGroupAddress("@runner:generic")).toEqual({ kind: "runner", runnerKind: "generic" })
    expect(() => parseOrchestrationGroupAddress("@runner:opencode")).toThrow(
      "unsupported @runner group address: @runner:opencode",
    )
  })
})
