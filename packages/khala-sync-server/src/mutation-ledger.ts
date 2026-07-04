import {
  canonicalJson,
  type ClientGroupId,
  type ClientId,
  MutationId,
  type MutationEnvelope,
  MutationResult,
  type MutationStatus,
  type MutatorName,
  type SyncSchemaVersion,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { KhalaSyncStorageError } from "./errors.js"
import type { SqlTag } from "./sql.js"

/**
 * Mutation ledger idempotency + client-group state (KS-2.4; SPEC §2.4/§3,
 * invariants 2-3).
 *
 * `khala_sync_mutations` is the per-client execution ledger: one row per
 * executed mutation, keyed `(client_group_id, client_id, mutation_id)`,
 * written INSIDE the same transaction as the mutator's business writes and
 * changelog appends. Because the ledger row commits atomically with the
 * effects, a replayed envelope (client retry, crash between execute and
 * respond, duplicated batch) hits the recorded row and is answered with
 * `status: "duplicate"` and the recorded outcome — the mutator NEVER
 * re-executes.
 *
 * Ordering: mutation ids are per-client sequential starting at 1.
 * `lastMutationId` is `MAX(mutation_id)` for the `(clientGroup, client)`
 * pair; the ledger is dense by construction because `checkAndReserve` only
 * lets `lastMutationId + 1` through to execution:
 *
 * - `mutationId <= lastMutationId` → duplicate: recorded result returned,
 *   nothing executes.
 * - `mutationId == lastMutationId + 1` → execute the mutator, then
 *   `recordMutation` in the same transaction.
 * - `mutationId >  lastMutationId + 1` → `out_of_order`: a typed IN-BAND
 *   rejection (never a 4xx/queue block, SPEC §2.4 acceptance rules) that
 *   acks NOTHING — no ledger row is written and `lastMutationId` does not
 *   advance, so the client re-pushes the missing prefix and the gap heals.
 *
 * `khala_sync_client_state` binds a client group to ONE user. `upsertClientState`
 * inserts-or-updates it on every push (schema version + `last_seen_at`
 * bump) and fails with a typed {@link KhalaSyncClientStateMismatchError}
 * when the stored `user_id` differs — a client group can never migrate
 * between users. The state row doubles as the per-client-group
 * serialization point: the push engine upserts it INSIDE the mutator
 * transaction before gating envelopes, which takes the group's row lock,
 * so concurrent pushes for one group serialize instead of racing
 * `MAX(mutation_id)`. `checkAndReserve` enforces that order by re-taking
 * the row lock (`SELECT … FOR UPDATE`) and failing typed when the row is
 * missing.
 *
 * All statements are single-transaction-safe: no session state, no
 * LISTEN/NOTIFY, no advisory locks — only ordinary row locks, the same
 * discipline the outbox writer uses for the scope counter — so this runs
 * through Hyperdrive's transaction-mode pooling (SPEC §4).
 */

// ---------------------------------------------------------------------------
// SQL handle
// ---------------------------------------------------------------------------

/**
 * Any tagged-template SQL handle (Bun `SQL`/`TransactionSQL` or postgres.js
 * — see ./sql.ts). Ledger writes (`recordMutation`, `checkAndReserve`) must
 * receive the MUTATOR TRANSACTION's handle (`writer.sql` from
 * `withSyncTransaction`) so they commit atomically with the mutator's
 * effects; reads accept either.
 */
export type SqlHandle = SqlTag

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

export interface MutationLedgerKey {
  readonly clientGroupId: ClientGroupId
  readonly clientId: ClientId
  readonly mutationId: MutationId
}

export interface RecordMutationInput extends MutationLedgerKey {
  readonly name: MutatorName
  /** Outcome at execution time — `applied` or `rejected`, never `duplicate`. */
  readonly status: MutationStatus
  readonly errorCode?: string
  /**
   * Canonical JSON of the encoded {@link MutationResult} the engine
   * responded with (or is about to respond with). Replays are answered
   * from this recording.
   */
  readonly resultJson?: string
  readonly scope?: SyncScope
}

/** A `khala_sync_mutations` row, decoded. */
export interface RecordedMutation {
  readonly clientGroupId: ClientGroupId
  readonly clientId: ClientId
  readonly mutationId: MutationId
  readonly name: string
  readonly status: MutationStatus
  readonly errorCode?: string
  /** Canonical JSON (re-canonicalized after jsonb normalization). */
  readonly resultJson?: string
  readonly scope?: string
  readonly committedAt: string
}

interface MutationRow {
  readonly client_group_id: string
  readonly client_id: string
  readonly mutation_id: string | number | bigint
  readonly name: string
  readonly status: string
  readonly error_code: string | null
  readonly result_json: string | object | null
  readonly scope: string | null
  readonly committed_at: Date | string
}

const MUTATION_STATUSES: ReadonlySet<string> = new Set([
  "applied",
  "rejected",
  "duplicate",
])

const toMutationIdNumber = (raw: string | number | bigint): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `mutation id out of safe range: ${String(raw)}`,
    )
  }
  return value
}

