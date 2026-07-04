import {
  type ChangelogEntry,
  decodePublicCounterEntity,
  encodePublicCounterEntity,
  EntityId,
  EntityType,
  PUBLIC_COUNTER_ENTITY_TYPE,
  type PublicCounterEntity,
  publicScope,
  TOKENS_SERVED_COUNTER_ID,
} from "@openagentsinc/khala-sync"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

/**
 * Public-counter scope projection (KS-6.3, #8304; SPEC §2.1
 * `scope.public.<channel>`, §7 invariant 8).
 *
 * The tokens-served headline used to be a live-at-read full-table
 * `SUM(...) FROM token_usage_events` on every public read. Here it becomes
 * a projected `public_counter` entity: the ingest path increments the
 * Postgres counter row AND appends the post-image to
 * `scope.public.tokens-served` in ONE transaction (`withSyncTransaction`,
 * so per-scope versions stay dense and commit-ordered).
 *
 * EXACT-ONCE PER SOURCE ROW (invariant 8: the sync path never invents
 * counter deltas): every increment is keyed by the source ledger event's
 * idempotency key through a guard insert into `khala_sync_counter_applied`
 * IN THE SAME transaction — `ON CONFLICT DO NOTHING` turns a replay into a
 * no-op (`applied: false`), and a rolled-back increment rolls its guard row
 * back with it.
 *
 * BRING-UP / NOT-INITIALIZED REFUSAL: the increment path only ever UPDATEs
 * an existing counter row. Before the admin backfill (`repairPublicCounter`
 * with source `backfill`, which sets the row to the exact source SUM) the
 * row does not exist and increments are refused with
 * `counter_not_initialized` — a fresh deploy can never publish a tiny
 * partial total as the network aggregate. Events skipped this way are
 * covered by the backfill SUM itself.
 *
 * RECONCILIATION (invariant 8): callers periodically recompute the exact
 * source SUM and compare against `readPublicCounter`. Drift is NEVER
 * silently overwritten — `repairPublicCounter` is an explicit admin action
 * that records the previous total, the exact total, a source
 * (`backfill` | `reconcile_repair`), and an audit note in
 * `khala_sync_public_counter_repairs`, and appends the repaired post-image
 * to the scope so subscribed clients converge.
 */

// ---------------------------------------------------------------------------
// Named system writers (SPEC §7 invariant 3)
// ---------------------------------------------------------------------------

export const PUBLIC_COUNTER_PROJECTION_SYSTEM_REF =
  "system:public_counter_projection.token_usage_ledger.v1"

export const PUBLIC_COUNTER_REPAIR_SYSTEM_REF =
  "system:public_counter_repair.v1"

/** The tokens-served counter's scope: `scope.public.tokens-served`. */
export const tokensServedPublicScope = () =>
  publicScope(TOKENS_SERVED_COUNTER_ID)

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/**
 * The counter row does not exist yet (pre-backfill). The increment refuses
 * (and its transaction — including the idempotency guard row — rolls back)
 * rather than inventing a partial total.
 */
export class PublicCounterNotInitializedError extends Error {
  readonly _tag = "PublicCounterNotInitializedError"
  override readonly name = "PublicCounterNotInitializedError"
  constructor(readonly counterId: string) {
    super(
      `public counter ${counterId} is not initialized — run the admin ` +
        "backfill (repair with source 'backfill') before increments apply",
    )
  }
}

export class PublicCounterInvalidInputError extends Error {
  readonly _tag = "PublicCounterInvalidInputError"
  override readonly name = "PublicCounterInvalidInputError"
  constructor(messageSafe: string) {
    super(messageSafe)
  }
}

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

export interface PublicCounterRow {
  readonly counterId: string
  readonly total: number
  readonly lastEventAt: string | null
}

const toIsoOrNull = (raw: Date | string | null): string | null =>
  raw === null
    ? null
    : raw instanceof Date
      ? raw.toISOString()
      : new Date(raw).toISOString()

const toSafeTotal = (raw: string | number | bigint): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new PublicCounterInvalidInputError(
      `public counter total out of safe range: ${String(raw)}`,
    )
  }
  return value
}

// ---------------------------------------------------------------------------
// Reads (single statement — Hyperdrive transaction-mode safe)
// ---------------------------------------------------------------------------

/** Read one public counter row, or null when it is not initialized. */
export const readPublicCounter = async (
  sql: SqlTag,
  counterId: string,
): Promise<PublicCounterRow | null> => {
  const rows: Array<{
    total: string | number | bigint
    last_event_at: Date | string | null
  }> = await sql`
    SELECT total, last_event_at
      FROM khala_sync_public_counters
     WHERE counter_id = ${counterId}
  `
  const row = rows[0]
  if (row === undefined) return null
  return {
    counterId,
    lastEventAt: toIsoOrNull(row.last_event_at),
    total: toSafeTotal(row.total),
  }
}

