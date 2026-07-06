import {
  type ChangelogEntry,
  type CreditBalanceEntity,
  CREDIT_BALANCE_ENTITY_TYPE,
  decodeCreditBalanceEntity,
  encodeCreditBalanceEntity,
  EntityId,
  EntityType,
  personalScope,
} from "@openagentsinc/khala-sync"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

/**
 * Per-user credit-balance scope projection (issue #8505, Part 2). Modeled
 * directly on `public-counter-projection.ts` — read that module's header
 * first; this doc only calls out where the per-user shape differs.
 *
 * AUTHORITY: unlike the public tokens-served counter (which only ever
 * increments), a user's balance moves in BOTH directions (a grant is
 * positive, a charge/clawback is negative) — see
 * docs/khala-code/2026-07-06-credits-ledger-vs-khala-sync-architecture-audit.md.
 * The D1 `agent_balances` ledger write ALWAYS happens first and is ALWAYS
 * authoritative; this module only mirrors the resulting delta into
 * `scope.user.<userId>` so the mobile balance chip can update live. A lost or
 * failed projection write never blocks, retries, or reverses the real D1
 * charge/grant — it is purely a fail-soft, best-effort copy.
 *
 * EXACT-ONCE PER SOURCE ROW: every delta is keyed by the SAME idempotency key
 * the source D1 ledger write already carries
 * (`inference:charge:<requestId>`, `<primitive>:charge:<chargeId>`,
 * `signup:github:<githubUserId>`, an admin grant/clawback ref) through a
 * guard insert into `khala_sync_user_credit_balance_applied` in the SAME
 * transaction as the balance UPDATE and the changelog append — a replayed
 * event is a no-op.
 *
 * BRING-UP / NOT-INITIALIZED REFUSAL: the delta path only ever UPDATEs an
 * existing row. Before the admin backfill (`repairUserCreditBalance` with
 * source `backfill`, which sets the row to the user's exact current D1
 * balance) the row does not exist and deltas are refused with
 * `credit_balance_not_initialized` — a fresh deploy can never serve a
 * fabricated zero balance for a real user.
 */

// ---------------------------------------------------------------------------
// Named system writers (SPEC §7 invariant 3)
// ---------------------------------------------------------------------------

export const USER_CREDIT_BALANCE_PROJECTION_SYSTEM_REF =
  "system:user_credit_balance_projection.agent_balances_ledger.v1"

export const USER_CREDIT_BALANCE_REPAIR_SYSTEM_REF =
  "system:user_credit_balance_repair.v1"

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/**
 * The user's balance row does not exist yet (pre-backfill). The delta
 * refuses (and its transaction — including the idempotency guard row — rolls
 * back) rather than inventing a partial balance.
 */
export class UserCreditBalanceNotInitializedError extends Error {
  readonly _tag = "UserCreditBalanceNotInitializedError"
  override readonly name = "UserCreditBalanceNotInitializedError"
  constructor(readonly userId: string) {
    super(
      `user credit balance for ${userId} is not initialized — run the ` +
        "admin backfill (repair with source 'backfill') before deltas apply",
    )
  }
}

export class UserCreditBalanceInvalidInputError extends Error {
  readonly _tag = "UserCreditBalanceInvalidInputError"
  override readonly name = "UserCreditBalanceInvalidInputError"
  constructor(messageSafe: string) {
    super(messageSafe)
  }
}

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

export interface UserCreditBalanceRow {
  readonly userId: string
  readonly balanceUsdCents: number
  readonly lastEventAt: string | null
}

const toIsoOrNull = (raw: Date | string | null): string | null =>
  raw === null
    ? null
    : raw instanceof Date
      ? raw.toISOString()
      : new Date(raw).toISOString()

const toSafeBalance = (raw: string | number | bigint): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new UserCreditBalanceInvalidInputError(
      `user credit balance out of safe range: ${String(raw)}`,
    )
  }
  return value
}

// ---------------------------------------------------------------------------
// Reads (single statement — Hyperdrive transaction-mode safe)
// ---------------------------------------------------------------------------