const toCommittedAt = (raw: Date | string): string =>
  raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()

const recordedMutationFromRow = (row: MutationRow): RecordedMutation => {
  if (!MUTATION_STATUSES.has(row.status)) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `unknown mutation status in ledger: ${row.status}`,
    )
  }
  const resultJson =
    row.result_json === null
      ? undefined
      : canonicalJson(
          typeof row.result_json === "string"
            ? JSON.parse(row.result_json)
            : row.result_json,
        )
  return {
    clientGroupId: row.client_group_id as ClientGroupId,
    clientId: row.client_id as ClientId,
    mutationId: MutationId.make(toMutationIdNumber(row.mutation_id)),
    name: row.name,
    status: row.status as MutationStatus,
    ...(row.error_code === null ? {} : { errorCode: row.error_code }),
    ...(resultJson === undefined ? {} : { resultJson }),
    ...(row.scope === null ? {} : { scope: row.scope }),
    committedAt: toCommittedAt(row.committed_at),
  }
}

// ---------------------------------------------------------------------------
// Ledger writes + reads
// ---------------------------------------------------------------------------

/**
 * Record one executed mutation in the ledger, INSIDE the mutator
 * transaction (`writer.sql`). Insert-once: a concurrent or replayed write
 * for the same `(clientGroup, client, mutationId)` is a no-op
 * (`ON CONFLICT DO NOTHING`) — the FIRST recording wins and `inserted`
 * reports whether THIS call created the row.
 */
export const recordMutation = async (
  sql: SqlHandle,
  input: RecordMutationInput,
): Promise<{ readonly inserted: boolean }> => {
  const rows: Array<{ mutation_id: string | number | bigint }> = await sql`
    INSERT INTO khala_sync_mutations
      (client_group_id, client_id, mutation_id, name, status,
       error_code, result_json, scope)
    VALUES
      (${input.clientGroupId}, ${input.clientId}, ${input.mutationId},
       ${input.name}, ${input.status}, ${input.errorCode ?? null},
       ${input.resultJson ?? null}::jsonb, ${input.scope ?? null})
    ON CONFLICT (client_group_id, client_id, mutation_id) DO NOTHING
    RETURNING mutation_id
  `
  return { inserted: rows.length > 0 }
}

/** Read one recorded mutation, or `null` when the ledger has no row. */
export const getMutation = async (
  sql: SqlHandle,
  key: MutationLedgerKey,
): Promise<RecordedMutation | null> => {
  const rows: Array<MutationRow> = await sql`
    SELECT client_group_id, client_id, mutation_id, name, status,
           error_code, result_json, scope, committed_at
      FROM khala_sync_mutations
     WHERE client_group_id = ${key.clientGroupId}
       AND client_id = ${key.clientId}
       AND mutation_id = ${key.mutationId}
  `
  const row = rows[0]
  return row === undefined ? null : recordedMutationFromRow(row)
}

