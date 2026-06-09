export type ForumTipUiAuthState = 'LoggedIn' | 'LoggedOut'

export type ForumTipUiRecipientReadiness = Readonly<{
  blockerRef?: string | null
  state?: string | null
  tippingAvailable?: boolean | null
}>

export type ForumTipUiPost = Readonly<{
  author?: Readonly<{
    displayName?: string | null
  }> | null
  postId?: string | null
  tipRecipientReadiness?: ForumTipUiRecipientReadiness | null
}>

export type ForumTipUiLaunchStatus = Readonly<{
  publicTipping?: Readonly<{
    postTips?: string | null
    remainingBeforeLiveTips?: ReadonlyArray<string> | null
    summary?: string | null
  }> | null
}>

export type ForumTipUiProjection = Readonly<{
  authRequired: boolean
  buttonLabel: string | null
  buttonVisible: boolean
  caveat: string
  detail: string
  reason: 'launch_gated' | 'login_required' | 'recipient_not_ready' | 'ready'
  recipientLabel: string
  statusLabel: string
}>

export type ForumTipReceiptState =
  | 'dispatched'
  | 'evidence_only'
  | 'failed'
  | 'paid'
  | 'payment_required'
  | 'previewed'
  | 'recipient_pending'
  | 'refunded'
  | 'reversed'
  | 'settled'

export const forumTipAmountLabel = 'Custom sats'

export const forumTipCaveat =
  'Content reward; receipt separates payment from settlement.'

const recipientLabelForPost = (post: ForumTipUiPost): string =>
  post.author?.displayName?.trim() || 'creator'

const firstLaunchBlocker = (
  launchStatus: ForumTipUiLaunchStatus | null,
): string =>
  launchStatus?.publicTipping?.remainingBeforeLiveTips?.[0] ??
  'payment verification'

const launchBlockerStatusLabel = (
  launchStatus: ForumTipUiLaunchStatus | null,
): string => {
  const blocker = firstLaunchBlocker(launchStatus).toLowerCase()

  if (blocker.includes('payer wallet')) {
    return 'Tip setup pending'
  }

  if (blocker.includes('smoke')) {
    return 'Live smoke pending'
  }

  return 'Self-serve tips pending'
}

export const forumTipUiProjectionForPost = (input: {
  authState: ForumTipUiAuthState
  launchStatus: ForumTipUiLaunchStatus | null
  post: ForumTipUiPost
}): ForumTipUiProjection => {
  const recipientLabel = recipientLabelForPost(input.post)

  if (input.launchStatus?.publicTipping?.postTips !== 'ready') {
    return {
      authRequired: false,
      buttonLabel: null,
      buttonVisible: false,
      caveat: forumTipCaveat,
      detail: firstLaunchBlocker(input.launchStatus),
      reason: 'launch_gated',
      recipientLabel,
      statusLabel: launchBlockerStatusLabel(input.launchStatus),
    }
  }

  if (input.post.tipRecipientReadiness?.tippingAvailable !== true) {
    return {
      authRequired: false,
      buttonLabel: null,
      buttonVisible: false,
      caveat: forumTipCaveat,
      detail: input.post.tipRecipientReadiness?.blockerRef ?? 'wallet pending',
      reason: 'recipient_not_ready',
      recipientLabel,
      statusLabel: 'Wallet pending',
    }
  }

  return {
    authRequired: input.authState === 'LoggedOut',
    buttonLabel: 'Tip',
    buttonVisible: true,
    caveat: forumTipCaveat,
    detail: `${forumTipAmountLabel} to ${recipientLabel}`,
    reason: input.authState === 'LoggedOut' ? 'login_required' : 'ready',
    recipientLabel,
    statusLabel: input.authState === 'LoggedOut' ? 'Log in required' : 'Ready',
  }
}

export const forumTipReceiptStateLabel = (
  state: ForumTipReceiptState,
): string =>
  ({
    dispatched: 'Payout dispatched',
    evidence_only: 'Receipt evidence only',
    failed: 'Payment failed',
    paid: 'Payment recorded',
    payment_required: 'Payment required',
    previewed: 'Previewed',
    recipient_pending: 'Creator settlement pending',
    refunded: 'Refunded',
    reversed: 'Reversed',
    settled: 'Recipient wallet paid',
  })[state]
