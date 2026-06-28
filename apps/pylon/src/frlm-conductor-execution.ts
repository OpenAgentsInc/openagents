import { createHash } from "node:crypto"
import { assertPublicProjectionSafe } from "./state.js"

export const FRLM_CONDUCTOR_EXECUTION_SCHEMA =
  "openagents.artanis.frlm_conductor_execution.v0.1"
export const FRLM_RLM_STEP_TRACE_SCHEMA =
  "openagents.artanis.frlm_rlm_step_trace.v0.1"

export const FRLM_RESPONSE_COMPOSITION_SCHEMA =
  "openagents.artanis.frlm_response_composition.v0.1"

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
  | "blocker.artanis.frlm_conductor.budget_policy_ref_missing"
  | "blocker.artanis.frlm_conductor.budget_policy_invalid"
  | "blocker.artanis.frlm_conductor.token_budget_exceeded"
  | "blocker.artanis.frlm_conductor.depth_limit_exceeded"
  | "blocker.artanis.frlm_conductor.sub_query_plan_missing"
  | "blocker.artanis.frlm_conductor.sub_query_failure_without_linear_fallback"
  | "blocker.artanis.frlm_conductor.linear_executor_ref_missing"
  | "blocker.artanis.frlm_conductor.unsafe_ref"

export type FrlmConductorBudgetPolicy = {
  budgetPolicyRef: string
  maxTokens: number
  maxDepth: number
}

export type FrlmResponseCompositionBlockerRef =
  | "blocker.artanis.frlm_response_composition.composition_ref_missing"
  | "blocker.artanis.frlm_response_composition.root_task_ref_missing"
  | "blocker.artanis.frlm_response_composition.execution_plan_ref_missing"
  | "blocker.artanis.frlm_response_composition.blueprint_signature_ref_missing"
  | "blocker.artanis.frlm_response_composition.response_blueprint_signature_ref_missing"
  | "blocker.artanis.frlm_response_composition.execution_plan_blocked"
  | "blocker.artanis.frlm_response_composition.response_segment_missing"
  | "blocker.artanis.frlm_response_composition.sub_query_incomplete"
  | "blocker.artanis.frlm_response_composition.sub_query_result_missing"
  | "blocker.artanis.frlm_response_composition.sub_query_response_text_missing"
  | "blocker.artanis.frlm_response_composition.unsafe_content"
  | "blocker.artanis.frlm_response_composition.unsafe_ref"

export type FrlmRlmTraceStepKind =
  | "blueprint_gate"
  | "recursive_sub_query"
  | "linear_fallback"
  | "result_synthesis"
export type FrlmConductorSchedulerBlockerRef =
  | FrlmConductorExecutionBlockerRef
  | "blocker.artanis.frlm_conductor.recursive_submitter_ref_missing"
  | "blocker.artanis.frlm_conductor.max_parallelism_invalid"
  | "blocker.artanis.frlm_conductor.sub_query_budget_exceeded"
  | "blocker.artanis.frlm_conductor.linear_fallback_budget_exceeded"

export type FrlmConductorSubQuery = {
  subQueryRef: string
  parentRef?: string | null
  state: FrlmConductorSubQueryState
  resultRef?: string | null
  failureRef?: string | null
  blueprintSignatureRef?: string | null
  depth?: number | null
  tokenCount?: number | null
}

