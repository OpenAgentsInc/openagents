import { Effect, Scope } from "effect"

import type {
  FleetRun,
  OrchestrationTaskStatus,
  PylonOrchestrationStore,
  WorkClaim,
} from "../../../../apps/pylon/src/orchestration/store.js"
import type {
  WorkPlannerClaimableUnit,
  WorkPlannerOutput,
} from "../../../../apps/pylon/src/orchestration/work-planner.js"

export const FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT = 10

export type FleetRunSupervisorAccount = {
  readonly accountRef: string
  readonly advertisedCapacity: number
  readonly cooldownUntil?: Date | string | null
}

export type FleetRunSupervisorLifecycleEvent = {
  readonly assignmentRef?: string | null
  readonly event: string
  readonly phase?: string | null
  readonly status?: string | null
}

export type FleetRunSupervisorDispatchInput = {
  readonly accountRef: string
  readonly claim: WorkClaim
  readonly run: FleetRun
  readonly taskId: string
  readonly workUnit: WorkPlannerClaimableUnit
}

export type FleetRunSupervisorDispatchResult = {
  readonly assignmentRef: string | null
  readonly lifecycle: readonly FleetRunSupervisorLifecycleEvent[]
  readonly status: "accepted" | "blocked" | "failed" | "completed"
  readonly summary?: string | null
}

export type FleetRunSupervisorActiveAssignment = {
  readonly accountRef: string
  readonly claim: WorkClaim
  readonly contextId: string
  readonly taskId: string
}

export type FleetRunSupervisorReconcileResult = FleetRunSupervisorDispatchResult & {
  readonly taskId: string
}

export type FleetRunSupervisorRunner = {
  readonly dispatch: (input: FleetRunSupervisorDispatchInput) => Promise<FleetRunSupervisorDispatchResult>
  readonly reconcile?: (input: {
    readonly activeAssignments: readonly FleetRunSupervisorActiveAssignment[]
    readonly now: Date
    readonly run: FleetRun
  }) => Promise<readonly FleetRunSupervisorReconcileResult[]>
}

export type FleetRunSupervisorPlanner = {
  readonly plan: (input: { readonly run: FleetRun; readonly now: Date }) => Promise<WorkPlannerOutput>
}

export type FleetRunSupervisorCapacity = {
  readonly accounts: (input: { readonly run: FleetRun; readonly now: Date }) => Promise<readonly FleetRunSupervisorAccount[]>
}

export type FleetRunSupervisorClock = {
  readonly now: () => Date
  readonly sleep: (ms: number) => Promise<void>
}

export type FleetRunSupervisorOptions = {
  readonly store: PylonOrchestrationStore
  readonly pylonRef: string
  readonly runRef: string
  readonly planner: FleetRunSupervisorPlanner
  readonly runner: FleetRunSupervisorRunner
  readonly capacity: FleetRunSupervisorCapacity
  readonly clock?: Partial<FleetRunSupervisorClock>
  readonly tickIntervalMs?: number
  readonly claimTtlMs?: number
  readonly maxSpawnPerTick?: number
  readonly onLifecycle?: (event: FleetRunSupervisorObservedEvent) => void | Promise<void>
}

export type FleetRunSupervisorObservedEvent =
  | {
    readonly kind: "tick"
    readonly runRef: string
    readonly activeAssignments: number
    readonly freeSlots: number
  }
  | {
    readonly kind: "dispatch"
    readonly runRef: string
    readonly taskId: string
    readonly claimRef: string
    readonly workUnitRef: string
    readonly accountRef: string
    readonly assignmentRef: string | null
    readonly status: FleetRunSupervisorDispatchResult["status"]
  }
  | {
    readonly kind: "lifecycle"
    readonly runRef: string
    readonly taskId: string
    readonly claimRef: string
    readonly accountRef: string
    readonly event: FleetRunSupervisorLifecycleEvent
  }
  | {
    readonly kind: "completed"
    readonly runRef: string
    readonly reason: "backlog_empty" | "drained"
  }

export type FleetRunSupervisorTickResult = {
  readonly activeAssignments: number
  readonly claimed: number
  readonly dispatched: number
  readonly freeSlots: number
  readonly run: FleetRun
}

export type FleetRunSupervisorHandle = {
  readonly pylonRef: string
  readonly runRef: string
  readonly stop: () => Effect.Effect<void>
  readonly tick: () => Effect.Effect<FleetRunSupervisorTickResult, FleetRunSupervisorError>
}

