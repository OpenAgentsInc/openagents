import type { DispatchContext, OrchestrationTask, PylonOrchestrationStore } from "./store.js"

export type DispatchLiveness = "fresh" | "stale" | "missing" | "hung"

export type DispatchEligibility =
  | { ok: true }
  | {
      ok: false
      reason:
        | "not_idle"
        | "circuit_broken"
        | "heartbeat_missing"
        | "heartbeat_stale"
        | "dispatch_hung"
        | "base_drift"
        | "runner_mismatch"
    }

export type SupervisorCoordinatorOptions = {
  now?: Date
  heartbeatFreshMs?: number
  hungAfterMs?: number
  maxBaseBehindBy?: number
  maxConcurrentSlots?: number
}

export type PlannedDispatch = {
  task: OrchestrationTask
  context: DispatchContext
}

export type DispatchCoordinatorResult = {
  planned: PlannedDispatch[]
  refused: Array<{ context: DispatchContext; eligibility: DispatchEligibility }>
}

const DEFAULT_HEARTBEAT_FRESH_MS = 5 * 60 * 1000
const DEFAULT_HUNG_AFTER_MS = 10 * 60 * 1000
const DEFAULT_MAX_BASE_BEHIND_BY = 20

export function dispatchLiveness(
  context: DispatchContext,
  options: Pick<SupervisorCoordinatorOptions, "now" | "heartbeatFreshMs" | "hungAfterMs"> = {},
): DispatchLiveness {
  if (context.lastHeartbeatAt === null) return "missing"
  const now = options.now ?? new Date()
  const freshMs = options.heartbeatFreshMs ?? DEFAULT_HEARTBEAT_FRESH_MS
  const hungMs = options.hungAfterMs ?? DEFAULT_HUNG_AFTER_MS
  const ageMs = now.getTime() - new Date(context.lastHeartbeatAt).getTime()
  if (ageMs > hungMs) return "hung"
  if (ageMs > freshMs) return "stale"
  return "fresh"
}

export function dispatchEligibility(
  context: DispatchContext,
  taskOrOptions?: OrchestrationTask | SupervisorCoordinatorOptions,
  maybeOptions: SupervisorCoordinatorOptions = {},
): DispatchEligibility {
  const task = taskOrOptions !== undefined && "status" in taskOrOptions && "spec" in taskOrOptions
    ? taskOrOptions
    : undefined
  const options = task === undefined
    ? (taskOrOptions as SupervisorCoordinatorOptions | undefined) ?? maybeOptions
    : maybeOptions
  if (context.status === "circuit_broken") return { ok: false, reason: "circuit_broken" }
  if (context.status !== "idle") return { ok: false, reason: "not_idle" }
  if (task?.spec.runnerKind !== undefined && context.runnerKind !== "generic" && context.runnerKind !== task.spec.runnerKind) {
    return { ok: false, reason: "runner_mismatch" }
  }
  const liveness = dispatchLiveness(context, options)
  if (liveness === "missing") return { ok: false, reason: "heartbeat_missing" }
  if (liveness === "stale") return { ok: false, reason: "heartbeat_stale" }
  if (liveness === "hung") return { ok: false, reason: "dispatch_hung" }
  if (context.baseBehindBy > (options.maxBaseBehindBy ?? DEFAULT_MAX_BASE_BEHIND_BY)) {
    return { ok: false, reason: "base_drift" }
  }
  return { ok: true }
}

export function planSupervisorDispatch(
  tasks: readonly OrchestrationTask[],
  contexts: readonly DispatchContext[],
  options: SupervisorCoordinatorOptions = {},
): DispatchCoordinatorResult {
  const maxSlots = options.maxConcurrentSlots ?? contexts.reduce((sum, context) => sum + context.maxConcurrentSlots, 0)
  const active = contexts.filter((context) => context.status === "dispatched").length
  const capacity = Math.max(0, maxSlots - active)
  const readyTasks = tasks.filter((task) => task.status === "ready")
  const planned: PlannedDispatch[] = []
  const refused: DispatchCoordinatorResult["refused"] = []
  const plannedTaskIds = new Set<string>()

  for (const context of contexts) {
    if (planned.length >= capacity || planned.length >= readyTasks.length) break
    const firstUnplannedTask = readyTasks.find((candidate) => !plannedTaskIds.has(candidate.id))
    const task = readyTasks.find((candidate) => {
      if (plannedTaskIds.has(candidate.id)) return false
      return dispatchEligibility(context, candidate, options).ok
    })
    const eligibility = dispatchEligibility(context, task ?? firstUnplannedTask, options)
    if (!eligibility.ok) {
      refused.push({ context, eligibility })
      continue
    }
    if (task === undefined) break
    plannedTaskIds.add(task.id)
    planned.push({ task, context })
  }

  return { planned, refused }
}

export function dispatchReadySupervisorTasks(
  store: PylonOrchestrationStore,
  options: SupervisorCoordinatorOptions = {},
): DispatchCoordinatorResult {
  store.promoteReadyTasks(options.now)
  const result = planSupervisorDispatch(store.listTasks(), store.listDispatchContexts(), options)
  for (const dispatch of result.planned) {
    const virtualHead = store.reserveVirtualHeadForTask(dispatch.task.id, options.now)
    store.markDispatched(dispatch.task.id, dispatch.context.id, options.now)
    store.appendMessage({
      id: `message.${dispatch.task.id}.${dispatch.context.id}.dispatch`,
      threadId: dispatch.task.threadId,
      taskId: dispatch.task.id,
      dispatchContextId: dispatch.context.id,
      kind: "dispatch",
      body:
        virtualHead === null
          ? `dispatch ${dispatch.task.id} -> ${dispatch.context.assigneeHandle}`
          : `dispatch ${dispatch.task.id} -> ${dispatch.context.assigneeHandle} from ${virtualHead.branchFrom} projecting ${virtualHead.projectedHead}`,
      now: options.now,
    })
  }
  return result
}
