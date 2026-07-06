// IAP credit-pack payment-intent + fulfillment (MM-E2, #8482).
//
// Extends the khala-code-paid-plan-payments.ts payment-intent -> fulfillment
// SHAPE (record intent row -> fulfill -> mark status) for a different
// product: a store-purchased credit pack, fulfilled into Pool B via the
// SAME usd_credit_grant ledger primitive `usd-credit-bridge.ts`'s
// usdCreditGrantStatements uses (RL-3 asset boundary preserved: USD-origin,
// never Bitcoin-withdrawable). Refunds claw back via the existing
// `clawbackInferenceCredits` (inference-abuse-controls.ts) — no new
// money-movement primitive invented here.

import { Effect } from 'effect'

import { runLedgerStatements } from '../payments-ledger'
import { compactRandomId, currentIsoTimestamp } from '../runtime-primitives'
import {
  agentRefForUser,
  usdCreditGrantReceiptRef,
  usdCreditGrantStatements,
} from './usd-credit-bridge'
import { usdCentsToMsatFloor } from './usd-msat-conversion'
import {
  clawbackInferenceCredits,
  type ClawbackOutcome,
} from './inference-abuse-controls'
import type { RevenueCatStore } from './iap-revenuecat-webhook'

/** Typed invariant-violation error (never a generic `throw new Error` — this
 * repo's zero-debt architecture check requires typed errors at the source,
 * matching the sibling `KhalaCodePaidPlanPaymentError` pattern this module
 * extends the shape of). Only ever thrown if the ledger write itself
 * succeeded but the immediate re-read somehow returns nothing — an
 * infrastructure anomaly, not a domain outcome. */
export class IapCreditPackPaymentError extends Error {
  override readonly name = 'IapCreditPackPaymentError'
}

export const IAP_REVENUECAT_RAIL = 'iap_revenuecat' as const

export type IapCreditPackPurchaseStatus = 'fulfilled' | 'refunded'

export type IapCreditPackPurchaseIntent = Readonly<{
  purchaseRef: string
  accountRef: string
  userId: string
  storeTransactionId: string
  sku: string
  store: RevenueCatStore
  amountUsdCents: number
  amountMsat: number
  status: IapCreditPackPurchaseStatus
  creditGrantRef: string | null
  refundReceiptRef: string | null
  createdAt: string
  updatedAt: string
}>

type IapCreditPackPurchaseIntentRow = Readonly<{
  purchase_ref: string
  account_ref: string
  user_id: string
  store_transaction_id: string
  sku: string
  store: string
  amount_usd_cents: number
  amount_msat: number
  status: string
  credit_grant_ref: string | null
  refund_receipt_ref: string | null
  created_at: string
  updated_at: string
}>

const mapRow = (row: IapCreditPackPurchaseIntentRow): IapCreditPackPurchaseIntent => ({
  accountRef: row.account_ref,
  amountMsat: row.amount_msat,
  amountUsdCents: row.amount_usd_cents,
  createdAt: row.created_at,
  creditGrantRef: row.credit_grant_ref,
  purchaseRef: row.purchase_ref,
  refundReceiptRef: row.refund_receipt_ref,
  sku: row.sku,
  status: row.status === 'refunded' ? 'refunded' : 'fulfilled',
  store: row.store === 'play_store' ? 'play_store' : 'app_store',
  storeTransactionId: row.store_transaction_id,
  updatedAt: row.updated_at,
  userId: row.user_id,
})

export const readIapPurchaseByStoreTransactionId = async (
  db: D1Database,
  storeTransactionId: string,
): Promise<IapCreditPackPurchaseIntent | null> => {
  const row = await db
    .prepare(
      `SELECT purchase_ref, account_ref, user_id, store_transaction_id, sku, store,
              amount_usd_cents, amount_msat, status, credit_grant_ref, refund_receipt_ref,
              created_at, updated_at
         FROM iap_credit_pack_purchase_intents
        WHERE store_transaction_id = ?`,
    )
    .bind(storeTransactionId)
    .first<IapCreditPackPurchaseIntentRow>()

  return row === null ? null : mapRow(row)
}

/** Standalone webhook-event replay guard (`iap_webhook_events_processed`).
 * INSERT OR IGNORE + a re-read tells the caller whether THIS call is the
 * one that actually inserted the row (first delivery) or a replay. */
export const claimIapWebhookEvent = async (
  db: D1Database,
  input: Readonly<{ eventId: string; eventType: string; nowIso: string }>,
): Promise<Readonly<{ firstDelivery: boolean }>> => {
  const result = await db
    .prepare(
      `INSERT OR IGNORE INTO iap_webhook_events_processed (event_id, event_type, processed_at)
       VALUES (?, ?, ?)`,
    )
    .bind(input.eventId, input.eventType, input.nowIso)
    .run()

  return { firstDelivery: (result.meta.changes ?? 0) > 0 }
}

export type IapFulfillOutcome =
  | Readonly<{ ok: true; alreadyFulfilled: boolean; purchase: IapCreditPackPurchaseIntent }>
  | Readonly<{ ok: false; reason: 'sku_not_in_catalog' | 'zero_after_conversion' }>

