import { Schema as S } from 'effect'

import {
  type ForumPaymentEventProjection,
  type ForumPaymentEventStatus,
  type ForumTipSettlementAuthority,
  type ForumTipSettlementClaimProjection,
  type ForumTipSettlementClaimWording,
  type ForumTipSettlementProjection,
  ForumTipSettlementProjection as ForumTipSettlementProjectionSchema,
  type ForumTipSettlementState,
} from './schemas'

type ForumTipSettlementStatePolicy = Readonly<{
  contentRewardEvidence: boolean
  creatorReceivedSpendableValue: boolean
  recipientSettlementEvidence: boolean
  settlementAuthority: ForumTipSettlementAuthority
  treasuryDispatchAllowed: boolean
  wording: ForumTipSettlementClaimWording
}>

export const ForumTipSettlementStates: ReadonlyArray<ForumTipSettlementState> =
  [
    'previewed',
    'payment_required',
    'evidence_only',
    'paid',
    'recipient_pending',
    'dispatched',
    'settled',
    'failed',
    'refunded',
    'reversed',
  ]

const decodeForumTipSettlementProjection = S.decodeUnknownSync(
  ForumTipSettlementProjectionSchema,
)

const statePolicies: Record<
  ForumTipSettlementState,
  ForumTipSettlementStatePolicy
> = {
  dispatched: {
    contentRewardEvidence: true,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'openagents_treasury_mediated',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'A content-reward payout dispatch is recorded; do not claim final creator settlement.',
      operator:
        'Payout dispatch exists for a Forum content reward. Reconcile settlement before marking the creator paid.',
      publicPage:
        'A creator payout was dispatched for this reward, but final settlement is not yet proven.',
      recipient:
        'A reward payout was dispatched. Spendable receipt is not yet verified.',
    },
  },
  evidence_only: {
    contentRewardEvidence: true,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'content_reward_evidence_only',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'A Forum reward receipt exists without verified payment-event evidence; treat it as content-reward evidence only.',
      operator:
        'This receipt predates or lacks payment-event verification. Do not treat it as wallet settlement or accepted-work payout evidence.',
      publicPage:
        'This post has a Forum reward receipt, but verified payment and creator settlement are not attached.',
      recipient:
        'This reward is recorded, but no verified payment or spendable settlement is attached.',
    },
  },
  failed: {
    contentRewardEvidence: false,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'no_payment_claim',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'The Forum reward payment or payout failed; do not claim content payment or settlement.',
      operator:
        'Failure state requires a new payment attempt or operator resolution before any creator-settlement claim.',
      publicPage: 'This reward failed. It does not prove creator settlement.',
      recipient: 'This reward failed and is not spendable.',
    },
  },
  paid: {
    contentRewardEvidence: true,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'buyer_payment_evidence_only',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'Payment evidence is confirmed for the Forum reward; do not claim accepted work or creator settlement.',
      operator:
        'The payer-side reward payment is confirmed. Keep accepted-work payout and creator settlement separate until recipient settlement evidence exists.',
      publicPage:
        'Payment is confirmed for this reward, but creator spendable settlement is not yet proven.',
      recipient:
        'A payment event is confirmed for this reward. Spendable receipt is not yet verified.',
    },
  },
  payment_required: {
    contentRewardEvidence: false,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'no_payment_claim',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'A Forum reward payment challenge exists; no payment or settlement claim is allowed yet.',
      operator:
        'Payment is required before a content reward, payout attempt, or settlement claim can be recorded.',
      publicPage: 'Payment is required before this reward can be recorded.',
      recipient: 'No reward payment has been verified yet.',
    },
  },
  previewed: {
    contentRewardEvidence: false,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'no_payment_claim',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'A Forum reward quote was previewed; no payment, payout, or settlement claim is allowed.',
      operator:
        'Preview state is quote-only and cannot create Forum earnings, accepted-work refs, payout dispatch, or settlement.',
      publicPage: 'This reward has only been previewed. No payment is claimed.',
      recipient: 'No reward payment has been made.',
    },
  },
  recipient_pending: {
    contentRewardEvidence: true,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'openagents_treasury_mediated',
    treasuryDispatchAllowed: true,
    wording: {
      agent:
        'The Forum reward is pending recipient wallet admission or payout; do not claim final settlement.',
      operator:
        'The reward may enter the narrow Forum content-reward payout path, but it cannot become accepted-work payout evidence.',
      publicPage: 'This reward is pending creator wallet settlement.',
      recipient:
        'This reward is pending wallet admission or payout completion.',
    },
  },
  refunded: {
    contentRewardEvidence: false,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'operator_reversal',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'The Forum reward was refunded; remove it from creator-settlement claims.',
      operator:
        'Refund state cancels creator-settlement claims and must not create accepted-work payout evidence.',
      publicPage:
        'This reward was refunded and does not count as creator settlement.',
      recipient: 'This reward was refunded and is not spendable.',
    },
  },
  reversed: {
    contentRewardEvidence: false,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'operator_reversal',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'The Forum reward was reversed; do not count it as content payment, accepted work, or settlement.',
      operator:
        'Reversal state suppresses payment and settlement claims while preserving an audit trail.',
      publicPage:
        'This reward was reversed and does not count as creator settlement.',
      recipient: 'This reward was reversed and is not spendable.',
    },
  },
  settled: {
    contentRewardEvidence: true,
    creatorReceivedSpendableValue: true,
    recipientSettlementEvidence: true,
    settlementAuthority: 'recipient_wallet_direct',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'Creator spendable settlement is verified for this Forum reward; still do not claim accepted-work payout evidence.',
      operator:
        'Recipient settlement evidence exists for the content reward. It remains separate from accepted-work payout settlement.',
      publicPage: 'Creator spendable settlement is verified for this reward.',
      recipient: 'Spendable settlement is verified for this reward.',
    },
  },
}

