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
  readonly accountRef?: string
  readonly advertisedCapacity?: number
  readonly env?: Record<string, string>
  readonly runner?: FleetRunSupervisorRunner
} = {}) => {
  fixtureOrdinal += 1
  const store = createPylonOrchestrationStore(new Database(":memory:"))
  const pylonRef = `pylon.test.${fixtureOrdinal}`
  const adapter = createKhalaCodeDesktopFleetRunSupervisorRpcAdapter({
    capacity: {
      accounts: async () => [{ accountRef: input.accountRef ?? "codex", advertisedCapacity: input.advertisedCapacity ?? 0 }],
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

  test("starts Claude worker-kind runs without storing or dispatching them as Codex", async () => {
    const dispatched: Array<{ readonly accountRef: string; readonly workerKind: string }> = []
    const { adapter, store } = adapterFixture({
      accountRef: "claude-a",
      advertisedCapacity: 1,
      runner: {
        dispatch: async input => {
          dispatched.push({
            accountRef: input.accountRef,
            workerKind: input.run.workerKind,
          })
          return {
            assignmentRef: "assignment.claude",
            lifecycle: [],
            status: "accepted",
            summary: null,
          }
        },
      },
    })

    const result = await adapter.start({
      objective: "Run Claude fixture target.",
      runRef: "fleet_run.adapter.claude",
      targetConcurrency: 1,
      tickImmediately: true,
      workerKind: "claude",
      workSource: { kind: "fixture", count: 1 },
    })

    expect(result.run.workerKind).toBe("claude")
    expect(store.getFleetRun("fleet_run.adapter.claude")?.workerKind).toBe("claude")
    expect(dispatched).toEqual([{ accountRef: "claude-a", workerKind: "claude" }])
  })

  test("starts plan_dag runs as Codex dispatches with node objectives", async () => {
    const dispatched: Array<{ readonly objective: string; readonly workUnitRef: string; readonly verify: string | undefined }> = []
    const { adapter, store } = adapterFixture({
      advertisedCapacity: 2,
      runner: {
        dispatch: async input => {
          dispatched.push({
            objective: input.workUnit.body ?? input.run.objective,
            workUnitRef: input.workUnit.workUnitRef,
            verify: input.workUnit.verify,
          })
          return {
            assignmentRef: "assignment.plan.root",
            lifecycle: [],
            status: "accepted",
            summary: null,
          }
        },
      },
    })

    const result = await adapter.start({
      objective: "Execute a Claude plan-mode DAG.",
      runRef: "fleet_run.adapter.plan_dag",
      targetConcurrency: 2,
      tickImmediately: true,
      workSource: {
        kind: "plan_dag",
        planRef: "plan.t9_4.adapter",
        repo: "OpenAgentsInc/openagents",
        baseCommit: "0123456789abcdef0123456789abcdef01234567",
        verify: "bun test clients/khala-code-desktop/tests/fleet-run-supervisor-rpc-adapter.test.ts",
        nodes: [
          {
            ref: "root",
            title: "Root node",
            objective: "Run the root plan node.",
            issue: 7873,
          },
          {
            ref: "dependent",
            title: "Dependent node",
            objective: "Run the dependent plan node.",
            dependsOn: ["root"],
          },
        ],
      },
    })

    expect(result.run.workerKind).toBe("codex")
    expect(result.run.workSource).toMatchObject({
      kind: "plan_dag",
      planRef: "plan.t9_4.adapter",
      nodes: [
        { ref: "root", objective: "Run the root plan node." },
        { ref: "dependent", dependsOn: ["root"] },
      ],
    })
    expect(dispatched).toEqual([{
      objective: "Run the root plan node.",
      workUnitRef: "plan_dag:plan.t9_4.adapter:node:root",
      verify: "bun test clients/khala-code-desktop/tests/fleet-run-supervisor-rpc-adapter.test.ts",
    }])
    expect(store.listTasks("dispatched")[0]?.spec).toMatchObject({
      fleetRunRef: "fleet_run.adapter.plan_dag",
      issueRef: "#7873",
      prompt: "Run the root plan node.",
      runnerKind: "codex",
    })
  })

  test("rejects invalid plan_dag work sources before creating a run", async () => {
    const { adapter, store } = adapterFixture({ advertisedCapacity: 2 })

    await expect(adapter.start({
      objective: "Execute an invalid Claude plan-mode DAG.",
      runRef: "fleet_run.adapter.invalid_plan_dag",
      targetConcurrency: 2,
      tickImmediately: true,
      workSource: {
        kind: "plan_dag",
        planRef: "plan.t9_4.invalid",
        repo: "OpenAgentsInc/openagents",
        baseCommit: "0123456789abcdef0123456789abcdef01234567",
        verify: "bun test clients/khala-code-desktop/tests/fleet-run-supervisor-rpc-adapter.test.ts",
        nodes: [{
          ref: "dependent",
          title: "Dependent node",
          objective: "Run the dependent plan node.",
          dependsOn: ["missing"],
        }],
      },
    })).rejects.toThrow(/unknown node/)

    expect(store.getFleetRun("fleet_run.adapter.invalid_plan_dag")).toBeNull()
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
                observedAt: "2026-07-01T12:00:00.000Z",
                phase: "runtime_active",
                schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
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
