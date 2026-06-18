import { Schema as S } from 'effect'

import {
  ProbeGepaCampaignProjection,
  assertProbeGepaCampaignProjectionSafe,
  probeGepaCampaignPublicSummary,
} from './probe-gepa-campaign-projection'
import { ProbeGepaStage1ShadowPromotionResult } from './probe-gepa-stage1-shadow-promotion-gate'
import { publicRefSegment, uniqueRefs } from './public-ref-format'

export const ArtanisProbeGepaBenchmarkSummarySchemaVersion =
  'omega.artanis_probe_gepa_benchmark_summary.v1'

export const ArtanisProbeGepaBenchmarkEvidenceLabel = S.Literals([
  'live_smoke',
  'retained_smoke',
  'retained_summary',
  'shadow_candidate',
  'validation_measured_only',
])
export type ArtanisProbeGepaBenchmarkEvidenceLabel =
  typeof ArtanisProbeGepaBenchmarkEvidenceLabel.Type

export const ArtanisProbeGepaBenchmarkPostingMode = S.Literals([
  'omega_operator_artanis_authorized',
])
export type ArtanisProbeGepaBenchmarkPostingMode =
  typeof ArtanisProbeGepaBenchmarkPostingMode.Type

export class ArtanisProbeGepaBenchmarkSummaryInput extends S.Class<ArtanisProbeGepaBenchmarkSummaryInput>(
  'ArtanisProbeGepaBenchmarkSummaryInput',
)({
  campaignProjection: ProbeGepaCampaignProjection,
  forumTopicRef: S.String,
  liveSmokeReceiptRefs: S.Array(S.String),
  operatorAuthorityRefs: S.Array(S.String),
  projectionAuthorityRefs: S.Array(S.String),
  publicReportRefs: S.Array(S.String),
  shadowPromotion: S.NullOr(ProbeGepaStage1ShadowPromotionResult),
}) {}

export class ArtanisProbeGepaBenchmarkSummaryProjection extends S.Class<ArtanisProbeGepaBenchmarkSummaryProjection>(
  'ArtanisProbeGepaBenchmarkSummaryProjection',
)({
  bodyText: S.String,
  claimBoundaryLine: S.String,
  evidenceLabel: ArtanisProbeGepaBenchmarkEvidenceLabel,
  forumTopicRef: S.String,
  idempotencyKey: S.String,
  noDistributedTrainingOverclaim: S.Boolean,
  noPaidWorkClaim: S.Boolean,
  noPublicBenchmarkScoreClaim: S.Boolean,
  noSettlementClaim: S.Boolean,
  operatorAuthorityRefs: S.Array(S.String),
  postingMode: ArtanisProbeGepaBenchmarkPostingMode,
  projectionAuthorityRefs: S.Array(S.String),
  publicReportRefs: S.Array(S.String),
  schemaVersion: S.Literal(ArtanisProbeGepaBenchmarkSummarySchemaVersion),
  sourceEvidenceRefs: S.Array(S.String),
  summaryRef: S.String,
  title: S.String,
}) {}

export class ArtanisProbeGepaBenchmarkSummaryUnsafe extends S.TaggedErrorClass<ArtanisProbeGepaBenchmarkSummaryUnsafe>()(
  'ArtanisProbeGepaBenchmarkSummaryUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|fixture[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(auth|benchmark|email|fixture|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|trace|traces)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisProbeGepaBenchmarkSummaryUnsafe({
      reason: `${label} contains private data, raw traces, provider secrets, wallet/payment material, private repo refs, or raw timestamps.`,
    })
  }
}

const evidenceLabelFor = (
  projection: ProbeGepaCampaignProjection,
  shadowPromotion: ProbeGepaStage1ShadowPromotionResult | null,
  liveSmokeReceiptRefs: ReadonlyArray<string>,
): ArtanisProbeGepaBenchmarkEvidenceLabel => {
  if (shadowPromotion !== null && shadowPromotion.decision === 'shadow') {
    return 'shadow_candidate'
  }

  if (liveSmokeReceiptRefs.length > 0) {
    return 'live_smoke'
  }

  switch (projection.claimState) {
    case 'measured_retained_smoke':
      return 'retained_smoke'
    case 'retained_summary':
      return 'retained_summary'
    case 'validation_measured_only':
      return 'validation_measured_only'
    case 'holdout_summary':
    case 'none':
      throw new ArtanisProbeGepaBenchmarkSummaryUnsafe({
        reason:
          'Artanis Probe GEPA summaries support retained smoke, retained summary, validation measured only, live smoke, or shadow candidate labels only.',
      })
  }
}

