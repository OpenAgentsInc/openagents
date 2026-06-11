import { describe, expect, test } from 'vitest'

import {
  artanisResponderTipReceiptRef,
  creditedTipStatements,
  isTipLadderReceiptRef,
  tipLadderCanonicalIdempotencyKey,
  tipLadderDecision,
  tipLadderReceiptRefFromIdempotencyKey,
} from './tip-ladder'

const baseDecisionInput = {
  amountSat: 50,
  recipientHasRegisteredOffer: true,
  recipientReceiveCreditsBelowSat: 10,
  recipientRef: 'agent:recipient',
  senderBalanceMsat: 100_000,
  senderRef: 'agent:sender',
  senderSendCreditsBelowSat: 10,
  tipsBufferConfigured: true,
}

describe('tip ladder decision', () => {
  test('refuses invalid amounts, self tips, and insufficient balance', () => {
    expect(tipLadderDecision({ ...baseDecisionInput, amountSat: 0 })).toEqual({
      kind: 'refused',
      reason: 'invalid_amount',
    })
    expect(
      tipLadderDecision({
        ...baseDecisionInput,
        recipientRef: 'agent:sender',
      }),
    ).toEqual({ kind: 'refused', reason: 'self_tip' })
    expect(
      tipLadderDecision({ ...baseDecisionInput, senderBalanceMsat: 49_999 }),
    ).toEqual({ kind: 'refused', reason: 'insufficient_sender_balance' })
  })

  test('micro-tips below thresholds never touch lightning', () => {
    expect(tipLadderDecision({ ...baseDecisionInput, amountSat: 5 })).toEqual({
      kind: 'credited',
      reason: 'below_send_threshold',
    })
    expect(
      tipLadderDecision({
        ...baseDecisionInput,
        amountSat: 15,
        recipientReceiveCreditsBelowSat: 21,
      }),
    ).toEqual({ kind: 'credited', reason: 'below_receive_threshold' })
  })

  test('missing offer or unconfigured buffer land on credited, never fail', () => {
    expect(
      tipLadderDecision({
        ...baseDecisionInput,
        recipientHasRegisteredOffer: false,
      }),
    ).toEqual({ kind: 'credited', reason: 'recipient_offer_missing' })
    expect(
      tipLadderDecision({
        ...baseDecisionInput,
        tipsBufferConfigured: false,
      }),
    ).toEqual({ kind: 'credited', reason: 'tips_buffer_unconfigured' })
  })

  test('direct rung only when offer registered and buffer configured', () => {
    expect(tipLadderDecision(baseDecisionInput)).toEqual({
      kind: 'direct_bolt12',
    })
  })
})

describe('credited tip statements', () => {
  test('one atomic batch: create debits sender, paid credits recipient', () => {
    const publicReceiptRef =
      'receipt.forum.tip_ladder.artanis_responder.topic_abc'
    const statements = creditedTipStatements(
      {
        amountSat: 50,
        fundingLegId: 'leg_in',
        idempotencyKey: 'tip:post:sender:1',
        ladderReason: 'recipient_offer_missing',
        payInId: 'payin_1',
        payoutLegId: 'leg_out',
        postId: 'post_abc',
        publicReceiptRef,
        recipientRef: 'agent:recipient',
        senderRef: 'agent:sender',
      },
      '2026-06-10T20:00:00.000Z',
    )

    const sql = statements.map(statement =>
      statement.sql.replace(/\s+/g, ' ').trim(),
    )

    expect(sql.some(line => line.startsWith('INSERT INTO pay_ins'))).toBe(true)
    expect(
      sql.some(line => line.includes('balance_msat = balance_msat - ?')),
    ).toBe(true)
    expect(
      sql.some(line => line.includes('balance_msat = balance_msat + ?')),
    ).toBe(true)
    expect(sql.some(line => line.includes("SET state = 'paid'"))).toBe(true)

    const insertParams = statements[0]!.params
    expect(insertParams).toContain('forum.post.post_abc')
    expect(insertParams).toContain('credited')
    expect(insertParams).toContain(publicReceiptRef)
    expect(insertParams).toContain(50_000)
  })
})

describe('tip ladder public receipt refs', () => {
  test('normalizes fallback idempotency keys before hashing', async () => {
    await expect(
      tipLadderReceiptRefFromIdempotencyKey('tip:post:sender:1'),
    ).resolves.toBe(
      await tipLadderReceiptRefFromIdempotencyKey(
        'tip:post:sender:1:credited_fallback',
      ),
    )
    expect(
      tipLadderCanonicalIdempotencyKey('tip:post:sender:1:credited_fallback'),
    ).toBe('tip:post:sender:1')
  })

  test('creates public-safe Artanis responder receipt refs', () => {
    const receiptRef = artanisResponderTipReceiptRef(
      'c336dd07-5a66-4786-bb51-116b4bb8121f',
    )

    expect(receiptRef).toBe(
      'receipt.forum.tip_ladder.artanis_responder.c336dd07-5a66-4786-bb51-116b4bb8121f',
    )
    expect(isTipLadderReceiptRef(receiptRef)).toBe(true)
  })
})
