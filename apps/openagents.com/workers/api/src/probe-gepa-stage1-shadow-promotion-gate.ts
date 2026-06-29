import { Schema as S } from 'effect'

import { ProbeGepaCandidateProductState } from './probe-gepa-outcome-metrics'

export const ProbeGepaStage1ShadowPromotionSchemaVersion =
  'omega.probe_gepa_stage1_shadow_promotion.v1'

export const ProbeGepaStage1RequestedPromotionState = S.Literals([
  'active',
  'release_candidate',
  'shadow',
])
export type ProbeGepaStage1RequestedPromotionState =
  typeof ProbeGepaStage1RequestedPromotionState.Type

export const ProbeGepaStage1ShadowDecision = S.Literals([
  'rejected',
  'shadow',
])
export type ProbeGepaStage1ShadowDecision =
  typeof ProbeGepaStage1ShadowDecision.Type

export const ProbeGepaStage1PolicyFindingSeverity = S.Literals([
  'blocking',
  'none',
  'notice',
])
export type ProbeGepaStage1PolicyFindingSeverity =
  typeof ProbeGepaStage1PolicyFindingSeverity.Type

export class ProbeGepaStage1PolicyFinding extends S.Class<ProbeGepaStage1PolicyFinding>(
  'ProbeGepaStage1PolicyFinding',
)({
  findingRef: S.String,
  severity: ProbeGepaStage1PolicyFindingSeverity,
}) {}

export class ProbeGepaStage1ShadowPromotionInput extends S.Class<ProbeGepaStage1ShadowPromotionInput>(
  'ProbeGepaStage1ShadowPromotionInput',
)({
  blueprintGateRefs: S.Array(S.String),
  candidateHash: S.String,
  candidateRef: S.String,
  omegaGateRefs: S.Array(S.String),
  policyFindings: S.Array(ProbeGepaStage1PolicyFinding),
  proofBundleRefs: S.Array(S.String),
  proofCompletenessBps: S.Number,
  psionicFrontierRefs: S.Array(S.String),
  requestedState: ProbeGepaStage1RequestedPromotionState,
  retainedResultRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  validationDeltaBps: S.Number,
  validationResultRefs: S.Array(S.String),
}) {}

export class ProbeGepaStage1ShadowPromotionResult extends S.Class<ProbeGepaStage1ShadowPromotionResult>(
  'ProbeGepaStage1ShadowPromotionResult',
)({
  activeProductionAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  blueprintGateRefs: S.Array(S.String),
  candidateHash: S.String,
  candidateRef: S.String,
  candidateState: ProbeGepaCandidateProductState,
  decision: ProbeGepaStage1ShadowDecision,
  omegaGateRefs: S.Array(S.String),
  policyFindingRefs: S.Array(S.String),
  promotionDecisionRef: S.String,
  publicClaimLabel: S.String,
  publicStatusLabel: S.String,
  releaseCandidateAllowed: S.Boolean,
  requestedState: ProbeGepaStage1RequestedPromotionState,
  routeScorecardRefs: S.Array(S.String),
  schemaVersion: S.Literal(ProbeGepaStage1ShadowPromotionSchemaVersion),
}) {}

export class ProbeGepaStage1ShadowPromotionUnsafe extends S.TaggedErrorClass<ProbeGepaStage1ShadowPromotionUnsafe>()(
  'ProbeGepaStage1ShadowPromotionUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|fixture[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(auth|benchmark|email|fixture|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|trace|traces)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ProbeGepaStage1ShadowPromotionUnsafe({
      reason: `${label} contains private data, raw traces, provider secrets, wallet/payment material, private repo refs, or raw timestamps.`,
    })
  }
}

const blockerRefsFor = (
  input: ProbeGepaStage1ShadowPromotionInput,
): ReadonlyArray<string> => {
  const blockingPolicyRefs = input.policyFindings
    .filter(finding => finding.severity === 'blocking')
    .map(finding => `blocker.probe_gepa.stage1.policy.${finding.findingRef}`)
  const evidenceBlockers = [
    ...(input.requestedState === 'active'
      ? ['blocker.probe_gepa.stage1.active_not_allowed_by_shadow_gate']
      : []),
    ...(input.requestedState === 'release_candidate'
      ? ['blocker.probe_gepa.stage1.release_candidate_requires_separate_gate']
      : []),
    ...(input.retainedResultRefs.length === 0
      ? ['blocker.probe_gepa.stage1.missing_retained_refs']
      : []),
    ...(input.validationResultRefs.length === 0
      ? ['blocker.probe_gepa.stage1.missing_validation_refs']
      : []),
    ...(input.psionicFrontierRefs.length === 0
      ? ['blocker.probe_gepa.stage1.missing_psionic_frontier_refs']
      : []),
    ...(input.routeScorecardRefs.length === 0
      ? ['blocker.probe_gepa.stage1.missing_route_scorecards']
      : []),
    ...(input.proofBundleRefs.length === 0
      ? ['blocker.probe_gepa.stage1.missing_proof_bundles']
      : []),
    ...(input.omegaGateRefs.length === 0
      ? ['blocker.probe_gepa.stage1.missing_omega_gate_refs']
      : []),
    ...(input.blueprintGateRefs.length === 0
      ? ['blocker.probe_gepa.stage1.missing_blueprint_gate_refs']
      : []),
    ...(input.validationDeltaBps < 0
      ? ['blocker.probe_gepa.stage1.validation_regression']
      : []),
    ...(input.proofCompletenessBps < 8_000
      ? ['blocker.probe_gepa.stage1.proof_incomplete']
      : []),
  ]

  return uniqueRefs([...blockingPolicyRefs, ...evidenceBlockers])
}

