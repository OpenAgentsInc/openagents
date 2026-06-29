import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const OpenAgentsPylonSettlementBridgeState = S.Literals([
  'accepted_work',
  'assigned',
  'blocked',
  'buyer_payment_evidence',
  'capability_snapshot',
  'payout_confirmed',
  'payout_dispatched',
  'payout_eligible',
  'payout_verified',
  'reward_intent',
  'settled',
  'wallet_readiness_checked',
])
export type OpenAgentsPylonSettlementBridgeState =
  typeof OpenAgentsPylonSettlementBridgeState.Type

export const OpenAgentsPylonWalletReadinessState = S.Literals([
  'attention_required',
  'insufficient_liquidity',
  'not_required',
  'receive_ready',
  'send_ready',
  'unknown',
])
export type OpenAgentsPylonWalletReadinessState =
  typeof OpenAgentsPylonWalletReadinessState.Type

export const OpenAgentsPylonProviderVisibility = S.Literals([
  'private',
  'public',
])
export type OpenAgentsPylonProviderVisibility =
  typeof OpenAgentsPylonProviderVisibility.Type

export const OpenAgentsPylonSettlementBridgeAuthorityBoundary = S.Literals([
  'evidence_only',
])
export type OpenAgentsPylonSettlementBridgeAuthorityBoundary =
  typeof OpenAgentsPylonSettlementBridgeAuthorityBoundary.Type

