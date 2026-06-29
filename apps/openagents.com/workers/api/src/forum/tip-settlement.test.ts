import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ForumPaymentEventProjection,
  type ForumPaymentEventProjection as ForumPaymentEventProjectionType,
} from './schemas'
import {
  ForumTipSettlementStates,
  forumTipSettlementAllowsAcceptedWorkPayoutClaim,
  forumTipSettlementProjectionForReceipt,
  forumTipSettlementProjectionForState,
} from './tip-settlement'

const confirmedPaymentEvent: ForumPaymentEventProjectionType =
  S.decodeUnknownSync(ForumPaymentEventProjection)({
    actionKind: 'post_reward',
    amount: { amount: 100, asset: 'sats' },
    challengeId: '77777777-7777-4777-8777-777777777777',
    createdAt: '2026-06-05T18:12:00.000Z',
    externalRef: 'external.payment.redacted_1',
    payerActorRef: 'actor.alice',
    paymentEventRef: 'payment_event.forum.reward_1',
    paymentMode: 'signet',
    providerRef: 'provider.mdk_l402.redacted',
    receiptRef: 'receipt.forum.reward.1',
    recipientActorRef: 'actor.comunero',
    redactedEvidenceRef: 'evidence.payment.redacted_1',
    settlementAuthority: 'buyer_payment_evidence_only',
    status: 'confirmed',
  })

describe('Forum tip settlement model', () => {
  test('defines public, agent, operator, and recipient claim wording for every state', () => {
    ForumTipSettlementStates.forEach(state => {
      const projection = forumTipSettlementProjectionForState(state)

      expect(projection.state).toBe(state)
      expect(projection.stateRef).toBe(
        `settlement_state.public.forum_tip.${state}`,
      )
      expect(projection.wording.agent.length).toBeGreaterThan(0)
      expect(projection.wording.operator.length).toBeGreaterThan(0)
      expect(projection.wording.publicPage.length).toBeGreaterThan(0)
      expect(projection.wording.recipient.length).toBeGreaterThan(0)
    })
  })

  test('never turns an ordinary Forum tip into accepted-work payout evidence', () => {
    ForumTipSettlementStates.forEach(state => {
      const projection = forumTipSettlementProjectionForState(state)

      expect(projection.acceptedWorkPayoutEvidence).toBe(false)
      expect(projection.treasuryAcceptedWorkClaimAllowed).toBe(false)
      expect(forumTipSettlementAllowsAcceptedWorkPayoutClaim(projection)).toBe(
        false,
      )
      expect(JSON.stringify(projection).toLowerCase()).not.toContain(
        'accepted work is proven',
      )
    })
  })

  test('answers spendable creator value only for recipient-wallet settlement', () => {
    ForumTipSettlementStates.forEach(state => {
      const projection = forumTipSettlementProjectionForState(state)
      // 'settled' (direct payment) and 'swept' (settled sweep payout to
      // the registered receive code) are the only recipient-wallet
      // settlement states; spendable-value and settlement-evidence
      // claims stay tied to that authority.
      const recipientWalletSettled = state === 'settled' || state === 'swept'

      expect(projection.settlementAuthority === 'recipient_wallet_direct').toBe(
        recipientWalletSettled,
      )
      expect(projection.creatorReceivedSpendableValue).toBe(
        recipientWalletSettled,
      )
      expect(projection.recipientSettlementEvidence).toBe(
        recipientWalletSettled,
      )
    })
  })

  test('labels paid, pending, settled, refunded, and reversed claim boundaries explicitly', () => {
    expect(forumTipSettlementProjectionForState('paid')).toMatchObject({
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'buyer_payment_evidence_only',
      state: 'paid',
      wording: {
        publicPage:
          'Payment is recorded for this Forum reward. Recipient wallet receipt is not yet verified.',
      },
    })
    expect(
      forumTipSettlementProjectionForState('recipient_pending'),
    ).toMatchObject({
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'openagents_treasury_mediated',
      state: 'recipient_pending',
      treasuryDispatchAllowed: true,
    })
    expect(forumTipSettlementProjectionForState('settled')).toMatchObject({
      creatorReceivedSpendableValue: true,
      recipientSettlementEvidence: true,
      settlementAuthority: 'recipient_wallet_direct',
      state: 'settled',
    })
    expect(forumTipSettlementProjectionForState('refunded')).toMatchObject({
      contentRewardEvidence: false,
      creatorReceivedSpendableValue: false,
      settlementAuthority: 'operator_reversal',
      state: 'refunded',
    })
    expect(forumTipSettlementProjectionForState('reversed')).toMatchObject({
      contentRewardEvidence: false,
      creatorReceivedSpendableValue: false,
      settlementAuthority: 'operator_reversal',
      state: 'reversed',
    })
  })

  test('projects current receipt shapes as evidence-only or payer-side paid', () => {
    expect(forumTipSettlementProjectionForReceipt(null)).toMatchObject({
      acceptedWorkPayoutEvidence: false,
      contentRewardEvidence: true,
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'content_reward_evidence_only',
      state: 'evidence_only',
    })

    expect(
      forumTipSettlementProjectionForReceipt(confirmedPaymentEvent),
    ).toMatchObject({
      acceptedWorkPayoutEvidence: false,
      contentRewardEvidence: true,
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'buyer_payment_evidence_only',
      state: 'paid',
    })
  })

  test('labels the credited and swept ladder buckets without settled/paid conflation (#4753)', () => {
    expect(forumTipSettlementProjectionForState('credited')).toMatchObject({
      contentRewardEvidence: true,
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'openagents_ledger_credited',
      state: 'credited',
      treasuryDispatchAllowed: false,
    })
    expect(forumTipSettlementProjectionForState('swept')).toMatchObject({
      contentRewardEvidence: true,
      creatorReceivedSpendableValue: true,
      recipientSettlementEvidence: true,
      settlementAuthority: 'recipient_wallet_direct',
      state: 'swept',
      treasuryDispatchAllowed: false,
    })
    // A confirmed ledger-credited payment event projects 'credited',
    // never 'paid' or 'settled'.
    expect(
      forumTipSettlementProjectionForReceipt({
        ...confirmedPaymentEvent,
        settlementAuthority: 'openagents_ledger_credited',
      }),
    ).toMatchObject({
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      state: 'credited',
    })
  })

  test('settles only recipient-wallet-direct payment events', () => {
    expect(
      forumTipSettlementProjectionForReceipt({
        ...confirmedPaymentEvent,
        settlementAuthority: 'recipient_wallet_direct',
      }),
    ).toMatchObject({
      creatorReceivedSpendableValue: true,
      recipientSettlementEvidence: true,
      settlementAuthority: 'recipient_wallet_direct',
      state: 'settled',
    })
  })

  test('projects refund and reversal payment events without settlement claims', () => {
    expect(
      forumTipSettlementProjectionForReceipt({
        ...confirmedPaymentEvent,
        status: 'refunded',
      }),
    ).toMatchObject({
      contentRewardEvidence: false,
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'operator_reversal',
      state: 'refunded',
      treasuryAcceptedWorkClaimAllowed: false,
    })

    expect(
      forumTipSettlementProjectionForReceipt({
        ...confirmedPaymentEvent,
        status: 'reversed',
      }),
    ).toMatchObject({
      contentRewardEvidence: false,
      creatorReceivedSpendableValue: false,
      recipientSettlementEvidence: false,
      settlementAuthority: 'operator_reversal',
      state: 'reversed',
      treasuryAcceptedWorkClaimAllowed: false,
    })
  })
})
