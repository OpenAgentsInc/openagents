// Khala Code paid-plan payment intent ledger (RL-4, #8248).
//
// This module records payment-intent state for the Stripe Checkout and
// Spark/MPP Lightning rails, then fulfills into the EXISTING paid-privacy
// entitlement receipt. It deliberately does NOT create a second plan entitlement
// truth: the public receipt remains
// /api/public/inference/privacy-receipts/{receiptRef}.

import { compactRandomId } from '../runtime-primitives'
import {
  firstDollarEvidenceBundleRef,
  recordRevenueEventProvenance,
} from '../revenue-event-provenance'
import { KHALA_CODE_PAID_PLAN_ID } from './khala-code-plan-catalog'
import { grantPaidPrivacyEntitlement } from './inference-privacy-receipt-routes'
import type { LightningInvoice } from './mpp/mpp-lightning-invoice'

export const KHALA_CODE_PAID_PLAN_STRIPE_PRODUCT =
  'khala_code_paid_plan' as const

export type KhalaCodePaidPlanRail = 'stripe_checkout' | 'lightning_mpp'
export type KhalaCodePaidPlanPaymentStatus =
  | 'requires_payment'
  | 'fulfilled'
  | 'failed'
  | 'expired'

export type KhalaCodePaidPlanPaymentIntent = Readonly<{
  accountRef: string
  amountCents: number | null
  amountSats: number | null
  createdAt: string
  entitlementReceiptRef: string | null
  failureReason: string | null
  fulfilledAt: string | null
  idempotencyKey: string
  lightningInvoice: string | null
  lightningInvoiceExpiresAt: string | null
  lightningNetwork: LightningInvoice['network'] | null
  lightningPaymentHash: string | null
  planId: typeof KHALA_CODE_PAID_PLAN_ID
  purchaseRef: string
  rail: KhalaCodePaidPlanRail
  status: KhalaCodePaidPlanPaymentStatus
  stripeCheckoutSessionId: string | null
  stripeCheckoutUrl: string | null
  updatedAt: string
}>

type KhalaCodePaidPlanPaymentIntentRow = Readonly<{
  account_ref: string
  amount_cents: number | null
  amount_sats: number | null
  created_at: string
  entitlement_receipt_ref: string | null
  failure_reason: string | null
  fulfilled_at: string | null
  idempotency_key: string
  lightning_invoice: string | null
  lightning_invoice_expires_at: string | null
  lightning_network: string | null
  lightning_payment_hash: string | null
  plan_id: string
  purchase_ref: string
  rail: string
  status: string
  stripe_checkout_session_id: string | null
  stripe_checkout_url: string | null
  updated_at: string
}>

export type KhalaCodePaidPlanFulfillment = Readonly<{
  ok: true
  captureExcluded: true
  entitlementRef: string
  planId: typeof KHALA_CODE_PAID_PLAN_ID
  purchaseRef: string
  rail: KhalaCodePaidPlanRail
  receiptRef: string
  receiptUrl: string
  status: 'fulfilled'
}>

export type KhalaCodePaidPlanStripeCheckout = Readonly<{
  ok: true
  checkoutUrl: string
  planId: typeof KHALA_CODE_PAID_PLAN_ID
  purchaseRef: string
  rail: 'stripe_checkout'
  status: 'payment_required'
  stripeCheckoutSessionId: string
}>

export type KhalaCodePaidPlanLightningPaymentRequest = Readonly<{
  ok: true
  bolt11: string
  invoiceExpiresAt?: string | undefined
  network: LightningInvoice['network']
  paymentHash: string
  planId: typeof KHALA_CODE_PAID_PLAN_ID
  purchaseRef: string
  rail: 'lightning_mpp'
  status: 'payment_required'
}>

export class KhalaCodePaidPlanPaymentError extends Error {
  override readonly name = 'KhalaCodePaidPlanPaymentError'
  readonly reason:
    | 'intent_not_recorded'
    | 'intent_not_found'
    | 'intent_rail_mismatch'
    | 'intent_account_mismatch'
    | 'intent_not_payable'
    | 'receipt_not_recorded'

  constructor(reason: KhalaCodePaidPlanPaymentError['reason']) {
    super(`khala code paid-plan payment failed: ${reason}`)
    this.reason = reason
  }
}

