import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  CardCreditSpendReceiptStore,
  PublicCardCreditSpendReceiptProjection,
} from './inference/card-credit-spend-receipt-store'
import { makePublicCardCreditSpendReceiptRoutes } from './public-card-credit-spend-receipt-routes'

const projection = (
  receiptRef: string,
): PublicCardCreditSpendReceiptProjection => ({
  authorityBoundary:
    'Public proof only. This card-credit-spend receipt read grants no checkout, spend, refund, payout, settlement, provider, public-claim, or registry authority.',
  caveatRefs: ['caveat.public.pending_is_not_paid_loop_completion'],
  generatedAt: '2026-06-20T00:01:00.000Z',
  receiptRef,
  resolution: {
    missing: 'spend',
    nextEvidenceRef: 'ledger.pay_ins.inference_charge.context_ref',
    status: 'pending',
  },
  schemaVersion: 'openagents.inference.card_credit_spend_receipt.v1',
  sourceRefs: [
    `route:/api/public/inference/card-credit-spend-receipts/${receiptRef}`,
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['billing_ledger_entries', 'pay_ins.public_receipt_ref'],
  },
})

const storeFor = (
  receipt: PublicCardCreditSpendReceiptProjection | null,
): CardCreditSpendReceiptStore => ({
  readCardCreditSpendReceipt: () => Promise.resolve(receipt),
})

const route = async (
  store: CardCreditSpendReceiptStore,
  receiptRef: string,
  init?: RequestInit,
) => {
  const routes = makePublicCardCreditSpendReceiptRoutes<{
    store: CardCreditSpendReceiptStore
  }>({
    makeStore: env => env.store,
    nowIso: () => '2026-06-20T00:01:00.000Z',
  })
  const response = routes.routePublicCardCreditSpendReceiptRequest(
    new Request(
      `https://openagents.com/api/public/inference/card-credit-spend-receipts/${encodeURIComponent(
        receiptRef,
      )}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('card-credit-spend receipt route did not match')
  }

  return Effect.runPromise(response)
}

describe('public card-credit-spend receipt routes', () => {
  test('serves an honest pending projection with staleness metadata', async () => {
    const receiptRef = 'receipt.inference.card_credit_spend.cs_test_123'
    const response = await route(storeFor(projection(receiptRef)), receiptRef)
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.receipt).toMatchObject({
      generatedAt: '2026-06-20T00:01:00.000Z',
      receiptRef,
      resolution: {
        missing: 'spend',
        nextEvidenceRef: 'ledger.pay_ins.inference_charge.context_ref',
        status: 'pending',
      },
      schemaVersion: 'openagents.inference.card_credit_spend_receipt.v1',
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
      },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /agent:|idempotency|invoice|lnbc|payment_hash|preimage|wallet/i,
    )
  })

  test('returns 404 when the receipt ref does not resolve', async () => {
    const response = await route(
      storeFor(null),
      'receipt.inference.card_credit_spend.nope',
    )

    expect(response.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(
      storeFor(null),
      'receipt.inference.card_credit_spend.cs_test_123',
      { method: 'POST' },
    )

    expect(response.status).toBe(405)
  })
})
