import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonAcceptedWorkPayoutRowPayoutClass = S.Literals([
  'modeled_reward',
  'payout_dispatch',
  'payout_eligibility',
  'payout_verification',
  'settled_payout',
])
export type PylonAcceptedWorkPayoutRowPayoutClass =
  typeof PylonAcceptedWorkPayoutRowPayoutClass.Type

export const PylonAcceptedWorkPayoutRowBasis = S.Literals([
  'accepted_work_reward',
  'operator_adjustment',
  'provider_capacity_reward',
  'revenue_share',
])
export type PylonAcceptedWorkPayoutRowBasis =
  typeof PylonAcceptedWorkPayoutRowBasis.Type

export const PylonAcceptedWorkPayoutRowWorkClass = S.Literals([
  'code_pr',
  'data_work',
  'forum_work',
  'other',
  'pylon_compute',
  'site_build',
  'training',
])
export type PylonAcceptedWorkPayoutRowWorkClass =
  typeof PylonAcceptedWorkPayoutRowWorkClass.Type

export const PylonAcceptedWorkPayoutRowProgressClass = S.Literals([
  'blocked',
  'confirmed',
  'dispatch_recorded',
  'eligible',
  'failed',
  'modeled',
  'settled',
  'skipped',
  'verified',
])
export type PylonAcceptedWorkPayoutRowProgressClass =
  typeof PylonAcceptedWorkPayoutRowProgressClass.Type

export const PylonAcceptedWorkPayoutRowSettlementState = S.Literals([
  'failed',
  'not_settled',
  'pending',
  'settled',
  'verified',
])
export type PylonAcceptedWorkPayoutRowSettlementState =
  typeof PylonAcceptedWorkPayoutRowSettlementState.Type

export const PylonAcceptedWorkPayoutRowVisibility = S.Literals([
  'private',
  'public',
])
export type PylonAcceptedWorkPayoutRowVisibility =
  typeof PylonAcceptedWorkPayoutRowVisibility.Type

export const PylonAcceptedWorkPayoutRowAuthorityBoundary = S.Literals([
  'read_only_public_projection',
])
export type PylonAcceptedWorkPayoutRowAuthorityBoundary =
  typeof PylonAcceptedWorkPayoutRowAuthorityBoundary.Type

