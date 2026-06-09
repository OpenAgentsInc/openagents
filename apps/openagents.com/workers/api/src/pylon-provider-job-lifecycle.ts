import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const PylonProviderJobStage = S.Literals([
  'accepted',
  'artifact_produced',
  'assigned',
  'blocked',
  'cancelled',
  'failed',
  'offered',
  'payout_confirmed',
  'payout_dispatched',
  'payout_verified',
  'reward_intent_recorded',
  'running',
  'settled',
])
export type PylonProviderJobStage = typeof PylonProviderJobStage.Type

export const PylonProviderJobProviderVisibility = S.Literals([
  'private',
  'public',
])
export type PylonProviderJobProviderVisibility =
  typeof PylonProviderJobProviderVisibility.Type

export class PylonProviderJobLifecycleRecord extends S.Class<PylonProviderJobLifecycleRecord>(
  'PylonProviderJobLifecycleRecord',
)({
  acceptanceRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  buyerPaymentEvidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  jobRef: S.String,
  offerRefs: S.Array(S.String),
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchRefs: S.Array(S.String),
  payoutVerificationRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: PylonProviderJobProviderVisibility,
  rewardIntentRefs: S.Array(S.String),
  runRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  stage: PylonProviderJobStage,
  updatedAtIso: S.String,
  workClassRef: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class PylonProviderJobLifecycleProjection extends S.Class<PylonProviderJobLifecycleProjection>(
  'PylonProviderJobLifecycleProjection',
)({
  acceptanceRefs: S.Array(S.String),
  acceptedWorkClaimAllowed: S.Boolean,
  artifactRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  blockerRefs: S.Array(S.String),
  buyerPaymentEvidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  jobRef: S.String,
  offerRefs: S.Array(S.String),
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchClaimAllowed: S.Boolean,
  payoutDispatchRefs: S.Array(S.String),
  payoutVerificationRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: PylonProviderJobProviderVisibility,
  rewardIntentClaimAllowed: S.Boolean,
  rewardIntentRefs: S.Array(S.String),
  runRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  stage: PylonProviderJobStage,
  stageLabel: S.String,
  updatedAtDisplay: S.String,
  workClassRef: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class PylonProviderJobLifecycleUnsafe extends S.TaggedErrorClass<PylonProviderJobLifecycleUnsafe>()(
  'PylonProviderJobLifecycleUnsafe',
  {
    reason: S.String,
  },
) {}

const stageRank: Record<PylonProviderJobStage, number> = {
  accepted: 4,
  artifact_produced: 3,
  assigned: 1,
  blocked: -1,
  cancelled: -1,
  failed: -1,
  offered: 0,
  payout_confirmed: 7,
  payout_dispatched: 6,
  payout_verified: 8,
  reward_intent_recorded: 5,
  running: 2,
  settled: 9,
}

const stageLabelByStage: Record<PylonProviderJobStage, string> = {
  accepted: 'Accepted',
  artifact_produced: 'Artifact produced',
  assigned: 'Assigned',
  blocked: 'Blocked',
  cancelled: 'Cancelled',
  failed: 'Failed',
  offered: 'Offered',
  payout_confirmed: 'Payout confirmed',
  payout_dispatched: 'Payout dispatched',
  payout_verified: 'Payout verified',
  reward_intent_recorded: 'Reward intent recorded',
  running: 'Running',
  settled: 'Settled',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage)|payout[_-]?(target|address|destination)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(buyer[_-]?payment|payout[_-]?(confirmation|dispatch|verification)|provider\.private|settlement\.private|workroom\.)/i
const customerUnsafeRefPattern =
  /(buyer[_-]?payment|payout[_-]?(confirmation|dispatch|verification)|provider\.private|settlement\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(buyer[_-]?payment|provider\.private|settlement\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stageAtLeast = (
  stage: PylonProviderJobStage,
  threshold: PylonProviderJobStage,
): boolean => stageRank[stage] >= stageRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, payout target, private repo, customer, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
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
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const providerRefForAudience = (
  record: PylonProviderJobLifecycleRecord,
  audience: BlueprintMissionBriefingAudience,
): string => {
  if (record.providerVisibility === 'public' || audience === 'operator') {
    return safeRefsForAudience('provider ref', [record.providerRef], audience)[0] ??
      'provider.redacted'
  }

  return 'provider.redacted'
}

const assertRecordSafe = (
  record: PylonProviderJobLifecycleRecord,
): void => {
  assertSafeRefs('provider job identity refs', [
    record.id,
    record.jobRef,
    record.providerRef,
    record.workClassRef,
  ])
  assertSafeRefs('provider job offer refs', record.offerRefs)
  assertSafeRefs('provider job assignment refs', record.assignmentRefs)
  assertSafeRefs('provider job run refs', record.runRefs)
  assertSafeRefs('provider job artifact refs', record.artifactRefs)
  assertSafeRefs('provider job acceptance refs', record.acceptanceRefs)
  assertSafeRefs('provider job reward intent refs', record.rewardIntentRefs)
  assertSafeRefs('provider job payout dispatch refs', record.payoutDispatchRefs)
  assertSafeRefs(
    'provider job payout confirmation refs',
    record.payoutConfirmationRefs,
  )
  assertSafeRefs(
    'provider job payout verification refs',
    record.payoutVerificationRefs,
  )
  assertSafeRefs('provider job settlement refs', record.settlementRefs)
  assertSafeRefs(
    'provider job buyer payment evidence refs',
    record.buyerPaymentEvidenceRefs,
  )
  assertSafeRefs('provider job blocker refs', record.blockerRefs)
  assertSafeRefs('provider job caveat refs', record.caveatRefs)
  assertSafeRefs('provider job evidence refs', record.evidenceRefs)
  assertSafeRefs('provider job workroom refs', record.workroomRefs)

  if (record.stage === 'blocked' && record.blockerRefs.length === 0) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Blocked provider jobs require blocker refs.',
    })
  }

  if (stageAtLeast(record.stage, 'artifact_produced') && record.artifactRefs.length === 0) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Artifact-producing provider jobs require artifact refs.',
    })
  }

  if (stageAtLeast(record.stage, 'accepted') && record.acceptanceRefs.length === 0) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Accepted provider jobs require acceptance refs.',
    })
  }

  if (
    stageAtLeast(record.stage, 'reward_intent_recorded') &&
    record.rewardIntentRefs.length === 0
  ) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Reward-intent provider jobs require reward intent refs.',
    })
  }

  if (
    stageAtLeast(record.stage, 'payout_dispatched') &&
    record.payoutDispatchRefs.length === 0
  ) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Payout-dispatched provider jobs require payout dispatch refs.',
    })
  }

  if (
    stageAtLeast(record.stage, 'payout_confirmed') &&
    record.payoutConfirmationRefs.length === 0
  ) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Payout-confirmed provider jobs require payout confirmation refs.',
    })
  }

  if (
    stageAtLeast(record.stage, 'payout_verified') &&
    record.payoutVerificationRefs.length === 0
  ) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Payout-verified provider jobs require payout verification refs.',
    })
  }

  if (record.stage === 'settled' && record.settlementRefs.length === 0) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Settled provider jobs require settlement refs.',
    })
  }
}

