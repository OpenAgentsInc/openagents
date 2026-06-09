import { Schema as S } from 'effect'

export const ProbeGepaOutcomeMetricsSchemaVersion =
  'omega.probe_gepa_outcome_metrics.v1'

export const ProbeGepaCandidateProductState = S.Literals([
  'active',
  'benchmark_only',
  'release_candidate',
  'shadow',
])
export type ProbeGepaCandidateProductState =
  typeof ProbeGepaCandidateProductState.Type

export const ProbeGepaProofState = S.Literals([
  'private_proof_available',
  'public_proof_available',
  'redacted',
  'withheld',
])
export type ProbeGepaProofState = typeof ProbeGepaProofState.Type

export const ProbeGepaOutcomeMetricsAudience = S.Literals([
  'operator',
  'public',
])
export type ProbeGepaOutcomeMetricsAudience =
  typeof ProbeGepaOutcomeMetricsAudience.Type

export class ProbeGepaCodingOutcomeMetricSnapshot extends S.Class<ProbeGepaCodingOutcomeMetricSnapshot>(
  'ProbeGepaCodingOutcomeMetricSnapshot',
)({
  acceptanceRateBps: S.Number,
  artifactCompletenessBps: S.Number,
  closeoutQualityBps: S.Number,
  costPerAcceptedOutcomeRef: S.String,
  failureFamilyReductionBps: S.Number,
  humanReviewMinutes: S.Number,
  publicProofState: ProbeGepaProofState,
  privateProofState: ProbeGepaProofState,
  proofBundleCompletenessBps: S.Number,
  regressionCount: S.Number,
  retryCount: S.Number,
  retriesPerAcceptedOutcome: S.Number,
  turnsPerAcceptedOutcome: S.Number,
}) {}

export class ProbeGepaOutcomeMetricsProjection extends S.Class<ProbeGepaOutcomeMetricsProjection>(
  'ProbeGepaOutcomeMetricsProjection',
)({
  acceptedOutcomeRefs: S.Array(S.String),
  after: ProbeGepaCodingOutcomeMetricSnapshot,
  before: ProbeGepaCodingOutcomeMetricSnapshot,
  benchmarkCampaignRefs: S.Array(S.String),
  benchmarkValidationRefs: S.Array(S.String),
  candidateHash: S.String,
  candidateRef: S.String,
  candidateState: ProbeGepaCandidateProductState,
  claimBoundaryRef: S.String,
  closeoutQualityRef: S.String,
  failureFamilyRefs: S.Array(S.String),
  privateProofRefs: S.Array(S.String),
  publicProofRefs: S.Array(S.String),
  regressionRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  schemaVersion: S.Literal(ProbeGepaOutcomeMetricsSchemaVersion),
  selectedSignatureRefs: S.Array(S.String),
  toolMenuRefs: S.Array(S.String),
  workroomComparisonRefs: S.Array(S.String),
  workroomOutcomeRefs: S.Array(S.String),
  workroomRefs: S.Array(S.String),
}) {}

export const ProbeGepaOutcomeMetricsAudienceProjectionSchemaVersion =
  'omega.probe_gepa_outcome_metrics_audience_projection.v1'

export class ProbeGepaOutcomeMetricsAudienceProjection extends S.Class<ProbeGepaOutcomeMetricsAudienceProjection>(
  'ProbeGepaOutcomeMetricsAudienceProjection',
)({
  acceptedOutcomeRefs: S.Array(S.String),
  acceptanceRateDeltaBps: S.Number,
  artifactCompletenessDeltaBps: S.Number,
  candidateHash: S.String,
  candidateRef: S.String,
  candidateState: ProbeGepaCandidateProductState,
  claimText: S.String,
  closeoutQualityDeltaBps: S.Number,
  failureFamilyReductionDeltaBps: S.Number,
  humanReviewMinutesDelta: S.Number,
  privateProofRefs: S.Array(S.String),
  productOutcomeClaimAllowed: S.Boolean,
  proofBundleCompletenessDeltaBps: S.Number,
  publicProofRefs: S.Array(S.String),
  regressionCountDelta: S.Number,
  retryCountDelta: S.Number,
  retriesPerAcceptedOutcomeDelta: S.Number,
  routeScorecardRefs: S.Array(S.String),
  schemaVersion: S.Literal(ProbeGepaOutcomeMetricsAudienceProjectionSchemaVersion),
  selectedSignatureRefs: S.Array(S.String),
  targetAudience: ProbeGepaOutcomeMetricsAudience,
  toolMenuRefs: S.Array(S.String),
  turnsPerAcceptedOutcomeDelta: S.Number,
  workroomComparisonRefs: S.Array(S.String),
  workroomOutcomeRefs: S.Array(S.String),
  workroomRefs: S.Array(S.String),
}) {}