export class OpenAgentsPylonSettlementBridgeAuthority extends S.Class<OpenAgentsPylonSettlementBridgeAuthority>(
  'OpenAgentsPylonSettlementBridgeAuthority',
)({
  authorityBoundary: OpenAgentsPylonSettlementBridgeAuthorityBoundary,
  noBuyerChargeMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OpenAgentsPylonSettlementBridgeRecord extends S.Class<OpenAgentsPylonSettlementBridgeRecord>(
  'OpenAgentsPylonSettlementBridgeRecord',
)({
  acceptedWorkRefs: S.Array(S.String),
  authority: OpenAgentsPylonSettlementBridgeAuthority,
  blockerRefs: S.Array(S.String),
  buyerPaymentEvidenceRefs: S.Array(S.String),
  capabilitySnapshotRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  operatorDiagnosticRefs: S.Array(S.String),
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchRefs: S.Array(S.String),
  payoutEligibilityRefs: S.Array(S.String),
  payoutVerificationRefs: S.Array(S.String),
  providerAssignmentRefs: S.Array(S.String),
  providerJobRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: OpenAgentsPylonProviderVisibility,
  rewardIntentRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  state: OpenAgentsPylonSettlementBridgeState,
  updatedAtIso: S.String,
  walletReadinessRefs: S.Array(S.String),
  walletReadinessState: OpenAgentsPylonWalletReadinessState,
  workroomRefs: S.Array(S.String),
}) {}

export class OpenAgentsPylonSettlementBridgeProjection extends S.Class<OpenAgentsPylonSettlementBridgeProjection>(
  'OpenAgentsPylonSettlementBridgeProjection',
)({
  acceptedWorkClaimAllowed: S.Boolean,
  acceptedWorkRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: OpenAgentsPylonSettlementBridgeAuthority,
  blockerRefs: S.Array(S.String),
  buyerPaymentEvidencePresent: S.Boolean,
  buyerPaymentEvidenceRefs: S.Array(S.String),
  capabilitySnapshotRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  operatorDiagnosticRefs: S.Array(S.String),
  payoutConfirmationClaimAllowed: S.Boolean,
  payoutConfirmationRefs: S.Array(S.String),
  payoutDispatchClaimAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutDispatchRefs: S.Array(S.String),
  payoutEligibilityClaimAllowed: S.Boolean,
  payoutEligibilityRefs: S.Array(S.String),
  payoutVerificationClaimAllowed: S.Boolean,
  payoutVerificationRefs: S.Array(S.String),
  providerAssignmentRefs: S.Array(S.String),
  providerJobRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: OpenAgentsPylonProviderVisibility,
  rewardIntentClaimAllowed: S.Boolean,
  rewardIntentRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  state: OpenAgentsPylonSettlementBridgeState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  walletReadinessRefs: S.Array(S.String),
  walletReadinessState: OpenAgentsPylonWalletReadinessState,
  walletReadinessStateLabel: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class OpenAgentsPylonSettlementBridgeUnsafe extends S.TaggedErrorClass<OpenAgentsPylonSettlementBridgeUnsafe>()(
  'OpenAgentsPylonSettlementBridgeUnsafe',
  {
    reason: S.String,
  },
) {}

export const OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY:
  OpenAgentsPylonSettlementBridgeAuthority = {
    authorityBoundary: 'evidence_only',
    noBuyerChargeMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPayoutTargetMutation: true,
    noSettlementMutation: true,
  }

const stateRank: Readonly<Record<OpenAgentsPylonSettlementBridgeState, number>> = {
  accepted_work: 4,
  assigned: 0,
  blocked: -1,
  buyer_payment_evidence: 3,
  capability_snapshot: 1,
  payout_confirmed: 8,
  payout_dispatched: 7,
  payout_eligible: 6,
  payout_verified: 9,
  reward_intent: 5,
  settled: 10,
  wallet_readiness_checked: 2,
}

const stateLabelByState:
  Readonly<Record<OpenAgentsPylonSettlementBridgeState, string>> = {
    accepted_work: 'Accepted work',
    assigned: 'Assigned',
    blocked: 'Blocked',
    buyer_payment_evidence: 'Buyer payment evidence',
    capability_snapshot: 'Capability snapshot',
    payout_confirmed: 'Payout confirmed',
    payout_dispatched: 'Payout dispatched',
    payout_eligible: 'Payout eligible',
    payout_verified: 'Payout verified',
    reward_intent: 'Reward intent',
    settled: 'Settled',
    wallet_readiness_checked: 'Wallet readiness checked',
  }

const walletReadinessLabelByState:
  Readonly<Record<OpenAgentsPylonWalletReadinessState, string>> = {
    attention_required: 'Needs attention',
    insufficient_liquidity: 'Insufficient liquidity',
    not_required: 'Not required',
    receive_ready: 'Receive ready',
    send_ready: 'Send ready',
    unknown: 'Unknown',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafePylonSettlementRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(buyer[_-]?payment|diagnostic\.operator|payout[_-]?(confirmation|dispatch|verification)|provider\.private|settlement\.private|workroom\.)/i
const customerUnsafeRefPattern =
  /(buyer[_-]?payment|diagnostic\.operator|payout[_-]?(confirmation|dispatch|verification)|provider\.private|settlement\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(buyer[_-]?payment|diagnostic\.operator|provider\.private|settlement\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

export const openAgentsPylonSettlementBridgeStateAtLeast = (
  state: OpenAgentsPylonSettlementBridgeState,
  threshold: OpenAgentsPylonSettlementBridgeState,
): boolean => stateRank[state] >= stateRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafePylonSettlementRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: `${label} contains private customer data, wallet material, raw bitcoin payment material, invoices, preimages, payout targets, private channel state, provider secrets, raw logs, private repo refs, or raw timestamps.`,
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

const providerRefForAudience = (
  record: OpenAgentsPylonSettlementBridgeRecord,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (
    record.providerVisibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefForAudience(
      'pylon provider ref',
      record.providerRef,
      audience,
    )
  }

  return 'provider.redacted'
}

export const openAgentsPylonSettlementBridgeHasNoSpendAuthority = (
  authority: OpenAgentsPylonSettlementBridgeAuthority,
): boolean =>
  authority.authorityBoundary === 'evidence_only' &&
  authority.noBuyerChargeMutation &&
  authority.noLiveWalletSpend &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetMutation &&
  authority.noSettlementMutation

export const openAgentsPylonSettlementBridgeCanMutateSettlement = (
  record: OpenAgentsPylonSettlementBridgeRecord,
): boolean =>
  !openAgentsPylonSettlementBridgeHasNoSpendAuthority(record.authority)

export const openAgentsPylonSettlementBridgeSettlementClaimAllowed = (
  record: OpenAgentsPylonSettlementBridgeRecord,
): boolean =>
  record.state === 'settled' &&
  record.settlementRefs.length > 0 &&
  record.payoutVerificationRefs.length > 0

const assertRecordSafe = (
  record: OpenAgentsPylonSettlementBridgeRecord,
): void => {
  assertSafeRefs('pylon settlement bridge identity refs', [
    record.id,
    record.providerRef,
  ])
  assertSafeRefs(
    'pylon settlement bridge provider assignment refs',
    record.providerAssignmentRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge provider job refs',
    record.providerJobRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge capability snapshot refs',
    record.capabilitySnapshotRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge wallet readiness refs',
    record.walletReadinessRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge buyer payment evidence refs',
    record.buyerPaymentEvidenceRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge accepted work refs',
    record.acceptedWorkRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge reward intent refs',
    record.rewardIntentRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge payout eligibility refs',
    record.payoutEligibilityRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge payout dispatch refs',
    record.payoutDispatchRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge payout confirmation refs',
    record.payoutConfirmationRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge payout verification refs',
    record.payoutVerificationRefs,
  )
  assertSafeRefs(
    'pylon settlement bridge settlement refs',
    record.settlementRefs,
  )
  assertSafeRefs('pylon settlement bridge blocker refs', record.blockerRefs)
  assertSafeRefs('pylon settlement bridge caveat refs', record.caveatRefs)
  assertSafeRefs('pylon settlement bridge evidence refs', record.evidenceRefs)
  assertSafeRefs(
    'pylon settlement bridge operator diagnostic refs',
    record.operatorDiagnosticRefs,
  )
  assertSafeRefs('pylon settlement bridge workroom refs', record.workroomRefs)

  if (!openAgentsPylonSettlementBridgeHasNoSpendAuthority(record.authority)) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Pylon settlement bridge records must be evidence-only and cannot carry live bitcoin spend, payout-dispatch, payout-target, buyer-charge, or settlement mutation authority.',
    })
  }

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Blocked Pylon settlement bridges require blocker refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(record.state, 'assigned') &&
    record.providerAssignmentRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Assigned Pylon settlement bridges require provider assignment refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'capability_snapshot',
    ) &&
    record.capabilitySnapshotRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Capability snapshot state requires capability snapshot refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'wallet_readiness_checked',
    ) &&
    record.walletReadinessState !== 'not_required' &&
    record.walletReadinessRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Wallet readiness state requires redacted readiness refs unless wallet readiness is not required.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'buyer_payment_evidence',
    ) &&
    record.buyerPaymentEvidenceRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Buyer payment evidence state requires buyer payment evidence refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'accepted_work',
    ) &&
    record.acceptedWorkRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Accepted work state requires accepted work refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(record.state, 'reward_intent') &&
    record.rewardIntentRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Reward intent state requires reward intent refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'payout_eligible',
    ) &&
    record.payoutEligibilityRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Payout eligible state requires payout eligibility refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'payout_dispatched',
    ) &&
    record.payoutDispatchRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Payout dispatched state requires payout dispatch refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'payout_confirmed',
    ) &&
    record.payoutConfirmationRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Payout confirmed state requires payout confirmation refs.',
    })
  }

  if (
    openAgentsPylonSettlementBridgeStateAtLeast(
      record.state,
      'payout_verified',
    ) &&
    record.payoutVerificationRefs.length === 0
  ) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Payout verified state requires payout verification refs.',
    })
  }

  if (record.state === 'settled' && record.settlementRefs.length === 0) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Settled Pylon bridge state requires settlement refs.',
    })
  }
}