/** Read one user's projected credit balance row, or null when it is not
 * initialized (pre-backfill). */
export const readUserCreditBalance = async (
  sql: SqlTag,
  userId: string,
): Promise<UserCreditBalanceRow | null> => {
  const rows: Array<{
    balance_usd_cents: string | number | bigint
    last_event_at: Date | string | null
  }> = await sql`
    SELECT balance_usd_cents, last_event_at
      FROM khala_sync_user_credit_balances
     WHERE user_id = ${userId}
  `
  const row = rows[0]
  if (row === undefined) return null
  return {
    balanceUsdCents: toSafeBalance(row.balance_usd_cents),
    lastEventAt: toIsoOrNull(row.last_event_at),
    userId,
  }
}

// ---------------------------------------------------------------------------
// Delta (exact-once by source idempotency key; signed — grants are
// positive, charges/clawbacks are negative)
// ---------------------------------------------------------------------------

export interface UserCreditBalanceDeltaInput {
  readonly userId: string
  /** The SOURCE D1 ledger write's idempotency key (exact-once per event). */
  readonly idempotencyKey: string
  /** Signed non-zero integer USD-cents delta for this one source event. */
  readonly deltaUsdCents: number
  /** The source event's observed-at ISO timestamp. */
  readonly observedAt: string
}

export type UserCreditBalanceDeltaApplied = Readonly<{
  applied: true
  balance: UserCreditBalanceRow
  entry: ChangelogEntry
}>

export type UserCreditBalanceDeltaResult =
  | UserCreditBalanceDeltaApplied
  | Readonly<{ applied: false; reason: "duplicate_idempotency_key" }>

const validateDeltaInput = (input: UserCreditBalanceDeltaInput): void => {
  if (input.userId.trim().length === 0) {
    throw new UserCreditBalanceInvalidInputError(
      "user credit balance deltas require a non-empty userId",
    )
  }
  if (input.idempotencyKey.trim().length === 0) {
    throw new UserCreditBalanceInvalidInputError(
      "user credit balance deltas require a non-empty idempotency key",
    )
  }
  if (!Number.isSafeInteger(input.deltaUsdCents) || input.deltaUsdCents === 0) {
    throw new UserCreditBalanceInvalidInputError(
      "user credit balance deltas require a non-zero safe-integer deltaUsdCents",
    )
  }
  if (!Number.isFinite(Date.parse(input.observedAt))) {
    throw new UserCreditBalanceInvalidInputError(
      "user credit balance deltas require a parseable observedAt timestamp",
    )
  }
}

const balancePostImage = (balance: UserCreditBalanceRow): unknown =>
  encodeCreditBalanceEntity(
    decodeCreditBalanceEntity({
      balanceUsdCents: balance.balanceUsdCents,
      lastEventAt: balance.lastEventAt,
      userId: balance.userId,
    } satisfies Record<keyof CreditBalanceEntity, unknown>),
  )

/**
 * Apply one exact-once signed balance delta + changelog append in ONE
 * Postgres transaction. Throws on storage failure, invalid input, or an
 * uninitialized balance row; a replayed idempotency key resolves
 * `{ applied: false }` without touching the balance. The
 * `CHECK (balance_usd_cents >= 0)` constraint aborts (and rolls back) a
 * delta that would drive the projection negative — a real production
 * possibility ONLY under projection drift (the D1 ledger itself can never go
 * negative, per its own CHECK), and exactly what the reconcile/repair path
 * below exists to realign.
 */
