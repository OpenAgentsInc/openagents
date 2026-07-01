import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"

import { createPylonOrchestrationStore } from "../../../apps/pylon/src/orchestration/store"
import { createKhalaCodeDesktopFleetRunSupervisorRpcAdapter } from "../src/bun/fleet-run-supervisor-rpc-adapter"
import type { FleetRunSupervisorRunner } from "../src/bun/fleet-run-supervisor"

const acceptingRunner: FleetRunSupervisorRunner = {
  dispatch: async () => ({
    assignmentRef: "assignment.test",
    lifecycle: [],
    status: "accepted",
    summary: null,
  }),
}

let fixtureOrdinal = 0

const adapterFixture = () => {
  fixtureOrdinal += 1
  const store = createPylonOrchestrationStore(new Database(":memory:"))
  const pylonRef = `pylon.test.${fixtureOrdinal}`
  const adapter = createKhalaCodeDesktopFleetRunSupervisorRpcAdapter({
    capacity: {
      accounts: async () => [{ accountRef: "codex", advertisedCapacity: 0 }],
    },
    env: { PYLON_HOME: "/tmp/khala-code-fleet-run-rpc-adapter-test" },
    pylonRef,
    runner: acceptingRunner,
    store,
    tickIntervalMs: 60_000,
  })
  return { adapter, pylonRef, store }
}

describe("Khala Code fleet run supervisor RPC adapter", () => {
  test("starts store-backed runs and retains normalized workSource projections", async () => {
    const { adapter, pylonRef, store } = adapterFixture()

    const result = await adapter.start({
      objective: "Burn down public issues.",
      runRef: "fleet_run.adapter.start",
      targetConcurrency: 2,
      workSource: {
        kind: "issue_list",
        repo: "OpenAgentsInc/openagents",
        issues: [{
          kind: "issue",
          number: 7932,
          state: "OPEN",
          title: "Finish fleet run RPC",
        }],
      },
    })

    expect(result.supervisorStarted).toBe(true)
    expect(store.getFleetRun("fleet_run.adapter.start")).toMatchObject({
      objective: "Burn down public issues.",
      state: "running",
      workSource: "issue_list",
    })
    expect(result.run).toMatchObject({
      objectiveProjected: false,
      pylonRef,
      workSource: {
        kind: "issue_list",
        repo: "OpenAgentsInc/openagents",
        issues: [{ number: 7932, state: "open" }],
      },
    })
    expect(result.run).not.toHaveProperty("objective")
  })

  test("accepts target_reached stop conditions in projections", async () => {
    const { adapter } = adapterFixture()

    const result = await adapter.start({
      objective: "Run fixture target.",
      runRef: "fleet_run.adapter.target",
      targetConcurrency: 1,
      refillPolicy: { stopCondition: "target_reached" },
      workSource: { kind: "fixture", count: 1 },
    })

    expect(result.run.refillPolicy.stopCondition).toBe("target_reached")
  })

  test("rejects invalid control transitions in the store-backed authority", async () => {
    const { adapter, store } = adapterFixture()
    store.createFleetRun({
      objective: "Already completed.",
      runRef: "fleet_run.adapter.completed",
      state: "completed",
      targetConcurrency: 1,
      workerKind: "codex",
      workSource: "fixture",
    })

    await expect(adapter.control({ runRef: "fleet_run.adapter.completed", verb: "pause" }))
      .rejects.toThrow("fleetRunControl cannot pause a completed fleet run")
  })

  test("rejects unknown control run refs", async () => {
    const { adapter } = adapterFixture()

    await expect(adapter.control({ runRef: "fleet_run.adapter.unknown", verb: "stop" }))
      .rejects.toThrow("unknown fleet run: fleet_run.adapter.unknown")
  })
})
