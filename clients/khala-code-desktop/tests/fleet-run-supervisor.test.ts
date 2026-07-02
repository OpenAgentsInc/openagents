import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
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
  type FleetRunSupervisorActiveAssignment,
  type FleetRunSupervisorObservedEvent,
  type FleetRunSupervisorDispatchInput,
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
  dispatch: async (input: FleetRunSupervisorDispatchInput) => {
    dispatched.push(input.workUnit.workUnitRef)
    return {
      assignmentRef: `assignment.${input.claim.claimRef}`,
      lifecycle: [{ event: "assignment.accepted", status: "accepted" }],
      status: "accepted",
    }
  },
})

function drainingRunner(input: {
  readonly completeAfterPeak?: number
  readonly completePerTick: number
  readonly dispatched?: string[]
  readonly onActive?: (assignments: readonly FleetRunSupervisorActiveAssignment[]) => void
}): FleetRunSupervisorRunner {
  let peakActive = 0
  return {
    dispatch: async (assignment: FleetRunSupervisorDispatchInput) => {
      input.dispatched?.push(assignment.workUnit.workUnitRef)
      return {
        assignmentRef: `assignment.${assignment.claim.claimRef}`,
        lifecycle: [{ event: "assignment.accepted", status: "accepted" }],
        status: "accepted",
      }
    },
    reconcile: async ({ activeAssignments }) => {
      peakActive = Math.max(peakActive, activeAssignments.length)
      input.onActive?.(activeAssignments)
      if (peakActive < (input.completeAfterPeak ?? 1)) return []
      return activeAssignments.slice(0, input.completePerTick).map(assignment => ({
        assignmentRef: `assignment.${assignment.claim.claimRef}`,
        lifecycle: [{ event: "assignment.closeout", status: "completed" }],
        status: "completed" as const,
        summary: "fixture assignment completed",
        taskId: assignment.taskId,
      }))
    },
  }
}

