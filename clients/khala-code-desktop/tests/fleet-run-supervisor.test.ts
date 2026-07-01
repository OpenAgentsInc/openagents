import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { Effect, Exit, Scope } from "effect"

import {
  createPylonOrchestrationStore,
  type FleetRun,
} from "../../../apps/pylon/src/orchestration/store.js"
import { fixtureCandidates, planFixtureWork, planWorkCandidates } from "../../../apps/pylon/src/orchestration/work-planner.js"
import {
  FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT,
  makeFleetRunSupervisor,
  startFleetRunSupervisor,
  tickFleetRunSupervisor,
  type FleetRunSupervisorAccount,
  type FleetRunSupervisorObservedEvent,
  type FleetRunSupervisorRunner,
} from "../src/bun/fleet-run-supervisor.js"

const fixedNow = new Date("2026-07-01T12:00:00.000Z")

function createStoreWithRun(input: {
  readonly runRef?: string
  readonly targetConcurrency: number
  readonly workUnits?: number
}) {
  const store = createPylonOrchestrationStore(new Database(":memory:"))
  const run = store.createFleetRun({
    runRef: input.runRef ?? "fleet_run.test",
    objective: "Run fixture fleet units.",
    workSource: "fixture",
    targetConcurrency: input.targetConcurrency,
    workerKind: "codex",
    state: "running",
    startedAt: fixedNow,
    now: fixedNow,
    counters: { workUnitsTotal: input.workUnits ?? input.targetConcurrency },
  })
  return { store, run }
}

const fixturePlanner = (count: number) => ({
  plan: async (input: { readonly run: FleetRun; readonly now: Date }) =>
    planFixtureWork({ kind: "fixture", count }, { now: input.now }),
})

const fixturePlannerWithClaims = (
  store: ReturnType<typeof createPylonOrchestrationStore>,
  count: number,
) => ({
  plan: async (input: { readonly now: Date }) =>
    planWorkCandidates("fixture", fixtureCandidates({ kind: "fixture", count }), {
      now: input.now,
      claimRegistry: store,
    }),
})

const capacity = (accounts: readonly FleetRunSupervisorAccount[]) => ({
  accounts: async () => accounts,
})

const acceptingRunner = (dispatched: string[] = []): FleetRunSupervisorRunner => ({
  dispatch: async input => {
    dispatched.push(input.workUnit.workUnitRef)
    return {
      assignmentRef: `assignment.${input.claim.claimRef}`,
      lifecycle: [{ event: "assignment.accepted", status: "accepted" }],
      status: "accepted",
    }
  },
})

