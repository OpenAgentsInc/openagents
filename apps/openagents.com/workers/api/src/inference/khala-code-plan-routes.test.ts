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
  type KhalaCodePlanRoutesDeps,
} from './khala-code-plan-routes'
import { readPublicPrivacyReceipt } from './inference-privacy-receipt-routes'
import { type LightningInvoice } from './mpp/mpp-lightning-invoice'
import { sha256Hex } from './mpp/mpp-lightning-verify'
import { readFirstDollarEvidenceBundle } from '../revenue-event-provenance'

type Row = Record<string, string | number | null>

class PlanFakeDb {
  readonly entitlements = new Map<string, Row>()
  readonly entitlementReceipts = new Map<string, Row>()
  readonly paidPlanPaymentIntents = new Map<string, Row>()
  readonly revenueEvents = new Map<string, Row>()
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
            if (sql.includes('WHERE receipt_ref = ?')) {
              return (this.entitlementReceipts.get(values[0] as string) ??
                null) as T | null
            }
            return (
              Array.from(this.entitlementReceipts.values()).find(
                row =>
                  row.idempotency_key === values[0] &&
                  (!sql.includes('account_ref = ?') ||
                    row.account_ref === values[1]),
              ) ?? null
            ) as T | null
          }
          if (sql.includes('FROM khala_code_paid_plan_payment_intents')) {
            const rows = Array.from(this.paidPlanPaymentIntents.values())
            if (sql.includes('WHERE idempotency_key = ? AND account_ref = ?')) {
              return (
                rows.find(
                  row =>
                    row.idempotency_key === values[0] &&
                    row.account_ref === values[1],
                ) ?? null
              ) as T | null
            }
            if (sql.includes('WHERE lightning_payment_hash = ?')) {
              return (
                rows.find(row => row.lightning_payment_hash === values[0]) ??
                null
              ) as T | null
            }
            if (sql.includes('WHERE stripe_checkout_session_id = ?')) {
              return (
                rows.find(
                  row => row.stripe_checkout_session_id === values[0],
                ) ?? null
              ) as T | null
            }
          }
          if (sql.includes('FROM revenue_event_provenance')) {
            if (sql.includes('WHERE idempotency_key = ?')) {
              return (
                Array.from(this.revenueEvents.values()).find(
                  row => row.idempotency_key === values[0],
                ) ?? null
              ) as T | null
            }
            if (sql.includes('WHERE evidence_bundle_ref = ?')) {
              return (
                this.revenueEvents.get(values[0] as string) ?? null
              ) as T | null
            }
          }
          return null
        },
        run: async () => {
          if (
            sql.includes('INSERT OR IGNORE INTO khala_code_paid_plan_payment_intents')
          ) {
            if (
              !Array.from(this.paidPlanPaymentIntents.values()).some(
                row => row.idempotency_key === values[2],
              )
            ) {
              const isLightning = sql.includes("'lightning_mpp'")
              this.paidPlanPaymentIntents.set(values[0] as string, {
                purchase_ref: values[0] as string,
                account_ref: values[1] as string,
                idempotency_key: values[2] as string,
                rail: isLightning ? 'lightning_mpp' : 'stripe_checkout',
                status: 'requires_payment',
                plan_id: KHALA_CODE_PAID_PLAN_ID,
                amount_cents: isLightning ? null : (values[4] ?? null),
                amount_sats: isLightning ? (values[4] ?? null) : null,
                stripe_checkout_session_id: isLightning
                  ? null
                  : (values[5] ?? null),
                stripe_checkout_url: isLightning ? null : (values[6] ?? null),
                lightning_payment_hash: isLightning
                  ? (values[5] ?? null)
                  : null,
                lightning_invoice: isLightning ? (values[6] ?? null) : null,
                lightning_network: isLightning ? (values[7] ?? null) : null,
                lightning_invoice_expires_at: isLightning
                  ? (values[8] ?? null)
                  : null,
                entitlement_receipt_ref: null,
                failure_reason: null,
                created_at: isLightning
                  ? (values[9] ?? null)
                  : (values[7] ?? null),
                updated_at: isLightning
                  ? (values[10] ?? null)
                  : (values[8] ?? null),
                fulfilled_at: null,
              })
            }
            return {}
          }
          if (
            sql.includes('UPDATE khala_code_paid_plan_payment_intents') &&
            sql.includes("status = 'fulfilled'")
          ) {
            const row = this.paidPlanPaymentIntents.get(values[3] as string)
            if (row !== undefined) {
              row.status = 'fulfilled'
              row.entitlement_receipt_ref = values[0] as string
              row.updated_at = values[1] as string
              row.fulfilled_at = row.fulfilled_at ?? (values[2] as string)
            }
            return {}
          }
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
          if (
            sql.includes('INSERT OR IGNORE INTO revenue_event_provenance')
          ) {
            if (!this.revenueEvents.has(values[1] as string)) {
              this.revenueEvents.set(values[1] as string, {
                event_ref: values[0] as string,
                evidence_bundle_ref: values[1] as string,
                idempotency_key: values[2] as string,
                product_ref: values[3] as string,
                revenue_surface_ref: values[4] as string,
                receipt_ref: values[5] as string,
                ledger_table: values[6] as string,
                ledger_row_ref: values[7] as string,
                demand_provenance: values[8] as string,
                payment_state: values[9] as string,
                amount_cents: values[10] ?? null,
                amount_sats: values[11] ?? null,
                public_evidence_refs_json: values[12] as string,
                caveat_refs_json: values[13] as string,
                source_refs_json: values[14] as string,
                recorded_at: values[15] as string,
                created_at: values[16] as string,
                updated_at: values[17] as string,
              })
            }
            return {}
          }
          return {}
        },
      }),
    }
  }
}