export const applyUserCreditBalanceDelta = async (
  sql: SyncSql,
  input: UserCreditBalanceDeltaInput,
): Promise<UserCreditBalanceDeltaResult> => {
  validateDeltaInput(input)
  return withSyncTransaction(sql, async (writer) => {
    // Exact-once guard: first writer for this (user, source event) wins; a
    // replay conflicts and applies nothing.
    const guard: Array<{ idempotency_key: string }> = await writer.sql`
      INSERT INTO khala_sync_user_credit_balance_applied (user_id, idempotency_key)
      VALUES (${input.userId}, ${input.idempotencyKey})
      ON CONFLICT (user_id, idempotency_key) DO NOTHING
      RETURNING idempotency_key
    `
    if (guard[0] === undefined) {
      return { applied: false, reason: "duplicate_idempotency_key" }
    }

    // UPDATE-only: a missing row means the backfill has not run — refuse
    // (rolling the guard row back with the transaction) rather than
    // inventing a partial balance.
    const updated: Array<{
      balance_usd_cents: string | number | bigint
      last_event_at: Date | string | null
    }> = await writer.sql`
      UPDATE khala_sync_user_credit_balances
         SET balance_usd_cents = balance_usd_cents + ${input.deltaUsdCents},
             last_event_at = GREATEST(
               COALESCE(last_event_at, ${input.observedAt}::timestamptz),
               ${input.observedAt}::timestamptz
             ),
             updated_at = now()
       WHERE user_id = ${input.userId}
       RETURNING balance_usd_cents, last_event_at
    `
    const row = updated[0]
    if (row === undefined) {
      throw new UserCreditBalanceNotInitializedError(input.userId)
    }
    const balance: UserCreditBalanceRow = {
      balanceUsdCents: toSafeBalance(row.balance_usd_cents),
      lastEventAt: toIsoOrNull(row.last_event_at),
      userId: input.userId,
    }

    const entry = await writer.appendChange({
      entityId: EntityId.make(input.userId),
      entityType: EntityType.make(CREDIT_BALANCE_ENTITY_TYPE),
      mutationRef: USER_CREDIT_BALANCE_PROJECTION_SYSTEM_REF,
      op: "upsert",
      postImage: balancePostImage(balance),
      scope: personalScope(input.userId),
    })

    return { applied: true, balance, entry }
  })
}

// ---------------------------------------------------------------------------
// Fail-soft producer wrapper (same discipline as the public-counter
// projection)
// ---------------------------------------------------------------------------

export interface UserCreditBalanceProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason:
    | "credit_balance_not_initialized"
    | "invalid_input"
    | "storage_failed"
    | "projection_failed"
  readonly messageSafe: string
}

export type UserCreditBalanceProjectionOutcome =
  | { readonly ok: true; readonly result: UserCreditBalanceDeltaResult }
  | {
      readonly ok: false
      readonly diagnostic: UserCreditBalanceProjectionDiagnostic
    }

const diagnosticFromUnknown = (
  error: unknown,
): UserCreditBalanceProjectionDiagnostic => {
  if (error instanceof UserCreditBalanceNotInitializedError) {
    return {
      messageSafe: error.message,
      reason: "credit_balance_not_initialized",
    }
  }
  if (error instanceof UserCreditBalanceInvalidInputError) {
    return { messageSafe: error.message, reason: "invalid_input" }
  }
  const tag = (error as { _tag?: unknown })?._tag
  if (tag === "KhalaSyncStorageError") {
    const messageSafe = (error as { messageSafe?: unknown }).messageSafe
    return {
      messageSafe:
        typeof messageSafe === "string" ? messageSafe : "storage failure",
      reason: "storage_failed",
    }
  }
  // Anything else (driver errors, decode failures, a CHECK violation from a
  // drifted negative delta) can embed raw values or connection strings —
  // never echo them.
  return {
    messageSafe: "user credit balance projection failed",
    reason: "projection_failed",
  }
}

/**
 * Apply one balance delta FAIL-SOFT: this function never throws — any
 * failure (connection, constraint, uninitialized balance) rolls the
 * projection transaction back and comes back as a typed diagnostic for the
 * caller to log. The caller's authoritative business write (the D1 ledger
 * charge/grant/clawback) must never fail, retry, or reverse because of this
 * projection; a lost delta is exactly what the reconcile job detects and the
 * admin repair realigns.
 */
