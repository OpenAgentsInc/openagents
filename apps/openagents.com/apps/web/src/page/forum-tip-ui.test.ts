import { describe, expect, test } from 'vitest'

import {
  forumTipReceiptStateLabel,
  forumTipUiProjectionForPost,
} from './forum-tip-ui'

const readyLaunch = {
  publicTipping: {
    postTips: 'ready',
    remainingBeforeLiveTips: [],
    summary: 'Forum post tips are ready for the public browser action.',
  },
}

const gatedLaunch = {
  publicTipping: {
    postTips: 'gated',
    remainingBeforeLiveTips: ['Tip payer wallet onboarding'],
    summary: 'Forum post tips remain gated.',
  },
}

const readyPost = {
  author: { displayName: 'Comunero' },
  postId: '66666666-6666-4666-8666-666666666666',
  tipRecipientReadiness: {
    state: 'ready',
    tippingAvailable: true,
  },
}

describe('Forum tip UI projection', () => {
  test('hides the button when backend tipping gates are not ready', () => {
    expect(
      forumTipUiProjectionForPost({
        authState: 'LoggedIn',
        launchStatus: gatedLaunch,
        post: readyPost,
      }),
    ).toMatchObject({
      buttonVisible: false,
      detail: 'Tip payer wallet onboarding',
      reason: 'launch_gated',
      statusLabel: 'Tip setup pending',
    })
  })

  test('labels launch gates with actionable blocker copy', () => {
    expect(
      forumTipUiProjectionForPost({
        authState: 'LoggedIn',
        launchStatus: {
          publicTipping: {
            postTips: 'gated',
            remainingBeforeLiveTips: ['Tip signet/live smoke'],
            summary: 'Forum post tips remain gated.',
          },
        },
        post: readyPost,
      }),
    ).toMatchObject({
      detail: 'Tip signet/live smoke',
      reason: 'launch_gated',
      statusLabel: 'Live smoke pending',
    })

    expect(
      forumTipUiProjectionForPost({
        authState: 'LoggedIn',
        launchStatus: {
          publicTipping: {
            postTips: 'gated',
            remainingBeforeLiveTips: ['Tip route payment verification'],
            summary: 'Forum post tips remain gated.',
          },
        },
        post: readyPost,
      }),
    ).toMatchObject({
      detail: 'Tip route payment verification',
      reason: 'launch_gated',
      statusLabel: 'Self-serve tips pending',
    })
  })

  test('hides the button when recipient readiness is missing', () => {
    expect(
      forumTipUiProjectionForPost({
        authState: 'LoggedIn',
        launchStatus: readyLaunch,
        post: {
          ...readyPost,
          tipRecipientReadiness: {
            blockerRef: 'blocker.public.forum_tip_recipient.wallet_missing',
            state: 'missing',
            tippingAvailable: false,
          },
        },
      }),
    ).toMatchObject({
      buttonVisible: false,
      detail: 'blocker.public.forum_tip_recipient.wallet_missing',
      reason: 'recipient_not_ready',
      statusLabel: 'Wallet pending',
    })
  })

  test('shows a compact tip action when launch, auth, and recipient gates pass', () => {
    expect(
      forumTipUiProjectionForPost({
        authState: 'LoggedIn',
        launchStatus: readyLaunch,
        post: readyPost,
      }),
    ).toMatchObject({
      authRequired: false,
      buttonLabel: 'Tip',
      buttonVisible: true,
      detail: 'Custom sats to Comunero',
      reason: 'ready',
      statusLabel: 'Ready',
    })
  })

  test('keeps the visible action auth-blocked for logged-out visitors', () => {
    expect(
      forumTipUiProjectionForPost({
        authState: 'LoggedOut',
        launchStatus: readyLaunch,
        post: readyPost,
      }),
    ).toMatchObject({
      authRequired: true,
      buttonLabel: 'Tip',
      buttonVisible: true,
      reason: 'login_required',
      statusLabel: 'Log in required',
    })
  })

  test('labels receipt states honestly', () => {
    expect(forumTipReceiptStateLabel('paid')).toBe('Payment recorded')
    expect(forumTipReceiptStateLabel('recipient_pending')).toBe(
      'Creator settlement pending',
    )
    expect(forumTipReceiptStateLabel('dispatched')).toBe('Payout dispatched')
    expect(forumTipReceiptStateLabel('settled')).toBe('Recipient wallet paid')
    expect(forumTipReceiptStateLabel('failed')).toBe('Payment failed')
    expect(forumTipReceiptStateLabel('refunded')).toBe('Refunded')
    expect(forumTipReceiptStateLabel('reversed')).toBe('Reversed')
  })
})
