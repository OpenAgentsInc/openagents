// Phase 3 — settled MPP payment → Khala credits, unified with the one-balance,
// two-inbound-rails money loop (EPIC #6049).
//
// THE UNIFICATION: a settled MPP payment (USDC via x402/MPP, or card via SPT)
// is an inbound rail ALONGSIDE Bitcoin/Spark. Rather than fork a new money path,
// a settled MPP charge mints Khala credits into the SAME `agent_balances` ledger
// the gateway's balance-gate + metering hook already read, by REUSING the
// existing USD-origin credit-grant seam (`usdCreditGrantStatements`,
// inference/usd-credit-bridge.ts, #5497). The agent then spends those credits on
// the normal Khala completion path, metered receipt-first by the EXISTING
// metering hook + receipt; the contributor payout that loop drives stays
// Bitcoin/Spark.
//
// RL-3 ASSET BOUNDARY (correct + load-bearing here): MPP funds are USDC/card
// settled into a Stripe balance — a USD liability, NOT Bitcoin. So the minted
// credit is recorded USD-origin (`agent_balances.usd_credit_msat`), making it
// inference-spendable but NOT Bitcoin-withdrawable — exactly the property the
// existing `usdCreditGrantStatements` enforces. We do NOT debit any internal USD
// `billing_ledger_entries` (unlike the card-checkout bridge): the funds are
// EXTERNAL Stripe money, so this is a pure mint of credit backed by the settled
// PaymentIntent, idempotent per payment id.

import { Effect } from 'effect'

import { currentIsoTimestamp } from '../../runtime-primitives'
import { runLedgerStatements } from '../../payments-ledger'
import { usdCentsToMsatFloor } from '../usd-msat-conversion'
import {
  agentRefForUser,
  usdCreditGrantReceiptRef,
  usdCreditGrantStatements,
} from '../usd-credit-bridge'

// Context-ref prefix binding a minted credit back to its settling MPP payment,
// so the grant row is dereferenceable to the PaymentIntent that funded it.
export const MPP_CREDIT_GRANT_CONTEXT_PREFIX = 'inference:mpp-credit:'

export const mppCreditGrantContextRef = (paymentIntentId: string): string =>
  `${MPP_CREDIT_GRANT_CONTEXT_PREFIX}${paymentIntentId}`

// A stable grant ref derived from the Stripe PaymentIntent id, so a replayed
// settlement (same payment) is idempotent (one payment = one grant) via the
// UNIQUE pay_ins idempotency key inside `usdCreditGrantStatements`.
export const mppGrantRef = (paymentIntentId: string): string =>
  `mpp:${paymentIntentId}`

export class MppCreditGrantError extends Error {
  override readonly name = 'MppCreditGrantError'
  override readonly cause: unknown
  constructor(cause: unknown) {
    super('mpp credit grant failed')
    this.cause = cause
  }
}

export type MppCreditGrantOutcome = Readonly<{
  grantRef: string
  grantedMsat: number
  receiptRef: string
  // The agent balance ref the credit landed in.
  accountRef: string
}>

// Mint Khala credits for a settled MPP payment. Atomic, idempotent per payment
// id, USD-origin (RL-3: inference-spendable, not Bitcoin-withdrawable). Returns
// the grant outcome. The amount is the settled charge converted at the SAME rate
// the metering hook charges at (`usdCentsToMsatFloor`).
export const mintMppCredits = (
  deps: Readonly<{
    db: D1Database
    nowIso?: () => string
    // Conversion override for tests. Defaults to the shared USD-cents->msat rate.
    usdCentsToMsat?: (amountCents: number) => number
  }>,
  input: Readonly<{
    // The account the credit lands in. The gateway authenticates the paid call
    // and resolves the account; for an unauthenticated machine-payment call the
    // account is the payment itself (keyed by the PaymentIntent id) so the credit
    // and the spend stay bound to one payer.
    accountRef: string
    paymentIntentId: string
    amountCents: number
  }>,
): Effect.Effect<MppCreditGrantOutcome, MppCreditGrantError> =>
  Effect.gen(function* () {
    const nowIso = deps.nowIso ?? currentIsoTimestamp
    const usdCentsToMsat = deps.usdCentsToMsat ?? usdCentsToMsatFloor
    const grantMsat = Math.max(1, usdCentsToMsat(input.amountCents))
    const grantRef = mppGrantRef(input.paymentIntentId)

    yield* Effect.tryPromise({
      catch: (cause: unknown) => new MppCreditGrantError(cause),
      try: () =>
        runLedgerStatements(
          deps.db,
          usdCreditGrantStatements(
            {
              accountRef: input.accountRef,
              contextRef: mppCreditGrantContextRef(input.paymentIntentId),
              grantMsat,
              grantRef,
            },
            nowIso(),
          ),
        ),
    })

    return {
      accountRef: input.accountRef,
      grantedMsat: grantMsat,
      grantRef,
      receiptRef: usdCreditGrantReceiptRef(grantRef),
    }
  })

// The agent-balance ref for an MPP payer that has no OpenAgents account (pure
// machine-payment caller). Bound to the PaymentIntent so the minted credit and
// the immediately-following Khala spend stay in one balance, distinct from any
// real agent account.
export const mppPayerAccountRef = (paymentIntentId: string): string =>
  agentRefForUser(`mpp:${paymentIntentId}`)
