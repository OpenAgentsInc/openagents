import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const OpenAgentsPsionicEvidenceKind = S.Literals([
  'candidate_module',
  'eval_run',
  'optimizer_run',
  'promotion_proposal',
  'rollback',
  'scorecard',
  'training_run',
])
export type OpenAgentsPsionicEvidenceKind =
  typeof OpenAgentsPsionicEvidenceKind.Type

export const OpenAgentsPsionicEvidenceStatus = S.Literals([
  'archived',
  'completed',
  'draft',
  'failed',
  'needs_review',
  'retained_failure',
  'running',
])
export type OpenAgentsPsionicEvidenceStatus =
  typeof OpenAgentsPsionicEvidenceStatus.Type

export const OpenAgentsPsionicAuthorityBoundary = S.Literals([
  'evidence_only',
])
export type OpenAgentsPsionicAuthorityBoundary =
  typeof OpenAgentsPsionicAuthorityBoundary.Type

export class OpenAgentsPsionicAuthority extends S.Class<OpenAgentsPsionicAuthority>(
  'OpenAgentsPsionicAuthority',
)({
  authorityBoundary: OpenAgentsPsionicAuthorityBoundary,
  noAcceptedOutcomeSettlement: S.Boolean,
  noDirectModulePromotion: S.Boolean,
  noPayoutMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRoutingMutation: S.Boolean,
}) {}

