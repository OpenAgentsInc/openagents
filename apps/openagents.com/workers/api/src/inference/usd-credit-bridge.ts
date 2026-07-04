// USD -> msat credit bridge (#5497) — close the paid-inference revenue loop.
//
// THE GAP this closes: a Stripe card purchase raises the USD
// `billing_ledger_entries` balance (keyed by user_id), which today pays only for
// Autopilot container time + Codex tokens. The inference gateway meters a
// SEPARATE msat `agent_balances` ledger (keyed by `agent:<id>`), funded only by
// Lightning. Nothing converted USD credit into the msat balance the gateway's
// balance-gate + metering hook read, so a card purchase could never fund paid
// `/v1/chat/completions`. This module is that bridge.
//
// THE TRIGGER (chosen design): an on-demand, user-initiated
// `POST /api/billing/inference-credit` action — "fund inference from my credit
// balance". It debits the caller's USD `billing_ledger_entries` and grants the
// equivalent msat into the caller's agent balance (`agent:<userId>`). We do NOT
// auto-move the user's whole balance on purchase; the user (or their agent)
// chooses how much credit to make inference-spendable. The conversion is the
// SINGLE source of truth in `usd-msat-conversion.ts` (the same rate the metering
// hook charges at). Both sides land in ONE D1 batch (atomic), idempotent per
// grant ref (UNIQUE), bounded by the available USD balance, and never negative.
//
// RL-3 ASSET BOUNDARY (openagents #5460, INVARIANTS "Credit<->Bitcoin Asset
// Boundary"): a card dollar is a USD liability, NOT Bitcoin. The granted msat is
// inference-spendable (it lands in `balance_msat`, which the gate/metering read)
// but it is recorded as USD-origin in `agent_balances.usd_credit_msat`. The
// Lightning sweep (the live Bitcoin-withdrawal path, `tips-sweep.ts`) subtracts
// `usd_credit_msat` from the sweepable amount, so a USD-purchased credit can
// never be paid out as real Bitcoin. This module also calls the shared
// `validateAssetBoundary` guard with `revenueAsset: 'usd'` so the boundary is
// enforced by the SAME primitive every other value-movement path uses.

import { Effect, Schema as S } from 'effect'

import {
  type AssetBoundaryAsset,
  validateAssetBoundary,
} from '../asset-bitcoin-boundary'
import {
  type BillingRuntime,
  readBillingBalanceCents,
  systemBillingRuntime,
} from '../billing'
import { workerLogEntry } from '../observability'
import { type LedgerStatement, runLedgerStatements } from '../payments-ledger'
import { cardCreditGrantContextRef } from './card-credit-provenance'
import { usdCentsToMsatFloor } from './usd-msat-conversion'

// Infrastructure failure (D1 read/write) while funding inference credit. Domain
// outcomes (insufficient balance, zero conversion, boundary denial) are NOT
// errors — they are part of the typed success value. This error channel carries
// only genuine storage failures, which the route surfaces as a 5xx.
export class UsdCreditBridgeError extends S.TaggedErrorClass<UsdCreditBridgeError>()(
  'UsdCreditBridgeError',
  { cause: S.Defect },
) {}

// The agent balance ref for a user. The inference gateway authenticates as
// `agent:<user.id>` and the USD ledger keys by the same user id, so the bridge
// grants into the same account the gateway meters.
export const agentRefForUser = (userId: string): string => `agent:${userId}`

// Stable, public-safe idempotency key for a credit grant. One key per grant ref
// so a retried/replayed fund action (same ref) hits the `pay_ins.idempotency_key
// UNIQUE` constraint and is a no-op (never double-grants). Neutral; no payment
// material.
export const usdCreditGrantIdempotencyKey = (grantRef: string): string =>
  `inference:usd-credit-grant:${grantRef}`

// Public-safe receipt ref for a credit grant. Resolvable without exposing the
// idempotency key, amount, or any payment material.
export const usdCreditGrantReceiptRef = (grantRef: string): string =>
  `receipt.inference.usd_credit_grant.${grantRef}`

