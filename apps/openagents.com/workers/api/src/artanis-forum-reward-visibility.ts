import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_CONFORMANCE_FIXTURES,
  ForumAcceptedContributionBridgeProjection,
  projectForumAcceptedContributionBridge,
} from './forum/accepted-contribution-proof-bridge'
import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisForumRewardVisibilityState = S.Literals([
  'blocked',
  'public_receipts_visible',
])
export type ArtanisForumRewardVisibilityState =
  typeof ArtanisForumRewardVisibilityState.Type

export class ArtanisForumRewardVisibilityAuthority extends S.Class<ArtanisForumRewardVisibilityAuthority>(
  'ArtanisForumRewardVisibilityAuthority',
)({
  noAcceptedWorkPayoutMutation: S.Boolean,
  noForumReceiptMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class ArtanisForumRewardVisibilityRecord extends S.Class<ArtanisForumRewardVisibilityRecord>(
  'ArtanisForumRewardVisibilityRecord',
)({
  acceptedContributionBridgeRefs: S.Array(S.String),
  acceptedWorkProofRefs: S.Array(S.String),
  agentRef: S.String,
  authority: ArtanisForumRewardVisibilityAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  earningActorRefs: S.Array(S.String),
  forumReceiptRefs: S.Array(S.String),
  paidActionRefs: S.Array(S.String),
  postRewardRefs: S.Array(S.String),
  publicCopyRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spendCapRefs: S.Array(S.String),
  state: ArtanisForumRewardVisibilityState,
  summaryRef: S.String,
  topicBoostRefs: S.Array(S.String),
  topicFundRefs: S.Array(S.String),
  updatedAtIso: S.String,
  walletAuthorityRefs: S.Array(S.String),
}) {}

export class ArtanisForumRewardVisibilityProjection extends S.Class<ArtanisForumRewardVisibilityProjection>(
  'ArtanisForumRewardVisibilityProjection',
)({
  acceptedContributionBridgeRefs: S.Array(S.String),
  acceptedContributionCount: S.Number,
  acceptedWorkPayoutClaimAllowed: S.Boolean,
  acceptedWorkProofRefs: S.Array(S.String),
  agentRef: S.String,
  audience: OmniProjectionAudience,
  authority: ArtanisForumRewardVisibilityAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  contentRewardCount: S.Number,
  earningActorRefs: S.Array(S.String),
  forumReceiptRefs: S.Array(S.String),
  liveWalletSpendAllowed: S.Boolean,
  paidActionRefs: S.Array(S.String),
  postRewardRefs: S.Array(S.String),
  publicCopyRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spendCapRefs: S.Array(S.String),
  state: ArtanisForumRewardVisibilityState,
  stateLabel: S.String,
  summaryRef: S.String,
  topicBoostRefs: S.Array(S.String),
  topicFundRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  walletAuthorityRefs: S.Array(S.String),
}) {}

export class ArtanisForumRewardVisibilityUnsafe extends S.TaggedErrorClass<ArtanisForumRewardVisibilityUnsafe>()(
  'ArtanisForumRewardVisibilityUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_FORUM_REWARD_VISIBILITY_READ_ONLY_AUTHORITY:
  ArtanisForumRewardVisibilityAuthority = {
    noAcceptedWorkPayoutMutation: true,
    noForumReceiptMutation: true,
    noLiveWalletSpend: true,
    noSettlementMutation: true,
  }

const stateLabelByState:
  Readonly<Record<ArtanisForumRewardVisibilityState, string>> = {
    blocked: 'Blocked',
    public_receipts_visible: 'Public receipts visible',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRewardVisibilityPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth[_-]?content[_-]?json|auth\.json|bearer|bolt11|bolt12|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|customer|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|customer|email|invoice|log|payment|payload|payout|prompt|provider|runner|run[_-]?log|source[_-]?archive|state|telemetry|text|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted_work\.private|actor\.private|authority\.private|blocker\.private|caveat\.private|earning\.private|evidence\.private|receipt\.private|reward\.private|source\.private|topic\.private|wallet\.private)/i

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
    unsafeRewardVisibilityPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason: `${label} contains private customer data, wallet material, raw bitcoin payment material, invoices, preimages, raw payout targets, private channel state, provider secrets, raw logs, private repo refs, credentials, or raw timestamps.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator' || audience === 'private') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const assertReadOnlyAuthority = (
  authority: ArtanisForumRewardVisibilityAuthority,
): void => {
  if (
    !authority.noAcceptedWorkPayoutMutation ||
    !authority.noForumReceiptMutation ||
    !authority.noLiveWalletSpend ||
    !authority.noSettlementMutation
  ) {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason:
        'Artanis Forum reward visibility is read-only and cannot mutate Forum receipts, spend wallets, mutate accepted-work payouts, or mutate settlement.',
    })
  }
}

const assertRecordSafe = (
  record: ArtanisForumRewardVisibilityRecord,
): void => {
  if (!Number.isFinite(Date.parse(record.updatedAtIso))) {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason: 'Artanis Forum reward visibility updatedAtIso must be valid.',
    })
  }

  assertSafeRefs('Artanis Forum reward identity refs', [
    record.agentRef,
    record.summaryRef,
  ])
  assertSafeRefs(
    'Artanis Forum accepted contribution bridge refs',
    record.acceptedContributionBridgeRefs,
  )
  assertSafeRefs(
    'Artanis Forum accepted-work proof refs',
    record.acceptedWorkProofRefs,
  )
  assertSafeRefs('Artanis Forum blocker refs', record.blockerRefs)
  assertSafeRefs('Artanis Forum caveat refs', record.caveatRefs)
  assertSafeRefs('Artanis Forum earning actor refs', record.earningActorRefs)
  assertSafeRefs('Artanis Forum receipt refs', record.forumReceiptRefs)
  assertSafeRefs('Artanis Forum paid action refs', record.paidActionRefs)
  assertSafeRefs('Artanis Forum post reward refs', record.postRewardRefs)
  assertSafeRefs('Artanis Forum public copy refs', record.publicCopyRefs)
  assertSafeRefs('Artanis Forum source refs', record.sourceRefs)
  assertSafeRefs('Artanis Forum spend cap refs', record.spendCapRefs)
  assertSafeRefs('Artanis Forum topic boost refs', record.topicBoostRefs)
  assertSafeRefs('Artanis Forum topic fund refs', record.topicFundRefs)
  assertSafeRefs('Artanis Forum wallet authority refs', record.walletAuthorityRefs)
  assertReadOnlyAuthority(record.authority)

  if (record.agentRef !== 'agent_artanis') {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason: 'Artanis Forum reward visibility must be reported by agent_artanis.',
    })
  }

  if (
    record.walletAuthorityRefs.length > 0 ||
    record.spendCapRefs.length > 0
  ) {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason:
        'Live wallet reward visibility remains blocked until wallet authority and spend caps are modeled in a separate spend-authority contract.',
    })
  }

  if (record.forumReceiptRefs.length === 0) {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason: 'Forum reward visibility requires public Forum receipt refs.',
    })
  }

  if (
    record.acceptedWorkProofRefs.length > 0 &&
    record.acceptedContributionBridgeRefs.length === 0
  ) {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason:
        'Accepted-work proof visibility requires accepted-contribution bridge refs.',
    })
  }
}

export const artanisForumRewardVisibilityProjectionHasPrivateMaterial = (
  projection: ArtanisForumRewardVisibilityProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return containsProviderSecretMaterial(serialized) ||
    unsafeRewardVisibilityPattern.test(serialized) ||
    rawTimestampPattern.test(serialized) ||
    (
      projection.audience !== 'operator' &&
      projection.audience !== 'private' &&
      publicUnsafeRefPattern.test(serialized)
    )
}

export const projectArtanisForumRewardVisibility = (
  record: ArtanisForumRewardVisibilityRecord,
  acceptedContributionBridges:
    ReadonlyArray<ForumAcceptedContributionBridgeProjection>,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisForumRewardVisibilityProjection => {
  assertRecordSafe(record)

  const contentRewardCount = acceptedContributionBridges.filter(
    bridge => bridge.contentRewardClaimAllowed,
  ).length
  const acceptedContributionCount = acceptedContributionBridges.filter(
    bridge => bridge.acceptedContributionClaimAllowed,
  ).length
  const acceptedWorkPayoutClaimAllowed = acceptedContributionBridges.some(
    bridge => bridge.settlementClaimAllowed,
  )
  const projection: ArtanisForumRewardVisibilityProjection = {
    acceptedContributionBridgeRefs: refsForAudience(
      'Artanis Forum accepted contribution bridge refs',
      record.acceptedContributionBridgeRefs,
      audience,
    ),
    acceptedContributionCount,
    acceptedWorkPayoutClaimAllowed,
    acceptedWorkProofRefs: refsForAudience(
      'Artanis Forum accepted-work proof refs',
      record.acceptedWorkProofRefs,
      audience,
    ),
    agentRef: record.agentRef,
    audience,
    authority: record.authority,
    blockerRefs: refsForAudience(
      'Artanis Forum blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artanis Forum caveat refs',
      record.caveatRefs,
      audience,
    ),
    contentRewardCount,
    earningActorRefs: refsForAudience(
      'Artanis Forum earning actor refs',
      record.earningActorRefs,
      audience,
    ),
    forumReceiptRefs: refsForAudience(
      'Artanis Forum receipt refs',
      record.forumReceiptRefs,
      audience,
    ),
    liveWalletSpendAllowed: false,
    paidActionRefs: refsForAudience(
      'Artanis Forum paid action refs',
      record.paidActionRefs,
      audience,
    ),
    postRewardRefs: refsForAudience(
      'Artanis Forum post reward refs',
      record.postRewardRefs,
      audience,
    ),
    publicCopyRefs: refsForAudience(
      'Artanis Forum public copy refs',
      record.publicCopyRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Artanis Forum source refs',
      record.sourceRefs,
      audience,
    ),
    spendCapRefs: refsForAudience(
      'Artanis Forum spend cap refs',
      record.spendCapRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    summaryRef: record.summaryRef,
    topicBoostRefs: refsForAudience(
      'Artanis Forum topic boost refs',
      record.topicBoostRefs,
      audience,
    ),
    topicFundRefs: refsForAudience(
      'Artanis Forum topic fund refs',
      record.topicFundRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletAuthorityRefs: refsForAudience(
      'Artanis Forum wallet authority refs',
      record.walletAuthorityRefs,
      audience,
    ),
  }

  if (artanisForumRewardVisibilityProjectionHasPrivateMaterial(projection)) {
    throw new ArtanisForumRewardVisibilityUnsafe({
      reason: 'Artanis Forum reward visibility projection contains private or unsafe material.',
    })
  }

  return projection
}

export const exampleArtanisForumRewardVisibilityRecord =
  (): ArtanisForumRewardVisibilityRecord => ({
    acceptedContributionBridgeRefs: [
      'bridge.public.forum.accepted_contribution_reward',
    ],
    acceptedWorkProofRefs: [
      'proof_link.public.forum_research_summary',
    ],
    agentRef: 'agent_artanis',
    authority: ARTANIS_FORUM_REWARD_VISIBILITY_READ_ONLY_AUTHORITY,
    blockerRefs: [
      'blocker.public.no_named_wallet_authority',
      'blocker.public.no_live_spend_cap',
    ],
    caveatRefs: [
      'caveat.public.content_rewards_not_accepted_work_payouts',
      'caveat.public.no_unconditional_earning_promise',
    ],
    earningActorRefs: ['agent.public.alice', 'agent.public.ben'],
    forumReceiptRefs: [
      'receipt.public.forum_reward_alice_to_ben',
      'receipt.public.forum_reward_ben_to_alice',
    ],
    paidActionRefs: [
      'paid_action.public.forum.post_reward',
      'paid_action.public.forum.topic_boost',
      'paid_action.public.forum.topic_fund',
    ],
    postRewardRefs: [
      'reward.public.forum.post_reward_alice_to_ben',
      'reward.public.forum.post_reward_ben_to_alice',
    ],
    publicCopyRefs: [
      'copy.public.forum_agents_can_earn_bitcoin_when_receipts_exist',
      'copy.public.rewards_are_possible_not_guaranteed',
    ],
    sourceRefs: [
      'docs/forum/2026-06-06-multi-agent-payment-tipping-simulation.md',
      'docs/forum/2026-06-06-accepted-contribution-proof-bridge.md',
    ],
    spendCapRefs: [],
    state: 'public_receipts_visible',
    summaryRef: 'summary.public.artanis.forum_reward_visibility',
    topicBoostRefs: ['boost.public.forum.topic_agent_coordination'],
    topicFundRefs: ['fund.public.forum.topic_pylon_v02'],
    updatedAtIso: '2026-06-07T02:00:00.000Z',
    walletAuthorityRefs: [],
  })

export const exampleArtanisForumAcceptedContributionBridgeProjections = (
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ReadonlyArray<ForumAcceptedContributionBridgeProjection> =>
  FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_CONFORMANCE_FIXTURES.map(record =>
    projectForumAcceptedContributionBridge(record, audience, nowIso),
  )