/**
 * The highest recorded mutation id for `(clientGroup, client)` — `0` when
 * the client has never pushed. Dense by construction (see module doc), so
 * this is also "the id through which everything is acked".
 */
export const lastMutationId = async (
  sql: SqlHandle,
  key: { readonly clientGroupId: ClientGroupId; readonly clientId: ClientId },
): Promise<number> => {
  const rows: Array<{ last: string | number | bigint | null }> = await sql`
    SELECT MAX(mutation_id) AS last
      FROM khala_sync_mutations
     WHERE client_group_id = ${key.clientGroupId}
       AND client_id = ${key.clientId}
  `
  const raw = rows[0]?.last ?? null
  return raw === null ? 0 : toMutationIdNumber(raw)
}

// ---------------------------------------------------------------------------
// checkAndReserve — the push engine's idempotency + ordering gate
// ---------------------------------------------------------------------------

export const OUT_OF_ORDER_ERROR_CODE = "out_of_order"

/** Build the `status: "duplicate"` response for a recorded mutation. */
export const duplicateResultFor = (
  recorded: RecordedMutation,
): MutationResult => {
  let errorMessageSafe: string | undefined
  if (recorded.resultJson !== undefined) {
    const parsed: unknown = JSON.parse(recorded.resultJson)
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      typeof (parsed as { errorMessageSafe?: unknown }).errorMessageSafe ===
        "string"
    ) {
      errorMessageSafe = (parsed as { errorMessageSafe: string })
        .errorMessageSafe
    }
  }
  return new MutationResult({
    mutationId: recorded.mutationId,
    status: "duplicate",
    ...(recorded.errorCode === undefined
      ? {}
      : { errorCode: recorded.errorCode }),
    ...(errorMessageSafe === undefined ? {} : { errorMessageSafe }),
  })
}

/** Build the typed in-band `out_of_order` rejection (acks nothing). */
export const outOfOrderResult = (
  mutationId: MutationId,
  last: number,
): MutationResult =>
  new MutationResult({
    mutationId,
    status: "rejected",
    errorCode: OUT_OF_ORDER_ERROR_CODE,
    errorMessageSafe:
      `mutation ${mutationId} is ahead of the acked sequence ` +
      `(lastMutationId ${last}, expected ${last + 1}); ` +
      "re-push the missing prefix first",
  })

export type CheckAndReserveOutcome =
  /** `mutationId == lastMutationId + 1`: run the mutator, then `recordMutation`. */
  | { readonly kind: "execute"; readonly lastMutationId: number }
  /**
   * Already recorded: respond with `result` (`status: "duplicate"`, the
   * recorded outcome in-band) WITHOUT executing anything.
   */
  | {
      readonly kind: "duplicate"
      readonly recorded: RecordedMutation
      readonly result: MutationResult
    }
  /**
   * Gap ahead of the acked sequence: respond with `result` (an in-band
   * `rejected`/`out_of_order`), record NOTHING, ack NOTHING.
   */
  | {
      readonly kind: "out_of_order"
      readonly lastMutationId: number
      readonly result: MutationResult
    }

/**
 * Idempotency + ordering gate, run INSIDE the mutator transaction before
 * executing an envelope, AFTER `upsertClientState` has bound the client
 * group in the same transaction. Holds the group's `khala_sync_client_state`
 * row lock (re-taken here via `SELECT … FOR UPDATE`; the upsert takes it
 * first), so concurrent pushes for the same client group serialize
 * instead of racing `MAX(mutation_id)` — an ordinary row lock, safe under
 * transaction-mode pooling.
 */