// ---------------------------------------------------------------------------
// Increment (exact-once by source idempotency key)
// ---------------------------------------------------------------------------

export interface PublicCounterIncrementInput {
  readonly counterId: string
  /** The SOURCE ledger event's idempotency key (exact-once per D1 row). */
  readonly idempotencyKey: string
  /** Positive integer token delta for this one source row. */
  readonly delta: number
  /** The source row's observed-at ISO timestamp. */
  readonly observedAt: string
}

export type PublicCounterIncrementApplied = Readonly<{
  applied: true
  counter: PublicCounterRow
  entry: ChangelogEntry
}>

export type PublicCounterIncrementResult =
  | PublicCounterIncrementApplied
  | Readonly<{ applied: false; reason: "duplicate_idempotency_key" }>

const validateIncrementInput = (input: PublicCounterIncrementInput): void => {
  if (input.idempotencyKey.trim().length === 0) {
    throw new PublicCounterInvalidInputError(
      "public counter increments require a non-empty idempotency key",
    )
  }
  if (!Number.isSafeInteger(input.delta) || input.delta <= 0) {
    throw new PublicCounterInvalidInputError(
      "public counter increments require a positive integer delta",
    )
  }
  if (!Number.isFinite(Date.parse(input.observedAt))) {
    throw new PublicCounterInvalidInputError(
      "public counter increments require a parseable observedAt timestamp",
    )
  }
}

const counterPostImage = (counter: PublicCounterRow): unknown =>
  encodePublicCounterEntity(
    decodePublicCounterEntity({
      counterId: counter.counterId,
      lastEventAt: counter.lastEventAt,
      total: counter.total,
    } satisfies Record<keyof PublicCounterEntity, unknown>),
  )

/**
 * Apply one exact-once counter increment + changelog append in ONE Postgres
 * transaction. Throws on storage failure, invalid input, or an
 * uninitialized counter; a replayed idempotency key resolves
 * `{ applied: false }` without touching the counter.
 */
export const applyPublicCounterIncrement = async (
  sql: SyncSql,
  input: PublicCounterIncrementInput,
): Promise<PublicCounterIncrementResult> => {
  validateIncrementInput(input)
  return withSyncTransaction(sql, async (writer) => {
    // Exact-once guard: first writer for this (counter, source event) wins;
    // a replay conflicts and applies nothing (invariant 8).
    const guard: Array<{ idempotency_key: string }> = await writer.sql`
      INSERT INTO khala_sync_counter_applied (counter_id, idempotency_key)
      VALUES (${input.counterId}, ${input.idempotencyKey})
      ON CONFLICT (counter_id, idempotency_key) DO NOTHING
      RETURNING idempotency_key
    `
    if (guard[0] === undefined) {
      return { applied: false, reason: "duplicate_idempotency_key" }
    }

    // UPDATE-only: a missing row means the backfill has not run — refuse
    // (rolling the guard row back with the transaction) rather than
    // inventing a partial total.
    const updated: Array<{
      total: string | number | bigint
      last_event_at: Date | string | null
    }> = await writer.sql`
      UPDATE khala_sync_public_counters
         SET total = total + ${input.delta},
             last_event_at = GREATEST(
               COALESCE(last_event_at, ${input.observedAt}::timestamptz),
               ${input.observedAt}::timestamptz
             ),
             updated_at = now()
       WHERE counter_id = ${input.counterId}
       RETURNING total, last_event_at
    `
    const row = updated[0]
    if (row === undefined) {
      throw new PublicCounterNotInitializedError(input.counterId)
    }
    const counter: PublicCounterRow = {
      counterId: input.counterId,
      lastEventAt: toIsoOrNull(row.last_event_at),
      total: toSafeTotal(row.total),
    }

    const entry = await writer.appendChange({
      entityId: EntityId.make(input.counterId),
      entityType: EntityType.make(PUBLIC_COUNTER_ENTITY_TYPE),
      mutationRef: PUBLIC_COUNTER_PROJECTION_SYSTEM_REF,
      op: "upsert",
      postImage: counterPostImage(counter),
      scope: publicScope(input.counterId),
    })

    return { applied: true, counter, entry }
  })
}

// ---------------------------------------------------------------------------
// Fail-soft producer wrapper (same discipline as the fleet projection)
// ---------------------------------------------------------------------------

export interface PublicCounterProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason:
    | "counter_not_initialized"
    | "invalid_input"
    | "storage_failed"
    | "projection_failed"
  readonly messageSafe: string
}