export type ProbeGepaOutcomeMetricDelta = Readonly<{
  acceptanceRateDeltaBps: number
  artifactCompletenessDeltaBps: number
  closeoutQualityDeltaBps: number
  failureFamilyReductionDeltaBps: number
  humanReviewMinutesDelta: number
  proofBundleCompletenessDeltaBps: number
  regressionCountDelta: number
  retriesPerAcceptedOutcomeDelta: number
  retryCountDelta: number
  turnsPerAcceptedOutcomeDelta: number
}>

export type ProbeGepaOutcomeMetricSummary = Readonly<{
  candidateState: ProbeGepaCandidateProductState
  claimText: string
  delta: ProbeGepaOutcomeMetricDelta
  productOutcomeClaimAllowed: boolean
}>

export class ProbeGepaOutcomeMetricsUnsafe extends S.TaggedErrorClass<ProbeGepaOutcomeMetricsUnsafe>()(
  'ProbeGepaOutcomeMetricsUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(auth|email|fixture|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|trace|traces)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const projectionRefFields = [
  'acceptedOutcomeRefs',
  'benchmarkCampaignRefs',
  'benchmarkValidationRefs',
  'failureFamilyRefs',
  'privateProofRefs',
  'publicProofRefs',
  'regressionRefs',
  'routeScorecardRefs',
  'selectedSignatureRefs',
  'toolMenuRefs',
  'workroomComparisonRefs',
  'workroomOutcomeRefs',
  'workroomRefs',
] as const

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

export const normalizeProbeGepaOutcomeMetricsProjection = (
  projection: ProbeGepaOutcomeMetricsProjection,
): ProbeGepaOutcomeMetricsProjection =>
  new ProbeGepaOutcomeMetricsProjection({
    ...projection,
    acceptedOutcomeRefs: uniqueRefs(projection.acceptedOutcomeRefs),
    benchmarkCampaignRefs: uniqueRefs(projection.benchmarkCampaignRefs),
    benchmarkValidationRefs: uniqueRefs(projection.benchmarkValidationRefs),
    failureFamilyRefs: uniqueRefs(projection.failureFamilyRefs),
    privateProofRefs: uniqueRefs(projection.privateProofRefs),
    publicProofRefs: uniqueRefs(projection.publicProofRefs),
    regressionRefs: uniqueRefs(projection.regressionRefs),
    routeScorecardRefs: uniqueRefs(projection.routeScorecardRefs),
    selectedSignatureRefs: uniqueRefs(projection.selectedSignatureRefs),
    toolMenuRefs: uniqueRefs(projection.toolMenuRefs),
    workroomComparisonRefs: uniqueRefs(projection.workroomComparisonRefs),
    workroomOutcomeRefs: uniqueRefs(projection.workroomOutcomeRefs),
    workroomRefs: uniqueRefs(projection.workroomRefs),
  })

