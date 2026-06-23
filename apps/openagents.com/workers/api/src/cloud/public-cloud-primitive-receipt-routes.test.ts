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
})