const projectionText = (
  projection: OpenAgentsPylonSettlementBridgeProjection,
): string =>
  [
    projection.id,
    projection.providerRef,
    ...projection.acceptedWorkRefs,
    ...projection.blockerRefs,
    ...projection.buyerPaymentEvidenceRefs,
    ...projection.capabilitySnapshotRefs,
    ...projection.caveatRefs,
    ...projection.evidenceRefs,
    ...projection.operatorDiagnosticRefs,
    ...projection.payoutConfirmationRefs,
    ...projection.payoutDispatchRefs,
    ...projection.payoutEligibilityRefs,
    ...projection.payoutVerificationRefs,
    ...projection.providerAssignmentRefs,
    ...projection.providerJobRefs,
    ...projection.rewardIntentRefs,
    ...projection.settlementRefs,
    ...projection.walletReadinessRefs,
    ...projection.workroomRefs,
  ].join(' ')

export const openAgentsPylonSettlementBridgeProjectionHasPrivateMaterial = (
  projection: OpenAgentsPylonSettlementBridgeProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafePylonSettlementRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectOpenAgentsPylonSettlementBridge = (
  record: OpenAgentsPylonSettlementBridgeRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsPylonSettlementBridgeProjection => {
  assertRecordSafe(record)

  const projection: OpenAgentsPylonSettlementBridgeProjection = {
    acceptedWorkClaimAllowed:
      openAgentsPylonSettlementBridgeStateAtLeast(
        record.state,
        'accepted_work',
      ) && record.acceptedWorkRefs.length > 0,
    acceptedWorkRefs: safeRefsForAudience(
      'pylon settlement bridge accepted work refs',
      record.acceptedWorkRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'pylon settlement bridge blocker refs',
      record.blockerRefs,
      audience,
    ),
    buyerPaymentEvidencePresent: record.buyerPaymentEvidenceRefs.length > 0,
    buyerPaymentEvidenceRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'pylon settlement bridge buyer payment evidence refs',
        record.buyerPaymentEvidenceRefs,
        audience,
      )
      : [],
    capabilitySnapshotRefs: safeRefsForAudience(
      'pylon settlement bridge capability snapshot refs',
      record.capabilitySnapshotRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'pylon settlement bridge caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'pylon settlement bridge evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: safeRefForAudience('pylon settlement bridge id', record.id, audience),
    liveWalletSpendAllowed: false,
    operatorDiagnosticRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'pylon settlement bridge operator diagnostic refs',
        record.operatorDiagnosticRefs,
        audience,
      )
      : [],
    payoutConfirmationClaimAllowed:
      openAgentsPylonSettlementBridgeStateAtLeast(
        record.state,
        'payout_confirmed',
      ) && record.payoutConfirmationRefs.length > 0,
    payoutConfirmationRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'pylon settlement bridge payout confirmation refs',
        record.payoutConfirmationRefs,
        audience,
      )
      : [],
    payoutDispatchClaimAllowed:
      openAgentsPylonSettlementBridgeStateAtLeast(
        record.state,
        'payout_dispatched',
      ) && record.payoutDispatchRefs.length > 0,
    payoutDispatchMutationAllowed: false,
    payoutDispatchRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'pylon settlement bridge payout dispatch refs',
        record.payoutDispatchRefs,
        audience,
      )
      : [],
    payoutEligibilityClaimAllowed:
      openAgentsPylonSettlementBridgeStateAtLeast(
        record.state,
        'payout_eligible',
      ) && record.payoutEligibilityRefs.length > 0,
    payoutEligibilityRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'pylon settlement bridge payout eligibility refs',
        record.payoutEligibilityRefs,
        audience,
      ),
    payoutVerificationClaimAllowed:
      openAgentsPylonSettlementBridgeStateAtLeast(
        record.state,
        'payout_verified',
      ) && record.payoutVerificationRefs.length > 0,
    payoutVerificationRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'pylon settlement bridge payout verification refs',
        record.payoutVerificationRefs,
        audience,
      )
      : [],
    providerAssignmentRefs: safeRefsForAudience(
      'pylon settlement bridge provider assignment refs',
      record.providerAssignmentRefs,
      audience,
    ),
    providerJobRefs: safeRefsForAudience(
      'pylon settlement bridge provider job refs',
      record.providerJobRefs,
      audience,
    ),
    providerRef: providerRefForAudience(record, audience),
    providerVisibility: record.providerVisibility,
    rewardIntentClaimAllowed:
      openAgentsPylonSettlementBridgeStateAtLeast(
        record.state,
        'reward_intent',
      ) && record.rewardIntentRefs.length > 0,
    rewardIntentRefs: safeRefsForAudience(
      'pylon settlement bridge reward intent refs',
      record.rewardIntentRefs,
      audience,
    ),
    settlementClaimAllowed:
      openAgentsPylonSettlementBridgeSettlementClaimAllowed(record),
    settlementMutationAllowed: false,
    settlementRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'pylon settlement bridge settlement refs',
        record.settlementRefs,
        audience,
      )
      : safeRefsForAudience(
        'pylon settlement bridge settlement refs',
        record.settlementRefs.filter(ref => !/settlement\.private/i.test(ref)),
        audience,
      ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletReadinessRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'pylon settlement bridge wallet readiness refs',
        record.walletReadinessRefs,
        audience,
      )
      : [],
    walletReadinessState: record.walletReadinessState,
    walletReadinessStateLabel:
      walletReadinessLabelByState[record.walletReadinessState],
    workroomRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'pylon settlement bridge workroom refs',
        record.workroomRefs,
        audience,
      ),
  }

  if (openAgentsPylonSettlementBridgeProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsPylonSettlementBridgeUnsafe({
      reason: 'Pylon settlement bridge projection contains private material.',
    })
  }

  return projection
}