const projectionText = (
  projection: PylonProviderJobLifecycleProjection,
): string =>
  [
    projection.id,
    projection.jobRef,
    projection.providerRef,
    projection.workClassRef,
    ...projection.offerRefs,
    ...projection.assignmentRefs,
    ...projection.runRefs,
    ...projection.artifactRefs,
    ...projection.acceptanceRefs,
    ...projection.rewardIntentRefs,
    ...projection.payoutDispatchRefs,
    ...projection.payoutConfirmationRefs,
    ...projection.payoutVerificationRefs,
    ...projection.settlementRefs,
    ...projection.buyerPaymentEvidenceRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.evidenceRefs,
    ...projection.workroomRefs,
  ].join(' ')

export const pylonProviderJobProjectionHasPrivateMaterial = (
  projection: PylonProviderJobLifecycleProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonProviderJobLifecycle = (
  record: PylonProviderJobLifecycleRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonProviderJobLifecycleProjection => {
  assertRecordSafe(record)

  const acceptedWorkClaimAllowed = stageAtLeast(record.stage, 'accepted') &&
    record.acceptanceRefs.length > 0
  const rewardIntentClaimAllowed =
    stageAtLeast(record.stage, 'reward_intent_recorded') &&
    record.rewardIntentRefs.length > 0
  const payoutDispatchClaimAllowed =
    stageAtLeast(record.stage, 'payout_dispatched') &&
    record.payoutDispatchRefs.length > 0
  const settlementClaimAllowed = record.stage === 'settled' &&
    record.settlementRefs.length > 0 &&
    record.payoutVerificationRefs.length > 0
  const projection: PylonProviderJobLifecycleProjection = {
    acceptanceRefs: safeRefsForAudience(
      'provider job acceptance refs',
      record.acceptanceRefs,
      audience,
    ),
    acceptedWorkClaimAllowed,
    artifactRefs: safeRefsForAudience(
      'provider job artifact refs',
      record.artifactRefs,
      audience,
    ),
    assignmentRefs: safeRefsForAudience(
      'provider job assignment refs',
      record.assignmentRefs,
      audience,
    ),
    audience,
    blockerRefs: safeRefsForAudience(
      'provider job blocker refs',
      record.blockerRefs,
      audience,
    ),
    buyerPaymentEvidenceRefs: audience === 'operator'
      ? safeRefsForAudience(
        'provider job buyer payment evidence refs',
        record.buyerPaymentEvidenceRefs,
        audience,
      )
      : [],
    caveatRefs: safeRefsForAudience(
      'provider job caveat refs',
      record.caveatRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'provider job evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: record.id,
    jobRef: record.jobRef,
    offerRefs: safeRefsForAudience(
      'provider job offer refs',
      record.offerRefs,
      audience,
    ),
    payoutConfirmationRefs: audience === 'operator'
      ? safeRefsForAudience(
        'provider job payout confirmation refs',
        record.payoutConfirmationRefs,
        audience,
      )
      : [],
    payoutDispatchClaimAllowed,
    payoutDispatchRefs: audience === 'operator'
      ? safeRefsForAudience(
        'provider job payout dispatch refs',
        record.payoutDispatchRefs,
        audience,
      )
      : [],
    payoutVerificationRefs: audience === 'operator'
      ? safeRefsForAudience(
        'provider job payout verification refs',
        record.payoutVerificationRefs,
        audience,
      )
      : [],
    providerRef: providerRefForAudience(record, audience),
    providerVisibility: record.providerVisibility,
    rewardIntentClaimAllowed,
    rewardIntentRefs: safeRefsForAudience(
      'provider job reward intent refs',
      record.rewardIntentRefs,
      audience,
    ),
    runRefs: safeRefsForAudience(
      'provider job run refs',
      record.runRefs,
      audience,
    ),
    settlementClaimAllowed,
    settlementRefs: audience === 'operator'
      ? safeRefsForAudience(
        'provider job settlement refs',
        record.settlementRefs,
        audience,
      )
      : [],
    stage: record.stage,
    stageLabel: stageLabelByStage[record.stage],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workClassRef: record.workClassRef,
    workroomRefs: audience === 'public'
      ? []
      : safeRefsForAudience(
        'provider job workroom refs',
        record.workroomRefs,
        audience,
      ),
  }

  if (pylonProviderJobProjectionHasPrivateMaterial(projection)) {
    throw new PylonProviderJobLifecycleUnsafe({
      reason: 'Provider job projection contains private material.',
    })
  }

  return projection
}

export const examplePylonProviderJobLifecycleRecord =
  (): PylonProviderJobLifecycleRecord => ({
    acceptanceRefs: ['acceptance.pylon_job.otel_trace_summarizer'],
    artifactRefs: ['artifact.pylon_job.trace_summary'],
    assignmentRefs: ['assignment.pylon_job.trace_summary'],
    blockerRefs: [],
    buyerPaymentEvidenceRefs: ['buyer_payment_evidence.omega_internal_budget'],
    caveatRefs: ['caveat.payout.not_settlement_until_receipt'],
    evidenceRefs: ['evidence.pylon_job.lifecycle_smoke'],
    id: 'pylon_provider_job_trace_summary_1',
    jobRef: 'pylon_job.trace_summary_1',
    offerRefs: ['offer.pylon.trace_summary_1'],
    payoutConfirmationRefs: ['payout_confirmation.trace_summary_1'],
    payoutDispatchRefs: ['payout_dispatch.trace_summary_1'],
    payoutVerificationRefs: ['payout_verification.trace_summary_1'],
    providerRef: 'provider.pylon_public_demo',
    providerVisibility: 'public',
    rewardIntentRefs: ['reward_intent.trace_summary_1'],
    runRefs: ['run.pylon.trace_summary_1'],
    settlementRefs: ['settlement.trace_summary_1'],
    stage: 'settled',
    updatedAtIso: '2026-06-06T21:20:00.000Z',
    workClassRef: 'work_class.flexible_inference_summary',
    workroomRefs: ['workroom.pylon_trace_summary'],
  })
