import { setTimeout as sleep } from "node:timers/promises"
import { describe, expect, test } from "vite-plus/test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { assertPublicProjectionSafe } from "../src/state.js"
import { fleetRunTaskIdForClaim } from "../src/orchestration/fleet-run-refs.js"
import { openPylonFleetRunRuntime } from "../src/orchestration/fleet-run-runtime.js"
import { openPylonStandingFleetRunExecutor } from "../src/orchestration/fleet-run-standing-executor.js"
import type { FleetRunSupervisorDispatchInput } from "../src/orchestration/fleet-run-supervisor.js"
import { planFixtureWork } from "../src/orchestration/work-planner.js"

const fixedNow = new Date("2026-07-09T21:00:00.000Z")

const waitUntil = async (predicate: () => boolean): Promise<void> => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) return
    await sleep(2)
  }
  throw new Error("timed out waiting for standing FleetRun dispatch")
}

describe("Pylon standing FleetRun executor", () => {
  test("opens the durable runtime, recovers first, and refills through Pylon-only adapters", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-standing-fleet-run-"))
    const pylonHome = join(root, "pylon-home")
    const env = { PYLON_HOME: pylonHome } as NodeJS.ProcessEnv
    const runRef = "fleet_run.fc2.standing"
    const interruptedClaimRef = "claim.fc2.standing.interrupted"
    const workUnitRef = "fixture:standing.refill"
    const interruptedTaskRef = fleetRunTaskIdForClaim(runRef, interruptedClaimRef)
    const privateWorktreePath = "/Users/owner/private/standing-fleet-worktree"

    try {
      const seed = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      seed.store.createFleetRun({
        runRef,
        objective: "Refill one bounded standing FleetRun fixture.",
        workSource: "fixture",
        targetConcurrency: 1,
        workerKind: "codex",
        state: "running",
        now: fixedNow,
      })
      const interruptedClaim = seed.store.tryClaimWorkUnit({
        claimRef: interruptedClaimRef,
        workUnitRef,
        runRef,
        assignmentRef: "assignment.public.fc2.interrupted",
        workerAccountRef: "codex-owner-isolated",
        ttl: 60_000,
        now: fixedNow,
      })
      expect(interruptedClaim).not.toBeNull()
      seed.store.createTask({
        id: interruptedTaskRef,
        spec: {
          title: "Interrupted standing fixture",
          prompt: "Private owner-local prompt must never enter recovery output.",
          fleetRunRef: runRef,
          issueRef: workUnitRef,
          runnerKind: "codex",
        },
        status: "ready",
        now: fixedNow,
      })
      seed.store.createDispatchContext({
        id: "context.fc2.standing.interrupted",
        assigneeHandle: "codex-owner-isolated",
        runnerKind: "codex",
        worktreePath: privateWorktreePath,
        lastHeartbeatAt: fixedNow,
        now: fixedNow,
      })
      seed.store.markDispatched(
        interruptedTaskRef,
        "context.fc2.standing.interrupted",
        fixedNow,
      )
      seed.store.updateWorkClaimState(interruptedClaimRef, "in_progress", fixedNow)
      await seed.close()

      const dispatched: FleetRunSupervisorDispatchInput[] = []
      const livenessInputs: unknown[] = []
      const planner = {
        plan: ({ now }: { now: Date }) => Promise.resolve(planFixtureWork({
          kind: "fixture" as const,
          units: [{ ref: "standing.refill", title: "Standing refill" }],
        }, { now })),
      }
      const capacity = {
        accounts: () => Promise.resolve([{
          accountRef: "codex-owner-isolated",
          advertisedCapacity: 1,
          marginalCostClass: "subscription" as const,
          workerKind: "codex" as const,
        }]),
      }
      const runner = {
        dispatch: async (input: FleetRunSupervisorDispatchInput) => {
          dispatched.push(input)
          return {
            assignmentRef: "assignment.public.fc2.refill",
            lifecycle: [],
            status: "accepted" as const,
            summary: "Standing refill accepted.",
          }
        },
      }
      const clock = {
        now: () => fixedNow,
        sleep: () => new Promise<void>(() => {}),
      }
      const standing = await openPylonStandingFleetRunExecutor({
        env,
        now: () => fixedNow,
        pylonRef: "pylon.public.fc2.standing",
        runRef,
        livenessProbe: (input) => {
          livenessInputs.push(input)
          return "dead"
        },
        planner,
        capacity,
        runner,
        clock,
        startImmediately: false,
      })

      try {
        await waitUntil(() => dispatched.length === 1)
        await waitUntil(() =>
          standing.runtime.store.listWorkClaims({ runRef })
            .some((claim) => claim.assignmentRef === "assignment.public.fc2.refill")
        )

        expect(standing.recovery).toMatchObject({
          inspectedAssignments: 1,
          liveAssignments: 0,
          recoveredAssignments: 1,
          closeouts: [{
            claimRef: interruptedClaimRef,
            status: "stale",
            blockerRefs: ["blocker.assignment.local_run_interrupted"],
            redacted: true,
          }],
          contentRedacted: true,
        })
        expect(standing.snapshot).toMatchObject({
          active: true,
          pylonRef: "pylon.public.fc2.standing",
          run: { runRef, state: "running" },
        })
        expect(livenessInputs).toEqual([{
          runRef,
          taskRef: interruptedTaskRef,
          claimRef: interruptedClaimRef,
          contextRef: "context.fc2.standing.interrupted",
          assignmentRef: "assignment.public.fc2.interrupted",
          runnerKind: "codex",
        }])
        expect(dispatched).toHaveLength(1)
        expect(dispatched[0]).toMatchObject({
          accountRef: "codex-owner-isolated",
          run: { runRef },
          workUnit: { workUnitRef },
          workerKind: "codex",
        })

        const claims = standing.runtime.store.listWorkClaims({ runRef })
        expect(claims).toHaveLength(2)
        expect(claims.find((claim) => claim.claimRef === interruptedClaimRef)?.state).toBe("released")
        expect(claims.filter((claim) => claim.state === "in_progress")).toHaveLength(1)
        expect(standing.runtime.store.getTask(interruptedTaskRef)?.status).toBe("failed")

        const duplicateResume = await standing.runtime.manager.resume({
          capacity,
          clock,
          planner,
          pylonRef: "pylon.public.fc2.standing",
          runRef,
          runner,
          startImmediately: false,
        })
        expect(duplicateResume.active).toBe(true)
        expect(dispatched).toHaveLength(1)

        const publicRecovery = JSON.stringify(standing.recovery)
        expect(publicRecovery).not.toContain(pylonHome)
        expect(publicRecovery).not.toContain(privateWorktreePath)
        expect(publicRecovery).not.toContain("Private owner-local prompt")
        expect(() => assertPublicProjectionSafe(standing.recovery)).not.toThrow()
      } finally {
        await standing.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
