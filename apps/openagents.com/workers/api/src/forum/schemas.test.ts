import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ForumBoardIndexResponse,
  ForumCreateReplyPostRequest,
  ForumCreateReplyPostResponse,
  ForumCreateTopicRequest,
  type ForumCreateTopicResponse,
  ForumCreateTopicResponse as ForumCreateTopicResponseSchema,
  ForumDiscoverability,
  ForumObjectKind,
  ForumPaidActionPreviewRequest,
  ForumPaidActionPreviewResponse,
  ForumPaidActionRedeemRequest,
  ForumPaidActionRedeemResponse,
  ForumPostDetailResponse,
  ForumPublicProjection,
  ForumPublicProjectionUnsafe,
  ForumReceiptLookupResponse,
  ForumTopicDetailResponse,
  ForumTopicListResponse,
  ForumWriteDenial,
  decodeForumPublicProjection,
  forumCreateTopicResponseHasFirstPost,
  forumWriteDenialRequiresPayment,
} from './schemas'
import { forumTipSettlementProjectionForState } from './tip-settlement'

const publicProjection = {
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: ['payment_private.invoice_redacted'],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: ['artifact.public_post_render_1'],
  safeReceiptRefs: ['receipt.forum.reward.public_1'],
  trustTier: 'reviewed',
}

const actor = {
  actorId: '6b0f1d3a-2b8a-4f2f-9b73-b1d2fd1b55c8',
  actorRef: 'actor.ben',
  displayName: 'Ben',
  groupRefs: ['group.site_owners'],
  isAgent: false,
  slug: 'ben-silone',
}

const missingTipRecipientReadiness = {
  actorRef: actor.actorRef,
  blockerRef: 'blocker.public.forum_tip_recipient.wallet_missing',
  caveatRefs: ['caveat.public.forum_tip_recipient.wallet_not_admitted'],
  directPayment: null,
  providerClass: null,
  readinessRefs: [],
  sourceRef: 'forum_tip_recipient_wallets',
  state: 'missing',
  tippingAvailable: false,
}

const forum = {
  boardId: '11111111-1111-4111-8111-111111111111',
  categoryId: '22222222-2222-4222-8222-222222222222',
  descriptionRef: 'content.forum.site_builder_help.description',
  discoverability: 'listed',
  forumId: '33333333-3333-4333-8333-333333333333',
  latestPostId: '66666666-6666-4666-8666-666666666666',
  latestTopicId: '44444444-4444-4444-8444-444444444444',
  locked: false,
  postCount: 2,
  publicProjection,
  slug: 'site-builder-help',
  title: 'Site Builder Help',
  topicCount: 1,
  visibility: 'public',
}

const topic = {
  author: actor,
  createdAt: '2026-06-05T18:00:00.000Z',
  firstPostId: '55555555-5555-4555-8555-555555555555',
  forumId: forum.forumId,
  latestPostId: '66666666-6666-4666-8666-666666666666',
  pinState: 'normal',
  postCount: 2,
  publicProjection,
  scoreRef: 'forum_score.topic.otec.v1',
  slug: 'otc-floating-datacenter',
  state: 'open',
  title: 'OTEC Floating Datacenter',
  topicHref: '/forum/t/44444444-4444-4444-8444-444444444444',
  topicId: '44444444-4444-4444-8444-444444444444',
  updatedAt: '2026-06-05T18:05:00.000Z',
  webUrl: 'https://openagents.com/forum/t/44444444-4444-4444-8444-444444444444',
}

const firstPost = {
  author: actor,
  contentRef: 'content.post.otec.first',
  createdAt: '2026-06-05T18:00:00.000Z',
  parentPostId: null,
  postId: '55555555-5555-4555-8555-555555555555',
  postNumber: 1,
  publicProjection,
  quotePostId: null,
  receiptRefs: ['receipt.forum.topic_create.otec'],
  revisionRef: null,
  state: 'visible',
  tipRecipientReadiness: missingTipRecipientReadiness,
  topicId: topic.topicId,
  updatedAt: '2026-06-05T18:00:00.000Z',
}

const replyPost = {
  ...firstPost,
  contentRef: 'content.post.otec.reply_1',
  createdAt: '2026-06-05T18:05:00.000Z',
  parentPostId: firstPost.postId,
  postId: '66666666-6666-4666-8666-666666666666',
  postNumber: 2,
  quotePostId: firstPost.postId,
  receiptRefs: ['receipt.forum.reply.otec_1'],
  updatedAt: '2026-06-05T18:05:00.000Z',
}

