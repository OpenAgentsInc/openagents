import {
  billingPostgresRawQueryForEnv,
  billingSyncFlagsFromEnv,
  type BillingSyncEnv,
  type MakeBillingStoreOptions,
} from './billing-store'
import { logWorkerRouteWarning } from './observability'
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

/**
 * The two narrow row reads this receipt store needs, abstracted away from
 * the underlying store (D1 vs Postgres). Both rows are ALREADY-SETTLED and
 * effectively immutable once `fulfillment_status = 'fulfilled'` and the
 * credit ledger row exists — this receipt read never gates a live
 * checkout/webhook/spend decision, which is what makes it eligible for the
 * #8337 bounded Postgres-served read allowlist
 * (`BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`, billing-store.ts).
 */
type StripeCheckoutReceiptRowReader = Readonly<{
  readCheckoutRow: (sessionId: string) => Promise<CheckoutRow | null>
  readLedgerRow: (sessionId: string) => Promise<LedgerRow | null>
}>

const makeStripeCheckoutReceiptStoreFromReader = (
  reader: StripeCheckoutReceiptRowReader,
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
    const checkout = await reader.readCheckoutRow(sessionId)

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

    const ledger = await reader.readLedgerRow(sessionId)

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

export const makeD1StripeCheckoutReceiptStore = (
  db: D1Database,
): StripeCheckoutReceiptStore =>
  makeStripeCheckoutReceiptStoreFromReader({
    readCheckoutRow: sessionId =>
      db
        .prepare(
          `SELECT payment_status, fulfillment_status
             FROM stripe_checkout_sessions
            WHERE session_id = ?
            LIMIT 1`,
        )
        .bind(sessionId)
        .first<CheckoutRow>(),
    readLedgerRow: sessionId =>
      db
        .prepare(
          `SELECT amount_cents
             FROM billing_ledger_entries
            WHERE source = 'stripe_checkout'
              AND amount_cents > 0
              AND idempotency_key = ?
            LIMIT 1`,
        )
        .bind(`billing:stripe-checkout:${sessionId}`)
        .first<LedgerRow>(),
  })

/**
 * #8337: the Postgres-served twin of `makeD1StripeCheckoutReceiptStore`,
 * built on the shared `BillingPostgresRawQuery` seam
 * (`billingPostgresRawQueryForEnv`, billing-store.ts) so this file does not
 * depend on the wider `PostgresBillingStore`/`BillingRuntime` surface.
 */
export const makePostgresStripeCheckoutReceiptStore = (
  query: (
    text: string,
    params: ReadonlyArray<unknown>,
  ) => Promise<ReadonlyArray<Readonly<Record<string, unknown>>>>,
): StripeCheckoutReceiptStore =>
  makeStripeCheckoutReceiptStoreFromReader({
    readCheckoutRow: async sessionId => {
      const rows = await query(
        `SELECT payment_status, fulfillment_status
           FROM stripe_checkout_sessions
          WHERE session_id = $1
          LIMIT 1`,
        [sessionId],
      )
      const row = rows[0]
      return row === undefined
        ? null
        : {
            fulfillment_status:
              row['fulfillment_status'] === null
                ? null
                : String(row['fulfillment_status']),
            payment_status:
              row['payment_status'] === null
                ? null
                : String(row['payment_status']),
          }
    },
    readLedgerRow: async sessionId => {
      const rows = await query(
        `SELECT amount_cents
           FROM billing_ledger_entries
          WHERE source = 'stripe_checkout'
            AND amount_cents > 0
            AND idempotency_key = $1
          LIMIT 1`,
        [`billing:stripe-checkout:${sessionId}`],
      )
      const row = rows[0]
      return row === undefined
        ? null
        : { amount_cents: Number(row['amount_cents']) }
    },
  })

/**
 * The #8337 KHALA_SYNC_BILLING_READS router for this receipt store:
 *   'd1'      — plain D1 (the default; also the fallback whenever the
 *               Postgres store is unavailable or the flag is 'd1')
 *   'compare' — read both, SERVE D1, log any divergence
 *   'postgres' — serve Postgres (single attempt), D1 fallback on failure
 * Mirrors the compare/postgres semantics in `billing-store.ts` exactly,
 * duplicated locally (rather than imported) to keep this file's dependency
 * surface to the shared `BillingPostgresRawQuery` seam only.
 */
export const makeReadsRoutedStripeCheckoutReceiptStore = (input: {
  d1: StripeCheckoutReceiptStore
  postgres: StripeCheckoutReceiptStore
  reads: 'compare' | 'postgres'
  log: (
    event:
      | 'khala_sync_billing_read_compare_mismatch'
      | 'khala_sync_billing_postgres_read_failed'
      | 'khala_sync_billing_postgres_read_fallback',
    fields: Readonly<{ messageSafe: string; op: string; refs: ReadonlyArray<string> }>,
  ) => void
}): StripeCheckoutReceiptStore => ({
  readStripeCheckoutReceipt: async (receiptRef, generatedAt) => {
    if (input.reads === 'postgres') {
      try {
        return await input.postgres.readStripeCheckoutReceipt(
          receiptRef,
          generatedAt,
        )
      } catch (error) {
        input.log('khala_sync_billing_postgres_read_fallback', {
          messageSafe: error instanceof Error ? error.message : String(error),
          op: 'readStripeCheckoutReceipt',
          refs: [receiptRef],
        })
      }
    }

    const d1Result = await input.d1.readStripeCheckoutReceipt(
      receiptRef,
      generatedAt,
    )

    if (input.reads === 'compare') {
      try {
        const postgresResult = await input.postgres.readStripeCheckoutReceipt(
          receiptRef,
          generatedAt,
        )
        if (JSON.stringify(postgresResult) !== JSON.stringify(d1Result)) {
          input.log('khala_sync_billing_read_compare_mismatch', {
            messageSafe: 'stripe checkout receipt differs between d1 and postgres',
            op: 'readStripeCheckoutReceipt',
            refs: [receiptRef],
          })
        }
      } catch (error) {
        input.log('khala_sync_billing_postgres_read_failed', {
          messageSafe: error instanceof Error ? error.message : String(error),
          op: 'readStripeCheckoutReceipt',
          refs: [receiptRef],
        })
      }
    }

    return d1Result
  },
})

/**
 * The env-wiring drop-in for `makePublicStripeCheckoutReceiptRoutes`'
 * `makeStore` (index.ts): builds the D1 store exactly as before, then —
 * only when `KHALA_SYNC_DB` is bound and `KHALA_SYNC_BILLING_READS !==
 * 'd1'` — wraps it with the #8337 compare/postgres router. Degrades to the
 * plain D1 store whenever the binding is absent or the flag is 'd1', the
 * same zero-overhead posture as every other billing env factory.
 */
export const stripeCheckoutReceiptStoreForEnv = (
  env: BillingSyncEnv,
  d1Store: StripeCheckoutReceiptStore,
  options: MakeBillingStoreOptions = {},
): StripeCheckoutReceiptStore => {
  const flags = billingSyncFlagsFromEnv(env)
  if (flags.reads === 'd1') {
    return d1Store
  }
  const query = billingPostgresRawQueryForEnv(env, options)
  if (query === undefined) {
    return d1Store
  }
  const log =
    options.log ??
    ((event, fields) =>
      logWorkerRouteWarning(event, {
        messageSafe: fields.messageSafe,
        op: fields.op,
        refs: fields.refs.slice(0, 10).join(','),
      }))
  return makeReadsRoutedStripeCheckoutReceiptStore({
    d1: d1Store,
    log,
    postgres: makePostgresStripeCheckoutReceiptStore(query),
    reads: flags.reads,
  })
}
