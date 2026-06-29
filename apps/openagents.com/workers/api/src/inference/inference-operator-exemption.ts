// Owner-controlled balance-gate EXEMPTION for the inference gateway (issue
// #6180). Lets approved/internal keys test our OWN inference lanes (e.g.
// `openagents/khala` / `khala-oss-20b` — GPT-OSS on the hourly Hydralisk box,
// zero marginal per-token cost to us) WITHOUT a funded balance, while Khala
// stays a paid product for the public. Mirrors `inference-premium-allowlist.ts`:
// an owner-keyed `grant`/`revoke`/`isExempt` store + migration, a pure decision
// (`decideOperatorExemption`), and a route-gate seam (`makeOperatorExemptionGate`).
//
// HOW IT FITS THE GATE: an exempt key bypasses the BALANCE GATE 402 in
// `chat-completions-routes.ts` (alongside the free-allowance bypass). Its usage
// is recorded as `operator_credit` — an HONEST zero-credit-debit record with a
// receipt (INVARIANTS.md: "Own-Pylon free-lane work may report zero credit debit
// only while still preserving local/provider usage refs"), NOT a silent skip and
// NO referral accrual. The `operator_credit` metering wrapper
// (`withOperatorCredit`) is the metering-hook decorator that records that
// zero-debit receipt instead of calling the inner ledger hook.
//
// HARD GUARDRAIL — own-infra / non-premium lanes ONLY. The exemption may apply
// only to NON-premium model classes (`gemini`, `open` incl. the `hydralisk`
// GPT-OSS lane). It must NEVER exempt a premium class (`claude`, `unknown` /
// partner passthrough): a premium model from an exempt key still hits the normal
// balance + premium-grant gates, so an exempt key can never rack up real
// third-party cost for free. Premium is DENY-BY-DEFAULT here; we reuse
// `isPremiumModel` (model-router classes) so the premium set stays single-source.
//
// IDENTITY: the exemption keys on the VERIFIED OWNER identity (`owner:<id>` —
// inference-owner-identity.ts), the SAME owner key the free pool / premium
// allowlist use. It REFUSES to exempt a synthetic unclaimed `account:` key (an
// unclaimed account has no verified owner to approve). Granting one verified
// owner covers ALL of that owner's accounts/autopilots.
//
// ARMED / OWNER-GATED: the store + gate are inert by default. The Worker wires
// the gate only when `INFERENCE_OPERATOR_EXEMPTION_ENABLED` is on (a fail-closed
// env flag, default OFF, like the other gateway gates). Granting is an
// owner/admin action. No owner id / token value is ever printed into
// logs/commits/receipts.

import { Effect } from 'effect'

import { workerLogEntry } from '../observability'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  type VerifiedOwnerIdentityResolver,
  isVerifiedOwnerKey,
  resolveOwnerKey,
} from './inference-owner-identity'
import {
  type MeteringContext,
  type MeteringHook,
  type MeteringOutcome,
} from './metering-hook'
import { isPremiumModel } from './inference-premium-allowlist'

class OperatorExemptionPersistenceError extends Error {
  readonly _tag = 'OperatorExemptionPersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'OperatorExemptionPersistenceError'
  }
}

const operatorExemptionPersistenceError = (error: unknown) =>
  new OperatorExemptionPersistenceError(error)

// ----------------------------------------------------------------------------
// Env flag (fail-closed, default OFF)
// ----------------------------------------------------------------------------

export const INFERENCE_OPERATOR_EXEMPTION_ENABLED_ENV_KEY =
  'INFERENCE_OPERATOR_EXEMPTION_ENABLED' as const

const ON_TOKENS = new Set(['1', 'on', 'true', 'yes'])

// Fail-closed flag read. Absent / non-string / any non-on value => disabled.
export const isOperatorExemptionEnabled = (value: unknown): boolean =>
  typeof value === 'string' && ON_TOKENS.has(value.trim().toLowerCase())

