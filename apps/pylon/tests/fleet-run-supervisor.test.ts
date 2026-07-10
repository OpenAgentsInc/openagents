import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"
import { assertPublicProjectionSafe } from "../src/state.js"
import { hashPylonAccountRef } from "../src/account-registry.js"

import {
  tickFleetRunSupervisor,
  type FleetRunSupervisorAccount,
  type FleetRunSupervisorActiveAssignment,
  type FleetRunSupervisorDispatchInput,
  type FleetRunSupervisorObservedEvent,
  type FleetRunSupervisorRunner,
} from "../src/orchestration/fleet-run-supervisor.js"
import {
  createPylonOrchestrationStore,
  type FleetRun,
  type PylonOrchestrationStore,
} from "../src/orchestration/store.js"
import {
  fixtureCandidates,
  planWorkCandidates,
} from "../src/orchestration/work-planner.js"

const fixedNow = new Date("2026-07-09T12:00:00.000Z")

const lifecycleEvent = (
  event: PylonAssignmentRunLifecycleEvent["event"],
  status: PylonAssignmentRunLifecycleEvent["status"],
): PylonAssignmentRunLifecycleEvent => ({
  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
  event,
  observedAt: fixedNow.toISOString(),
  status,
})

const createRun = (
  store: PylonOrchestrationStore,
  input: {
    readonly runRef: string
    readonly workUnits: number
    readonly targetConcurrency?: number
    readonly workerKind?: "auto" | "claude" | "codex" | "grok"
  },
): FleetRun => store.createFleetRun({
  runRef: input.runRef,
  objective: "Run one pinned unit on every connected harness.",
  workSource: "fixture",
  targetConcurrency: input.targetConcurrency ?? 3,
  workerKind: input.workerKind ?? "auto",
  state: "running",
  startedAt: fixedNow,
  now: fixedNow,
  counters: { workUnitsTotal: input.workUnits },
})

const planner = (store: PylonOrchestrationStore, count: number) => ({
  plan: async (input: { readonly now: Date }) =>
    planWorkCandidates("fixture", fixtureCandidates({ kind: "fixture", count }), {
      claimRegistry: store,
      now: input.now,
    }),
})

const mixedCapacity: readonly FleetRunSupervisorAccount[] = [
  {
    accountRef: "codex-owner",
    advertisedCapacity: 1,
    marginalCostClass: "subscription",
    workerKind: "codex",
  },
  {
    accountRef: "claude-owner",
    advertisedCapacity: 1,
    marginalCostClass: "subscription",
    workerKind: "claude",
  },
  {
    accountRef: "grok-owner",
    advertisedCapacity: 1,
    marginalCostClass: "not_measured",
    workerKind: "grok",
  },
]

const capacity = () => ({ accounts: async () => mixedCapacity })