const claimBoundaryLineFor = (
  label: ArtanisProbeGepaBenchmarkEvidenceLabel,
): string => {
  switch (label) {
    case 'live_smoke':
      return 'Claim boundary: live smoke measured only; no public Terminal-Bench score, paid-work claim, or settlement claim.'
    case 'retained_smoke':
      return 'Claim boundary: measured retained smoke only; no public Terminal-Bench score, paid-work claim, or settlement claim.'
    case 'retained_summary':
      return 'Claim boundary: retained summary only; no public Terminal-Bench score, paid-work claim, or settlement claim.'
    case 'shadow_candidate':
      return 'Claim boundary: shadow candidate; validation measured only, with no active production claim.'
    case 'validation_measured_only':
      return 'Claim boundary: validation measured only; no frozen holdout or public Terminal-Bench score claim.'
  }
}

const titleFor = (label: ArtanisProbeGepaBenchmarkEvidenceLabel): string => {
  switch (label) {
    case 'live_smoke':
      return 'Artanis Probe GEPA live smoke summary'
    case 'retained_smoke':
      return 'Artanis Probe GEPA retained smoke summary'
    case 'retained_summary':
      return 'Artanis Probe GEPA retained summary'
    case 'shadow_candidate':
      return 'Artanis Probe GEPA shadow candidate summary'
    case 'validation_measured_only':
      return 'Artanis Probe GEPA validation summary'
  }
}

const labelTextFor = (
  label: ArtanisProbeGepaBenchmarkEvidenceLabel,
): string => {
  switch (label) {
    case 'live_smoke':
      return 'live smoke'
    case 'retained_smoke':
      return 'retained smoke'
    case 'retained_summary':
      return 'retained summary'
    case 'shadow_candidate':
      return 'shadow candidate'
    case 'validation_measured_only':
      return 'validation measured only'
  }
}

const assertGeneratedBodySafe = (bodyText: string): void => {
  const distributedTrainingMentioned = /distributed training/i.test(bodyText)
  const allowedDistributedWording =
    /Pylon-distributed GEPA rollout optimization/i.test(bodyText) &&
    /not distributed neural-network training/i.test(bodyText)

  if (
    unsafeRefPattern.test(bodyText) ||
    rawTimestampPattern.test(bodyText) ||
    /beats terminal-bench|leaderboard winner|public benchmark score|frozen holdout performance/i.test(
      bodyText,
    ) ||
    (distributedTrainingMentioned && !allowedDistributedWording)
  ) {
    throw new ArtanisProbeGepaBenchmarkSummaryUnsafe({
      reason:
        'Generated Artanis Probe GEPA summary contains unsafe material or overclaims benchmark, payout, settlement, or training authority.',
    })
  }
}

