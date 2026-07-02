import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdtemp, readFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

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

const adapterFixture = (input: {
  readonly advertisedCapacity?: number
  readonly env?: Record<string, string>
  readonly runner?: FleetRunSupervisorRunner
} = {}) => {
  fixtureOrdinal += 1
  const store = createPylonOrchestrationStore(new Database(":memory:"))
  const pylonRef = `pylon.test.${fixtureOrdinal}`
  const adapter = createKhalaCodeDesktopFleetRunSupervisorRpcAdapter({
    capacity: {
      accounts: async () => [{ accountRef: "codex", advertisedCapacity: input.advertisedCapacity ?? 0 }],
    },
    env: input.env ?? { PYLON_HOME: "/tmp/khala-code-fleet-run-rpc-adapter-test" },
    pylonRef,
    runner: input.runner ?? acceptingRunner,
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

  test("worker retry releases the active claim and re-enqueues the work unit", async () => {
    let sequence = 0
    const { adapter, store } = adapterFixture({
      advertisedCapacity: 1,
      runner: {
        dispatch: async () => {
          sequence += 1
          return {
            assignmentRef: `assignment.retry.${sequence}`,
            lifecycle: [],
            status: "accepted",
            summary: null,
          }
        },
      },
    })

    await adapter.start({
      objective: "Retry active work.",
      runRef: "fleet_run.adapter.retry",
      targetConcurrency: 1,
      tickImmediately: true,
      workSource: { kind: "fixture", count: 1 },
    })
    const first = store.listWorkClaims({ runRef: "fleet_run.adapter.retry" })[0]
    expect(first).toMatchObject({ assignmentRef: "assignment.retry.1", state: "in_progress" })

    expect(adapter.workerControl).toBeDefined()
    const result = await adapter.workerControl!({
      assignmentRef: "assignment.retry.1",
      issueRef: null,
      runRef: "fleet_run.adapter.retry",
      verb: "retry",
      workerRefHash: "ref.retry",
    })

    expect(result).toMatchObject({ accepted: true, verb: "retry" })
    const claims = store.listWorkClaims({ runRef: "fleet_run.adapter.retry" })
    expect(claims.find(claim => claim.assignmentRef === "assignment.retry.1")?.state).toBe("released")
    expect(claims.find(claim => claim.assignmentRef === "assignment.retry.2")?.state).toBe("in_progress")
  })

  test("worker interrupt stops the active supervisor and releases the active claim", async () => {
    const { adapter, store } = adapterFixture({ advertisedCapacity: 1 })
    await adapter.start({
      objective: "Interrupt active work.",
      runRef: "fleet_run.adapter.interrupt",
      targetConcurrency: 1,
      tickImmediately: true,
      workSource: { kind: "fixture", count: 1 },
    })

    expect(adapter.workerControl).toBeDefined()
    const result = await adapter.workerControl!({
      assignmentRef: "assignment.test",
      issueRef: null,
      runRef: "fleet_run.adapter.interrupt",
      verb: "interrupt",
      workerRefHash: "ref.interrupt",
    })

    expect(result).toMatchObject({ accepted: true, verb: "interrupt" })
    expect((await adapter.status({ runRef: "fleet_run.adapter.interrupt" })).supervisorActive).toBe(false)
    expect(store.listWorkClaims({ runRef: "fleet_run.adapter.interrupt" })[0]?.state).toBe("released")
  })

  test("worker flag appends a persisted inbox row", async () => {
    const home = await mkdtemp(join(tmpdir(), "khala-code-fleet-worker-flag-"))
    const { adapter } = adapterFixture({ env: { PYLON_HOME: home } })

    expect(adapter.workerControl).toBeDefined()
    const result = await adapter.workerControl!({
      assignmentRef: "assignment.flag",
      issueRef: "#7946",
      note: "Needs operator review.",
      runRef: "fleet_run.adapter.flag",
      verb: "flag",
      workerRefHash: "ref.flag",
    })

    expect(result.inboxItemRef).toStartWith("inbox.assignment.ref.flag.flag.")
    const ledger = await readFile(join(home, "fleet-worker-inbox.jsonl"), "utf8")
    expect(JSON.parse(ledger.trim())).toMatchObject({
      assignmentRef: "assignment.flag",
      note: "Needs operator review.",
      ref: result.inboxItemRef,
      verb: "flag",
    })
  })

  test("publishes supervisor lifecycle events as panel NDJSON", async () => {
    const lines: string[] = []
    const { adapter } = (() => {
      fixtureOrdinal += 1
      const store = createPylonOrchestrationStore(new Database(":memory:"))
      return {
        adapter: createKhalaCodeDesktopFleetRunSupervisorRpcAdapter({
          capacity: {
            accounts: async () => [{ accountRef: "codex", advertisedCapacity: 1 }],
          },
          env: { PYLON_HOME: "/tmp/khala-code-fleet-run-rpc-adapter-test" },
          onLifecycleNdjson: line => {
            lines.push(line)
          },
          pylonRef: `pylon.test.${fixtureOrdinal}`,
          runner: {
            dispatch: async () => ({
              assignmentRef: "assignment.lifecycle",
              lifecycle: [{
                assignmentRef: "assignment.lifecycle",
                event: "assignment_run.runtime_progress",
                phase: "runtime_active",
                status: "running",
              }],
              status: "accepted",
              summary: null,
            }),
          },
          store,
          tickIntervalMs: 60_000,
        }),
      }
    })()

    await adapter.start({
      objective: "Stream lifecycle.",
      runRef: "fleet_run.adapter.lifecycle",
      targetConcurrency: 1,
      tickImmediately: true,
      workSource: { kind: "fixture", count: 1 },
    })

    expect(lines).toHaveLength(1)
    expect(JSON.parse(lines[0]!)).toMatchObject({
      assignmentRef: "assignment.lifecycle",
      event: "assignment_run.runtime_progress",
      phase: "runtime_active",
      schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
      status: "running",
    })
  })
})
