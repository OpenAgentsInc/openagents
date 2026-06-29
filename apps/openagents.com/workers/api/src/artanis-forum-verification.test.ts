import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_FORUM_VERIFICATION_NO_EXTRA_AUTHORITY,
  ArtanisForumVerificationProjection,
  ArtanisForumVerificationUnsafe,
  exampleArtanisForumVerificationRecord,
  projectArtanisForumVerification,
} from './artanis-forum-verification'

const nowIso = '2026-06-07T07:30:00.000Z'

describe('Artanis Forum verification evidence', () => {
  test('projects delivered status-post evidence without granting authority', () => {
    const projection = projectArtanisForumVerification(
      exampleArtanisForumVerificationRecord(nowIso),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisForumVerificationProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      acceptedWorkPayoutAllowed: false,
      approvedDeliveryBridgeRequired: true,
      audience: 'public',
      deliveredPostCount: 1,
      deliveryState: 'delivered',
      directForumPublishAllowed: false,
      dispatchAllowed: false,
      listenerNotificationCount: 1,
      listenerState: 'reply_draft',
      moderationAllowed: false,
      normalAgentPostingAllowed: false,
      paymentSpendAllowed: false,
      privateEvidenceRefs: [],
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      pylonReleaseWorkLogEvidencePresent: true,
      schedulerEnablementAllowed: false,
      state: 'verified',
      statusTopicEvidencePresent: true,
      walletSpendAllowed: false,
    })
    expect(projection.triageDraftRefs).toContain(
      'draft.public.artanis.forum_listener.status_question_reply',
    )
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(JSON.stringify(projection)).not.toContain('redacted_notification')
  })

  test('retains safe operator private evidence refs by reference', () => {
    const projection = projectArtanisForumVerification(
      exampleArtanisForumVerificationRecord(nowIso),
      'operator',
      nowIso,
    )

    expect(projection.privateEvidenceRefs).toEqual([
      'evidence.operator.artanis.forum_listener.redacted_notification_digest',
    ])
    expect(projection.state).toBe('verified')
  })

  test('records a no-new-post listener pass with read/no-op refs', () => {
    const projection = projectArtanisForumVerification(
      {
        ...exampleArtanisForumVerificationRecord(nowIso),
        listenerNotificationRefs: [],
        listenerState: 'no_new_posts',
        noOpReadRefs: [
          'read.public.artanis.forum_listener.no_new_posts_20260607',
        ],
        triageDraftRefs: [],
      },
      'public',
      nowIso,
    )

    expect(projection.listenerState).toBe('no_new_posts')
    expect(projection.noOpReadCount).toBe(1)
    expect(projection.state).toBe('verified')
  })

  test('records operator-question and reply-draft triage separately', () => {
    const operatorProjection = projectArtanisForumVerification(
      {
        ...exampleArtanisForumVerificationRecord(nowIso),
        listenerState: 'operator_question',
        operatorQuestionRefs: [
          'question.public.artanis.operator.launch_gate_check',
        ],
        triageDraftRefs: [],
      },
      'public',
      nowIso,
    )
    const replyProjection = projectArtanisForumVerification(
      exampleArtanisForumVerificationRecord(nowIso),
      'public',
      nowIso,
    )

    expect(operatorProjection.listenerState).toBe('operator_question')
    expect(operatorProjection.operatorQuestionCount).toBe(1)
    expect(operatorProjection.triageDraftCount).toBe(0)
    expect(replyProjection.listenerState).toBe('reply_draft')
    expect(replyProjection.triageDraftCount).toBe(1)
  })

  test('records duplicate idempotency collapse against the original post', () => {
    const projection = projectArtanisForumVerification(
      {
        ...exampleArtanisForumVerificationRecord(nowIso),
        deliveredPostRefs: ['post.public.forum.artanis.status.2'],
        deliveryState: 'duplicate_collapsed',
        idempotencyRefs: [
          'idempotency.public.artanis.forum_delivery.status_post_2',
          'idempotency.public.artanis.forum_delivery.status_post_2.retry',
        ],
      },
      'public',
      nowIso,
    )

    expect(projection.deliveryState).toBe('duplicate_collapsed')
    expect(projection.deliveredPostRefs).toEqual([
      'post.public.forum.artanis.status.2',
    ])
    expect(projection.idempotencyRefs).toEqual([
      'idempotency.public.artanis.forum_delivery.status_post_2',
      'idempotency.public.artanis.forum_delivery.status_post_2.retry',
    ])
    expect(projection.state).toBe('verified')
  })

  test('records locked, hidden, and archived target-topic blockers', () => {
    const locked = projectArtanisForumVerification(
      {
        ...exampleArtanisForumVerificationRecord(nowIso),
        blockerRefs: [
          'blocker.public.artanis.forum_verification.topic_locked',
        ],
        deliveredPostRefs: [],
        deliveryReceiptRefs: [],
        deliveryState: 'target_blocked',
        idempotencyRefs: [],
        intendedPostRefs: [],
        targetTopicState: 'locked',
      },
      'public',
      nowIso,
    )
    const hidden = projectArtanisForumVerification(
      {
        ...exampleArtanisForumVerificationRecord(nowIso),
        blockerRefs: [
          'blocker.public.artanis.forum_verification.topic_hidden',
        ],
        deliveredPostRefs: [],
        deliveryReceiptRefs: [],
        deliveryState: 'target_blocked',
        idempotencyRefs: [],
        intendedPostRefs: [],
        targetTopicState: 'hidden',
      },
      'public',
      nowIso,
    )
    const archived = projectArtanisForumVerification(
      {
        ...exampleArtanisForumVerificationRecord(nowIso),
        blockerRefs: [
          'blocker.public.artanis.forum_verification.topic_archived',
        ],
        deliveredPostRefs: [],
        deliveryReceiptRefs: [],
        deliveryState: 'target_blocked',
        idempotencyRefs: [],
        intendedPostRefs: [],
        targetTopicState: 'archived',
      },
      'public',
      nowIso,
    )

    expect(locked.state).toBe('blocked')
    expect(hidden.state).toBe('blocked')
    expect(archived.state).toBe('blocked')
    expect(locked.blockerRefs).toContain(
      'blocker.public.artanis.forum_verification.topic_locked',
    )
    expect(hidden.blockerRefs).toContain(
      'blocker.public.artanis.forum_verification.topic_hidden',
    )
    expect(archived.blockerRefs).toContain(
      'blocker.public.artanis.forum_verification.topic_archived',
    )
  })

  test('rejects unsafe private, raw, payment, wallet, provider, and timestamp material', () => {
    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          listenerNotificationRefs: ['raw_forum_payload.full_text'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)

    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          sourceRefs: ['provider_token.raw'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)

    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          deliveryReceiptRefs: ['payment_invoice.lnbc1raw'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)

    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          privateEvidenceRefs: ['wallet.secret.material'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)

    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          caveatRefs: ['caveat.public.checked_at.2026-06-07T07:30:00'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)
  })

  test('rejects mutable authority and missing required evidence', () => {
    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          authority: {
            ...ARTANIS_FORUM_VERIFICATION_NO_EXTRA_AUTHORITY,
            moderationAllowed: true,
          },
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)

    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          targetTopicRefs: ['topic.public.forum.artanis.status'],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)

    expect(() =>
      projectArtanisForumVerification(
        {
          ...exampleArtanisForumVerificationRecord(nowIso),
          listenerState: 'no_new_posts',
          noOpReadRefs: [],
          triageDraftRefs: [],
        },
        'operator',
        nowIso,
      )
    ).toThrow(ArtanisForumVerificationUnsafe)
  })
})
