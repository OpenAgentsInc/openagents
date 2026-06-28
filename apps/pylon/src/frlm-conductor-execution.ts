import { createHash } from "node:crypto"
import { assertPublicProjectionSafe } from "./state.js"

export const FRLM_CONDUCTOR_EXECUTION_SCHEMA =
  "openagents.artanis.frlm_conductor_execution.v0.1"
export const FRLM_CONDUCTOR_DISPATCH_SCHEMA =
  "openagents.artanis.frlm_conductor_dispatch.v0.1"

export type FrlmConductorSubQueryState =
  | "planned"
  | "running"
  | "completed"
  | "failed"
  | "timed_out"
  | "rejected"

export type FrlmConductorExecutionMode =
  | "recursive_parallel"
  | "fallback_linear"
  | "blocked"

export type FrlmConductorExecutionBlockerRef =
  | "blocker.artanis.frlm_conductor.execution_ref_missing"
  | "blocker.artanis.frlm_conductor.root_task_ref_missing"
  | "blocker.artanis.frlm_conductor.blueprint_signature_ref_missing"
  | "blocker.artanis.frlm_conductor.sub_query_plan_missing"
  | "blocker.artanis.frlm_conductor.sub_query_failure_without_linear_fallback"
  | "blocker.artanis.frlm_conductor.linear_executor_ref_missing"
  | "blocker.artanis.frlm_conductor.unsafe_ref"

export type FrlmConductorDispatchBlockerRef =
  | "blocker.artanis.frlm_conductor.dispatch.execution_not_recursive_parallel"
  | "blocker.artanis.frlm_conductor.dispatch.no_available_pylon_slots"
  | "blocker.artanis.frlm_conductor.dispatch.sub_query_dispatch_plan_missing"
  | "blocker.artanis.frlm_conductor.dispatch.unsafe_ref"

export type FrlmConductorSubQuery = {
  subQueryRef: string
  parentRef?: string | null
  state: FrlmConductorSubQueryState
  resultRef?: string | null
  failureRef?: string | null
  blueprintSignatureRef?: string | null
}

export type FrlmConductorPylonSlot = {
  slotRef: string
  pylonRef: string
  accountRef?: string | null
  ready?: boolean
  capacity?: number
  busy?: number
  capabilityRefs?: string[]
}

export type FrlmConductorDispatchAssignment = {
  dispatchRef: string
  subQueryRef: string
  blueprintSignatureRef: string | null
  pylonRef: string
  slotRef: string
  accountRef: string | null
  laneIndex: number
}

export type FrlmConductorExecutionProjection = {
  schema: typeof FRLM_CONDUCTOR_EXECUTION_SCHEMA
  observedAt: string
  executionRef: string | null
  rootTaskRef: string | null
  blueprintSignatureRef: string | null
  executionMode: FrlmConductorExecutionMode
  canExecute: boolean
  recursiveSubQueryRefs: string[]
  failedSubQueryRefs: string[]
  linearFallbackStepRefs: string[]
  linearExecutorRef: string | null
  fallbackReasonRef: string | null
  executionPlanRef: string | null
  evidenceRefs: string[]
  blockerRefs: FrlmConductorExecutionBlockerRef[]
  authorityBoundary: string
  contentRedacted: true
}

export type FrlmConductorDispatchProjection = {
  schema: typeof FRLM_CONDUCTOR_DISPATCH_SCHEMA
  observedAt: string
  executionRef: string | null
  executionPlanRef: string | null
  executionMode: FrlmConductorExecutionMode
  canDispatch: boolean
  dispatchWidth: number
  dispatchAssignments: FrlmConductorDispatchAssignment[]
  queuedSubQueryRefs: string[]
  availableSlotRefs: string[]
  evidenceRefs: string[]
  blockerRefs: FrlmConductorDispatchBlockerRef[]
  authorityBoundary: string
  contentRedacted: true
}

const publicRefPattern = /^[a-z][a-z0-9._:/-]{1,220}$/i
const unsafeRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const failedStates = new Set<FrlmConductorSubQueryState>([
  "failed",
  "timed_out",
  "rejected",
])