export type FrlmConductorExecutionProjection = {
  schema: typeof FRLM_CONDUCTOR_EXECUTION_SCHEMA
  observedAt: string
  executionRef: string | null
  rootTaskRef: string | null
  blueprintSignatureRef: string | null
  budgetPolicyRef: string | null
  tokenBudget: number | null
  projectedTokenCount: number
  depthLimit: number | null
  projectedMaxDepth: number
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

export type FrlmResponseCompositionSegment = {
  subQueryRef: string
  state: FrlmConductorSubQueryState
  responseText?: string | null
  resultRef?: string | null
  responseRef?: string | null
  parentRef?: string | null
  blueprintSignatureRef?: string | null
  order?: number | null
}

export type FrlmResponseCompositionProjection = {
  schema: typeof FRLM_RESPONSE_COMPOSITION_SCHEMA
  observedAt: string
  compositionRef: string | null
  rootTaskRef: string | null
  executionPlanRef: string | null
  blueprintSignatureRef: string | null
  responseBlueprintSignatureRef: string | null
  canComposeResponse: boolean
  composedResponseText: string | null
  composedResponseDigest: string | null
  composedResponseRef: string | null
  responseSegmentRefs: string[]
  incompleteSubQueryRefs: string[]
  missingResultSubQueryRefs: string[]
  missingResponseTextSubQueryRefs: string[]
  orderedSubQueryRefs: string[]
  evidenceRefs: string[]
  blockerRefs: FrlmResponseCompositionBlockerRef[]
  authorityBoundary: string
  contentRedacted: true
}

export type FrlmRlmTraceStep = {
  stepRef: string
  stepIndex: number
  kind: FrlmRlmTraceStepKind
  state: FrlmConductorSubQueryState | FrlmConductorExecutionMode | "emitted"
  parentRef: string | null
  subQueryRef: string | null
  blueprintSignatureRef: string | null
  evidenceRefs: string[]
}

export type FrlmRlmStepTraceProjection = {
  schema: typeof FRLM_RLM_STEP_TRACE_SCHEMA
  observedAt: string
  traceRef: string | null
  traceDigestRef: string | null
  executionRef: string | null
  rootTaskRef: string | null
  blueprintSignatureRef: string | null
  executionMode: FrlmConductorExecutionMode
  stepCount: number
  steps: FrlmRlmTraceStep[]
  evidenceRefs: string[]
  blockerRefs: FrlmConductorExecutionBlockerRef[]
  externalDependencyRefs: string[]
  authorityBoundary: string
  contentRedacted: true
}

export type FrlmConductorScheduleState =
  | "recursive_fanout_ready"
  | "linear_fallback_ready"
  | "waiting_for_recursive_results"
  | "completed"
  | "blocked"

export type FrlmConductorRecursiveBatch = {
  batchRef: string
  submitterRef: string
  subQueryRefs: string[]
}

export type FrlmConductorLinearFallbackStep = {
  stepRef: string
  executorRef: string
  subQueryRef: string
}

export type FrlmConductorScheduleBudget = {
  maxSubQueries?: number
  maxLinearFallbackSteps?: number
}

export type FrlmConductorScheduleInput = {
  observedAt: string
  executionRef?: string | null
  rootTaskRef?: string | null
  blueprintSignatureRef?: string | null
  subQueries?: FrlmConductorSubQuery[]
  linearFallbackEnabled?: boolean
  linearExecutorRef?: string | null
  recursiveSubmitterRef?: string | null
  maxParallelSubQueries?: number
  budget?: FrlmConductorScheduleBudget
}

export type FrlmConductorSchedule = {
  schema: "openagents.artanis.frlm_conductor_schedule.v0.1"
  observedAt: string
  scheduleRef: string | null
  state: FrlmConductorScheduleState
  canSchedule: boolean
  projection: FrlmConductorExecutionProjection
  recursiveBatches: FrlmConductorRecursiveBatch[]
  linearFallbackSteps: FrlmConductorLinearFallbackStep[]
  nextActionRef: string | null
  traceRefs: string[]
  blockerRefs: FrlmConductorSchedulerBlockerRef[]
  authorityBoundary: string
  contentRedacted: true
}

export type FrlmConductorOptions = {
  recursiveSubmitterRef?: string | null
  linearFallbackEnabled?: boolean
  linearExecutorRef?: string | null
  maxParallelSubQueries?: number
  budget?: FrlmConductorScheduleBudget
}

const publicRefPattern = /^[a-z][a-z0-9._:/-]{1,220}$/i
const unsafeRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i
const unsafeResponseTextPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer\s+[a-z0-9._-]{6,}|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_ -]?(hash|preimage)|preimage|private key|raw prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const failedStates = new Set<FrlmConductorSubQueryState>([
  "failed",
  "timed_out",
  "rejected",
])