export const assertProbeGepaOutcomeMetricsSafe = (
  projection: ProbeGepaOutcomeMetricsProjection,
): ProbeGepaOutcomeMetricsProjection => {
  const normalized = normalizeProbeGepaOutcomeMetricsProjection(projection)

  assertSafeRefs('Probe GEPA outcome identity refs', [
    normalized.candidateRef,
    normalized.candidateHash,
    normalized.claimBoundaryRef,
    normalized.closeoutQualityRef,
  ])

  for (const field of projectionRefFields) {
    assertSafeRefs(`Probe GEPA outcome ${field}`, normalized[field])
  }

  validateSnapshot('before', normalized.before)
  validateSnapshot('after', normalized.after)

  if (normalized.routeScorecardRefs.length === 0) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Route scorecard refs are required for outcome metric comparison.',
    })
  }

  if (normalized.selectedSignatureRefs.length === 0) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Selected signature refs are required before outcome metric comparison.',
    })
  }

  if (normalized.toolMenuRefs.length === 0) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Tool menu refs are required before outcome metric comparison.',
    })
  }

  if (normalized.workroomComparisonRefs.length === 0) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Workroom comparison refs are required before product comparison.',
    })
  }

  if (normalized.benchmarkValidationRefs.length === 0) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Benchmark validation refs are required before product comparison.',
    })
  }

  if (
    probeGepaProductOutcomeClaimAllowed(normalized) &&
    (normalized.publicProofRefs.length === 0 ||
      normalized.privateProofRefs.length === 0)
  ) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Accepted coding outcome improvement claims require public and private proof refs.',
    })
  }

  if (
    probeGepaProductOutcomeClaimAllowed(normalized) &&
    normalized.workroomOutcomeRefs.length === 0
  ) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Accepted coding outcome improvement claims require workroom outcome refs.',
    })
  }

  if (
    normalized.candidateState === 'active' &&
    !probeGepaProductOutcomeClaimAllowed(normalized)
  ) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason:
        'Active candidates require accepted outcome refs and proof refs; benchmark-only evidence is not enough.',
    })
  }

  return normalized
}

export const probeGepaOutcomeMetricDelta = (
  projection: ProbeGepaOutcomeMetricsProjection,
): ProbeGepaOutcomeMetricDelta => ({
  acceptanceRateDeltaBps:
    projection.after.acceptanceRateBps - projection.before.acceptanceRateBps,
  artifactCompletenessDeltaBps:
    projection.after.artifactCompletenessBps -
    projection.before.artifactCompletenessBps,
  closeoutQualityDeltaBps:
    projection.after.closeoutQualityBps - projection.before.closeoutQualityBps,
  failureFamilyReductionDeltaBps:
    projection.after.failureFamilyReductionBps -
    projection.before.failureFamilyReductionBps,
  humanReviewMinutesDelta:
    projection.after.humanReviewMinutes - projection.before.humanReviewMinutes,
  proofBundleCompletenessDeltaBps:
    projection.after.proofBundleCompletenessBps -
    projection.before.proofBundleCompletenessBps,
  regressionCountDelta:
    projection.after.regressionCount - projection.before.regressionCount,
  retriesPerAcceptedOutcomeDelta:
    projection.after.retriesPerAcceptedOutcome -
    projection.before.retriesPerAcceptedOutcome,
  retryCountDelta: projection.after.retryCount - projection.before.retryCount,
  turnsPerAcceptedOutcomeDelta:
    projection.after.turnsPerAcceptedOutcome -
    projection.before.turnsPerAcceptedOutcome,
})

export const probeGepaProductOutcomeClaimAllowed = (
  projection: ProbeGepaOutcomeMetricsProjection,
): boolean =>
  projection.acceptedOutcomeRefs.length > 0 &&
  projection.publicProofRefs.length > 0 &&
  projection.privateProofRefs.length > 0

export const probeGepaOutcomeMetricSummary = (
  projection: ProbeGepaOutcomeMetricsProjection,
): ProbeGepaOutcomeMetricSummary => {
  const safeProjection = assertProbeGepaOutcomeMetricsSafe(projection)
  const productOutcomeClaimAllowed =
    probeGepaProductOutcomeClaimAllowed(safeProjection)

  return {
    candidateState: safeProjection.candidateState,
    claimText: productOutcomeClaimAllowed
      ? 'Accepted coding outcome comparison; paid customer outcome improvement is linked to accepted outcome refs and proof refs.'
      : 'Benchmark validation only; no paid customer outcome improvement claim.',
    delta: probeGepaOutcomeMetricDelta(safeProjection),
    productOutcomeClaimAllowed,
  }
}

