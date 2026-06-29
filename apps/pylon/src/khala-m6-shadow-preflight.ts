import { assertPublicProjectionSafe } from "./state.js"
import type { PsionicTrainingBoundaryProjection } from "./psionic-training-boundary.js"
import type { PylonRealServingReadinessPreflight } from "./serving-capability.js"

export const KHALA_M6_SHADOW_PREFLIGHT_SCHEMA =
  "openagents.khala.m6.shadow_run_preflight.v0.1"

export type KhalaM6ShadowPreflightBlockerRef =
  | "blocker.khala.m6.shadow_preflight.owner_confirmation_missing"
  | "blocker.khala.m6.shadow_preflight.owner_approval_ref_missing"
  | "blocker.khala.m6.shadow_preflight.spend_cap_missing"
  | "blocker.khala.m6.shadow_preflight.spend_cap_exceeded"
  | "blocker.khala.m6.shadow_preflight.psionic_training_boundary_not_ready"
  | "blocker.khala.m6.shadow_preflight.pylon_serving_preflight_not_ready"
  | "blocker.khala.m6.shadow_preflight.verdict_source_missing"
  | "blocker.khala.m6.shadow_preflight.verdict_source_not_armed"
  | "blocker.khala.m6.shadow_preflight.shadow_candidate_missing"
  | "blocker.khala.m6.shadow_preflight.baseline_router_missing"
  | "blocker.khala.m6.shadow_preflight.live_rollout_missing"
  | "blocker.khala.m6.shadow_preflight.paid_shadow_win_missing"
  | "blocker.khala.m6.shadow_preflight.publication_ref_missing"
  | "blocker.khala.m6.shadow_preflight.unsafe_ref"

export type KhalaM6ShadowPreflightProjection = {
  schema: typeof KHALA_M6_SHADOW_PREFLIGHT_SCHEMA
  observedAt: string
  canStartShadowRun: boolean
  canPublishM6Claim: boolean
  ownerApprovalRef: string | null
  dailySpendCapMsats: number | null
  plannedShadowSpendMsats: number | null
  psionicTrainingBoundaryRef: string | null
  pylonServingPreflightRef: string | null
  verdictSourceRef: string | null
  shadowCandidateRef: string | null
  baselineRouterRef: string | null
  liveRolloutRef: string | null
  paidShadowWinRef: string | null
  publicationRef: string | null
  evidenceRefs: string[]
  blockerRefs: KhalaM6ShadowPreflightBlockerRef[]
  externalDependencyRefs: string[]
  authorityBoundary: string
  contentRedacted: true
}

const publicRefPattern = /^[a-z][a-z0-9._:/-]{1,220}$/i
const unsafeRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const externalDependencyRefs = [
  "external.psionic.m6.live_training_driver",
  "external.psionic.m6.shadow_candidate_contract",
  "external.openagents.khala.live_verdict_source",
  "external.pylon.real_serving_preflight",
]

