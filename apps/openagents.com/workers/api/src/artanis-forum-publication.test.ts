import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisForumPublicationIntentRecord,
  ArtanisForumPublicationQueueProjection,
  ArtanisForumPublicationQueueRecord,
  ArtanisForumPublicationUnsafe,
  artanisForumPublicationProjectionHasPrivateMaterial,
  exampleArtanisForumPublicationQueue,
  projectArtanisForumPublicationQueue,
  selectReadyArtanisForumPublicationIntents,
} from './artanis-forum-publication'

const nowIso = '2026-06-07T01:30:00.000Z'

const queueRecord = (
  overrides: Partial<ArtanisForumPublicationQueueRecord> = {},
): ArtanisForumPublicationQueueRecord =>
  S.decodeUnknownSync(ArtanisForumPublicationQueueRecord)({
    ...exampleArtanisForumPublicationQueue(),
    ...overrides,
  })

const intentRecord = (
  overrides: Partial<ArtanisForumPublicationIntentRecord> = {},
): ArtanisForumPublicationIntentRecord =>
  S.decodeUnknownSync(ArtanisForumPublicationIntentRecord)({
    ...exampleArtanisForumPublicationQueue().intents[0]!,
    ...overrides,
  })

describe('Artanis Forum publication queue', () => {
  test('projects public-safe publication intents with refs, topic targets, and post state', () => {
    const projection = projectArtanisForumPublicationQueue(
      exampleArtanisForumPublicationQueue(),
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisForumPublicationQueueProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      agentId: 'agent_artanis',
      deliveredCount: 0,
      deliverableIntentRefs: [
        'forum.public.artanis.status_intent.20260607T0121',
      ],
      duplicateIntentRefs: [],
      intentCount: 1,
      redactionPolicyRef: 'redaction.forum.public.artanis.v1',
    })
    expect(projection.intents[0]).toMatchObject({
      deliveryState: 'ready',
      goalRefs: ['goal.public.artanis.pylon_model_lab'],
      idempotencyKey: 'artanis-forum:status:20260607T0121:v1',
      modelLabReportRefs: [
        'model_lab.public.report.autopilot_benchmark_loop',
      ],
      pageUrls: [
        'https://openagents.com/artanis',
        'https://openagents.com/forum/f/artanis',
      ],
      postRef: null,
      pylonNexusPublicRefs: [
        'campaign.public.pylon.v0_2',
        'omega.public.pylon_api.registrations',
        'pylon.public.resource_modes',
      ],
      r10ClaimRefs: ['claim.public.r10.pylon_learning_loop'],
      receiptRefs: ['receipt.public.artanis.loop_closeout'],
      targetTopicRef: 'topic.public.forum.artanis.status',
      targetTopicState: 'open',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(artanisForumPublicationProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('makes exact retry intents idempotent and exposes duplicate intent refs', () => {
    const base = exampleArtanisForumPublicationQueue()
    const retry = intentRecord({
      ...base.intents[0]!,
      createdAtIso: '2026-06-07T01:23:00.000Z',
      intentRef: 'forum.public.artanis.status_intent.retry_duplicate',
      updatedAtIso: '2026-06-07T01:24:00.000Z',
    })
    const projection = projectArtanisForumPublicationQueue(
      queueRecord({ intents: [base.intents[0]!, retry] }),
      nowIso,
    )

    expect(projection.intentCount).toBe(1)
    expect(projection.intents[0]!.intentRef).toBe(
      'forum.public.artanis.status_intent.20260607T0121',
    )
    expect(projection.duplicateIntentRefs).toEqual([
      'forum.public.artanis.status_intent.retry_duplicate',
    ])
    expect(selectReadyArtanisForumPublicationIntents(
      queueRecord({ intents: [base.intents[0]!, retry] }),
    )).toHaveLength(1)
  })

  test('rejects idempotency key reuse for a different target or body', () => {
    const base = exampleArtanisForumPublicationQueue()
    const changedBody = intentRecord({
      ...base.intents[0]!,
      bodyText:
        'Artanis changed this status update while reusing the same idempotency key.',
      intentRef: 'forum.public.artanis.status_intent.conflict',
    })
    const changedTarget = intentRecord({
      ...base.intents[0]!,
      intentRef: 'forum.public.artanis.status_intent.target_conflict',
      targetTopicRef: 'topic.public.forum.artanis.operator_questions',
    })

    expect(() =>
      projectArtanisForumPublicationQueue(
        queueRecord({ intents: [base.intents[0]!, changedBody] }),
        nowIso,
      ),
    ).toThrow(ArtanisForumPublicationUnsafe)
    expect(() =>
      projectArtanisForumPublicationQueue(
        queueRecord({ intents: [base.intents[0]!, changedTarget] }),
        nowIso,
      ),
    ).toThrow(ArtanisForumPublicationUnsafe)
  })

  test('denies ready or delivered posts to locked, hidden, archived, or unavailable topics', () => {
    for (const targetTopicState of [
      'locked',
      'hidden',
      'archived',
      'unavailable',
    ] as const) {
      expect(() =>
        projectArtanisForumPublicationQueue(
          queueRecord({
            intents: [
              intentRecord({
                blockerRefs: [],
                deliveryState: 'ready',
                targetTopicState,
              }),
            ],
          }),
          nowIso,
        ),
      ).toThrow(ArtanisForumPublicationUnsafe)
    }

    const blocked = projectArtanisForumPublicationQueue(
      queueRecord({
        intents: [
          intentRecord({
            blockerRefs: ['blocker.public.forum.topic_locked'],
            deliveryState: 'blocked',
            targetTopicState: 'locked',
          }),
        ],
      }),
      nowIso,
    )

    expect(blocked.deliverableIntentRefs).toEqual([])
    expect(blocked.intents[0]!.blockerRefs).toEqual([
      'blocker.public.forum.topic_locked',
    ])
  })

  test('rejects unsafe refs, non-public URLs, and raw/private body material before posting', () => {
    const base = exampleArtanisForumPublicationQueue()

    for (const unsafeIntent of [
      intentRecord({
        ...base.intents[0]!,
        modelLabReportRefs: ['model_lab.private.raw_weights'],
      }),
      intentRecord({
        ...base.intents[0]!,
        pageUrls: ['https://openagents.com/artanis?token=secret'],
      }),
      intentRecord({
        ...base.intents[0]!,
        bodyText:
          'Artanis status update includes raw prompt data and sk-secret material.',
      }),
      intentRecord({
        ...base.intents[0]!,
        redactionPolicyRef: 'redaction.private.operator.v1',
      }),
    ]) {
      expect(() =>
        projectArtanisForumPublicationQueue(
          queueRecord({ intents: [unsafeIntent] }),
          nowIso,
        ),
      ).toThrow(ArtanisForumPublicationUnsafe)
    }
  })

  test('requires delivered intents to carry post and delivery receipt state', () => {
    const delivered = intentRecord({
      deliveredAtIso: '2026-06-07T01:25:00.000Z',
      deliveryReceiptRefs: ['receipt.public.artanis.forum_status_post'],
      deliveryState: 'delivered',
      postRef: 'post.public.forum.artanis.status.1',
      updatedAtIso: '2026-06-07T01:25:00.000Z',
    })
    const projection = projectArtanisForumPublicationQueue(
      queueRecord({ intents: [delivered] }),
      nowIso,
    )

    expect(projection.deliveredCount).toBe(1)
    expect(projection.deliverableIntentRefs).toEqual([])
    expect(projection.intents[0]).toMatchObject({
      deliveredAtDisplay: '5 minutes ago',
      deliveryReceiptRefs: ['receipt.public.artanis.forum_status_post'],
      postRef: 'post.public.forum.artanis.status.1',
    })

    expect(() =>
      projectArtanisForumPublicationQueue(
        queueRecord({
          intents: [
            intentRecord({
              deliveryState: 'delivered',
              postRef: null,
            }),
          ],
        }),
        nowIso,
      ),
    ).toThrow(ArtanisForumPublicationUnsafe)
  })
})
