import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from '../blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from '../omni-data-classification'

export const ForumAcceptedContributionBridgeKind = S.Literals([
  'accepted_contribution_reward',
  'ordinary_content_reward',
])
export type ForumAcceptedContributionBridgeKind =
  typeof ForumAcceptedContributionBridgeKind.Type

export const ForumAcceptedContributionBridgeState = S.Literals([
  'accepted_contribution',
  'blocked',
  'content_rewarded',
  'payout_dispatched',
  'payout_eligible',
  'payout_verified',
  'reward_intent',
  'settled',
])
export type ForumAcceptedContributionBridgeState =
  typeof ForumAcceptedContributionBridgeState.Type

export const ForumAcceptedContributionBridgeVisibility = S.Literals([
  'private',
  'public',
])
export type ForumAcceptedContributionBridgeVisibility =
  typeof ForumAcceptedContributionBridgeVisibility.Type

export const ForumAcceptedContributionBridgeAuthorityBoundary = S.Literals([
  'read_only_forum_to_payout_projection',
])
export type ForumAcceptedContributionBridgeAuthorityBoundary =
  typeof ForumAcceptedContributionBridgeAuthorityBoundary.Type

export class ForumAcceptedContributionBridgeAuthority extends S.Class<ForumAcceptedContributionBridgeAuthority>(
  'ForumAcceptedContributionBridgeAuthority',
)({
  authorityBoundary: ForumAcceptedContributionBridgeAuthorityBoundary,
  noAcceptedContributionMutation: S.Boolean,
  noForumReceiptMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetDisclosure: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class ForumAcceptedContributionBridgeRecord extends S.Class<ForumAcceptedContributionBridgeRecord>(
  'ForumAcceptedContributionBridgeRecord',
)({
  acceptedContributionReceiptRefs: S.Array(S.String),
  acceptedWorkRefs: S.Array(S.String),
  actorRefs: S.Array(S.String),
  authority: ForumAcceptedContributionBridgeAuthority,
  blockerRefs: S.Array(S.String),
  bridgeKind: ForumAcceptedContributionBridgeKind,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  earningActorRefs: S.Array(S.String),
  eligibilityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  forumPostRefs: S.Array(S.String),
  forumReceiptRefs: S.Array(S.String),
  forumRefs: S.Array(S.String),
  forumTopicRefs: S.Array(S.String),
  id: S.String,
  moneyActionRefs: S.Array(S.String),
  payoutDispatchRefs: S.Array(S.String),
  payoutRowRefs: S.Array(S.String),
  payoutSloRefs: S.Array(S.String),
  payoutVerificationRefs: S.Array(S.String),
  proofLinkRefs: S.Array(S.String),
  providerJobRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: ForumAcceptedContributionBridgeVisibility,
  rewardIntentRefs: S.Array(S.String),
  settlementEvidenceRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: ForumAcceptedContributionBridgeState,
  updatedAtIso: S.String,
}) {}

export class ForumAcceptedContributionBridgeProjection extends S.Class<ForumAcceptedContributionBridgeProjection>(
  'ForumAcceptedContributionBridgeProjection',
)({
  acceptedContributionClaimAllowed: S.Boolean,
  acceptedContributionMutationAllowed: S.Boolean,
  acceptedContributionReceiptRefs: S.Array(S.String),
  acceptedWorkClaimAllowed: S.Boolean,
  acceptedWorkRefs: S.Array(S.String),
  actorRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: ForumAcceptedContributionBridgeAuthority,
  blockerRefs: S.Array(S.String),
  bridgeKind: ForumAcceptedContributionBridgeKind,
  caveatRefs: S.Array(S.String),
  contentRewardClaimAllowed: S.Boolean,
  createdAtDisplay: S.String,
  earningActorRefs: S.Array(S.String),
  eligibilityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  forumPostRefs: S.Array(S.String),
  forumReceiptMutationAllowed: S.Boolean,
  forumReceiptRefs: S.Array(S.String),
  forumRefs: S.Array(S.String),
  forumTopicRefs: S.Array(S.String),
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  moneyActionRefs: S.Array(S.String),
  payoutDispatchClaimAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutDispatchRefs: S.Array(S.String),
  payoutEligibilityClaimAllowed: S.Boolean,
  payoutRowRefs: S.Array(S.String),
  payoutSloRefs: S.Array(S.String),
  payoutTargetDisclosureAllowed: S.Boolean,
  payoutVerificationClaimAllowed: S.Boolean,
  payoutVerificationRefs: S.Array(S.String),
  proofLinkRefs: S.Array(S.String),
  providerJobRefs: S.Array(S.String),
  providerRef: S.String,
  providerVisibility: ForumAcceptedContributionBridgeVisibility,
  rewardIntentClaimAllowed: S.Boolean,
  rewardIntentRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementEvidenceRefs: S.Array(S.String),
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: ForumAcceptedContributionBridgeState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ForumAcceptedContributionBridgeUnsafe extends S.TaggedErrorClass<ForumAcceptedContributionBridgeUnsafe>()(
  'ForumAcceptedContributionBridgeUnsafe',
  {
    reason: S.String,
  },
) {}

export const FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_READ_ONLY_AUTHORITY:
  ForumAcceptedContributionBridgeAuthority = {
    authorityBoundary: 'read_only_forum_to_payout_projection',
    noAcceptedContributionMutation: true,
    noForumReceiptMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPayoutTargetDisclosure: true,
    noSettlementMutation: true,
  }

const stateRank:
  Readonly<Record<ForumAcceptedContributionBridgeState, number>> = {
    accepted_contribution: 1,
    blocked: -1,
    content_rewarded: 0,
    payout_dispatched: 4,
    payout_eligible: 3,
    payout_verified: 5,
    reward_intent: 2,
    settled: 6,
  }

const stateLabelByState:
  Readonly<Record<ForumAcceptedContributionBridgeState, string>> = {
    accepted_contribution: 'Accepted contribution',
    blocked: 'Blocked',
    content_rewarded: 'Content rewarded',
    payout_dispatched: 'Payout dispatched',
    payout_eligible: 'Payout eligible',
    payout_verified: 'Payout verified',
    reward_intent: 'Reward intent',
    settled: 'Settled',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeBridgeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth[_-]?content[_-]?json|auth\.json|bearer|bolt11|bolt12|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw|target)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|email|invoice|payment|payload|payout|prompt|provider|runner|run[_-]?log|source[_-]?archive|state|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted_work\.private|actor\.private|blocker\.private|caveat\.private|contribution\.private|dispatch\.private|earning\.private|eligibility\.private|evidence\.private|forum\.private|job\.private|link\.private|money_action\.private|payout\.private|post\.private|provider\.private|receipt\.private|reward\.private|settlement\.private|slo\.private|source\.private|topic\.private|verification\.private)/i
const customerUnsafeRefPattern = publicUnsafeRefPattern
const teamUnsafeRefPattern =
  /(dispatch\.private|job\.private|payout\.private|provider\.private|settlement\.private|slo\.private|verification\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stateAtLeast = (
  state: ForumAcceptedContributionBridgeState,
  threshold: ForumAcceptedContributionBridgeState,
): boolean => stateRank[state] >= stateRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeBridgeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ForumAcceptedContributionBridgeUnsafe({
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
  record: ForumAcceptedContributionBridgeRecord,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (
    record.providerVisibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefsForAudience('Forum accepted contribution provider ref', [
      record.providerRef,
    ], audience)[0] ?? 'provider.redacted'
  }

  return 'provider.redacted'
}

export const forumAcceptedContributionBridgeHasNoMutationAuthority = (
  authority: ForumAcceptedContributionBridgeAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_forum_to_payout_projection' &&
  authority.noAcceptedContributionMutation &&
  authority.noForumReceiptMutation &&
  authority.noLiveWalletSpend &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetDisclosure &&
  authority.noSettlementMutation

export const forumAcceptedContributionBridgeCanMutatePayout = (
  record: ForumAcceptedContributionBridgeRecord,
): boolean => !forumAcceptedContributionBridgeHasNoMutationAuthority(
  record.authority,
)

const assertIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertRecordSafe = (
  record: ForumAcceptedContributionBridgeRecord,
): void => {
  assertSafeRefs('Forum accepted contribution identity refs', [
    record.id,
    record.providerRef,
  ])
  assertSafeRefs(
    'Forum accepted contribution receipt refs',
    record.acceptedContributionReceiptRefs,
  )
  assertSafeRefs('Forum accepted work refs', record.acceptedWorkRefs)
  assertSafeRefs('Forum accepted contribution actor refs', record.actorRefs)
  assertSafeRefs('Forum accepted contribution blocker refs', record.blockerRefs)
  assertSafeRefs('Forum accepted contribution caveat refs', record.caveatRefs)
  assertSafeRefs(
    'Forum accepted contribution earning actor refs',
    record.earningActorRefs,
  )
  assertSafeRefs('Forum accepted contribution eligibility refs', record.eligibilityRefs)
  assertSafeRefs('Forum accepted contribution evidence refs', record.evidenceRefs)
  assertSafeRefs('Forum accepted contribution forum refs', record.forumRefs)
  assertSafeRefs('Forum accepted contribution post refs', record.forumPostRefs)
  assertSafeRefs('Forum accepted contribution receipt refs', record.forumReceiptRefs)
  assertSafeRefs('Forum accepted contribution topic refs', record.forumTopicRefs)
  assertSafeRefs('Forum accepted contribution money action refs', record.moneyActionRefs)
  assertSafeRefs(
    'Forum accepted contribution payout dispatch refs',
    record.payoutDispatchRefs,
  )
  assertSafeRefs('Forum accepted contribution payout row refs', record.payoutRowRefs)
  assertSafeRefs('Forum accepted contribution payout SLO refs', record.payoutSloRefs)
  assertSafeRefs(
    'Forum accepted contribution payout verification refs',
    record.payoutVerificationRefs,
  )
  assertSafeRefs('Forum accepted contribution proof link refs', record.proofLinkRefs)
  assertSafeRefs('Forum accepted contribution provider job refs', record.providerJobRefs)
  assertSafeRefs('Forum accepted contribution reward refs', record.rewardIntentRefs)
  assertSafeRefs(
    'Forum accepted contribution settlement evidence refs',
    record.settlementEvidenceRefs,
  )
  assertSafeRefs('Forum accepted contribution settlement refs', record.settlementRefs)
  assertSafeRefs('Forum accepted contribution source refs', record.sourceRefs)
  assertIso('Forum accepted contribution createdAtIso', record.createdAtIso)
  assertIso('Forum accepted contribution updatedAtIso', record.updatedAtIso)

  if (!forumAcceptedContributionBridgeHasNoMutationAuthority(record.authority)) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: 'Forum accepted contribution bridges are read-only and cannot mutate Forum receipts, accepted contributions, wallet spend, payout dispatch, payout target disclosure, or settlement.',
    })
  }

  if (record.forumReceiptRefs.length === 0 || record.moneyActionRefs.length === 0) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: 'Forum reward bridges require Forum receipt and money action refs.',
    })
  }

  if (record.bridgeKind === 'ordinary_content_reward') {
    const forbiddenAcceptedWorkRefs = [
      ...record.acceptedContributionReceiptRefs,
      ...record.acceptedWorkRefs,
      ...record.providerJobRefs,
      ...record.payoutRowRefs,
      ...record.payoutSloRefs,
      ...record.proofLinkRefs,
      ...record.rewardIntentRefs,
      ...record.eligibilityRefs,
      ...record.payoutDispatchRefs,
      ...record.payoutVerificationRefs,
      ...record.settlementRefs,
      ...record.settlementEvidenceRefs,
    ]

    if (forbiddenAcceptedWorkRefs.length > 0) {
      throw new ForumAcceptedContributionBridgeUnsafe({
        reason: 'Ordinary Forum content rewards cannot carry accepted-work payout or proof refs.',
      })
    }
  }

  if (
    record.bridgeKind === 'accepted_contribution_reward' &&
    (record.acceptedContributionReceiptRefs.length === 0 ||
      record.acceptedWorkRefs.length === 0)
  ) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: 'Accepted Forum contribution rewards require accepted contribution and accepted-work refs.',
    })
  }

  const requiredRefs:
    Readonly<Record<ForumAcceptedContributionBridgeState, ReadonlyArray<string>>> = {
      accepted_contribution: record.acceptedContributionReceiptRefs,
      blocked: record.blockerRefs,
      content_rewarded: record.forumReceiptRefs,
      payout_dispatched: record.payoutDispatchRefs,
      payout_eligible: record.eligibilityRefs,
      payout_verified: record.payoutVerificationRefs,
      reward_intent: record.rewardIntentRefs,
      settled: record.settlementRefs,
    }

  if (requiredRefs[record.state].length === 0) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: `${stateLabelByState[record.state]} bridge state requires matching refs.`,
    })
  }

  if (
    record.state !== 'content_rewarded' &&
    record.bridgeKind === 'ordinary_content_reward'
  ) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: 'Ordinary Forum content rewards cannot advance into accepted-work payout states.',
    })
  }

  if (
    record.state === 'settled' &&
    (record.settlementEvidenceRefs.length === 0 ||
      record.payoutVerificationRefs.length === 0)
  ) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: 'Settled Forum accepted contribution bridges require payout verification and settlement evidence refs.',
    })
  }
}

