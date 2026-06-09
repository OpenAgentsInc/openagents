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

  test('answers spendable creator value only at settled state', () => {
    ForumTipSettlementStates.forEach(state => {
      const projection = forumTipSettlementProjectionForState(state)

      expect(projection.creatorReceivedSpendableValue).toBe(state === 'settled')
      expect(projection.recipientSettlementEvidence).toBe(state === 'settled')
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
          'Payment is confirmed for this reward, but creator spendable settlement is not yet proven.',
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