export class OpenAgentsPsionicEvidenceRecord extends S.Class<OpenAgentsPsionicEvidenceRecord>(
  'OpenAgentsPsionicEvidenceRecord',
)({
  authority: OpenAgentsPsionicAuthority,
  candidateModuleRefs: S.Array(S.String),
  createdAtIso: S.String,
  datasetRefs: S.Array(S.String),
  evidenceKind: OpenAgentsPsionicEvidenceKind,
  evidenceReceiptRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  fixtureRefs: S.Array(S.String),
  id: S.String,
  metricRefs: S.Array(S.String),
  modelRefs: S.Array(S.String),
  optimizerRefs: S.Array(S.String),
  promotionProposalRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  reviewRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  scorecardRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: OpenAgentsPsionicEvidenceStatus,
  trainingRunRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OpenAgentsPsionicEvidenceProjection extends S.Class<OpenAgentsPsionicEvidenceProjection>(
  'OpenAgentsPsionicEvidenceProjection',
)({
  audience: OmniProjectionAudience,
  authority: OpenAgentsPsionicAuthority,
  candidateModuleRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  datasetRefs: S.Array(S.String),
  evidenceKind: OpenAgentsPsionicEvidenceKind,
  evidenceOnly: S.Boolean,
  evidenceReceiptRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  fixtureRefs: S.Array(S.String),
  id: S.String,
  metricRefs: S.Array(S.String),
  modelRefs: S.Array(S.String),
  optimizerRefs: S.Array(S.String),
  promotionAllowed: S.Boolean,
  promotionProposalRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  reviewRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  routingMutationAllowed: S.Boolean,
  scorecardRefs: S.Array(S.String),
  settlementAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  status: OpenAgentsPsionicEvidenceStatus,
  trainingRunRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OpenAgentsPsionicEvidenceUnsafe extends S.TaggedErrorClass<OpenAgentsPsionicEvidenceUnsafe>()(
  'OpenAgentsPsionicEvidenceUnsafe',
  {
    reason: S.String,
  },
) {}

export const OPENAGENTS_PSIONIC_EVIDENCE_ONLY_AUTHORITY:
  OpenAgentsPsionicAuthority = {
    authorityBoundary: 'evidence_only',
    noAcceptedOutcomeSettlement: true,
    noDirectModulePromotion: true,
    noPayoutMutation: true,
    noPublicClaimUpgrade: true,
    noRoutingMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafePsionicRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|dataset\.(raw|private)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(auth|dataset|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(dataset\.private|failure\.operator|metric\.operator|provider\.private|review\.operator|source\.private)/i
const customerUnsafeRefPattern =
  /(dataset\.private|failure\.operator|metric\.operator|provider\.private|review\.operator|source\.private)/i
const teamUnsafeRefPattern =
  /(dataset\.private|provider\.private|source\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafePsionicRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsPsionicEvidenceUnsafe({
      reason: `${label} contains raw datasets, private customer data, provider payloads, secrets, wallet/payment material, raw source archives, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: typeof OmniProjectionAudience.Type,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const evidenceRefs = (
  record: OpenAgentsPsionicEvidenceRecord,
): ReadonlyArray<string> => [
  record.id,
  ...record.candidateModuleRefs,
  ...record.datasetRefs,
  ...record.evidenceReceiptRefs,
  ...record.failureRefs,
  ...record.fixtureRefs,
  ...record.metricRefs,
  ...record.modelRefs,
  ...record.optimizerRefs,
  ...record.promotionProposalRefs,
  ...record.providerRefs,
  ...record.retainedFailureRefs,
  ...record.reviewRefs,
  ...record.rollbackRefs,
  ...record.scorecardRefs,
  ...record.sourceRefs,
  ...record.trainingRunRefs,
]

const assertRecordSafe = (
  record: OpenAgentsPsionicEvidenceRecord,
): void => {
  assertSafeRefs('Psionic evidence refs', evidenceRefs(record))
}

export const openAgentsPsionicAuthorityIsEvidenceOnly = (
  authority: OpenAgentsPsionicAuthority,
): boolean =>
  authority.authorityBoundary === 'evidence_only' &&
  authority.noAcceptedOutcomeSettlement &&
  authority.noDirectModulePromotion &&
  authority.noPayoutMutation &&
  authority.noPublicClaimUpgrade &&
  authority.noRoutingMutation

export const openAgentsPsionicEvidenceCanMutateRuntime = (
  record: OpenAgentsPsionicEvidenceRecord,
): boolean => !openAgentsPsionicAuthorityIsEvidenceOnly(record.authority)

export const openAgentsPsionicEvidenceNeedsReview = (
  record: OpenAgentsPsionicEvidenceRecord,
): boolean =>
  record.status === 'needs_review' ||
  record.promotionProposalRefs.length > 0 ||
  record.rollbackRefs.length > 0 ||
  record.candidateModuleRefs.length > 0

export const projectOpenAgentsPsionicEvidence = (
  record: OpenAgentsPsionicEvidenceRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsPsionicEvidenceProjection => {
  assertRecordSafe(record)

  const evidenceOnly = openAgentsPsionicAuthorityIsEvidenceOnly(record.authority)
  const projection: OpenAgentsPsionicEvidenceProjection = {
    audience,
    authority: record.authority,
    candidateModuleRefs: safeRefsForAudience(
      'Psionic candidate module refs',
      record.candidateModuleRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    datasetRefs: safeRefsForAudience(
      'Psionic dataset refs',
      record.datasetRefs,
      audience,
    ),
    evidenceKind: record.evidenceKind,
    evidenceOnly,
    evidenceReceiptRefs: safeRefsForAudience(
      'Psionic evidence receipt refs',
      record.evidenceReceiptRefs,
      audience,
    ),
    failureRefs: safeRefsForAudience(
      'Psionic failure refs',
      record.failureRefs,
      audience,
    ),
    fixtureRefs: safeRefsForAudience(
      'Psionic fixture refs',
      record.fixtureRefs,
      audience,
    ),
    id: safeRefForAudience('Psionic evidence id', record.id, audience),
    metricRefs: safeRefsForAudience(
      'Psionic metric refs',
      record.metricRefs,
      audience,
    ),
    modelRefs: safeRefsForAudience(
      'Psionic model refs',
      record.modelRefs,
      audience,
    ),
    optimizerRefs: safeRefsForAudience(
      'Psionic optimizer refs',
      record.optimizerRefs,
      audience,
    ),
    promotionAllowed: !record.authority.noDirectModulePromotion,
    promotionProposalRefs: safeRefsForAudience(
      'Psionic promotion proposal refs',
      record.promotionProposalRefs,
      audience,
    ),
    providerRefs: safeRefsForAudience(
      'Psionic provider refs',
      record.providerRefs,
      audience,
    ),
    retainedFailureRefs: safeRefsForAudience(
      'Psionic retained failure refs',
      record.retainedFailureRefs,
      audience,
    ),
    reviewRefs: safeRefsForAudience(
      'Psionic review refs',
      record.reviewRefs,
      audience,
    ),
    rollbackRefs: safeRefsForAudience(
      'Psionic rollback refs',
      record.rollbackRefs,
      audience,
    ),
    routingMutationAllowed: !record.authority.noRoutingMutation,
    scorecardRefs: safeRefsForAudience(
      'Psionic scorecard refs',
      record.scorecardRefs,
      audience,
    ),
    settlementAllowed: !record.authority.noAcceptedOutcomeSettlement,
    sourceRefs: safeRefsForAudience(
      'Psionic source refs',
      record.sourceRefs,
      audience,
    ),
    status: record.status,
    trainingRunRefs: safeRefsForAudience(
      'Psionic training run refs',
      record.trainingRunRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (openAgentsPsionicEvidenceProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsPsionicEvidenceUnsafe({
      reason: 'Psionic evidence projection contains unsafe material.',
    })
  }

  return projection
}

export const openAgentsPsionicEvidenceProjectionHasPrivateMaterial = (
  projection: OpenAgentsPsionicEvidenceProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return unsafePsionicRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
}

export const OPENAGENTS_PSIONIC_CONFORMANCE_FIXTURES:
  ReadonlyArray<OpenAgentsPsionicEvidenceRecord> = [
    {
      authority: OPENAGENTS_PSIONIC_EVIDENCE_ONLY_AUTHORITY,
      candidateModuleRefs: ['candidate_module.continuation_v2'],
      createdAtIso: '2026-06-07T02:30:00.000Z',
      datasetRefs: ['dataset.public_eval_fixture'],
      evidenceKind: 'optimizer_run',
      evidenceReceiptRefs: ['receipt.psionic.evidence_1'],
      failureRefs: [],
      fixtureRefs: ['fixture.continuation.release_gate'],
      id: 'psionic_evidence.optimizer_1',
      metricRefs: ['metric.scorecard.acceptance_rate'],
      modelRefs: ['model.qwen.local_adapter'],
      optimizerRefs: ['optimizer.gepa_style_reflection'],
      promotionProposalRefs: ['promotion_proposal.requires_operator_review'],
      providerRefs: ['provider.local_model_safe'],
      retainedFailureRefs: ['retained_failure.probe.timeout_1'],
      reviewRefs: ['review.operator_required'],
      rollbackRefs: ['rollback.anchor.module_v1'],
      scorecardRefs: ['scorecard.continuation_candidate'],
      sourceRefs: ['source.public_fixture_summary'],
      status: 'needs_review',
      trainingRunRefs: ['training_run.psionic.small_fixture'],
      updatedAtIso: '2026-06-07T02:45:00.000Z',
    },
    {
      authority: OPENAGENTS_PSIONIC_EVIDENCE_ONLY_AUTHORITY,
      candidateModuleRefs: [],
      createdAtIso: '2026-06-07T02:40:00.000Z',
      datasetRefs: ['dataset.public_scorecard_fixture'],
      evidenceKind: 'scorecard',
      evidenceReceiptRefs: ['receipt.psionic.scorecard_1'],
      failureRefs: [],
      fixtureRefs: ['fixture.scorecard.public'],
      id: 'psionic_evidence.scorecard_1',
      metricRefs: ['metric.scorecard.public_quality'],
      modelRefs: ['model.local_scorecard_adapter'],
      optimizerRefs: [],
      promotionProposalRefs: [],
      providerRefs: ['provider.local_model_safe'],
      retainedFailureRefs: [],
      reviewRefs: [],
      rollbackRefs: [],
      scorecardRefs: ['scorecard.public_quality'],
      sourceRefs: ['source.public_fixture_summary'],
      status: 'completed',
      trainingRunRefs: [],
      updatedAtIso: '2026-06-07T02:50:00.000Z',
    },
  ]
