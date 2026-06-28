import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from '../public-projection-staleness'
import { cardCreditGrantContextRef } from './card-credit-provenance'
import {
  type CardCreditPurchaseLeg,
  type CardCreditSpendReceipt,
  type CreditToMsatGrantLeg,
  type InferenceSpendLeg,
  cardCreditSpendReceiptRef,
} from './card-credit-spend-receipt'
import {
  type CardCreditSpendReceiptResolution,
  resolveCardCreditSpendReceipt,
} from './card-credit-spend-receipt-resolver'
import { parseInferenceChargeContextRef } from './metering-hook'

export type PublicCardCreditSpendReceiptProjection = Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  receiptRef: string
  resolution:
    | Readonly<{ status: 'ok'; receipt: CardCreditSpendReceipt }>
    | Readonly<{ status: 'pending'; missing: 'purchase' | 'grant' | 'spend' }>
    | Readonly<{ status: 'invalid'; reason: string; message: string }>
  schemaVersion: 'openagents.inference.card_credit_spend_receipt.v1'
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
}>

export type CardCreditSpendReceiptStore = Readonly<{
  readCardCreditSpendReceipt: (
    receiptRef: string,
    generatedAt: string,
  ) => Promise<PublicCardCreditSpendReceiptProjection | null>
}>

export const publicCardCreditSpendReceiptSourceRefs: ReadonlyArray<string> = [
  'route:/api/public/inference/card-credit-spend-receipts/{receiptRef}',
  'ledger.billing_ledger_entries.stripe_checkout',
  'ledger.pay_ins.usd_credit_grant.context_ref',
  'ledger.pay_ins.inference_charge.context_ref',
  'apps/openagents.com/workers/api/src/billing-routes.ts#handleBillingInferenceCreditApi',
  'apps/openagents.com/workers/api/src/inference/usd-credit-bridge.ts',
  'apps/openagents.com/workers/api/src/inference/metering-hook.ts',
]

const prefix = 'receipt.inference.card_credit_spend.'

const sessionIdFromReceiptRef = (receiptRef: string): string | null =>
  receiptRef.startsWith(prefix) && receiptRef.length > prefix.length
    ? receiptRef.slice(prefix.length)
    : null

const publicProjectionFromResolution = (
  receiptRef: string,
  generatedAt: string,
  resolution: CardCreditSpendReceiptResolution,
): PublicCardCreditSpendReceiptProjection | null => {
  if (!resolution.ok && resolution.status === 'blank_session') {
    return null
  }

  const projection: PublicCardCreditSpendReceiptProjection = {
    authorityBoundary:
      'Public proof only. This card-credit-spend receipt read grants no checkout, spend, refund, payout, settlement, provider, public-claim, or registry authority.',
    caveatRefs: [
      'caveat.public.no_private_payment_material',
      'caveat.public.card_credit_chain_requires_all_three_legs',
      'caveat.public.pending_is_not_paid_loop_completion',
    ],
    generatedAt,
    receiptRef,
    resolution: resolution.ok
      ? { receipt: resolution.receipt, status: 'ok' }
      : resolution.status === 'pending'
        ? { missing: resolution.missing, status: 'pending' }
        : {
            message: resolution.message,
            reason: resolution.reason,
            status: 'invalid',
          },
    schemaVersion: 'openagents.inference.card_credit_spend_receipt.v1',
    sourceRefs: publicCardCreditSpendReceiptSourceRefs.map(ref =>
      ref ===
      'route:/api/public/inference/card-credit-spend-receipts/{receiptRef}'
        ? `route:/api/public/inference/card-credit-spend-receipts/${receiptRef}`
        : ref,
    ),
    staleness: liveAtReadStaleness([
      'billing_ledger_entries',
      'pay_ins.public_receipt_ref',
      'pay_ins.context_ref',
    ]),
  }

  return projection
}

type PurchaseRow = Readonly<{
  amount_cents: number
  user_id: string
}>

type GrantRow = Readonly<{
  context_ref: string | null
  cost_msat: number
  created_at: string
  payer_ref: string
  public_receipt_ref: string | null
}>

type DebitRow = Readonly<{ amount_cents: number }>

type SpendRow = Readonly<{
  context_ref: string | null
  cost_msat: number
  public_receipt_ref: string | null
}>

const grantRefFromReceiptRef = (receiptRef: string | null): string | null => {
  const grantPrefix = 'receipt.inference.usd_credit_grant.'
  return receiptRef !== null && receiptRef.startsWith(grantPrefix)
    ? receiptRef.slice(grantPrefix.length)
    : null
}