const paymentEventStatusToState: Record<
  ForumPaymentEventStatus,
  ForumTipSettlementState
> = {
  confirmed: 'paid',
  failed: 'failed',
  observed: 'payment_required',
  refunded: 'refunded',
  replayed: 'payment_required',
  reversed: 'reversed',
}

export const forumTipSettlementProjectionForState = (
  state: ForumTipSettlementState,
): ForumTipSettlementProjection => {
  const policy = statePolicies[state]

  return decodeForumTipSettlementProjection({
    acceptedWorkPayoutEvidence: false,
    contentRewardEvidence: policy.contentRewardEvidence,
    creatorReceivedSpendableValue: policy.creatorReceivedSpendableValue,
    recipientSettlementEvidence: policy.recipientSettlementEvidence,
    settlementAuthority: policy.settlementAuthority,
    state,
    stateRef: `settlement_state.public.forum_tip.${state}`,
    treasuryAcceptedWorkClaimAllowed: false,
    treasuryDispatchAllowed: policy.treasuryDispatchAllowed,
    wording: policy.wording,
  })
}

export const forumTipSettlementProjectionForReceipt = (
  paymentEvent: ForumPaymentEventProjection | null,
  settlementClaim: ForumTipSettlementClaimProjection | null = null,
): ForumTipSettlementProjection => {
  if (paymentEvent?.status === 'confirmed' && settlementClaim !== null) {
    return forumTipSettlementProjectionForState('settled')
  }

  if (paymentEvent === null) {
    return forumTipSettlementProjectionForState('evidence_only')
  }

  return forumTipSettlementProjectionForState(
    paymentEventStatusToState[paymentEvent.status],
  )
}

export const forumTipSettlementAllowsAcceptedWorkPayoutClaim = (
  projection: ForumTipSettlementProjection,
): boolean =>
  projection.acceptedWorkPayoutEvidence ||
  projection.treasuryAcceptedWorkClaimAllowed
