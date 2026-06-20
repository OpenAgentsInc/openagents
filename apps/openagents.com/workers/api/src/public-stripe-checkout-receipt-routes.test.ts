import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makePublicStripeCheckoutReceiptRoutes } from './public-stripe-checkout-receipt-routes'
import type {
  PublicStripeCheckoutReceiptProjection,
  StripeCheckoutReceiptStore,
} from './stripe-checkout-receipts'

const projection = (
  receiptRef: string,
  status: 'ok' | 'pending' = 'ok',
): PublicStripeCheckoutReceiptProjection => ({
  authorityBoundary:
    'Public proof only. This Stripe checkout receipt read grants no checkout, spend, refund, payout, settlement, provider, public-claim, or registry authority.',
  caveatRefs: ['caveat.public.no_private_payment_material'],
  generatedAt: '2026-06-20T00:01:00.000Z',
  receiptRef,
  resolution:
    status === 'ok'
      ? {
          creditLedgerState: 'credited',
          fulfillmentState: 'fulfilled',
          paymentState: 'paid',
          sessionMode: 'test',
          status: 'ok',
        }
      : {
          fulfillmentState: 'pending',
          missing: 'webhook_credit',
          paymentState: 'paid',
          sessionMode: 'test',
          status: 'pending',
        },
  schemaVersion: 'openagents.billing.stripe_checkout_receipt.v1',
  sourceRefs: [
    `route:/api/public/billing/stripe-checkout-receipts/${receiptRef}`,
    'ledger.stripe_checkout_sessions.fulfillment_status',
    'ledger.billing_ledger_entries.stripe_checkout',
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['stripe_checkout_sessions', 'billing_ledger_entries'],
  },
})

const storeFor = (
  receipt: PublicStripeCheckoutReceiptProjection | null,
): StripeCheckoutReceiptStore => ({
  readStripeCheckoutReceipt: () => Promise.resolve(receipt),
})

const route = async (
  store: StripeCheckoutReceiptStore,
  receiptRef: string,
  init?: RequestInit,
) => {
  const routes = makePublicStripeCheckoutReceiptRoutes<{
    store: StripeCheckoutReceiptStore
  }>({
    makeStore: env => env.store,
    nowIso: () => '2026-06-20T00:01:00.000Z',
  })
  const response = routes.routePublicStripeCheckoutReceiptRequest(
    new Request(
      `https://openagents.com/api/public/billing/stripe-checkout-receipts/${encodeURIComponent(
        receiptRef,
      )}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('stripe checkout receipt route did not match')
  }

  return Effect.runPromise(response)
}

describe('public Stripe checkout receipt routes', () => {
  test('serves fulfilled checkout credit receipts with staleness metadata', async () => {
    const receiptRef = 'receipt.billing.stripe_checkout.cs_test_123'
    const response = await route(storeFor(projection(receiptRef)), receiptRef)
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.receipt).toMatchObject({
      generatedAt: '2026-06-20T00:01:00.000Z',
      receiptRef,
      resolution: {
        creditLedgerState: 'credited',
        fulfillmentState: 'fulfilled',
        paymentState: 'paid',
        sessionMode: 'test',
        status: 'ok',
      },
      schemaVersion: 'openagents.billing.stripe_checkout_receipt.v1',
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
      },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /checkout_url|customer|email|idempotency|invoice|ledger_entry|payment_intent|preimage|secret|sk-|wallet/i,
    )
  })

  test('serves pending checkout receipts honestly', async () => {
    const receiptRef = 'receipt.billing.stripe_checkout.cs_test_pending'
    const response = await route(
      storeFor(projection(receiptRef, 'pending')),
      receiptRef,
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(body.receipt.resolution).toMatchObject({
      missing: 'webhook_credit',
      status: 'pending',
    })
  })

  test('returns 404 when the receipt ref does not resolve', async () => {
    const response = await route(
      storeFor(null),
      'receipt.billing.stripe_checkout.nope',
    )

    expect(response.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(
      storeFor(null),
      'receipt.billing.stripe_checkout.cs_test_123',
      { method: 'POST' },
    )

    expect(response.status).toBe(405)
  })
})
