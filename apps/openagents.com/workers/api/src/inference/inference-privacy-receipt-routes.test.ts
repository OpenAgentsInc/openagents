import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  CONFIDENTIAL_COMPUTE_RECEIPT_SCHEMA_VERSION,
  PRIVACY_ENTITLEMENT_RECEIPT_SCHEMA_VERSION,
  handleConfidentialComputeExecutionReceipt,
  handlePaidPrivacyPurchase,
  handlePublicPrivacyReceiptRead,
} from './inference-privacy-receipt-routes'

type Row = Record<string, string | number | null>

class PrivacyReceiptFakeDb {
  readonly entitlements = new Map<string, Row>()
  readonly entitlementReceipts = new Map<string, Row>()
  readonly confidentialReceipts = new Map<string, Row>()

  prepare(sql: string) {
    return {
      bind: (...values: ReadonlyArray<string | number | null>) => ({
        first: async <T>(): Promise<T | null> => {
          if (sql.includes('FROM inference_privacy_entitlement_receipts')) {
            if (sql.includes('WHERE idempotency_key')) {
              return (
                Array.from(this.entitlementReceipts.values()).find(
                  row => row.idempotency_key === values[0],
                ) ?? null
              ) as T | null
            }
            return (
              Array.from(this.entitlementReceipts.values()).find(
                row => row.receipt_ref === values[0],
              ) ?? null
            ) as T | null
          }
          if (
            sql.includes(
              'FROM inference_confidential_compute_execution_receipts',
            )
          ) {
            if (sql.includes('WHERE idempotency_key')) {
              return (
                Array.from(this.confidentialReceipts.values()).find(
                  row => row.idempotency_key === values[0],
                ) ?? null
              ) as T | null
            }
            return (
              Array.from(this.confidentialReceipts.values()).find(
                row => row.receipt_ref === values[0],
              ) ?? null
            ) as T | null
          }
          return null
        },
        run: async () => {
          if (
            sql.includes('INSERT INTO inference_privacy_entitlement_receipts')
          ) {
            if (
              Array.from(this.entitlementReceipts.values()).some(
                row => row.idempotency_key === values[4],
              )
            ) {
              return {}
            }
            this.entitlementReceipts.set(values[0] as string, {
              receipt_ref: values[0] as string,
              entitlement_ref: values[1] as string,
              account_ref: values[2] as string,
              purchase_ref: values[3] as string,
              idempotency_key: values[4] as string,
              privacy_tier: 'paid_privacy',
              capture_excluded: 1,
              reason_ref: values[5] as string,
              created_at: values[6] as string,
              updated_at: values[7] as string,
            })
          }
          if (sql.includes('INSERT INTO inference_privacy_entitlements')) {
            this.entitlements.set(values[0] as string, {
              account_ref: values[0] as string,
              privacy_tier: 'paid_privacy',
              note: values[1] as string,
              created_at: values[2] as string,
              updated_at: values[3] as string,
            })
          }
          if (
            sql.includes(
              'INSERT INTO inference_confidential_compute_execution_receipts',
            )
          ) {
            if (
              Array.from(this.confidentialReceipts.values()).some(
                row => row.idempotency_key === values[4],
              )
            ) {
              return {}
            }
            this.confidentialReceipts.set(values[0] as string, {
              receipt_ref: values[0] as string,
              execution_ref: values[1] as string,
              account_ref: values[2] as string,
              request_ref: values[3] as string,
              idempotency_key: values[4] as string,
              capture_excluded: 1,
              reason_ref: values[5] as string,
              created_at: values[6] as string,
              updated_at: values[7] as string,
            })
          }
          return {}
        },
      }),
    }
  }
}

const run = (effect: Effect.Effect<Response>) => Effect.runPromise(effect)

const deps = (db = new PrivacyReceiptFakeDb(), confidential = true) => ({
  authenticate: async () => ({ accountRef: 'agent:user-1' }),
  confidentialComputeEnabled: confidential,
  db: db as unknown as D1Database,
  nowIso: () => '2026-06-28T00:00:00.000Z',
})

describe('privacy entitlement receipt routes', () => {
  it('purchase grants the paid-privacy entitlement and public receipt', async () => {
    const db = new PrivacyReceiptFakeDb()
    const response = await run(
      handlePaidPrivacyPurchase(
        new Request(
          'https://openagents.com/v1/inference/privacy/paid-privacy/purchases',
          {
            body: JSON.stringify({ idempotencyKey: 'privacy-test-1' }),
            method: 'POST',
          },
        ),
        deps(db),
      ),
    )
    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      entitlementRef: string
      receiptRef: string
    }
    expect(
      body.entitlementRef.startsWith('entitlement.inference.paid_privacy.'),
    ).toBe(true)
    expect(db.entitlements.has('agent:user-1')).toBe(true)

    const read = await run(
      handlePublicPrivacyReceiptRead(
        new Request(
          `https://openagents.com/api/public/inference/privacy-receipts/${body.receiptRef}`,
        ),
        deps(db),
      ),
    )
    expect(read.status).toBe(200)
    const readBody = (await read.json()) as {
      receipt: {
        receipt: { schemaVersion: string; captureExcluded: boolean }
        staleness: { composition: string }
      }
    }
    expect(readBody.receipt.receipt.schemaVersion).toBe(
      PRIVACY_ENTITLEMENT_RECEIPT_SCHEMA_VERSION,
    )
    expect(readBody.receipt.receipt.captureExcluded).toBe(true)
    expect(readBody.receipt.staleness.composition).toBe('live_at_read')
  })

  it('confidential-compute receipt is public-safe and capture-excluded', async () => {
    const db = new PrivacyReceiptFakeDb()
    const response = await run(
      handleConfidentialComputeExecutionReceipt(
        new Request(
          'https://openagents.com/v1/inference/privacy/confidential-compute/executions',
          {
            body: JSON.stringify({ idempotencyKey: 'confidential-test-1' }),
            method: 'POST',
          },
        ),
        deps(db),
      ),
    )
    expect(response.status).toBe(201)
    const body = (await response.json()) as { receiptRef: string }

    const read = await run(
      handlePublicPrivacyReceiptRead(
        new Request(
          `https://openagents.com/api/public/inference/privacy-receipts/${body.receiptRef}`,
        ),
        deps(db),
      ),
    )
    expect(read.status).toBe(200)
    const readBody = (await read.json()) as {
      receipt: { receipt: { schemaVersion: string; captureExcluded: boolean } }
    }
    expect(readBody.receipt.receipt.schemaVersion).toBe(
      CONFIDENTIAL_COMPUTE_RECEIPT_SCHEMA_VERSION,
    )
    expect(readBody.receipt.receipt.captureExcluded).toBe(true)
    expect(JSON.stringify(readBody)).not.toContain('agent:user-1')
  })

  it('confidential-compute receipt endpoint fails closed when disabled', async () => {
    const response = await run(
      handleConfidentialComputeExecutionReceipt(
        new Request(
          'https://openagents.com/v1/inference/privacy/confidential-compute/executions',
          { method: 'POST' },
        ),
        deps(new PrivacyReceiptFakeDb(), false),
      ),
    )
    expect(response.status).toBe(404)
  })
})
