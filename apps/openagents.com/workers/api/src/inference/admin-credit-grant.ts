// Manual owner credit grant (AIUR-2, #8500, epic #8467).
//
// THE GAP this closes: the first Khala Code mobile MVP build ships WITHOUT
// RevenueCat/IAP (#8481 postponed) — credits are assigned manually by the
// owner through the Aiur admin panel (apps/aiur/) instead. This is the
// server-side grant primitive Aiur's `/api/admin/credits/grant` route calls.
//
// REUSES THE EXISTING BRIDGE PRIMITIVE, exactly like the $10 GitHub-signup
// grant (github-signup-credit-grant.ts) — this module is a near-exact copy of
// that one's shape, with two differences: (1) the idempotency key is
// CALLER-SUPPLIED (the Aiur UI mints one per grant *attempt*, resent
// unchanged on retry) rather than derived from a GitHub account id, since an
// admin may legitimately grant credit to the SAME user more than once; (2)
// there is no account-age/IP-mint anti-abuse floor, since the caller is
// already owner-gated (Aiur's `AIUR_OWNER_USER_IDS` allowlist, enforced
// before this module is ever reached) rather than a public self-serve signup
// path.
//
// RL-3 ASSET BOUNDARY: tagged `revenueAsset: 'free'` (never purchased), same
// as the signup grant — inference-spendable, never Bitcoin-withdrawable.
//
// IDEMPOTENCY: `grantRef` is required from the caller and becomes BOTH the
// `pay_ins` UNIQUE idempotency key (via `usdCreditGrantIdempotencyKey`) AND
// the UNIQUE primary key of `admin_credit_grants` — a retried/raced call with
// the SAME grantRef is guaranteed to be a no-op on either surface.
//
// CLAWBACK: `clawbackAdminCreditGrant` below wraps the existing generic
// `clawbackInferenceCredits` (inference-abuse-controls.ts), which already
// debits `agent_balances.balance_msat` for any `accountRef` regardless of how
// it was funded — no special-casing needed.
//
// QUERYABLE PER USER (for the Aiur credits console's balance/history view):
// every grant is a row in `admin_credit_grants` keyed by `user_id`.
// `readAdminCreditGrantsForUser` is the reader.

import { Effect, Schema as S } from 'effect'

import type { BillingDomainMirror } from '../billing'
import {
  type AssetBoundaryAsset,
  validateAssetBoundary,
} from '../asset-bitcoin-boundary'
import {
  clawbackInferenceCredits,
  inferenceClawbackIdempotencyKey,
  type ClawbackOutcome,
} from './inference-abuse-controls'
import { workerLogEntry } from '../observability'
import { type LedgerStatement, runLedgerStatements } from '../payments-ledger'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  agentRefForUser,
  usdCreditGrantIdempotencyKey,
  usdCreditGrantReceiptRef,
  usdCreditGrantStatements,
} from './usd-credit-bridge'
import { usdCentsToMsatFloor } from './usd-msat-conversion'

// Issue #8505 (Part 2): fail-soft, best-effort per-user credit-balance
// projection into Khala Sync (`scope.user.<userId>`) — same seam as the
// inference/cloud metering hooks and the signup-grant path. Called AFTER a
// FRESH grant/clawback commits, with the SAME idempotency key the D1 write
// used. Optional; a deployment without the Khala Sync binding (or a test)
// grants/claws back exactly as before.
export type CreditBalanceProjectionRecorder = (
  event: Readonly<{
    accountRef: string
    idempotencyKey: string
    deltaUsdCents: number
    observedAt: string
  }>,
) => Promise<void>

// ----------------------------------------------------------------------------
// Stable refs
// ----------------------------------------------------------------------------

// The caller (Aiur UI) supplies the distinguishing suffix (e.g. a client-
// generated UUID minted once per grant attempt); this just namespaces it so
// an admin grant ref can never collide with a signup-grant or business-
// starter-credit ref sharing the same underlying `pay_ins.idempotency_key`
// space.
export const adminCreditGrantRef = (idempotencyKey: string): string =>
  `admin:credit-grant:${idempotencyKey}`

// ----------------------------------------------------------------------------
// D1 reads/writes
// ----------------------------------------------------------------------------