export function planFrlmConductorExecution(input: {
  observedAt: string
  executionRef?: string | null
  rootTaskRef?: string | null
  blueprintSignatureRef?: string | null
  subQueries?: FrlmConductorSubQuery[]
  linearFallbackEnabled?: boolean
  linearExecutorRef?: string | null
}): FrlmConductorExecutionProjection {
  const blockerRefs = new Set<FrlmConductorExecutionBlockerRef>()
  const subQueries = input.subQueries ?? []
  const rawRefs = [
    normalizedRef(input.executionRef),
    normalizedRef(input.rootTaskRef),
    normalizedRef(input.blueprintSignatureRef),
    normalizedRef(input.linearExecutorRef),
    ...subQueries.flatMap((subQuery) => [
      normalizedRef(subQuery.subQueryRef),
      normalizedRef(subQuery.parentRef),
      normalizedRef(subQuery.resultRef),
      normalizedRef(subQuery.failureRef),
      normalizedRef(subQuery.blueprintSignatureRef),
    ]),
  ]
  const safeRefs = new Map<string, string | null>()
  for (const ref of rawRefs) {
    if (ref !== null && !safeRefs.has(ref)) {
      safeRefs.set(ref, isSafeRef(ref) ? ref : null)
    }
  }
  const safeRef = (value: string | null | undefined) => {
    const ref = normalizedRef(value)
    return ref === null ? null : safeRefs.get(ref) ?? null
  }

  const executionRef = safeRef(input.executionRef)
  const rootTaskRef = safeRef(input.rootTaskRef)
  const blueprintSignatureRef = safeRef(input.blueprintSignatureRef)
  const linearExecutorRef = safeRef(input.linearExecutorRef)
  const recursiveSubQueryRefs = uniqueRefs(subQueries.map((subQuery) => safeRef(subQuery.subQueryRef)))
  const failedSubQueryRefs = uniqueRefs(
    subQueries
      .filter((subQuery) => failedStates.has(subQuery.state))
      .map((subQuery) => safeRef(subQuery.subQueryRef)),
  )
  const linearFallbackStepRefs = failedSubQueryRefs.length === 0
    ? []
    : recursiveSubQueryRefs.map((subQueryRef, index) =>
      `step.artanis.frlm.linear.${stableHash(`${rootTaskRef ?? "unknown"}:${index}:${subQueryRef}`)}`)

  if (executionRef === null) {
    blockerRefs.add("blocker.artanis.frlm_conductor.execution_ref_missing")
  }
  if (rootTaskRef === null) {
    blockerRefs.add("blocker.artanis.frlm_conductor.root_task_ref_missing")
  }
  if (blueprintSignatureRef === null) {
    blockerRefs.add("blocker.artanis.frlm_conductor.blueprint_signature_ref_missing")
  }
  if (recursiveSubQueryRefs.length === 0) {
    blockerRefs.add("blocker.artanis.frlm_conductor.sub_query_plan_missing")
  }
  if (failedSubQueryRefs.length > 0 && input.linearFallbackEnabled !== true) {
    blockerRefs.add("blocker.artanis.frlm_conductor.sub_query_failure_without_linear_fallback")
  }
  if (failedSubQueryRefs.length > 0 && linearExecutorRef === null) {
    blockerRefs.add("blocker.artanis.frlm_conductor.linear_executor_ref_missing")
  }
  if (rawRefs.some((ref) => ref !== null && !isSafeRef(ref))) {
    blockerRefs.add("blocker.artanis.frlm_conductor.unsafe_ref")
  }

  const executionMode: FrlmConductorExecutionMode =
    blockerRefs.size > 0
      ? "blocked"
      : failedSubQueryRefs.length > 0
        ? "fallback_linear"
        : "recursive_parallel"
  const canExecute = blockerRefs.size === 0
  const fallbackReasonRef = failedSubQueryRefs.length === 0
    ? null
    : `reason.artanis.frlm.sub_query_failure.${stableHash(failedSubQueryRefs.join(":"))}`
  const executionPlanRef =
    canExecute && executionRef !== null
      ? `plan.artanis.frlm.${executionMode}.${stableHash(executionRef)}`
      : null
  const evidenceRefs = uniqueRefs([
    executionPlanRef,
    executionRef,
    rootTaskRef,
    blueprintSignatureRef,
    linearExecutorRef,
    fallbackReasonRef,
    ...recursiveSubQueryRefs,
    ...failedSubQueryRefs,
    ...linearFallbackStepRefs,
    ...subQueries.flatMap((subQuery) => [
      safeRef(subQuery.parentRef),
      safeRef(subQuery.resultRef),
      safeRef(subQuery.failureRef),
      safeRef(subQuery.blueprintSignatureRef),
    ]),
  ])

  const projection: FrlmConductorExecutionProjection = {
    schema: FRLM_CONDUCTOR_EXECUTION_SCHEMA,
    observedAt: input.observedAt,
    executionRef,
    rootTaskRef,
    blueprintSignatureRef,
    executionMode,
    canExecute,
    recursiveSubQueryRefs,
    failedSubQueryRefs,
    linearFallbackStepRefs,
    linearExecutorRef,
    fallbackReasonRef,
    executionPlanRef,
    evidenceRefs,
    blockerRefs: [...blockerRefs].sort(),
    authorityBoundary:
      "Read-only FRLM Conductor execution projection. It selects recursive or linear fallback planning from public evidence refs only; it does not dispatch workers, run Python, spend sats, publish claims, or move settlement authority.",
    contentRedacted: true,
  }
  assertPublicProjectionSafe(projection)
  return projection
}

