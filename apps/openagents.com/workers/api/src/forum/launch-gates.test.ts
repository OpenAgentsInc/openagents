import { describe, expect, test } from 'vitest'

import {
  CurrentForumLaunchGateInput,
  forumLaunchGateStatus,
} from './launch-gates'

describe('Forum launch gates', () => {
  test('reports Forum posting and self-serve tipping as ready', () => {
    const status = forumLaunchGateStatus()

    expect(status.status).toBe('ready')
    expect(status.publicPosting).toStrictEqual({
      listedForums: 'ready',
      voidLane: 'degraded',
    })
    expect(status.publicTipping).toMatchObject({
      onboarding: {
        publicCopyRefs: ['copy.public.forum_tips.self_serve_ready'],
        recipientStateRefs: [
          'state.public.forum_post_tip.recipient_missing',
          'state.public.forum_post_tip.recipient_receive_ready',
        ],
        settlementStateRefs: [
          'state.public.forum_post_tip.paid_pending_settlement',
          'state.public.forum_post_tip.settled',
        ],
      },
      postTips: 'ready',
      remainingBeforeLiveTips: [],
      summary: 'Forum post tips are ready for the public browser action.',
    })
    expect(status.publicTipping.gates).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'tip_payer_wallet_onboarding',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_route_payment_verification',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_private_payment_payload',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_smoke',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_abuse_refund_policy',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_signet_or_live_smoke',
          severity: 'required',
          state: 'ready',
        }),
      ]),
    )
    expect(status.remainingBeforeBroadLaunch).toStrictEqual([])
  })

  test('gates launch when a required guardrail fails', () => {
    const status = forumLaunchGateStatus({
      ...CurrentForumLaunchGateInput,
      paymentRedaction: false,
    })

    expect(status.status).toBe('gated')
    expect(status.gates).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'payment_redaction',
          severity: 'required',
          state: 'gated',
        }),
      ]),
    )
  })

  test('reports ready only when required and recommended gates pass', () => {
    const status = forumLaunchGateStatus({
      ...CurrentForumLaunchGateInput,
      defaultRateLimitPolicy: true,
    })

    expect(status.status).toBe('ready')
    expect(status.remainingBeforeBroadLaunch).toStrictEqual([])
  })

  test('reports live post tipping only when all tipping gates pass', () => {
    const status = forumLaunchGateStatus({
      ...CurrentForumLaunchGateInput,
      tipAbuseRefundPolicy: true,
      tipPayerWalletOnboarding: true,
      tipPrivatePaymentPayload: true,
      tipRoutePaymentVerification: true,
      tipSignetOrLiveSmoke: true,
      tipSmoke: true,
    })

    expect(status.status).toBe('ready')
    expect(status.publicTipping).toMatchObject({
      postTips: 'ready',
      remainingBeforeLiveTips: [],
      summary: 'Forum post tips are ready for the public browser action.',
    })
    expect(status.publicTipping.gates).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'tip_payer_wallet_onboarding',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_route_payment_verification',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_private_payment_payload',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_smoke',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_signet_or_live_smoke',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'tip_abuse_refund_policy',
          severity: 'required',
          state: 'ready',
        }),
      ]),
    )
  })

  test('keeps live post tipping gated when the smoke gate is not passing', () => {
    const status = forumLaunchGateStatus({
      ...CurrentForumLaunchGateInput,
      tipAbuseRefundPolicy: true,
      tipPayerWalletOnboarding: true,
      tipPrivatePaymentPayload: true,
      tipRoutePaymentVerification: true,
      tipSignetOrLiveSmoke: true,
      tipSmoke: false,
    })

    expect(status.publicTipping).toMatchObject({
      postTips: 'gated',
      remainingBeforeLiveTips: ['Tip contract smoke'],
      summary:
        'Forum post tips remain gated until these gates pass: Tip contract smoke.',
    })
  })
})