type ExistingAdminGrantRow = Readonly<{
  grant_ref: string
  amount_usd_cents: number
  amount_msat: number
  credit_receipt_ref: string
  reason: string
  granted_by_user_id: string
  created_at: string
}>

export const readAdminCreditGrant = async (
  db: D1Database,
  grantRef: string,
): Promise<ExistingAdminGrantRow | null> => {
  const row = await db
    .prepare(
      `SELECT grant_ref, amount_usd_cents, amount_msat, credit_receipt_ref,
              reason, granted_by_user_id, created_at
         FROM admin_credit_grants
        WHERE grant_ref = ?
        LIMIT 1`,
    )
    .bind(grantRef)
    .first<ExistingAdminGrantRow>()
  return row ?? null
}

export type AdminCreditGrantRecord = Readonly<{
  grantRef: string
  userId: string
  amountUsdCents: number
  amountMsat: number
  reason: string
  grantedByUserId: string
  creditReceiptRef: string
  createdAt: string
}>

type AdminGrantHistoryRow = Readonly<{
  grant_ref: string
  user_id: string
  amount_usd_cents: number
  amount_msat: number
  reason: string
  granted_by_user_id: string
  credit_receipt_ref: string
  created_at: string
}>

// Per-user grant history (the Aiur credits console's balance/history view
// and recent-grants ledger view). Newest first.
export const readAdminCreditGrantsForUser = async (
  db: D1Database,
  userId: string,
): Promise<ReadonlyArray<AdminCreditGrantRecord>> => {
  const result = await db
    .prepare(
      `SELECT grant_ref, user_id, amount_usd_cents, amount_msat, reason,
              granted_by_user_id, credit_receipt_ref, created_at
         FROM admin_credit_grants
        WHERE user_id = ?
        ORDER BY created_at DESC`,
    )
    .bind(userId)
    .all<AdminGrantHistoryRow>()
  return result.results.map(row => ({
    amountMsat: row.amount_msat,
    amountUsdCents: row.amount_usd_cents,
    createdAt: row.created_at,
    creditReceiptRef: row.credit_receipt_ref,
    grantedByUserId: row.granted_by_user_id,
    grantRef: row.grant_ref,
    reason: row.reason,
    userId: row.user_id,
  }))
}

// Recent admin grants across ALL users (the Aiur credits console's
// "recent-grants ledger view"). Newest first, capped.
export const readRecentAdminCreditGrants = async (
  db: D1Database,
  limit: number,
): Promise<ReadonlyArray<AdminCreditGrantRecord>> => {
  const boundedLimit = Math.max(1, Math.min(200, Math.trunc(limit)))
  const result = await db
    .prepare(
      `SELECT grant_ref, user_id, amount_usd_cents, amount_msat, reason,
              granted_by_user_id, credit_receipt_ref, created_at
         FROM admin_credit_grants
        ORDER BY created_at DESC
        LIMIT ?`,
    )
    .bind(boundedLimit)
    .all<AdminGrantHistoryRow>()
  return result.results.map(row => ({
    amountMsat: row.amount_msat,
    amountUsdCents: row.amount_usd_cents,
    createdAt: row.created_at,
    creditReceiptRef: row.credit_receipt_ref,
    grantedByUserId: row.granted_by_user_id,
    grantRef: row.grant_ref,
    reason: row.reason,
    userId: row.user_id,
  }))
}

