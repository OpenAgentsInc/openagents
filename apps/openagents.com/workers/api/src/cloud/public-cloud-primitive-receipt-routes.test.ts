import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  CloudPrimitiveReceiptReadStore,
  CloudPrimitiveReceiptRecord,
} from './cloud-primitive-receipts'
import { makePublicCloudPrimitiveReceiptRoutes } from './public-cloud-primitive-receipt-routes'

const receiptRecord = (
  input: Partial<CloudPrimitiveReceiptRecord> &
    Pick<CloudPrimitiveReceiptRecord, 'receiptRef'>,
): CloudPrimitiveReceiptRecord => {
  const { receiptRef, ...overrides } = input

  return {
    contextRef: null,
    createdAt: '2026-06-23T00:00:00.000Z',
    payInType: 'adjustment',
    receiptRef,
    state: 'paid',
    stateChangedAt: '2026-06-23T00:00:01.000Z',
    ...overrides,
  }
}

const storeFor = (
  records: ReadonlyArray<CloudPrimitiveReceiptRecord>,
): CloudPrimitiveReceiptReadStore => ({
  readCloudPrimitiveReceiptByRef: receiptRef =>
    Promise.resolve(
      records.find(record => record.receiptRef === receiptRef) ?? null,
    ),
})

const routesFor = () =>
  makePublicCloudPrimitiveReceiptRoutes<{
    store: CloudPrimitiveReceiptReadStore
  }>({
    makeStore: env => env.store,
    nowIso: () => '2026-06-23T00:01:00.000Z',
  })

const route = async (
  store: CloudPrimitiveReceiptReadStore,
  receiptRef: string,
  init?: RequestInit,
) => {
  const response = routesFor().routePublicCloudPrimitiveReceiptRequest(
    new Request(
      `https://openagents.com/api/public/cloud/receipts/${encodeURIComponent(
        receiptRef,
      )}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('cloud primitive receipt route did not match')
  }

  return Effect.runPromise(response)
}

/**
 * Route a RAW path tail, without `encodeURIComponent`, so a malformed escape
 * survives into the matcher (PRO-101).
 */
const routeRawPathTail = async (
  store: CloudPrimitiveReceiptReadStore,
  rawTail: string,
  init?: RequestInit,
) => {
  const response = routesFor().routePublicCloudPrimitiveReceiptRequest(
    new Request(
      `https://openagents.com/api/public/cloud/receipts/${rawTail}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('cloud primitive receipt route did not match')
  }

  return Effect.runPromise(response)
}

describe('public cloud primitive receipt routes', () => {
  test('serves a paid sandbox rental charge receipt without payment material', async () => {
    const ref = 'receipt.cloud.sandbox_compute.rental.charge.sbx_1'
    const response = await route(storeFor([receiptRecord({ receiptRef: ref })]), ref)
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.receipt).toMatchObject({
      generatedAt: '2026-06-23T00:01:00.000Z',
      kind: 'sandbox_compute_rental',
      ledgerState: 'paid',
      receiptRef: ref,
      schemaVersion: 'openagents.cloud.primitive.receipt.v1',
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
      },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /agent:|amountMsat|amount_msat|cost_msat|idempotency|invoice|lnbc|payment_hash|preimage|stripe|wallet/i,
    )
  })

  test('serves a paid fine-tuning job charge receipt', async () => {
    const ref = 'receipt.cloud.fine_tuning.job.charge.ftjob_1'
    const response = await route(storeFor([receiptRecord({ receiptRef: ref })]), ref)
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(body.receipt.kind).toBe('fine_tuning_job')
  })

  test('does not expose pending, mismatched, or non-cloud receipts', async () => {
    const pending = await route(
      storeFor([
        receiptRecord({
          receiptRef: 'receipt.cloud.sandbox_compute.rental.charge.pending',
          state: 'pending',
        }),
      ]),
      'receipt.cloud.sandbox_compute.rental.charge.pending',
    )
    const mismatched = await route(
      storeFor([
        receiptRecord({
          payInType: 'tip',
          receiptRef: 'receipt.cloud.sandbox_compute.rental.charge.tip',
        }),
      ]),
      'receipt.cloud.sandbox_compute.rental.charge.tip',
    )
    const nonCloud = await route(
      storeFor([
        receiptRecord({ receiptRef: 'receipt.inference.charge.req1' }),
      ]),
      'receipt.inference.charge.req1',
    )

    expect(pending.status).toBe(404)
    expect(mismatched.status).toBe(404)
    expect(nonCloud.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(
      storeFor([]),
      'receipt.cloud.sandbox_compute.rental.charge.sbx_1',
      { method: 'POST' },
    )

    expect(response.status).toBe(405)
  })

  // PRO-101. This is the reader the production repro used:
  //   GET /api/public/cloud/receipts/receipt.test.abc -> 404 (correct)
  //   GET /api/public/cloud/receipts/%               -> 500 (the defect)
  // The unguarded `decodeURIComponent` threw `URIError` as an Effect DEFECT,
  // so under `Effect.runPromise` these assertions also trip on the defect.
  describe('a malformed percent-escape reads like an unknown ref, not a fault', () => {
    test('a bare `%` is a 404, not a 500', async () => {
      const response = await routeRawPathTail(storeFor([]), '%')

      expect(response.status).toBe(404)
    })

    test('other malformed escapes are 404 too', async () => {
      const responses = await Promise.all(
        [
          '%zz',
          'receipt.cloud.sandbox_compute.rental.charge.%',
          '%E0%A4%A',
        ].map(rawTail => routeRawPathTail(storeFor([]), rawTail)),
      )

      expect(responses.map(response => response.status)).toEqual([404, 404, 404])
    })

    test('a well-formed unknown ref is still a 404', async () => {
      const response = await route(storeFor([]), 'receipt.test.abc')

      expect(response.status).toBe(404)
    })

    test('a malformed escape is still refused by method before ref', async () => {
      const response = await routeRawPathTail(storeFor([]), '%', {
        method: 'POST',
      })

      expect(response.status).toBe(405)
    })
  })
})