export const applyUserCreditBalanceDeltaBestEffort = async (
  sql: SyncSql,
  input: UserCreditBalanceDeltaInput,
): Promise<UserCreditBalanceProjectionOutcome> => {
  try {
    return { ok: true, result: await applyUserCreditBalanceDelta(sql, input) }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}

// ---------------------------------------------------------------------------
// Repair / backfill (explicit, audited — never silent)
// ---------------------------------------------------------------------------

export interface UserCreditBalanceRepairInput {
  readonly userId: string
  /** The exact source-of-truth balance (D1 agent_balances, converted to USD
   * cents at the shared rate) at the instant of this repair. */
  readonly exactBalanceUsdCents: number
  /** `backfill` (first bring-up for this user) or `reconcile_repair` (drift
   * realign). */
  readonly source: "backfill" | "reconcile_repair"
  /** Human audit note recorded with the repair (required, non-empty). */
  readonly auditNote: string
}

export interface UserCreditBalanceRepairResult {
  readonly balance: UserCreditBalanceRow
  readonly previousBalanceUsdCents: number | null
  readonly entry: ChangelogEntry
}

/**
 * Set the projection to the exact source D1 balance in ONE transaction:
 * upsert the row, record the audit row, and append the repaired post-image
 * to the user's personal scope. This is also the first-deploy-for-this-user
 * backfill (source `backfill`); until it runs for a given user, deltas
 * refuse with `credit_balance_not_initialized` — the mobile client must
 * never read a fabricated zero balance for a user who has real D1 credit.
 */
export const repairUserCreditBalance = async (
  sql: SyncSql,
  input: UserCreditBalanceRepairInput,
): Promise<UserCreditBalanceRepairResult> => {
  if (
    !Number.isSafeInteger(input.exactBalanceUsdCents) ||
    input.exactBalanceUsdCents < 0
  ) {
    throw new UserCreditBalanceInvalidInputError(
      "user credit balance repairs require a non-negative safe-integer exact balance",
    )
  }
  if (input.auditNote.trim().length === 0) {
    throw new UserCreditBalanceInvalidInputError(
      "user credit balance repairs require a non-empty audit note",
    )
  }
  return withSyncTransaction(sql, async (writer) => {
    const previous: Array<{ balance_usd_cents: string | number | bigint }> =
      await writer.sql`
      SELECT balance_usd_cents FROM khala_sync_user_credit_balances
       WHERE user_id = ${input.userId}
       FOR UPDATE
    `
    const previousBalanceUsdCents =
      previous[0] === undefined ? null : toSafeBalance(previous[0].balance_usd_cents)

    const upserted: Array<{
      balance_usd_cents: string | number | bigint
      last_event_at: Date | string | null
    }> = await writer.sql`
      INSERT INTO khala_sync_user_credit_balances (user_id, balance_usd_cents, updated_at)
      VALUES (${input.userId}, ${input.exactBalanceUsdCents}, now())
      ON CONFLICT (user_id) DO UPDATE SET
        balance_usd_cents = EXCLUDED.balance_usd_cents,
        updated_at = now()
      RETURNING balance_usd_cents, last_event_at
    `
    const row = upserted[0]
    if (row === undefined) {
      throw new UserCreditBalanceInvalidInputError(
        "user credit balance repair upsert returned no row",
      )
    }
    const balance: UserCreditBalanceRow = {
      balanceUsdCents: toSafeBalance(row.balance_usd_cents),
      lastEventAt: toIsoOrNull(row.last_event_at),
      userId: input.userId,
    }

    await writer.sql`
      INSERT INTO khala_sync_user_credit_balance_repairs
        (user_id, previous_balance, new_balance, source, audit_note)
      VALUES
        (${input.userId}, ${previousBalanceUsdCents}, ${input.exactBalanceUsdCents},
         ${input.source}, ${input.auditNote})
    `

    const entry = await writer.appendChange({
      entityId: EntityId.make(input.userId),
      entityType: EntityType.make(CREDIT_BALANCE_ENTITY_TYPE),
      mutationRef: USER_CREDIT_BALANCE_REPAIR_SYSTEM_REF,
      op: "upsert",
      postImage: balancePostImage(balance),
      scope: personalScope(input.userId),
    })

    return { balance, entry, previousBalanceUsdCents }
  })
}
