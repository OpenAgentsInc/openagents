import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { lookupForumPaidActionReceipt, readForumCreatorEarnings } from './index'

const receiptRef =
  'receipt.forum.tip_ladder.artanis_responder.c336dd07-5a66-4786-bb51-116b4bb8121f'

const ladderRow = {
  cost_msat: 50_000,
  created_at: '2026-06-11T01:40:39.000Z',
  pay_in_id: 'payin_artanis_tip_1',
  payer_ref: 'agent:artanis',
  payout_external_ref: 'ladder.recipient_destination_missing',
  public_receipt_ref: receiptRef,
  recipient_actor_ref: 'agent:orrery',
  rung: 'credited',
  state: 'paid',
  state_changed_at: '2026-06-11T01:40:39.000Z',
  target_forum_id: 'forum_pylon',
  target_post_id: 'post_orrery_question',
  target_topic_id: 'c336dd07-5a66-4786-bb51-116b4bb8121f',
}

class TipLadderReceiptStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(private readonly query: string) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values
    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM forum_receipts')) {
      return Promise.resolve(null)
    }

    if (
      this.query.includes('FROM pay_ins p') &&
      this.query.includes('p.public_receipt_ref = ?')
    ) {
      return Promise.resolve(
        this.values[0] === receiptRef ? (ladderRow as T) : null,
      )
    }

    if (
      this.query.includes('COUNT(*) AS count') &&
      this.query.includes('FROM forum_money_actions')
    ) {
      return Promise.resolve({ count: 0 } as T)
    }

    if (
      this.query.includes('COUNT(*) AS count') &&
      this.query.includes('FROM pay_ins p')
    ) {
      return Promise.resolve({
        count: this.values[0] === 'agent:orrery' ? 1 : 0,
      } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM forum_money_actions')) {
      return Promise.resolve({ results: [] } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM pay_ins p')) {
      return Promise.resolve({
        results: this.values[0] === 'agent:orrery' ? [ladderRow] : [],
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const db: D1Database = {
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new TipLadderReceiptStatement(query),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
}

describe('tip ladder public receipts', () => {
  test('resolves ladder pay-ins through the public Forum receipt API shape', async () => {
    const receipt = await Effect.runPromise(
      lookupForumPaidActionReceipt(db, receiptRef),
    )

    expect(receipt).toMatchObject({
      actionKind: 'post_reward',
      amount: { amount: 50, asset: 'sats' },
      paymentEvent: {
        paymentEventRef: 'payment_event.forum.tip_ladder.payin_artanis_tip_1',
        settlementAuthority: 'openagents_ledger_credited',
        status: 'confirmed',
      },
      receiptRef,
      recipientActorRef: 'agent:orrery',
      settlementClaim: null,
      target: {
        postId: 'post_orrery_question',
        topicId: 'c336dd07-5a66-4786-bb51-116b4bb8121f',
      },
    })
    // Credited-rung tips read as the explicit credited bucket (#4753),
    // never as payer-side 'paid' and never as 'settled'.
    expect(receipt?.tipSettlement).toMatchObject({
      acceptedWorkPayoutEvidence: false,
      creatorReceivedSpendableValue: false,
      state: 'credited',
    })
  })

  test('includes ladder pay-ins in recipient creator earnings', async () => {
    const earnings = await Effect.runPromise(
      readForumCreatorEarnings(
        db,
        { actorRef: 'agent:orrery', limit: 10 },
        { nowIso: () => '2026-06-11T01:41:00.000Z' },
      ),
    )

    expect(earnings.earnings).toHaveLength(1)
    expect(earnings.earnings[0]).toMatchObject({
      amount: { amount: 50, asset: 'sats' },
      earningActorRef: 'agent:orrery',
      paymentState: 'confirmed',
      receiptRef,
      settlementState: 'credited',
      target: {
        postId: 'post_orrery_question',
        topicId: 'c336dd07-5a66-4786-bb51-116b4bb8121f',
      },
    })
    expect(earnings.summary).toMatchObject({
      creditedCount: 1,
      paidCount: 0,
      sweptCount: 0,
      totalCount: 1,
      totalCreditedSats: 50,
      totalPaidSats: 50,
      totalSettledSats: 0,
      totalSweptSats: 0,
    })
  })
})