export class FleetRunSupervisorError extends Error {
  readonly _tag = "FleetRunSupervisorError"
  override readonly cause: unknown

  constructor(message: string, cause?: unknown) {
    super(message)
    this.name = "FleetRunSupervisorError"
    this.cause = cause
  }
}

const activeSupervisors = new Map<string, string>()

const defaultClock: FleetRunSupervisorClock = {
  now: () => new Date(),
  sleep: ms => new Promise(resolve => setTimeout(resolve, ms)),
}

const liveTaskStatuses = new Set<OrchestrationTaskStatus>(["dispatched"])

const terminalStatusForDispatch = (
  result: FleetRunSupervisorDispatchResult,
): Extract<OrchestrationTaskStatus, "completed" | "failed" | "blocked"> | null => {
  if (result.status === "completed") return "completed"
  if (result.status === "failed") return "failed"
  if (result.status === "blocked") return "blocked"
  const terminal = [...result.lifecycle].reverse().find(event =>
    event.status === "completed" || event.status === "failed" || event.status === "blocked"
  )
  if (terminal?.status === "completed") return "completed"
  if (terminal?.status === "failed") return "failed"
  if (terminal?.status === "blocked") return "blocked"
  return null
}

const isoSafe = (date: Date | string): string =>
  typeof date === "string" ? date : date.toISOString()

const isCoolingDown = (account: FleetRunSupervisorAccount, now: Date): boolean => {
  if (account.cooldownUntil === null || account.cooldownUntil === undefined) return false
  const until = Date.parse(isoSafe(account.cooldownUntil))
  return !Number.isNaN(until) && until > now.getTime()
}

const activeAssignmentsForRun = (store: PylonOrchestrationStore, runRef: string): number =>
  store.listTasks().filter(task => task.spec.fleetRunRef === runRef && liveTaskStatuses.has(task.status)).length

const liveClaimsForRun = (store: PylonOrchestrationStore, runRef: string, now: Date): WorkClaim[] =>
  store.listLiveWorkClaims(now).filter(claim => claim.runRef === runRef)

const activeByAccount = (claims: readonly WorkClaim[]): Map<string, number> => {
  const counts = new Map<string, number>()
  for (const claim of claims) {
    if (claim.state === "closeout") continue
    counts.set(claim.workerAccountRef, (counts.get(claim.workerAccountRef) ?? 0) + 1)
  }
  return counts
}

const collectActiveAssignments = (
  store: PylonOrchestrationStore,
  runRef: string,
): FleetRunSupervisorActiveAssignment[] => {
  const tasks = store.listTasks("dispatched").filter(task => task.spec.fleetRunRef === runRef)
  const claims = new Map(store.listWorkClaims({ runRef }).map(claim => [taskIdFor(runRef, claim.claimRef), claim]))
  return tasks.flatMap(task => {
    const claim = claims.get(task.id)
    if (claim === undefined || claim.state !== "in_progress") return []
    return [{
      accountRef: claim.workerAccountRef,
      claim,
      contextId: contextIdFor(claim.workerAccountRef, task.id),
      taskId: task.id,
    }]
  })
}

const pickAccount = (
  accounts: readonly FleetRunSupervisorAccount[],
  activeCounts: Map<string, number>,
  now: Date,
): FleetRunSupervisorAccount | null => {
  const eligible = accounts
    .filter(account => account.advertisedCapacity > 0)
    .filter(account => !isCoolingDown(account, now))
    .map(account => ({
      account,
      free: Math.max(0, account.advertisedCapacity - (activeCounts.get(account.accountRef) ?? 0)),
    }))
    .filter(entry => entry.free > 0)
    .sort((a, b) => b.free - a.free || a.account.accountRef.localeCompare(b.account.accountRef))
  return eligible[0]?.account ?? null
}

const taskIdFor = (runRef: string, claimRef: string): string =>
  `${runRef}.task.${claimRef.replace(/[^a-zA-Z0-9_.-]/g, "_")}`

const contextIdFor = (accountRef: string, taskId: string): string =>
  `${accountRef}.ctx.${taskId}`.replace(/[^a-zA-Z0-9_.-]/g, "_")

const claimRefFor = (runRef: string, workUnitRef: string, now: Date, ordinal: number): string =>
  `${runRef}.claim.${workUnitRef.replace(/[^a-zA-Z0-9_.-]/g, "_")}.${now.getTime()}.${ordinal}`

const emit = async (
  sink: FleetRunSupervisorOptions["onLifecycle"],
  event: FleetRunSupervisorObservedEvent,
): Promise<void> => {
  if (sink !== undefined) await sink(event)
}