// ----------------------------------------------------------------------------
// Pure decision
// ----------------------------------------------------------------------------

export type OperatorExemptionDecision = Readonly<{
  // True when the request may BYPASS the balance gate (a non-premium / own-infra
  // model AND the resolved owner is exempt). False => the gate is unchanged (the
  // request must clear the normal balance gate, paid-Khala behavior intact).
  exempt: boolean
  // Whether the requested model is premium (diagnostics). A premium model is
  // NEVER exempt regardless of the grant.
  premium: boolean
  // Stable, neutral reason ref (observability). Never payment material.
  reasonRef: string
}>

export const OPERATOR_EXEMPTION_REASON_EXEMPT =
  'reason.inference_operator_exemption.exempt' as const
export const OPERATOR_EXEMPTION_REASON_NOT_GRANTED =
  'reason.inference_operator_exemption.owner_not_granted' as const
export const OPERATOR_EXEMPTION_REASON_PREMIUM_DENIED =
  'reason.inference_operator_exemption.premium_never_exempt' as const

// Decide whether a request for `model` may bypass the balance gate, given
// whether the resolved owner is on the exemption store. PURE. The HARD GUARDRAIL
// is enforced first: a PREMIUM model (`claude` / `unknown`) is NEVER exempt — it
// falls through to the normal balance + premium gates even for a granted owner.
// A non-premium / own-infra model is exempt only when the owner is granted.
export const decideOperatorExemption = (
  input: Readonly<{
    model: string
    ownerExempt: boolean
  }>,
): OperatorExemptionDecision => {
  // GUARDRAIL: premium models are never exempted (deny-by-default for premium).
  if (isPremiumModel(input.model)) {
    return {
      exempt: false,
      premium: true,
      reasonRef: OPERATOR_EXEMPTION_REASON_PREMIUM_DENIED,
    }
  }
  if (input.ownerExempt) {
    return {
      exempt: true,
      premium: false,
      reasonRef: OPERATOR_EXEMPTION_REASON_EXEMPT,
    }
  }
  return {
    exempt: false,
    premium: false,
    reasonRef: OPERATOR_EXEMPTION_REASON_NOT_GRANTED,
  }
}

// ----------------------------------------------------------------------------
// Exemption store (owner/admin-controlled)
// ----------------------------------------------------------------------------

// Default grant scope. Reserves room for per-lane scopes later; today a row
// exempts the whole non-premium / own-infra lane set.
export const DEFAULT_OPERATOR_EXEMPTION_SCOPE = 'own_infra_non_premium' as const

export type OperatorExemptionGrant = Readonly<{
  ownerKey: string
  scope: string
  grantedBy: string | null
  note: string | null
}>

// Plain-async read of whether an owner key is on the exemption store. The route
// gate (a Promise-returning seam) uses this directly so no Effect->Promise bridge
// runs on the hot path. Returns false on any read error (fail-closed).
export const readOwnerExempt = async (
  db: D1Database,
  ownerKey: string,
): Promise<boolean> => {
  try {
    const row = await db
      .prepare(
        `SELECT owner_key FROM inference_operator_exemption WHERE owner_key = ? LIMIT 1`,
      )
      .bind(ownerKey)
      .first<{ owner_key: string }>()
    return row !== null
  } catch {
    return false
  }
}

// Whether an owner key is currently exempt, as an Effect (for Effect-shaped
// callers / tests). Wraps `readOwnerExempt`.
export const isExempt = (
  db: D1Database,
  ownerKey: string,
): Effect.Effect<boolean> => Effect.promise(() => readOwnerExempt(db, ownerKey))

