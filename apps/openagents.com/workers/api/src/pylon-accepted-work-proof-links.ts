import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonAcceptedWorkProofLinkState = S.Literals([
  'accepted_work',
  'blocked',
  'payout_confirmed',
  'payout_dispatched',
  'payout_eligible',
  'payout_verified',
  'reward_intent',
  'settled',
])
export type PylonAcceptedWorkProofLinkState =
  typeof PylonAcceptedWorkProofLinkState.Type

export const PylonAcceptedWorkProofLinkSurface = S.Literals([
  'agent_api',
  'customer_dashboard',
  'operator_receipt',
  'public_proof',
  'site_order',
])
export type PylonAcceptedWorkProofLinkSurface =
  typeof PylonAcceptedWorkProofLinkSurface.Type

export const PylonAcceptedWorkProofLinkVisibility = S.Literals([
  'private',
  'public',
])
export type PylonAcceptedWorkProofLinkVisibility =
  typeof PylonAcceptedWorkProofLinkVisibility.Type

export const PylonAcceptedWorkProofLinkAuthorityBoundary = S.Literals([
  'read_only_receipt_link',
])
export type PylonAcceptedWorkProofLinkAuthorityBoundary =
  typeof PylonAcceptedWorkProofLinkAuthorityBoundary.Type

export class PylonAcceptedWorkProofLinkAuthority extends S.Class<PylonAcceptedWorkProofLinkAuthority>(
  'PylonAcceptedWorkProofLinkAuthority',
)({
  authorityBoundary: PylonAcceptedWorkProofLinkAuthorityBoundary,
  noAcceptedWorkMutation: S.Boolean,
  noBuyerChargeMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetDisclosure: S.Boolean,
  noProviderEligibilityMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
  noSiteReleaseMutation: S.Boolean,
}) {}

export class PylonAcceptedWorkProofLinkRecord extends S.Class<PylonAcceptedWorkProofLinkRecord>(
  'PylonAcceptedWorkProofLinkRecord',
)({
  acceptedWorkRefs: S.Array(S.String),
  authority: PylonAcceptedWorkProofLinkAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  consumerSurfaces: S.Array(PylonAcceptedWorkProofLinkSurface),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  orderRefs: S.Array(S.String),
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchRefs: S.Array(S.String),
  payoutEligibilityRefs: S.Array(S.String),
  payoutRowRefs: S.Array(S.String),
  payoutSloRefs: S.Array(S.String),
  payoutVerificationRefs: S.Array(S.String),
  providerJobRefs: S.Array(S.String),
  providerPayoutClaimIncluded: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonAcceptedWorkProofLinkVisibility,
  publicProofRefs: S.Array(S.String),
  receiptLinkRefs: S.Array(S.String),
  rewardIntentRefs: S.Array(S.String),
  settlementBridgeRefs: S.Array(S.String),
  settlementEvidenceRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  siteRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PylonAcceptedWorkProofLinkState,
  updatedAtIso: S.String,
  versionRefs: S.Array(S.String),
}) {}

export class PylonAcceptedWorkProofLinkProjection extends S.Class<PylonAcceptedWorkProofLinkProjection>(
  'PylonAcceptedWorkProofLinkProjection',
)({
  acceptedWorkClaimAllowed: S.Boolean,
  acceptedWorkMutationAllowed: S.Boolean,
  acceptedWorkRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: PylonAcceptedWorkProofLinkAuthority,
  blockerRefs: S.Array(S.String),
  buyerChargeMutationAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  consumerSurfaces: S.Array(PylonAcceptedWorkProofLinkSurface),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  orderRefs: S.Array(S.String),
  payoutConfirmationClaimAllowed: S.Boolean,
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchClaimAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutDispatchRefs: S.Array(S.String),
  payoutEligibilityClaimAllowed: S.Boolean,
  payoutEligibilityRefs: S.Array(S.String),
  payoutRowRefs: S.Array(S.String),
  payoutSloRefs: S.Array(S.String),
  payoutTargetDisclosureAllowed: S.Boolean,
  payoutVerificationClaimAllowed: S.Boolean,
  payoutVerificationRefs: S.Array(S.String),
  providerEligibilityMutationAllowed: S.Boolean,
  providerJobRefs: S.Array(S.String),
  providerPayoutClaimIncluded: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonAcceptedWorkProofLinkVisibility,
  publicProofRefs: S.Array(S.String),
  receiptLinkRefs: S.Array(S.String),
  rewardIntentClaimAllowed: S.Boolean,
  rewardIntentRefs: S.Array(S.String),
  settlementBridgeRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementEvidenceRefs: S.Array(S.String),
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  siteRefs: S.Array(S.String),
  siteReleaseMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: PylonAcceptedWorkProofLinkState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  versionRefs: S.Array(S.String),
}) {}

