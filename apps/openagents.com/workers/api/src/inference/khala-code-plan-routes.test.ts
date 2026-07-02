import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  KHALA_CODE_FREE_PLAN_ID,
  KHALA_CODE_PAID_PLAN_ID,
} from './khala-code-plan-catalog'
import {
  handleKhalaCodePlanCatalogApi,
  handleKhalaCodePlanPurchase,
  handleKhalaCodePlanStatus,
} from './khala-code-plan-routes'

type Row = Record<string, string | number | null>

class PlanFakeDb {
  readonly entitlements = new Map<string, Row>()
  readonly entitlementReceipts = new Map<string, Row>()
  failReads = false

  prepare(sql: string) {
    return {
      bind: (...values: ReadonlyArray<string | number | null>) => ({
        first: async <T>(): Promise<T | null> => {
          if (this.failReads) {
            throw new Error('read_failed')
          }
          if (
            sql.includes('FROM inference_privacy_entitlements') &&
            sql.includes('WHERE account_ref')
          ) {
            return (this.entitlements.get(values[0] as string) ??
              null) as T | null
          }
          if (sql.includes('FROM inference_privacy_entitlement_receipts')) {
            return (
              Array.from(this.entitlementReceipts.values()).find(
                row => row.idempotency_key === values[0],
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
              !Array.from(this.entitlementReceipts.values()).some(
                row => row.idempotency_key === values[4],
              )
            ) {
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
            return {}
          }
          if (sql.includes('INSERT INTO inference_privacy_entitlements')) {
            this.entitlements.set(values[0] as string, {
              account_ref: values[0] as string,
              privacy_tier: 'paid_privacy',
              note: values[1] as string,
              created_at: values[2] as string,
              updated_at: values[3] as string,
            })
            return {}
          }
          return {}
        },
      }),
    }
  }
}

const asDb = (db: PlanFakeDb): D1Database => db as unknown as D1Database

const authedDeps = (
  db: PlanFakeDb,
  overrides?: Partial<{
    accountRef: string | undefined
    confidentialComputeEnabled: boolean
    paidPlanPurchaseArmed: boolean
  }>,
) => ({
  authenticate: async () =>
    overrides !== undefined && 'accountRef' in overrides
      ? overrides.accountRef === undefined
        ? undefined
        : { accountRef: overrides.accountRef }
      : { accountRef: 'agent:user-1' },
  confidentialComputeEnabled: overrides?.confidentialComputeEnabled ?? false,
  db: asDb(db),
  nowIso: () => '2026-07-01T00:00:00.000Z',
  paidPlanPurchaseArmed: overrides?.paidPlanPurchaseArmed ?? false,
})

describe('handleKhalaCodePlanCatalogApi', () => {
  it('serves the catalog with the real purchasability state', async () => {
    const response = await Effect.runPromise(
      handleKhalaCodePlanCatalogApi(
        new Request('https://openagents.com/api/public/khala-code/plans'),
        { paidPlanPurchaseArmed: false },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      catalog: {
        plans: ReadonlyArray<{ purchase?: { armed: boolean } }>
        generatedAt: string
        staleness: { composition: string; maxStalenessSeconds: number }
      }
    }
    expect(body.catalog.plans).toHaveLength(2)
    expect(body.catalog.plans[1]?.purchase?.armed).toBe(false)
    expect(body.catalog.generatedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/)
    expect(body.catalog.staleness.composition).toBe('live_at_read')
    expect(body.catalog.staleness.maxStalenessSeconds).toBe(0)
  })

  it('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleKhalaCodePlanCatalogApi(
        new Request('https://openagents.com/api/public/khala-code/plans', {
          method: 'POST',
        }),
        { paidPlanPurchaseArmed: false },
      ),
    )
    expect(response.status).toBe(405)
  })
})

