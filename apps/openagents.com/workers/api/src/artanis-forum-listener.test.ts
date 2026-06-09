import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_FORUM_LISTENER_NO_EXTRA_AUTHORITY,
  ArtanisForumListenerProjection,
  ArtanisForumListenerUnsafe,
  artanisForumListenerProjectionHasPrivateMaterial,
  exampleArtanisForumListenerInput,
  projectArtanisForumListener,
} from './artanis-forum-listener'

const nowIso = '2026-06-07T04:15:00.000Z'

describe('Artanis Forum listener', () => {
  test('detects a public-safe question and produces a reply publication intent', () => {
    const projection = projectArtanisForumListener(
      exampleArtanisForumListenerInput(),
      nowIso,
    )
    const decision = projection.decisions[0]!

    expect(S.decodeUnknownSync(ArtanisForumListenerProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      agentId: 'agent_artanis',
      decisionCount: 1,
      forbiddenAuthority: ARTANIS_FORUM_LISTENER_NO_EXTRA_AUTHORITY,
      notificationCount: 1,
      notificationReadIntentCount: 1,
      replyDraftCount: 1,
    })
    expect(decision).toMatchObject({
      decisionKind: 'reply_draft',
      notificationId: 'mention:88888888-7001-4001-8001-888888888888',
      targetTopicRef: 'topic.public.forum.artanis.status',
    })
    expect(decision.publicationIntent).toMatchObject({
      authorAgentId: 'agent_artanis',
      deliveryState: 'ready',
      idempotencyKey:
        'artanis-forum-listener:reply:mention_88888888_7001_4001_8001_888888888888:v1',
      intentRef:
        'forum.public.artanis.reply_intent.mention_88888888_7001_4001_8001_888888888888',
      targetForumRef: 'forum.public.artanis',
      targetTopicRef: 'topic.public.forum.artanis.status',
    })
    expect(projection.notificationReadIntents[0]).toMatchObject({
      decisionReceiptRefs: [
        'receipt.public.artanis.forum_listener.mention_88888888_7001_4001_8001_888888888888',
      ],
      idempotencyKey:
        'artanis-forum-listener:notification-read:mention_88888888_7001_4001_8001_888888888888:v1',
      notificationId: 'mention:88888888-7001-4001-8001-888888888888',
    })
    expect(projection.watchIntents.map(intent => intent.targetTopicRef))
      .toEqual(
        expect.arrayContaining([
          'topic.public.forum.artanis.status',
          'topic.public.forum.artanis.model_lab',
          'topic.public.forum.artanis.resource_modes',
        ]),
      )
    expect(artanisForumListenerProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('deduplicates notifications and keeps reply drafts idempotent', () => {
    const input = exampleArtanisForumListenerInput()
    const duplicateInput = {
      ...input,
      notifications: [
        input.notifications[0]!,
        {
          ...input.notifications[0]!,
          bodyText: 'A later duplicate should not create another draft.',
        },
      ],
    }

    const first = projectArtanisForumListener(duplicateInput, nowIso)
    const second = projectArtanisForumListener(duplicateInput, nowIso)

    expect(first.notificationCount).toBe(1)
    expect(first.decisionCount).toBe(1)
    expect(first.decisions[0]!.publicationIntent?.intentRef)
      .toBe(second.decisions[0]!.publicationIntent?.intentRef)
    expect(first.decisions[0]!.publicationIntent?.idempotencyKey)
      .toBe(second.decisions[0]!.publicationIntent?.idempotencyKey)
  })

  test('turns unsafe private material into a report intent, not a reply', () => {
    const input = exampleArtanisForumListenerInput()
    const projection = projectArtanisForumListener({
      ...input,
      notifications: [
        {
          ...input.notifications[0]!,
          bodyText:
            'Here is raw_run_log plus wallet.secret material for @artanis.',
        },
      ],
    }, nowIso)
    const decision = projection.decisions[0]!

    expect(projection.replyDraftCount).toBe(0)
    expect(projection.reportIntentCount).toBe(1)
    expect(decision).toMatchObject({
      blockerRefs: [
        'blocker.public.artanis.forum_listener_unsafe_material',
      ],
      decisionKind: 'moderation_report_intent',
      publicationIntent: null,
      reportIntentRefs: [
        'report.public.artanis.forum_listener.mention_88888888_7001_4001_8001_888888888888',
      ],
    })
    expect(projection.forbiddenAuthority.moderationAllowed).toBe(false)
  })

  test('classifies operator questions and work-routing posts without publishing', () => {
    const input = exampleArtanisForumListenerInput()
    const projection = projectArtanisForumListener({
      ...input,
      notifications: [
        {
          ...input.notifications[0]!,
          bodyText: 'Should an operator approve the next launch gate?',
          id: 'mention:operator-question',
        },
        {
          ...input.notifications[0]!,
          bodyText: 'Route a Pylon inference job through work routing.',
          id: 'watched_topic_reply:pylon-work',
          kind: 'watched_topic_reply',
        },
      ],
    }, nowIso)

    expect(projection.operatorQuestionCount).toBe(1)
    expect(projection.workRoutingProposalCount).toBe(1)
    expect(projection.replyDraftCount).toBe(0)
    expect(projection.decisions.map(decision => decision.decisionKind))
      .toEqual(['operator_question', 'work_routing_proposal'])
    expect(projection.decisions[0]!.operatorQuestionRefs).toEqual([
      'question.public.artanis.operator.mention_operator_question',
    ])
    expect(projection.decisions[1]!.workRoutingProposalRefs).toEqual([
      'proposal.public.artanis.work_routing.watched_topic_reply_pylon_work',
    ])
  })

  test('does not create read intents for already-read notifications', () => {
    const input = exampleArtanisForumListenerInput()
    const projection = projectArtanisForumListener({
      ...input,
      notifications: [
        {
          ...input.notifications[0]!,
          readAt: '5 minutes ago',
          readState: 'read',
        },
      ],
    }, nowIso)

    expect(projection.decisionCount).toBe(1)
    expect(projection.notificationReadIntentCount).toBe(0)
    expect(projection.notificationReadIntents).toEqual([])
  })

  test('rejects unsafe listener refs before projecting public state', () => {
    const input = exampleArtanisForumListenerInput()

    expect(() =>
      projectArtanisForumListener({
        ...input,
        sourceRefs: ['source.private.raw_run_log'],
      }, nowIso),
    ).toThrow(ArtanisForumListenerUnsafe)
  })
})