const recordTerminalAssignment = async (
  options: FleetRunSupervisorOptions,
  assignment: FleetRunSupervisorActiveAssignment,
  result: FleetRunSupervisorDispatchResult,
  now: Date,
): Promise<boolean> => {
  const terminalStatus = terminalStatusForDispatch(result)
  if (terminalStatus === null) return false
  for (const event of result.lifecycle) {
    await emit(options.onLifecycle, {
      kind: "lifecycle",
      runRef: options.runRef,
      taskId: assignment.taskId,
      claimRef: assignment.claim.claimRef,
      accountRef: assignment.accountRef,
      event,
    })
  }
  if (result.assignmentRef !== null) {
    options.store.updateWorkClaimAssignmentRef(assignment.claim.claimRef, result.assignmentRef, now)
  }
  options.store.recordWorkerDone({
    contextId: assignment.contextId,
    taskId: assignment.taskId,
    status: terminalStatus,
    result: JSON.stringify({
      assignmentRef: result.assignmentRef,
      summary: result.summary ?? null,
    }),
    now,
  })
  options.store.updateWorkClaimState(
    assignment.claim.claimRef,
    terminalStatus === "completed" ? "closeout" : "released",
    now,
  )
  await emit(options.onLifecycle, {
    kind: "dispatch",
    runRef: options.runRef,
    taskId: assignment.taskId,
    claimRef: assignment.claim.claimRef,
    workUnitRef: assignment.claim.workUnitRef,
    accountRef: assignment.accountRef,
    assignmentRef: result.assignmentRef,
    status: result.status,
  })
  return true
}

