// Free-allowance accounting for the inference gateway (EPIC #5474, free-tier
// enablement §1/§3).
//
// This module owns the Sybil-resistant free pool that lets agents try the
// gateway WITHOUT spending credits, keyed to the VERIFIED OWNER-CLAIM identity
// rather than the per-Autopilot account. The shape is a `MeteringHook`
// decorator (`withFreeAllowance`) that wraps the live ledger hook (the single
// "a request was PAID" point):
//
//   - UNDER allowance + a FREE-ELIGIBLE model (Gemini Flash today): we EAT the
//     cost. The decorator accrues the priced charge against the owner's free
//     pool (idempotent per request) and returns WITHOUT calling the inner hook,
//     so NO credit decrement and NO referral accrual happen — the request is
//     genuinely free.
//   - OVER allowance, or a non-free-eligible model: the decorator falls through
//     to the inner hook (normal metering decrement + referral accrual).
//
// SYBIL RESISTANCE. The $10 free pool keys to the resolved owner identity
// (`owner:<ownerUserId>` for a verified X owner claim — the SAME surface the
// #5486 light-KYC gate reads via `readVerifiedPublicIdentityForAgentUserId`),
// so ALL accounts/autopilots under one verified owner share ONE pool. An
// UNCLAIMED account keys to a synthetic `account:<accountRef>` owner key and
// gets only a tiny taste (~$0.50) before it must claim — registering N
// autopilots does not multiply the pool, and an unclaimed swarm gets one taste
// each, not the full $10 each.
//
// IDEMPOTENCY. Each accrual writes one row in `inference_free_usage_events`
// keyed by the request id (UNIQUE), inside the same D1 batch that increments
// the per-owner tally. A retried/replayed settle for the SAME request hits the
// UNIQUE constraint, the batch aborts, and we treat it as already-accrued (no
// double-count) — exactly the discipline the credit ledger uses.
//
// EARNED ALLOWANCE. Beyond the base $10, additional free allowance accrues via
// RL-1 contribution (e.g. each referred signup adds $X to the owner's pool).
// `accrueEarnedAllowance` records one idempotent row per contribution ref and
// bumps `inference_earned_allowance.earned_free_usd_micros`; the effective cap
// = base cap + earned. This is a basic, tunable hook — the dispatcher that
// CALLS it on a qualifying RL-1 event is a thin follow-up; the accounting and
// idempotency live here.
//
// PUBLIC-SAFE. Every table this touches carries owner/account refs, model ids,
// USD-micros tallies, and request/accrual refs only — never prompts,
// completions, wallet/payment material, or secrets (migration 0210 header).
// The decorator never fails the customer's inference call: an accrual error is
// logged (public-safe) and the request falls through to normal metering rather
// than breaking.

import { Effect } from 'effect'

import type {
  InferenceEntitlementsGateReads,
  InferenceEntitlementsMirror,
} from '../inference-entitlements-store'
import { workerLogEntry } from '../observability'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  type MeteringContext,
  type MeteringHook,
  type MeteringOutcome,
} from './metering-hook'
import { priceRequest } from './pricing'
import {
  type VerifiedOwnerIdentityResolver,
  resolveOwnerKey,
} from './inference-owner-identity'

class FreeAllowancePersistenceError extends Error {
  readonly _tag = 'FreeAllowancePersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'FreeAllowancePersistenceError'
  }
}

const freeAllowancePersistenceError = (error: unknown) =>
  new FreeAllowancePersistenceError(error)

// ----------------------------------------------------------------------------
// Tunable constants (all free-tier thresholds in ONE place)
// ----------------------------------------------------------------------------

// USD micros per whole USD (1e-6 USD), the unit `inference_free_usage_tally`
// and `inference_earned_allowance` store, so sub-cent requests accrue precisely.
export const USD_MICROS_PER_USD = 1_000_000 as const

// Base free pool for a VERIFIED owner-claim identity: $10 (USD-equiv). All
// accounts/autopilots under one verified owner SHARE this single pool.
// !! TUNABLE: free-tier enablement §1.
export const VERIFIED_OWNER_FREE_CAP_USD_MICROS = 10 * USD_MICROS_PER_USD // $10.00

// Taste allowance for an UNCLAIMED / unverified account (keyed to a synthetic
// account owner key): ~$0.50 before the account must claim to unlock the full
// pool. !! TUNABLE: free-tier enablement §1 (Sybil resistance).
export const UNCLAIMED_TASTE_FREE_CAP_USD_MICROS = USD_MICROS_PER_USD / 2 // $0.50

