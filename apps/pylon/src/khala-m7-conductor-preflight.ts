import { assertPublicProjectionSafe } from "./state.js"
import type { KhalaM6ShadowPreflightProjection } from "./khala-m6-shadow-preflight.js"

export const KHALA_M7_CONDUCTOR_PREFLIGHT_SCHEMA =
  "openagents.khala.m7.conductor_preflight.v0.1"

export type KhalaM7ConductorPreflightBlockerRef =
  | "blocker.khala.m7.conductor_preflight.owner_confirmation_missing"
  | "blocker.khala.m7.conductor_preflight.owner_approval_ref_missing"
  | "blocker.khala.m7.conductor_preflight.spend_cap_missing"
  | "blocker.khala.m7.conductor_preflight.spend_cap_exceeded"
  | "blocker.khala.m7.conductor_preflight.m6_shadow_win_missing"
  | "blocker.khala.m7.conductor_preflight.policy_backend_missing"
  | "blocker.khala.m7.conductor_preflight.policy_backend_not_wired"
  | "blocker.khala.m7.conductor_preflight.training_run_missing"
  | "blocker.khala.m7.conductor_preflight.training_run_not_executed"
  | "blocker.khala.m7.conductor_preflight.paid_verdict_source_missing"
  | "blocker.khala.m7.conductor_preflight.paid_verdict_source_not_armed"
  | "blocker.khala.m7.conductor_preflight.verse_fanout_missing"
  | "blocker.khala.m7.conductor_preflight.crossy_road_composition_missing"
  | "blocker.khala.m7.conductor_preflight.publication_ref_missing"
  | "blocker.khala.m7.conductor_preflight.unsafe_ref"

export type KhalaM7ConductorPreflightProjection = {
  schema: typeof KHALA_M7_CONDUCTOR_PREFLIGHT_SCHEMA
  observedAt: string
  canStartConductorTraining: boolean
  canPublishM7Claim: boolean
  ownerApprovalRef: string | null
  dailySpendCapMsats: number | null
  plannedTrainingSpendMsats: number | null
  m6ShadowPreflightRef: string | null
  m6PaidShadowWinRef: string | null
  policyBackendRef: string | null
  trainingRunRef: string | null
  paidVerdictSourceRef: string | null
  verseFanoutRef: string | null
  crossyRoadCompositionRef: string | null
  publicationRef: string | null
  evidenceRefs: string[]
  blockerRefs: KhalaM7ConductorPreflightBlockerRef[]
  externalDependencyRefs: string[]
  authorityBoundary: string
  contentRedacted: true
}

const publicRefPattern = /^[a-z][a-z0-9._:/-]{1,220}$/i
const unsafeRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const externalDependencyRefs = [
  "external.psionic.m7.conductor_policy_backend",
  "external.psionic.m7.grpo_training_run",
  "external.openagents.khala.live_paid_verdict_source",
  "external.openagents.verse.multi_worker_fanout",
  "external.openagents.khala.m6_paid_shadow_win",
]

