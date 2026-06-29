import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ForumTipAbusePolicyProjection,
  ForumTipPreviewRateLimit,
  ForumTipSelfTippingDenialRef,
  forumTipAbusePolicyHasPrivateMaterial,
  forumTipImmediatePreviewPolicyDenial,
  forumTipRateLimitPreviewPolicyDenial,
  projectForumTipAbusePolicy,
} from './tip-abuse-policy'

describe('Forum tip abuse/refund policy', () => {
  test('projects a public-safe policy for self-tipping, duplicate tips, refunds, reversals, and payment boundaries', () => {
    const projection = projectForumTipAbusePolicy()

    expect(
      S.decodeUnknownSync(ForumTipAbusePolicyProjection)(projection),
    ).toEqual(projection)
    expect(projection.selfTipping).toStrictEqual({
      denialKind: 'safety_denied',
      denialRef: ForumTipSelfTippingDenialRef,
      state: 'blocked',
    })
    expect(projection.rateLimit).toMatchObject({
      denialKind: 'rate_limited',
      limit: ForumTipPreviewRateLimit.limit,
      windowSeconds: ForumTipPreviewRateLimit.windowSeconds,
    })
    expect(projection.refundSettlementStates).toEqual(['refunded'])
    expect(projection.reversalSettlementStates).toEqual(['reversed'])
    expect(projection.paymentCannotUnlockRefs).toEqual(
      expect.arrayContaining([
        'scope.public.forum_tip.cannot_unlock_admin',
        'scope.public.forum_tip.cannot_unlock_moderation',
        'scope.public.forum_tip.cannot_unlock_owner',
        'scope.public.forum_tip.cannot_unlock_private_data',
        'scope.public.forum_tip.cannot_unlock_safety',
      ]),
    )
    expect(forumTipAbusePolicyHasPrivateMaterial(projection)).toBe(false)
    expect(JSON.stringify(projection)).not.toContain('lnbc1')
    expect(JSON.stringify(projection)).not.toContain('payment_preimage')
    expect(JSON.stringify(projection)).not.toContain('/Users/')
  })

  test('blocks self-tipping before challenge issuance', () => {
    expect(
      forumTipImmediatePreviewPolicyDenial({
        actionKind: 'post_reward',
        actorRef: 'actor.alice',
        recipientActorRef: 'actor.alice',
      }),
    ).toStrictEqual({
      denialKind: 'safety_denied',
      denialRef: ForumTipSelfTippingDenialRef,
      requiredPermission: null,
    })

    expect(
      forumTipImmediatePreviewPolicyDenial({
        actionKind: 'post_reward',
        actorRef: 'actor.alice',
        recipientActorRef: 'actor.ben',
      }),
    ).toBeNull()
  })

  test('rate-limits new post reward challenges after the configured window count', () => {
    expect(
      forumTipRateLimitPreviewPolicyDenial({
        actionKind: 'post_reward',
        recentChallengeCount: ForumTipPreviewRateLimit.limit - 1,
      }),
    ).toBeNull()
    expect(
      forumTipRateLimitPreviewPolicyDenial({
        actionKind: 'post_reward',
        recentChallengeCount: ForumTipPreviewRateLimit.limit,
      }),
    ).toMatchObject({
      denialKind: 'rate_limited',
    })
    expect(
      forumTipRateLimitPreviewPolicyDenial({
        actionKind: 'post_boost',
        recentChallengeCount: ForumTipPreviewRateLimit.limit,
      }),
    ).toBeNull()
  })
})