const incompleteStates = new Set<FrlmConductorSubQueryState>([
  "planned",
  "running",
])

export class FrlmConductor {
  readonly #options: FrlmConductorOptions

  constructor(options: FrlmConductorOptions = {}) {
    this.#options = { ...options }
  }

  schedule(input: FrlmConductorScheduleInput): FrlmConductorSchedule {
    return scheduleFrlmConductor({
      ...this.#options,
      ...input,
      budget: {
        ...this.#options.budget,
        ...input.budget,
      },
    })
  }
}

export function scheduleFrlmConductor(
  input: FrlmConductorScheduleInput,
): FrlmConductorSchedule {
  const projection = planFrlmConductorExecution(input)
  const blockerRefs = new Set<FrlmConductorSchedulerBlockerRef>(
    projection.blockerRefs,
  )
  const maxParallelSubQueries = input.maxParallelSubQueries ?? 1
  const recursiveSubmitterRef = safeScheduleRef(input.recursiveSubmitterRef)
  const plannedSubQueryRefs = uniqueRefs(
    (input.subQueries ?? [])
      .filter((subQuery) => subQuery.state === "planned")
      .map((subQuery) => safeScheduleRef(subQuery.subQueryRef)),
  )
  const incompleteSubQueryRefs = uniqueRefs(
    (input.subQueries ?? [])
      .filter((subQuery) => incompleteStates.has(subQuery.state))
      .map((subQuery) => safeScheduleRef(subQuery.subQueryRef)),
  )

  if (!Number.isInteger(maxParallelSubQueries) || maxParallelSubQueries <= 0) {
    blockerRefs.add("blocker.artanis.frlm_conductor.max_parallelism_invalid")
  }
  if (plannedSubQueryRefs.length > 0 && recursiveSubmitterRef === null) {
    blockerRefs.add("blocker.artanis.frlm_conductor.recursive_submitter_ref_missing")
  }

  const maxSubQueries = input.budget?.maxSubQueries
  if (
    typeof maxSubQueries === "number" &&
    maxSubQueries >= 0 &&
    projection.recursiveSubQueryRefs.length > maxSubQueries
  ) {
    blockerRefs.add("blocker.artanis.frlm_conductor.sub_query_budget_exceeded")
  }

  const maxLinearFallbackSteps = input.budget?.maxLinearFallbackSteps
  if (
    typeof maxLinearFallbackSteps === "number" &&
    maxLinearFallbackSteps >= 0 &&
    projection.linearFallbackStepRefs.length > maxLinearFallbackSteps
  ) {
    blockerRefs.add("blocker.artanis.frlm_conductor.linear_fallback_budget_exceeded")
  }

  const canSchedule = blockerRefs.size === 0
  const recursiveBatches =
    canSchedule && plannedSubQueryRefs.length > 0 && recursiveSubmitterRef !== null
      ? chunkRefs(plannedSubQueryRefs, maxParallelSubQueries).map((subQueryRefs, index) => ({
        batchRef: `batch.artanis.frlm.recursive.${stableHash(`${projection.executionRef ?? "unknown"}:${index}:${subQueryRefs.join(":")}`)}`,
        submitterRef: recursiveSubmitterRef,
        subQueryRefs,
      }))
      : []
  const linearFallbackSteps =
    canSchedule && projection.executionMode === "fallback_linear" && projection.linearExecutorRef !== null
      ? projection.recursiveSubQueryRefs.map((subQueryRef, index) => ({
        stepRef: projection.linearFallbackStepRefs[index] ??
          `step.artanis.frlm.linear.${stableHash(`${projection.rootTaskRef ?? "unknown"}:${index}:${subQueryRef}`)}`,
        executorRef: projection.linearExecutorRef as string,
        subQueryRef,
      }))
      : []
  const state: FrlmConductorScheduleState =
    !canSchedule
      ? "blocked"
      : projection.executionMode === "fallback_linear"
        ? "linear_fallback_ready"
        : recursiveBatches.length > 0
          ? "recursive_fanout_ready"
          : incompleteSubQueryRefs.length > 0
            ? "waiting_for_recursive_results"
            : "completed"
  const scheduleRef =
    canSchedule && projection.executionRef !== null
      ? `schedule.artanis.frlm.${stableHash(`${projection.executionRef}:${state}:${projection.evidenceRefs.join(":")}`)}`
      : null
  const nextActionRef =
    state === "recursive_fanout_ready"
      ? recursiveBatches[0]?.batchRef ?? null
      : state === "linear_fallback_ready"
        ? linearFallbackSteps[0]?.stepRef ?? null
        : state === "completed"
          ? projection.executionPlanRef
          : null
  const traceRefs = uniqueRefs([
    scheduleRef,
    nextActionRef,
    ...recursiveBatches.flatMap((batch) => [
      batch.batchRef,
      batch.submitterRef,
      ...batch.subQueryRefs,
    ]),
    ...linearFallbackSteps.flatMap((step) => [
      step.stepRef,
      step.executorRef,
      step.subQueryRef,
    ]),
    ...projection.evidenceRefs,
  ])

  const schedule: FrlmConductorSchedule = {
    schema: "openagents.artanis.frlm_conductor_schedule.v0.1",
    observedAt: input.observedAt,
    scheduleRef,
    state,
    canSchedule,
    projection,
    recursiveBatches,
    linearFallbackSteps,
    nextActionRef,
    traceRefs,
    blockerRefs: [...blockerRefs].sort(),
    authorityBoundary:
      "Read-only FRLM Conductor scheduler. It computes recursive fanout batches and local linear fallback steps from public evidence refs only; it does not dispatch workers, run Python, spend sats, publish claims, or move settlement authority.",
    contentRedacted: true,
  }
  assertPublicProjectionSafe(schedule)
  return schedule
}