export async function tickFleetRunSupervisor(
  options: FleetRunSupervisorOptions,
): Promise<FleetRunSupervisorTickResult> {
  const clock = { ...defaultClock, ...options.clock }
  const now = clock.now()
  const store = options.store
  const maxSpawnPerTick = Math.min(
    Math.max(1, Math.trunc(options.maxSpawnPerTick ?? FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT)),
    FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT,
  )
  const claimTtlMs = options.claimTtlMs ?? 30 * 60 * 1000

  store.reconcileWorkClaims({ now })
  const expectedWorkUnitsTotal = store.getFleetRun(options.runRef)?.counters.workUnitsTotal ?? 0
  let run = store.reconcileFleetRun(options.runRef, now)
  if (run.state !== "running" && run.counters.workUnitsTotal < expectedWorkUnitsTotal) {
    run = store.upsertFleetRun({
      ...run,
      state: "running",
      counters: {
        ...run.counters,
        workUnitsTotal: expectedWorkUnitsTotal,
      },
      updatedAt: now.toISOString(),
    })
  }
  if (run.state !== "running") {
    return {
      activeAssignments: activeAssignmentsForRun(store, run.runRef),
      claimed: 0,
      dispatched: 0,
      freeSlots: 0,
      run,
    }
  }

  const activeBeforeReconcile = collectActiveAssignments(store, run.runRef)
  if (options.runner.reconcile !== undefined && activeBeforeReconcile.length > 0) {
    const activeByTask = new Map(activeBeforeReconcile.map(assignment => [assignment.taskId, assignment]))
    const reconciled = await options.runner.reconcile({ activeAssignments: activeBeforeReconcile, now, run })
    for (const result of reconciled) {
      const assignment = activeByTask.get(result.taskId)
      if (assignment === undefined) continue
      await recordTerminalAssignment(options, assignment, result, now)
    }
    run = store.reconcileFleetRun(options.runRef, now)
    if (run.state !== "running" && run.counters.workUnitsTotal < expectedWorkUnitsTotal) {
      run = store.upsertFleetRun({
        ...run,
        state: "running",
        counters: {
          ...run.counters,
          workUnitsTotal: expectedWorkUnitsTotal,
        },
        updatedAt: now.toISOString(),
      })
    }
    if (run.state !== "running") {
      return {
        activeAssignments: activeAssignmentsForRun(store, run.runRef),
        claimed: 0,
        dispatched: 0,
        freeSlots: 0,
        run,
      }
    }
  }

  const liveClaims = liveClaimsForRun(store, run.runRef, now)
  const accounts = await options.capacity.accounts({ run, now })
  const activeCounts = activeByAccount(liveClaims)
  const activeAssignments = activeAssignmentsForRun(store, run.runRef)
  const targetFreeSlots = Math.max(0, run.targetConcurrency - activeAssignments)
  const advertisedFreeSlots = accounts.reduce((total, account) => {
    if (isCoolingDown(account, now)) return total
    return total + Math.max(0, account.advertisedCapacity - (activeCounts.get(account.accountRef) ?? 0))
  }, 0)
  const freeSlots = Math.min(targetFreeSlots, advertisedFreeSlots, maxSpawnPerTick)

  await emit(options.onLifecycle, {
    kind: "tick",
    runRef: run.runRef,
    activeAssignments,
    freeSlots,
  })

  if (freeSlots <= 0) {
    return { activeAssignments, claimed: 0, dispatched: 0, freeSlots, run }
  }

  const plan = await options.planner.plan({ run, now })
  const plannedWorkUnitsTotal = Math.max(expectedWorkUnitsTotal, run.counters.workUnitsTotal)
  let claimed = 0
  let dispatched = 0
  const claimOrdinalBase = store.listWorkClaims({ runRef: run.runRef }).length

  for (const workUnit of plan.claimable) {
    if (dispatched >= freeSlots) break
    const account = pickAccount(accounts, activeCounts, now)
    if (account === null) break
    const claim = store.tryClaimWorkUnit({
      claimRef: claimRefFor(run.runRef, workUnit.workUnitRef, now, claimOrdinalBase + claimed),
      workUnitRef: workUnit.workUnitRef,
      runRef: run.runRef,
      workerAccountRef: account.accountRef,
      ttl: claimTtlMs,
      now,
    })
    if (claim === null) continue
    claimed += 1
    activeCounts.set(account.accountRef, (activeCounts.get(account.accountRef) ?? 0) + 1)

    const taskId = taskIdFor(run.runRef, claim.claimRef)
    const contextId = contextIdFor(account.accountRef, taskId)
    if (store.getDispatchContext(contextId) === null) {
      store.createDispatchContext({
        id: contextId,
        assigneeHandle: account.accountRef,
        runnerKind: run.workerKind === "claude" ? "claude_agent" : "codex",
        lastHeartbeatAt: now,
        maxConcurrentSlots: 1,
        now,
      })
    }
    store.createTask({
      id: taskId,
      spec: {
        title: workUnit.title,
        prompt: run.objective,
        runnerKind: run.workerKind === "claude" ? "claude_agent" : "codex",
        ...(workUnit.repo === undefined ? {} : { repo: workUnit.repo }),
        issueRef: workUnit.number === undefined ? workUnit.workUnitRef : `#${workUnit.number}`,
        fleetRunRef: run.runRef,
      },
      status: "ready",
      now,
    })
    store.markDispatched(taskId, contextId, now)
    store.updateWorkClaimState(claim.claimRef, "in_progress", now)

    let result: FleetRunSupervisorDispatchResult
    try {
      result = await options.runner.dispatch({ accountRef: account.accountRef, claim, run, taskId, workUnit })
      dispatched += 1
    } catch (error) {
      dispatched += 1
      store.recordWorkerDone({
        contextId,
        taskId,
        status: "failed",
        result: JSON.stringify({
          assignmentRef: null,
          summary: error instanceof Error ? error.message : String(error),
        }),
        now,
      })
      store.releaseWorkClaim(claim.claimRef, now)
      activeCounts.set(account.accountRef, Math.max(0, (activeCounts.get(account.accountRef) ?? 1) - 1))
      await emit(options.onLifecycle, {
        kind: "dispatch",
        runRef: run.runRef,
        taskId,
        claimRef: claim.claimRef,
        workUnitRef: workUnit.workUnitRef,
        accountRef: account.accountRef,
        assignmentRef: null,
        status: "failed",
      })
      continue
    }

    for (const event of result.lifecycle) {
      await emit(options.onLifecycle, {
        kind: "lifecycle",
        runRef: run.runRef,
        taskId,
        claimRef: claim.claimRef,
        accountRef: account.accountRef,
        event,
      })
    }
    if (result.assignmentRef !== null) {
      store.updateWorkClaimAssignmentRef(claim.claimRef, result.assignmentRef, now)
    }

    const terminalStatus = terminalStatusForDispatch(result)
    if (terminalStatus === null) {
      await emit(options.onLifecycle, {
        kind: "dispatch",
        runRef: run.runRef,
        taskId,
        claimRef: claim.claimRef,
        workUnitRef: workUnit.workUnitRef,
        accountRef: account.accountRef,
        assignmentRef: result.assignmentRef,
        status: result.status,
      })
      continue
    }

    store.recordWorkerDone({
      contextId,
      taskId,
      status: terminalStatus,
      result: JSON.stringify({
        assignmentRef: result.assignmentRef,
        summary: result.summary ?? null,
      }),
      now,
    })
    store.updateWorkClaimState(
      claim.claimRef,
      terminalStatus === "completed" ? "closeout" : "released",
      now,
    )
    await emit(options.onLifecycle, {
      kind: "dispatch",
      runRef: run.runRef,
      taskId,
      claimRef: claim.claimRef,
      workUnitRef: workUnit.workUnitRef,
      accountRef: account.accountRef,
      assignmentRef: result.assignmentRef,
      status: result.status,
    })
  }

  let reconciled = store.reconcileFleetRun(run.runRef, clock.now())
  const hasClaimableBacklog = plan.claimable.length > claimed
  if (reconciled.counters.workUnitsTotal < plannedWorkUnitsTotal) {
    reconciled = store.upsertFleetRun({
      ...reconciled,
      state: hasClaimableBacklog ? "running" : reconciled.state,
      counters: {
        ...reconciled.counters,
        workUnitsTotal: plannedWorkUnitsTotal,
      },
      updatedAt: clock.now().toISOString(),
    })
  } else if (reconciled.state !== "running" && hasClaimableBacklog) {
    reconciled = store.updateFleetRunState(run.runRef, "running", clock.now())
  }
  if (reconciled.state === "completed") {
    await emit(options.onLifecycle, { kind: "completed", runRef: run.runRef, reason: "drained" })
  }
  if (
    reconciled.state === "running" &&
    plan.claimable.length === 0 &&
    activeAssignmentsForRun(store, run.runRef) === 0 &&
    reconciled.refillPolicy.stopCondition === "backlog_empty"
  ) {
    const completed = store.updateFleetRunState(run.runRef, "completed", clock.now())
    await emit(options.onLifecycle, { kind: "completed", runRef: run.runRef, reason: "backlog_empty" })
    return {
      activeAssignments: activeAssignmentsForRun(store, run.runRef),
      claimed,
      dispatched,
      freeSlots,
      run: completed,
    }
  }

  return {
    activeAssignments: activeAssignmentsForRun(store, run.runRef),
    claimed,
    dispatched,
    freeSlots,
    run: reconciled,
  }
}