const asDb = (db: PlanFakeDb): D1Database => db as unknown as D1Database

type AuthedDepsOverrides = Partial<
  Omit<
    KhalaCodePlanRoutesDeps,
    'authenticate' | 'confidentialComputeEnabled' | 'db' | 'nowIso'
  >
> &
  Partial<{
    accountRef: string | undefined
    confidentialComputeEnabled: boolean
  }>

const authedDeps = (
  db: PlanFakeDb,
  overrides?: AuthedDepsOverrides,
) => ({
  authenticate: async () =>
    overrides !== undefined && 'accountRef' in overrides
      ? overrides.accountRef === undefined
        ? undefined
        : { accountRef: overrides.accountRef }
      : { accountRef: 'agent:user-1' },
  confidentialComputeEnabled: overrides?.confidentialComputeEnabled ?? false,
  ...(overrides?.createStripePaidPlanCheckout === undefined
    ? {}
    : { createStripePaidPlanCheckout: overrides.createStripePaidPlanCheckout }),
  db: asDb(db),
  ...(overrides?.mintLightningInvoice === undefined
    ? {}
    : { mintLightningInvoice: overrides.mintLightningInvoice }),
  nowIso: () => '2026-07-01T00:00:00.000Z',
  ...(overrides?.paidPlanPriceSats === undefined
    ? {}
    : { paidPlanPriceSats: overrides.paidPlanPriceSats }),
  paidPlanPurchaseArmed: overrides?.paidPlanPurchaseArmed ?? false,
})