// Stable, idempotent USD-debit idempotency key for the billing side of the same
// grant. Paired 1:1 with the msat grant ref so the USD debit and the msat grant
// are both no-ops on replay.
export const usdCreditDebitIdempotencyKey = (grantRef: string): string =>
  `billing:inference-credit:${grantRef}`

// Build the atomic ledger statements for a USD-origin msat credit grant. This is
// a single `usd_credit_grant` pay-in marked `paid`, that:
//   1. ensures the balance row exists,
//   2. credits `balance_msat` (inference-spendable), AND
//   3. bumps `usd_credit_msat` by the same amount (USD-origin tag, RL-3),
// then records an `in`/`balance` leg with the resulting balance for audit.
// Reuses the same one-batch-is-one-transaction discipline as the rest of the
// ledger; the credit + the USD-origin tag move together or not at all.
export const usdCreditGrantStatements = (
  input: Readonly<{
    grantRef: string
    accountRef: string
    grantMsat: number
    contextRef: string
  }>,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  // Precondition (enforced by the only caller, `fundInferenceFromCredit`):
  // grantMsat is a positive integer. Clamp defensively so a malformed call can
  // never write a zero/fractional grant that would trip the cost_msat CHECK.
  const grantMsat = Math.max(1, Math.trunc(input.grantMsat))

  const payInId = `inference:usd-credit:${input.grantRef}`
  const legId = `${input.grantRef}:grant`
  const idempotencyKey = usdCreditGrantIdempotencyKey(input.grantRef)
  const receiptRef = usdCreditGrantReceiptRef(input.grantRef)

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
            VALUES (?, 'usd_credit_grant', ?, ?, 'paid', NULL, ?, ?, ?, NULL, ?, ?)`,
    },
    {
      mirror: {
        keyColumn: 'actor_ref',
        keys: [input.accountRef],
        table: 'agent_balances',
      },
      params: [input.accountRef, nowIso, nowIso],
      sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
            VALUES (?, 0, ?, ?)
            ON CONFLICT (actor_ref) DO NOTHING`,
    },
    {
      // Credit the spendable balance AND tag the USD-origin portion in one UPDATE
      // so the two can never diverge (RL-3: usd_credit_msat <= balance_msat).
      // GUARDED on the leg not already existing: on an idempotent replay the
      // pay_in INSERT OR IGNOREd, so the leg row already exists and this credit
      // is a no-op — never double-credits.
      mirror: {
        keyColumn: 'actor_ref',
        keys: [input.accountRef],
        table: 'agent_balances',
      },
      params: [grantMsat, grantMsat, nowIso, input.accountRef, legId],
      sql: `UPDATE agent_balances
            SET balance_msat = balance_msat + ?,
                usd_credit_msat = COALESCE(usd_credit_msat, 0) + ?,
                updated_at = ?
            WHERE actor_ref = ?
              AND NOT EXISTS (SELECT 1 FROM pay_in_legs WHERE id = ?)`,
    },
    {
      // The audit leg. INSERT OR IGNORE so a replay (same legId) is a no-op,
      // pairing with the guarded credit above for exactly-once semantics.
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
            VALUES (?, ?, 'in', 'balance', ?, ?,
                    (SELECT balance_msat FROM agent_balances WHERE actor_ref = ?),
                    'usd_credit_grant', NULL, ?)`,
    },
  ]
}

// The USD-debit insert (negative cents) for the billing side of the grant. The
// USD ledger balance is SUM(amount_cents), so a negative row debits it. Idempotent
// per grant ref via the UNIQUE idempotency_key (INSERT OR IGNORE).
const usdCreditDebitStatement = (
  input: Readonly<{
    grantRef: string
    userId: string
    amountCents: number
    grantMsat: number
  }>,
  nowIso: string,
  runtime: BillingRuntime,
): LedgerStatement => ({
  params: [
    runtime.randomId('bill'),
    input.userId,
    `Funded inference credit: ${input.amountCents} credit cents`,
    -input.amountCents,
    'USD',
    input.amountCents,
    'credit_cents',
    JSON.stringify({
      grantMsat: input.grantMsat,
      grantRef: input.grantRef,
      reason: 'inference_credit_grant',
    }),
    usdCreditDebitIdempotencyKey(input.grantRef),
    nowIso,
  ],
  // 'manual_adjustment' is the existing source for an operator/internal balance
  // movement on the USD ledger; this is an internal USD->msat transfer, not a new
  // external charge, so it reuses that source (no billing schema change needed).
  sql: `INSERT OR IGNORE INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
        VALUES (?, ?, NULL, NULL, 'manual_adjustment', ?, ?, ?, ?, ?, NULL, ?, ?, ?)`,
})

export type FundInferenceFromCreditOutcome =
  | Readonly<{
      ok: true
      grantRef: string
      grantedCents: number
      grantedMsat: number
      receiptRef: string
      // USD balance (cents) AFTER the debit; derived (SUM), never a mutable
      // counter. Public-safe to the authenticated owner.
      remainingCreditCents: number
    }>
  | Readonly<{
      ok: false
      reason:
        | 'amount_invalid'
        | 'insufficient_credit'
        | 'zero_after_conversion'
        | 'asset_boundary_violation'
      message: string
      // The boundary denial reason ref when reason === 'asset_boundary_violation'.
      reasonRef?: string
    }>

export type FundInferenceFromCreditDeps = Readonly<{
  db: D1Database
  // ISO timestamp source. Defaults to the runtime clock through the billing
  // runtime, but injectable for deterministic tests.
  nowIso?: () => string
  // USD-cents -> msat conversion (the single source). Defaults to
  // `usdCentsToMsatFloor` at the shared rate; tests/oracle inject their own.
  usdCentsToMsat?: (amountCents: number) => number
  // Billing runtime (random id + clock). Defaults to the system runtime.
  billingRuntime?: BillingRuntime
  // The asset the contributor/credit side resolves to. ALWAYS 'usd' for a card
  // purchase; surfaced as a dep only so a future Bitcoin-funded credit-purchase
  // rail can pass 'bitcoin'. The boundary check below uses it.
  revenueAsset?: AssetBoundaryAsset
}>

// Fund inference from a user's USD credit balance. Debits `amountCents` of the
// user's USD `billing_ledger_entries` and grants the equivalent msat into the
// user's agent balance as a USD-origin (`usd_credit_msat`) credit. Bounded by the
// available USD balance, idempotent per grant ref, atomic, never negative, and
// RL-3-honored (USD-origin => inference-spendable, NOT Bitcoin-withdrawable).
export const fundInferenceFromCredit = (
  input: Readonly<{
    userId: string
    // Requested USD cents to convert. Clamped to the available balance.
    amountCents: number
    // Idempotency/correlation ref for THIS fund action (e.g. a request id or a
    // client-supplied token). One ref = one grant.
    grantRef: string
    // OPTIONAL provenance: the Stripe checkout session that funded this grant.
    // When present, the grant's `context_ref` is stamped with the card-origin
    // format (`card-credit-provenance.ts`) so the grant is dereferenceable back
    // to its purchase for the card->credit->inference-spend chain receipt. When
    // absent (Lightning-/balance-funded grants), the legacy generic context_ref
    // is used and the chain has no card origin to bind.
    sourceCheckoutSessionId?: string
  }>,
  deps: FundInferenceFromCreditDeps,
): Effect.Effect<FundInferenceFromCreditOutcome, UsdCreditBridgeError> =>
  Effect.gen(function* () {
    const runtime = deps.billingRuntime ?? systemBillingRuntime
    const nowIso = deps.nowIso ?? runtime.nowIso
    const usdCentsToMsat = deps.usdCentsToMsat ?? usdCentsToMsatFloor
    const revenueAsset: AssetBoundaryAsset = deps.revenueAsset ?? 'usd'

    const requested = Math.trunc(input.amountCents)
    if (!Number.isFinite(requested) || requested <= 0) {
      return {
        message: 'Amount must be a positive number of credit cents.',
        ok: false,
        reason: 'amount_invalid',
      }
    }

    // RL-3: this grant must NEVER create a withdrawable Bitcoin liability. The
    // granted msat is recorded USD-origin (non-withdrawable), so the
    // contributor/share asset is the same non-Bitcoin asset as the revenue. We
    // assert that crossing here through the shared guard: a USD/credit "spend"
    // movement may fund a USD/credit share but not a Bitcoin one. (We never
    // request a Bitcoin contributor asset here; the assertion documents and
    // enforces that the grant is not a Bitcoin liability.)
    const violation = validateAssetBoundary({
      contributorAsset: revenueAsset,
      movement: 'spend',
      revenueAsset,
    })
    if (violation !== null) {
      yield* Effect.logInfo(
        workerLogEntry('inference.usd_credit_grant.boundary_denied', {
          grantRef: input.grantRef,
          reasonRef: violation.reasonRef,
          userId: input.userId,
        }),
      )
      return {
        message: violation.reason,
        ok: false,
        reason: 'asset_boundary_violation',
        reasonRef: violation.reasonRef,
      }
    }

    const availableCents = yield* Effect.tryPromise({
      catch: (cause: unknown) => new UsdCreditBridgeError({ cause }),
      try: () => readBillingBalanceCents(deps.db, input.userId),
    })

    // Bound the grant by the available USD balance (never overdraw the USD
    // ledger). Clamp DOWN to what is actually available.
    const grantCents = Math.min(requested, Math.max(0, availableCents))
    if (grantCents <= 0) {
      return {
        message: 'Not enough credit balance to fund inference.',
        ok: false,
        reason: 'insufficient_credit',
      }
    }

    const grantMsat = usdCentsToMsat(grantCents)
    if (grantMsat <= 0) {
      // The requested cents rounded below 1 msat at the current rate; nothing to
      // grant. We do NOT debit USD for a zero grant.
      return {
        message: 'Amount is too small to convert to a credit grant.',
        ok: false,
        reason: 'zero_after_conversion',
      }
    }

    const accountRef = agentRefForUser(input.userId)
    const now = nowIso()

    // Stamp the grant with its card-purchase origin when we know it, so the
    // grant row is dereferenceable back to the funding Stripe session for the
    // card->credit->inference-spend chain receipt. Falls back to the legacy
    // generic context_ref for non-card (Lightning/balance) grants.
    const contextRef =
      (input.sourceCheckoutSessionId !== undefined
        ? cardCreditGrantContextRef(input.sourceCheckoutSessionId)
        : undefined) ?? `inference:usd-credit:${input.userId}`

    // Atomic: USD debit + msat grant in ONE D1 batch. The USD debit uses
    // INSERT OR IGNORE on a UNIQUE idempotency key and the msat grant uses the
    // UNIQUE pay_ins idempotency key, so a replayed fund (same grantRef) is a
    // no-op on both sides — the grant is exactly-once.
    const mirror = runtime.mirror
    yield* Effect.tryPromise({
      catch: (cause: unknown) => new UsdCreditBridgeError({ cause }),
      try: () =>
        runLedgerStatements(deps.db, [
          usdCreditDebitStatement(
            {
              amountCents: grantCents,
              grantMsat,
              grantRef: input.grantRef,
              userId: input.userId,
            },
            now,
            runtime,
          ),
          ...usdCreditGrantStatements(
            {
              accountRef,
              contextRef,
              grantMsat,
              grantRef: input.grantRef,
            },
            now,
          ),
        ], mirror),
    })

    // KS-8.7: mirror the USD-debit ledger row too (the pay-in + legs were
    // mirrored by runLedgerStatements via their annotations).
    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror(deps.db, [
          {
            key: {
              idempotency_key: usdCreditDebitIdempotencyKey(input.grantRef),
            },
            table: 'billing_ledger_entries',
          },
        ]),
      )
    }

    const remainingCreditCents = yield* Effect.tryPromise({
      catch: (cause: unknown) => new UsdCreditBridgeError({ cause }),
      try: () => readBillingBalanceCents(deps.db, input.userId),
    })

    yield* Effect.logInfo(
      workerLogEntry('inference.usd_credit_grant.funded', {
        grantMsat,
        grantRef: input.grantRef,
        grantedCents: grantCents,
        userId: input.userId,
      }),
    )

    return {
      grantRef: input.grantRef,
      grantedCents: grantCents,
      grantedMsat: grantMsat,
      ok: true,
      receiptRef: usdCreditGrantReceiptRef(input.grantRef),
      remainingCreditCents,
    }
  })