// Grant the balance-gate exemption to an owner key (idempotent upsert). The
// owner/admin surface calls this. REFUSES a non-verified (synthetic unclaimed
// `account:`) owner key — exemption is verified-owner-only. Returns whether the
// grant is now in place.
export const grantOperatorExemption = (
  db: D1Database,
  input: Readonly<{
    ownerKey: string
    grantedBy?: string | null | undefined
    note?: string | null | undefined
    scope?: string | undefined
    nowIso?: (() => string) | undefined
  }>,
): Effect.Effect<boolean> =>
  Effect.gen(function* () {
    // Exemption applies to VERIFIED owner identities only; an unclaimed account
    // key is never exempted (it has no verified owner to approve).
    if (!isVerifiedOwnerKey(input.ownerKey)) {
      return false
    }
    const nowIso = (input.nowIso ?? currentIsoTimestamp)()
    return yield* Effect.tryPromise({
      catch: operatorExemptionPersistenceError,
      try: async () => {
        await db
          .prepare(
            `INSERT INTO inference_operator_exemption
               (owner_key, scope, granted_by, note, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(owner_key) DO UPDATE SET
               scope = excluded.scope,
               granted_by = excluded.granted_by,
               note = excluded.note,
               updated_at = excluded.updated_at`,
          )
          .bind(
            input.ownerKey,
            input.scope ?? DEFAULT_OPERATOR_EXEMPTION_SCOPE,
            input.grantedBy ?? null,
            input.note ?? null,
            nowIso,
            nowIso,
          )
          .run()
        return true
      },
    }).pipe(Effect.catch(() => Effect.succeed(false)))
  })

// Revoke an owner key's exemption (idempotent). Returns whether the call
// completed without error (the key is absent afterward regardless).
export const revokeOperatorExemption = (
  db: D1Database,
  ownerKey: string,
): Effect.Effect<boolean> =>
  Effect.tryPromise({
    catch: operatorExemptionPersistenceError,
    try: async () => {
      await db
        .prepare(
          `DELETE FROM inference_operator_exemption WHERE owner_key = ?`,
        )
        .bind(ownerKey)
        .run()
      return true
    },
  }).pipe(Effect.catch(() => Effect.succeed(false)))

// ----------------------------------------------------------------------------
// Route gate seam (balance-gate bypass)
// ----------------------------------------------------------------------------

export type OperatorExemptionGateDeps = Readonly<{
  db: D1Database
  resolveOwnerIdentity: VerifiedOwnerIdentityResolver
}>

// A route-level balance-gate exemption check: given an account ref + requested
// model, returns whether the request may bypass the balance gate. PREMIUM models
// short-circuit to NOT-exempt WITHOUT a DB read (deny-by-default guardrail), so a
// premium model from an exempt key still hits the normal gates. An unclaimed
// account resolves to a synthetic `account:` key, which is never on the store
// (grant refuses it), so it is never exempt. Wired into the chat-completions
// route's `checkOperatorExemption` seam (open/no-op when unwired or flag-off).
export type OperatorExemptionGate = (
  accountRef: string,
  model: string,
) => Promise<OperatorExemptionDecision>

export const makeOperatorExemptionGate = (
  deps: OperatorExemptionGateDeps,
): OperatorExemptionGate => {
  return async (accountRef: string, model: string) => {
    // GUARDRAIL: a premium model is never exempt — never touch the store.
    if (isPremiumModel(model)) {
      return decideOperatorExemption({ model, ownerExempt: false })
    }
    const identity = await deps.resolveOwnerIdentity(accountRef)
    const ownerKey = resolveOwnerKey(accountRef, identity)
    // An unclaimed account resolves to a synthetic `account:` key which the
    // grant surface refuses, so this read is always false for it (defense in
    // depth on top of the grant-time refusal).
    const ownerExempt = isVerifiedOwnerKey(ownerKey)
      ? await readOwnerExempt(deps.db, ownerKey)
      : false
    return decideOperatorExemption({ model, ownerExempt })
  }
}

// ----------------------------------------------------------------------------
// operator_credit metering wrapper (zero-debit, receipt-first, no referral)
// ----------------------------------------------------------------------------

// Public-safe `operator_credit` receipt ref for an exempt request. Resolvable
// without exposing any owner id, amount, destination, or payment material. The
// `operator_credit` infix marks the honest zero-debit accounting state.
export const operatorCreditReceiptRef = (requestId: string): string =>
  `receipt.inference.operator_credit.${requestId}`