export function makeFleetRunSupervisor(
  options: FleetRunSupervisorOptions,
): Effect.Effect<FleetRunSupervisorHandle, FleetRunSupervisorError> {
  return Effect.sync(() => {
    const existing = activeSupervisors.get(options.pylonRef)
    if (existing !== undefined) {
      throw new FleetRunSupervisorError(`fleet run supervisor already active for pylon ${options.pylonRef}: ${existing}`)
    }
    activeSupervisors.set(options.pylonRef, options.runRef)
    let stopped = false
    return {
      pylonRef: options.pylonRef,
      runRef: options.runRef,
      stop: () => Effect.sync(() => {
        stopped = true
        if (activeSupervisors.get(options.pylonRef) === options.runRef) activeSupervisors.delete(options.pylonRef)
      }),
      tick: () => Effect.tryPromise({
        try: async () => {
          if (stopped) throw new FleetRunSupervisorError(`fleet run supervisor stopped: ${options.runRef}`)
          return await tickFleetRunSupervisor(options)
        },
        catch: (error: unknown) => error instanceof FleetRunSupervisorError
          ? error
          : new FleetRunSupervisorError("fleet run supervisor tick failed", error),
      }),
    }
  })
}

export function startFleetRunSupervisor(
  options: FleetRunSupervisorOptions,
): Effect.Effect<FleetRunSupervisorHandle, FleetRunSupervisorError, Scope.Scope> {
  return Effect.gen(function* () {
    const handle = yield* makeFleetRunSupervisor(options)
    const scope = yield* Effect.scope
    let loopStopped = false
    yield* Scope.addFinalizer(
      scope,
      Effect.gen(function* () {
        loopStopped = true
        yield* handle.stop()
      }),
    )
    // tickFleetRunSupervisor intentionally bypasses the one-supervisor guard as the direct test seam.
    const context = yield* Effect.context<never>()
    void (async () => {
      const clock = { ...defaultClock, ...options.clock }
      while (!loopStopped) {
        try {
          await Effect.runPromiseWith(context)(handle.tick())
        } catch {
          // Keep the scoped supervisor alive; individual dispatch failures are recorded by tick.
        }
        if (!loopStopped) await clock.sleep(options.tickIntervalMs ?? 1000)
      }
    })()
    return handle
  })
}