describe("Pylon-owned FleetRun supervisor", () => {
  test("waits for every residual attempt before emitting an operator-stop terminal", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc3.operator_stop_residual",
      workUnits: 2,
      targetConcurrency: 2,
      workerKind: "codex",
    })
    const observed: FleetRunSupervisorObservedEvent[] = []
    let releaseReconcile!: () => void
    let reconcileEntered!: () => void
    const heldReconcile = new Promise<void>(resolve => {
      releaseReconcile = resolve
    })
    const reconcileStarted = new Promise<void>(resolve => {
      reconcileEntered = resolve
    })
    let reconcileCalls = 0
    const options = {
      store,
      pylonRef: "pylon.owner.fc3.operator_stop_residual",
      runRef: run.runRef,
      planner: planner(store, 2),
      capacity: {
        accounts: async () => [{
          accountRef: "codex-owner",
          advertisedCapacity: 2,
          marginalCostClass: "subscription" as const,
          workerKind: "codex" as const,
        }],
      },
      runner: {
        dispatch: async (input: FleetRunSupervisorDispatchInput) => ({
          assignmentRef: `assignment.public.${input.workUnit.workUnitRef}`,
          lifecycle: [],
          status: "accepted" as const,
        }),
        reconcile: async ({
          activeAssignments,
        }: {
          readonly activeAssignments: readonly FleetRunSupervisorActiveAssignment[]
        }) => {
          reconcileCalls += 1
          if (reconcileCalls === 1) {
            reconcileEntered()
            await heldReconcile
            const assignment = activeAssignments[0]!
            return [{
              assignmentRef: assignment.claim.assignmentRef,
              lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
              status: "completed" as const,
              summary: "first residual closed",
              taskId: assignment.taskId,
            }]
          }
          return activeAssignments.map(assignment => ({
            assignmentRef: assignment.claim.assignmentRef,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed" as const,
            summary: "final residual closed",
            taskId: assignment.taskId,
          }))
        },
      },
      clock: { now: () => fixedNow },
      onLifecycle: (event: FleetRunSupervisorObservedEvent) => {
        observed.push(event)
      },
    }

    const initial = await tickFleetRunSupervisor(options)
    expect(initial.activeAssignments).toBe(2)
    observed.length = 0

    const heldTick = tickFleetRunSupervisor(options)
    await reconcileStarted
    store.controlFleetRun(run.runRef, "stop", fixedNow)
    releaseReconcile()
    const partiallyReconciled = await heldTick

    expect(partiallyReconciled.run.state).toBe("stopped")
    expect(partiallyReconciled.activeAssignments).toBe(1)
    expect(observed.filter(event => event.kind === "terminal")).toHaveLength(0)

    const terminalReconcile = await tickFleetRunSupervisor(options)
    expect(terminalReconcile.activeAssignments).toBe(0)
    expect(observed.filter(event => event.kind === "terminal")).toEqual([{
      kind: "terminal",
      runRef: run.runRef,
      terminalState: "stopped",
      blockerRefs: [],
    }])
  })

  test("emits a failed run terminal when the last local attempt fails or blocks", async () => {
    for (const status of ["failed", "blocked"] as const) {
      const store = createPylonOrchestrationStore(new Database(":memory:"))
      const run = createRun(store, {
        runRef: `fleet_run.fc3.terminal_${status}`,
        workUnits: 1,
        targetConcurrency: 1,
        workerKind: "codex",
      })
      const observed: FleetRunSupervisorObservedEvent[] = []
      const result = await tickFleetRunSupervisor({
        store,
        pylonRef: "pylon.owner.fc3.terminal",
        runRef: run.runRef,
        planner: planner(store, 1),
        capacity: {
          accounts: async () => [{
            accountRef: "codex-owner",
            advertisedCapacity: 1,
            marginalCostClass: "subscription",
            workerKind: "codex",
          }],
        },
        runner: {
          dispatch: async () => ({
            accountRefHash: null,
            assignmentRef: null,
            closeoutRef: null,
            lifecycle: [],
            status,
            summary: "The bounded fixture failed safely.",
            usageEvidence: null,
          }),
        },
        clock: { now: () => fixedNow },
        onLifecycle: async event => {
          observed.push(event)
        },
      })

      expect(result.run.state).toBe("stopped")
      expect(observed).toContainEqual({
        kind: "terminal",
        runRef: run.runRef,
        terminalState: "failed",
        blockerRefs: ["blocker.pylon.fleet_run.local_attempt_failed"],
      })
    }
  })

  test("projects the durable claim immediately and streams lifecycle before dispatch returns", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc3.lifecycle_immediate",
      workUnits: 1,
      targetConcurrency: 1,
      workerKind: "codex",
    })
    const observed: FleetRunSupervisorObservedEvent[] = []
    let release!: () => void
    let entered!: () => void
    const gate = new Promise<void>(resolve => {
      release = resolve
    })
    const started = new Promise<void>(resolve => {
      entered = resolve
    })
    const runtimeStarted = {
      ...lifecycleEvent("assignment_run.runtime_started", "running"),
      assignmentRef: "assignment.public.lifecycle_immediate",
      accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
    } satisfies PylonAssignmentRunLifecycleEvent
    const ticking = tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc3.lifecycle",
      runRef: run.runRef,
      planner: planner(store, 1),
      runner: {
        dispatch: async input => {
          await input.onLifecycle?.(runtimeStarted)
          entered()
          await gate
          return {
            assignmentRef: runtimeStarted.assignmentRef ?? null,
            lifecycle: [runtimeStarted],
            status: "accepted",
          }
        },
      },
      capacity: {
        accounts: async () => [mixedCapacity[0]!],
      },
      clock: { now: () => fixedNow },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    await started
    expect(observed.some(event =>
      event.kind === "dispatch" && event.status === "accepted" && event.assignmentRef === null
    )).toBe(true)
    expect(observed.some(event =>
      event.kind === "lifecycle" && event.event.event === "assignment_run.runtime_started"
    )).toBe(true)
    release()
    await ticking
    expect(observed.filter(event =>
      event.kind === "lifecycle" && event.event.event === "assignment_run.runtime_started"
    )).toHaveLength(1)
  })

  test("terminalizes locally before a slow lifecycle projection can trigger double reconciliation", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc3.slow_terminal_projection",
      workUnits: 1,
      targetConcurrency: 1,
      workerKind: "codex",
    })
    let releaseProjection!: () => void
    let markProjectionStarted!: () => void
    let markProjectionFinished!: () => void
    let markTerminalDispatchProjected!: () => void
    const projectionGate = new Promise<void>(resolve => {
      releaseProjection = resolve
    })
    const projectionStarted = new Promise<void>(resolve => {
      markProjectionStarted = resolve
    })
    const projectionFinished = new Promise<void>(resolve => {
      markProjectionFinished = resolve
    })
    const terminalDispatchProjected = new Promise<void>(resolve => {
      markTerminalDispatchProjected = resolve
    })
    let reconcileCalls = 0
    const observed: FleetRunSupervisorObservedEvent[] = []
    const options = {
      store,
      pylonRef: "pylon.owner.fc3.slow_terminal_projection",
      runRef: run.runRef,
      planner: planner(store, 1),
      capacity: { accounts: async () => [mixedCapacity[0]!] },
      clock: { now: () => fixedNow },
      runner: {
        dispatch: async () => ({
          assignmentRef: "assignment.public.slow_terminal_projection",
          lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
          status: "completed" as const,
          summary: "The exact fixture completed.",
        }),
        reconcile: async () => {
          reconcileCalls += 1
          return []
        },
      },
      onLifecycle: async (event: FleetRunSupervisorObservedEvent) => {
        observed.push(event)
        if (event.kind === "dispatch" && event.status === "completed") {
          markTerminalDispatchProjected()
        }
        if (event.kind !== "lifecycle") return
        markProjectionStarted()
        await projectionGate
        markProjectionFinished()
      },
    }

    await tickFleetRunSupervisor({ ...options, awaitDispatches: false })
    await projectionStarted

    const [task] = store.listTasks().filter(candidate =>
      candidate.spec.fleetRunRef === run.runRef
    )
    const [claim] = store.listWorkClaims({ runRef: run.runRef })
    const [context] = store.listDispatchContexts()
    expect(task?.status).toBe("completed")
    expect(claim).toMatchObject({
      assignmentRef: "assignment.public.slow_terminal_projection",
      state: "closeout",
    })
    expect(context).toMatchObject({ status: "idle", currentTaskId: null })

    const second = await tickFleetRunSupervisor(options)
    expect(second.activeAssignments).toBe(1)
    expect(reconcileCalls).toBe(0)
    expect(observed.some(event => event.kind === "completed")).toBe(false)
    expect(observed.some(event => event.kind === "terminal")).toBe(false)

    releaseProjection()
    await projectionFinished
    await terminalDispatchProjected
    // Let the fire-and-forget bookkeeping continuation clear its finalizing
    // marker after the terminal lifecycle sink returns.
    await Bun.sleep(1)
    expect(store.getTask(task!.id)?.status).toBe("completed")
    expect(store.getWorkClaim(claim!.claimRef)?.state).toBe("closeout")

    const third = await tickFleetRunSupervisor(options)
    expect(third.activeAssignments).toBe(0)
    const workTerminalIndex = observed.findIndex(
      event => event.kind === "dispatch" && event.status === "completed",
    )
    const runTerminalIndexes = observed.flatMap((event, index) =>
      event.kind === "completed" ? [index] : [],
    )
    expect(workTerminalIndex).toBeGreaterThanOrEqual(0)
    expect(runTerminalIndexes).toHaveLength(1)
    expect(runTerminalIndexes[0]!).toBeGreaterThan(workTerminalIndex)
  })

  test("binds a real executor approval signal to the exact live attempt and stable worker", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const pylonRef = "pylon.owner.fc3.approval"
    const authorityClaimRef = `claim.sarah_fleet_run.${"a".repeat(24)}`
    const baseRun = createRun(store, {
      runRef: "fleet_run.sarah.0123456789abcdef0123",
      workUnits: 1,
      targetConcurrency: 1,
      workerKind: "codex",
    })
    const run = store.upsertFleetRun({
      ...baseRun,
      authorityBinding: {
        schema: "openagents.pylon.fleet_run_authority_binding.v1",
        source: "sarah_authority",
        authorityFingerprint: "b".repeat(64),
        claimRef: authorityClaimRef,
        pylonRef,
        targetPreference: "owner_local",
        phase: "accepted",
      },
    })
    const assignmentRef = "assignment.public.fc3.approval"
    const approvalRef = "approval.public.fc3.write_file"
    const observed: FleetRunSupervisorObservedEvent[] = []
    let conflictingReplayRejected = false

    await tickFleetRunSupervisor({
      store,
      pylonRef,
      runRef: run.runRef,
      planner: planner(store, 1),
      capacity: { accounts: async () => [mixedCapacity[0]!] },
      clock: { now: () => fixedNow },
      runner: {
        dispatch: async input => {
          await input.onApprovalRequested?.({
            approvalRef,
            assignmentRef,
            toolClass: "write_file",
          })
          // Exact executor replay is idempotent; changing the bound tool is not.
          await input.onApprovalRequested?.({
            approvalRef,
            assignmentRef,
            toolClass: "write_file",
          })
          try {
            await input.onApprovalRequested?.({
              approvalRef,
              assignmentRef,
              toolClass: "shell_command",
            })
          } catch {
            conflictingReplayRejected = true
          }
          return {
            assignmentRef,
            lifecycle: [],
            status: "accepted",
          }
        },
      },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    const approval = store.getFleetRunSteeringApprovalBinding(approvalRef)
    expect(approval).toMatchObject({
      approvalRef,
      pylonRef,
      runRef: run.runRef,
      claimRef: authorityClaimRef,
      assignmentRef,
      workerKind: "codex",
      accountRefHash: hashPylonAccountRef("codex", "codex-owner"),
      toolClass: "write_file",
      state: "pending",
      createdAt: fixedNow.toISOString(),
    })
    expect(approval?.workerRef).toMatch(/^worker\.pylon\.codex\.[a-f0-9]{24}$/u)
    expect(approval?.workerRef).not.toContain("codex-owner")
    expect(store.getWorkClaim(approval?.workClaimRef ?? "")?.assignmentRef).toBe(
      assignmentRef,
    )
    expect(observed.filter(event => event.kind === "approval_requested")).toHaveLength(2)
    expect(conflictingReplayRejected).toBe(true)
  })

  test("gives simultaneous slots on one account distinct worker refs with stable replay", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const pylonRef = "pylon.owner.fc3.concurrent_approvals"
    const authorityClaimRef = `claim.sarah_fleet_run.${"c".repeat(24)}`
    const baseRun = createRun(store, {
      runRef: "fleet_run.sarah.concurrent_approvals",
      workUnits: 2,
      targetConcurrency: 2,
      workerKind: "codex",
    })
    const run = store.upsertFleetRun({
      ...baseRun,
      authorityBinding: {
        schema: "openagents.pylon.fleet_run_authority_binding.v1",
        source: "sarah_authority",
        authorityFingerprint: "d".repeat(64),
        claimRef: authorityClaimRef,
        pylonRef,
        targetPreference: "owner_local",
        phase: "accepted",
      },
    })
    const observed: FleetRunSupervisorObservedEvent[] = []
    let approvalsReady = 0
    let replayCount = 0
    let releaseApprovals!: () => void
    const bothApprovalsReady = new Promise<void>(resolve => {
      releaseApprovals = resolve
    })

    const result = await tickFleetRunSupervisor({
      store,
      pylonRef,
      runRef: run.runRef,
      planner: planner(store, 2),
      capacity: {
        accounts: async () => [{
          accountRef: "codex-owner",
          advertisedCapacity: 2,
          marginalCostClass: "subscription",
          workerKind: "codex",
        }],
      },
      clock: { now: () => fixedNow },
      runner: {
        dispatch: async input => {
          const suffix = input.workUnit.workUnitRef.replace(/[^a-zA-Z0-9_.-]/gu, "_")
          const request = {
            approvalRef: `approval.public.fc3.${suffix}`,
            assignmentRef: `assignment.public.fc3.${suffix}`,
            toolClass: "write_file",
          } as const
          await input.onApprovalRequested?.(request)
          approvalsReady += 1
          if (approvalsReady === 2) releaseApprovals()
          await bothApprovalsReady
          await input.onApprovalRequested?.(request)
          replayCount += 1
          return {
            assignmentRef: request.assignmentRef,
            lifecycle: [],
            status: "accepted",
          }
        },
      },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    const approvals = store.listFleetRunSteeringApprovalBindings({
      pylonRef,
      runRef: run.runRef,
      claimRef: authorityClaimRef,
      pendingOnly: false,
    })
    expect(result.activeAssignments).toBe(2)
    expect(result.dispatched).toBe(2)
    expect(approvalsReady).toBe(2)
    expect(replayCount).toBe(2)
    expect(approvals).toHaveLength(2)
    expect(new Set(approvals.map(approval => approval.workerRef)).size).toBe(2)
    expect(approvals.every(approval =>
      approval.workerRef !== null &&
      /^worker\.pylon\.codex\.[a-f0-9]{24}$/u.test(approval.workerRef)
    )).toBe(true)
    expect(observed.filter(event => event.kind === "approval_requested")).toHaveLength(4)
  })

  test("replays buffered lifecycle when the first live projection fails", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc3.lifecycle_retry",
      workUnits: 1,
      targetConcurrency: 1,
      workerKind: "codex",
    })
    const event = {
      ...lifecycleEvent("assignment_run.runtime_started", "running"),
      assignmentRef: "assignment.public.lifecycle_retry",
    } satisfies PylonAssignmentRunLifecycleEvent
    let lifecycleAttempts = 0
    const observed: FleetRunSupervisorObservedEvent[] = []
    await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc3.lifecycle-retry",
      runRef: run.runRef,
      planner: planner(store, 1),
      runner: {
        dispatch: async input => {
          await input.onLifecycle?.(event).catch(() => undefined)
          return {
            assignmentRef: event.assignmentRef ?? null,
            lifecycle: [event],
            status: "accepted",
          }
        },
      },
      capacity: { accounts: async () => [mixedCapacity[0]!] },
      clock: { now: () => fixedNow },
      onLifecycle: lifecycle => {
        if (lifecycle.kind === "lifecycle") {
          lifecycleAttempts += 1
          if (lifecycleAttempts === 1) throw new Error("transient projection failure")
        }
        observed.push(lifecycle)
      },
    })

    expect(lifecycleAttempts).toBe(2)
    expect(observed.filter(lifecycle => lifecycle.kind === "lifecycle")).toHaveLength(1)
  })

  test("starts one simultaneous stream per concrete harness and persists each worker kind", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc2.mixed_concurrent",
      workUnits: 3,
    })
    const dispatched: FleetRunSupervisorDispatchInput[] = []
    let release!: () => void
    const allStarted = new Promise<void>((resolve) => {
      release = resolve
    })
    const runner: FleetRunSupervisorRunner = {
      dispatch: async (input) => {
        dispatched.push(input)
        if (dispatched.length === 3) release()
        await allStarted
        return {
          assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
          lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
          status: "completed",
          summary: `${input.workerKind} fixture closed`,
        }
      },
    }

    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc2",
      runRef: run.runRef,
      planner: planner(store, 3),
      runner,
      capacity: capacity(),
      clock: { now: () => fixedNow },
    })

    expect(result.dispatched).toBe(3)
    expect(new Set(dispatched.map((entry) => entry.workerKind))).toEqual(
      new Set(["codex", "claude", "grok"]),
    )
    expect(new Set(dispatched.map((entry) => entry.accountRef))).toEqual(
      new Set(["codex-owner", "claude-owner", "grok-owner"]),
    )
    expect(store.listTasks().map((task) => task.spec.runnerKind).sort()).toEqual([
      "claude_agent",
      "codex",
      "grok_cli",
    ])
    expect(store.listWorkClaims({ runRef: run.runRef, state: "closeout" })).toHaveLength(3)
    expect(new Set(store.listWorkClaims({ runRef: run.runRef }).map((claim) => claim.workUnitRef)).size).toBe(3)
  })

  test("spreads the default three-slot auto wave across harnesses despite extra ready Codex capacity", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc5.default_harness_spread",
      workUnits: 3,
      targetConcurrency: 3,
    })
    const dispatched: FleetRunSupervisorDispatchInput[] = []
    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc5.spread",
      runRef: run.runRef,
      planner: planner(store, 3),
      capacity: {
        accounts: async () => [
          {
            accountRef: "codex-a",
            advertisedCapacity: 5,
            marginalCostClass: "subscription",
            workerKind: "codex",
          },
          {
            accountRef: "codex-b",
            advertisedCapacity: 5,
            marginalCostClass: "subscription",
            workerKind: "codex",
          },
          {
            accountRef: "claude-a",
            advertisedCapacity: 3,
            marginalCostClass: "subscription",
            workerKind: "claude",
          },
          {
            accountRef: "grok-a",
            advertisedCapacity: 3,
            marginalCostClass: "not_measured",
            workerKind: "grok",
          },
        ],
      },
      runner: {
        dispatch: async input => {
          dispatched.push(input)
          return {
            assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed",
          }
        },
      },
      clock: { now: () => fixedNow },
    })

    expect(result.dispatched).toBe(3)
    expect(dispatched.map(entry => entry.workerKind)).toEqual([
      "codex",
      "claude",
      "grok",
    ])
    expect(dispatched.filter(entry => entry.workerKind === "codex")).toHaveLength(1)
  })

  test("includes an already-live harness claim when spreading the next tick", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc5.live_harness_spread",
      workUnits: 2,
      targetConcurrency: 2,
    })
    const dispatched: FleetRunSupervisorDispatchInput[] = []
    const options = {
      store,
      pylonRef: "pylon.owner.fc5.live-spread",
      runRef: run.runRef,
      planner: planner(store, 2),
      capacity: capacity(),
      runner: {
        dispatch: async (input: FleetRunSupervisorDispatchInput) => {
          dispatched.push(input)
          return {
            assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
            lifecycle: [],
            status: "accepted" as const,
          }
        },
      },
      clock: { now: () => fixedNow },
      maxSpawnPerTick: 1,
    }

    expect((await tickFleetRunSupervisor(options)).dispatched).toBe(1)
    expect((await tickFleetRunSupervisor(options)).dispatched).toBe(1)
    expect(dispatched.map(entry => entry.workerKind)).toEqual(["codex", "claude"])
  })

  test("keeps a single default auto slot Codex-first", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc5.default_single_slot",
      workUnits: 1,
      targetConcurrency: 1,
    })
    const dispatched: FleetRunSupervisorDispatchInput[] = []
    await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc5.single",
      runRef: run.runRef,
      planner: planner(store, 1),
      capacity: capacity(),
      runner: {
        dispatch: async input => {
          dispatched.push(input)
          return {
            assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed",
          }
        },
      },
      clock: { now: () => fixedNow },
    })

    expect(dispatched.map(entry => entry.workerKind)).toEqual(["codex"])
  })

  test("records an exhausted Grok fallback before filling the next tied harness slot", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc5.exhausted_grok_spread",
      workUnits: 3,
      targetConcurrency: 3,
    })
    const observed: FleetRunSupervisorObservedEvent[] = []
    const dispatched: FleetRunSupervisorDispatchInput[] = []
    await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc5.exhausted-grok",
      runRef: run.runRef,
      planner: planner(store, 3),
      capacity: {
        accounts: async () => [
          {
            accountRef: "codex-a",
            advertisedCapacity: 3,
            marginalCostClass: "subscription",
            workerKind: "codex",
          },
          {
            accountRef: "claude-a",
            advertisedCapacity: 3,
            marginalCostClass: "subscription",
            workerKind: "claude",
          },
          {
            accountRef: "grok-exhausted",
            advertisedCapacity: 0,
            marginalCostClass: "not_measured",
            unavailabilityReason: "account_exhausted",
            workerKind: "grok",
          },
        ],
      },
      runner: {
        dispatch: async input => {
          dispatched.push(input)
          return {
            assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed",
          }
        },
      },
      clock: { now: () => fixedNow },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    expect(dispatched.map(entry => entry.workerKind)).toEqual([
      "codex",
      "claude",
      "codex",
    ])
    expect(dispatched.some(entry => entry.workerKind === "grok")).toBe(false)
    expect(observed.filter(event => event.kind === "fallback")).toContainEqual({
      kind: "fallback",
      runRef: run.runRef,
      policySchema: "khala.fleet_auto_policy.v1",
      event: expect.objectContaining({
        type: "account_exhausted",
        harnessKind: "grok",
        accountRef: "grok-exhausted",
      }),
    })
  })

  test("does not fabricate a Grok selection when no Grok account exists", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc5.missing_grok",
      workUnits: 3,
      targetConcurrency: 3,
    })
    const dispatched: FleetRunSupervisorDispatchInput[] = []
    await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc5.missing-grok",
      runRef: run.runRef,
      planner: planner(store, 3),
      capacity: {
        accounts: async () => [
          { ...mixedCapacity[0]!, advertisedCapacity: 3 },
          { ...mixedCapacity[1]!, advertisedCapacity: 3 },
        ],
      },
      runner: {
        dispatch: async input => {
          dispatched.push(input)
          return {
            assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed",
          }
        },
      },
      clock: { now: () => fixedNow },
    })

    expect(dispatched.map(entry => entry.workerKind)).toEqual([
      "codex",
      "claude",
      "codex",
    ])
    expect(dispatched.some(entry => entry.workerKind === "grok")).toBe(false)
  })

  test("uses the shared default auto policy and emits one public-safe fallback per skipped candidate", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc2.typed_auto_fallback",
      workUnits: 1,
      targetConcurrency: 1,
    })
    const observed: FleetRunSupervisorObservedEvent[] = []
    const dispatched: FleetRunSupervisorDispatchInput[] = []
    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc2.auto",
      runRef: run.runRef,
      planner: planner(store, 1),
      runner: {
        dispatch: async input => {
          dispatched.push(input)
          return {
            assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed",
          }
        },
      },
      capacity: {
        accounts: async () => [
          {
            accountRef: "codex-exhausted",
            advertisedCapacity: 0,
            marginalCostClass: "subscription",
            unavailabilityReason: "account_exhausted",
            workerKind: "codex",
          },
          {
            accountRef: "codex-reauth",
            advertisedCapacity: 0,
            marginalCostClass: "subscription",
            unavailabilityReason: "account_requires_reauth",
            workerKind: "codex",
          },
          {
            accountRef: "claude-rate-limited",
            advertisedCapacity: 0,
            marginalCostClass: "subscription",
            unavailabilityReason: "account_rate_limited",
            workerKind: "claude",
          },
          {
            accountRef: "claude-unavailable",
            advertisedCapacity: 0,
            marginalCostClass: "not_measured",
            unavailabilityReason: "account_unavailable",
            workerKind: "claude",
          },
          {
            accountRef: "grok-ready",
            advertisedCapacity: 1,
            marginalCostClass: "not_measured",
            workerKind: "grok",
          },
        ],
      },
      clock: { now: () => fixedNow },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    expect(result.dispatched).toBe(1)
    expect(dispatched.map(entry => [entry.accountRef, entry.workerKind])).toEqual([
      ["grok-ready", "grok"],
    ])
    const fallbacks = observed.filter(event => event.kind === "fallback")
    expect(fallbacks.map(({ event }) => [event.accountRef, event.type])).toEqual([
      ["codex-exhausted", "account_exhausted"],
      ["codex-reauth", "account_requires_reauth"],
      ["claude-rate-limited", "account_rate_limited"],
      ["claude-unavailable", "account_unavailable"],
    ])
    expect(fallbacks.every(event => event.policySchema === "khala.fleet_auto_policy.v1")).toBe(true)
    expect(() => assertPublicProjectionSafe(fallbacks, "fleetAutoFallbacks")).not.toThrow()
  })

  test("emits a typed cost-ceiling fallback before selecting an allowed named account", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc2.typed_auto_cost_ceiling",
      workUnits: 1,
      targetConcurrency: 1,
    })
    const observed: FleetRunSupervisorObservedEvent[] = []
    const selected: string[] = []
    await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc2.auto-cost",
      runRef: run.runRef,
      planner: planner(store, 1),
      runner: {
        dispatch: async input => {
          selected.push(input.accountRef)
          return {
            assignmentRef: `assignment.${input.workerKind}.${input.claim.claimRef}`,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed",
          }
        },
      },
      capacity: {
        accounts: async () => [
          {
            accountRef: "grok-metered",
            advertisedCapacity: 1,
            marginalCostClass: "api_metered",
            workerKind: "grok",
          },
          {
            accountRef: "codex-subscription",
            advertisedCapacity: 1,
            marginalCostClass: "subscription",
            workerKind: "codex",
          },
        ],
      },
      autoPolicy: {
        schema: "khala.fleet_auto_policy.v1",
        preferenceOrder: ["grok", "codex"],
        maxMarginalCostClass: "subscription",
      },
      clock: { now: () => fixedNow },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    expect(selected).toEqual(["codex-subscription"])
    expect(observed.filter(event => event.kind === "fallback")).toEqual([
      {
        kind: "fallback",
        runRef: run.runRef,
        policySchema: "khala.fleet_auto_policy.v1",
        event: {
          type: "cost_ceiling_exceeded",
          harnessKind: "grok",
          accountRef: "grok-metered",
          nextHarnessKind: "codex",
          nextAccountRef: "codex-subscription",
        },
      },
    ])
  })

  test("records a failed Grok account as a durable account-lane breaker", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc2.grok_breaker",
      workUnits: 1,
      targetConcurrency: 1,
      workerKind: "grok",
    })
    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc2.grok-breaker",
      runRef: run.runRef,
      planner: planner(store, 1),
      runner: {
        dispatch: async input => ({
          assignmentRef: `assignment.grok.${input.claim.claimRef}`,
          lifecycle: [{
            ...lifecycleEvent("assignment_run.completed", "rejected"),
            blockerRefs: ["blocker.pylon.fleet_runner.grok_account_rate_limited"],
          }],
          status: "failed",
          summary: "The named Grok account was rate limited.",
        }),
      },
      capacity: {
        accounts: async () => [{
          accountRef: "grok-throttled",
          advertisedCapacity: 1,
          marginalCostClass: "not_measured",
          workerKind: "grok",
        }],
      },
      clock: { now: () => fixedNow },
    })

    expect(result.dispatched).toBe(1)
    expect(store.listActiveDispatchBreakers(fixedNow)).toEqual([
      expect.objectContaining({
        accountRefHash: hashPylonAccountRef("grok", "grok-throttled"),
        failureKind: "transient",
        lane: "grok",
        reason: "account_rate_limited",
      }),
    ])
  })

  test("does not quarantine a healthy account for task or verifier failure", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc2.grok_task_failure",
      workUnits: 1,
      targetConcurrency: 1,
      workerKind: "grok",
    })
    await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc2.grok-task-failure",
      runRef: run.runRef,
      planner: planner(store, 1),
      runner: {
        dispatch: async input => ({
          assignmentRef: `assignment.grok.${input.claim.claimRef}`,
          lifecycle: [{
            ...lifecycleEvent("assignment_run.completed", "rejected"),
            blockerRefs: ["blocker.pylon.fleet_runner.grok_verification_failed"],
          }],
          status: "failed",
          summary: "The claimed task verifier failed.",
        }),
      },
      capacity: {
        accounts: async () => [{
          accountRef: "grok-healthy",
          advertisedCapacity: 1,
          marginalCostClass: "not_measured",
          workerKind: "grok",
        }],
      },
      clock: { now: () => fixedNow },
    })

    expect(store.listActiveDispatchBreakers(fixedNow)).toEqual([])
  })

  test("keeps explicit concrete runs constrained to their requested harness", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = createRun(store, {
      runRef: "fleet_run.fc2.explicit_codex",
      workUnits: 1,
      targetConcurrency: 1,
      workerKind: "codex",
    })
    const observed: FleetRunSupervisorObservedEvent[] = []
    let dispatches = 0
    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.fc2.explicit",
      runRef: run.runRef,
      planner: planner(store, 1),
      runner: {
        dispatch: async () => {
          dispatches += 1
          throw new Error("explicit codex run must not reach a Claude adapter")
        },
      },
      capacity: {
        accounts: async () => [
          {
            accountRef: "codex-exhausted-malformed-slots",
            advertisedCapacity: 9,
            marginalCostClass: "subscription",
            unavailabilityReason: "account_exhausted",
            workerKind: "codex",
          },
          {
            accountRef: "claude-only",
            advertisedCapacity: 1,
            marginalCostClass: "subscription",
            workerKind: "claude",
          },
        ],
      },
      clock: { now: () => fixedNow },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    expect(result.dispatched).toBe(0)
    expect(dispatches).toBe(0)
    expect(observed).toContainEqual(expect.objectContaining({
      kind: "skip",
      accountRef: "claude-only",
      requestedWorkerKind: "claude",
    }))
  })

  test("reopens the Pylon-home SQLite store, reconciles in-flight work, and refills without a duplicate claim", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc2-supervisor-"))
    const dbPath = join(root, "orchestration.sqlite")
    try {
      const firstDb = new Database(dbPath)
      const firstStore = createPylonOrchestrationStore(firstDb)
      const run = createRun(firstStore, {
        runRef: "fleet_run.fc2.restart_refill",
        workUnits: 4,
      })
      const firstKinds: string[] = []
      await tickFleetRunSupervisor({
        store: firstStore,
        pylonRef: "pylon.owner.fc2.restart",
        runRef: run.runRef,
        planner: planner(firstStore, 4),
        runner: {
          dispatch: async (input) => {
            firstKinds.push(input.workerKind)
            return {
              assignmentRef: `assignment.before-restart.${input.claim.claimRef}`,
              lifecycle: [lifecycleEvent("assignment_run.accepted", "accepted")],
              status: "accepted",
            }
          },
        },
        capacity: capacity(),
        clock: { now: () => fixedNow },
      })
      expect(new Set(firstKinds)).toEqual(new Set(["codex", "claude", "grok"]))
      expect(firstStore.listTasks("dispatched")).toHaveLength(3)
      firstDb.close()

      const secondDb = new Database(dbPath)
      const secondStore = createPylonOrchestrationStore(secondDb)
      const resumedAssignments: FleetRunSupervisorActiveAssignment[][] = []
      const refilled: FleetRunSupervisorDispatchInput[] = []
      const runner: FleetRunSupervisorRunner = {
        reconcile: async ({ activeAssignments }) => {
          resumedAssignments.push([...activeAssignments])
          return activeAssignments.map((assignment) => ({
            assignmentRef: assignment.claim.assignmentRef,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed" as const,
            summary: "reconciled after standing Pylon restart",
            taskId: assignment.taskId,
          }))
        },
        dispatch: async (input) => {
          refilled.push(input)
          return {
            assignmentRef: `assignment.refill.${input.claim.claimRef}`,
            lifecycle: [lifecycleEvent("assignment_run.completed", "closed")],
            status: "completed",
          }
        },
      }
      let nowMs = fixedNow.getTime() + 1_000
      const options = {
        store: secondStore,
        pylonRef: "pylon.owner.fc2.restart",
        runRef: run.runRef,
        planner: planner(secondStore, 4),
        runner,
        capacity: capacity(),
        clock: { now: () => new Date(nowMs) },
      }

      const refillTick = await tickFleetRunSupervisor(options)
      nowMs += 1_000
      const closeTick = await tickFleetRunSupervisor(options)

      expect(resumedAssignments[0]).toHaveLength(3)
      expect(refillTick.dispatched).toBe(1)
      expect(refilled).toHaveLength(1)
      expect(closeTick.run.state).toBe("completed")
      const claims = secondStore.listWorkClaims({ runRef: run.runRef })
      expect(claims).toHaveLength(4)
      expect(new Set(claims.map((claim) => claim.workUnitRef)).size).toBe(4)
      expect(new Set(claims.map((claim) => claim.claimRef)).size).toBe(4)
      expect(secondStore.listTasks("completed")).toHaveLength(4)
      expect(secondStore.listWorkClaims({ runRef: run.runRef, state: "released" })).toHaveLength(0)
      secondDb.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
