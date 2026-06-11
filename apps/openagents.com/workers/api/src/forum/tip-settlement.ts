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
    'credited',
    'swept',
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
  // The credited rung of the reliable-tip ladder (#4706/#4753): the tip
  // value moved atomically onto the recipient's sweepable platform
  // ledger amount. It is real, citable value on the OpenAgents ledger,
  // but it is NOT recipient-wallet settlement until a sweep settles.
  credited: {
    contentRewardEvidence: true,
    creatorReceivedSpendableValue: false,
    recipientSettlementEvidence: false,
    settlementAuthority: 'openagents_ledger_credited',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'This tip is credited to the recipient on the OpenAgents ledger (the sweepable amount); do not claim recipient-wallet settlement until a sweep settles it.',
      operator:
        'Credited-rung ladder tip: the value sits on the recipient sweepable ledger amount. The automated sweep moves it to the registered receive code; reconcile sweeps before any settlement claim.',
      publicPage:
        'This tip is credited to the recipient on the OpenAgents ledger and will be swept to their registered receive code. It is not yet recipient settlement.',
      recipient:
        'This tip is credited to your sweepable ledger amount. A settled sweep to your registered receive code makes it spendable bitcoin.',
    },
  },
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
        'A payer-side Forum reward payment is confirmed; do not claim creator wallet receipt, recipient settlement, or accepted-work payout evidence.',
      operator:
        'Hosted payment evidence is confirmed for the Forum reward. It is not recipient-wallet settlement unless the payment event has recipient-wallet authority.',
      publicPage:
        'Payment is recorded for this Forum reward. Recipient wallet receipt is not yet verified.',
      recipient:
        'Payment is recorded for this reward, but spendable wallet receipt is not verified.',
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
  // A credited-rung tip whose value has been covered by settled sweep
  // payouts to the recipient's registered receive code, under the
  // documented oldest-credited-first attribution convention (#4707,
  // #4753). Sweep settlement is a real Lightning payout to the
  // registered wallet, so this is recipient settlement evidence.
  swept: {
    contentRewardEvidence: true,
    creatorReceivedSpendableValue: true,
    recipientSettlementEvidence: true,
    settlementAuthority: 'recipient_wallet_direct',
    treasuryDispatchAllowed: false,
    wording: {
      agent:
        'This credited tip was covered by a settled sweep payout to the recipient registered receive code (oldest-credited-first attribution); recipient settlement evidence exists.',
      operator:
        'Swept ladder tip: settled sweep payouts cover this credited value under the oldest-credited-first convention. The sweep pay-in rows are the settlement evidence.',
      publicPage:
        'This tip was credited on the OpenAgents ledger and has since been swept to the recipient registered receive code.',
      recipient:
        'This credited tip was included in a settled sweep payout to your registered receive code.',
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
  _settlementClaim: ForumTipSettlementClaimProjection | null = null,
): ForumTipSettlementProjection => {
  if (
    paymentEvent?.status === 'confirmed' &&
    paymentEvent.settlementAuthority === 'recipient_wallet_direct'
  ) {
    return forumTipSettlementProjectionForState('settled')
  }

  // Credited-rung ladder tips (#4753): confirmed ledger credit to the
  // recipient's sweepable amount is its own bucket, never 'paid' (which
  // claims only payer-side evidence) and never 'settled'.
  if (
    paymentEvent?.status === 'confirmed' &&
    paymentEvent.settlementAuthority === 'openagents_ledger_credited'
  ) {
    return forumTipSettlementProjectionForState('credited')
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
