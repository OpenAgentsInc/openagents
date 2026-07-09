import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"

import { createPylonOrchestrationStore, type FleetRun } from "../../../apps/pylon/src/orchestration/store.js"
import { planFixtureWork } from "../../../apps/pylon/src/orchestration/work-planner.js"
import {
  resolveSupervisorWorkerKind,
  tickFleetRunSupervisor,
  type FleetRunSupervisorDispatchInput,
  type FleetRunSupervisorRunner,
} from "../src/bun/fleet-run-supervisor.js"
import { narrowToDelegateWorkerKind } from "@openagentsinc/khala-tools"

const fixedNow = new Date("2026-07-09T12:00:00.000Z")

describe("FleetRunSupervisor Grok dispatch (MH-4)", () => {
  test("resolveSupervisorWorkerKind marks grok available (concrete)", () => {
    expect(resolveSupervisorWorkerKind("grok")).toEqual({
      available: true,
      workerKind: "grok",
    })
    expect(narrowToDelegateWorkerKind("grok")).toBe("grok")
  })

  test("grok-only FleetRun dispatches with workerKind=grok and unique claims", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const run = store.createFleetRun({
      runRef: "fleet_run.mh4.grok",
      objective: "fixture objective",
      workSource: "fixture",
      targetConcurrency: 2,
      workerKind: "grok",
      state: "running",
      startedAt: fixedNow,
      now: fixedNow,
      counters: { workUnitsTotal: 4 },
    })

    const dispatched: FleetRunSupervisorDispatchInput[] = []
    const runner: FleetRunSupervisorRunner = {
      dispatch: async (input) => {
        dispatched.push(input)
        return {
          assignmentRef: `grok.${input.claim.claimRef}`,
          lifecycle: [],
          status: "accepted",
        }
      },
    }

    await tickFleetRunSupervisor({
      capacity: {
        accounts: async () => [
          {
            accountRef: "grok-local",
            advertisedCapacity: 2,
            workerKind: "grok",
          },
        ],
      },
      planner: {
        plan: async (input: { readonly run: FleetRun; readonly now: Date }) =>
          planFixtureWork({ kind: "fixture", count: 4 }, { now: input.now }),
      },
      pylonRef: "local",
      runRef: run.runRef,
      runner,
      store,
      clock: { now: () => fixedNow },
    })

    expect(dispatched.length).toBeGreaterThan(0)
    expect(dispatched.every((d) => d.workerKind === "grok")).toBe(true)
    const claimRefs = new Set(dispatched.map((d) => d.claim.claimRef))
    expect(claimRefs.size).toBe(dispatched.length)
  })
})