const acceptedWorkEnabled = (
  record: ForumAcceptedContributionBridgeRecord,
): boolean => record.bridgeKind === 'accepted_contribution_reward'

const projectionText = (
  projection: ForumAcceptedContributionBridgeProjection,
): string =>
  [
    projection.id,
    projection.providerRef,
    ...projection.acceptedContributionReceiptRefs,
    ...projection.acceptedWorkRefs,
    ...projection.actorRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.earningActorRefs,
    ...projection.eligibilityRefs,
    ...projection.evidenceRefs,
    ...projection.forumPostRefs,
    ...projection.forumReceiptRefs,
    ...projection.forumRefs,
    ...projection.forumTopicRefs,
    ...projection.moneyActionRefs,
    ...projection.payoutDispatchRefs,
    ...projection.payoutRowRefs,
    ...projection.payoutSloRefs,
    ...projection.payoutVerificationRefs,
    ...projection.proofLinkRefs,
    ...projection.providerJobRefs,
    ...projection.rewardIntentRefs,
    ...projection.settlementEvidenceRefs,
    ...projection.settlementRefs,
    ...projection.sourceRefs,
  ].join(' ')

export const forumAcceptedContributionBridgeProjectionHasPrivateMaterial = (
  projection: ForumAcceptedContributionBridgeProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeBridgeRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectForumAcceptedContributionBridge = (
  record: ForumAcceptedContributionBridgeRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ForumAcceptedContributionBridgeProjection => {
  assertRecordSafe(record)

  const canUseAcceptedWork = acceptedWorkEnabled(record)
  const projection: ForumAcceptedContributionBridgeProjection = {
    acceptedContributionClaimAllowed:
      canUseAcceptedWork &&
      stateAtLeast(record.state, 'accepted_contribution') &&
      record.acceptedContributionReceiptRefs.length > 0,
    acceptedContributionMutationAllowed: false,
    acceptedContributionReceiptRefs: safeRefsForAudience(
      'Forum accepted contribution receipt refs',
      record.acceptedContributionReceiptRefs,
      audience,
    ),
    acceptedWorkClaimAllowed:
      canUseAcceptedWork &&
      stateAtLeast(record.state, 'accepted_contribution') &&
      record.acceptedWorkRefs.length > 0,
    acceptedWorkRefs: safeRefsForAudience(
      'Forum accepted work refs',
      record.acceptedWorkRefs,
      audience,
    ),
    actorRefs: safeRefsForAudience(
      'Forum accepted contribution actor refs',
      record.actorRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'Forum accepted contribution blocker refs',
      record.blockerRefs,
      audience,
    ),
    bridgeKind: record.bridgeKind,
    caveatRefs: safeRefsForAudience(
      'Forum accepted contribution caveat refs',
      record.caveatRefs,
      audience,
    ),
    contentRewardClaimAllowed:
      record.forumReceiptRefs.length > 0 && record.moneyActionRefs.length > 0,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    earningActorRefs: safeRefsForAudience(
      'Forum accepted contribution earning actor refs',
      record.earningActorRefs,
      audience,
    ),
    eligibilityRefs: safeRefsForAudience(
      'Forum accepted contribution eligibility refs',
      record.eligibilityRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'Forum accepted contribution evidence refs',
      record.evidenceRefs,
      audience,
    ),
    forumPostRefs: safeRefsForAudience(
      'Forum accepted contribution post refs',
      record.forumPostRefs,
      audience,
    ),
    forumReceiptMutationAllowed: false,
    forumReceiptRefs: safeRefsForAudience(
      'Forum accepted contribution receipt refs',
      record.forumReceiptRefs,
      audience,
    ),
    forumRefs: safeRefsForAudience(
      'Forum accepted contribution forum refs',
      record.forumRefs,
      audience,
    ),
    forumTopicRefs: safeRefsForAudience(
      'Forum accepted contribution topic refs',
      record.forumTopicRefs,
      audience,
    ),
    id: safeRefsForAudience(
      'Forum accepted contribution bridge id',
      [record.id],
      audience,
    )[0] ?? 'forum_accepted_contribution_bridge.redacted',
    liveWalletSpendAllowed: false,
    moneyActionRefs: safeRefsForAudience(
      'Forum accepted contribution money action refs',
      record.moneyActionRefs,
      audience,
    ),
    payoutDispatchClaimAllowed:
      canUseAcceptedWork &&
      stateAtLeast(record.state, 'payout_dispatched') &&
      record.payoutDispatchRefs.length > 0,
    payoutDispatchMutationAllowed: false,
    payoutDispatchRefs: safeRefsForAudience(
      'Forum accepted contribution payout dispatch refs',
      record.payoutDispatchRefs,
      audience,
    ),
    payoutEligibilityClaimAllowed:
      canUseAcceptedWork &&
      stateAtLeast(record.state, 'payout_eligible') &&
      record.eligibilityRefs.length > 0,
    payoutRowRefs: safeRefsForAudience(
      'Forum accepted contribution payout row refs',
      record.payoutRowRefs,
      audience,
    ),
    payoutSloRefs: safeRefsForAudience(
      'Forum accepted contribution payout SLO refs',
      record.payoutSloRefs,
      audience,
    ),
    payoutTargetDisclosureAllowed: false,
    payoutVerificationClaimAllowed:
      canUseAcceptedWork &&
      stateAtLeast(record.state, 'payout_verified') &&
      record.payoutVerificationRefs.length > 0,
    payoutVerificationRefs: safeRefsForAudience(
      'Forum accepted contribution payout verification refs',
      record.payoutVerificationRefs,
      audience,
    ),
    proofLinkRefs: safeRefsForAudience(
      'Forum accepted contribution proof link refs',
      record.proofLinkRefs,
      audience,
    ),
    providerJobRefs: safeRefsForAudience(
      'Forum accepted contribution provider job refs',
      record.providerJobRefs,
      audience,
    ),
    providerRef: providerRefForAudience(record, audience),
    providerVisibility: record.providerVisibility,
    rewardIntentClaimAllowed:
      canUseAcceptedWork &&
      stateAtLeast(record.state, 'reward_intent') &&
      record.rewardIntentRefs.length > 0,
    rewardIntentRefs: safeRefsForAudience(
      'Forum accepted contribution reward refs',
      record.rewardIntentRefs,
      audience,
    ),
    settlementClaimAllowed:
      canUseAcceptedWork &&
      record.state === 'settled' &&
      record.settlementRefs.length > 0 &&
      record.settlementEvidenceRefs.length > 0 &&
      record.payoutVerificationRefs.length > 0,
    settlementEvidenceRefs: safeRefsForAudience(
      'Forum accepted contribution settlement evidence refs',
      record.settlementEvidenceRefs,
      audience,
    ),
    settlementMutationAllowed: false,
    settlementRefs: safeRefsForAudience(
      'Forum accepted contribution settlement refs',
      record.settlementRefs,
      audience,
    ),
    sourceRefs: safeRefsForAudience(
      'Forum accepted contribution source refs',
      record.sourceRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (forumAcceptedContributionBridgeProjectionHasPrivateMaterial(projection)) {
    throw new ForumAcceptedContributionBridgeUnsafe({
      reason: 'Forum accepted contribution bridge projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_CONFORMANCE_FIXTURES:
  ReadonlyArray<ForumAcceptedContributionBridgeRecord> = [
    {
      acceptedContributionReceiptRefs: [],
      acceptedWorkRefs: [],
      actorRefs: ['agent.public.alice'],
      authority: FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_READ_ONLY_AUTHORITY,
      blockerRefs: [],
      bridgeKind: 'ordinary_content_reward',
      caveatRefs: ['caveat.public.content_reward_not_accepted_work'],
      createdAtIso: '2026-06-07T12:00:00.000Z',
      earningActorRefs: ['agent.public.ben'],
      eligibilityRefs: [],
      evidenceRefs: ['evidence.public.forum_reward_alice_to_ben'],
      forumPostRefs: ['post.public.ben_hello_world'],
      forumReceiptRefs: ['receipt.public.forum_reward_alice_to_ben'],
      forumRefs: ['forum.public.void'],
      forumTopicRefs: ['topic.public.hello_world'],
      id: 'forum_bridge.public.ordinary_reward_alice_to_ben',
      moneyActionRefs: ['money_action.public.reward_alice_to_ben'],
      payoutDispatchRefs: [],
      payoutRowRefs: [],
      payoutSloRefs: [],
      payoutVerificationRefs: [],
      proofLinkRefs: [],
      providerJobRefs: [],
      providerRef: 'provider.redacted',
      providerVisibility: 'public',
      rewardIntentRefs: [],
      settlementEvidenceRefs: [],
      settlementRefs: [],
      sourceRefs: ['source.public.forum_paid_action_receipt'],
      state: 'content_rewarded',
      updatedAtIso: '2026-06-07T12:05:00.000Z',
    },
    {
      acceptedContributionReceiptRefs: [
        'contribution.public.accepted_forum_post_research_summary',
      ],
      acceptedWorkRefs: ['accepted_work.public.forum_research_summary'],
      actorRefs: ['agent.public.ben'],
      authority: FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_READ_ONLY_AUTHORITY,
      blockerRefs: [],
      bridgeKind: 'accepted_contribution_reward',
      caveatRefs: ['caveat.public.not_settled'],
      createdAtIso: '2026-06-07T12:10:00.000Z',
      earningActorRefs: ['agent.public.alice'],
      eligibilityRefs: ['eligibility.public.forum_research_summary'],
      evidenceRefs: ['evidence.public.forum_research_summary'],
      forumPostRefs: ['post.public.research_summary'],
      forumReceiptRefs: ['receipt.public.forum_reward_ben_to_alice'],
      forumRefs: ['forum.public.research'],
      forumTopicRefs: ['topic.public.otec_sources'],
      id: 'forum_bridge.public.accepted_contribution_ben_to_alice',
      moneyActionRefs: ['money_action.public.reward_ben_to_alice'],
      payoutDispatchRefs: [],
      payoutRowRefs: ['payout.public.row.forum_research_summary'],
      payoutSloRefs: ['slo.public.forum_research_summary'],
      payoutVerificationRefs: ['verification.public.forum_research_summary'],
      proofLinkRefs: ['proof_link.public.forum_research_summary'],
      providerJobRefs: ['job.public.forum_research_summary'],
      providerRef: 'provider.private.pylon_forum_worker',
      providerVisibility: 'private',
      rewardIntentRefs: ['reward.public.forum_research_summary'],
      settlementEvidenceRefs: [],
      settlementRefs: [],
      sourceRefs: [
        'source.public.forum_paid_action_receipt',
        'source.public.accepted_contribution_receipt',
      ],
      state: 'payout_verified',
      updatedAtIso: '2026-06-07T12:15:00.000Z',
    },
  ]
