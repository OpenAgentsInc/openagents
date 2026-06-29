import { Schema as S } from 'effect'

export const ProbeGepaCampaignProjectionSchemaVersion =
  'omega.probe_gepa_campaign_projection.v1'

export const ProbeGepaCampaignStage = S.Literals([
  'stage_0_smoke',
  'stage_1_retained_sprint',
  'validation_sweep',
  'holdout_review',
])
export type ProbeGepaCampaignStage = typeof ProbeGepaCampaignStage.Type

export const ProbeGepaCampaignClaimState = S.Literals([
  'none',
  'measured_retained_smoke',
  'retained_summary',
  'validation_measured_only',
  'holdout_summary',
])
export type ProbeGepaCampaignClaimState =
  typeof ProbeGepaCampaignClaimState.Type

export class ProbeGepaCampaignProjection extends S.Class<ProbeGepaCampaignProjection>(
  'ProbeGepaCampaignProjection',
)({
  activeCandidateRefs: S.Array(S.String),
  artifactManifestRefs: S.Array(S.String),
  baselineCandidateRef: S.NullOr(S.String),
  benchmarkSuiteRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  campaignRef: S.String,
  candidateHashRefs: S.Array(S.String),
  claimState: ProbeGepaCampaignClaimState,
  completedMetricCalls: S.Number,
  costSummaryRefs: S.Array(S.String),
  holdoutResultRefs: S.Array(S.String),
  invalidMetricCalls: S.Number,
  nextActionRefs: S.Array(S.String),
  objectiveRef: S.String,
  plannedMetricCalls: S.Number,
  policyFindingRefs: S.Array(S.String),
  probeCommitRefs: S.Array(S.String),
  promotionDecisionRefs: S.Array(S.String),
  pylonBatchRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  resourceReceiptRefs: S.Array(S.String),
  retainedResultRefs: S.Array(S.String),
  schemaVersion: S.Literal(ProbeGepaCampaignProjectionSchemaVersion),
  settlementReceiptRefs: S.Array(S.String),
  splitManifestRefs: S.Array(S.String),
  stage: ProbeGepaCampaignStage,
  validMetricCalls: S.Number,
  validationResultRefs: S.Array(S.String),
}) {}

export class ProbeGepaCampaignProjectionUnsafe extends S.TaggedErrorClass<ProbeGepaCampaignProjectionUnsafe>()(
  'ProbeGepaCampaignProjectionUnsafe',
  {
    reason: S.String,
  },
) {}

export type ProbeGepaCampaignEvidenceCounts = Readonly<{
  retained: number
  validation: number
  holdout: number
}>