export const makeKhalaCodePaidPlanPurchaseRef = (): string =>
  compactRandomId('khala_code_paid_plan')

const mapIntentRow = (
  row: KhalaCodePaidPlanPaymentIntentRow,
): KhalaCodePaidPlanPaymentIntent => ({
  accountRef: row.account_ref,
  amountCents: row.amount_cents,
  amountSats: row.amount_sats,
  createdAt: row.created_at,
  entitlementReceiptRef: row.entitlement_receipt_ref,
  failureReason: row.failure_reason,
  fulfilledAt: row.fulfilled_at,
  idempotencyKey: row.idempotency_key,
  lightningInvoice: row.lightning_invoice,
  lightningInvoiceExpiresAt: row.lightning_invoice_expires_at,
  lightningNetwork:
    row.lightning_network === 'mainnet' ||
    row.lightning_network === 'regtest' ||
    row.lightning_network === 'signet'
      ? row.lightning_network
      : null,
  lightningPaymentHash: row.lightning_payment_hash,
  planId: KHALA_CODE_PAID_PLAN_ID,
  purchaseRef: row.purchase_ref,
  rail:
    row.rail === 'lightning_mpp' ? 'lightning_mpp' : 'stripe_checkout',
  status:
    row.status === 'fulfilled' ||
    row.status === 'failed' ||
    row.status === 'expired'
      ? row.status
      : 'requires_payment',
  stripeCheckoutSessionId: row.stripe_checkout_session_id,
  stripeCheckoutUrl: row.stripe_checkout_url,
  updatedAt: row.updated_at,
})

const readIntent = async (
  db: D1Database,
  sql: string,
  values: ReadonlyArray<string>,
): Promise<KhalaCodePaidPlanPaymentIntent | null> => {
  const row = await db
    .prepare(sql)
    .bind(...values)
    .first<KhalaCodePaidPlanPaymentIntentRow>()

  return row === null ? null : mapIntentRow(row)
}

export const readKhalaCodePaidPlanIntentByIdempotencyKey = (
  db: D1Database,
  input: Readonly<{ accountRef: string; idempotencyKey: string }>,
) =>
  readIntent(
    db,
    `SELECT purchase_ref, account_ref, idempotency_key, rail, status, plan_id,
            amount_cents, amount_sats, stripe_checkout_session_id,
            stripe_checkout_url, lightning_payment_hash, lightning_invoice,
            lightning_network, lightning_invoice_expires_at,
            entitlement_receipt_ref, failure_reason, created_at, updated_at,
            fulfilled_at
     FROM khala_code_paid_plan_payment_intents
     WHERE idempotency_key = ? AND account_ref = ?`,
    [input.idempotencyKey, input.accountRef],
  )

export const readKhalaCodePaidPlanIntentByStripeSession = (
  db: D1Database,
  sessionId: string,
) =>
  readIntent(
    db,
    `SELECT purchase_ref, account_ref, idempotency_key, rail, status, plan_id,
            amount_cents, amount_sats, stripe_checkout_session_id,
            stripe_checkout_url, lightning_payment_hash, lightning_invoice,
            lightning_network, lightning_invoice_expires_at,
            entitlement_receipt_ref, failure_reason, created_at, updated_at,
            fulfilled_at
     FROM khala_code_paid_plan_payment_intents
     WHERE stripe_checkout_session_id = ?`,
    [sessionId],
  )

export const readKhalaCodePaidPlanIntentByLightningPaymentHash = (
  db: D1Database,
  paymentHash: string,
) =>
  readIntent(
    db,
    `SELECT purchase_ref, account_ref, idempotency_key, rail, status, plan_id,
            amount_cents, amount_sats, stripe_checkout_session_id,
            stripe_checkout_url, lightning_payment_hash, lightning_invoice,
            lightning_network, lightning_invoice_expires_at,
            entitlement_receipt_ref, failure_reason, created_at, updated_at,
            fulfilled_at
     FROM khala_code_paid_plan_payment_intents
     WHERE lightning_payment_hash = ?`,
    [paymentHash],
  )