describe("FleetRunSupervisor", () => {
  test("target-25 fixture acceptance reaches 25 simulated concurrent assignments and drains", async () => {
    const { store, run } = createStoreWithRun({ runRef: "fleet_run.acceptance.target_25", targetConcurrency: 25, workUnits: 25 })
    const dispatched: string[] = []
    const activeCounts: number[] = []
    let nowMs = fixedNow.getTime()
    const options = {
      store,
      pylonRef: "pylon.owner.acceptance",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 25),
      runner: drainingRunner({
        completeAfterPeak: 25,
        completePerTick: 6,
        dispatched,
        onActive: assignments => activeCounts.push(assignments.length),
      }),
      capacity: capacity([
        { accountRef: "codex", advertisedCapacity: 10 },
        { accountRef: "codex-2", advertisedCapacity: 10 },
        { accountRef: "codex-3", advertisedCapacity: 10 },
      ]),
      clock: { now: () => new Date(nowMs) },
    }

    for (let tick = 0; tick < 20; tick += 1) {
      await tickFleetRunSupervisor(options)
      nowMs += 1_000
      if (store.getFleetRun(run.runRef)?.state === "completed") break
    }

    expect(activeCounts).toContain(25)
    expect(new Set(dispatched).size).toBe(25)
    expect(store.getFleetRun(run.runRef)?.state).toBe("completed")
    expect(store.getFleetRun(run.runRef)?.counters).toMatchObject({
      activeAssignments: 0,
      completedAssignments: 25,
      failedAssignments: 0,
      blockedAssignments: 0,
      workUnitsTotal: 25,
    })
    expect(store.listWorkClaims({ runRef: run.runRef, state: "closeout" })).toHaveLength(25)
  })

  test("fixture run survives host restart over the same sqlite file and continues refilling", async () => {
    const root = await mkdtemp(join(tmpdir(), "khala-fleet-run-restart-"))
    try {
      const dbPath = join(root, "orchestration.sqlite")
      const firstDb = new Database(dbPath)
      const firstStore = createPylonOrchestrationStore(firstDb)
      const run = firstStore.createFleetRun({
        runRef: "fleet_run.acceptance.restart",
        objective: "Run restart fixture fleet units.",
        workSource: "fixture",
        targetConcurrency: 10,
        workerKind: "codex",
        state: "running",
        startedAt: fixedNow,
        now: fixedNow,
        counters: { workUnitsTotal: 15 },
      })
      const firstDispatched: string[] = []
      await tickFleetRunSupervisor({
        store: firstStore,
        pylonRef: "pylon.owner.restart",
        runRef: run.runRef,
        planner: fixturePlannerWithClaims(firstStore, 15),
        runner: acceptingRunner(firstDispatched),
        capacity: capacity([{ accountRef: "codex", advertisedCapacity: 10 }]),
        clock: { now: () => fixedNow },
      })
      expect(firstStore.listTasks("dispatched")).toHaveLength(10)
      firstDb.close()

      const secondDb = new Database(dbPath)
      const secondStore = createPylonOrchestrationStore(secondDb)
      const secondDispatched: string[] = []
      const activeCounts: number[] = []
      let nowMs = fixedNow.getTime() + 1_000
      const options = {
        store: secondStore,
        pylonRef: "pylon.owner.restart",
        runRef: run.runRef,
        planner: fixturePlannerWithClaims(secondStore, 15),
        runner: drainingRunner({
          completePerTick: 5,
          dispatched: secondDispatched,
          onActive: assignments => activeCounts.push(assignments.length),
        }),
        capacity: capacity([{ accountRef: "codex", advertisedCapacity: 10 }]),
        clock: { now: () => new Date(nowMs) },
      }

      for (let tick = 0; tick < 10; tick += 1) {
        await tickFleetRunSupervisor(options)
        nowMs += 1_000
        if (secondStore.getFleetRun(run.runRef)?.state === "completed") break
      }

      expect(activeCounts[0]).toBe(10)
      expect(new Set([...firstDispatched, ...secondDispatched]).size).toBe(15)
      expect(secondStore.getFleetRun(run.runRef)?.state).toBe("completed")
      expect(secondStore.getFleetRun(run.runRef)?.counters.completedAssignments).toBe(15)
      expect(secondStore.listWorkClaims({ runRef: run.runRef, state: "released" })).toHaveLength(0)
      secondDb.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

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

  test("never claims a paused ready account during a supervisor tick", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 2, workUnits: 2 })

    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 2),
      runner: acceptingRunner(),
      capacity: capacity([
        { accountRef: "codex-a", advertisedCapacity: 1, paused: true },
        { accountRef: "codex-b", advertisedCapacity: 1 },
      ]),
      clock: { now: () => fixedNow },
    })

    expect(result.dispatched).toBe(1)
    expect(store.listWorkClaims({ runRef: run.runRef }).map(claim => claim.workerAccountRef)).toEqual([
      "codex-b",
    ])
  })

  test("never auto-revives a paused run mid-backlog (#7975)", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 2, workUnits: 3 })
    store.controlFleetRun(run.runRef, "pause", fixedNow)

    const result = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 3),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex-a", advertisedCapacity: 2 }]),
      clock: { now: () => fixedNow },
    })

    expect(result.run.state).toBe("paused")
    expect(result.claimed).toBe(0)
    expect(result.dispatched).toBe(0)
    expect(store.listWorkClaims({ runRef: run.runRef })).toEqual([])
    expect(store.getFleetRun(run.runRef)?.state).toBe("paused")
  })

  test("never auto-revives a drained run after drain completes mid-backlog (#7975)", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 1, workUnits: 3 })
    store.createTask({
      id: "task.drain.inflight",
      spec: {
        title: "in-flight unit",
        prompt: run.objective,
        runnerKind: "codex",
        issueRef: "work_unit.fixture.drain",
        fleetRunRef: run.runRef,
      },
      now: fixedNow,
    })
    store.createDispatchContext({
      id: "ctx.codex.drain",
      assigneeHandle: "codex-a",
      runnerKind: "codex",
      lastHeartbeatAt: fixedNow,
      now: fixedNow,
    })
    store.markDispatched("task.drain.inflight", "ctx.codex.drain", fixedNow)

    // Operator drains while the unit is still in flight, then the unit
    // finishes. Reconciliation closes the drain — that close completes the
    // operator's decision, so later ticks must not reopen the run for the
    // remaining planner backlog.
    store.controlFleetRun(run.runRef, "drain", fixedNow)
    store.recordWorkerDone({
      contextId: "ctx.codex.drain",
      taskId: "task.drain.inflight",
      status: "completed",
      now: fixedNow,
    })
    const drained = store.reconcileFleetRun(run.runRef, fixedNow)
    expect(drained.state).toBe("completed")
    expect(drained.stateSource).toBe("operator")

    const after = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 3),
      runner: acceptingRunner(),
      capacity: capacity([{ accountRef: "codex-a", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    })
    expect(after.run.state).toBe("completed")
    expect(after.claimed).toBe(0)
    expect(after.dispatched).toBe(0)
  })

  test("still revives a prematurely auto-closed running run with pending backlog", async () => {
    const { store, run } = createStoreWithRun({ targetConcurrency: 1, workUnits: 3 })
    const dispatched: string[] = []

    const first = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 1),
      runner: {
        dispatch: async (input: FleetRunSupervisorDispatchInput) => {
          dispatched.push(input.workUnit.workUnitRef)
          return {
            assignmentRef: `assignment.${input.claim.claimRef}`,
            lifecycle: [{ event: "assignment.completed", status: "completed" }],
            status: "completed" as const,
          }
        },
      },
      capacity: capacity([{ accountRef: "codex-a", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    })
    expect(first.dispatched).toBe(1)

    // All created tasks are terminal while the planner backlog still has
    // units, so reconciliation auto-closes the running run; the next tick
    // must undo exactly that auto-close and keep working.
    const second = await tickFleetRunSupervisor({
      store,
      pylonRef: "pylon.owner",
      runRef: run.runRef,
      planner: fixturePlannerWithClaims(store, 2),
      runner: acceptingRunner(dispatched),
      capacity: capacity([{ accountRef: "codex-a", advertisedCapacity: 1 }]),
      clock: { now: () => fixedNow },
    })
    expect(second.run.state).toBe("running")
    expect(second.dispatched).toBe(1)
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
        dispatch: async (input: FleetRunSupervisorDispatchInput) => ({
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
        dispatch: async (input: FleetRunSupervisorDispatchInput) => {
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
        dispatch: async (input: FleetRunSupervisorDispatchInput) => {
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
        dispatch: async (input: FleetRunSupervisorDispatchInput) => {
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
    const sleepResolvers: Array<() => void> = []
    let firstTickResolve: (() => void) | null = null
    const firstTick = new Promise<void>(resolve => {
      firstTickResolve = resolve
    })

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
          sleep: () => new Promise<void>(resolve => {
            sleepResolvers.push(resolve)
          }),
        },
        onLifecycle: event => {
          if (event.kind !== "tick" || scopeClosed) return
          tickCount += 1
          firstTickResolve?.()
        },
      }),
      Scope.Scope,
      scope,
    ))

    await firstTick
    await Effect.runPromise(Scope.close(scope, Exit.void))
    scopeClosed = true
    const ticksAtClose = tickCount
    for (const resolve of sleepResolvers.splice(0)) resolve()
    await Promise.resolve()

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