export type PublicCounterProjectionOutcome =
  | { readonly ok: true; readonly result: PublicCounterIncrementResult }
  | { readonly ok: false; readonly diagnostic: PublicCounterProjectionDiagnostic }

const diagnosticFromUnknown = (
  error: unknown,
): PublicCounterProjectionDiagnostic => {
  if (error instanceof PublicCounterNotInitializedError) {
    return { messageSafe: error.message, reason: "counter_not_initialized" }
  }
  if (error instanceof PublicCounterInvalidInputError) {
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
  // Anything else (driver errors, decode failures) can embed raw values or
  // connection strings — never echo them.
  return {
    messageSafe: "public counter projection failed",
    reason: "projection_failed",
  }
}

/**
 * Apply one counter increment FAIL-SOFT: this function never throws — any
 * failure (connection, constraint, uninitialized counter) rolls the
 * projection transaction back and comes back as a typed diagnostic for the
 * caller to log. The caller's authoritative business write (the D1 ledger
 * insert) must never fail because of the projection; a lost increment is
 * exactly what the reconcile job detects and the admin repair realigns.
 */
export const applyPublicCounterIncrementBestEffort = async (
  sql: SyncSql,
  input: PublicCounterIncrementInput,
): Promise<PublicCounterProjectionOutcome> => {
  try {
    return { ok: true, result: await applyPublicCounterIncrement(sql, input) }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}

// ---------------------------------------------------------------------------
// Repair / backfill (explicit, audited — never silent)
// ---------------------------------------------------------------------------

export interface PublicCounterRepairInput {
  readonly counterId: string
  /** The exact source-of-truth total (SUM over exact ledger rows). */
  readonly exactTotal: number
  /** `backfill` (first bring-up) or `reconcile_repair` (drift realign). */
  readonly source: "backfill" | "reconcile_repair"
  /** Human audit note recorded with the repair (required, non-empty). */
  readonly auditNote: string
}

export interface PublicCounterRepairResult {
  readonly counter: PublicCounterRow
  readonly previousTotal: number | null
  readonly entry: ChangelogEntry
}

/**
 * Set the projection to the exact source SUM in ONE transaction: upsert the
 * counter row, record the audit row, and append the repaired post-image to
 * the scope. This is also the first-deploy backfill (source `backfill`);
 * until it runs, increments refuse with `counter_not_initialized`.
 */
export const repairPublicCounter = async (
  sql: SyncSql,
  input: PublicCounterRepairInput,
): Promise<PublicCounterRepairResult> => {
  if (!Number.isSafeInteger(input.exactTotal) || input.exactTotal < 0) {
    throw new PublicCounterInvalidInputError(
      "public counter repairs require a non-negative safe-integer exact total",
    )
  }
  if (input.auditNote.trim().length === 0) {
    throw new PublicCounterInvalidInputError(
      "public counter repairs require a non-empty audit note",
    )
  }
  return withSyncTransaction(sql, async (writer) => {
    const previous: Array<{ total: string | number | bigint }> = await writer.sql`
      SELECT total FROM khala_sync_public_counters
       WHERE counter_id = ${input.counterId}
       FOR UPDATE
    `
    const previousTotal =
      previous[0] === undefined ? null : toSafeTotal(previous[0].total)

    const upserted: Array<{
      total: string | number | bigint
      last_event_at: Date | string | null
    }> = await writer.sql`
      INSERT INTO khala_sync_public_counters (counter_id, total, updated_at)
      VALUES (${input.counterId}, ${input.exactTotal}, now())
      ON CONFLICT (counter_id) DO UPDATE SET
        total = EXCLUDED.total,
        updated_at = now()
      RETURNING total, last_event_at
    `
    const row = upserted[0]
    if (row === undefined) {
      throw new PublicCounterInvalidInputError(
        "public counter repair upsert returned no row",
      )
    }
    const counter: PublicCounterRow = {
      counterId: input.counterId,
      lastEventAt: toIsoOrNull(row.last_event_at),
      total: toSafeTotal(row.total),
    }

    await writer.sql`
      INSERT INTO khala_sync_public_counter_repairs
        (counter_id, previous_total, new_total, source, audit_note)
      VALUES
        (${input.counterId}, ${previousTotal}, ${input.exactTotal},
         ${input.source}, ${input.auditNote})
    `

    const entry = await writer.appendChange({
      entityId: EntityId.make(input.counterId),
      entityType: EntityType.make(PUBLIC_COUNTER_ENTITY_TYPE),
      mutationRef: PUBLIC_COUNTER_REPAIR_SYSTEM_REF,
      op: "upsert",
      postImage: counterPostImage(counter),
      scope: publicScope(input.counterId),
    })

    return { counter, entry, previousTotal }
  })
}