export function planFrlmConductorExecution(input: {
  observedAt: string
  executionRef?: string | null
  rootTaskRef?: string | null
  blueprintSignatureRef?: string | null
  budgetPolicy?: FrlmConductorBudgetPolicy | null
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
    normalizedRef(input.budgetPolicy?.budgetPolicyRef),
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
  const budgetPolicyRef = safeRef(input.budgetPolicy?.budgetPolicyRef)
  const tokenBudget = positiveInteger(input.budgetPolicy?.maxTokens)
  const depthLimit = positiveInteger(input.budgetPolicy?.maxDepth)
  const linearExecutorRef = safeRef(input.linearExecutorRef)
  const recursiveSubQueryRefs = uniqueRefs(subQueries.map((subQuery) => safeRef(subQuery.subQueryRef)))
  const projectedTokenCount = subQueries.reduce(
    (total, subQuery) => total + nonNegativeInteger(subQuery.tokenCount),
    0,
  )
  const projectedMaxDepth = maxProjectedDepth(rootTaskRef, subQueries)
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
  if (budgetPolicyRef === null) {
    blockerRefs.add("blocker.artanis.frlm_conductor.budget_policy_ref_missing")
  }
  if (input.budgetPolicy === null || input.budgetPolicy === undefined || tokenBudget === null || depthLimit === null) {
    blockerRefs.add("blocker.artanis.frlm_conductor.budget_policy_invalid")
  }
  if (tokenBudget !== null && projectedTokenCount > tokenBudget) {
    blockerRefs.add("blocker.artanis.frlm_conductor.token_budget_exceeded")
  }
  if (depthLimit !== null && projectedMaxDepth > depthLimit) {
    blockerRefs.add("blocker.artanis.frlm_conductor.depth_limit_exceeded")
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
    budgetPolicyRef,
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
    budgetPolicyRef,
    tokenBudget,
    projectedTokenCount,
    depthLimit,
    projectedMaxDepth,
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

export function composeFrlmRecursiveResponse(input: {
  observedAt: string
  compositionRef?: string | null
  rootTaskRef?: string | null
  executionPlan?: FrlmConductorExecutionProjection | null
  executionPlanRef?: string | null
  blueprintSignatureRef?: string | null
  responseBlueprintSignatureRef?: string | null
  segments?: FrlmResponseCompositionSegment[]
}): FrlmResponseCompositionProjection {
  const blockerRefs = new Set<FrlmResponseCompositionBlockerRef>()
  const segments = input.segments ?? []
  const rawRefs = [
    normalizedRef(input.compositionRef),
    normalizedRef(input.rootTaskRef),
    normalizedRef(input.executionPlanRef),
    normalizedRef(input.executionPlan?.executionPlanRef),
    normalizedRef(input.blueprintSignatureRef),
    normalizedRef(input.executionPlan?.blueprintSignatureRef),
    normalizedRef(input.responseBlueprintSignatureRef),
    ...segments.flatMap((segment) => [
      normalizedRef(segment.subQueryRef),
      normalizedRef(segment.parentRef),
      normalizedRef(segment.resultRef),
      normalizedRef(segment.responseRef),
      normalizedRef(segment.blueprintSignatureRef),
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

  const compositionRef = safeRef(input.compositionRef)
  const rootTaskRef = safeRef(input.rootTaskRef) ?? input.executionPlan?.rootTaskRef ?? null
  const executionPlanRef = safeRef(input.executionPlanRef) ?? input.executionPlan?.executionPlanRef ?? null
  const blueprintSignatureRef = safeRef(input.blueprintSignatureRef) ?? input.executionPlan?.blueprintSignatureRef ?? null
  const responseBlueprintSignatureRef = safeRef(input.responseBlueprintSignatureRef)
  const orderedSegments = [...segments].sort(compareResponseSegments)
  const orderedSubQueryRefs = uniqueRefsPreservingOrder(orderedSegments.map((segment) => safeRef(segment.subQueryRef)))
  const incompleteSubQueryRefs = uniqueRefsPreservingOrder(
    orderedSegments
      .filter((segment) => segment.state !== "completed")
      .map((segment) => safeRef(segment.subQueryRef)),
  )
  const missingResultSubQueryRefs = uniqueRefsPreservingOrder(
    orderedSegments
      .filter((segment) => safeRef(segment.resultRef) === null)
      .map((segment) => safeRef(segment.subQueryRef)),
  )
  const missingResponseTextSubQueryRefs = uniqueRefsPreservingOrder(
    orderedSegments
      .filter((segment) => normalizedResponseText(segment.responseText) === null)
      .map((segment) => safeRef(segment.subQueryRef)),
  )
  const responseTextUnsafe = orderedSegments.some((segment) => {
    const text = normalizedResponseText(segment.responseText)
    return text !== null && unsafeResponseTextPattern.test(text)
  })
  const responseSegmentRefs = uniqueRefsPreservingOrder(
    orderedSegments.map((segment) => safeRef(segment.responseRef) ?? safeRef(segment.resultRef)),
  )

  if (compositionRef === null) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.composition_ref_missing")
  }
  if (rootTaskRef === null) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.root_task_ref_missing")
  }
  if (executionPlanRef === null) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.execution_plan_ref_missing")
  }
  if (blueprintSignatureRef === null) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.blueprint_signature_ref_missing")
  }
  if (responseBlueprintSignatureRef === null) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.response_blueprint_signature_ref_missing")
  }
  if (input.executionPlan !== null && input.executionPlan !== undefined && input.executionPlan.canExecute !== true) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.execution_plan_blocked")
  }
  if (orderedSegments.length === 0 || responseSegmentRefs.length === 0) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.response_segment_missing")
  }
  if (incompleteSubQueryRefs.length > 0) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.sub_query_incomplete")
  }
  if (missingResultSubQueryRefs.length > 0) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.sub_query_result_missing")
  }
  if (missingResponseTextSubQueryRefs.length > 0) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.sub_query_response_text_missing")
  }
  if (responseTextUnsafe) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.unsafe_content")
  }
  if (rawRefs.some((ref) => ref !== null && !isSafeRef(ref))) {
    blockerRefs.add("blocker.artanis.frlm_response_composition.unsafe_ref")
  }

  const canComposeResponse = blockerRefs.size === 0
  const composedResponseText = canComposeResponse
    ? orderedSegments
      .map((segment, index) => `[${index + 1}] ${normalizedResponseText(segment.responseText)}`)
      .join("\n\n")
    : null
  const composedResponseDigest = composedResponseText === null ? null : stableHash(composedResponseText, 32)
  const composedResponseRef =
    composedResponseDigest === null
      ? null
      : `response.artanis.frlm.composed.${composedResponseDigest.slice(0, 20)}`
  const evidenceRefs = uniqueRefs([
    compositionRef,
    rootTaskRef,
    executionPlanRef,
    blueprintSignatureRef,
    responseBlueprintSignatureRef,
    composedResponseRef,
    ...responseSegmentRefs,
    ...orderedSubQueryRefs,
    ...incompleteSubQueryRefs,
    ...missingResultSubQueryRefs,
    ...missingResponseTextSubQueryRefs,
    ...orderedSegments.flatMap((segment) => [
      safeRef(segment.parentRef),
      safeRef(segment.resultRef),
      safeRef(segment.responseRef),
      safeRef(segment.blueprintSignatureRef),
    ]),
    ...(input.executionPlan?.evidenceRefs ?? []),
  ])

  const projection: FrlmResponseCompositionProjection = {
    schema: FRLM_RESPONSE_COMPOSITION_SCHEMA,
    observedAt: input.observedAt,
    compositionRef,
    rootTaskRef,
    executionPlanRef,
    blueprintSignatureRef,
    responseBlueprintSignatureRef,
    canComposeResponse,
    composedResponseText,
    composedResponseDigest,
    composedResponseRef,
    responseSegmentRefs,
    incompleteSubQueryRefs,
    missingResultSubQueryRefs,
    missingResponseTextSubQueryRefs,
    orderedSubQueryRefs,
    evidenceRefs,
    blockerRefs: [...blockerRefs].sort(),
    authorityBoundary:
      "Read-only FRLM recursive response composition projection. It orders completed RLM sub-query outputs under Blueprint signatures and emits deterministic response evidence; it does not run Python, issue model calls, dispatch workers, spend sats, publish public claims, or move settlement authority.",
    contentRedacted: true,
  }
  assertPublicProjectionSafe(projection)
  return projection
}