export const recordKhalaCodePaidPlanStripeIntent = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    amountCents: number | null
    checkoutUrl: string
    idempotencyKey: string
    nowIso: string
    purchaseRef: string
    stripeCheckoutSessionId: string
  }>,
): Promise<KhalaCodePaidPlanPaymentIntent> => {
  await db
    .prepare(
      `INSERT OR IGNORE INTO khala_code_paid_plan_payment_intents
        (purchase_ref, account_ref, idempotency_key, rail, status, plan_id,
         amount_cents, amount_sats, stripe_checkout_session_id,
         stripe_checkout_url, lightning_payment_hash, lightning_invoice,
         lightning_network, lightning_invoice_expires_at,
         entitlement_receipt_ref, failure_reason, created_at, updated_at,
         fulfilled_at)
       VALUES (?, ?, ?, 'stripe_checkout', 'requires_payment', ?, ?, NULL, ?,
               ?, NULL, NULL, NULL, NULL, NULL, NULL, ?, ?, NULL)`,
    )
    .bind(
      input.purchaseRef,
      input.accountRef,
      input.idempotencyKey,
      KHALA_CODE_PAID_PLAN_ID,
      input.amountCents,
      input.stripeCheckoutSessionId,
      input.checkoutUrl,
      input.nowIso,
      input.nowIso,
    )
    .run()

  const row = await readKhalaCodePaidPlanIntentByIdempotencyKey(db, input)

  if (row === null) {
    throw new KhalaCodePaidPlanPaymentError('intent_not_recorded')
  }
  return row
}

export const recordKhalaCodePaidPlanLightningIntent = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    amountSats: number
    idempotencyKey: string
    invoice: LightningInvoice
    nowIso: string
    purchaseRef: string
  }>,
): Promise<KhalaCodePaidPlanPaymentIntent> => {
  await db
    .prepare(
      `INSERT OR IGNORE INTO khala_code_paid_plan_payment_intents
        (purchase_ref, account_ref, idempotency_key, rail, status, plan_id,
         amount_cents, amount_sats, stripe_checkout_session_id,
         stripe_checkout_url, lightning_payment_hash, lightning_invoice,
         lightning_network, lightning_invoice_expires_at,
         entitlement_receipt_ref, failure_reason, created_at, updated_at,
         fulfilled_at)
       VALUES (?, ?, ?, 'lightning_mpp', 'requires_payment', ?, NULL, ?, NULL,
               NULL, ?, ?, ?, ?, NULL, NULL, ?, ?, NULL)`,
    )
    .bind(
      input.purchaseRef,
      input.accountRef,
      input.idempotencyKey,
      KHALA_CODE_PAID_PLAN_ID,
      input.amountSats,
      input.invoice.paymentHash,
      input.invoice.bolt11,
      input.invoice.network,
      input.invoice.invoiceExpiresAt ?? null,
      input.nowIso,
      input.nowIso,
    )
    .run()

  const row = await readKhalaCodePaidPlanIntentByIdempotencyKey(db, input)

  if (row === null) {
    throw new KhalaCodePaidPlanPaymentError('intent_not_recorded')
  }
  return row
}

export const stripeCheckoutResponseFromIntent = (
  intent: KhalaCodePaidPlanPaymentIntent,
): KhalaCodePaidPlanStripeCheckout => {
  if (
    intent.rail !== 'stripe_checkout' ||
    intent.stripeCheckoutSessionId === null ||
    intent.stripeCheckoutUrl === null
  ) {
    throw new KhalaCodePaidPlanPaymentError('intent_rail_mismatch')
  }
  return {
    ok: true,
    checkoutUrl: intent.stripeCheckoutUrl,
    planId: KHALA_CODE_PAID_PLAN_ID,
    purchaseRef: intent.purchaseRef,
    rail: 'stripe_checkout',
    status: 'payment_required',
    stripeCheckoutSessionId: intent.stripeCheckoutSessionId,
  }
}

export const lightningPaymentRequestFromIntent = (
  intent: KhalaCodePaidPlanPaymentIntent,
): KhalaCodePaidPlanLightningPaymentRequest => {
  if (
    intent.rail !== 'lightning_mpp' ||
    intent.lightningInvoice === null ||
    intent.lightningPaymentHash === null ||
    intent.lightningNetwork === null
  ) {
    throw new KhalaCodePaidPlanPaymentError('intent_rail_mismatch')
  }
  return {
    ok: true,
    bolt11: intent.lightningInvoice,
    ...(intent.lightningInvoiceExpiresAt === null
      ? {}
      : { invoiceExpiresAt: intent.lightningInvoiceExpiresAt }),
    network: intent.lightningNetwork,
    paymentHash: intent.lightningPaymentHash,
    planId: KHALA_CODE_PAID_PLAN_ID,
    purchaseRef: intent.purchaseRef,
    rail: 'lightning_mpp',
    status: 'payment_required',
  }
}