describe("FleetRunSupervisor", () => {
  test("refills arbitrary target concurrency across ticks while keeping MAX_SPAWN_COUNT per tick", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 25, workUnits: 25 })
    const dispatched: string[] = []

    const options = {
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 25),
      runner: acceptingRunner(dispatched),
      capacity: capacity([
        { accountRef: "codex", advertisedCapacity: 10 },
        { accountRef: "codex-2", advertisedCapacity: 10 },
        { accountRef: "codex-3", advertisedCapacity: 10 },
      ]),
      clock: { now: () => fixedNow },
    }

    const first = await tickFleetRunSupervisor(options)
    const second = await tickFleetRunSupervisor(options)
    const third = await tickFleetRunSupervisor(options)

    expect(first.dispatched).toBe(FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT)
    expect(second.dispatched).toBe(FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT)
    expect(third.dispatched).toBe(5)
    expect(store.listTasks("dispatched")).toHaveLength(25)
    expect(new Set(dispatched).size).toBe(25)
    expect(store.getFleetRun(run.runRef)?.counters.activeAssignments).toBe(25)
  })

  test("respects advertised account capacity and cooldowns", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 6, workUnits: 6 })

    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 6),
      runner: acceptingRunner(),
      capacity: capacity([
        { accountRef: "codex", advertisedCapacity: 3, cooldownUntil: "2026-07-01T12:10:00.000Z" },
        { accountRef: "codex-2", advertisedCapacity: 2 },
      ]),
      clock: { now: () => fixedNow },
    })

    expect(result.dispatched).toBe(2)
    expect(store.listWorkClaims({ runRef: run.runRef }).map(claim => claim.workerAccountRef)).toEqual([
      "codex-2",
      "codex-2",
    ])
  })

  test("streams terminal lifecycle into counters and drains cleanly when backlog is empty", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 3, workUnits: 3 })
    const observed: FleetRunSupervisorObservedEvent[] = []

    await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 3),
      runner: {
        dispatch: async input => ({
          assignmentRef: `assignment.${input.claim.claimRef}`,
          lifecycle: [
            { event: "assignment.accepted", status: "accepted" },
            { event: "assignment.closeout", status: "completed" },
          ],
          status: "completed",
          summary: "fixture completed",
        }),
      },
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 3 }]),
      clock: { now: () => fixedNow },
      onLifecycle: event => {
        observed.push(event)
      },
    })
    const drained = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 3),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 3 }]),
      clock: { now: () => fixedNow },
      onLifecycle: event => {
        observed.push(event)
      },
    })

    expect(drained.run.state).toBe("completed")
    expect(store.getFleetRun(run.runRef)?.counters).toMatchObject({
      activeAssignments: 0,
      completedAssignments: 3,
      failedAssignments: 0,
      blockedAssignments: 0,
    })
    expect(observed.filter(event => event.kind === "lifecycle")).toHaveLength(6)
    expect(observed).toContainEqual({ kind: "completed", runRef: run.runRef, reason: "drained" })
  })

  test("keeps refilling target 2 over 4 fixture units until all work drains", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 2, workUnits: 4 })
    const dispatched: string[] = []
    const options = {
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 4),
      runner: {
        dispatch: async input => {
          dispatched.push(input.workUnit.workUnitRef)
          return {
            assignmentRef: `assignment.${input.claim.claimRef}`,
            lifecycle: [{ event: "assignment.closeout", status: "completed" }],
            status: "completed" as const,
          }
        },
      },
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 2 }]),
      clock: { now: () => fixedNow },
    }

    const first = await tickFleetRunSupervisor(options)
    const second = await tickFleetRunSupervisor(options)
    const third = await tickFleetRunSupervisor(options)

    expect(first.dispatched).toBe(2)
    expect(second.dispatched).toBe(2)
    expect(third.dispatched).toBe(0)
    expect(new Set(dispatched).size).toBe(4)
    expect(store.listTasks("completed")).toHaveLength(4)
    expect(store.getFleetRun(run.runRef)?.state).toBe("completed")
    expect(store.getFleetRun(run.runRef)?.counters.workUnitsTotal).toBe(4)
  })

  test("closeout claims do not consume account capacity on the next tick", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 1, workUnits: 2 })
    const dispatched: string[] = []
    const options = {
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 2),
      runner: {
        dispatch: async input => {
          dispatched.push(input.workUnit.workUnitRef)
          return {
            assignmentRef: `assignment.${input.claim.claimRef}`,
            lifecycle: [{ event: "assignment.closeout", status: "completed" }],
            status: "completed" as const,
          }
        },
      },
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    }

    const first = await tickFleetRunSupervisor(options)
    const second = await tickFleetRunSupervisor(options)

    expect(first.dispatched).toBe(1)
    expect(second.freeSlots).toBe(1)
    expect(second.dispatched).toBe(1)
    expect(dispatched).toHaveLength(2)
    expect(store.listWorkClaims({ runRef: run.runRef, state: "closeout" })).toHaveLength(2)
  })

  test("throwing dispatch releases the claim, fails the unit, and later ticks continue", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 1, workUnits: 2 })
    let calls = 0
    const options = {
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 2),
      runner: {
        dispatch: async input => {
          calls += 1
          if (calls === 1) throw new Error("dispatch exploded")
          return {
            assignmentRef: `assignment.${input.claim.claimRef}`,
            lifecycle: [{ event: "assignment.closeout", status: "completed" }],
            status: "completed" as const,
          }
        },
      },
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    }

    const first = await tickFleetRunSupervisor(options)
    const second = await tickFleetRunSupervisor(options)

    expect(first.dispatched).toBe(1)
    expect(second.dispatched).toBe(1)
    expect(calls).toBe(2)
    expect(store.listWorkClaims({ runRef: run.runRef }).map(claim => claim.state)).toEqual(["released", "closeout"])
    expect(store.listTasks("failed")).toHaveLength(1)
    expect(store.listTasks("completed")).toHaveLength(1)
  })

  test("refuses a second active supervisor for the same Pylon", async () => {
    const first = createStoreWithRun({ runRef: "fleet_run.one", targetConcurrency: 1 })
    const second = createStoreWithRun({ runRef: "fleet_run.two", targetConcurrency: 1 })

    const firstHandle = await Effect.runPromise(makeFleetRunSupervisor({
      store: first.store,
      pylonRef: "pylon.owner",
      runRef: first.run.runRef,
      planner: fixturePlanner(1),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    }))

    await expect(Effect.runPromise(makeFleetRunSupervisor({
      store: second.store,
      pylonRef: "pylon.owner",
      runRef: second.run.runRef,
      planner: fixturePlanner(1),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    }))).rejects.toThrow(/already active/)

    await Effect.runPromise(firstHandle.stop())
  })

  test("refuses a duplicate active supervisor for the same run", async () => {
    const first = createStoreWithRun({ runRef: "fleet_run.same", targetConcurrency: 1 })

    const firstHandle = await Effect.runPromise(makeFleetRunSupervisor({
      store: first.store,
      pylonRef: "pylon.owner.same",
      runRef: first.run.runRef,
      planner: fixturePlanner(1),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    }))

    await expect(Effect.runPromise(makeFleetRunSupervisor({
      store: first.store,
      pylonRef: "pylon.owner.same",
      runRef: first.run.runRef,
      planner: fixturePlanner(1),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    }))).rejects.toThrow(/already active/)

    await Effect.runPromise(firstHandle.stop())
  })

  test("start supervisor ticks until scope close and frees the registry", async () => {
    const { store, run } = createStoreWithRun({ runRef: "fleet_run.scoped", targetConcurrency: 1, workUnits: 1 })
    let tickCount = 0
    let scopeClosed = false
    const scope = Effect.runSync(Scope.make())

    const handle = await Effect.runPromise(Effect.provideService(
      startFleetRunSupervisor({
        store,
        pylonRef: "pylon.owner.scoped",
        runRef: run.runRef,
        planner: fixturePlannerWithClaims(store, 1),
        runner: acceptingRunner(),
        capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
        tickIntervalMs: 1,
        clock: {
          now: () => fixedNow,
          sleep: () => Promise.resolve(),
        },
        onLifecycle: event => {
          if (event.kind === "tick" && !scopeClosed) tickCount += 1
        },
      }),
      Scope.Scope,
      scope,
    ))

    await new Promise<void>((resolve, reject) => {
      const started = Date.now()
      const poll = () => {
        if (tickCount > 0) return resolve()
        if (Date.now() - started > 1000) return reject(new Error("supervisor did not tick"))
        setTimeout(poll, 1)
      }
      poll()
    })
    await Effect.runPromise(Scope.close(scope, Exit.void))
    scopeClosed = true
    const ticksAtClose = tickCount
    await new Promise(resolve => setTimeout(resolve, 5))

    expect(ticksAtClose).toBeGreaterThan(0)
    expect(tickCount).toBe(ticksAtClose)
    const replacementHandle = await Effect.runPromise(makeFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner.scoped",
      runRef: run.runRef,
      planner: fixturePlanner(1),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    }))
    expect(replacementHandle.pylonRef).toBe(handle.pylonRef)
    await Effect.runPromise(replacementHandle.stop())
  })
})