// The metadata-table insert, folded into the SAME atomic batch as the ledger
// statements so the UNIQUE grant_ref primary key is a second, independent
// idempotency guard alongside the pay_ins UNIQUE idempotency key.
const adminCreditGrantMetadataStatement = (
  input: Readonly<{
    grantRef: string
    userId: string
    accountRef: string
    amountUsdCents: number
    amountMsat: number
    reason: string
    grantedByUserId: string
    creditReceiptRef: string
  }>,
  nowIso: string,
): LedgerStatement => ({
  params: [
    input.grantRef,
    input.userId,
    input.accountRef,
    input.amountUsdCents,
    input.amountMsat,
    input.reason,
    input.grantedByUserId,
    input.creditReceiptRef,
    nowIso,
  ],
  sql: `INSERT OR IGNORE INTO admin_credit_grants
          (grant_ref, user_id, account_ref, amount_usd_cents, amount_msat,
           reason, granted_by_user_id, credit_receipt_ref, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
})

// ----------------------------------------------------------------------------
// The grant action
// ----------------------------------------------------------------------------

export class AdminCreditGrantError extends S.TaggedErrorClass<AdminCreditGrantError>()(
  'AdminCreditGrantError',
  { cause: S.Defect },
) {}

export type AdminCreditGrantOutcome =
  | Readonly<{
      ok: true
      alreadyGranted: boolean
      grantRef: string
      grantedCents: number
      grantedMsat: number
      receiptRef: string
    }>
  | Readonly<{
      ok: false
      reason: 'amount_invalid'
      message: string
    }>
  | Readonly<{
      ok: false
      reason: 'reason_required'
      message: string
    }>
  | Readonly<{
      ok: false
      reason: 'asset_boundary_violation'
      message: string
      reasonRef?: string
    }>

export type AdminCreditGrantDeps = Readonly<{
  db: D1Database
  nowIso?: (() => string) | undefined
  mirror?: BillingDomainMirror | undefined
  recordCreditBalanceProjection?: CreditBalanceProjectionRecorder | undefined
}>

// Grant credit to a user, idempotent forever on the caller-supplied
// `grantRef`. The Aiur UI mints one grantRef per grant *attempt* (e.g. a
// client-generated UUID created when the confirm button is pressed) and
// resends the SAME value on retry, so a double-click or a retried network
// request can never double-grant.
export const grantAdminCredit = (
  input: Readonly<{
    userId: string
    grantRef: string
    amountUsdCents: number
    reason: string
    grantedByUserId: string
  }>,
  deps: AdminCreditGrantDeps,
): Effect.Effect<AdminCreditGrantOutcome, AdminCreditGrantError> =>
  Effect.gen(function* () {
    const nowIso = (deps.nowIso ?? currentIsoTimestamp)()

    const existing = yield* Effect.tryPromise({
      catch: (cause: unknown) => new AdminCreditGrantError({ cause }),
      try: () => readAdminCreditGrant(deps.db, input.grantRef),
    })
    if (existing !== null) {
      return {
        alreadyGranted: true,
        grantedCents: existing.amount_usd_cents,
        grantedMsat: existing.amount_msat,
        grantRef: existing.grant_ref,
        ok: true,
        receiptRef: existing.credit_receipt_ref,
      }
    }

    const reason = input.reason.trim()
    if (reason.length === 0) {
      return {
        message: 'A reason is required for every admin credit grant.',
        ok: false,
        reason: 'reason_required',
      }
    }

    if (!Number.isFinite(input.amountUsdCents) || input.amountUsdCents <= 0) {
      return {
        message: 'amountUsdCents must be a positive finite number.',
        ok: false,
        reason: 'amount_invalid',
      }
    }

    // RL-3: this grant must never create a withdrawable Bitcoin liability.
    // Tagged 'free' (owner-minted, not fiat-purchased).
    const revenueAsset: AssetBoundaryAsset = 'free'
    const violation = validateAssetBoundary({
      contributorAsset: revenueAsset,
      movement: 'spend',
      revenueAsset,
    })
    if (violation !== null) {
      yield* Effect.logInfo(
        workerLogEntry('inference.admin_credit_grant.boundary_denied', {
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

    const accountRef = agentRefForUser(input.userId)
    const contextRef = `admin-credit-grant:${input.userId}`
    const grantMsat = usdCentsToMsatFloor(input.amountUsdCents)

    if (grantMsat <= 0) {
      return {
        message: 'amountUsdCents converted to a zero msat grant; refusing.',
        ok: false,
        reason: 'amount_invalid',
      }
    }

    const creditReceiptRef = usdCreditGrantReceiptRef(input.grantRef)

    yield* Effect.tryPromise({
      catch: (cause: unknown) => new AdminCreditGrantError({ cause }),
      try: () =>
        runLedgerStatements(
          deps.db,
          [
            ...usdCreditGrantStatements(
              { accountRef, contextRef, grantMsat, grantRef: input.grantRef },
              nowIso,
            ),
            adminCreditGrantMetadataStatement(
              {
                accountRef,
                amountMsat: grantMsat,
                amountUsdCents: input.amountUsdCents,
                creditReceiptRef,
                grantedByUserId: input.grantedByUserId,
                grantRef: input.grantRef,
                reason,
                userId: input.userId,
              },
              nowIso,
            ),
          ],
          deps.mirror,
        ),
    })

    yield* Effect.logInfo(
      workerLogEntry('inference.admin_credit_grant.granted', {
        grantedCents: input.amountUsdCents,
        grantedMsat: grantMsat,
        grantedByUserId: input.grantedByUserId,
        grantRef: input.grantRef,
        userId: input.userId,
      }),
    )

    // Issue #8505 (Part 2): best-effort live projection of the FRESH admin
    // grant into scope.user.<userId>, reusing the SAME idempotency key the
    // D1 grant just used. Fail-soft by contract and never blocks/reverses
    // the D1 grant above, which already committed.
    if (deps.recordCreditBalanceProjection !== undefined) {
      yield* Effect.promise(() =>
        deps
          .recordCreditBalanceProjection!({
            accountRef,
            deltaUsdCents: Math.trunc(input.amountUsdCents),
            idempotencyKey: usdCreditGrantIdempotencyKey(input.grantRef),
            observedAt: nowIso,
          })
          .catch(() => undefined),
      )
    }

    return {
      alreadyGranted: false,
      grantedCents: input.amountUsdCents,
      grantedMsat: grantMsat,
      grantRef: input.grantRef,
      ok: true,
      receiptRef: creditReceiptRef,
    }
  })

// ----------------------------------------------------------------------------
// Clawback (wraps the existing generic primitive — no special-casing needed)
// ----------------------------------------------------------------------------

export const adminCreditClawbackSourceRef = (clawbackRef: string): string =>
  `admin:credit-clawback:${clawbackRef}`

export type AdminCreditClawbackDeps = Readonly<{
  db: D1Database
  nowIso?: () => string
  mirror?: BillingDomainMirror | undefined
  recordCreditBalanceProjection?: CreditBalanceProjectionRecorder | undefined
}>

// Claw back credit from a user, idempotent on the caller-supplied
// `clawbackRef` (mint one per clawback attempt, same discipline as grants).
// Reuses `clawbackInferenceCredits` directly — it is funding-source-agnostic
// and already guards against going balance-negative via the
// `CHECK (balance_msat >= 0)` constraint.
export const clawbackAdminCredit = (
  input: Readonly<{
    userId: string
    clawbackRef: string
    amountUsdCents: number
    reason: string
  }>,
  deps: AdminCreditClawbackDeps,
): Effect.Effect<ClawbackOutcome> =>
  Effect.gen(function* () {
    const accountRef = agentRefForUser(input.userId)
    const sourceRef = adminCreditClawbackSourceRef(input.clawbackRef)
    const outcome = yield* clawbackInferenceCredits(
      {
        accountRef,
        clawbackMsat: usdCentsToMsatFloor(input.amountUsdCents),
        contextRef: `admin-credit-clawback:${input.userId}:${input.reason.trim().slice(0, 200)}`,
        sourceRef,
      },
      { db: deps.db, mirror: deps.mirror, ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }) },
    )

    // Issue #8505 (Part 2): best-effort live projection of the clawback into
    // scope.user.<userId>, reusing the SAME idempotency key the D1 clawback
    // used (`inferenceClawbackIdempotencyKey`). `clawedBack` covers BOTH a
    // fresh clawback AND an idempotent replay — safe either way, since the
    // projection's own exact-once guard (keyed on the SAME idempotency key)
    // makes a replayed call here a no-op too. Never called when the balance
    // CHECK aborted the decrement (`insufficientBalance`) — nothing to
    // project for a clawback that did not happen.
    if (outcome.clawedBack && deps.recordCreditBalanceProjection !== undefined) {
      const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
      yield* Effect.promise(() =>
        deps
          .recordCreditBalanceProjection!({
            accountRef,
            deltaUsdCents: -Math.trunc(input.amountUsdCents),
            idempotencyKey: inferenceClawbackIdempotencyKey(sourceRef),
            observedAt: nowIso,
          })
          .catch(() => undefined),
      )
    }

    return outcome
  })
