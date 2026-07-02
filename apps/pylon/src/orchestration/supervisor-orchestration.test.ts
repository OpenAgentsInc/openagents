import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import fc from "fast-check"
import { mkdtemp, readFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { dispatchEligibility, dispatchLiveness, dispatchReadySupervisorTasks } from "./coordinator.js"
import { parseOrchestrationGroupAddress, resolveOrchestrationGroup } from "./groups.js"
import {
  createMergeWaveResolverJob,
  decideMergePolicy,
  evaluateCloseoutReviewGate,
  type VerifyCommandEvidence,
} from "./merge-policy.js"
import { encodeAgentRunnerStatusEventForDispatchContext } from "./status-control.js"
import {
  FLEET_RUN_SCHEMA,
  WORK_CLAIM_SCHEMA,
  createPylonOrchestrationStore,
  fleetRunOwnerLocalStatePath,
  isAutoRevivableFleetRun,
  isStoredOrchestrationRunnerKind,
  loadFleetRunOwnerLocalState,
  normalizeOrchestrationRunnerKind,
  reconcileFleetRunsFromOwnerLocalState,
  saveFleetRunOwnerLocalState,
  syncFleetRunsToOwnerLocalState,
} from "./store.js"

const baseTaskSpec = {
  title: "Issue #6405",
  prompt: "Implement public issue #6405.",
  verifyCommand: "bun test",
  repo: "OpenAgentsInc/openagents",
  branch: "main",
  baseCommit: "abc123",
}

const greenVerifyEvidence = (commandRef = "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2"): VerifyCommandEvidence => ({
  commandRef,
  status: "green",
  exitCode: 0,
  workspaceKind: "worker_workspace",
  evidenceRef: "verify.public.fixture.green",
})

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
      supportedControlVerbs: ["status.list", "task.list", "task.update", "task.dispatch", "dispatch.cancel"],
    })
    expect(String(event.taskId)).toStartWith("task.public.pylon.")
    expect(String(event.dispatchContextId)).toStartWith("dispatch-context.public.pylon.")
    expect(String(event.pylonRef)).toStartWith("pylon.public.")
    expect(String(event.worktreeRef)).toStartWith("worktree.public.pylon.")
    expect(JSON.stringify(event)).not.toContain("ctx.codex.7809")
    expect(JSON.stringify(event)).not.toContain("/Users/private/worktree")
  })

  test("ingests agent runner status events as live status and updates dispatch contexts", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({
      id: "task.status-spine",
      spec: { ...baseTaskSpec, title: "Status spine", runnerKind: "codex" },
      now,
    })
    store.createDispatchContext({
      id: "ctx.status-spine",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      worktreeId: "wt-status-spine",
      worktreePath: "/Users/private/status-spine",
      lastHeartbeatAt: now,
      now,
    })

    const status = store.ingestAgentRunnerStatusEvent({
      event: {
        eventRef: "event.raw.status-spine.1",
        runnerRef: "runner.raw.codex-1",
        runnerKind: "codex",
        state: "working",
        stateStartedAt: "2026-07-01T12:01:00.000Z",
        updatedAt: "2026-07-01T12:01:00.000Z",
        assignmentRef: "assignment.secret.local",
        taskId: "task.status-spine",
        dispatchContextId: "ctx.status-spine",
        pylonRef: "pylon.local.secret",
        worktreeId: "wt-status-spine",
        refs: ["local:/Users/private/status-spine"],
      },
      now: new Date("2026-07-01T12:01:00.000Z"),
    })

    expect(status.retentionState).toBe("live")
    expect(status.state).toBe("working")
    expect(String(status.assignmentRef)).toStartWith("assignment.public.pylon.")
    expect(String(status.taskId)).toStartWith("task.public.pylon.")
    expect(String(status.dispatchContextId)).toStartWith("dispatch-context.public.pylon.")
    expect(String(status.refs?.[0])).toStartWith("ref.public.pylon.")
    expect(status.stateStartedAt).toBe("2026-07-01T12:01:00.000Z")
    expect(status.updatedAt).toBe("2026-07-01T12:01:00.000Z")
    expect(store.getDispatchContext("ctx.status-spine")).toMatchObject({
      status: "dispatched",
      currentTaskId: "task.status-spine",
      lastHeartbeatAt: "2026-07-01T12:01:00.000Z",
    })
    expect(JSON.stringify(status)).not.toContain("/Users/private")
    expect(JSON.stringify(status)).not.toContain("assignment.secret.local")
  })

  test("retains previous live status entries and rolls state history", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const base = {
      runnerRef: "runner.raw.codex-history",
      runnerKind: "codex",
      dispatchContextId: "ctx.history",
    }

    const working = store.ingestAgentRunnerStatusEvent({
      event: {
        ...base,
        eventRef: "event.history.working",
        state: "working",
        stateStartedAt: "2026-07-01T12:00:00.000Z",
        updatedAt: "2026-07-01T12:00:00.000Z",
      },
      now: new Date("2026-07-01T12:00:00.000Z"),
    })
    const waiting = store.ingestAgentRunnerStatusEvent({
      event: {
        ...base,
        eventRef: "event.history.waiting",
        state: "waiting",
        stateStartedAt: "2026-07-01T12:02:00.000Z",
        updatedAt: "2026-07-01T12:02:00.000Z",
      },
      now: new Date("2026-07-01T12:02:00.000Z"),
    })

    expect(store.getAgentRunnerStatusEvent(working.eventRef)?.retentionState).toBe("retained")
    expect(store.getAgentRunnerStatusEvent(working.eventRef)?.retainedAt).toBe("2026-07-01T12:02:00.000Z")
    expect(waiting.retentionState).toBe("live")
    expect(waiting.stateHistory).toEqual([
      { state: "working", stateStartedAt: "2026-07-01T12:00:00.000Z" },
      { state: "waiting", stateStartedAt: "2026-07-01T12:02:00.000Z" },
    ])
    expect(store.listAgentRunnerStatusEvents({ retentionState: "live" })).toHaveLength(1)
    expect(store.listAgentRunnerStatusEvents({ retentionState: "retained" })).toHaveLength(1)
  })

  test("caps runner status history at the requested rolling limit", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    let latest = null as ReturnType<typeof store.ingestAgentRunnerStatusEvent> | null
    for (let index = 0; index < 25; index += 1) {
      const state = index % 2 === 0 ? "working" : "waiting"
      const at = new Date(Date.UTC(2026, 6, 1, 12, index, 0))
      latest = store.ingestAgentRunnerStatusEvent({
        event: {
          eventRef: `event.rolling.${index}`,
          runnerRef: "runner.raw.rolling",
          runnerKind: "codex",
          state,
          stateStartedAt: at.toISOString(),
          updatedAt: at.toISOString(),
        },
        now: at,
        historyLimit: 20,
      })
    }

    expect(latest?.stateHistory).toHaveLength(20)
    expect(latest?.stateHistory?.[0]).toEqual({
      state: "waiting",
      stateStartedAt: "2026-07-01T12:05:00.000Z",
    })
    expect(latest?.stateHistory?.at(-1)).toEqual({
      state: "working",
      stateStartedAt: "2026-07-01T12:24:00.000Z",
    })
  })

  test("decays stale live runner statuses to idle while retaining the active entry", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const working = store.ingestAgentRunnerStatusEvent({
      event: {
        eventRef: "event.decay.working",
        runnerRef: "runner.raw.decay",
        runnerKind: "codex",
        state: "working",
        stateStartedAt: "2026-07-01T12:00:00.000Z",
        updatedAt: "2026-07-01T12:00:00.000Z",
      },
      now: new Date("2026-07-01T12:00:00.000Z"),
    })

    const decayed = store.decayAgentRunnerStatuses({
      now: new Date("2026-07-01T12:06:00.000Z"),
      staleAfterMs: 5 * 60 * 1000,
    })

    expect(decayed).toHaveLength(1)
    expect(decayed[0]?.state).toBe("idle")
    expect(decayed[0]?.stateStartedAt).toBe("2026-07-01T12:06:00.000Z")
    expect(store.getAgentRunnerStatusEvent(working.eventRef)?.retentionState).toBe("retained")
    expect(store.listAgentRunnerStatusEvents({ retentionState: "live" }).map((entry) => entry.state)).toEqual(["idle"])
    expect(decayed[0]?.stateHistory?.at(-1)).toEqual({
      state: "idle",
      stateStartedAt: "2026-07-01T12:06:00.000Z",
    })
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
        fleetRunRef: null,
        createdAt: "2026-06-27T12:00:00.000Z",
        updatedAt: "2026-06-27T12:00:00.000Z",
      },
    ])
    expect(JSON.stringify(snapshot)).not.toContain("Public issue prompt")
    expect(JSON.stringify(snapshot)).not.toContain("/private/local/worktree/path")
    expect(snapshot.dispatchContexts[0]?.worktreePath).toBeNull()
  })

  test("persists FleetRun records on the orchestration store with supervised-dispatch taxonomy", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = store.createFleetRun({
      runRef: "fleet_run.t2_1",
      objective: "Burn down the public Fable fixture backlog.",
      workSource: "fixture",
      targetConcurrency: 25,
      workerKind: "codex",
      state: "running",
      refillPolicy: {
        maxPerAccount: 3,
        cooldownAware: true,
        stopCondition: "backlog_empty",
      },
      now,
    })

    expect(run).toMatchObject({
      schema: FLEET_RUN_SCHEMA,
      runRef: "fleet_run.t2_1",
      workSource: "fixture",
      targetConcurrency: 25,
      workerKind: "codex",
      state: "running",
      dispatchKind: "supervised_dispatch",
      dagTracked: true,
      startedAt: "2026-07-01T12:00:00.000Z",
      counters: {
        workUnitsTotal: 0,
        activeAssignments: 0,
        completedAssignments: 0,
        failedAssignments: 0,
        blockedAssignments: 0,
      },
    })
    expect(store.getFleetRun("fleet_run.t2_1")).toEqual(run)
    expect(store.listFleetRuns("running").map((entry) => entry.runRef)).toEqual(["fleet_run.t2_1"])
  })

  test("rejects FleetRun handoffs that claim DAG tracking", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const handoff = store.createFleetRun({
      runRef: "fleet_run.handoff",
      objective: "One-shot handoff.",
      workSource: "fixture",
      targetConcurrency: 1,
      workerKind: "codex",
      dispatchKind: "handoff",
    })

    expect(handoff.dispatchKind).toBe("handoff")
    expect(handoff.dagTracked).toBe(false)
    expect(() => store.upsertFleetRun({ ...handoff, dagTracked: true })).toThrow(
      "fleet run handoff records must not be DAG-tracked",
    )
  })

  test("reconciles FleetRun counters from existing orchestration tasks", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const doneAt = new Date("2026-07-01T12:05:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createFleetRun({
      runRef: "fleet_run.reconcile",
      objective: "Run three fixture work units.",
      workSource: "fixture",
      targetConcurrency: 2,
      workerKind: "codex",
      state: "running",
      now,
    })
    store.createTask({
      id: "task.completed",
      spec: { ...baseTaskSpec, title: "completed", fleetRunRef: "fleet_run.reconcile" },
      now,
    })
    store.createTask({
      id: "task.dispatched",
      spec: { ...baseTaskSpec, title: "dispatched", fleetRunRef: "fleet_run.reconcile" },
      now,
    })
    store.createTask({
      id: "task.blocked",
      spec: { ...baseTaskSpec, title: "blocked", fleetRunRef: "fleet_run.reconcile" },
      now,
    })
    store.createDispatchContext({
      id: "ctx.codex.reconcile",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })
    store.markDispatched("task.dispatched", "ctx.codex.reconcile", now)
    store.completeTask("task.completed", JSON.stringify({ ok: true }), doneAt)
    store.updateTaskSpec("task.blocked", { ...baseTaskSpec, title: "blocked", fleetRunRef: "fleet_run.reconcile" }, doneAt)
    store.recordWorkerDone({
      contextId: "ctx.codex.reconcile",
      taskId: "task.dispatched",
      status: "completed",
      now: doneAt,
    })
    store.updateFleetRunState("fleet_run.reconcile", "running", doneAt)

    const reconciled = store.reconcileFleetRun("fleet_run.reconcile", doneAt)

    expect(reconciled.counters).toEqual({
      workUnitsTotal: 3,
      activeAssignments: 0,
      completedAssignments: 2,
      failedAssignments: 0,
      blockedAssignments: 0,
    })
    expect(reconciled.state).toBe("running")
  })

  test("reconciles completed FleetRuns when every DAG-tracked work unit is terminal", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createFleetRun({
      runRef: "fleet_run.completed",
      objective: "Finish a fixture work unit.",
      workSource: "fixture",
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now,
    })
    store.createTask({
      id: "task.done",
      spec: { ...baseTaskSpec, title: "done", fleetRunRef: "fleet_run.completed" },
      now,
    })
    store.completeTask("task.done", JSON.stringify({ ok: true }), now)

    const reconciled = store.reconcileFleetRun("fleet_run.completed", now)

    expect(reconciled.state).toBe("completed")
    expect(reconciled.counters.completedAssignments).toBe(1)
  })

  test("tracks state provenance: operator verbs stamp operator, reconcile auto-close stamps reconcile (#7975)", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createFleetRun({
      runRef: "fleet_run.provenance",
      objective: "Provenance fixture.",
      workSource: "fixture",
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now,
    })

    const paused = store.controlFleetRun("fleet_run.provenance", "pause", now).run
    expect(paused.stateSource).toBe("operator")
    expect(isAutoRevivableFleetRun(paused)).toBe(false)

    const stopped = store.controlFleetRun("fleet_run.provenance", "stop", now).run
    expect(stopped.stateSource).toBe("operator")
    expect(isAutoRevivableFleetRun(stopped)).toBe(false)

    store.createFleetRun({
      runRef: "fleet_run.provenance.close",
      objective: "Provenance auto-close fixture.",
      workSource: "fixture",
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      now,
    })
    store.createTask({
      id: "task.provenance.done",
      spec: { ...baseTaskSpec, title: "done", fleetRunRef: "fleet_run.provenance.close" },
      now,
    })
    store.completeTask("task.provenance.done", JSON.stringify({ ok: true }), now)
    const autoClosed = store.reconcileFleetRun("fleet_run.provenance.close", now)
    expect(autoClosed.state).toBe("completed")
    expect(autoClosed.stateSource).toBe("reconcile")
    expect(isAutoRevivableFleetRun(autoClosed)).toBe(true)

    // Legacy rows without provenance keep the historical auto-revive behavior.
    expect(isAutoRevivableFleetRun({ state: "completed", stateSource: undefined })).toBe(true)
    expect(isAutoRevivableFleetRun({ state: "paused", stateSource: undefined })).toBe(false)
  })

  test("mirrors FleetRun records to owner-local state and rehydrates a fresh store", async () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const home = await mkdtemp(join(tmpdir(), "pylon-fleet-runs-"))
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = store.createFleetRun({
      runRef: "fleet_run.owner_local",
      objective: "Resume fixture work after restart.",
      workSource: "fixture",
      targetConcurrency: 4,
      workerKind: "auto",
      state: "paused",
      now,
    })

    await syncFleetRunsToOwnerLocalState(store, { home })
    const statePath = fleetRunOwnerLocalStatePath({ home })
    const file = JSON.parse(await readFile(statePath, "utf8")) as { runs: unknown[] }
    expect(file.runs).toHaveLength(1)
    expect(file.runs[0]).toMatchObject({ runRef: run.runRef, schema: FLEET_RUN_SCHEMA })

    const freshStore = createPylonOrchestrationStore(new Database(":memory:"))
    const rehydrated = await reconcileFleetRunsFromOwnerLocalState(freshStore, { home }, { now })

    expect(rehydrated.map((entry) => entry.runRef)).toEqual(["fleet_run.owner_local"])
    expect(freshStore.getFleetRun("fleet_run.owner_local")?.state).toBe("paused")
    expect(await loadFleetRunOwnerLocalState({ home })).toMatchObject({
      runs: [{ runRef: "fleet_run.owner_local" }],
    })
  })

  test("owner-local FleetRun state rejects malformed persisted records", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-fleet-runs-bad-"))

    await expect(saveFleetRunOwnerLocalState({ home }, {
      schema: "openagents.khala_code.fleet_runs.owner_local.v1",
      runs: [
        {
          schema: FLEET_RUN_SCHEMA,
          runRef: "fleet_run.bad",
          objective: "bad",
          workSource: "fixture",
          targetConcurrency: 0,
          workerKind: "codex",
          refillPolicy: { maxPerAccount: 1, cooldownAware: true, stopCondition: "backlog_empty" },
          state: "draft",
          dispatchKind: "supervised_dispatch",
          dagTracked: true,
          startedAt: null,
          counters: {
            workUnitsTotal: 0,
            activeAssignments: 0,
            completedAssignments: 0,
            failedAssignments: 0,
            blockedAssignments: 0,
          },
          createdAt: "2026-07-01T12:00:00.000Z",
          updatedAt: "2026-07-01T12:00:00.000Z",
        },
      ],
    })).rejects.toThrow("fleet run targetConcurrency must be a positive integer")
  })

  test("enforces one live work claim per work unit with TTL reuse", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const first = store.tryClaimWorkUnit({
      claimRef: "claim.1",
      workUnitRef: "issue.7827",
      runRef: "fleet_run.claims",
      assignmentRef: "assignment.1",
      workerAccountRef: "codex-1",
      ttl: 60_000,
      now,
    })
    const duplicate = store.tryClaimWorkUnit({
      claimRef: "claim.2",
      workUnitRef: "issue.7827",
      runRef: "fleet_run.claims",
      assignmentRef: "assignment.2",
      workerAccountRef: "codex-2",
      ttl: 60_000,
      now,
    })

    expect(first).toMatchObject({
      schema: WORK_CLAIM_SCHEMA,
      claimRef: "claim.1",
      workUnitRef: "issue.7827",
      state: "claimed",
      workerAccountRef: "codex-1",
    })
    expect(duplicate).toBeNull()
    expect(store.listLiveWorkClaims(now).map((claim) => claim.claimRef)).toEqual(["claim.1"])
    expect(store.refreshLiveWorkClaim("issue.7827", new Date("2026-07-01T12:00:30.000Z"))).toMatchObject({
      claimRef: "claim.1",
      expiresAt: "2026-07-01T12:01:30.000Z",
    })
    expect(store.releaseLiveWorkClaim("issue.7827", new Date("2026-07-01T12:00:31.000Z"))).toMatchObject({
      claimRef: "claim.1",
      state: "released",
    })
    expect(store.listLiveWorkClaims(new Date("2026-07-01T12:00:31.000Z"))).toEqual([])

    const afterTtl = new Date("2026-07-01T12:01:00.001Z")
    const replacement = store.tryClaimWorkUnit({
      claimRef: "claim.3",
      workUnitRef: "issue.7827",
      runRef: "fleet_run.claims",
      assignmentRef: "assignment.3",
      workerAccountRef: "codex-3",
      ttl: 60_000,
      now: afterTtl,
    })

    expect(store.getWorkClaim("claim.1")?.state).toBe("released")
    expect(replacement?.claimRef).toBe("claim.3")
    expect(store.listLiveWorkClaims(afterTtl).map((claim) => claim.claimRef)).toEqual(["claim.3"])
  })

  test("releases live work claims when worker heartbeat evidence is dead", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createDispatchContext({
      id: "ctx.dead",
      assigneeHandle: "codex-dead",
      runnerKind: "codex",
      lastHeartbeatAt: new Date("2026-07-01T11:50:00.000Z"),
      now,
    })
    store.createDispatchContext({
      id: "ctx.fresh",
      assigneeHandle: "codex-fresh",
      runnerKind: "codex",
      lastHeartbeatAt: new Date("2026-07-01T11:59:30.000Z"),
      now,
    })
    store.tryClaimWorkUnit({
      claimRef: "claim.dead-worker",
      workUnitRef: "issue.dead-worker",
      runRef: "fleet_run.claims",
      workerAccountRef: "codex-dead",
      ttl: 60 * 60 * 1000,
      now,
    })
    store.tryClaimWorkUnit({
      claimRef: "claim.fresh-worker",
      workUnitRef: "issue.fresh-worker",
      runRef: "fleet_run.claims",
      workerAccountRef: "codex-fresh",
      ttl: 60 * 60 * 1000,
      now,
    })

    const reconciled = store.reconcileWorkClaims({ now, workerHeartbeatTtlMs: 5 * 60 * 1000 })

    expect(reconciled.expired).toEqual([])
    expect(reconciled.released.map((claim) => claim.claimRef)).toEqual(["claim.dead-worker"])
    expect(store.getWorkClaim("claim.dead-worker")?.state).toBe("released")
    expect(store.getWorkClaim("claim.fresh-worker")?.state).toBe("claimed")
  })

  test("property: concurrent claim interleavings and expiries never create duplicate live unit claims", () => {
    const operationArbitrary = fc.oneof(
      fc.record({
        kind: fc.constant("claim" as const),
        unit: fc.integer({ min: 0, max: 4 }),
        worker: fc.integer({ min: 0, max: 5 }),
        ttl: fc.integer({ min: 1, max: 50 }),
      }),
      fc.record({
        kind: fc.constant("advance" as const),
        ms: fc.integer({ min: 0, max: 75 }),
      }),
      fc.record({
        kind: fc.constant("expire" as const),
      }),
    )

    fc.assert(
      fc.property(fc.array(operationArbitrary, { minLength: 1, maxLength: 120 }), (operations) => {
        const store = createPylonOrchestrationStore(new Database(":memory:"))
        let nowMs = Date.parse("2026-07-01T12:00:00.000Z")
        let claimSeq = 0

        for (const operation of operations) {
          const now = new Date(nowMs)
          if (operation.kind === "claim") {
            store.tryClaimWorkUnit({
              claimRef: `claim.${claimSeq++}`,
              workUnitRef: `fixture.unit.${operation.unit}`,
              runRef: "fleet_run.property",
              assignmentRef: `assignment.${claimSeq}`,
              workerAccountRef: `worker.${operation.worker}`,
              ttl: operation.ttl,
              now,
            })
          } else if (operation.kind === "advance") {
            nowMs += operation.ms
            store.expireWorkClaims(new Date(nowMs))
          } else {
            store.expireWorkClaims(now)
          }

          const liveCounts = new Map<string, number>()
          for (const claim of store.listLiveWorkClaims(new Date(nowMs))) {
            liveCounts.set(claim.workUnitRef, (liveCounts.get(claim.workUnitRef) ?? 0) + 1)
          }
          expect([...liveCounts.values()].every((count) => count <= 1)).toBe(true)
        }
      }),
      { numRuns: 200 },
    )
  })

  test("closeout gate refuses ready_for_review without verify-green evidence", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const claim = store.tryClaimWorkUnit({
      claimRef: "claim.t4_3.verify",
      workUnitRef: "issue.7836",
      runRef: "fleet_run.t4_3",
      workerAccountRef: "codex-1",
      ttl: 60_000,
      now,
    })

    const gate = evaluateCloseoutReviewGate({
      pinnedVerifyCommandRef: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
      claim,
      verify: { ...greenVerifyEvidence(), status: "red", exitCode: 1 },
      now,
    })

    expect(gate).toMatchObject({
      status: "blocked",
      readyForReview: false,
      blockerRefs: ["blocker.public.pylon.closeout.verify_not_green"],
    })
  })

  test("closeout gate refuses ready_for_review when the claim expired", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const claim = store.tryClaimWorkUnit({
      claimRef: "claim.t4_3.expired",
      workUnitRef: "issue.7836",
      runRef: "fleet_run.t4_3",
      workerAccountRef: "codex-1",
      ttl: 1_000,
      now,
    })
    const afterTtl = new Date("2026-07-01T12:00:01.001Z")
    store.expireWorkClaims(afterTtl)

    const gate = evaluateCloseoutReviewGate({
      pinnedVerifyCommandRef: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
      claim: store.getWorkClaim(claim?.claimRef ?? ""),
      verify: greenVerifyEvidence(),
      now: afterTtl,
    })

    expect(gate).toMatchObject({
      status: "blocked",
      readyForReview: false,
      blockerRefs: ["blocker.public.pylon.closeout.claim_expired"],
    })
  })

  test("merge policy defaults to manual review and auto-merges only clean owner-toggled closeouts", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const claim = store.tryClaimWorkUnit({
      claimRef: "claim.t4_3.policy",
      workUnitRef: "issue.7836",
      runRef: "fleet_run.t4_3",
      workerAccountRef: "codex-1",
      ttl: 60_000,
      now,
    })
    const closeout = evaluateCloseoutReviewGate({
      pinnedVerifyCommandRef: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
      claim,
      verify: greenVerifyEvidence(),
      now,
    })

    expect(decideMergePolicy({
      closeout,
      mergeable: true,
      verifyGreen: true,
      hasConflicts: false,
      diffWithinScope: true,
    })).toMatchObject({
      mode: "manual_review",
      action: "manual_review",
      ownerToggleRequired: false,
    })

    expect(decideMergePolicy({
      mode: "auto_merge_clean",
      closeout,
      mergeable: true,
      verifyGreen: true,
      hasConflicts: false,
      diffWithinScope: true,
    })).toMatchObject({
      mode: "auto_merge_clean",
      action: "auto_merge",
      ownerToggleRequired: true,
    })

    expect(decideMergePolicy({
      mode: "auto_merge_clean",
      closeout,
      mergeable: false,
      verifyGreen: true,
      hasConflicts: true,
      diffWithinScope: true,
      siblingConflictRefs: ["pr.1", "pr.2"],
    })).toMatchObject({
      action: "merge_wave_resolver",
      ownerToggleRequired: true,
      blockerRefs: ["blocker.public.pylon.merge_conflict_wave"],
    })
  })

  test("Claude second-pass review is advisory: it can demote auto_merge_clean but never approve", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const claim = store.tryClaimWorkUnit({
      claimRef: "claim.t9_5.policy",
      workUnitRef: "issue.7874",
      runRef: "fleet_run.t9_5",
      workerAccountRef: "codex-1",
      ttl: 60_000,
      now,
    })
    const closeout = evaluateCloseoutReviewGate({
      pinnedVerifyCommandRef: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
      claim,
      verify: greenVerifyEvidence(),
      now,
    })

    expect(decideMergePolicy({
      mode: "manual_review",
      closeout,
      mergeable: true,
      verifyGreen: true,
      hasConflicts: false,
      diffWithinScope: true,
      advisoryReview: {
        reviewer: "claude_second_pass",
        recommendation: "approve",
        verdictRef: "review.public.pylon.claude_second_pass.clean",
      },
    })).toMatchObject({
      mode: "manual_review",
      action: "manual_review",
      ownerToggleRequired: false,
    })

    expect(decideMergePolicy({
      mode: "auto_merge_clean",
      closeout,
      mergeable: true,
      verifyGreen: true,
      hasConflicts: false,
      diffWithinScope: true,
      advisoryReview: {
        reviewer: "claude_second_pass",
        recommendation: "request_changes",
        verdictRef: "review.public.pylon.claude_second_pass.risky",
        riskRefs: ["risk.public.pylon.review.semantic_regression"],
      },
    })).toMatchObject({
      mode: "auto_merge_clean",
      action: "manual_review",
      ownerToggleRequired: false,
      blockerRefs: [
        "blocker.public.pylon.merge.claude_second_pass_manual_review",
        "risk.public.pylon.review.semantic_regression",
      ],
    })
  })

  test("merge-wave resolver has one live claim per wave work unit", () => {
    const now = new Date("2026-07-01T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const job = createMergeWaveResolverJob({
      waveRef: "wave.t4_3.conflicts",
      runRef: "fleet_run.t4_3",
      siblingRefs: ["pr.101", "pr.102"],
      workerAccountRef: "codex-merge",
      ttl: 60_000,
      now,
    })
    const duplicate = createMergeWaveResolverJob({
      waveRef: "wave.t4_3.conflicts",
      runRef: "fleet_run.t4_3",
      siblingRefs: ["pr.101", "pr.102"],
      workerAccountRef: "codex-merge-2",
      claimRef: "claim.public.pylon.merge_wave.duplicate",
      ttl: 60_000,
      now,
    })

    expect(job).toMatchObject({
      sequential: true,
      execution: "owner_toggle_required",
      refs: expect.arrayContaining(["blocker.public.pylon.merge_conflict_wave"]),
      taskSpec: {
        runnerKind: "codex",
        fleetRunRef: "fleet_run.t4_3",
      },
    })
    expect(store.tryClaimWorkUnit(job.claim)?.workUnitRef).toBe(job.workUnitRef)
    expect(store.tryClaimWorkUnit(duplicate.claim)).toBeNull()
    expect(store.listLiveWorkClaims(now).map((claim) => claim.claimRef)).toEqual([job.claim.claimRef])
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

  test("records worker heartbeat and done messages while releasing completed dispatches", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const heartbeatAt = new Date("2026-06-27T12:01:00.000Z")
    const doneAt = new Date("2026-06-27T12:02:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({ id: "task.root", spec: baseTaskSpec, now })
    store.createTask({
      id: "task.child",
      spec: { ...baseTaskSpec, title: "child" },
      deps: ["task.root"],
      now,
    })
    store.createDispatchContext({
      id: "ctx.codex",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })

    dispatchReadySupervisorTasks(store, { now })
    const heartbeat = store.recordWorkerHeartbeat({
      contextId: "ctx.codex",
      at: heartbeatAt,
      baseBehindBy: 2,
      body: "codex-1 still running task.root",
    })
    const contextAfterHeartbeat = store.getDispatchContext("ctx.codex")

    expect(heartbeat.kind).toBe("heartbeat")
    expect(heartbeat.threadId).toBe("task.root")
    expect(heartbeat.taskId).toBe("task.root")
    expect(contextAfterHeartbeat?.lastHeartbeatAt).toBe(heartbeatAt.toISOString())
    expect(contextAfterHeartbeat?.baseBehindBy).toBe(2)

    const released = store.recordWorkerDone({
      contextId: "ctx.codex",
      taskId: "task.root",
      status: "completed",
      result: JSON.stringify({ ok: true }),
      now: doneAt,
    })

    expect(store.getTask("task.root")?.status).toBe("completed")
    expect(store.getTask("task.child")?.status).toBe("ready")
    expect(released.status).toBe("idle")
    expect(released.currentTaskId).toBeNull()
    expect(released.failureCount).toBe(0)
    expect(store.getVirtualHead("OpenAgentsInc/openagents", "main")?.pendingTaskIds).toEqual([])
    expect(store.listMessages("task.root").map((message) => message.kind)).toEqual([
      "dispatch",
      "heartbeat",
      "worker_done",
    ])
  })

  test("worker failures increment the dispatch circuit breaker and preserve dependent blockers", () => {
    const now = new Date("2026-06-27T12:00:00.000Z")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createTask({ id: "task.fail", spec: baseTaskSpec, now })
    store.createTask({
      id: "task.dependent",
      spec: { ...baseTaskSpec, title: "dependent" },
      deps: ["task.fail"],
      now,
    })
    store.createDispatchContext({
      id: "ctx.codex",
      assigneeHandle: "codex-1",
      runnerKind: "codex",
      lastHeartbeatAt: now,
      now,
    })

    dispatchReadySupervisorTasks(store, { now })
    const context = store.recordWorkerDone({
      contextId: "ctx.codex",
      taskId: "task.fail",
      status: "failed",
      maxFailures: 1,
      now,
    })

    expect(store.getTask("task.fail")?.status).toBe("failed")
    expect(store.getTask("task.dependent")?.status).toBe("pending")
    expect(context.status).toBe("circuit_broken")
    expect(context.failureCount).toBe(1)
    expect(context.currentTaskId).toBeNull()
    expect(store.listMessages("task.fail").map((message) => message.kind)).toEqual([
      "dispatch",
      "worker_done",
    ])
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