export class PylonAcceptedWorkPayoutRowAuthority extends S.Class<PylonAcceptedWorkPayoutRowAuthority>(
  'PylonAcceptedWorkPayoutRowAuthority',
)({
  authorityBoundary: PylonAcceptedWorkPayoutRowAuthorityBoundary,
  noBuyerChargeMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class PylonAcceptedWorkPayoutRowRecord extends S.Class<PylonAcceptedWorkPayoutRowRecord>(
  'PylonAcceptedWorkPayoutRowRecord',
)({
  acceptedWorkRefs: S.Array(S.String),
  authority: PylonAcceptedWorkPayoutRowAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  confirmationRefs: S.Array(S.String),
  createdAtIso: S.String,
  dispatchRefs: S.Array(S.String),
  eligibilityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  linkRefs: S.Array(S.String),
  payoutBasis: PylonAcceptedWorkPayoutRowBasis,
  payoutClass: PylonAcceptedWorkPayoutRowPayoutClass,
  progressClass: PylonAcceptedWorkPayoutRowProgressClass,
  providerRef: S.String,
  providerVisibility: PylonAcceptedWorkPayoutRowVisibility,
  rewardIntentRefs: S.Array(S.String),
  rowRef: S.String,
  settlementRefs: S.Array(S.String),
  settlementState: PylonAcceptedWorkPayoutRowSettlementState,
  sourceRefs: S.Array(S.String),
  surfaceRefs: S.Array(S.String),
  updatedAtIso: S.String,
  verificationRefs: S.Array(S.String),
  workClass: PylonAcceptedWorkPayoutRowWorkClass,
}) {}

export class PylonAcceptedWorkPayoutRowProjection extends S.Class<PylonAcceptedWorkPayoutRowProjection>(
  'PylonAcceptedWorkPayoutRowProjection',
)({
  acceptedWorkClaimAllowed: S.Boolean,
  acceptedWorkRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: PylonAcceptedWorkPayoutRowAuthority,
  blockerRefs: S.Array(S.String),
  buyerChargeMutationAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  confirmationClaimAllowed: S.Boolean,
  confirmationRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dispatchClaimAllowed: S.Boolean,
  dispatchRefs: S.Array(S.String),
  eligibilityClaimAllowed: S.Boolean,
  eligibilityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  linkRefs: S.Array(S.String),
  liveWalletSpendAllowed: S.Boolean,
  payoutBasis: PylonAcceptedWorkPayoutRowBasis,
  payoutBasisLabel: S.String,
  payoutClass: PylonAcceptedWorkPayoutRowPayoutClass,
  payoutClassLabel: S.String,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutTargetMutationAllowed: S.Boolean,
  progressClass: PylonAcceptedWorkPayoutRowProgressClass,
  progressClassLabel: S.String,
  providerRef: S.String,
  providerVisibility: PylonAcceptedWorkPayoutRowVisibility,
  publicClaimUpgradeAllowed: S.Boolean,
  rewardIntentClaimAllowed: S.Boolean,
  rewardIntentRefs: S.Array(S.String),
  rowRef: S.String,
  settlementClaimAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  settlementState: PylonAcceptedWorkPayoutRowSettlementState,
  settlementStateLabel: S.String,
  sourceRefs: S.Array(S.String),
  surfaceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  verificationClaimAllowed: S.Boolean,
  verificationRefs: S.Array(S.String),
  workClass: PylonAcceptedWorkPayoutRowWorkClass,
  workClassLabel: S.String,
}) {}

export class PylonAcceptedWorkPayoutRowUnsafe extends S.TaggedErrorClass<PylonAcceptedWorkPayoutRowUnsafe>()(
  'PylonAcceptedWorkPayoutRowUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_ACCEPTED_WORK_PAYOUT_ROW_READ_ONLY_AUTHORITY:
  PylonAcceptedWorkPayoutRowAuthority = {
    authorityBoundary: 'read_only_public_projection',
    noBuyerChargeMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPayoutTargetMutation: true,
    noPublicClaimUpgrade: true,
    noSettlementMutation: true,
  }

const payoutClassLabelByClass:
  Readonly<Record<PylonAcceptedWorkPayoutRowPayoutClass, string>> = {
    modeled_reward: 'Modeled reward',
    payout_dispatch: 'Payout dispatch',
    payout_eligibility: 'Payout eligibility',
    payout_verification: 'Payout verification',
    settled_payout: 'Settled payout',
  }

const payoutBasisLabelByBasis:
  Readonly<Record<PylonAcceptedWorkPayoutRowBasis, string>> = {
    accepted_work_reward: 'Accepted-work reward',
    operator_adjustment: 'Operator adjustment',
    provider_capacity_reward: 'Provider capacity reward',
    revenue_share: 'Revenue share',
  }

const workClassLabelByClass:
  Readonly<Record<PylonAcceptedWorkPayoutRowWorkClass, string>> = {
    code_pr: 'Code PR',
    data_work: 'Data work',
    forum_work: 'Forum work',
    other: 'Other',
    pylon_compute: 'Pylon compute',
    site_build: 'Site build',
    training: 'Training',
  }

const progressClassLabelByClass:
  Readonly<Record<PylonAcceptedWorkPayoutRowProgressClass, string>> = {
    blocked: 'Blocked',
    confirmed: 'Confirmed',
    dispatch_recorded: 'Dispatch recorded',
    eligible: 'Eligible',
    failed: 'Failed',
    modeled: 'Modeled',
    settled: 'Settled',
    skipped: 'Skipped',
    verified: 'Verified',
  }

const settlementStateLabelByState:
  Readonly<Record<PylonAcceptedWorkPayoutRowSettlementState, string>> = {
    failed: 'Failed',
    not_settled: 'Not settled',
    pending: 'Pending',
    settled: 'Settled',
    verified: 'Verified',
  }

const progressRank:
  Partial<Record<PylonAcceptedWorkPayoutRowProgressClass, number>> = {
    confirmed: 4,
    dispatch_recorded: 3,
    eligible: 2,
    modeled: 1,
    settled: 6,
    verified: 5,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeAcceptedWorkPayoutRowRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw|target)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|invoice|payment|payload|payout|prompt|provider|runner|run[_-]?log|state|target|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(blocker\.private|caveat\.private|confirmation\.private|dispatch\.private|eligibility\.private|evidence\.private|link\.private|provider\.private|reward\.private|settlement\.private|source\.private|surface\.private|verification\.private)/i
const customerUnsafeRefPattern =
  /(blocker\.private|caveat\.private|confirmation\.private|dispatch\.private|eligibility\.private|evidence\.private|link\.private|provider\.private|reward\.private|settlement\.private|source\.private|surface\.private|verification\.private)/i
const teamUnsafeRefPattern =
  /(confirmation\.private|dispatch\.private|provider\.private|settlement\.private|verification\.private)/i

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
    unsafeAcceptedWorkPayoutRowRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonAcceptedWorkPayoutRowUnsafe({
      reason: `${label} contains private customer data, wallet material, raw bitcoin payment material, invoices, preimages, raw payout targets, private channel state, provider secrets, raw logs, private repo refs, or raw timestamps.`,
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

const visibleProviderRef = (
  record: PylonAcceptedWorkPayoutRowRecord,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (
    record.providerVisibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefsForAudience(
      'accepted-work payout row provider ref',
      [record.providerRef],
      audience,
    )[0] ?? 'provider.redacted'
  }

  return 'provider.redacted'
}

const progressAtLeast = (
  progress: PylonAcceptedWorkPayoutRowProgressClass,
  threshold: PylonAcceptedWorkPayoutRowProgressClass,
): boolean =>
  (progressRank[progress] ?? -1) >= (progressRank[threshold] ?? 999)

export const pylonAcceptedWorkPayoutRowHasNoMutationAuthority = (
  authority: PylonAcceptedWorkPayoutRowAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_public_projection' &&
  authority.noBuyerChargeMutation &&
  authority.noLiveWalletSpend &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetMutation &&
  authority.noPublicClaimUpgrade &&
  authority.noSettlementMutation

export const pylonAcceptedWorkPayoutRowCanUpgradePublicClaim = (
  record: PylonAcceptedWorkPayoutRowRecord,
): boolean => !pylonAcceptedWorkPayoutRowHasNoMutationAuthority(record.authority)

const assertIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new PylonAcceptedWorkPayoutRowUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertRecordSafe = (
  record: PylonAcceptedWorkPayoutRowRecord,
): void => {
  assertSafeRefs('accepted-work payout row identity refs', [
    record.id,
    record.rowRef,
    record.providerRef,
  ])
  assertSafeRefs(
    'accepted-work payout row accepted-work refs',
    record.acceptedWorkRefs,
  )
  assertSafeRefs('accepted-work payout row reward refs', record.rewardIntentRefs)
  assertSafeRefs('accepted-work payout row eligibility refs', record.eligibilityRefs)
  assertSafeRefs('accepted-work payout row dispatch refs', record.dispatchRefs)
  assertSafeRefs(
    'accepted-work payout row confirmation refs',
    record.confirmationRefs,
  )
  assertSafeRefs(
    'accepted-work payout row verification refs',
    record.verificationRefs,
  )
  assertSafeRefs('accepted-work payout row settlement refs', record.settlementRefs)
  assertSafeRefs('accepted-work payout row blocker refs', record.blockerRefs)
  assertSafeRefs('accepted-work payout row caveat refs', record.caveatRefs)
  assertSafeRefs('accepted-work payout row evidence refs', record.evidenceRefs)
  assertSafeRefs('accepted-work payout row source refs', record.sourceRefs)
  assertSafeRefs('accepted-work payout row link refs', record.linkRefs)
  assertSafeRefs('accepted-work payout row surface refs', record.surfaceRefs)
  assertIso('accepted-work payout row createdAtIso', record.createdAtIso)
  assertIso('accepted-work payout row updatedAtIso', record.updatedAtIso)

  if (!pylonAcceptedWorkPayoutRowHasNoMutationAuthority(record.authority)) {
    throw new PylonAcceptedWorkPayoutRowUnsafe({
      reason: 'Accepted-work payout rows are read-only and cannot carry buyer charge, wallet spend, payout dispatch, payout target, public claim upgrade, or settlement mutation authority.',
    })
  }

  const requiredProgressRefs:
    Readonly<Record<PylonAcceptedWorkPayoutRowProgressClass, ReadonlyArray<string>>> = {
      blocked: record.blockerRefs,
      confirmed: record.confirmationRefs,
      dispatch_recorded: record.dispatchRefs,
      eligible: record.eligibilityRefs,
      failed: record.blockerRefs,
      modeled: record.rewardIntentRefs,
      settled: record.settlementRefs,
      skipped: record.caveatRefs,
      verified: record.verificationRefs,
    }

  if (requiredProgressRefs[record.progressClass].length === 0) {
    throw new PylonAcceptedWorkPayoutRowUnsafe({
      reason: `${progressClassLabelByClass[record.progressClass]} payout row progress requires matching evidence refs.`,
    })
  }

  if (
    record.settlementState === 'settled' &&
    (record.settlementRefs.length === 0 || record.verificationRefs.length === 0)
  ) {
    throw new PylonAcceptedWorkPayoutRowUnsafe({
      reason: 'Settled payout rows require verification and settlement refs.',
    })
  }

  if (
    record.payoutClass === 'settled_payout' &&
    record.settlementState !== 'settled'
  ) {
    throw new PylonAcceptedWorkPayoutRowUnsafe({
      reason: 'Settled payout class requires settled settlement state.',
    })
  }
}

const projectionText = (
  projection: PylonAcceptedWorkPayoutRowProjection,
): string =>
  [
    projection.id,
    projection.rowRef,
    projection.providerRef,
    ...projection.acceptedWorkRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.confirmationRefs,
    ...projection.dispatchRefs,
    ...projection.eligibilityRefs,
    ...projection.evidenceRefs,
    ...projection.linkRefs,
    ...projection.rewardIntentRefs,
    ...projection.settlementRefs,
    ...projection.sourceRefs,
    ...projection.surfaceRefs,
    ...projection.verificationRefs,
  ].join(' ')

export const pylonAcceptedWorkPayoutRowProjectionHasPrivateMaterial = (
  projection: PylonAcceptedWorkPayoutRowProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeAcceptedWorkPayoutRowRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonAcceptedWorkPayoutRow = (
  record: PylonAcceptedWorkPayoutRowRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonAcceptedWorkPayoutRowProjection => {
  assertRecordSafe(record)

  const projection: PylonAcceptedWorkPayoutRowProjection = {
    acceptedWorkClaimAllowed: record.acceptedWorkRefs.length > 0,
    acceptedWorkRefs: safeRefsForAudience(
      'accepted-work payout row accepted-work refs',
      record.acceptedWorkRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'accepted-work payout row blocker refs',
      record.blockerRefs,
      audience,
    ),
    buyerChargeMutationAllowed: false,
    caveatRefs: safeRefsForAudience(
      'accepted-work payout row caveat refs',
      record.caveatRefs,
      audience,
    ),
    confirmationClaimAllowed:
      progressAtLeast(record.progressClass, 'confirmed') &&
      record.confirmationRefs.length > 0,
    confirmationRefs: safeRefsForAudience(
      'accepted-work payout row confirmation refs',
      record.confirmationRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    dispatchClaimAllowed:
      progressAtLeast(record.progressClass, 'dispatch_recorded') &&
      record.dispatchRefs.length > 0,
    dispatchRefs: safeRefsForAudience(
      'accepted-work payout row dispatch refs',
      record.dispatchRefs,
      audience,
    ),
    eligibilityClaimAllowed:
      progressAtLeast(record.progressClass, 'eligible') &&
      record.eligibilityRefs.length > 0,
    eligibilityRefs: safeRefsForAudience(
      'accepted-work payout row eligibility refs',
      record.eligibilityRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'accepted-work payout row evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: safeRefsForAudience('accepted-work payout row id', [record.id], audience)[0] ??
      'payout_row.redacted',
    linkRefs: safeRefsForAudience(
      'accepted-work payout row link refs',
      record.linkRefs,
      audience,
    ),
    liveWalletSpendAllowed: false,
    payoutBasis: record.payoutBasis,
    payoutBasisLabel: payoutBasisLabelByBasis[record.payoutBasis],
    payoutClass: record.payoutClass,
    payoutClassLabel: payoutClassLabelByClass[record.payoutClass],
    payoutDispatchMutationAllowed: false,
    payoutTargetMutationAllowed: false,
    progressClass: record.progressClass,
    progressClassLabel: progressClassLabelByClass[record.progressClass],
    providerRef: visibleProviderRef(record, audience),
    providerVisibility: record.providerVisibility,
    publicClaimUpgradeAllowed: false,
    rewardIntentClaimAllowed:
      progressAtLeast(record.progressClass, 'modeled') &&
      record.rewardIntentRefs.length > 0,
    rewardIntentRefs: safeRefsForAudience(
      'accepted-work payout row reward refs',
      record.rewardIntentRefs,
      audience,
    ),
    rowRef: record.rowRef,
    settlementClaimAllowed:
      record.progressClass === 'settled' &&
      record.settlementState === 'settled' &&
      record.settlementRefs.length > 0 &&
      record.verificationRefs.length > 0,
    settlementMutationAllowed: false,
    settlementRefs: safeRefsForAudience(
      'accepted-work payout row settlement refs',
      record.settlementRefs,
      audience,
    ),
    settlementState: record.settlementState,
    settlementStateLabel: settlementStateLabelByState[record.settlementState],
    sourceRefs: safeRefsForAudience(
      'accepted-work payout row source refs',
      record.sourceRefs,
      audience,
    ),
    surfaceRefs: safeRefsForAudience(
      'accepted-work payout row surface refs',
      record.surfaceRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    verificationClaimAllowed:
      progressAtLeast(record.progressClass, 'verified') &&
      record.verificationRefs.length > 0,
    verificationRefs: safeRefsForAudience(
      'accepted-work payout row verification refs',
      record.verificationRefs,
      audience,
    ),
    workClass: record.workClass,
    workClassLabel: workClassLabelByClass[record.workClass],
  }

  if (pylonAcceptedWorkPayoutRowProjectionHasPrivateMaterial(projection)) {
    throw new PylonAcceptedWorkPayoutRowUnsafe({
      reason: 'Accepted-work payout row projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_ACCEPTED_WORK_PAYOUT_ROW_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonAcceptedWorkPayoutRowRecord> = [
    {
      acceptedWorkRefs: ['accepted_work.public.site_otc_revision_3'],
      authority: PYLON_ACCEPTED_WORK_PAYOUT_ROW_READ_ONLY_AUTHORITY,
      blockerRefs: [],
      caveatRefs: ['caveat.public.row_links_settlement_receipt_only'],
      confirmationRefs: [
        'confirmation.public.site_otc_revision_3',
        'confirmation.private.operator_trace',
      ],
      createdAtIso: '2026-06-07T09:00:00.000Z',
      dispatchRefs: [
        'dispatch.public.site_otc_revision_3',
        'dispatch.private.operator_trace',
      ],
      eligibilityRefs: ['eligibility.public.site_otc_revision_3'],
      evidenceRefs: ['evidence.public.site_otc_revision_3'],
      id: 'payout_row.site_otc_revision_3',
      linkRefs: [
        'link.public.site_order.otc',
        'link.public.proof.otc',
        'link.private.operator_console.otc',
      ],
      payoutBasis: 'accepted_work_reward',
      payoutClass: 'settled_payout',
      progressClass: 'settled',
      providerRef: 'provider.private.pylon_1',
      providerVisibility: 'private',
      rewardIntentRefs: ['reward.public.site_otc_revision_3'],
      rowRef: 'payout_row.public.site_otc_revision_3',
      settlementRefs: ['settlement.public.site_otc_revision_3'],
      settlementState: 'settled',
      sourceRefs: ['source.public.nexus_treasury_projection'],
      surfaceRefs: [
        'surface.public.site_order.otc',
        'surface.public.public_proof.otc',
      ],
      updatedAtIso: '2026-06-07T09:05:00.000Z',
      verificationRefs: [
        'verification.public.site_otc_revision_3',
        'verification.private.operator_trace',
      ],
      workClass: 'site_build',
    },
  ]