export const buildArtanisProbeGepaBenchmarkSummary = (
  input: ArtanisProbeGepaBenchmarkSummaryInput,
): ArtanisProbeGepaBenchmarkSummaryProjection => {
  const decoded = S.decodeUnknownSync(ArtanisProbeGepaBenchmarkSummaryInput)(
    input,
  )
  const projection = assertProbeGepaCampaignProjectionSafe(
    decoded.campaignProjection,
  )
  const publicSummary = probeGepaCampaignPublicSummary(projection)
  const liveSmokeReceiptRefs = uniqueRefs(decoded.liveSmokeReceiptRefs)
  const operatorAuthorityRefs = uniqueRefs(decoded.operatorAuthorityRefs)
  const projectionAuthorityRefs = uniqueRefs(decoded.projectionAuthorityRefs)
  const publicReportRefs = uniqueRefs(decoded.publicReportRefs)
  const evidenceLabel = evidenceLabelFor(
    projection,
    decoded.shadowPromotion,
    liveSmokeReceiptRefs,
  )
  const sourceEvidenceRefs = uniqueRefs([
    projection.campaignRef,
    ...projection.retainedResultRefs,
    ...projection.validationResultRefs,
    ...projection.artifactManifestRefs,
    ...projection.receiptRefs,
    ...projection.resourceReceiptRefs,
    ...projection.pylonBatchRefs,
    ...liveSmokeReceiptRefs,
    ...(decoded.shadowPromotion === null
      ? []
      : [
          decoded.shadowPromotion.promotionDecisionRef,
          ...decoded.shadowPromotion.routeScorecardRefs,
        ]),
  ])

  assertSafeRefs('Artanis Probe GEPA source evidence refs', sourceEvidenceRefs)
  assertSafeRefs(
    'Artanis Probe GEPA operator authority refs',
    operatorAuthorityRefs,
  )
  assertSafeRefs(
    'Artanis Probe GEPA projection authority refs',
    projectionAuthorityRefs,
  )
  assertSafeRefs('Artanis Probe GEPA public report refs', publicReportRefs)
  assertSafeRefs('Artanis Probe GEPA forum topic refs', [decoded.forumTopicRef])

  if (operatorAuthorityRefs.length === 0 || projectionAuthorityRefs.length === 0) {
    throw new ArtanisProbeGepaBenchmarkSummaryUnsafe({
      reason:
        'Artanis Probe GEPA summaries require operator and projection authority refs.',
    })
  }

  const claimBoundaryLine = claimBoundaryLineFor(evidenceLabel)
  const bodyText = [
    'Artanis Probe GEPA status:',
    '',
    `The current evidence label is ${labelTextFor(evidenceLabel)}.`,
    `Campaign ref: ${projection.campaignRef}.`,
    `Evidence refs: ${sourceEvidenceRefs.join(', ')}.`,
    `Authority refs: ${[...operatorAuthorityRefs, ...projectionAuthorityRefs].join(', ')}.`,
    `Metric calls: ${projection.completedMetricCalls} completed; ${projection.validMetricCalls} valid and ${projection.invalidMetricCalls} invalid.`,
    `Evidence counts: retained=${publicSummary.evidenceCounts.retained}, validation=${publicSummary.evidenceCounts.validation}, holdout=${publicSummary.evidenceCounts.holdout}.`,
    claimBoundaryLine,
    'This is Pylon-distributed GEPA rollout optimization, not distributed neural-network training.',
    'The next gate remains Omega and Blueprint review before broader release, activation, payout, or settlement language.',
  ].join('\n')

  assertGeneratedBodySafe(bodyText)

  return assertArtanisProbeGepaBenchmarkSummaryProjectionSafe(
    new ArtanisProbeGepaBenchmarkSummaryProjection({
      bodyText,
      claimBoundaryLine,
      evidenceLabel,
      forumTopicRef: decoded.forumTopicRef,
      idempotencyKey: [
        'forum-artanis-probe-gepa',
        publicRefSegment(projection.campaignRef, 'summary'),
        evidenceLabel,
      ].join('-'),
      noDistributedTrainingOverclaim: true,
      noPaidWorkClaim: true,
      noPublicBenchmarkScoreClaim: true,
      noSettlementClaim: true,
      operatorAuthorityRefs,
      postingMode: 'omega_operator_artanis_authorized',
      projectionAuthorityRefs,
      publicReportRefs,
      schemaVersion: ArtanisProbeGepaBenchmarkSummarySchemaVersion,
      sourceEvidenceRefs,
      summaryRef: `summary.artanis.probe_gepa.${publicRefSegment(
        projection.campaignRef,
        'summary',
      )}.${evidenceLabel}`,
      title: titleFor(evidenceLabel),
    }),
  )
}

export const assertArtanisProbeGepaBenchmarkSummaryProjectionSafe = (
  projection: ArtanisProbeGepaBenchmarkSummaryProjection,
): ArtanisProbeGepaBenchmarkSummaryProjection => {
  const decoded = S.decodeUnknownSync(ArtanisProbeGepaBenchmarkSummaryProjection)(
    projection,
  )

  assertSafeRefs('Artanis Probe GEPA summary identity refs', [
    decoded.summaryRef,
    decoded.idempotencyKey,
    decoded.forumTopicRef,
  ])
  assertSafeRefs(
    'Artanis Probe GEPA summary source evidence refs',
    decoded.sourceEvidenceRefs,
  )
  assertSafeRefs(
    'Artanis Probe GEPA summary operator authority refs',
    decoded.operatorAuthorityRefs,
  )
  assertSafeRefs(
    'Artanis Probe GEPA summary projection authority refs',
    decoded.projectionAuthorityRefs,
  )
  assertSafeRefs(
    'Artanis Probe GEPA summary public report refs',
    decoded.publicReportRefs,
  )

  if (
    decoded.operatorAuthorityRefs.length === 0 ||
    decoded.projectionAuthorityRefs.length === 0 ||
    !decoded.noDistributedTrainingOverclaim ||
    !decoded.noPaidWorkClaim ||
    !decoded.noPublicBenchmarkScoreClaim ||
    !decoded.noSettlementClaim ||
    decoded.postingMode !== 'omega_operator_artanis_authorized'
  ) {
    throw new ArtanisProbeGepaBenchmarkSummaryUnsafe({
      reason:
        'Artanis Probe GEPA summaries require Omega/operator authority and all no-overclaim flags.',
    })
  }

  assertGeneratedBodySafe(decoded.bodyText)

  return decoded
}
