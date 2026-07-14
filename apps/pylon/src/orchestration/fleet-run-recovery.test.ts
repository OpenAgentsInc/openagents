import { afterEach, describe, expect, test } from "vite-plus/test"
import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { assertPublicProjectionSafe } from "../state.js"
import { createPylonAssignmentFleetRunOwnerLocalLivenessProbe } from "./fleet-run-assignment-liveness.js"
import {
  FLEET_RUN_INTERRUPTED_CLOSEOUT_SCHEMA,
  FLEET_RUN_INTERRUPTED_RECOVERY_SCHEMA,
  FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER,
  recoverInterruptedFleetRunAssignments,
} from "./fleet-run-recovery.js"
import { fleetRunTaskIdForClaim } from "./fleet-run-refs.js"
import { createPylonOrchestrationStore, type PylonOrchestrationStore } from "./store.js"

const now = new Date("2026-07-09T14:00:00.000Z")
const temporaryDirectories: string[] = []

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { force: true, recursive: true })))
})

const seedActiveAssignment = (
  store: PylonOrchestrationStore,
  suffix: string,
  input: { assignmentRef?: string | null; privateWorktreePath?: string } = {},
) => {
  const runRef = `fleet_run.recovery.${suffix}`
  const claimRef = `claim.recovery.${suffix}`
  const contextRef = `context.recovery.${suffix}`
  const workUnitRef = `issue.recovery.${suffix}`
  store.createFleetRun({
    runRef,
    objective: `Recover fixture ${suffix}`,
    workSource: "fixture",
    targetConcurrency: 1,
    workerKind: "codex",
    state: "running",
    now,
  })
  const claim = store.tryClaimWorkUnit({
    claimRef,
    workUnitRef,
    runRef,
    assignmentRef: input.assignmentRef ?? `assignment.public.recovery.${suffix}`,
    workerAccountRef: `codex-${suffix}`,
    ttl: 60_000,
    now,
  })
  if (claim === null) throw new Error("fixture claim was not created")
  const taskRef = fleetRunTaskIdForClaim(runRef, claimRef)
  store.createTask({
    id: taskRef,
    spec: {
      title: `Recovery fixture ${suffix}`,
      prompt: `Private fixture prompt ${suffix}`,
      fleetRunRef: runRef,
      issueRef: workUnitRef,
      runnerKind: "codex",
    },
    status: "ready",
    now,
  })
  store.createDispatchContext({
    id: contextRef,
    assigneeHandle: `codex-${suffix}`,
    runnerKind: "codex",
    worktreePath: input.privateWorktreePath,
    lastHeartbeatAt: now,
    now,
  })
  store.markDispatched(taskRef, contextRef, now)
  store.updateWorkClaimState(claimRef, "in_progress", now)
  return { claimRef, contextRef, runRef, taskRef, workUnitRef }
}

const livenessProbeInput = (assignmentRef: string | null) => ({
  runRef: "fleet_run.recovery.adapter",
  taskRef: "task.recovery.adapter",
  claimRef: "claim.recovery.adapter",
  contextRef: "context.recovery.adapter",
  assignmentRef,
  runnerKind: "codex" as const,
})

const writeAssignmentEvidence = async (
  path: string,
  leases: Record<string, unknown>,
): Promise<void> => {
  await writeFile(path, `${JSON.stringify({
    schema: "openagents.pylon.assignment_state.v0.3",
    leases,
  }, null, 2)}\n`)
}