// Earned allowance granted per qualifying RL-1 contribution event (e.g. a
// referred signup). ADDS to the owner's effective cap. !! TUNABLE: free-tier
// enablement §3 (earned accrual). Default $1.00 per referred signup.
export const EARNED_ALLOWANCE_PER_REFERRED_SIGNUP_USD_MICROS = USD_MICROS_PER_USD // $1.00

// Optional hard ceiling on TOTAL earned allowance per owner so the earned hook
// cannot grant an unbounded free pool. !! TUNABLE: 0 disables the ceiling.
// Default $100.00 of earned headroom.
export const EARNED_ALLOWANCE_CEILING_USD_MICROS = 100 * USD_MICROS_PER_USD // $100.00

// The exact model ids whose served usage is FREE-ELIGIBLE (we eat the cost
// under allowance). Only the first-party Gemini Flash taste lane today;
// OpenAgents-branded paid aliases such as Khala may share a backing lane without
// inheriting the free pool.
export const FREE_ELIGIBLE_MODEL_IDS: ReadonlyArray<string> = [
  'gemini',
  'gemini-3.5-flash',
]

export const isFreeEligibleModel = (model: string): boolean =>
  FREE_ELIGIBLE_MODEL_IDS.includes(model.trim().toLowerCase())

// ----------------------------------------------------------------------------
// Identity kind + effective cap
// ----------------------------------------------------------------------------

// Whether the owner key is a verified owner-claim identity or a bare unclaimed
// account. Drives which BASE cap applies. Mirrors `identity_kind` in the
// 0210 tally table.
export type FreeIdentityKind = 'verified' | 'unclaimed'

// The base free cap for an identity kind (before earned allowance).
export const baseFreeCapUsdMicros = (kind: FreeIdentityKind): number =>
  kind === 'verified'
    ? VERIFIED_OWNER_FREE_CAP_USD_MICROS
    : UNCLAIMED_TASTE_FREE_CAP_USD_MICROS

// ----------------------------------------------------------------------------
// Pure decision
// ----------------------------------------------------------------------------

// The state needed to decide whether a priced request is free. All amounts in
// USD micros.
export type FreeAllowanceState = Readonly<{
  identityKind: FreeIdentityKind
  // Cumulative free (we-eat-the-cost) usage already accrued for this owner.
  cumulativeFreeUsdMicros: number
  // Earned allowance (ADDS to the base cap).
  earnedFreeUsdMicros: number
}>

export type FreeAllowanceDecision = Readonly<{
  // True when the WHOLE priced charge fits under the remaining free allowance
  // (we eat it; no credit decrement). False => fall through to normal metering.
  free: boolean
  // The effective cap = base cap (by identity kind) + earned allowance.
  effectiveCapUsdMicros: number
  // Remaining free allowance BEFORE this request.
  remainingUsdMicros: number
  // The priced charge for this request (USD micros), the amount that would
  // accrue if `free`.
  chargeUsdMicros: number
}>

// Decide whether a priced request (chargeUsdMicros) is covered by the owner's
// remaining free allowance. Conservative: the ENTIRE charge must fit under the
// remaining allowance, so we never partially-free a request (no split between
// free and metered for one call — the next request that does not fit falls
// through to metering whole). A zero/negative charge is free by definition
// (an empty completion is never billed).
export const decideFreeAllowance = (
  input: Readonly<{
    state: FreeAllowanceState
    chargeUsdMicros: number
  }>,
): FreeAllowanceDecision => {
  const charge = Math.max(0, Math.trunc(input.chargeUsdMicros))
  const effectiveCap =
    baseFreeCapUsdMicros(input.state.identityKind) +
    Math.max(0, Math.trunc(input.state.earnedFreeUsdMicros))
  const used = Math.max(0, Math.trunc(input.state.cumulativeFreeUsdMicros))
  const remaining = Math.max(0, effectiveCap - used)
  return {
    chargeUsdMicros: charge,
    effectiveCapUsdMicros: effectiveCap,
    free: charge <= remaining,
    remainingUsdMicros: remaining,
  }
}