export function planFrlmConductorPylonDispatch(input: {
  observedAt: string
  executionRef?: string | null
  rootTaskRef?: string | null
  blueprintSignatureRef?: string | null
  subQueries?: FrlmConductorSubQuery[]
  linearFallbackEnabled?: boolean
  linearExecutorRef?: string | null
  pylonSlots?: FrlmConductorPylonSlot[]
}): FrlmConductorDispatchProjection {
  const execution = planFrlmConductorExecution(input)
  const blockerRefs = new Set<FrlmConductorDispatchBlockerRef>()
  const subQueries = input.subQueries ?? []
  const pylonSlots = input.pylonSlots ?? []
  const rawRefs = pylonSlots.flatMap((slot) => [
    normalizedRef(slot.slotRef),
    normalizedRef(slot.pylonRef),
    normalizedRef(slot.accountRef),
    ...(slot.capabilityRefs ?? []).map(normalizedRef),
  ])
  const safeRefs = new Map<string, string | null>()
  for (const ref of rawRefs) {
    if (ref !== null && !safeRefs.has(ref)) {
      safeRefs.set(ref, isSafeRef(ref) ? ref : null)
    }
  }
  const safeRef = (value: string | null | undefined) => {
    const ref = normalizedRef(value)
    return ref === null ? null : safeRefs.get(ref) ?? null
  }
  const runnableSubQueries = subQueries
    .filter((subQuery) => subQuery.state === "planned")
    .map((subQuery) => ({
      subQueryRef: execution.recursiveSubQueryRefs.find((ref) => ref === normalizedRef(subQuery.subQueryRef)) ?? null,
      blueprintSignatureRef: normalizedRef(subQuery.blueprintSignatureRef) === null
        ? execution.blueprintSignatureRef
        : execution.evidenceRefs.find((ref) => ref === normalizedRef(subQuery.blueprintSignatureRef)) ?? null,
    }))
    .filter((subQuery): subQuery is { subQueryRef: string; blueprintSignatureRef: string | null } =>
      subQuery.subQueryRef !== null)
    .sort((left, right) => left.subQueryRef.localeCompare(right.subQueryRef))
  const lanes = pylonSlots.flatMap((slot) => {
    const slotRef = safeRef(slot.slotRef)
    const pylonRef = safeRef(slot.pylonRef)
    const accountRef = safeRef(slot.accountRef)
    if (slot.ready === false || slotRef === null || pylonRef === null) return []
    const capacity = Math.max(0, Math.floor(slot.capacity ?? 1))
    const busy = Math.max(0, Math.floor(slot.busy ?? 0))
    const free = Math.max(0, capacity - busy)
    return Array.from({ length: free }, (_, laneIndex) => ({
      accountRef,
      laneIndex,
      pylonRef,
      slotRef,
    }))
  }).sort((left, right) =>
    `${left.pylonRef}:${left.slotRef}:${left.laneIndex}`.localeCompare(
      `${right.pylonRef}:${right.slotRef}:${right.laneIndex}`,
    ))

  if (execution.executionMode !== "recursive_parallel" || !execution.canExecute) {
    blockerRefs.add("blocker.artanis.frlm_conductor.dispatch.execution_not_recursive_parallel")
  }
  if (runnableSubQueries.length === 0) {
    blockerRefs.add("blocker.artanis.frlm_conductor.dispatch.sub_query_dispatch_plan_missing")
  }
  if (lanes.length === 0) {
    blockerRefs.add("blocker.artanis.frlm_conductor.dispatch.no_available_pylon_slots")
  }
  if (rawRefs.some((ref) => ref !== null && !isSafeRef(ref))) {
    blockerRefs.add("blocker.artanis.frlm_conductor.dispatch.unsafe_ref")
  }

  const dispatchWidth = blockerRefs.size === 0 ? Math.min(runnableSubQueries.length, lanes.length) : 0
  const dispatchAssignments = Array.from({ length: dispatchWidth }, (_, index) => {
    const subQuery = runnableSubQueries[index]!
    const lane = lanes[index]!
    return {
      dispatchRef: `dispatch.artanis.frlm.pylon.${stableHash(`${execution.executionRef}:${subQuery.subQueryRef}:${lane.slotRef}:${lane.laneIndex}`)}`,
      subQueryRef: subQuery.subQueryRef,
      blueprintSignatureRef: subQuery.blueprintSignatureRef,
      pylonRef: lane.pylonRef,
      slotRef: lane.slotRef,
      accountRef: lane.accountRef,
      laneIndex: lane.laneIndex,
    }
  })
  const queuedSubQueryRefs = runnableSubQueries
    .slice(dispatchWidth)
    .map((subQuery) => subQuery.subQueryRef)
  const availableSlotRefs = uniqueRefs(lanes.map((lane) => lane.slotRef))
  const evidenceRefs = uniqueRefs([
    execution.executionRef,
    execution.executionPlanRef,
    ...execution.evidenceRefs,
    ...dispatchAssignments.flatMap((assignment) => [
      assignment.dispatchRef,
      assignment.subQueryRef,
      assignment.blueprintSignatureRef,
      assignment.pylonRef,
      assignment.slotRef,
      assignment.accountRef,
    ]),
    ...queuedSubQueryRefs,
    ...availableSlotRefs,
  ])
  const projection: FrlmConductorDispatchProjection = {
    schema: FRLM_CONDUCTOR_DISPATCH_SCHEMA,
    observedAt: input.observedAt,
    executionRef: execution.executionRef,
    executionPlanRef: execution.executionPlanRef,
    executionMode: execution.executionMode,
    canDispatch: blockerRefs.size === 0,
    dispatchWidth,
    dispatchAssignments,
    queuedSubQueryRefs,
    availableSlotRefs,
    evidenceRefs,
    blockerRefs: [...blockerRefs].sort(),
    authorityBoundary:
      "Read-only FRLM Conductor Pylon-slot dispatch projection. It maps planned recursive sub-queries to public Pylon capacity refs only; it does not run workers, execute Python, spend sats, publish claims, or move settlement authority.",
    contentRedacted: true,
  }
  assertPublicProjectionSafe(projection)
  return projection
}

function normalizedRef(value: string | null | undefined): string | null {
  if (value === null || value === undefined) return null
  const trimmed = value.trim()
  return trimmed === "" ? null : trimmed
}

function isSafeRef(value: string) {
  return publicRefPattern.test(value) && !unsafeRefPattern.test(value)
}

function stableHash(input: string) {
  return createHash("sha256").update(input).digest("hex").slice(0, 20)
}

function uniqueRefs(refs: (string | null)[]) {
  return [...new Set(refs.filter((ref): ref is string => ref !== null && ref.trim().length > 0))]
    .map((ref) => ref.trim())
    .sort()
}