export class PylonAcceptedWorkProofLinkUnsafe extends S.TaggedErrorClass<PylonAcceptedWorkProofLinkUnsafe>()(
  'PylonAcceptedWorkProofLinkUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_ACCEPTED_WORK_PROOF_LINK_READ_ONLY_AUTHORITY:
  PylonAcceptedWorkProofLinkAuthority = {
    authorityBoundary: 'read_only_receipt_link',
    noAcceptedWorkMutation: true,
    noBuyerChargeMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPayoutTargetDisclosure: true,
    noProviderEligibilityMutation: true,
    noSettlementMutation: true,
    noSiteReleaseMutation: true,
  }

const stateRank: Readonly<Record<PylonAcceptedWorkProofLinkState, number>> = {
  accepted_work: 1,
  blocked: -1,
  payout_confirmed: 5,
  payout_dispatched: 4,
  payout_eligible: 3,
  payout_verified: 6,
  reward_intent: 2,
  settled: 7,
}

const stateLabelByState:
  Readonly<Record<PylonAcceptedWorkProofLinkState, string>> = {
    accepted_work: 'Accepted work',
    blocked: 'Blocked',
    payout_confirmed: 'Payout confirmed',
    payout_dispatched: 'Payout dispatched',
    payout_eligible: 'Payout eligible',
    payout_verified: 'Payout verified',
    reward_intent: 'Reward intent',
    settled: 'Settled',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeAcceptedWorkProofLinkRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth[_-]?content[_-]?json|auth\.json|bearer|bolt11|bolt12|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw|target)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|email|invoice|payment|payload|payout|prompt|provider|runner|run[_-]?log|source[_-]?archive|state|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(blocker\.private|caveat\.private|confirmation\.private|dispatch\.private|eligibility\.private|evidence\.private|job\.private|link\.private|order\.private|payout\.private|provider\.private|receipt\.private|reward\.private|settlement\.private|site\.private|source\.private|slo\.private|verification\.private|version\.private)/i
const customerUnsafeRefPattern =
  /(blocker\.private|caveat\.private|confirmation\.private|dispatch\.private|eligibility\.private|evidence\.private|job\.private|link\.private|order\.private|payout\.private|provider\.private|receipt\.private|reward\.private|settlement\.private|site\.private|source\.private|slo\.private|verification\.private|version\.private)/i
const teamUnsafeRefPattern =
  /(confirmation\.private|dispatch\.private|job\.private|payout\.private|provider\.private|receipt\.private|settlement\.private|slo\.private|verification\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stateAtLeast = (
  state: PylonAcceptedWorkProofLinkState,
  threshold: PylonAcceptedWorkProofLinkState,
): boolean => stateRank[state] >= stateRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeAcceptedWorkProofLinkRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonAcceptedWorkProofLinkUnsafe({
      reason: `${label} contains private customer data, wallet material, raw bitcoin payment material, invoices, preimages, raw payout targets, private channel state, provider secrets, raw logs, private repo refs, credentials, or raw timestamps.`,
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

const providerRefForAudience = (
  record: PylonAcceptedWorkProofLinkRecord,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (
    record.providerVisibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefsForAudience('accepted-work proof provider ref', [
      record.providerRef,
    ], audience)[0] ?? 'provider.redacted'
  }

  return 'provider.redacted'
}

export const pylonAcceptedWorkProofLinkHasNoMutationAuthority = (
  authority: PylonAcceptedWorkProofLinkAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_receipt_link' &&
  authority.noAcceptedWorkMutation &&
  authority.noBuyerChargeMutation &&
  authority.noLiveWalletSpend &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetDisclosure &&
  authority.noProviderEligibilityMutation &&
  authority.noSettlementMutation &&
  authority.noSiteReleaseMutation

export const pylonAcceptedWorkProofLinkCanMutateSettlement = (
  record: PylonAcceptedWorkProofLinkRecord,
): boolean => !pylonAcceptedWorkProofLinkHasNoMutationAuthority(record.authority)

const assertIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new PylonAcceptedWorkProofLinkUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertRecordSafe = (
  record: PylonAcceptedWorkProofLinkRecord,
): void => {
  assertSafeRefs('accepted-work proof identity refs', [
    record.id,
    record.providerRef,
  ])
  assertSafeRefs('accepted-work proof accepted-work refs', record.acceptedWorkRefs)
  assertSafeRefs('accepted-work proof blocker refs', record.blockerRefs)
  assertSafeRefs('accepted-work proof caveat refs', record.caveatRefs)
  assertSafeRefs('accepted-work proof evidence refs', record.evidenceRefs)
  assertSafeRefs('accepted-work proof order refs', record.orderRefs)
  assertSafeRefs(
    'accepted-work proof payout confirmation refs',
    record.payoutConfirmationRefs,
  )
  assertSafeRefs(
    'accepted-work proof payout dispatch refs',
    record.payoutDispatchRefs,
  )
  assertSafeRefs(
    'accepted-work proof payout eligibility refs',
    record.payoutEligibilityRefs,
  )
  assertSafeRefs('accepted-work proof payout row refs', record.payoutRowRefs)
  assertSafeRefs('accepted-work proof payout SLO refs', record.payoutSloRefs)
  assertSafeRefs(
    'accepted-work proof payout verification refs',
    record.payoutVerificationRefs,
  )
  assertSafeRefs('accepted-work proof provider job refs', record.providerJobRefs)
  assertSafeRefs('accepted-work proof public proof refs', record.publicProofRefs)
  assertSafeRefs('accepted-work proof receipt link refs', record.receiptLinkRefs)
  assertSafeRefs('accepted-work proof reward intent refs', record.rewardIntentRefs)
  assertSafeRefs(
    'accepted-work proof settlement bridge refs',
    record.settlementBridgeRefs,
  )
  assertSafeRefs(
    'accepted-work proof settlement evidence refs',
    record.settlementEvidenceRefs,
  )
  assertSafeRefs('accepted-work proof settlement refs', record.settlementRefs)
  assertSafeRefs('accepted-work proof site refs', record.siteRefs)
  assertSafeRefs('accepted-work proof source refs', record.sourceRefs)
  assertSafeRefs('accepted-work proof version refs', record.versionRefs)
  assertIso('accepted-work proof createdAtIso', record.createdAtIso)
  assertIso('accepted-work proof updatedAtIso', record.updatedAtIso)

  if (!pylonAcceptedWorkProofLinkHasNoMutationAuthority(record.authority)) {
    throw new PylonAcceptedWorkProofLinkUnsafe({
      reason: 'Accepted-work proof links are read-only and cannot carry accepted-work, buyer charge, wallet spend, payout dispatch, payout target, provider eligibility, settlement, or Site release mutation authority.',
    })
  }

  if (record.providerPayoutClaimIncluded && record.providerJobRefs.length === 0) {
    throw new PylonAcceptedWorkProofLinkUnsafe({
      reason: 'Provider payout proof links require provider job refs.',
    })
  }

  const requiredRefs:
    Readonly<Record<PylonAcceptedWorkProofLinkState, ReadonlyArray<string>>> = {
      accepted_work: record.acceptedWorkRefs,
      blocked: record.blockerRefs,
      payout_confirmed: record.payoutConfirmationRefs,
      payout_dispatched: record.payoutDispatchRefs,
      payout_eligible: record.payoutEligibilityRefs,
      payout_verified: record.payoutVerificationRefs,
      reward_intent: record.rewardIntentRefs,
      settled: record.settlementRefs,
    }

  if (requiredRefs[record.state].length === 0) {
    throw new PylonAcceptedWorkProofLinkUnsafe({
      reason: `${stateLabelByState[record.state]} proof link state requires matching refs.`,
    })
  }

  if (
    record.state === 'settled' &&
    (record.settlementEvidenceRefs.length === 0 ||
      record.payoutVerificationRefs.length === 0)
  ) {
    throw new PylonAcceptedWorkProofLinkUnsafe({
      reason: 'Settled accepted-work proof links require payout verification and settlement evidence refs.',
    })
  }
}

const projectionText = (
  projection: PylonAcceptedWorkProofLinkProjection,
): string =>
  [
    projection.id,
    projection.providerRef,
    ...projection.acceptedWorkRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.evidenceRefs,
    ...projection.orderRefs,
    ...projection.payoutConfirmationRefs,
    ...projection.payoutDispatchRefs,
    ...projection.payoutEligibilityRefs,
    ...projection.payoutRowRefs,
    ...projection.payoutSloRefs,
    ...projection.payoutVerificationRefs,
    ...projection.providerJobRefs,
    ...projection.publicProofRefs,
    ...projection.receiptLinkRefs,
    ...projection.rewardIntentRefs,
    ...projection.settlementBridgeRefs,
    ...projection.settlementEvidenceRefs,
    ...projection.settlementRefs,
    ...projection.siteRefs,
    ...projection.sourceRefs,
    ...projection.versionRefs,
  ].join(' ')

export const pylonAcceptedWorkProofLinkProjectionHasPrivateMaterial = (
  projection: PylonAcceptedWorkProofLinkProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeAcceptedWorkProofLinkRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonAcceptedWorkProofLink = (
  record: PylonAcceptedWorkProofLinkRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonAcceptedWorkProofLinkProjection => {
  assertRecordSafe(record)

  const projection: PylonAcceptedWorkProofLinkProjection = {
    acceptedWorkClaimAllowed:
      stateAtLeast(record.state, 'accepted_work') &&
      record.acceptedWorkRefs.length > 0,
    acceptedWorkMutationAllowed: false,
    acceptedWorkRefs: safeRefsForAudience(
      'accepted-work proof accepted-work refs',
      record.acceptedWorkRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'accepted-work proof blocker refs',
      record.blockerRefs,
      audience,
    ),
    buyerChargeMutationAllowed: false,
    caveatRefs: safeRefsForAudience(
      'accepted-work proof caveat refs',
      record.caveatRefs,
      audience,
    ),
    consumerSurfaces: [...new Set(record.consumerSurfaces)].sort(),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'accepted-work proof evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: safeRefsForAudience('accepted-work proof id', [record.id], audience)[0] ??
      'accepted_work_proof_link.redacted',
    liveWalletSpendAllowed: false,
    orderRefs: safeRefsForAudience(
      'accepted-work proof order refs',
      record.orderRefs,
      audience,
    ),
    payoutConfirmationClaimAllowed:
      stateAtLeast(record.state, 'payout_confirmed') &&
      record.payoutConfirmationRefs.length > 0,
    payoutConfirmationRefs: safeRefsForAudience(
      'accepted-work proof payout confirmation refs',
      record.payoutConfirmationRefs,
      audience,
    ),
    payoutDispatchClaimAllowed:
      stateAtLeast(record.state, 'payout_dispatched') &&
      record.payoutDispatchRefs.length > 0,
    payoutDispatchMutationAllowed: false,
    payoutDispatchRefs: safeRefsForAudience(
      'accepted-work proof payout dispatch refs',
      record.payoutDispatchRefs,
      audience,
    ),
    payoutEligibilityClaimAllowed:
      stateAtLeast(record.state, 'payout_eligible') &&
      record.payoutEligibilityRefs.length > 0,
    payoutEligibilityRefs: safeRefsForAudience(
      'accepted-work proof payout eligibility refs',
      record.payoutEligibilityRefs,
      audience,
    ),
    payoutRowRefs: safeRefsForAudience(
      'accepted-work proof payout row refs',
      record.payoutRowRefs,
      audience,
    ),
    payoutSloRefs: safeRefsForAudience(
      'accepted-work proof payout SLO refs',
      record.payoutSloRefs,
      audience,
    ),
    payoutTargetDisclosureAllowed: false,
    payoutVerificationClaimAllowed:
      stateAtLeast(record.state, 'payout_verified') &&
      record.payoutVerificationRefs.length > 0,
    payoutVerificationRefs: safeRefsForAudience(
      'accepted-work proof payout verification refs',
      record.payoutVerificationRefs,
      audience,
    ),
    providerEligibilityMutationAllowed: false,
    providerJobRefs: safeRefsForAudience(
      'accepted-work proof provider job refs',
      record.providerJobRefs,
      audience,
    ),
    providerPayoutClaimIncluded: record.providerPayoutClaimIncluded,
    providerRef: providerRefForAudience(record, audience),
    providerVisibility: record.providerVisibility,
    publicProofRefs: safeRefsForAudience(
      'accepted-work proof public proof refs',
      record.publicProofRefs,
      audience,
    ),
    receiptLinkRefs: safeRefsForAudience(
      'accepted-work proof receipt link refs',
      record.receiptLinkRefs,
      audience,
    ),
    rewardIntentClaimAllowed:
      stateAtLeast(record.state, 'reward_intent') &&
      record.rewardIntentRefs.length > 0,
    rewardIntentRefs: safeRefsForAudience(
      'accepted-work proof reward intent refs',
      record.rewardIntentRefs,
      audience,
    ),
    settlementBridgeRefs: safeRefsForAudience(
      'accepted-work proof settlement bridge refs',
      record.settlementBridgeRefs,
      audience,
    ),
    settlementClaimAllowed:
      record.state === 'settled' &&
      record.settlementRefs.length > 0 &&
      record.settlementEvidenceRefs.length > 0 &&
      record.payoutVerificationRefs.length > 0,
    settlementEvidenceRefs: safeRefsForAudience(
      'accepted-work proof settlement evidence refs',
      record.settlementEvidenceRefs,
      audience,
    ),
    settlementMutationAllowed: false,
    settlementRefs: safeRefsForAudience(
      'accepted-work proof settlement refs',
      record.settlementRefs,
      audience,
    ),
    siteRefs: safeRefsForAudience(
      'accepted-work proof site refs',
      record.siteRefs,
      audience,
    ),
    siteReleaseMutationAllowed: false,
    sourceRefs: safeRefsForAudience(
      'accepted-work proof source refs',
      record.sourceRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    versionRefs: safeRefsForAudience(
      'accepted-work proof version refs',
      record.versionRefs,
      audience,
    ),
  }

  if (pylonAcceptedWorkProofLinkProjectionHasPrivateMaterial(projection)) {
    throw new PylonAcceptedWorkProofLinkUnsafe({
      reason: 'Accepted-work proof link projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_ACCEPTED_WORK_PROOF_LINK_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonAcceptedWorkProofLinkRecord> = [
    {
      acceptedWorkRefs: ['accepted_work.public.site_otc_revision_3'],
      authority: PYLON_ACCEPTED_WORK_PROOF_LINK_READ_ONLY_AUTHORITY,
      blockerRefs: [],
      caveatRefs: ['caveat.public.provider_payout_claim_separate'],
      consumerSurfaces: ['site_order', 'public_proof', 'customer_dashboard'],
      createdAtIso: '2026-06-07T11:00:00.000Z',
      evidenceRefs: ['evidence.public.site_otc_revision_3'],
      id: 'accepted_work_proof_link.public.site_otc_revision_3',
      orderRefs: ['order.public.otec'],
      payoutConfirmationRefs: [
        'confirmation.public.site_otc_revision_3',
        'confirmation.private.operator_trace',
      ],
      payoutDispatchRefs: [
        'dispatch.public.site_otc_revision_3',
        'dispatch.private.operator_trace',
      ],
      payoutEligibilityRefs: ['eligibility.public.site_otc_revision_3'],
      payoutRowRefs: ['payout.public.row.site_otc_revision_3'],
      payoutSloRefs: [
        'slo.public.site_otc_revision_3',
        'slo.private.operator_trace',
      ],
      payoutVerificationRefs: [
        'verification.public.site_otc_revision_3',
        'verification.private.operator_trace',
      ],
      providerJobRefs: [
        'job.public.site_otc_revision_3',
        'job.private.operator_trace',
      ],
      providerPayoutClaimIncluded: true,
      providerRef: 'provider.private.pylon_1',
      providerVisibility: 'private',
      publicProofRefs: ['proof.public.otec'],
      receiptLinkRefs: ['receipt.public.site_otc_revision_3'],
      rewardIntentRefs: ['reward.public.site_otc_revision_3'],
      settlementBridgeRefs: ['settlement.public.bridge.site_otc_revision_3'],
      settlementEvidenceRefs: [
        'settlement.public.evidence.site_otc_revision_3',
        'settlement.private.operator_trace',
      ],
      settlementRefs: ['settlement.public.site_otc_revision_3'],
      siteRefs: ['site.public.otec'],
      sourceRefs: ['source.public.pylon_settlement_bridge'],
      state: 'settled',
      updatedAtIso: '2026-06-07T11:05:00.000Z',
      versionRefs: ['version.public.site_otc_revision_3'],
    },
  ]