// Convert a USD charge (from `priceRequest`) into integer USD micros, rounding
// UP so a tiny nonzero charge still consumes at least 1 micro of allowance (a
// nonzero charge is never "free for accounting purposes"). A zero/negative/
// non-finite charge maps to 0.
export const usdToMicrosCeil = (chargeUsd: number): number => {
  if (!Number.isFinite(chargeUsd) || chargeUsd <= 0) return 0
  // Round away binary float dust before ceiling so an exact-micro charge is not
  // pushed up by 1 from a representation error.
  const FLOAT_DUST = 1e-9
  return Math.max(1, Math.ceil(chargeUsd * USD_MICROS_PER_USD - FLOAT_DUST))
}

// ----------------------------------------------------------------------------
// D1 reads + idempotent accrual
// ----------------------------------------------------------------------------

type FreeUsageTallyRow = Readonly<{
  owner_key: string
  identity_kind: string
  cumulative_free_usd_micros: number
  free_request_count: number
}>

type EarnedAllowanceRow = Readonly<{
  earned_free_usd_micros: number
}>

// Read the owner's current free-allowance state. A missing tally row means the
// owner has used $0 free so far; the identity kind comes from the resolved
// owner key (the caller passes it). Read-only and bounded.
const readFreeAllowanceState = async (
  db: D1Database,
  ownerKey: string,
  identityKind: FreeIdentityKind,
  // KS-8.9 (#8320): routed enforcement read (compare/postgres modes).
  // Absent => the untouched inline D1 reads below.
  gateReads?: Pick<InferenceEntitlementsGateReads, 'freeUsageState'>,
): Promise<FreeAllowanceState> => {
  if (gateReads !== undefined) {
    const state = await gateReads.freeUsageState(ownerKey)
    return {
      cumulativeFreeUsdMicros: state.cumulativeFreeUsdMicros,
      earnedFreeUsdMicros: state.earnedFreeUsdMicros,
      identityKind,
    }
  }
  const tally = await db
    .prepare(
      `SELECT owner_key, identity_kind, cumulative_free_usd_micros, free_request_count
         FROM inference_free_usage_tally
        WHERE owner_key = ?
        LIMIT 1`,
    )
    .bind(ownerKey)
    .first<FreeUsageTallyRow>()
  const earned = await db
    .prepare(
      `SELECT earned_free_usd_micros
         FROM inference_earned_allowance
        WHERE owner_key = ?
        LIMIT 1`,
    )
    .bind(ownerKey)
    .first<EarnedAllowanceRow>()
  return {
    cumulativeFreeUsdMicros:
      typeof tally?.cumulative_free_usd_micros === 'number'
        ? tally.cumulative_free_usd_micros
        : 0,
    earnedFreeUsdMicros:
      typeof earned?.earned_free_usd_micros === 'number'
        ? earned.earned_free_usd_micros
        : 0,
    identityKind,
  }
}

// ----------------------------------------------------------------------------
// Pre-flight free-allowance gate (balance-gate bypass)
// ----------------------------------------------------------------------------

// The decision the balance gate consults BEFORE rejecting a zero-balance
// account. `eligible` is true only for a free-eligible model AND a resolving
// owner who still has remaining free allowance — i.e. the request would be
// EATEN by `withFreeAllowance` after dispatch, so the read-only balance gate
// must not 402 it. `remainingUsdMicros` is the owner's pool headroom (for
// diagnostics); `ownerKey`/`identityKind` echo the resolution.
export type FreePreflightDecision = Readonly<{
  eligible: boolean
  remainingUsdMicros: number
  ownerKey: string | null
  identityKind: FreeIdentityKind | null
}>

// Deps for the pre-flight reader.
export type FreePreflightDeps = Readonly<{
  db: D1Database
  resolveOwnerIdentity: VerifiedOwnerIdentityResolver
  // KS-8.9 (#8320): routed enforcement read (compare/postgres modes).
  gateReads?: Pick<InferenceEntitlementsGateReads, 'freeUsageState'> | undefined
}>