describe("FleetRun interrupted owner-local recovery", () => {
  test("assignment-state adapter resolves live, dead, and absent evidence without exposing process metadata", async () => {
    const directory = await mkdtemp(join(tmpdir(), "pylon-fleet-liveness-"))
    temporaryDirectories.push(directory)
    const assignmentStatePath = join(directory, "assignment-state.json")
    const observedProcessIds: number[] = []
    const probe = createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
      assignmentStatePath,
      now: () => now,
      processIsAlive: (processId) => {
        observedProcessIds.push(processId)
        return processId === 4101
      },
    })

    expect(await probe(livenessProbeInput("assignment.public.live"))).toBe("unknown")
    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.live": {
        assignmentRef: "assignment.public.live",
        status: "running",
        ownerHeartbeatAt: now.toISOString(),
        ownerProcessId: 4101,
      },
      "lease.public.dead": {
        assignmentRef: "assignment.public.dead",
        status: "accepted",
        ownerHeartbeatAt: now.toISOString(),
        ownerProcessId: 4102,
      },
    })

    expect(await probe(livenessProbeInput("assignment.public.live"))).toBe("live")
    expect(await probe(livenessProbeInput("assignment.public.dead"))).toBe("dead")
    expect(await probe(livenessProbeInput(null))).toBe("unknown")
    expect(observedProcessIds).toEqual([4101, 4102])
    expect(JSON.stringify(await probe(livenessProbeInput("assignment.public.live"))))
      .not.toMatch(/4101|process|assignment-state/i)
  })

  test("assignment-state adapter fails unknown on mismatched, ambiguous, or corrupt ownership evidence", async () => {
    const directory = await mkdtemp(join(tmpdir(), "pylon-fleet-liveness-corrupt-"))
    temporaryDirectories.push(directory)
    const assignmentStatePath = join(directory, "assignment-state.json")
    let processProbeCalls = 0
    const probe = createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
      assignmentStatePath,
      now: () => now,
      processIsAlive: () => {
        processProbeCalls += 1
        return true
      },
    })

    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.other": {
        assignmentRef: "assignment.public.other",
        status: "running",
        ownerProcessId: 4201,
      },
    })
    expect(await probe(livenessProbeInput("assignment.public.expected"))).toBe("unknown")

    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.first": {
        assignmentRef: "assignment.public.expected",
        status: "running",
        ownerProcessId: 4201,
      },
      "lease.public.second": {
        assignmentRef: "assignment.public.expected",
        status: "running",
        ownerProcessId: 4202,
      },
    })
    expect(await probe(livenessProbeInput("assignment.public.expected"))).toBe("unknown")

    await writeFile(assignmentStatePath, "{ truncated private assignment evidence")
    expect(await probe(livenessProbeInput("assignment.public.expected"))).toBe("unknown")
    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.corrupt": {
        assignmentRef: "assignment.public.expected",
        status: "running",
        ownerProcessId: "not-a-process-id",
      },
    })
    expect(await probe(livenessProbeInput("assignment.public.expected"))).toBe("unknown")
    expect(processProbeCalls).toBe(0)

    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.throwing": {
        assignmentRef: "assignment.public.expected",
        status: "running",
        ownerHeartbeatAt: now.toISOString(),
        ownerProcessId: 4203,
      },
    })
    const throwingProbe = createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
      assignmentStatePath,
      now: () => now,
      processIsAlive: () => {
        throw new Error("raw process error /Users/owner/private/command")
      },
    })
    expect(await throwingProbe(livenessProbeInput("assignment.public.expected"))).toBe("unknown")
  })

  test("assignment-state adapter keeps the exact heartbeat boundary live and marks a live PID stale after it", async () => {
    const directory = await mkdtemp(join(tmpdir(), "pylon-fleet-liveness-boundary-"))
    temporaryDirectories.push(directory)
    const assignmentStatePath = join(directory, "assignment-state.json")
    const observedAt = new Date("2026-07-09T14:00:00.000Z")
    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.boundary": {
        assignmentRef: "assignment.public.boundary",
        status: "running",
        ownerHeartbeatAt: "2026-07-09T13:59:59.000Z",
        ownerProcessId: 4251,
      },
      "lease.public.stale": {
        assignmentRef: "assignment.public.stale",
        status: "running",
        ownerHeartbeatAt: "2026-07-09T13:59:58.999Z",
        ownerProcessId: 4252,
      },
    })
    const probe = createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
      assignmentStatePath,
      heartbeatStaleAfterMs: 1_000,
      now: () => observedAt,
      processIsAlive: () => true,
    })

    expect(await probe(livenessProbeInput("assignment.public.boundary"))).toBe("live")
    expect(await probe(livenessProbeInput("assignment.public.stale"))).toBe("dead")
  })

  test("assignment-state adapter rejects PID-reuse and missing, invalid, or future heartbeat evidence", async () => {
    const directory = await mkdtemp(join(tmpdir(), "pylon-fleet-liveness-heartbeat-"))
    temporaryDirectories.push(directory)
    const assignmentStatePath = join(directory, "assignment-state.json")
    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.reused_pid": {
        assignmentRef: "assignment.public.reused_pid",
        status: "running",
        ownerHeartbeatAt: "2026-07-09T13:44:00.000Z",
        ownerProcessId: 4261,
      },
      "lease.public.missing_heartbeat": {
        assignmentRef: "assignment.public.missing_heartbeat",
        status: "running",
        ownerProcessId: 4262,
      },
      "lease.public.invalid_heartbeat": {
        assignmentRef: "assignment.public.invalid_heartbeat",
        status: "running",
        ownerHeartbeatAt: "not-an-iso-heartbeat",
        ownerProcessId: 4263,
      },
      "lease.public.future_heartbeat": {
        assignmentRef: "assignment.public.future_heartbeat",
        status: "running",
        ownerHeartbeatAt: "2026-07-09T14:00:00.001Z",
        ownerProcessId: 4264,
      },
    })
    const probe = createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
      assignmentStatePath,
      // An unbounded caller value is capped at fifteen minutes, so this
      // sixteen-minute-old live PID cannot keep a restarted assignment alive.
      heartbeatStaleAfterMs: Number.MAX_SAFE_INTEGER,
      now: () => now,
      processIsAlive: () => true,
    })

    expect(await probe(livenessProbeInput("assignment.public.reused_pid"))).toBe("dead")
    expect(await probe(livenessProbeInput("assignment.public.missing_heartbeat"))).toBe("unknown")
    expect(await probe(livenessProbeInput("assignment.public.invalid_heartbeat"))).toBe("unknown")
    expect(await probe(livenessProbeInput("assignment.public.future_heartbeat"))).toBe("unknown")
  })

  test("assignment-state adapter treats a terminal local assignment as positively dead", async () => {
    const directory = await mkdtemp(join(tmpdir(), "pylon-fleet-liveness-terminal-"))
    temporaryDirectories.push(directory)
    const assignmentStatePath = join(directory, "assignment-state.json")
    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.closed": {
        assignmentRef: "assignment.public.closed",
        status: "closed",
        ownerProcessId: 4301,
      },
    })
    const probe = createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
      assignmentStatePath,
      processIsAlive: () => {
        throw new Error("terminal assignments must not probe private process state")
      },
    })

    expect(await probe(livenessProbeInput("assignment.public.closed"))).toBe("dead")
  })

  test("leaves a live owner-local assignment completely untouched", async () => {
    const store = createPylonOrchestrationStore(new NodeTestDatabase(":memory:"))
    const fixture = seedActiveAssignment(store, "live")

    const receipt = await recoverInterruptedFleetRunAssignments({
      store,
      probe: () => "live",
      now,
    })

    expect(receipt).toEqual({
      schema: FLEET_RUN_INTERRUPTED_RECOVERY_SCHEMA,
      observedAt: now.toISOString(),
      inspectedAssignments: 1,
      liveAssignments: 1,
      recoveredAssignments: 0,
      closeouts: [],
      contentRedacted: true,
    })
    expect(store.getTask(fixture.taskRef)?.status).toBe("dispatched")
    expect(store.getDispatchContext(fixture.contextRef)).toMatchObject({
      status: "dispatched",
      currentTaskId: fixture.taskRef,
    })
    expect(store.getWorkClaim(fixture.claimRef)?.state).toBe("in_progress")
    expect(store.listMessages()).toEqual([])
  })

  test("recovers a dead assignment once, persists restart idempotency, and permits one replacement claim", async () => {
    const directory = await mkdtemp(join(tmpdir(), "pylon-fleet-recovery-"))
    temporaryDirectories.push(directory)
    const databasePath = join(directory, "orchestration.sqlite")
    const privateWorktreePath = "/Users/owner/private/worktree-with-secret"
    const database = new NodeTestDatabase(databasePath)
    const store = createPylonOrchestrationStore(database)
    const fixture = seedActiveAssignment(store, "dead", { privateWorktreePath })

    const first = await recoverInterruptedFleetRunAssignments({
      store,
      probe: () => "dead",
      now,
    })
    const encoded = JSON.stringify(first)

    expect(first).toMatchObject({
      schema: FLEET_RUN_INTERRUPTED_RECOVERY_SCHEMA,
      inspectedAssignments: 1,
      liveAssignments: 0,
      recoveredAssignments: 1,
      contentRedacted: true,
      closeouts: [{
        schema: FLEET_RUN_INTERRUPTED_CLOSEOUT_SCHEMA,
        runRef: fixture.runRef,
        taskRef: fixture.taskRef,
        claimRef: fixture.claimRef,
        status: "stale",
        liveness: "dead",
        taskState: "failed",
        claimState: "released",
        blockerRefs: [FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER],
        redacted: true,
      }],
    })
    expect(encoded).not.toContain(privateWorktreePath)
    expect(encoded).not.toContain("Private fixture prompt")
    expect(encoded).not.toMatch(/processId|pid|command|credential|rawOutput/i)
    expect(() => assertPublicProjectionSafe(first)).not.toThrow()
    expect(store.getTask(fixture.taskRef)?.status).toBe("failed")
    expect(store.getDispatchContext(fixture.contextRef)).toMatchObject({
      status: "idle",
      currentTaskId: null,
    })
    expect(store.getWorkClaim(fixture.claimRef)?.state).toBe("released")
    expect(store.getFleetRun(fixture.runRef)).toMatchObject({
      state: "running",
      stateSource: "reconcile",
      counters: { activeAssignments: 0, failedAssignments: 1, workUnitsTotal: 2 },
    })
    expect(store.listMessages()).toHaveLength(1)
    expect(store.listMessages()[0]?.body).toBe(
      `fleet_run_local_assignment_stale ${FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER}`,
    )

    const replacement = store.tryClaimWorkUnit({
      claimRef: "claim.recovery.dead.replacement",
      workUnitRef: fixture.workUnitRef,
      runRef: fixture.runRef,
      workerAccountRef: "codex-replacement",
      ttl: 60_000,
      now: new Date("2026-07-09T14:00:01.000Z"),
    })
    expect(replacement?.state).toBe("claimed")
    expect(store.tryClaimWorkUnit({
      claimRef: "claim.recovery.dead.double",
      workUnitRef: fixture.workUnitRef,
      runRef: fixture.runRef,
      workerAccountRef: "codex-double",
      ttl: 60_000,
      now: new Date("2026-07-09T14:00:01.000Z"),
    })).toBeNull()

    database.close()
    const reopenedDatabase = new NodeTestDatabase(databasePath)
    const reopenedStore = createPylonOrchestrationStore(reopenedDatabase)
    let probeCalls = 0
    const afterRestart = await recoverInterruptedFleetRunAssignments({
      store: reopenedStore,
      probe: () => {
        probeCalls += 1
        return "dead"
      },
      now: new Date("2026-07-09T14:01:00.000Z"),
    })

    expect(afterRestart).toMatchObject({
      inspectedAssignments: 0,
      liveAssignments: 0,
      recoveredAssignments: 0,
      closeouts: [],
    })
    expect(probeCalls).toBe(0)
    expect(reopenedStore.listMessages()).toHaveLength(1)
    expect(reopenedStore.getWorkClaim(fixture.claimRef)?.state).toBe("released")
    reopenedDatabase.close()
  })

  test("treats a failed liveness probe as unknown and emits only a public-safe stale closeout", async () => {
    const store = createPylonOrchestrationStore(new NodeTestDatabase(":memory:"))
    const fixture = seedActiveAssignment(store, "unknown", {
      privateWorktreePath: "/private/never-project-this",
    })

    const receipt = await recoverInterruptedFleetRunAssignments({
      store,
      probe: () => {
        throw new Error("raw liveness probe output /private/never-project-this")
      },
      now,
    })

    expect(receipt.closeouts).toHaveLength(1)
    expect(receipt.closeouts[0]).toMatchObject({
      taskRef: fixture.taskRef,
      liveness: "unknown",
      blockerRefs: [FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER],
      redacted: true,
    })
    expect(JSON.stringify(receipt)).not.toContain("never-project-this")
    expect(store.getWorkClaim(fixture.claimRef)?.state).toBe("released")
  })

  test("assignment-state adapter closes a stale live PID with a public-safe recovery receipt", async () => {
    const directory = await mkdtemp(join(tmpdir(), "pylon-fleet-liveness-recovery-"))
    temporaryDirectories.push(directory)
    const assignmentStatePath = join(directory, "private-assignment-state.json")
    const store = createPylonOrchestrationStore(new NodeTestDatabase(":memory:"))
    const fixture = seedActiveAssignment(store, "adapter_dead", {
      assignmentRef: "assignment.public.recovery.adapter_dead",
      privateWorktreePath: "/Users/owner/private/adapter-dead-worktree",
    })
    await writeAssignmentEvidence(assignmentStatePath, {
      "lease.public.recovery.adapter_dead": {
        assignmentRef: "assignment.public.recovery.adapter_dead",
        status: "running",
        ownerHeartbeatAt: "2026-07-09T13:58:29.999Z",
        ownerProcessId: 4401,
      },
    })

    const receipt = await recoverInterruptedFleetRunAssignments({
      store,
      probe: createPylonAssignmentFleetRunOwnerLocalLivenessProbe({
        assignmentStatePath,
        now: () => now,
        processIsAlive: () => true,
      }),
      now,
    })
    const encoded = JSON.stringify(receipt)

    expect(receipt).toMatchObject({
      inspectedAssignments: 1,
      liveAssignments: 0,
      recoveredAssignments: 1,
      closeouts: [{
        runRef: fixture.runRef,
        assignmentRef: "assignment.public.recovery.adapter_dead",
        liveness: "dead",
        redacted: true,
      }],
      contentRedacted: true,
    })
    expect(() => assertPublicProjectionSafe(receipt)).not.toThrow()
    expect(encoded).not.toContain(assignmentStatePath)
    expect(encoded).not.toContain("4401")
    expect(encoded).not.toContain("adapter-dead-worktree")
    expect(encoded).not.toMatch(/ownerProcessId|pid|command|raw error/i)
  })
})