const requestIdFromChargeReceiptRef = (
  receiptRef: string | null,
): string | null => {
  const chargePrefix = 'receipt.inference.charge.'
  return receiptRef !== null && receiptRef.startsWith(chargePrefix)
    ? receiptRef.slice(chargePrefix.length)
    : null
}

export const makeD1CardCreditSpendReceiptStore = (
  db: D1Database,
): CardCreditSpendReceiptStore => ({
  readCardCreditSpendReceipt: async (receiptRef, generatedAt) => {
    const sessionId = sessionIdFromReceiptRef(receiptRef)
    if (
      sessionId === null ||
      cardCreditSpendReceiptRef(sessionId) !== receiptRef
    ) {
      return null
    }

    let purchaseUserId: string | undefined
    let grantAccountRef: string | undefined
    let grantCreatedAt: string | undefined
    let grantMsat: number | undefined

    const resolution = await resolveCardCreditSpendReceipt(sessionId, {
      readPurchaseLeg: async (): Promise<CardCreditPurchaseLeg | undefined> => {
        const row = await db
          .prepare(
            `SELECT user_id, amount_cents
               FROM billing_ledger_entries
              WHERE source = 'stripe_checkout'
                AND amount_cents > 0
                AND idempotency_key = ?
              LIMIT 1`,
          )
          .bind(`billing:stripe-checkout:${sessionId}`)
          .first<PurchaseRow>()

        if (row === null) {
          return undefined
        }

        purchaseUserId = row.user_id
        return { purchasedCents: row.amount_cents, sessionId }
      },
      readGrantLeg: async (): Promise<CreditToMsatGrantLeg | undefined> => {
        const contextRef = cardCreditGrantContextRef(sessionId)
        if (contextRef === undefined || purchaseUserId === undefined) {
          return undefined
        }

        const row = await db
          .prepare(
            `SELECT payer_ref, cost_msat, context_ref, public_receipt_ref, created_at
               FROM pay_ins
              WHERE pay_in_type = 'usd_credit_grant'
                AND state = 'paid'
                AND context_ref = ?
                AND payer_ref = ?
              ORDER BY created_at ASC
              LIMIT 1`,
          )
          .bind(contextRef, `agent:${purchaseUserId}`)
          .first<GrantRow>()

        const grantRef = grantRefFromReceiptRef(row?.public_receipt_ref ?? null)
        if (row === null || grantRef === null) {
          return undefined
        }

        const debit = await db
          .prepare(
            `SELECT amount_cents
               FROM billing_ledger_entries
              WHERE idempotency_key = ?
                AND amount_cents < 0
              LIMIT 1`,
          )
          .bind(`billing:inference-credit:${grantRef}`)
          .first<DebitRow>()

        if (debit === null) {
          return undefined
        }

        grantAccountRef = row.payer_ref
        grantCreatedAt = row.created_at
        grantMsat = row.cost_msat
        return {
          ...(row.context_ref === null ? {} : { contextRef: row.context_ref }),
          grantRef,
          grantedCents: Math.abs(debit.amount_cents),
          grantedMsat: row.cost_msat,
        }
      },
      readSpendLeg: async (): Promise<InferenceSpendLeg | undefined> => {
        if (
          grantAccountRef === undefined ||
          grantCreatedAt === undefined ||
          grantMsat === undefined
        ) {
          return undefined
        }

        const rows = await db
          .prepare(
            `SELECT public_receipt_ref, cost_msat, context_ref
               FROM pay_ins
              WHERE pay_in_type = 'adjustment'
                AND state = 'paid'
                AND payer_ref = ?
                AND public_receipt_ref LIKE 'receipt.inference.charge.%'
                AND created_at >= ?
                AND cost_msat <= ?
              ORDER BY created_at ASC
              LIMIT 20`,
          )
          .bind(grantAccountRef, grantCreatedAt, grantMsat)
          .all<SpendRow>()

        const spendRow = rows.results.find(row => {
          const requestId = requestIdFromChargeReceiptRef(
            row.public_receipt_ref,
          )
          const context = parseInferenceChargeContextRef(row.context_ref ?? '')
          return requestId !== null && context !== undefined
        })
        if (spendRow === undefined) {
          return undefined
        }

        const requestId = requestIdFromChargeReceiptRef(
          spendRow.public_receipt_ref,
        )
        const context = parseInferenceChargeContextRef(
          spendRow.context_ref ?? '',
        )
        if (requestId === null || context === undefined) {
          return undefined
        }

        return {
          requestId,
          servedModel: context.servedModel,
          spentMsat: spendRow.cost_msat,
          totalTokens: context.totalTokens,
        }
      },
    })

    return publicProjectionFromResolution(receiptRef, generatedAt, resolution)
  },
})
