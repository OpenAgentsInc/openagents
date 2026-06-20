import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  InferenceReceiptRecord,
  InferenceReceiptStore,
} from './inference-receipts'
import { makePublicInferenceReceiptRoutes } from './public-inference-receipt-routes'

const receiptRecord = (
  input: Partial<InferenceReceiptRecord> &
    Pick<InferenceReceiptRecord, 'receiptRef'>,
): InferenceReceiptRecord => {
  const { receiptRef, ...overrides } = input

  return {
    createdAt: '2026-06-20T00:00:00.000Z',
    payInType: receiptRef.startsWith('receipt.inference.usd_credit_grant.')
      ? 'usd_credit_grant'
      : 'adjustment',
    receiptRef,
    state: 'paid',
    stateChangedAt: '2026-06-20T00:00:01.000Z',
    ...overrides,
  }
}

const storeFor = (
  records: ReadonlyArray<InferenceReceiptRecord>,
): InferenceReceiptStore => ({
  readInferenceReceiptByRef: receiptRef =>
    Promise.resolve(
      records.find(record => record.receiptRef === receiptRef) ?? null,
    ),
})

const routesFor = (store: InferenceReceiptStore) =>
  makePublicInferenceReceiptRoutes<{ store: InferenceReceiptStore }>({
    makeStore: env => env.store,
    nowIso: () => '2026-06-20T00:01:00.000Z',
  })

const route = async (
  store: InferenceReceiptStore,
  receiptRef: string,
  init?: RequestInit,
) => {
  const response = routesFor(store).routePublicInferenceReceiptRequest(
    new Request(
      `https://openagents.com/api/public/inference/receipts/${encodeURIComponent(
        receiptRef,
      )}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('receipt route did not match')
  }

  return Effect.runPromise(response)
}

describe('public inference receipt routes', () => {
  test('serves paid inference charge receipts without private payment material', async () => {
    const response = await route(
      storeFor([
        receiptRecord({
          receiptRef: 'receipt.inference.charge.chatcmpl_123',
        }),
      ]),
      'receipt.inference.charge.chatcmpl_123',
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.receipt).toMatchObject({
      generatedAt: '2026-06-20T00:01:00.000Z',
      kind: 'charge',
      ledgerState: 'paid',
      receiptRef: 'receipt.inference.charge.chatcmpl_123',
      schemaVersion: 'openagents.inference.receipt.v1',
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
      },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /agent:|amountMsat|amount_msat|billing_ledger|cost_msat|cs_test|idempotency|invoice|lnbc|payment_hash|preimage|stripe|wallet/i,
    )
  })

  test('serves paid inference credit-grant receipts', async () => {
    const response = await route(
      storeFor([
        receiptRecord({
          receiptRef: 'receipt.inference.usd_credit_grant.ep239-stg-1',
        }),
      ]),
      'receipt.inference.usd_credit_grant.ep239-stg-1',
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(body.receipt.kind).toBe('usd_credit_grant')
  })

  test('does not expose pending, mismatched, or unsafe receipt projections', async () => {
    const pending = await route(
      storeFor([
        receiptRecord({
          receiptRef: 'receipt.inference.charge.pending',
          state: 'pending',
        }),
      ]),
      'receipt.inference.charge.pending',
    )
    const mismatched = await route(
      storeFor([
        receiptRecord({
          payInType: 'tip',
          receiptRef: 'receipt.inference.charge.tip',
        }),
      ]),
      'receipt.inference.charge.tip',
    )
    const unsafe = await route(
      storeFor([
        receiptRecord({
          receiptRef: 'receipt.inference.charge.cs_test_123',
        }),
      ]),
      'receipt.inference.charge.cs_test_123',
    )

    expect(pending.status).toBe(404)
    expect(mismatched.status).toBe(404)
    expect(unsafe.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(
      storeFor([]),
      'receipt.inference.charge.chatcmpl_123',
      { method: 'POST' },
    )

    expect(response.status).toBe(405)
  })
})