describe('handleKhalaCodePlanStatus', () => {
  const request = () =>
    new Request('https://openagents.com/v1/khala-code/plan')

  it('requires auth', async () => {
    const db = new PlanFakeDb()
    const response = await Effect.runPromise(
      handleKhalaCodePlanStatus(
        request(),
        authedDeps(db, { accountRef: undefined }),
      ),
    )
    expect(response.status).toBe(401)
  })

  it('reports the free default plan when no entitlement row exists', async () => {
    const db = new PlanFakeDb()
    const response = await Effect.runPromise(
      handleKhalaCodePlanStatus(request(), authedDeps(db)),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ok: boolean
      plan: { planId: string; kind: string; captureExcluded: boolean }
    }
    expect(body.ok).toBe(true)
    expect(body.plan.planId).toBe(KHALA_CODE_FREE_PLAN_ID)
    expect(body.plan.kind).toBe('free')
    expect(body.plan.captureExcluded).toBe(false)
  })

  it('reports the paid plan for an entitled account', async () => {
    const db = new PlanFakeDb()
    db.entitlements.set('agent:user-1', { account_ref: 'agent:user-1' })
    const response = await Effect.runPromise(
      handleKhalaCodePlanStatus(request(), authedDeps(db)),
    )
    const body = (await response.json()) as {
      plan: { planId: string; kind: string; captureExcluded: boolean }
    }
    expect(body.plan.planId).toBe(KHALA_CODE_PAID_PLAN_ID)
    expect(body.plan.kind).toBe('paid')
    expect(body.plan.captureExcluded).toBe(true)
  })

  it('keeps confidential-compute mode a capture exclusion, not a purchased plan', async () => {
    const db = new PlanFakeDb()
    const response = await Effect.runPromise(
      handleKhalaCodePlanStatus(
        request(),
        authedDeps(db, { confidentialComputeEnabled: true }),
      ),
    )
    const body = (await response.json()) as {
      plan: { kind: string; captureExcluded: boolean }
    }
    expect(body.plan.kind).toBe('free')
    expect(body.plan.captureExcluded).toBe(true)
  })

  it('fails closed to 503 on an entitlement read error instead of fabricating a plan', async () => {
    const db = new PlanFakeDb()
    db.failReads = true
    const response = await Effect.runPromise(
      handleKhalaCodePlanStatus(request(), authedDeps(db)),
    )
    expect(response.status).toBe(503)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('khala_code_plan_status_unavailable')
  })
})

describe('handleKhalaCodePlanPurchase', () => {
  const request = (body?: unknown) =>
    new Request('https://openagents.com/v1/khala-code/plans/purchases', {
      method: 'POST',
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    })

  it('fails closed with 503 while the seam is unarmed (the default)', async () => {
    const db = new PlanFakeDb()
    const response = await Effect.runPromise(
      handleKhalaCodePlanPurchase(request(), authedDeps(db)),
    )
    expect(response.status).toBe(503)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('khala_code_paid_plans_not_enabled')
    expect(db.entitlements.size).toBe(0)
    expect(db.entitlementReceipts.size).toBe(0)
  })

  it('requires auth when armed', async () => {
    const db = new PlanFakeDb()
    const response = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request(),
        authedDeps(db, {
          accountRef: undefined,
          paidPlanPurchaseArmed: true,
        }),
      ),
    )
    expect(response.status).toBe(401)
  })

  it('grants the paid-privacy entitlement and returns the dereferenceable receipt when armed', async () => {
    const db = new PlanFakeDb()
    const response = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'plan-purchase-1' }),
        authedDeps(db, { paidPlanPurchaseArmed: true }),
      ),
    )
    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      ok: boolean
      planId: string
      captureExcluded: boolean
      entitlementRef: string
      receiptRef: string
      receiptUrl: string
    }
    expect(body.ok).toBe(true)
    expect(body.planId).toBe(KHALA_CODE_PAID_PLAN_ID)
    expect(body.captureExcluded).toBe(true)
    expect(body.receiptRef).toContain('khala_code_paid_plan')
    expect(body.receiptUrl).toBe(
      `/api/public/inference/privacy-receipts/${encodeURIComponent(body.receiptRef)}`,
    )
    expect(db.entitlements.has('agent:user-1')).toBe(true)
    expect(db.entitlementReceipts.size).toBe(1)
  })

  it('is idempotent per idempotency key', async () => {
    const db = new PlanFakeDb()
    const deps = authedDeps(db, { paidPlanPurchaseArmed: true })
    const first = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'plan-purchase-same' }),
        deps,
      ),
    )
    const second = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'plan-purchase-same' }),
        deps,
      ),
    )
    const firstBody = (await first.json()) as { receiptRef: string }
    const secondBody = (await second.json()) as { receiptRef: string }
    expect(secondBody.receiptRef).toBe(firstBody.receiptRef)
    expect(db.entitlementReceipts.size).toBe(1)
  })

  it('rejects malformed bodies', async () => {
    const db = new PlanFakeDb()
    const deps = authedDeps(db, { paidPlanPurchaseArmed: true })
    const invalidJson = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        new Request(
          'https://openagents.com/v1/khala-code/plans/purchases',
          { body: '{nope', method: 'POST' },
        ),
        deps,
      ),
    )
    expect(invalidJson.status).toBe(400)
    const invalidSchema = await Effect.runPromise(
      handleKhalaCodePlanPurchase(request({ idempotencyKey: 5 }), deps),
    )
    expect(invalidSchema.status).toBe(400)
  })
})