export function emitFrlmRlmStepTrace(input: {
  observedAt: string
  executionRef?: string | null
  rootTaskRef?: string | null
  blueprintSignatureRef?: string | null
  subQueries?: FrlmConductorSubQuery[]
  linearFallbackEnabled?: boolean
  linearExecutorRef?: string | null
}): FrlmRlmStepTraceProjection {
  const execution = planFrlmConductorExecution(input)
  const subQueriesByRef = new Map(
    (input.subQueries ?? [])
      .map((subQuery) => [normalizedRef(subQuery.subQueryRef), subQuery] as const)
      .filter((entry): entry is readonly [string, FrlmConductorSubQuery] => entry[0] !== null),
  )
  const safeRef = (value: string | null | undefined) => {
    const ref = normalizedRef(value)
    return ref !== null && isSafeRef(ref) ? ref : null
  }
  const steps: FrlmRlmTraceStep[] = []
  const pushStep = (step: Omit<FrlmRlmTraceStep, "stepIndex">) => {
    steps.push({ ...step, stepIndex: steps.length })
  }

  if (execution.executionRef !== null && execution.rootTaskRef !== null) {
    pushStep({
      stepRef: `step.artanis.frlm.rlm.blueprint_gate.${stableHash(`${execution.executionRef}:blueprint_gate`)}`,
      kind: "blueprint_gate",
      state: execution.executionMode,
      parentRef: execution.rootTaskRef,
      subQueryRef: null,
      blueprintSignatureRef: execution.blueprintSignatureRef,
      evidenceRefs: uniqueRefs([
        execution.executionRef,
        execution.rootTaskRef,
        execution.blueprintSignatureRef,
        execution.executionPlanRef,
      ]),
    })
  }

  for (const subQueryRef of execution.recursiveSubQueryRefs) {
    const subQuery = subQueriesByRef.get(subQueryRef)
    pushStep({
      stepRef: `step.artanis.frlm.rlm.sub_query.${stableHash(`${execution.executionRef ?? "unknown"}:${subQueryRef}`)}`,
      kind: "recursive_sub_query",
      state: subQuery?.state ?? "planned",
      parentRef: safeRef(subQuery?.parentRef) ?? execution.rootTaskRef,
      subQueryRef,
      blueprintSignatureRef: safeRef(subQuery?.blueprintSignatureRef) ?? execution.blueprintSignatureRef,
      evidenceRefs: uniqueRefs([
        subQueryRef,
        safeRef(subQuery?.parentRef),
        safeRef(subQuery?.resultRef),
        safeRef(subQuery?.failureRef),
        safeRef(subQuery?.blueprintSignatureRef),
      ]),
    })
  }

  if (execution.executionMode === "fallback_linear") {
    execution.linearFallbackStepRefs.forEach((stepRef, index) => {
      const subQueryRef = execution.recursiveSubQueryRefs[index] ?? null
      pushStep({
        stepRef,
        kind: "linear_fallback",
        state: "planned",
        parentRef: subQueryRef ?? execution.rootTaskRef,
        subQueryRef,
        blueprintSignatureRef: execution.blueprintSignatureRef,
        evidenceRefs: uniqueRefs([
          stepRef,
          execution.linearExecutorRef,
          execution.fallbackReasonRef,
          subQueryRef,
        ]),
      })
    })
  }

  if (execution.canExecute && execution.executionPlanRef !== null) {
    pushStep({
      stepRef: `step.artanis.frlm.rlm.result_synthesis.${stableHash(execution.executionPlanRef)}`,
      kind: "result_synthesis",
      state: "emitted",
      parentRef: execution.executionPlanRef,
      subQueryRef: null,
      blueprintSignatureRef: execution.blueprintSignatureRef,
      evidenceRefs: uniqueRefs([
        execution.executionPlanRef,
        execution.fallbackReasonRef,
        ...execution.recursiveSubQueryRefs,
        ...execution.failedSubQueryRefs,
      ]),
    })
  }

  const evidenceRefs = uniqueRefs([
    ...execution.evidenceRefs,
    ...steps.flatMap((step) => [step.stepRef, ...step.evidenceRefs]),
  ])
  const traceDigestRef = steps.length === 0
    ? null
    : `trace.artanis.frlm.rlm.digest.${stableHash(JSON.stringify(steps))}`
  const traceRef = traceDigestRef === null
    ? null
    : `trace.artanis.frlm.rlm.${stableHash(`${execution.executionRef ?? "unknown"}:${traceDigestRef}`)}`
  const projection: FrlmRlmStepTraceProjection = {
    schema: FRLM_RLM_STEP_TRACE_SCHEMA,
    observedAt: input.observedAt,
    traceRef,
    traceDigestRef,
    executionRef: execution.executionRef,
    rootTaskRef: execution.rootTaskRef,
    blueprintSignatureRef: execution.blueprintSignatureRef,
    executionMode: execution.executionMode,
    stepCount: steps.length,
    steps,
    evidenceRefs: uniqueRefs([traceRef, traceDigestRef, ...evidenceRefs]),
    blockerRefs: execution.blockerRefs,
    externalDependencyRefs: [
      "external.rlm.repl_leaf_executor",
      "external.blueprint.signature_lookup",
      "external.nip90.sub_query_fanout",
    ],
    authorityBoundary:
      "Read-only structured RLM step trace for FRLM Conductor planning. It emits public-safe step refs, states, Blueprint signature refs, and evidence digests only; it does not include prompts, REPL code, model outputs, local paths, credentials, dispatch authority, spend authority, or settlement authority.",
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

function normalizedResponseText(value: string | null | undefined): string | null {
  if (value === null || value === undefined) return null
  const trimmed = value.trim().replace(/\s+/g, " ")
  return trimmed.length === 0 ? null : trimmed
}

function compareResponseSegments(a: FrlmResponseCompositionSegment, b: FrlmResponseCompositionSegment) {
  const orderA = typeof a.order === "number" && Number.isFinite(a.order) ? a.order : Number.POSITIVE_INFINITY
  const orderB = typeof b.order === "number" && Number.isFinite(b.order) ? b.order : Number.POSITIVE_INFINITY
  if (orderA !== orderB) return orderA - orderB
  return a.subQueryRef.localeCompare(b.subQueryRef)
}

function safeScheduleRef(value: string | null | undefined): string | null {
  const ref = normalizedRef(value)
  return ref === null || !isSafeRef(ref) ? null : ref
}

function stableHash(input: string, length = 20) {
  return createHash("sha256").update(input).digest("hex").slice(0, length)
}

function uniqueRefs(refs: (string | null)[]) {
  return [...new Set(refs.filter((ref): ref is string => ref !== null && ref.trim().length > 0))]
    .map((ref) => ref.trim())
    .sort()
}

function positiveInteger(value: number | null | undefined): number | null {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : null
}

function nonNegativeInteger(value: number | null | undefined): number {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : 0
}

function maxProjectedDepth(rootTaskRef: string | null, subQueries: FrlmConductorSubQuery[]) {
  const parentByRef = new Map<string, string | null>()
  for (const subQuery of subQueries) {
    const subQueryRef = normalizedRef(subQuery.subQueryRef)
    if (subQueryRef !== null) {
      parentByRef.set(subQueryRef, normalizedRef(subQuery.parentRef))
    }
  }

  let maxDepth = 0
  for (const subQuery of subQueries) {
    const explicitDepth = positiveInteger(subQuery.depth)
    if (explicitDepth !== null) {
      maxDepth = Math.max(maxDepth, explicitDepth)
      continue
    }

    const subQueryRef = normalizedRef(subQuery.subQueryRef)
    if (subQueryRef !== null) {
      maxDepth = Math.max(maxDepth, inferredDepth(rootTaskRef, subQueryRef, parentByRef))
    }
  }
  return maxDepth
}

function inferredDepth(rootTaskRef: string | null, subQueryRef: string, parentByRef: Map<string, string | null>) {
  let depth = 1
  let currentRef: string | null = subQueryRef
  const seen = new Set<string>()

  while (currentRef !== null && !seen.has(currentRef)) {
    seen.add(currentRef)
    const parentRef: string | null = parentByRef.get(currentRef) ?? null
    if (parentRef === null || parentRef === rootTaskRef) {
      return depth
    }
    depth += 1
    currentRef = parentByRef.has(parentRef) ? parentRef : null
  }

  return depth
}

function uniqueRefsPreservingOrder(refs: (string | null)[]) {
  const out: string[] = []
  for (const ref of refs) {
    const normalized = normalizedRef(ref)
    if (normalized !== null && !out.includes(normalized)) {
      out.push(normalized)
    }
  }
  return out
}

function chunkRefs(refs: string[], size: number): string[][] {
  const chunks: string[][] = []
  for (let index = 0; index < refs.length; index += size) {
    chunks.push(refs.slice(index, index + size))
  }
  return chunks
}