const lightningInvoice = (
  paymentHash: string,
): LightningInvoice => ({
  bolt11: 'lnbcrt1khala20260704paidplan',
  network: 'regtest',
  paymentHash,
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

  it('still reports a purchased paid plan under confidential-compute mode', async () => {
    const db = new PlanFakeDb()
    db.entitlements.set('agent:user-1', { account_ref: 'agent:user-1' })
    const response = await Effect.runPromise(
      handleKhalaCodePlanStatus(
        request(),
        authedDeps(db, { confidentialComputeEnabled: true }),
      ),
    )
    const body = (await response.json()) as {
      plan: { kind: string; captureExcluded: boolean }
    }
    expect(body.plan.kind).toBe('paid')
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

  it('creates a Stripe Checkout payment requirement without granting entitlement', async () => {
    const db = new PlanFakeDb()
    const stripeCalls: Array<
      Parameters<
        NonNullable<KhalaCodePlanRoutesDeps['createStripePaidPlanCheckout']>
      >[0]
    > = []
    const response = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'plan-purchase-1' }),
        authedDeps(db, {
          createStripePaidPlanCheckout: async input => {
            stripeCalls.push(input)
            return {
              ok: true,
              checkoutUrl: 'https://checkout.stripe.test/cs_test_1',
              planId: KHALA_CODE_PAID_PLAN_ID,
              purchaseRef: input.purchaseRef,
              rail: 'stripe_checkout',
              status: 'payment_required',
              stripeCheckoutSessionId: 'cs_test_1',
            }
          },
          paidPlanPurchaseArmed: true,
        }),
      ),
    )
    expect(response.status).toBe(202)
    const body = (await response.json()) as {
      ok: boolean
      checkoutUrl: string
      planId: string
      rail: string
      status: string
      stripeCheckoutSessionId: string
    }
    expect(body.ok).toBe(true)
    expect(body.status).toBe('payment_required')
    expect(body.rail).toBe('stripe_checkout')
    expect(body.planId).toBe(KHALA_CODE_PAID_PLAN_ID)
    expect(body.checkoutUrl).toBe('https://checkout.stripe.test/cs_test_1')
    expect(body.stripeCheckoutSessionId).toBe('cs_test_1')
    expect(stripeCalls[0]?.idempotencyKey).toContain(
      'khala-code-plan-purchase:agent:user-1:stripe_checkout:plan-purchase-1',
    )
    expect(db.entitlements.size).toBe(0)
    expect(db.entitlementReceipts.size).toBe(0)
  })

  it('settles the Lightning rail into an idempotent receipt projection', async () => {
    const db = new PlanFakeDb()
    const preimage = '00'.repeat(32)
    const paymentHash = await sha256Hex(new Uint8Array(32))
    const deps = authedDeps(db, {
      mintLightningInvoice: () => Effect.succeed(lightningInvoice(paymentHash)),
      paidPlanPriceSats: 1999,
      paidPlanPurchaseArmed: true,
    })
    const payment = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'plan-lightning-1', rail: 'lightning_mpp' }),
        deps,
      ),
    )
    expect(payment.status).toBe(202)
    const paymentBody = (await payment.json()) as {
      bolt11: string
      paymentHash: string
      rail: string
      status: string
    }
    expect(paymentBody.status).toBe('payment_required')
    expect(paymentBody.rail).toBe('lightning_mpp')
    expect(paymentBody.paymentHash).toBe(paymentHash)
    expect(paymentBody.bolt11).toBe('lnbcrt1khala20260704paidplan')
    expect(db.entitlementReceipts.size).toBe(0)

    const firstSettlement = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({
          lightningPaymentHash: paymentHash,
          preimage,
          rail: 'lightning_mpp',
        }),
        deps,
      ),
    )
    expect(firstSettlement.status).toBe(201)
    const firstBody = (await firstSettlement.json()) as {
      captureExcluded: boolean
      receiptRef: string
      receiptUrl: string
      status: string
    }
    expect(firstBody.status).toBe('fulfilled')
    expect(firstBody.captureExcluded).toBe(true)
    expect(firstBody.receiptUrl).toBe(
      `/api/public/inference/privacy-receipts/${encodeURIComponent(firstBody.receiptRef)}`,
    )

    const projection = await readPublicPrivacyReceipt(
      asDb(db),
      firstBody.receiptRef,
      '2026-07-01T00:00:00.000Z',
    )
    expect(projection?.receipt.receiptRef).toBe(firstBody.receiptRef)
    expect(projection?.receipt.captureExcluded).toBe(true)

    const secondSettlement = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({
          lightningPaymentHash: paymentHash,
          preimage,
          rail: 'lightning_mpp',
        }),
        deps,
      ),
    )
    const secondBody = (await secondSettlement.json()) as { receiptRef: string }
    expect(secondBody.receiptRef).toBe(firstBody.receiptRef)
    expect(db.entitlementReceipts.size).toBe(1)
    expect(db.entitlements.has('agent:user-1')).toBe(true)
    expect(db.revenueEvents.size).toBe(1)
    const revenueEvent = Array.from(db.revenueEvents.values())[0]
    expect(revenueEvent).toMatchObject({
      product_ref: 'khala_code',
      demand_provenance: 'external',
      payment_state: 'fulfilled',
      amount_sats: 1999,
      receipt_ref: firstBody.receiptRef,
      ledger_table: 'khala_code_paid_plan_payment_intents',
    })

    const evidenceBundle = await readFirstDollarEvidenceBundle(
      asDb(db),
      revenueEvent?.evidence_bundle_ref as string,
      '2026-07-01T00:00:00.000Z',
    )
    expect(evidenceBundle?.registryEvidenceRefs).toEqual(
      expect.arrayContaining([
        `receipt:${firstBody.receiptRef}`,
        `ledger:revenue_event_provenance:${revenueEvent?.event_ref}`,
      ]),
    )
  })

  it('rejects a supplied-but-invalid idempotency key instead of silently replacing it', async () => {
    const db = new PlanFakeDb()
    const deps = authedDeps(db, { paidPlanPurchaseArmed: true })
    const response = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'x'.repeat(200) }),
        deps,
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('invalid_idempotency_key')
    expect(db.entitlementReceipts.size).toBe(0)
  })

  it('confines a reused client idempotency key to its own account', async () => {
    const db = new PlanFakeDb()
    const idempotencyKeys: Array<string> = []
    const createStripePaidPlanCheckout: NonNullable<
      KhalaCodePlanRoutesDeps['createStripePaidPlanCheckout']
    > = async input => {
      idempotencyKeys.push(input.idempotencyKey)
      return {
        ok: true,
        checkoutUrl: `https://checkout.stripe.test/${input.accountRef}`,
        planId: KHALA_CODE_PAID_PLAN_ID,
        purchaseRef: input.purchaseRef,
        rail: 'stripe_checkout',
        status: 'payment_required',
        stripeCheckoutSessionId: `cs_${input.accountRef.replaceAll(':', '_')}`,
      }
    }
    const first = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'shared-key' }),
        authedDeps(db, {
          accountRef: 'agent:user-1',
          createStripePaidPlanCheckout,
          paidPlanPurchaseArmed: true,
        }),
      ),
    )
    const second = await Effect.runPromise(
      handleKhalaCodePlanPurchase(
        request({ idempotencyKey: 'shared-key' }),
        authedDeps(db, {
          accountRef: 'agent:user-2',
          createStripePaidPlanCheckout,
          paidPlanPurchaseArmed: true,
        }),
      ),
    )
    expect(first.status).toBe(202)
    expect(second.status).toBe(202)
    // Each account gets ITS OWN Stripe idempotency namespace — a key collision
    // must never return (or publicly attribute) another account's purchase.
    expect(idempotencyKeys[0]).toContain('agent:user-1:stripe_checkout:shared-key')
    expect(idempotencyKeys[1]).toContain('agent:user-2:stripe_checkout:shared-key')
    expect(idempotencyKeys[0]).not.toBe(idempotencyKeys[1])
    expect(db.entitlementReceipts.size).toBe(0)
    expect(db.entitlements.size).toBe(0)
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