export type ProbeGepaCampaignPublicSummary = Readonly<{
  campaignRef: string
  claimState: ProbeGepaCampaignClaimState
  completedMetricCalls: number
  evidenceCounts: ProbeGepaCampaignEvidenceCounts
  payoutClaimAllowed: boolean
  pylonWorkVisibleWithoutPayoutClaim: boolean
  stage: ProbeGepaCampaignStage
}>

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|fixture[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(auth|benchmark|email|fixture|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|trace|traces)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const projectionRefFields = [
  'activeCandidateRefs',
  'artifactManifestRefs',
  'benchmarkSuiteRefs',
  'blockerRefs',
  'candidateHashRefs',
  'costSummaryRefs',
  'holdoutResultRefs',
  'nextActionRefs',
  'policyFindingRefs',
  'probeCommitRefs',
  'promotionDecisionRefs',
  'pylonBatchRefs',
  'receiptRefs',
  'resourceReceiptRefs',
  'retainedResultRefs',
  'settlementReceiptRefs',
  'splitManifestRefs',
  'validationResultRefs',
] as const

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const unsafeRef = (refs: ReadonlyArray<string>): string | undefined =>
  uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = unsafeRef(refs)

  if (unsafe !== undefined) {
    throw new ProbeGepaCampaignProjectionUnsafe({
      reason: `${label} contains raw prompts, raw traces, raw benchmark fixtures, provider credentials, account refs, bearer material, wallet material, invoices/preimages, private repo paths, or local filesystem paths.`,
    })
  }
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new ProbeGepaCampaignProjectionUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

export const normalizeProbeGepaCampaignProjection = (
  projection: ProbeGepaCampaignProjection,
): ProbeGepaCampaignProjection => {
  const normalized = {
    ...projection,
    activeCandidateRefs: uniqueRefs(projection.activeCandidateRefs),
    artifactManifestRefs: uniqueRefs(projection.artifactManifestRefs),
    benchmarkSuiteRefs: uniqueRefs(projection.benchmarkSuiteRefs),
    blockerRefs: uniqueRefs(projection.blockerRefs),
    candidateHashRefs: uniqueRefs(projection.candidateHashRefs),
    costSummaryRefs: uniqueRefs(projection.costSummaryRefs),
    holdoutResultRefs: uniqueRefs(projection.holdoutResultRefs),
    nextActionRefs: uniqueRefs(projection.nextActionRefs),
    policyFindingRefs: uniqueRefs(projection.policyFindingRefs),
    probeCommitRefs: uniqueRefs(projection.probeCommitRefs),
    promotionDecisionRefs: uniqueRefs(projection.promotionDecisionRefs),
    pylonBatchRefs: uniqueRefs(projection.pylonBatchRefs),
    receiptRefs: uniqueRefs(projection.receiptRefs),
    resourceReceiptRefs: uniqueRefs(projection.resourceReceiptRefs),
    retainedResultRefs: uniqueRefs(projection.retainedResultRefs),
    settlementReceiptRefs: uniqueRefs(projection.settlementReceiptRefs),
    splitManifestRefs: uniqueRefs(projection.splitManifestRefs),
    validationResultRefs: uniqueRefs(projection.validationResultRefs),
  }

  return new ProbeGepaCampaignProjection(normalized)
}

export const assertProbeGepaCampaignProjectionSafe = (
  projection: ProbeGepaCampaignProjection,
): ProbeGepaCampaignProjection => {
  const normalized = normalizeProbeGepaCampaignProjection(projection)

  assertSafeRefs('Probe GEPA campaign identity refs', [
    normalized.campaignRef,
    normalized.objectiveRef,
    ...(normalized.baselineCandidateRef === null
      ? []
      : [normalized.baselineCandidateRef]),
  ])

  for (const field of projectionRefFields) {
    assertSafeRefs(`Probe GEPA campaign ${field}`, normalized[field])
  }

  assertNonNegativeInteger('plannedMetricCalls', normalized.plannedMetricCalls)
  assertNonNegativeInteger(
    'completedMetricCalls',
    normalized.completedMetricCalls,
  )
  assertNonNegativeInteger('validMetricCalls', normalized.validMetricCalls)
  assertNonNegativeInteger('invalidMetricCalls', normalized.invalidMetricCalls)

  if (
    normalized.completedMetricCalls >
    Math.max(normalized.plannedMetricCalls, normalized.validMetricCalls)
  ) {
    throw new ProbeGepaCampaignProjectionUnsafe({
      reason:
        'completedMetricCalls cannot exceed plannedMetricCalls unless validMetricCalls records the completed imports.',
    })
  }

  if (
    normalized.validMetricCalls + normalized.invalidMetricCalls !==
    normalized.completedMetricCalls
  ) {
    throw new ProbeGepaCampaignProjectionUnsafe({
      reason:
        'validMetricCalls plus invalidMetricCalls must equal completedMetricCalls.',
    })
  }

  if (
    normalized.claimState === 'measured_retained_smoke' ||
    normalized.claimState === 'retained_summary'
  ) {
    requireEvidence(normalized, 'retainedResultRefs', normalized.claimState)
  }

  if (normalized.claimState === 'validation_measured_only') {
    requireEvidence(normalized, 'validationResultRefs', normalized.claimState)
  }

  if (normalized.claimState === 'holdout_summary') {
    requireEvidence(normalized, 'holdoutResultRefs', normalized.claimState)
  }

  if (
    normalized.pylonBatchRefs.length > 0 &&
    normalized.settlementReceiptRefs.length === 0
  ) {
    assertSafeRefs('Probe GEPA visible Pylon no-payout caveat', [
      'caveat.public.pylon_work_visible_without_payout_claim',
    ])
  }

  if (
    probeGepaCampaignPayoutClaimAllowed(normalized) &&
    normalized.receiptRefs.length === 0
  ) {
    throw new ProbeGepaCampaignProjectionUnsafe({
      reason:
        'Payout claims require public receipt refs and settlement receipt refs.',
    })
  }

  return normalized
}

export const probeGepaCampaignEvidenceCounts = (
  projection: ProbeGepaCampaignProjection,
): ProbeGepaCampaignEvidenceCounts => ({
  holdout: projection.holdoutResultRefs.length,
  retained: projection.retainedResultRefs.length,
  validation: projection.validationResultRefs.length,
})

export const probeGepaCampaignPayoutClaimAllowed = (
  projection: ProbeGepaCampaignProjection,
): boolean =>
  projection.pylonBatchRefs.length > 0 &&
  projection.receiptRefs.length > 0 &&
  projection.settlementReceiptRefs.length > 0

export const probeGepaCampaignPublicSummary = (
  projection: ProbeGepaCampaignProjection,
): ProbeGepaCampaignPublicSummary => {
  const safeProjection = assertProbeGepaCampaignProjectionSafe(projection)
  const payoutClaimAllowed = probeGepaCampaignPayoutClaimAllowed(safeProjection)

  return {
    campaignRef: safeProjection.campaignRef,
    claimState: safeProjection.claimState,
    completedMetricCalls: safeProjection.completedMetricCalls,
    evidenceCounts: probeGepaCampaignEvidenceCounts(safeProjection),
    payoutClaimAllowed,
    pylonWorkVisibleWithoutPayoutClaim:
      safeProjection.pylonBatchRefs.length > 0 && !payoutClaimAllowed,
    stage: safeProjection.stage,
  }
}

const requireEvidence = (
  projection: ProbeGepaCampaignProjection,
  field: 'holdoutResultRefs' | 'retainedResultRefs' | 'validationResultRefs',
  claimState: ProbeGepaCampaignClaimState,
): void => {
  if (projection[field].length === 0) {
    throw new ProbeGepaCampaignProjectionUnsafe({
      reason: `${claimState} requires ${field}.`,
    })
  }
}