export const exampleOpenAgentsPylonSettlementBridgeRecord =
  (): OpenAgentsPylonSettlementBridgeRecord => ({
    acceptedWorkRefs: ['accepted_work.pylon_trace_summary'],
    authority: OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY,
    blockerRefs: [],
    buyerPaymentEvidenceRefs: ['buyer_payment_evidence.omega_order_budget'],
    capabilitySnapshotRefs: ['capability_snapshot.pylon_provider_demo'],
    caveatRefs: ['caveat.pylon.no_live_spend'],
    createdAtIso: '2026-06-07T03:10:00.000Z',
    evidenceRefs: ['evidence.pylon.bridge.fixture'],
    id: 'pylon_settlement_bridge.trace_summary_1',
    operatorDiagnosticRefs: ['diagnostic.operator.pylon_bridge_safe_summary'],
    payoutConfirmationRefs: ['payout_confirmation.trace_summary_1'],
    payoutDispatchRefs: ['payout_dispatch.trace_summary_1'],
    payoutEligibilityRefs: ['payout_eligibility.trace_summary_1'],
    payoutVerificationRefs: ['payout_verification.trace_summary_1'],
    providerAssignmentRefs: ['provider_assignment.trace_summary_1'],
    providerJobRefs: ['provider_job.trace_summary_1'],
    providerRef: 'provider.pylon_public_demo',
    providerVisibility: 'public',
    rewardIntentRefs: ['reward_intent.trace_summary_1'],
    settlementRefs: ['settlement.public_receipt.trace_summary_1'],
    state: 'settled',
    updatedAtIso: '2026-06-07T03:45:00.000Z',
    walletReadinessRefs: ['readiness_summary.pylon_provider_demo'],
    walletReadinessState: 'receive_ready',
    workroomRefs: ['workroom.pylon_trace_summary'],
  })

export const OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES:
  ReadonlyArray<OpenAgentsPylonSettlementBridgeRecord> = [
    exampleOpenAgentsPylonSettlementBridgeRecord(),
    {
      ...exampleOpenAgentsPylonSettlementBridgeRecord(),
      acceptedWorkRefs: [],
      buyerPaymentEvidenceRefs: ['buyer_payment_evidence.order_pending'],
      id: 'pylon_settlement_bridge.buyer_payment_only',
      payoutConfirmationRefs: [],
      payoutDispatchRefs: [],
      payoutEligibilityRefs: [],
      payoutVerificationRefs: [],
      rewardIntentRefs: [],
      settlementRefs: [],
      state: 'buyer_payment_evidence',
      updatedAtIso: '2026-06-07T03:20:00.000Z',
    },
  ]