export function preflightKhalaM7Conductor(input: {
  observedAt: string
  ownerConfirmed?: boolean
  ownerApprovalRef?: string | null
  dailySpendCapMsats?: number | null
  plannedTrainingSpendMsats?: number | null
  m6ShadowPreflight?: KhalaM6ShadowPreflightProjection | null
  policyBackendRef?: string | null
  policyBackendWired?: boolean
  trainingRunRef?: string | null
  trainingRunExecuted?: boolean
  paidVerdictSourceRef?: string | null
  paidVerdictSourceArmed?: boolean
  verseFanoutRef?: string | null
  crossyRoadCompositionRef?: string | null
  crossyRoadCompositionVerified?: boolean
  publicationRef?: string | null
}): KhalaM7ConductorPreflightProjection {
  const blockerRefs = new Set<KhalaM7ConductorPreflightBlockerRef>()
  const rawRefs = [
    normalizedRef(input.ownerApprovalRef),
    normalizedRef(input.policyBackendRef),
    normalizedRef(input.trainingRunRef),
    normalizedRef(input.paidVerdictSourceRef),
    normalizedRef(input.verseFanoutRef),
    normalizedRef(input.crossyRoadCompositionRef),
    normalizedRef(input.publicationRef),
  ]
  const [
    ownerApprovalRef,
    policyBackendRef,
    trainingRunRef,
    paidVerdictSourceRef,
    verseFanoutRef,
    crossyRoadCompositionRef,
    publicationRef,
  ] = rawRefs.map((ref) => (ref !== null && isSafeRef(ref) ? ref : null))
  const dailySpendCapMsats =
    typeof input.dailySpendCapMsats === "number" && Number.isFinite(input.dailySpendCapMsats)
      ? Math.trunc(input.dailySpendCapMsats)
      : null
  const plannedTrainingSpendMsats =
    typeof input.plannedTrainingSpendMsats === "number" && Number.isFinite(input.plannedTrainingSpendMsats)
      ? Math.trunc(input.plannedTrainingSpendMsats)
      : null
  const m6ShadowPreflightRef =
    input.m6ShadowPreflight?.canPublishM6Claim === true
      ? "preflight.khala.m6.shadow_run.publishable.v0_1"
      : null
  const m6PaidShadowWinRef = input.m6ShadowPreflight?.paidShadowWinRef ?? null

  if (input.ownerConfirmed !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.owner_confirmation_missing")
  }
  if (ownerApprovalRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.owner_approval_ref_missing")
  }
  if (dailySpendCapMsats === null || dailySpendCapMsats <= 0 || plannedTrainingSpendMsats === null || plannedTrainingSpendMsats <= 0) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.spend_cap_missing")
  } else if (plannedTrainingSpendMsats > dailySpendCapMsats) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.spend_cap_exceeded")
  }
  if (m6ShadowPreflightRef === null || m6PaidShadowWinRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.m6_shadow_win_missing")
  }
  if (policyBackendRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.policy_backend_missing")
  }
  if (input.policyBackendWired !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.policy_backend_not_wired")
  }
  if (trainingRunRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.training_run_missing")
  }
  if (input.trainingRunExecuted !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.training_run_not_executed")
  }
  if (paidVerdictSourceRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.paid_verdict_source_missing")
  }
  if (input.paidVerdictSourceArmed !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.paid_verdict_source_not_armed")
  }
  if (verseFanoutRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.verse_fanout_missing")
  }
  if (crossyRoadCompositionRef === null || input.crossyRoadCompositionVerified !== true) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.crossy_road_composition_missing")
  }
  if (publicationRef === null) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.publication_ref_missing")
  }

  const allRefs = [
    ownerApprovalRef,
    m6ShadowPreflightRef,
    m6PaidShadowWinRef,
    policyBackendRef,
    trainingRunRef,
    paidVerdictSourceRef,
    verseFanoutRef,
    crossyRoadCompositionRef,
    publicationRef,
    ...(input.m6ShadowPreflight?.evidenceRefs ?? []),
  ]
  if (
    rawRefs.some((ref) => ref !== null && !isSafeRef(ref)) ||
    allRefs.some((ref) => ref !== null && !isSafeRef(ref))
  ) {
    blockerRefs.add("blocker.khala.m7.conductor_preflight.unsafe_ref")
  }

  const canStartConductorTraining = [...blockerRefs].every(
    (ref) =>
      ref === "blocker.khala.m7.conductor_preflight.training_run_missing" ||
      ref === "blocker.khala.m7.conductor_preflight.training_run_not_executed" ||
      ref === "blocker.khala.m7.conductor_preflight.verse_fanout_missing" ||
      ref === "blocker.khala.m7.conductor_preflight.crossy_road_composition_missing" ||
      ref === "blocker.khala.m7.conductor_preflight.publication_ref_missing",
  )
  const canPublishM7Claim = blockerRefs.size === 0

  const projection: KhalaM7ConductorPreflightProjection = {
    schema: KHALA_M7_CONDUCTOR_PREFLIGHT_SCHEMA,
    observedAt: input.observedAt,
    canStartConductorTraining,
    canPublishM7Claim,
    ownerApprovalRef,
    dailySpendCapMsats,
    plannedTrainingSpendMsats,
    m6ShadowPreflightRef,
    m6PaidShadowWinRef,
    policyBackendRef,
    trainingRunRef,
    paidVerdictSourceRef,
    verseFanoutRef,
    crossyRoadCompositionRef,
    publicationRef,
    evidenceRefs: uniqueRefs(allRefs),
    blockerRefs: [...blockerRefs].sort(),
    externalDependencyRefs,
    authorityBoundary:
      "Read-only M7 Conductor readiness projection. It does not run GRPO, serve a 7B policy, dispatch worker calls, spend sats, publish benchmark claims, promote runtime artifacts, or move settlement authority.",
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

function uniqueRefs(refs: (string | null)[]) {
  return [...new Set(refs.filter((ref): ref is string => ref !== null && ref.trim().length > 0))]
    .map((ref) => ref.trim())
    .sort()
}