export function preflightKhalaM6ShadowRun(input: {
  observedAt: string
  ownerConfirmed?: boolean
  ownerApprovalRef?: string | null
  dailySpendCapMsats?: number | null
  plannedShadowSpendMsats?: number | null
  psionicTrainingBoundary?: PsionicTrainingBoundaryProjection | null
  pylonServingPreflight?: PylonRealServingReadinessPreflight | null
  verdictSourceRef?: string | null
  verdictSourceArmed?: boolean
  shadowCandidateRef?: string | null
  baselineRouterRef?: string | null
  liveRolloutRef?: string | null
  paidShadowWinRef?: string | null
  publicationRef?: string | null
}): KhalaM6ShadowPreflightProjection {
  const blockerRefs = new Set<KhalaM6ShadowPreflightBlockerRef>()
  const rawRefs = [
    normalizedRef(input.ownerApprovalRef),
    normalizedRef(input.verdictSourceRef),
    normalizedRef(input.shadowCandidateRef),
    normalizedRef(input.baselineRouterRef),
    normalizedRef(input.liveRolloutRef),
    normalizedRef(input.paidShadowWinRef),
    normalizedRef(input.publicationRef),
  ]
  const [
    ownerApprovalRef,
    verdictSourceRef,
    shadowCandidateRef,
    baselineRouterRef,
    liveRolloutRef,
    paidShadowWinRef,
    publicationRef,
  ] = rawRefs.map((ref) => (ref !== null && isSafeRef(ref) ? ref : null))
  const dailySpendCapMsats =
    typeof input.dailySpendCapMsats === "number" && Number.isFinite(input.dailySpendCapMsats)
      ? Math.trunc(input.dailySpendCapMsats)
      : null
  const plannedShadowSpendMsats =
    typeof input.plannedShadowSpendMsats === "number" && Number.isFinite(input.plannedShadowSpendMsats)
      ? Math.trunc(input.plannedShadowSpendMsats)
      : null

  if (input.ownerConfirmed !== true) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.owner_confirmation_missing")
  }
  if (ownerApprovalRef === null) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.owner_approval_ref_missing")
  }
  if (dailySpendCapMsats === null || dailySpendCapMsats <= 0 || plannedShadowSpendMsats === null || plannedShadowSpendMsats <= 0) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.spend_cap_missing")
  } else if (plannedShadowSpendMsats > dailySpendCapMsats) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.spend_cap_exceeded")
  }
  if (input.psionicTrainingBoundary?.supportsTraining !== true) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.psionic_training_boundary_not_ready")
  }
  if (input.pylonServingPreflight?.canArmRealServing !== true) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.pylon_serving_preflight_not_ready")
  }
  if (verdictSourceRef === null) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.verdict_source_missing")
  }
  if (input.verdictSourceArmed !== true) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.verdict_source_not_armed")
  }
  if (shadowCandidateRef === null) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.shadow_candidate_missing")
  }
  if (baselineRouterRef === null) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.baseline_router_missing")
  }
  if (liveRolloutRef === null) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.live_rollout_missing")
  }
  if (paidShadowWinRef === null) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.paid_shadow_win_missing")
  }
  if (publicationRef === null) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.publication_ref_missing")
  }

  const allRefs = [
    ownerApprovalRef,
    input.psionicTrainingBoundary?.supportsTraining === true
      ? "boundary.pylon.psionic_training.ready.v0_3"
      : null,
    input.pylonServingPreflight?.canArmRealServing === true
      ? "preflight.pylon.real_serving.ready.v0_1"
      : null,
    verdictSourceRef,
    shadowCandidateRef,
    baselineRouterRef,
    liveRolloutRef,
    paidShadowWinRef,
    publicationRef,
    ...(input.psionicTrainingBoundary?.evidenceRefs ?? []),
    ...(input.pylonServingPreflight?.evidenceRefs ?? []),
  ]
  if (
    rawRefs.some((ref) => ref !== null && !isSafeRef(ref)) ||
    allRefs.some((ref) => ref !== null && !isSafeRef(ref))
  ) {
    blockerRefs.add("blocker.khala.m6.shadow_preflight.unsafe_ref")
  }

  const canStartShadowRun = [...blockerRefs].every(
    (ref) =>
      ref === "blocker.khala.m6.shadow_preflight.paid_shadow_win_missing" ||
      ref === "blocker.khala.m6.shadow_preflight.publication_ref_missing",
  )
  const canPublishM6Claim = blockerRefs.size === 0

  const projection: KhalaM6ShadowPreflightProjection = {
    schema: KHALA_M6_SHADOW_PREFLIGHT_SCHEMA,
    observedAt: input.observedAt,
    canStartShadowRun,
    canPublishM6Claim,
    ownerApprovalRef,
    dailySpendCapMsats,
    plannedShadowSpendMsats,
    psionicTrainingBoundaryRef:
      input.psionicTrainingBoundary?.supportsTraining === true
        ? "boundary.pylon.psionic_training.ready.v0_3"
        : null,
    pylonServingPreflightRef:
      input.pylonServingPreflight?.canArmRealServing === true
        ? "preflight.pylon.real_serving.ready.v0_1"
        : null,
    verdictSourceRef,
    shadowCandidateRef,
    baselineRouterRef,
    liveRolloutRef,
    paidShadowWinRef,
    publicationRef,
    evidenceRefs: uniqueRefs(allRefs),
    blockerRefs: [...blockerRefs].sort(),
    externalDependencyRefs,
    authorityBoundary:
      "Read-only M6 shadow-run readiness projection. It does not dispatch Psionic training, call a Pylon, arm live serving, spend sats, promote runtime artifacts, publish benchmark claims, or move settlement authority.",
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
