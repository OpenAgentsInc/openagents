import { afterEach, describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { assertPublicProjectionSafe } from "../state.js"
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

describe("FleetRun interrupted owner-local recovery", () => {
  test("leaves a live owner-local assignment completely untouched", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
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
    const database = new Database(databasePath)
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
    const reopenedDatabase = new Database(databasePath)
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
    const store = createPylonOrchestrationStore(new Database(":memory:"))
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
})
