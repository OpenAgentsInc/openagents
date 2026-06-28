import { createHash } from "node:crypto"
import { assertPublicProjectionSafe } from "./state.js"

export const FRLM_CONDUCTOR_EXECUTION_SCHEMA =
  "openagents.artanis.frlm_conductor_execution.v0.1"

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
