import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export type StripeCheckoutReceiptSessionMode = 'test' | 'live' | 'unknown'

export type PublicStripeCheckoutReceiptProjection = Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  receiptRef: string
  resolution:
    | Readonly<{
        creditLedgerState: 'credited'
        fulfillmentState: 'fulfilled'
        paymentState: 'paid'
        sessionMode: StripeCheckoutReceiptSessionMode
        status: 'ok'
      }>
    | Readonly<{
        fulfillmentState: 'pending' | 'unpaid' | 'expired' | 'unknown'
        missing: 'payment' | 'webhook_credit'
        paymentState: 'paid' | 'unpaid' | 'unknown'
        sessionMode: StripeCheckoutReceiptSessionMode
        status: 'pending'
      }>
    | Readonly<{
        message: string
        reason: string
        sessionMode: StripeCheckoutReceiptSessionMode
        status: 'invalid'
      }>
  schemaVersion: 'openagents.billing.stripe_checkout_receipt.v1'
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
}>

export type StripeCheckoutReceiptStore = Readonly<{
  readStripeCheckoutReceipt: (
    receiptRef: string,
    generatedAt: string,
  ) => Promise<PublicStripeCheckoutReceiptProjection | null>
}>

const receiptPrefix = 'receipt.billing.stripe_checkout.'

export const stripeCheckoutReceiptRef = (sessionId: string): string =>
  `${receiptPrefix}${sessionId}`

const sessionIdFromReceiptRef = (receiptRef: string): string | null =>
  receiptRef.startsWith(receiptPrefix) && receiptRef.length > receiptPrefix.length
    ? receiptRef.slice(receiptPrefix.length)
    : null

const sessionMode = (sessionId: string): StripeCheckoutReceiptSessionMode =>
  sessionId.startsWith('cs_test_')
    ? 'test'
    : sessionId.startsWith('cs_live_')
      ? 'live'
      : 'unknown'

const paymentState = (value: string | null): 'paid' | 'unpaid' | 'unknown' =>
  value === 'paid' ? 'paid' : value === 'unpaid' ? 'unpaid' : 'unknown'

const fulfillmentState = (
  value: string | null,
): 'fulfilled' | 'pending' | 'unpaid' | 'expired' | 'unknown' =>
  value === 'fulfilled' ||
  value === 'pending' ||
  value === 'unpaid' ||
  value === 'expired'
    ? value
    : 'unknown'

type CheckoutRow = Readonly<{
  fulfillment_status: string | null
  payment_status: string | null
}>

type LedgerRow = Readonly<{ amount_cents: number }>

const projection = (
  receiptRef: string,
  generatedAt: string,
  resolution: PublicStripeCheckoutReceiptProjection['resolution'],
): PublicStripeCheckoutReceiptProjection => ({
  authorityBoundary:
    'Public proof only. This Stripe checkout receipt read grants no checkout, spend, refund, payout, settlement, provider, public-claim, or registry authority.',
  caveatRefs: [
    'caveat.public.no_private_payment_material',
    'caveat.public.checkout_receipt_requires_webhook_credit_row',
    'caveat.public.pending_is_not_card_credit_completion',
  ],
  generatedAt,
  receiptRef,
  resolution,
  schemaVersion: 'openagents.billing.stripe_checkout_receipt.v1',
  sourceRefs: [
    `route:/api/public/billing/stripe-checkout-receipts/${receiptRef}`,
    'ledger.stripe_checkout_sessions.fulfillment_status',
    'ledger.billing_ledger_entries.stripe_checkout',
  ],
  staleness: liveAtReadStaleness([
    'stripe_checkout_sessions',
    'billing_ledger_entries',
  ]),
})

export const makeD1StripeCheckoutReceiptStore = (
  db: D1Database,
): StripeCheckoutReceiptStore => ({
  readStripeCheckoutReceipt: async (receiptRef, generatedAt) => {
    const sessionId = sessionIdFromReceiptRef(receiptRef)
    if (
      sessionId === null ||
      stripeCheckoutReceiptRef(sessionId) !== receiptRef
    ) {
      return null
    }

    const mode = sessionMode(sessionId)
    const checkout = await db
      .prepare(
        `SELECT payment_status, fulfillment_status
           FROM stripe_checkout_sessions
          WHERE session_id = ?
          LIMIT 1`,
      )
      .bind(sessionId)
      .first<CheckoutRow>()

    if (checkout === null) {
      return null
    }

    const payment = paymentState(checkout.payment_status)
    const fulfillment = fulfillmentState(checkout.fulfillment_status)

    if (payment !== 'paid') {
      return projection(receiptRef, generatedAt, {
        fulfillmentState:
          fulfillment === 'fulfilled' ? 'unknown' : fulfillment,
        missing: 'payment',
        paymentState: payment,
        sessionMode: mode,
        status: 'pending',
      })
    }

    const ledger = await db
      .prepare(
        `SELECT amount_cents
           FROM billing_ledger_entries
          WHERE source = 'stripe_checkout'
            AND amount_cents > 0
            AND idempotency_key = ?
          LIMIT 1`,
      )
      .bind(`billing:stripe-checkout:${sessionId}`)
      .first<LedgerRow>()

    if (fulfillment !== 'fulfilled') {
      return projection(receiptRef, generatedAt, {
        fulfillmentState: fulfillment,
        missing: 'webhook_credit',
        paymentState: 'paid',
        sessionMode: mode,
        status: 'pending',
      })
    }

    if (ledger === null) {
      return projection(receiptRef, generatedAt, {
        message:
          'The stored checkout session is fulfilled, but the positive Stripe checkout credit ledger row is missing.',
        reason: 'stripe_checkout_credit_ledger_missing',
        sessionMode: mode,
        status: 'invalid',
      })
    }

    return projection(receiptRef, generatedAt, {
      creditLedgerState: 'credited',
      fulfillmentState: 'fulfilled',
      paymentState: 'paid',
      sessionMode: mode,
      status: 'ok',
    })
  },
})