const normalizeInput = (
  input: ProbeGepaStage1ShadowPromotionInput,
): ProbeGepaStage1ShadowPromotionInput =>
  new ProbeGepaStage1ShadowPromotionInput({
    ...input,
    blueprintGateRefs: uniqueRefs(input.blueprintGateRefs),
    omegaGateRefs: uniqueRefs(input.omegaGateRefs),
    policyFindings: input.policyFindings.map(
      finding =>
        new ProbeGepaStage1PolicyFinding({
          findingRef: finding.findingRef,
          severity: finding.severity,
        }),
    ),
    proofBundleRefs: uniqueRefs(input.proofBundleRefs),
    psionicFrontierRefs: uniqueRefs(input.psionicFrontierRefs),
    retainedResultRefs: uniqueRefs(input.retainedResultRefs),
    routeScorecardRefs: uniqueRefs(input.routeScorecardRefs),
    validationResultRefs: uniqueRefs(input.validationResultRefs),
  })

export const evaluateProbeGepaStage1ShadowPromotion = (
  input: ProbeGepaStage1ShadowPromotionInput,
): ProbeGepaStage1ShadowPromotionResult => {
  const normalized = normalizeInput(
    S.decodeUnknownSync(ProbeGepaStage1ShadowPromotionInput)(input),
  )
  const policyFindingRefs = uniqueRefs(
    normalized.policyFindings.map(finding => finding.findingRef),
  )

  assertSafeRefs('Probe GEPA Stage 1 shadow promotion identity refs', [
    normalized.candidateRef,
    normalized.candidateHash,
    ...policyFindingRefs,
  ])
  assertSafeRefs('Probe GEPA Stage 1 retained refs', normalized.retainedResultRefs)
  assertSafeRefs(
    'Probe GEPA Stage 1 validation refs',
    normalized.validationResultRefs,
  )
  assertSafeRefs(
    'Probe GEPA Stage 1 Psionic frontier refs',
    normalized.psionicFrontierRefs,
  )
  assertSafeRefs(
    'Probe GEPA Stage 1 route scorecard refs',
    normalized.routeScorecardRefs,
  )
  assertSafeRefs(
    'Probe GEPA Stage 1 proof bundle refs',
    normalized.proofBundleRefs,
  )
  assertSafeRefs('Probe GEPA Stage 1 Omega gate refs', normalized.omegaGateRefs)
  assertSafeRefs(
    'Probe GEPA Stage 1 Blueprint gate refs',
    normalized.blueprintGateRefs,
  )

  if (
    !Number.isInteger(normalized.validationDeltaBps) ||
    !Number.isInteger(normalized.proofCompletenessBps) ||
    normalized.proofCompletenessBps < 0 ||
    normalized.proofCompletenessBps > 10_000
  ) {
    throw new ProbeGepaStage1ShadowPromotionUnsafe({
      reason:
        'Stage 1 shadow promotion deltas and proof completeness must be integer basis-point values.',
    })
  }

  const blockerRefs = blockerRefsFor(normalized)
  const decision: ProbeGepaStage1ShadowDecision =
    blockerRefs.length === 0 ? 'shadow' : 'rejected'

  return new ProbeGepaStage1ShadowPromotionResult({
    activeProductionAllowed: false,
    blockerRefs,
    blueprintGateRefs: normalized.blueprintGateRefs,
    candidateHash: normalized.candidateHash,
    candidateRef: normalized.candidateRef,
    candidateState: decision === 'shadow' ? 'shadow' : 'benchmark_only',
    decision,
    omegaGateRefs: normalized.omegaGateRefs,
    policyFindingRefs,
    promotionDecisionRef:
      decision === 'shadow'
        ? `promotion_decision.probe_gepa.stage1.shadow.${normalized.candidateRef}`
        : `promotion_decision.probe_gepa.stage1.rejected.${normalized.candidateRef}`,
    publicClaimLabel:
      decision === 'shadow'
        ? 'shadow candidate; validation measured only'
        : 'candidate rejected by shadow gate',
    publicStatusLabel:
      decision === 'shadow' ? 'shadow candidate' : 'rejected candidate',
    releaseCandidateAllowed: false,
    requestedState: normalized.requestedState,
    routeScorecardRefs: normalized.routeScorecardRefs,
    schemaVersion: ProbeGepaStage1ShadowPromotionSchemaVersion,
  })
}