export const checkAndReserve = async (
  sql: SqlHandle,
  input: {
    readonly clientGroupId: ClientGroupId
    readonly clientId: ClientId
    readonly envelope: MutationEnvelope
  },
): Promise<CheckAndReserveOutcome> => {
  const { clientGroupId, clientId, envelope } = input
  const stateRows: Array<{ client_group_id: string }> = await sql`
    SELECT client_group_id FROM khala_sync_client_state
     WHERE client_group_id = ${clientGroupId}
       FOR UPDATE
  `
  if (stateRows.length === 0) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      "no khala_sync_client_state row for this client group: " +
        "upsertClientState must run before checkAndReserve " +
        "in the same push transaction",
    )
  }
  const last = await lastMutationId(sql, { clientGroupId, clientId })
  const id = Number(envelope.mutationId)

  if (id <= last) {
    const recorded = await getMutation(sql, {
      clientGroupId,
      clientId,
      mutationId: envelope.mutationId,
    })
    if (recorded === null) {
      // Dense-by-construction ledger cannot have a hole at or below
      // lastMutationId; a missing row means the ledger was tampered with.
      throw new KhalaSyncStorageError(
        "constraint_violation",
        `mutation ledger hole: id ${id} <= lastMutationId ${last} has no row`,
      )
    }
    return {
      kind: "duplicate",
      recorded,
      result: duplicateResultFor(recorded),
    }
  }

  if (id > last + 1) {
    return {
      kind: "out_of_order",
      lastMutationId: last,
      result: outOfOrderResult(envelope.mutationId, last),
    }
  }

  return { kind: "execute", lastMutationId: last }
}

// ---------------------------------------------------------------------------
// Client-group state
// ---------------------------------------------------------------------------

/** A client group is bound to ONE user for its whole lifetime (SPEC §2.1). */
export class KhalaSyncClientStateMismatchError extends Error {
  readonly _tag = "KhalaSyncClientStateMismatchError"
  override readonly name = "KhalaSyncClientStateMismatchError"
  constructor(
    readonly clientGroupId: ClientGroupId,
    readonly storedUserId: string,
    readonly requestedUserId: string,
  ) {
    super(
      "client group is bound to a different user; " +
        "client groups never migrate between users",
    )
  }
}

/**
 * Insert-or-update `khala_sync_client_state` on push: new groups are bound
 * to `userId`; existing groups get `schema_version` refreshed and
 * `last_seen_at` bumped. Throws {@link KhalaSyncClientStateMismatchError}
 * (leaving the row untouched) when the group is bound to a different user.
 */
export const upsertClientState = async (
  sql: SqlHandle,
  input: {
    readonly clientGroupId: ClientGroupId
    readonly userId: string
    readonly schemaVersion: SyncSchemaVersion
  },
): Promise<{ readonly created: boolean }> => {
  const rows: Array<{ created: boolean }> = await sql`
    INSERT INTO khala_sync_client_state (client_group_id, user_id, schema_version)
    VALUES (${input.clientGroupId}, ${input.userId}, ${input.schemaVersion})
    ON CONFLICT (client_group_id) DO UPDATE SET
      schema_version = EXCLUDED.schema_version,
      last_seen_at = now()
    WHERE khala_sync_client_state.user_id = EXCLUDED.user_id
    RETURNING (xmax = 0) AS created
  `
  const row = rows[0]
  if (row !== undefined) return { created: row.created === true }

  // Conflict row exists but the user-binding WHERE filtered it out.
  const stored: Array<{ user_id: string }> = await sql`
    SELECT user_id FROM khala_sync_client_state
     WHERE client_group_id = ${input.clientGroupId}
  `
  const storedUserId = stored[0]?.user_id
  if (storedUserId === undefined) {
    throw new KhalaSyncStorageError(
      "unavailable",
      "client state upsert returned no row and no stored row was found",
    )
  }
  throw new KhalaSyncClientStateMismatchError(
    input.clientGroupId,
    storedUserId,
    input.userId,
  )
}
