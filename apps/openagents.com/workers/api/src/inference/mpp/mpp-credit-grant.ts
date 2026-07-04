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
import type { BillingDomainMirror } from '../../billing'
import { type LedgerStatement, runLedgerStatements } from '../../payments-ledger'
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
    /** KS-8.7 (#8318) fail-soft Postgres mirror (billing-store.ts). */
    mirror?: BillingDomainMirror | undefined
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
          deps.mirror,
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

// ---- Lightning rail (REAL Bitcoin inbound) ----
//
// RL-3 ASSET BOUNDARY (load-bearing, the OPPOSITE of the USDC/card rails): a
// settled Lightning charge is REAL Bitcoin (a paid BOLT11 invoice), NOT a USD/
// Stripe liability. So the minted credit MUST NOT be tagged `usd_credit_msat`
// (the USD-origin, NON-Bitcoin-withdrawable bucket the Lightning sweep subtracts
// in `tips-sweep.ts`). Tagging real Bitcoin as USD-origin would mislabel a real
// sat as non-withdrawable. The Lightning credit lands in `balance_msat` ONLY,
// as a Bitcoin-origin `lightning_charge` pay-in — Bitcoin-withdrawable, exactly
// like a forum tip, distinct from the USD-origin MPP credit minted for the
// USDC/card rails.
//
// In practice the credit lands in the per-payment ephemeral payer account
// (`mppPayerAccountRef`, keyed by the paymentHash) and is spent immediately by
// the SAME paid completion, so it never sits as a withdrawable balance for a
// real agent — but the asset TAG is still kept honest (Bitcoin-origin) so the
// accounting can never misclassify the inbound asset.

// Context-ref prefix binding a minted Lightning credit back to its settling
// BOLT11 paymentHash, so the grant row is dereferenceable to the paid invoice.
// Keyed on the PAYMENT HASH (public), NEVER the preimage (bearer secret).
export const MPP_LIGHTNING_CREDIT_GRANT_CONTEXT_PREFIX =
  'inference:mpp-lightning-credit:'

export const mppLightningCreditGrantContextRef = (paymentHash: string): string =>
  `${MPP_LIGHTNING_CREDIT_GRANT_CONTEXT_PREFIX}${paymentHash}`

// A stable grant ref derived from the paymentHash, so a replayed settlement
// (same paid invoice) is idempotent (one invoice = one grant) via the UNIQUE
// pay_ins idempotency key.
export const mppLightningGrantRef = (paymentHash: string): string =>
  `mpp-lightning:${paymentHash}`

export const mppLightningGrantReceiptRef = (grantRef: string): string =>
  `receipt.inference.lightning_charge.${grantRef}`

// The agent-balance ref for a Lightning MPP payer. Bound to the paymentHash so
// the minted Bitcoin-origin credit and the immediately-following Khala spend
// stay in one balance.
export const mppLightningPayerAccountRef = (paymentHash: string): string =>
  agentRefForUser(`mpp-lightning:${paymentHash}`)

// Build the atomic ledger statements for a BITCOIN-ORIGIN msat credit grant from
// a settled Lightning charge. Mirrors `usdCreditGrantStatements` but as a
// `lightning_charge` pay-in that credits `balance_msat` ONLY (it does NOT bump
// `usd_credit_msat`), so the credit is Bitcoin-withdrawable — the correct asset
// classification for real Bitcoin inbound. Idempotent per grant ref via the
// UNIQUE pay_ins idempotency key.
export const lightningCreditGrantStatements = (
  input: Readonly<{
    grantRef: string
    accountRef: string
    grantMsat: number
    contextRef: string
  }>,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  const grantMsat = Math.max(1, Math.trunc(input.grantMsat))
  const payInId = `inference:lightning-charge:${input.grantRef}`
  const legId = `${input.grantRef}:grant`
  const idempotencyKey = `inference:lightning-credit-grant:${input.grantRef}`
  const receiptRef = mppLightningGrantReceiptRef(input.grantRef)

  return [
    {
      params: [
        payInId,
        input.accountRef,
        grantMsat,
        input.contextRef,
        idempotencyKey,
        receiptRef,
        nowIso,
        nowIso,
      ],
      payInId,
      sql: `INSERT OR IGNORE INTO pay_ins
            (id, pay_in_type, payer_ref, cost_msat, state, rung, context_ref,
             idempotency_key, public_receipt_ref, genesis_id, created_at,
             state_changed_at)
            VALUES (?, 'lightning_charge', ?, ?, 'paid', NULL, ?, ?, ?, NULL, ?, ?)`,
    },
    {
      params: [input.accountRef, nowIso, nowIso],
      sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
            VALUES (?, 0, ?, ?)
            ON CONFLICT (actor_ref) DO NOTHING`,
    },
    {
      // Bitcoin-origin: credit balance_msat ONLY (NO usd_credit_msat bump).
      // GUARDED on the leg not already existing so a replay is a no-op.
      params: [grantMsat, nowIso, input.accountRef, legId],
      sql: `UPDATE agent_balances
            SET balance_msat = balance_msat + ?,
                updated_at = ?
            WHERE actor_ref = ?
              AND NOT EXISTS (SELECT 1 FROM pay_in_legs WHERE id = ?)`,
    },
    {
      params: [
        legId,
        payInId,
        grantMsat,
        input.accountRef,
        input.accountRef,
        nowIso,
      ],
      payInId,
      sql: `INSERT OR IGNORE INTO pay_in_legs
            (id, pay_in_id, direction, kind, party_ref, amount_msat,
             resulting_balance_msat, external_ref, refund_of_leg_id, created_at)
            VALUES (?, ?, 'in', 'lightning', ?, ?,
                    (SELECT balance_msat FROM agent_balances WHERE actor_ref = ?),
                    'lightning_charge', NULL, ?)`,
    },
  ]
}

// Mint Bitcoin-origin Khala credits for a settled Lightning charge. Atomic,
// idempotent per paymentHash, Bitcoin-origin (NOT tagged usd_credit_msat). The
// amount is the settled sats converted to msat (1 sat = 1000 msat).
export const mintLightningCredits = (
  deps: Readonly<{
    db: D1Database
    /** KS-8.7 (#8318) fail-soft Postgres mirror (billing-store.ts). */
    mirror?: BillingDomainMirror | undefined
    nowIso?: () => string
  }>,
  input: Readonly<{
    accountRef: string
    paymentHash: string
    amountSats: number
  }>,
): Effect.Effect<MppCreditGrantOutcome, MppCreditGrantError> =>
  Effect.gen(function* () {
    const nowIso = deps.nowIso ?? currentIsoTimestamp
    const grantMsat = Math.max(1, Math.trunc(input.amountSats) * 1000)
    const grantRef = mppLightningGrantRef(input.paymentHash)

    yield* Effect.tryPromise({
      catch: (cause: unknown) => new MppCreditGrantError(cause),
      try: () =>
        runLedgerStatements(
          deps.db,
          lightningCreditGrantStatements(
            {
              accountRef: input.accountRef,
              contextRef: mppLightningCreditGrantContextRef(input.paymentHash),
              grantMsat,
              grantRef,
            },
            nowIso(),
          ),
          deps.mirror,
        ),
    })

    return {
      accountRef: input.accountRef,
      grantedMsat: grantMsat,
      grantRef,
      receiptRef: mppLightningGrantReceiptRef(grantRef),
    }
  })
