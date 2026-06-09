import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisLaunchSmokeProjection,
  ArtanisLaunchSmokeUnsafe,
  exampleArtanisLaunchSmokeInput,
  projectArtanisLaunchSmoke,
} from './artanis-launch-smoke'

const nowIso = '2026-06-07T01:35:00.000Z'

describe('Artanis launch smoke', () => {
  test('proves operator steering to loop claim to safe result to Forum post to public summary', () => {
    const projection = projectArtanisLaunchSmoke(
      exampleArtanisLaunchSmokeInput(),
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(S.decodeUnknownSync(ArtanisLaunchSmokeProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      agentId: 'agent_artanis',
      forumIntentRef: 'forum.public.artanis.status_intent',
      forumPostRef: 'post.public.forum.artanis.status.20260607T0124',
      goalRef: 'goal.public.artanis.pylon_model_lab',
      loopRef: 'loop.public.artanis.primary',
      safeActionRef: 'action.public.artanis.status_projection',
      smokeRef: 'smoke.public.artanis.launch_e2e',
      tickRef: 'tick.public.artanis.20260607T0052',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.stages.map(stage => stage.stage)).toEqual([
      'operator_goal',
      'loop_claim',
      'safe_result',
      'forum_post',
      'public_summary',
    ])
    expect(projection.publicSummaryRefs).toEqual(
      expect.arrayContaining([
        'https://openagents.com/artanis',
        '/forum/f/artanis',
        'route:/api/public/artanis/report',
      ]),
    )
    expect(projection.receiptRefs).toEqual(
      expect.arrayContaining([
        'receipt.public.artanis.context_loaded',
        'receipt.public.artanis.forum_status_delivered',
        'receipt.public.artanis.loop_closeout',
        'receipt.public.artanis.tick_closeout',
      ]),
    )
    expect(serialized).not.toContain('authGrantRef')
    expect(serialized).not.toContain('payloadJson')
    expect(serialized).not.toContain('provider_account')
    expect(serialized).not.toContain('raw_runner')
    expect(serialized).not.toContain('wallet.secret')
    expect(serialized).not.toContain('wallet.material')
    expect(serialized).not.toContain('wallet_mnemonic')
    expect(serialized).not.toContain('payment_preimage')
    expect(serialized).not.toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/)
  })

  test('keeps live spend, provider mutation, runtime promotion, and settlement blocked', () => {
    const projection = projectArtanisLaunchSmoke(
      exampleArtanisLaunchSmokeInput(),
      nowIso,
    )

    expect(projection.blockedBeforeAuthorityRefs).toEqual(
      expect.arrayContaining([
        'blocker.artanis.live_spend_requires_operator_gate',
        'blocker.artanis.provider_mutation_requires_authority',
        'blocker.artanis.runtime_promotion_requires_release_gate',
        'blocker.artanis.settlement_requires_public_receipts',
      ]),
    )
  })

  test('rejects launch smoke without a delivered Forum post', () => {
    const input = exampleArtanisLaunchSmokeInput()

    expect(() =>
      projectArtanisLaunchSmoke({
        ...input,
        forumQueue: {
          ...input.forumQueue,
          intents: input.forumQueue.intents.map(intent => ({
            ...intent,
            deliveredAtIso: null,
            deliveryReceiptRefs: [],
            deliveryState: 'ready',
            postRef: null,
          })),
        },
      }, nowIso),
    ).toThrow(ArtanisLaunchSmokeUnsafe)
  })

  test('rejects public summary without the Artanis public page link', () => {
    const input = exampleArtanisLaunchSmokeInput()

    expect(() =>
      projectArtanisLaunchSmoke({
        ...input,
        publicReport: {
          ...input.publicReport,
          publicUrls: [],
        },
      }, nowIso),
    ).toThrow(ArtanisLaunchSmokeUnsafe)
  })

  test('rejects unsafe public launch smoke material', () => {
    const input = exampleArtanisLaunchSmokeInput()

    expect(() =>
      projectArtanisLaunchSmoke({
        ...input,
        smokeRef: 'smoke.private.wallet_secret',
      }, nowIso),
    ).toThrow(ArtanisLaunchSmokeUnsafe)
  })
})