const pagination = {
  cursor: null,
  hasMore: false,
  limit: 20,
  nextCursor: null,
}

describe('Forum API schemas', () => {
  test('decodes a public board index with categories and forums', () => {
    const boardIndex = {
      boardId: forum.boardId,
      categories: [
        {
          boardId: forum.boardId,
          categoryId: forum.categoryId,
          descriptionRef: 'content.category.sites.description',
          discoverability: 'listed',
          forumIds: [forum.forumId],
          orderIndex: 1,
          slug: 'sites',
          title: 'Sites',
        },
      ],
      forums: [forum],
      generatedAt: '2026-06-05T18:10:00.000Z',
      publicProjection,
      slug: 'openagents',
      title: 'OpenAgents',
    }

    expect(S.decodeUnknownSync(ForumBoardIndexResponse)(boardIndex)).toEqual(
      boardIndex,
    )
  })

  test('decodes topic list, topic detail, post detail, and reply request shapes', () => {
    expect(
      S.decodeUnknownSync(ForumTopicListResponse)({
        forum,
        pagination,
        topics: [topic],
      }),
    ).toEqual({
      forum,
      pagination,
      topics: [topic],
    })

    expect(
      S.decodeUnknownSync(ForumTopicDetailResponse)({
        pagination,
        posts: [firstPost, replyPost],
        topic,
      }),
    ).toEqual({
      pagination,
      posts: [firstPost, replyPost],
      topic,
    })

    expect(
      S.decodeUnknownSync(ForumPostDetailResponse)({
        containingTopicId: topic.topicId,
        post: replyPost,
      }),
    ).toEqual({
      containingTopicId: topic.topicId,
      post: replyPost,
    })

    expect(
      S.decodeUnknownSync(ForumCreateReplyPostRequest)({
        actorRef: actor.actorRef,
        contentRef: 'content.post.otec.reply_2',
        idempotencyKey:
          'forum:reply:44444444-4444-4444-8444-444444444444:actor.ben:2',
        parentPostId: firstPost.postId,
        paymentProofRef: null,
        quotePostId: firstPost.postId,
        topicId: topic.topicId,
      }),
    ).toEqual({
      actorRef: actor.actorRef,
      contentRef: 'content.post.otec.reply_2',
      idempotencyKey:
        'forum:reply:44444444-4444-4444-8444-444444444444:actor.ben:2',
      parentPostId: firstPost.postId,
      paymentProofRef: null,
      quotePostId: firstPost.postId,
      topicId: topic.topicId,
    })
  })

  test('models topic creation as a topic plus first post', () => {
    const request = {
      actorRef: actor.actorRef,
      firstPost: {
        contentRef: firstPost.contentRef,
        quotePostId: null,
      },
      forumId: forum.forumId,
      idempotencyKey:
        'forum:topic:33333333-3333-4333-8333-333333333333:actor.ben:1',
      paymentProofRef: 'entitlement.forum.topic_create.1',
      requestedSlug: 'otc-floating-datacenter',
      title: topic.title,
    }

    const response = S.decodeUnknownSync(ForumCreateTopicResponseSchema)({
      firstPost,
      receiptRefs: ['receipt.forum.topic_create.otec'],
      topic,
    })

    expect(S.decodeUnknownSync(ForumCreateTopicRequest)(request)).toEqual(
      request,
    )
    expect(
      forumCreateTopicResponseHasFirstPost(
        response as ForumCreateTopicResponse,
      ),
    ).toBe(true)
  })

  test('decodes paid-action preview, redeem, receipt, and reply response envelopes', () => {
    const target = {
      forumId: forum.forumId,
      postId: firstPost.postId,
      topicId: topic.topicId,
    }
    const price = {
      amount: 100,
      asset: 'sats',
    }
    const challenge = {
      actionKind: 'post_reward',
      actorRef: actor.actorRef,
      challengeId: '77777777-7777-4777-8777-777777777777',
      expiresAt: '2026-06-05T18:15:00.000Z',
      l402: {
        acceptedWorkSettlementAuthority: false,
        checkoutLaunchPath: '/checkout/forum_reward_1',
        checkoutRef: 'mdk_checkout.forum_reward_1',
        checkoutUrlRef: 'mdk_checkout_url.forum_reward_1',
        credentialRef: 'credential.forum_l402.reward_1',
        endpointRef: 'endpoint.forum_paid_action.post_reward',
        entitlementScopeRefs: ['entitlement.forum.post_reward.single'],
        environment: 'sandbox',
        implementationState: 'fake_provider_contract',
        invoiceRef: 'mdk_invoice.redacted.forum_reward_1',
        paymentHashRef: 'mdk_payment_hash.redacted.forum_reward_1',
        provider: 'mdk_hosted',
        providerMode: 'hosted_mdk',
        providerPayoutAuthority: false,
        providerRef: 'provider.forum.mdk.sandbox',
        replayNonceRef: 'replay_nonce.forum_l402.reward_1',
        sandbox: true,
        settlementAuthority: 'buyer_payment_evidence_only',
        wwwAuthenticate: 'L402 challenge_ref="challenge.forum_l402.reward_1"',
      },
      method: 'POST',
      path: `/api/forum/posts/${firstPost.postId}/rewards`,
      price,
      recipientActorRef: actor.actorRef,
      recipientReadinessRef:
        'readiness.public.forum_tip_recipient.receive_ready',
      requestBodyDigest: 'sha256:reward-body',
      routeParams: {
        postId: firstPost.postId,
      },
      spendCap: price,
      target,
    }

    expect(
      S.decodeUnknownSync(ForumPaidActionPreviewRequest)({
        actionKind: 'post_reward',
        actorRef: actor.actorRef,
        idempotencyKey:
          'forum:reward:55555555-5555-4555-8555-555555555555:actor.ben:1',
        method: 'POST',
        path: `/api/forum/posts/${firstPost.postId}/rewards`,
        requestBodyDigest: 'sha256:reward-body',
        routeParams: {
          postId: firstPost.postId,
        },
        spendCap: price,
        target,
      }),
    ).toEqual({
      actionKind: 'post_reward',
      actorRef: actor.actorRef,
      idempotencyKey:
        'forum:reward:55555555-5555-4555-8555-555555555555:actor.ben:1',
      method: 'POST',
      path: `/api/forum/posts/${firstPost.postId}/rewards`,
      requestBodyDigest: 'sha256:reward-body',
      routeParams: {
        postId: firstPost.postId,
      },
      spendCap: price,
      target,
    })

    expect(
      S.decodeUnknownSync(ForumPaidActionPreviewResponse)({
        challenge,
        entitlementRef: null,
        paymentRequired: true,
        writeDenial: null,
      }),
    ).toEqual({
      challenge,
      entitlementRef: null,
      paymentRequired: true,
      writeDenial: null,
    })

    expect(
      S.decodeUnknownSync(ForumPaidActionRedeemRequest)({
        actorRef: actor.actorRef,
        challengeId: challenge.challengeId,
        idempotencyKey: 'forum:redeem:77777777-7777-4777-8777-777777777777',
        l402ProofRef: 'l402.proof.redacted_1',
      }),
    ).toEqual({
      actorRef: actor.actorRef,
      challengeId: challenge.challengeId,
      idempotencyKey: 'forum:redeem:77777777-7777-4777-8777-777777777777',
      l402ProofRef: 'l402.proof.redacted_1',
    })

    expect(
      S.decodeUnknownSync(ForumPaidActionRedeemResponse)({
        entitlementRef: 'entitlement.forum.reward.1',
        originalReceiptRef: null,
        receiptRef: 'receipt.forum.reward.1',
        replayed: false,
      }),
    ).toEqual({
      entitlementRef: 'entitlement.forum.reward.1',
      originalReceiptRef: null,
      receiptRef: 'receipt.forum.reward.1',
      replayed: false,
    })

    expect(
      S.decodeUnknownSync(ForumReceiptLookupResponse)({
        actionKind: 'post_reward',
        amount: price,
        createdAt: '2026-06-05T18:12:00.000Z',
        paymentEvent: {
          actionKind: 'post_reward',
          amount: price,
          challengeId: challenge.challengeId,
          createdAt: '2026-06-05T18:12:00.000Z',
          externalRef: 'external.payment.redacted_1',
          payerActorRef: actor.actorRef,
          paymentEventRef: 'payment_event.forum.reward_1',
          paymentMode: 'signet',
          providerRef: 'provider.mdk_l402.redacted',
          receiptRef: 'receipt.forum.reward.1',
          recipientActorRef: actor.actorRef,
          redactedEvidenceRef: 'evidence.payment.redacted_1',
          status: 'confirmed',
        },
        publicProjection,
        receiptRef: 'receipt.forum.reward.1',
        recipientActorRef: actor.actorRef,
        settlementClaim: null,
        target,
        targetPostPermalink:
          'https://openagents.com/forum/t/44444444-4444-4444-8444-444444444444#post-55555555-5555-4555-8555-555555555555',
        tipSettlement: forumTipSettlementProjectionForState('paid'),
      }),
    ).toEqual({
      actionKind: 'post_reward',
      amount: price,
      createdAt: '2026-06-05T18:12:00.000Z',
      paymentEvent: {
        actionKind: 'post_reward',
        amount: price,
        challengeId: challenge.challengeId,
        createdAt: '2026-06-05T18:12:00.000Z',
        externalRef: 'external.payment.redacted_1',
        payerActorRef: actor.actorRef,
        paymentEventRef: 'payment_event.forum.reward_1',
        paymentMode: 'signet',
        providerRef: 'provider.mdk_l402.redacted',
        receiptRef: 'receipt.forum.reward.1',
        recipientActorRef: actor.actorRef,
        redactedEvidenceRef: 'evidence.payment.redacted_1',
        status: 'confirmed',
      },
      publicProjection,
      receiptRef: 'receipt.forum.reward.1',
      recipientActorRef: actor.actorRef,
      settlementClaim: null,
      target,
      targetPostPermalink:
        'https://openagents.com/forum/t/44444444-4444-4444-8444-444444444444#post-55555555-5555-4555-8555-555555555555',
      tipSettlement: forumTipSettlementProjectionForState('paid'),
    })

    expect(
      S.decodeUnknownSync(ForumCreateReplyPostResponse)({
        post: replyPost,
        receiptRefs: ['receipt.forum.reply.otec_1'],
        topic,
      }),
    ).toEqual({
      post: replyPost,
      receiptRefs: ['receipt.forum.reply.otec_1'],
      topic,
    })
  })

  test('keeps payment denial distinct from ACL and scope denial', () => {
    const paymentDenial = S.decodeUnknownSync(ForumWriteDenial)({
      actorRef: actor.actorRef,
      denialKind: 'payment_required',
      denialRef: 'denial.forum.payment_required.topic_create',
      payable: true,
      requiredPermission: 'f_create_topic',
    })
    const moderatorDenial = S.decodeUnknownSync(ForumWriteDenial)({
      actorRef: actor.actorRef,
      denialKind: 'scope_denied',
      denialRef: 'denial.forum.scope.m_lock_topic',
      payable: false,
      requiredPermission: 'm_lock_topic',
    })
    const recipientDenial = S.decodeUnknownSync(ForumWriteDenial)({
      actorRef: actor.actorRef,
      denialKind: 'recipient_not_ready',
      denialRef: 'blocker.public.forum_tip_recipient.wallet_missing',
      payable: false,
      requiredPermission: null,
    })

    expect(forumWriteDenialRequiresPayment(paymentDenial)).toBe(true)
    expect(forumWriteDenialRequiresPayment(moderatorDenial)).toBe(false)
    expect(forumWriteDenialRequiresPayment(recipientDenial)).toBe(false)
  })

  test('rejects invalid enum values and private projection material', () => {
    expect(() => S.decodeUnknownSync(ForumObjectKind)('submolt')).toThrow()
    expect(() =>
      S.decodeUnknownSync(ForumDiscoverability)('search_only'),
    ).toThrow()

    expect(() =>
      S.decodeUnknownSync(ForumPaidActionPreviewRequest)({
        actionKind: 'nostr_relay_publish',
        actorRef: actor.actorRef,
        idempotencyKey: 'forum:nostr:1',
        method: 'POST',
        path: '/api/forum/relay',
        requestBodyDigest: 'sha256:relay',
        routeParams: {},
        spendCap: {
          amount: 100,
          asset: 'sats',
        },
        target: {
          forumId: forum.forumId,
          postId: null,
          topicId: null,
        },
      }),
    ).toThrow()

    expect(() =>
      decodeForumPublicProjection({
        ...publicProjection,
        rawInvoiceRef: 'lnbc2500n1rawinvoice',
      }),
    ).toThrow(ForumPublicProjectionUnsafe)

    expect(() =>
      decodeForumPublicProjection({
        ...publicProjection,
        dataClassification: 'payment_private',
      }),
    ).toThrow(ForumPublicProjectionUnsafe)
  })

  test('accepts only public-safe projection payloads through the projection decoder', () => {
    expect(decodeForumPublicProjection(publicProjection)).toEqual(
      publicProjection,
    )
    expect(
      S.decodeUnknownSync(ForumPublicProjection)(publicProjection),
    ).toEqual(publicProjection)
  })
})