// The zero-debit `operator_credit` outcome the decorator returns. `metered:
// false` (no credit was decremented) with a public-safe receipt ref so the
// record is HONEST about being a zero-debit operator_credit, not a silent skip.
const operatorCreditOutcome = (requestId: string): MeteringOutcome => ({
  metered: false,
  receiptRef: operatorCreditReceiptRef(requestId),
})

export type OperatorCreditDeps = Readonly<{
  db: D1Database
  resolveOwnerIdentity: VerifiedOwnerIdentityResolver
}>

/**
 * Wrap a metering hook so an EXEMPT verified owner's request for a NON-premium /
 * own-infra model is recorded as `operator_credit` (zero credit debit,
 * receipt-first, NO referral accrual) instead of decrementing credits.
 *
 * GUARDRAIL: a PREMIUM model (`claude` / `unknown`) ALWAYS falls through to the
 * inner hook (the normal ledger debit), even for an exempt owner — an exempt key
 * can never get a premium model for free. A non-exempt owner also falls through
 * to the inner hook (paid-Khala behavior intact). Any DB error falls through to
 * the inner hook (never grant a free request we could not account for, never fail
 * the inference call).
 *
 * Wired as the OUTERMOST metering wrapper alongside `withFreeAllowance`. It runs
 * AFTER dispatch; the route's `checkOperatorExemption` balance-gate seam is what
 * lets the zero-balance request reach dispatch in the first place. The two agree
 * because they share the SAME owner-identity resolver and the SAME guardrail.
 */
export const withOperatorCredit = (
  inner: MeteringHook,
  deps: OperatorCreditDeps,
): MeteringHook => {
  return (context: MeteringContext) =>
    Effect.gen(function* () {
      // GUARDRAIL: premium served models are NEVER operator_credit. They meter
      // normally (real debit), so an exempt key cannot get a premium model free.
      // Key on the SERVED model (the lane that actually incurred cost), matching
      // the ledger hook + free-allowance decorator.
      if (isPremiumModel(context.servedModel)) {
        return yield* inner(context)
      }

      const exempt = yield* Effect.tryPromise({
        catch: operatorExemptionPersistenceError,
        try: async () => {
          const identity = await deps.resolveOwnerIdentity(context.accountRef)
          const ownerKey = resolveOwnerKey(context.accountRef, identity)
          if (!isVerifiedOwnerKey(ownerKey)) {
            return { ownerExempt: false, ownerKey }
          }
          const ownerExempt = await readOwnerExempt(deps.db, ownerKey)
          return { ownerExempt, ownerKey }
        },
      }).pipe(
        Effect.catch(error =>
          Effect.gen(function* () {
            // Public-safe diagnostic only; never break the inference response.
            // On error meter normally (charge) rather than grant a free request
            // we could not account for.
            yield* Effect.logInfo(
              workerLogEntry('inference.operator_credit.error', {
                accountRef: context.accountRef,
                adapterId: context.adapterId,
                reason: error.message,
                requestId: context.requestId,
                servedModel: context.servedModel,
              }),
            )
            return { ownerExempt: false, ownerKey: null }
          }),
        ),
      )

      if (!exempt.ownerExempt) {
        // Not exempt (or error): meter normally (decrement + referral).
        return yield* inner(context)
      }

      // Exempt: record operator_credit (zero debit, receipt-first, NO referral)
      // and return WITHOUT calling the inner hook. Log a public-safe diagnostic
      // (refs + token counts only — never owner id value, prompt, or completion).
      yield* Effect.logInfo(
        workerLogEntry('inference.operator_credit.recorded', {
          accountRef: context.accountRef,
          adapterId: context.adapterId,
          requestId: context.requestId,
          servedModel: context.servedModel,
          totalTokens: context.usage.totalTokens,
        }),
      )
      return operatorCreditOutcome(context.requestId) satisfies MeteringOutcome
    })
}