/**
 * Records the intent + grants credit into Pool B, idempotent per
 * `store_transaction_id` (a replayed purchase webhook for the SAME
 * transaction is a no-op — the UNIQUE constraint below makes the INSERT a
 * no-op and the re-read returns the ALREADY-fulfilled row, never a second
 * grant). Caller has already resolved the SKU against the catalog and
 * passes the resulting `amountUsdCents` — never trust the webhook payload's
 * own price field.
 */
export const fulfillIapCreditPackPurchase = (
  db: D1Database,
  input: Readonly<{
    userId: string
    sku: string
    amountUsdCents: number
    store: RevenueCatStore
    storeTransactionId: string
    /** RevenueCat's event id — folded into the ledger idempotency key too,
     * so a replay of the SAME event can never double-grant even if the
     * webhook-event claim above were ever bypassed. */
    eventId: string
  }>,
): Effect.Effect<IapFulfillOutcome> =>
  Effect.gen(function* () {
    const existing = yield* Effect.promise(() =>
      readIapPurchaseByStoreTransactionId(db, input.storeTransactionId),
    )
    if (existing !== null) {
      return { alreadyFulfilled: true, ok: true, purchase: existing } satisfies IapFulfillOutcome
    }

    const amountMsat = usdCentsToMsatFloor(input.amountUsdCents)
    if (amountMsat <= 0) {
      return { ok: false, reason: 'zero_after_conversion' } satisfies IapFulfillOutcome
    }

    const purchaseRef = compactRandomId('iap_credit_pack')
    const accountRef = agentRefForUser(input.userId)
    const nowIso = currentIsoTimestamp()
    const grantRef = `iap:${input.storeTransactionId}`
    const contextRef = `iap:revenuecat:${input.storeTransactionId}`

    yield* Effect.promise(() =>
      runLedgerStatements(db, [
        ...usdCreditGrantStatements({ accountRef, contextRef, grantMsat: amountMsat, grantRef }, nowIso),
      ]),
    )

    yield* Effect.promise(() =>
      db
        .prepare(
          `INSERT OR IGNORE INTO iap_credit_pack_purchase_intents
            (purchase_ref, account_ref, user_id, idempotency_key, rail, store, sku,
             store_transaction_id, amount_usd_cents, amount_msat, status, credit_grant_ref,
             refund_receipt_ref, created_at, updated_at, fulfilled_at, refunded_at)
           VALUES (?, ?, ?, ?, 'iap_revenuecat', ?, ?, ?, ?, ?, 'fulfilled', ?, NULL, ?, ?, ?, NULL)`,
        )
        .bind(
          purchaseRef,
          accountRef,
          input.userId,
          `iap:event:${input.eventId}`,
          input.store === 'play_store' ? 'play_store' : 'app_store',
          input.sku,
          input.storeTransactionId,
          input.amountUsdCents,
          amountMsat,
          usdCreditGrantReceiptRef(grantRef),
          nowIso,
          nowIso,
          nowIso,
        )
        .run(),
    )

    const row = yield* Effect.promise(() =>
      readIapPurchaseByStoreTransactionId(db, input.storeTransactionId),
    )
    if (row === null) {
      throw new IapCreditPackPaymentError('iap credit pack purchase intent not recorded after fulfillment')
    }

    return { alreadyFulfilled: false, ok: true, purchase: row } satisfies IapFulfillOutcome
  })

export type IapRefundOutcome =
  | Readonly<{ ok: true; alreadyRefunded: boolean; clawback: ClawbackOutcome }>
  | Readonly<{ ok: false; reason: 'purchase_not_found' }>

/** Claws back a previously-fulfilled purchase by store_transaction_id.
 * Idempotent: refunding an already-refunded purchase is a no-op (reports
 * `alreadyRefunded: true` without re-clawing). */
export const refundIapCreditPackPurchase = (
  db: D1Database,
  storeTransactionId: string,
): Effect.Effect<IapRefundOutcome> =>
  Effect.gen(function* () {
    const purchase = yield* Effect.promise(() =>
      readIapPurchaseByStoreTransactionId(db, storeTransactionId),
    )
    if (purchase === null) {
      return { ok: false, reason: 'purchase_not_found' } satisfies IapRefundOutcome
    }

    if (purchase.status === 'refunded') {
      return {
        alreadyRefunded: true,
        clawback: {
          clawedBack: false,
          insufficientBalance: false,
          receiptRef: purchase.refundReceiptRef ?? '',
        },
        ok: true,
      } satisfies IapRefundOutcome
    }

    const clawback = yield* clawbackInferenceCredits(
      {
        accountRef: purchase.accountRef,
        clawbackMsat: purchase.amountMsat,
        contextRef: `iap:refund:${storeTransactionId}`,
        sourceRef: purchase.purchaseRef,
      },
      { db },
    )

    const nowIso = currentIsoTimestamp()
    yield* Effect.promise(() =>
      db
        .prepare(
          `UPDATE iap_credit_pack_purchase_intents
           SET status = 'refunded', refund_receipt_ref = ?, updated_at = ?, refunded_at = COALESCE(refunded_at, ?)
           WHERE store_transaction_id = ?`,
        )
        .bind(clawback.receiptRef, nowIso, nowIso, storeTransactionId)
        .run(),
    )

    return { alreadyRefunded: false, clawback, ok: true } satisfies IapRefundOutcome
  })
