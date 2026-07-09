import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"

import {
  tickFleetRunSupervisor,
  type FleetRunSupervisorAccount,
  type FleetRunSupervisorActiveAssignment,
  type FleetRunSupervisorDispatchInput,
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
  input: { readonly runRef: string; readonly workUnits: number },
): FleetRun => store.createFleetRun({
  runRef: input.runRef,
  objective: "Run one pinned unit on every connected harness.",
  workSource: "fixture",
  targetConcurrency: 3,
  workerKind: "auto",
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
            assignmentRef: `assignment.after-restart.${assignment.claim.claimRef}`,
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