// A reader that answers "can this (account, model) ride the free pool right
// now?" The Worker wires this into the chat-completions balance gate so a valid
// free-allowance request is NOT rejected with insufficient_credits before the
// metering hook (which owns the authoritative per-request accrual) ever runs.
//
// This is the read-only mirror of the gate inside `withFreeAllowance`: a
// free-eligible model whose owner has ANY remaining free allowance is eligible.
// It is intentionally COARSE (any remaining headroom => allow): the exact
// per-request charge is not known until the provider returns usage, and the
// metering hook does the precise decideFreeAllowance + idempotent accrual. The
// gate only needs to avoid a false 402 on the free path; an over-allowance
// request still falls through to a real charge in the hook. On any resolution
// error it returns `eligible: false` so the normal balance gate stands (we
// never grant a free bypass we could not account for).
export const checkFreeAllowancePreflight =
  (deps: FreePreflightDeps) =>
  async (accountRef: string, model: string): Promise<FreePreflightDecision> => {
    if (!isFreeEligibleModel(model)) {
      return {
        eligible: false,
        identityKind: null,
        ownerKey: null,
        remainingUsdMicros: 0,
      }
    }
    try {
      const identity = await deps.resolveOwnerIdentity(accountRef)
      const ownerKey = resolveOwnerKey(accountRef, identity)
      const identityKind: FreeIdentityKind =
        identity === undefined ? 'unclaimed' : 'verified'
      const state = await readFreeAllowanceState(
        deps.db,
        ownerKey,
        identityKind,
        deps.gateReads,
      )
      // A zero-charge probe (chargeUsdMicros = 0) yields the remaining headroom
      // without committing to a price; `remainingUsdMicros > 0` means the pool
      // can cover at least a minimal request.
      const decision = decideFreeAllowance({ chargeUsdMicros: 0, state })
      return {
        eligible: decision.remainingUsdMicros > 0,
        identityKind,
        ownerKey,
        remainingUsdMicros: decision.remainingUsdMicros,
      }
    } catch {
      // Resolution/read error: do NOT bypass the balance gate.
      return {
        eligible: false,
        identityKind: null,
        ownerKey: null,
        remainingUsdMicros: 0,
      }
    }
  }