export const projectProbeGepaOutcomeMetricsForAudience = (
  projection: ProbeGepaOutcomeMetricsProjection,
  targetAudience: ProbeGepaOutcomeMetricsAudience,
): ProbeGepaOutcomeMetricsAudienceProjection => {
  const safeProjection = assertProbeGepaOutcomeMetricsSafe(projection)
  const summary = probeGepaOutcomeMetricSummary(safeProjection)
  const operatorAudience = targetAudience === 'operator'
  const productOutcomeClaimAllowed = summary.productOutcomeClaimAllowed

  return new ProbeGepaOutcomeMetricsAudienceProjection({
    acceptedOutcomeRefs: productOutcomeClaimAllowed
      ? safeProjection.acceptedOutcomeRefs
      : [],
    acceptanceRateDeltaBps: summary.delta.acceptanceRateDeltaBps,
    artifactCompletenessDeltaBps:
      summary.delta.artifactCompletenessDeltaBps,
    candidateHash: safeProjection.candidateHash,
    candidateRef: safeProjection.candidateRef,
    candidateState: safeProjection.candidateState,
    claimText: summary.claimText,
    closeoutQualityDeltaBps: summary.delta.closeoutQualityDeltaBps,
    failureFamilyReductionDeltaBps:
      summary.delta.failureFamilyReductionDeltaBps,
    humanReviewMinutesDelta: summary.delta.humanReviewMinutesDelta,
    privateProofRefs: operatorAudience ? safeProjection.privateProofRefs : [],
    productOutcomeClaimAllowed,
    proofBundleCompletenessDeltaBps:
      summary.delta.proofBundleCompletenessDeltaBps,
    publicProofRefs: productOutcomeClaimAllowed
      ? safeProjection.publicProofRefs
      : [],
    regressionCountDelta: summary.delta.regressionCountDelta,
    retryCountDelta: summary.delta.retryCountDelta,
    retriesPerAcceptedOutcomeDelta:
      summary.delta.retriesPerAcceptedOutcomeDelta,
    routeScorecardRefs: safeProjection.routeScorecardRefs,
    schemaVersion: ProbeGepaOutcomeMetricsAudienceProjectionSchemaVersion,
    selectedSignatureRefs: safeProjection.selectedSignatureRefs,
    targetAudience,
    toolMenuRefs: safeProjection.toolMenuRefs,
    turnsPerAcceptedOutcomeDelta: summary.delta.turnsPerAcceptedOutcomeDelta,
    workroomComparisonRefs: operatorAudience || productOutcomeClaimAllowed
      ? safeProjection.workroomComparisonRefs
      : [],
    workroomOutcomeRefs: productOutcomeClaimAllowed
      ? safeProjection.workroomOutcomeRefs
      : [],
    workroomRefs: operatorAudience ? safeProjection.workroomRefs : [],
  })
}

const validateSnapshot = (
  label: string,
  snapshot: ProbeGepaCodingOutcomeMetricSnapshot,
): void => {
  const bpsFields = [
    snapshot.acceptanceRateBps,
    snapshot.artifactCompletenessBps,
    snapshot.closeoutQualityBps,
    snapshot.failureFamilyReductionBps,
    snapshot.proofBundleCompletenessBps,
  ]

  if (
    bpsFields.some(
      value => !Number.isInteger(value) || value < 0 || value > 10_000,
    )
  ) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason: `${label} basis-point metrics must be integers between 0 and 10000.`,
    })
  }

  if (
    [
      snapshot.humanReviewMinutes,
      snapshot.regressionCount,
      snapshot.retryCount,
      snapshot.retriesPerAcceptedOutcome,
      snapshot.turnsPerAcceptedOutcome,
    ].some(value => !Number.isFinite(value) || value < 0)
  ) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason: `${label} count and duration metrics must be non-negative.`,
    })
  }

  assertSafeRefs(`${label} costPerAcceptedOutcomeRef`, [
    snapshot.costPerAcceptedOutcomeRef,
  ])
}

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ProbeGepaOutcomeMetricsUnsafe({
      reason: `${label} contains private data, raw traces, provider secrets, wallet/payment material, private repo refs, or raw timestamps.`,
    })
  }
}