export const fulfillKhalaCodePaidPlanPaymentIntent = async (
  db: D1Database,
  input: Readonly<{
    intent: KhalaCodePaidPlanPaymentIntent
    nowIso: string
  }>,
): Promise<KhalaCodePaidPlanFulfillment> => {
  if (input.intent.status === 'failed' || input.intent.status === 'expired') {
    throw new KhalaCodePaidPlanPaymentError('intent_not_payable')
  }

  const row = await grantPaidPrivacyEntitlement(db, {
    accountRef: input.intent.accountRef,
    idempotencyKey: input.intent.idempotencyKey,
    nowIso: input.nowIso,
    purchaseRef: input.intent.purchaseRef,
  })

  if (row === null) {
    throw new KhalaCodePaidPlanPaymentError('receipt_not_recorded')
  }

  await db
    .prepare(
      `UPDATE khala_code_paid_plan_payment_intents
       SET status = 'fulfilled', entitlement_receipt_ref = ?,
           failure_reason = NULL, updated_at = ?, fulfilled_at = COALESCE(fulfilled_at, ?)
       WHERE purchase_ref = ?`,
    )
    .bind(
      row.receipt_ref,
      input.nowIso,
      input.nowIso,
      input.intent.purchaseRef,
    )
    .run()

  const eventRef = `revenue_event.khala_code.paid_plan.${input.intent.purchaseRef}`
  await recordRevenueEventProvenance(db, {
    amountCents: input.intent.amountCents,
    amountSats: input.intent.amountSats,
    caveatRefs: [
      'caveat.revenue.first_dollar.owner_signoff_required_for_public_claim',
      'caveat.revenue.khala_code_paid_plan_no_green_claim_from_receipt_alone',
    ],
    demandProvenance: 'external',
    eventRef,
    evidenceBundleRef: firstDollarEvidenceBundleRef('khala_code', eventRef),
    idempotencyKey: `revenue-event:khala-code-paid-plan:${input.intent.purchaseRef}`,
    ledgerRowRef: input.intent.purchaseRef,
    ledgerTable: 'khala_code_paid_plan_payment_intents',
    paymentState: 'fulfilled',
    productRef: 'khala_code',
    publicEvidenceRefs: [
      row.receipt_ref,
      `route:/api/public/inference/privacy-receipts/${row.receipt_ref}`,
      'promise:khala_code.free_paid_plans.v1',
    ],
    receiptRef: row.receipt_ref,
    recordedAt: input.nowIso,
    revenueSurfaceRef: 'khala_code.paid_plan',
    sourceRefs: [
      'route:/v1/khala-code/plans/purchases',
      'table:khala_code_paid_plan_payment_intents',
      'table:inference_privacy_entitlement_receipts',
    ],
  })

  return {
    ok: true,
    captureExcluded: true,
    entitlementRef: row.entitlement_ref,
    planId: KHALA_CODE_PAID_PLAN_ID,
    purchaseRef: input.intent.purchaseRef,
    rail: input.intent.rail,
    receiptRef: row.receipt_ref,
    receiptUrl: `/api/public/inference/privacy-receipts/${encodeURIComponent(row.receipt_ref)}`,
    status: 'fulfilled',
  }
}

export const markKhalaCodePaidPlanStripeIntentUnpaid = async (
  db: D1Database,
  input: Readonly<{
    nowIso: string
    sessionId: string
    status: 'expired' | 'failed' | 'requires_payment'
  }>,
): Promise<void> => {
  await db
    .prepare(
      `UPDATE khala_code_paid_plan_payment_intents
       SET status = ?, failure_reason = ?, updated_at = ?
       WHERE stripe_checkout_session_id = ? AND status != 'fulfilled'`,
    )
    .bind(input.status, input.status, input.nowIso, input.sessionId)
    .run()
}