// Accrue a free-usage charge against the owner's pool, idempotently. Writes the
// per-request event row (UNIQUE request_id) and increments the cumulative tally
// in ONE D1 batch. On a duplicate request id the UNIQUE constraint aborts the
// batch and we treat it as already-accrued (no double-count). Returns whether a
// NEW accrual was recorded.
const accrueFreeUsage = async (
  db: D1Database,
  input: Readonly<{
    ownerKey: string
    identityKind: FreeIdentityKind
    accountRef: string
    requestId: string
    servedModel: string
    chargeUsdMicros: number
    nowIso: string
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror. EVENT-KEYED
  // (request_id) so the Postgres tally can never double-count.
  mirror?: InferenceEntitlementsMirror | undefined,
): Promise<boolean> => {
  try {
    await db.batch([
      db
        .prepare(
          `INSERT INTO inference_free_usage_events
             (request_id, owner_key, account_ref, served_model, free_usd_micros, created_at)
           VALUES (?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.requestId,
          input.ownerKey,
          input.accountRef,
          input.servedModel,
          input.chargeUsdMicros,
          input.nowIso,
        ),
      db
        .prepare(
          `INSERT INTO inference_free_usage_tally
             (owner_key, identity_kind, cumulative_free_usd_micros, free_request_count, created_at, updated_at)
           VALUES (?, ?, ?, 1, ?, ?)
           ON CONFLICT(owner_key) DO UPDATE SET
             cumulative_free_usd_micros = cumulative_free_usd_micros + excluded.cumulative_free_usd_micros,
             free_request_count = free_request_count + 1,
             identity_kind = excluded.identity_kind,
             updated_at = excluded.updated_at`,
        )
        .bind(
          input.ownerKey,
          input.identityKind,
          input.chargeUsdMicros,
          input.nowIso,
          input.nowIso,
        ),
    ])
    mirror?.([
      {
        event: {
          accountRef: input.accountRef,
          createdAt: input.nowIso,
          freeUsdMicros: input.chargeUsdMicros,
          ownerKey: input.ownerKey,
          requestId: input.requestId,
          servedModel: input.servedModel,
        },
        identityKind: input.identityKind,
        kind: 'accrue_free_usage',
      },
    ])
    return true
  } catch {
    // Duplicate request id (UNIQUE on inference_free_usage_events.request_id) =>
    // this request already accrued. Idempotent no-op; never double-count.
    return false
  }
}

// ----------------------------------------------------------------------------
// Earned-allowance accrual hook (free-tier enablement §3)
// ----------------------------------------------------------------------------

// A qualifying RL-1 contribution that earns extra free allowance. Today only
// referred signups; the kind is a bounded enum so adding a source is a tabled
// change, not an intent parser.
export type EarnedAllowanceKind = 'referred_signup'

// Earned amount (USD micros) for a contribution kind. Tabled + tunable.
export const earnedAllowanceForKind = (kind: EarnedAllowanceKind): number => {
  switch (kind) {
    case 'referred_signup':
      return EARNED_ALLOWANCE_PER_REFERRED_SIGNUP_USD_MICROS
  }
}

// Stable accrual-event ref for an earned-allowance contribution, the UNIQUE
// idempotency guard (one bonus per contribution). Neutral, no payment material.
export const earnedAllowanceEventRef = (
  kind: EarnedAllowanceKind,
  sourceRef: string,
): string => `${kind}:${sourceRef}`

// Record an earned-allowance accrual for an owner, idempotently. Writes the
// per-event row (UNIQUE accrual_event_ref) and bumps the cumulative earned
// total in ONE batch, capped at EARNED_ALLOWANCE_CEILING_USD_MICROS. A repeated
// contribution ref hits the UNIQUE constraint and is a no-op. Returns whether a
// NEW accrual was recorded.
export const accrueEarnedAllowance = (
  db: D1Database,
  input: Readonly<{
    ownerKey: string
    kind: EarnedAllowanceKind
    sourceRef: string
    nowIso?: (() => string) | undefined
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror. EVENT-KEYED
  // (accrual_event_ref) so the Postgres tally can never double-count.
  mirror?: InferenceEntitlementsMirror | undefined,
): Effect.Effect<boolean> =>
  Effect.gen(function* () {
    const nowIso = (input.nowIso ?? currentIsoTimestamp)()
    const eventRef = earnedAllowanceEventRef(input.kind, input.sourceRef)
    const amount = earnedAllowanceForKind(input.kind)
    const recorded = yield* Effect.tryPromise({
      catch: freeAllowancePersistenceError,
      try: async () => {
        // Read current earned total so the ceiling caps the new accrual without
        // a non-portable SQL MIN-on-update expression.
        const current = await db
          .prepare(
            `SELECT earned_free_usd_micros FROM inference_earned_allowance WHERE owner_key = ? LIMIT 1`,
          )
          .bind(input.ownerKey)
          .first<EarnedAllowanceRow>()
        const existing =
          typeof current?.earned_free_usd_micros === 'number'
            ? current.earned_free_usd_micros
            : 0
        const headroom =
          EARNED_ALLOWANCE_CEILING_USD_MICROS <= 0
            ? amount
            : Math.max(0, EARNED_ALLOWANCE_CEILING_USD_MICROS - existing)
        const grant = Math.min(amount, headroom)
        await db.batch([
          db
            .prepare(
              `INSERT INTO inference_earned_allowance_events
                 (accrual_event_ref, owner_key, accrual_kind, earned_usd_micros, created_at)
               VALUES (?, ?, ?, ?, ?)`,
            )
            .bind(eventRef, input.ownerKey, input.kind, grant, nowIso),
          db
            .prepare(
              `INSERT INTO inference_earned_allowance
                 (owner_key, earned_free_usd_micros, accrual_count, created_at, updated_at)
               VALUES (?, ?, 1, ?, ?)
               ON CONFLICT(owner_key) DO UPDATE SET
                 earned_free_usd_micros = earned_free_usd_micros + excluded.earned_free_usd_micros,
                 accrual_count = accrual_count + 1,
                 updated_at = excluded.updated_at`,
            )
            .bind(input.ownerKey, grant, nowIso, nowIso),
        ])
        mirror?.([
          {
            event: {
              accrualEventRef: eventRef,
              accrualKind: input.kind,
              createdAt: nowIso,
              earnedUsdMicros: grant,
              ownerKey: input.ownerKey,
            },
            kind: 'accrue_earned_allowance',
          },
        ])
        return true
      },
    }).pipe(Effect.catch(() => Effect.succeed(false)))
    return recorded
  })

// ----------------------------------------------------------------------------
// The metering-hook decorator
// ----------------------------------------------------------------------------

export type FreeAllowanceDeps = Readonly<{
  db: D1Database
  // Resolves the verified owner identity for an account ref (reuses the
  // owner-claim surface). The decorator maps the result to an owner key.
  resolveOwnerIdentity: VerifiedOwnerIdentityResolver
  nowIso?: (() => string) | undefined
  // KS-8.9 (#8320): routed enforcement reads + fire-safe dual-write
  // mirror. Absent => untouched D1-only behavior.
  gateReads?: Pick<InferenceEntitlementsGateReads, 'freeUsageState'> | undefined
  mirror?: InferenceEntitlementsMirror | undefined
}>

// The free outcome the decorator returns when it eats the cost. `metered:
// false` (no charge was metered against credits) but we tag it free with a
// public-safe receipt ref. `zeroCharge` stays absent here so this is
// distinguishable from a metered-but-$0 ledger outcome.
const freeOutcome = (requestId: string): MeteringOutcome => ({
  metered: false,
  receiptRef: `receipt.inference.free.${requestId}`,
})

/**
 * Wrap a live metering hook with the Sybil-resistant free-allowance gate.
 *
 * For a FREE-ELIGIBLE model (Gemini Flash) whose priced charge fits under the
 * resolving owner's remaining free pool, the decorator EATS the cost: it
 * accrues the charge against the per-owner pool (idempotent per request) and
 * returns WITHOUT calling the inner hook — no credit decrement, no referral
 * accrual. Otherwise (over allowance, non-free model, unresolved owner on an
 * accrual error) it falls through to the inner hook unchanged.
 *
 * The customer's inference call is never failed by free-allowance bookkeeping:
 * any DB error logs a public-safe diagnostic and falls through to normal
 * metering.
 */
export const withFreeAllowance = (
  inner: MeteringHook,
  deps: FreeAllowanceDeps,
): MeteringHook => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  return (context: MeteringContext) =>
    Effect.gen(function* () {
      // Only the free-eligible lane is ever free; everything else meters.
      if (!isFreeEligibleModel(context.servedModel)) {
        return yield* inner(context)
      }

      // Price the request (USD-equiv) from the REAL provider usage, keyed on the
      // served model — the same engine + funding kind the ledger hook uses. The
      // free pool is denominated in the customer SELL price (grossChargeUsd, no
      // Bitcoin discount) so the pool is consumed consistently regardless of
      // funding kind.
      const priced = priceRequest({
        batch: context.batch ?? false,
        fundingKind: context.fundingKind,
        model: context.servedModel,
        usage: context.usage,
      })
      const chargeUsdMicros = usdToMicrosCeil(priced.grossChargeUsd)

      const gated = yield* Effect.tryPromise({
        catch: freeAllowancePersistenceError,
        try: async () => {
          const identity = await deps.resolveOwnerIdentity(context.accountRef)
          const ownerKey = resolveOwnerKey(context.accountRef, identity)
          const identityKind: FreeIdentityKind =
            identity === undefined ? 'unclaimed' : 'verified'
          const state = await readFreeAllowanceState(
            deps.db,
            ownerKey,
            identityKind,
            deps.gateReads,
          )
          const decision = decideFreeAllowance({ chargeUsdMicros, state })
          if (!decision.free) {
            return { accrued: false, free: false as const, ownerKey }
          }
          const accrued = await accrueFreeUsage(
            deps.db,
            {
              accountRef: context.accountRef,
              chargeUsdMicros,
              identityKind,
              nowIso: nowIso(),
              ownerKey,
              requestId: context.requestId,
              servedModel: context.servedModel,
            },
            deps.mirror,
          )
          return { accrued, free: true as const, ownerKey }
        },
      }).pipe(
        Effect.catch(error =>
          Effect.gen(function* () {
            // Public-safe diagnostic only; never break the inference response.
            // On error we fall through to normal metering (charge the account)
            // rather than risk granting free usage we could not account for.
            yield* Effect.logInfo(
              workerLogEntry('inference.free_allowance.error', {
                accountRef: context.accountRef,
                adapterId: context.adapterId,
                reason: error.message,
                requestId: context.requestId,
                servedModel: context.servedModel,
              }),
            )
            return { accrued: false, free: false as const, ownerKey: null }
          }),
        ),
      )

      if (!gated.free) {
        // Over allowance (or error): meter normally (decrement + referral).
        return yield* inner(context)
      }

      // Under allowance: we ate the cost. Log a public-safe diagnostic and
      // return WITHOUT calling the inner hook (no decrement, no referral).
      yield* Effect.logInfo(
        workerLogEntry('inference.free_allowance.granted', {
          accountRef: context.accountRef,
          accrued: gated.accrued,
          adapterId: context.adapterId,
          chargeUsdMicros,
          ownerKey: gated.ownerKey,
          requestId: context.requestId,
          servedModel: context.servedModel,
        }),
      )
      return freeOutcome(context.requestId) satisfies MeteringOutcome
    })
}
