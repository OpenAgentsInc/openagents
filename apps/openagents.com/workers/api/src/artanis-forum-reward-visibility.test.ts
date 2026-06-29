import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_FORUM_REWARD_VISIBILITY_READ_ONLY_AUTHORITY,
  ArtanisForumRewardVisibilityProjection,
  ArtanisForumRewardVisibilityUnsafe,
  artanisForumRewardVisibilityProjectionHasPrivateMaterial,
  exampleArtanisForumAcceptedContributionBridgeProjections,
  exampleArtanisForumRewardVisibilityRecord,
  projectArtanisForumRewardVisibility,
} from './artanis-forum-reward-visibility'

const nowIso = '2026-06-07T02:10:00.000Z'

describe('Artanis Forum reward visibility', () => {
  test('projects content reward and accepted-contribution bridge state safely', () => {
    const projection = projectArtanisForumRewardVisibility(
      exampleArtanisForumRewardVisibilityRecord(),
      exampleArtanisForumAcceptedContributionBridgeProjections('public', nowIso),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisForumRewardVisibilityProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      acceptedContributionCount: 1,
      acceptedWorkPayoutClaimAllowed: false,
      contentRewardCount: 2,
      liveWalletSpendAllowed: false,
      state: 'public_receipts_visible',
      stateLabel: 'Public receipts visible',
      updatedAtDisplay: '10 minutes ago',
    })
    expect(projection.authority)
      .toEqual(ARTANIS_FORUM_REWARD_VISIBILITY_READ_ONLY_AUTHORITY)
    expect(projection.paidActionRefs).toEqual([
      'paid_action.public.forum.post_reward',
      'paid_action.public.forum.topic_boost',
      'paid_action.public.forum.topic_fund',
    ])
    expect(projection.caveatRefs).toEqual(
      expect.arrayContaining([
        'caveat.public.content_rewards_not_accepted_work_payouts',
        'caveat.public.no_unconditional_earning_promise',
      ]),
    )
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.no_live_spend_cap',
        'blocker.public.no_named_wallet_authority',
      ]),
    )
    expect(artanisForumRewardVisibilityProjectionHasPrivateMaterial(projection))
      .toBe(false)
    expect(JSON.stringify(projection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('does not treat ordinary Forum rewards as accepted-work payouts', () => {
    const ordinaryOnly = exampleArtanisForumAcceptedContributionBridgeProjections(
      'public',
      nowIso,
    ).filter(bridge => bridge.bridgeKind === 'ordinary_content_reward')
    const projection = projectArtanisForumRewardVisibility(
      {
        ...exampleArtanisForumRewardVisibilityRecord(),
        acceptedContributionBridgeRefs: [],
        acceptedWorkProofRefs: [],
      },
      ordinaryOnly,
      'public',
      nowIso,
    )

    expect(projection.contentRewardCount).toBe(1)
    expect(projection.acceptedContributionCount).toBe(0)
    expect(projection.acceptedWorkPayoutClaimAllowed).toBe(false)
    expect(projection.acceptedWorkProofRefs).toEqual([])
  })

  test('rejects live spend authority, spend caps, and mutable authority on this visibility surface', () => {
    const base = exampleArtanisForumRewardVisibilityRecord()
    const bridges = exampleArtanisForumAcceptedContributionBridgeProjections(
      'operator',
      nowIso,
    )

    expect(() =>
      projectArtanisForumRewardVisibility({
        ...base,
        walletAuthorityRefs: ['wallet.public.operator_approved'],
      }, bridges, 'operator', nowIso),
    ).toThrow(ArtanisForumRewardVisibilityUnsafe)
    expect(() =>
      projectArtanisForumRewardVisibility({
        ...base,
        spendCapRefs: ['spend_cap.public.operator_approved'],
      }, bridges, 'operator', nowIso),
    ).toThrow(ArtanisForumRewardVisibilityUnsafe)
    expect(() =>
      projectArtanisForumRewardVisibility({
        ...base,
        authority: {
          ...ARTANIS_FORUM_REWARD_VISIBILITY_READ_ONLY_AUTHORITY,
          noLiveWalletSpend: false,
        },
      }, bridges, 'operator', nowIso),
    ).toThrow(ArtanisForumRewardVisibilityUnsafe)
  })

  test('rejects raw payment, wallet, payout, customer, provider, and timestamp material', () => {
    const base = exampleArtanisForumRewardVisibilityRecord()
    const bridges = exampleArtanisForumAcceptedContributionBridgeProjections(
      'operator',
      nowIso,
    )

    for (const unsafe of [
      { ...base, forumReceiptRefs: ['invoice.lnbc123'] },
      { ...base, postRewardRefs: ['payment_id.raw_reward'] },
      { ...base, acceptedWorkProofRefs: ['payout_target.raw_destination'] },
      { ...base, caveatRefs: ['wallet.secret.seed'] },
      { ...base, sourceRefs: ['provider_token.local'] },
      { ...base, earningActorRefs: ['customer_email_ben@example.com'] },
      { ...base, topicBoostRefs: ['boost.public.2026-06-07T02:00:00.000Z'] },
    ]) {
      expect(() =>
        projectArtanisForumRewardVisibility(unsafe, bridges, 'operator', nowIso),
      ).toThrow(ArtanisForumRewardVisibilityUnsafe)
    }
  })
})
