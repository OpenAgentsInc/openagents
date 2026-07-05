// KS-8.6 (#8317): Artanis supervision domain — D1 → Cloud SQL migration
// machinery, following the KS-8.1 pylon template (pylon-dispatch-store.ts).
//
// Twenty `artanis_*` tables, six every-minute cron ticks
// (ArtanisScheduledRunner.runTick, ArtanisResponder.scan/compose,
// ArtanisAdmin.tick/closeoutVerifier, ArtanisFleet.tick). Unlike the pylon
// lane, this domain never had a single store interface — its SQL lives in
// eleven owning modules — so the seam here is a DATABASE-SHAPED handle
// instead of a per-operation store:
//
//  1. `ArtanisDatabase = D1Database | ArtanisDomainHandle` — every
//     artanis_* module signature takes this union. A plain `D1Database`
//     still works (no mirroring, no routing — fail-safe), and
//     `artanisAuthorityDb(db)` recovers the authoritative D1 handle either
//     way. `makeArtanisDatabaseForEnv(env)` is the index.ts drop-in that
//     upgrades the six cron ticks and the artanis routes to the seam.
//
//  2. `mirrorArtanisRows(db, table, keyColumn, keys)` — the dual-write.
//     After the authoritative D1 write, the RESOLVED row(s) are read back
//     from D1 by key and converged into Postgres as full-row upserts
//     (`ON CONFLICT (natural key) DO UPDATE`), so a row touched by
//     dual-write self-heals even before the backfill reaches it, and
//     re-mirroring is idempotent by construction. A Postgres failure NEVER
//     fails the request — it logs the typed
//     `khala_sync_artanis_dual_write_failed` diagnostic (the drift metric)
//     and moves on. This deliberately preserves the Artanis operator-chat
//     fail-soft precedent (2d46d808): persistence degradation must never
//     take down a tick or a chat turn.
//
//  3. `artanisRead(db, op, refs, readD1, readPostgres)` — flag-routed
//     reads: d1 (default; the cron ticks keep D1 authority until the
//     read-cutover evidence lands), compare (read both, SERVE D1, log
//     mismatches), postgres (bounded retry, D1 fallback + diagnostic on
//     exhaustion). Aggregation/JOIN read paths that have no Postgres twin
//     yet simply pass no `readPostgres` and stay on D1.
//
// Flags (per KS-8 convention):
//   KHALA_SYNC_ARTANIS_DUAL_WRITE  (default ON; '0'|'off'|'false'|'disabled'|'no')
//   KHALA_SYNC_ARTANIS_READS       (default 'd1'; 'd1'|'postgres'|'compare')
// With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Artanis supervision domain"):
// dual-write on → backfill (khala-sync-server scripts/backfill-artanis.ts)
// → second sweep → --verify → compare reads → postgres reads → re-home the
// six cron ticks → drop the D1 tables in the follow-up decommission issue.

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type ArtanisDomainReadsMode = 'd1' | 'postgres' | 'compare'

export type ArtanisDomainFlags = Readonly<{
  dualWrite: boolean
  reads: ArtanisDomainReadsMode
}>

export type ArtanisDomainFlagEnv = Readonly<{
  KHALA_SYNC_ARTANIS_DUAL_WRITE?: string | undefined
  KHALA_SYNC_ARTANIS_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.6 migration flags from Worker vars. Dual-write defaults ON
 * (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority until the runbook's cutover sequence flips
 * them. Unknown read values fall back to 'd1' — never fail open into an
 * unproven read path on a typo.
 */
export const artanisDomainFlagsFromEnv = (
  env: ArtanisDomainFlagEnv,
): ArtanisDomainFlags => {
  const dualWriteRaw = env.KHALA_SYNC_ARTANIS_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_ARTANIS_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type ArtanisDomainDiagnosticEvent =
  | 'khala_sync_artanis_dual_write_failed'
  | 'khala_sync_artanis_read_compare_mismatch'
  | 'khala_sync_artanis_postgres_read_failed'
  | 'khala_sync_artanis_postgres_read_fallback'

export type ArtanisDomainDiagnostic = Readonly<{
  /** The mirrored table or read operation, e.g. 'artanis_loop_ticks'. */
  op: string
  /** Public-safe refs identifying the affected rows (never payloads). */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type ArtanisDomainLog = (
  event: ArtanisDomainDiagnosticEvent,
  fields: ArtanisDomainDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// Table registry
// ---------------------------------------------------------------------------
//
// Column lists mirror khala-sync-server migration 0011_artanis_domain.sql
// (which mirrors the live D1 schema: worker migrations 0119/0120/0161/0163/
// 0164/0165/0169/0213/0215/0245/0248/0249/0256) and the registry in
// packages/khala-sync-server/src/artanis-backfill.ts. The contract test
// proves the registry against BOTH engines' real SQL.

export type ArtanisDomainTable =
  | 'artanis_runtime_snapshots'
  | 'artanis_loop_records'
  | 'artanis_loop_ticks'
  | 'artanis_approval_gates'
  | 'artanis_health_snapshots'
  | 'artanis_work_routing_proposals'
  | 'artanis_forum_publication_intents'
  | 'artanis_nexus_pylon_adapter_dispatches'
  | 'artanis_responder_state'
  | 'artanis_responder_actions'
  | 'artanis_responder_ticks'
  | 'artanis_admin_tick_decisions'
  | 'artanis_closeout_verdicts'
  | 'artanis_fleet_overseer_decisions'
  | 'artanis_standing_spend_grants'
  | 'artanis_spend_decisions'
  | 'artanis_labor_unattended_receipts'
  | 'artanis_owner_memory'
  | 'artanis_threads'
  | 'artanis_messages'

const LEDGER_COLUMNS = [
  'id',
  'agent_id',
  'record_ref',
  'idempotency_key',
  'state',
  'active',
  'source_kind',
  'scope_ref',
  'parent_ref',
  'record_json',
  'public_projection_json',
  'content_hash',
  'closeout_json',
  'created_at',
  'updated_at',
  'closed_at',
] as const

type ArtanisDomainTableSpec = Readonly<{
  columns: ReadonlyArray<string>
  /** Conflict target for the converge upsert (the table's natural key). */
  conflictKey: string
  /** Columns modules may key mirrors/reads by (validated, never dynamic). */
  keyColumns: ReadonlyArray<string>
  /** Column latest-N reads order by (text ISO timestamps sort correctly). */
  orderColumn: string
}>

const ledgerSpec: ArtanisDomainTableSpec = {
  columns: LEDGER_COLUMNS,
  conflictKey: 'record_ref',
  keyColumns: ['record_ref', 'idempotency_key'],
  orderColumn: 'updated_at',
}

export const ARTANIS_DOMAIN_TABLES: Readonly<
  Record<ArtanisDomainTable, ArtanisDomainTableSpec>
> = {
  artanis_admin_tick_decisions: {
    columns: ['id', 'state', 'action_json', 'assignment_ref', 'created_at'],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'created_at',
  },
  artanis_approval_gates: ledgerSpec,
  artanis_closeout_verdicts: {
    columns: [
      'id',
      'assignment_ref',
      'outcome',
      'claimed_trace_digest_prefix',
      'accept_state',
      'detail',
      'created_at',
    ],
    conflictKey: 'assignment_ref',
    keyColumns: ['assignment_ref'],
    orderColumn: 'created_at',
  },
  artanis_fleet_overseer_decisions: {
    columns: [
      'id',
      'state',
      'action_json',
      'context_json',
      'approval_gate_ref',
      'health_snapshot_ref',
      'created_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'created_at',
  },
  artanis_forum_publication_intents: ledgerSpec,
  artanis_health_snapshots: ledgerSpec,
  artanis_labor_unattended_receipts: {
    columns: ['receipt_ref', 'serialized_json', 'terminal_state', 'created_at'],
    conflictKey: 'receipt_ref',
    keyColumns: ['receipt_ref'],
    orderColumn: 'created_at',
  },
  artanis_loop_records: ledgerSpec,
  artanis_loop_ticks: ledgerSpec,
  artanis_messages: {
    columns: [
      'message_ref',
      'thread_ref',
      'caller_id',
      'author_id',
      'author_kind',
      'body',
      'metadata_json',
      'created_at',
    ],
    conflictKey: 'message_ref',
    keyColumns: ['message_ref', 'thread_ref'],
    orderColumn: 'created_at',
  },
  artanis_nexus_pylon_adapter_dispatches: ledgerSpec,
  artanis_owner_memory: {
    columns: [
      'memory_ref',
      'owner_id',
      'kind',
      'role',
      'note_category',
      'body',
      'created_at',
    ],
    conflictKey: 'memory_ref',
    keyColumns: ['memory_ref'],
    orderColumn: 'created_at',
  },
  artanis_responder_actions: {
    columns: [
      'id',
      'topic_id',
      'first_post_id',
      'question_class',
      'state',
      'proposal_json',
      'reply_post_id',
      'asked_at',
      'replied_at',
      'created_at',
      'updated_at',
      'tip_receipt_ref',
      'tip_pay_in_id',
      'tip_ladder_rung',
      'tip_ladder_reason',
      'asker_actor_ref',
      'asker_provenance',
    ],
    conflictKey: 'topic_id',
    keyColumns: ['topic_id', 'id'],
    orderColumn: 'updated_at',
  },
  artanis_responder_state: {
    columns: [
      'id',
      'scan_cursor_iso',
      'responses_today',
      'responses_day',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  artanis_responder_ticks: {
    columns: [
      'tick_ref',
      'scheduled_at',
      'scan_state',
      'scan_scanned',
      'scan_proposed',
      'scan_blocked',
      'scan_skipped',
      'scan_skipped_reason',
      'compose_state',
      'compose_considered',
      'compose_responded',
      'compose_blocked',
      'compose_tipped',
      'compose_skipped_reason',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'scheduled_at',
    keyColumns: ['scheduled_at', 'tick_ref'],
    orderColumn: 'scheduled_at',
  },
  artanis_runtime_snapshots: ledgerSpec,
  artanis_spend_decisions: {
    columns: [
      'id',
      'grant_ref',
      'state',
      'intended_amount_sat',
      'paid_amount_sat',
      'destination_source_ref',
      'recipient_ref',
      'rationale',
      'payment_ref',
      'policy_applied',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'id',
    keyColumns: ['id'],
    orderColumn: 'updated_at',
  },
  artanis_standing_spend_grants: {
    columns: [
      'grant_ref',
      'per_payout_cap_sat',
      'per_day_cap_sat',
      'authority_ref',
      'active',
      'created_at',
      'revoked_at',
    ],
    conflictKey: 'grant_ref',
    keyColumns: ['grant_ref'],
    orderColumn: 'created_at',
  },
  artanis_threads: {
    columns: [
      'thread_ref',
      'caller_id',
      'caller_kind',
      'subject_agent_ref',
      'subject_agent_kind',
      'title',
      'status',
      'source_ref',
      'metadata_json',
      'last_message_at',
      'created_at',
      'updated_at',
    ],
    conflictKey: 'thread_ref',
    keyColumns: ['thread_ref'],
    orderColumn: 'updated_at',
  },
  artanis_work_routing_proposals: ledgerSpec,
}

export type ArtanisDomainRow = Readonly<Record<string, unknown>>

class ArtanisDomainKeyColumnError extends TypeError {}

class ArtanisDomainSqlCapabilityError extends TypeError {}

const requireKeyColumn = (
  table: ArtanisDomainTable,
  keyColumn: string,
): string => {
  if (!ARTANIS_DOMAIN_TABLES[table].keyColumns.includes(keyColumn)) {
    throw new ArtanisDomainKeyColumnError(
      `artanis domain store: ${keyColumn} is not a registered key column of ${table}`,
    )
  }
  return keyColumn
}

/**
 * Validate a caller-supplied column-ownership scope (#8409) against the
 * table's registered column list. Every name must be a real column of
 * `table` — always a compile-time literal array at the call site, never
 * dynamic/user-controlled.
 */
const requireUpdateColumns = (
  table: ArtanisDomainTable,
  updateColumns: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  for (const column of updateColumns) {
    if (!ARTANIS_DOMAIN_TABLES[table].columns.includes(column)) {
      throw new ArtanisDomainKeyColumnError(
        `artanis domain store: ${column} is not a registered column of ${table}`,
      )
    }
  }
  return updateColumns
}

// ---------------------------------------------------------------------------
// Postgres store (registry-driven, single parameterized statements)
// ---------------------------------------------------------------------------

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally (the same
 * discipline as the khala-sync-server backfill cores). Every statement
 * built here is ONE parameterized statement whose dynamic text comes only
 * from the compile-time table registry — no session state, so it stays
 * Hyperdrive transaction-mode safe.
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== 'function') {
    throw new ArtanisDomainSqlCapabilityError(
      'artanis domain store requires a driver exposing unsafe(text, params)',
    )
  }
  return unsafe
}

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === 'number') return value
  if (typeof value === 'boolean') return value ? 1 : 0
  return String(value)
}

export type PostgresArtanisDomainStore = Readonly<{
  /**
   * Converge Postgres to the RESOLVED rows the authoritative D1 write
   * produced. The INSERT side always supplies every registry column (a row
   * touched by dual-write self-heals even before the backfill reaches it,
   * and re-mirroring the same row is a no-op). The `ON CONFLICT DO UPDATE`
   * side, though, only overwrites `updateColumns` when given (default: all
   * non-key columns) — #8409: two independent writers of DISJOINT columns
   * on the SAME natural key (e.g. the Artanis responder scan/compose
   * cron ticks) must each only clobber the columns they actually own, or
   * whichever writer's Postgres round trip lands LAST wins the WHOLE row
   * and silently reverts the other writer's concurrent column update.
   */
  upsertRows: (
    table: ArtanisDomainTable,
    rows: ReadonlyArray<ArtanisDomainRow>,
    updateColumns?: ReadonlyArray<string>,
  ) => Promise<void>
  /** Registry-validated key lookup (read cutover + compare mode). */
  selectRowsByKey: (
    table: ArtanisDomainTable,
    keyColumn: string,
    keys: ReadonlyArray<string | number>,
  ) => Promise<Array<ArtanisDomainRow>>
  /** Latest-N by the table's order column (read cutover + compare mode). */
  selectLatestRows: (
    table: ArtanisDomainTable,
    limit: number,
  ) => Promise<Array<ArtanisDomainRow>>
}>

export type MakePostgresArtanisDomainStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the push route.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresArtanisDomainStore = (
  deps: MakePostgresArtanisDomainStoreDependencies,
): PostgresArtanisDomainStore => {
  const withSql = async <A>(
    fn: (unsafe: UnsafeQuery) => Promise<A>,
  ): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(requireUnsafe(client.sql))
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  return {
    selectLatestRows: (table, limit) =>
      withSql(unsafe => {
        const spec = ARTANIS_DOMAIN_TABLES[table]
        return unsafe(
          `SELECT ${spec.columns.join(', ')} FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT $1`,
          [Math.max(1, Math.min(200, Math.trunc(limit)))],
        )
      }),

    selectRowsByKey: (table, keyColumn, keys) =>
      keys.length === 0
        ? Promise.resolve([])
        : withSql(unsafe => {
            const spec = ARTANIS_DOMAIN_TABLES[table]
            const column = requireKeyColumn(table, keyColumn)
            const placeholders = keys
              .map((_, index) => `$${index + 1}`)
              .join(', ')
            return unsafe(
              `SELECT ${spec.columns.join(', ')} FROM ${table} WHERE ${column} IN (${placeholders})`,
              [...keys],
            )
          }),

    upsertRows: (table, rows, updateColumns) =>
      rows.length === 0
        ? Promise.resolve()
        : withSql(async unsafe => {
            const spec = ARTANIS_DOMAIN_TABLES[table]
            const columnsSql = spec.columns.join(', ')
            const setColumns = (
              updateColumns === undefined
                ? spec.columns
                : requireUpdateColumns(table, updateColumns)
            ).filter(column => column !== spec.conflictKey)
            // #8409: the conflict clause is EITHER a scoped column-owned
            // update (never touches the other writer's columns) OR, if a
            // caller ever scopes down to just the conflict key itself, a
            // no-op update — never an empty (invalid) SET list.
            const conflictClause =
              setColumns.length === 0
                ? `ON CONFLICT (${spec.conflictKey}) DO NOTHING`
                : `ON CONFLICT (${spec.conflictKey}) DO UPDATE SET ${setColumns
                    .map(column => `${column} = EXCLUDED.${column}`)
                    .join(', ')}`
            for (const row of rows) {
              const values = spec.columns.map(column =>
                normalizeValue(row[column]),
              )
              const placeholders = values
                .map((_, index) => `$${index + 1}`)
                .join(', ')
              await unsafe(
                `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ${conflictClause}`,
                values as Array<unknown>,
              )
            }
          }),
  }
}

// ---------------------------------------------------------------------------
// The seam handle
// ---------------------------------------------------------------------------

export type ArtanisDomainHandle = Readonly<{
  /** Brand — discriminates the handle from a bare D1Database. */
  artanisDomainSeam: true
  /** The authoritative D1 database (writes and default reads). */
  d1: D1Database
  flags: ArtanisDomainFlags
  log: ArtanisDomainLog
  /** Undefined when no KHALA_SYNC_DB binding: plain-D1 degradation. */
  postgres: PostgresArtanisDomainStore | undefined
  /** Bounded-retry backoff hook (tests inject a no-op). */
  wait: (ms: number) => Promise<void>
}>

/**
 * What every artanis_* module signature takes. A plain `D1Database` keeps
 * working (no mirroring, no routing), so non-artanis call sites and tests
 * need no ceremony; `makeArtanisDatabaseForEnv` upgrades the artanis call
 * sites to the dual-write seam.
 */
export type ArtanisDatabase = D1Database | ArtanisDomainHandle

export const isArtanisDomainHandle = (
  db: ArtanisDatabase,
): db is ArtanisDomainHandle =>
  (db as { artanisDomainSeam?: unknown }).artanisDomainSeam === true

/** The authoritative D1 handle, whichever side of the union arrived. */
export const artanisAuthorityDb = (db: ArtanisDatabase): D1Database =>
  isArtanisDomainHandle(db) ? db.d1 : db

const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

export type MakeArtanisDomainHandleDependencies = Readonly<{
  d1: D1Database
  flags: ArtanisDomainFlags
  log?: ArtanisDomainLog | undefined
  postgres: PostgresArtanisDomainStore | undefined
  wait?: ((ms: number) => Promise<void>) | undefined
}>

export const makeArtanisDomainHandle = (
  deps: MakeArtanisDomainHandleDependencies,
): ArtanisDomainHandle => ({
  artanisDomainSeam: true,
  d1: deps.d1,
  flags: deps.flags,
  log: deps.log ?? (() => {}),
  postgres: deps.postgres,
  wait:
    deps.wait ??
    ((ms: number) => new Promise(resolve => setTimeout(resolve, ms))),
})

// ---------------------------------------------------------------------------
// Dual-write mirror
// ---------------------------------------------------------------------------

/**
 * Best-effort Postgres mirror after an authoritative D1 write: reads the
 * RESOLVED row(s) back from D1 by `keyColumn` and converges the Postgres
 * twins. NEVER throws — any failure (including the D1 read-back) logs the
 * `khala_sync_artanis_dual_write_failed` diagnostic and returns, preserving
 * the Artanis fail-soft persistence semantics (2d46d808) through the seam.
 * On a plain D1Database, a missing binding, or dual-write off it is a
 * no-op.
 *
 * `updateColumns` (#8409) scopes the Postgres `ON CONFLICT DO UPDATE` to
 * only the columns THIS caller just wrote in D1. Pass it whenever a table
 * has more than one independent writer touching disjoint columns of the
 * SAME natural key (e.g. the Artanis responder scan/compose cron ticks
 * both writing `artanis_responder_ticks`/`artanis_responder_state` every
 * minute) — otherwise, whichever writer's D1-read-back + Postgres-upsert
 * round trip lands last can overwrite the WHOLE row with a snapshot that
 * predates the other writer's concurrent update, silently reverting it.
 * Omit it (the default) for single-writer tables, where a full-row upsert
 * is correct and simplest.
 */
export const mirrorArtanisRows = async (
  db: ArtanisDatabase,
  table: ArtanisDomainTable,
  keyColumn: string,
  keys: ReadonlyArray<string | number>,
  updateColumns?: ReadonlyArray<string>,
): Promise<void> => {
  if (!isArtanisDomainHandle(db)) return
  const { d1, flags, log, postgres } = db
  if (postgres === undefined || !flags.dualWrite || keys.length === 0) return

  const refs = keys.map(String)
  try {
    const spec = ARTANIS_DOMAIN_TABLES[table]
    const column = requireKeyColumn(table, keyColumn)
    if (updateColumns !== undefined) requireUpdateColumns(table, updateColumns)
    const placeholders = keys.map(() => '?').join(', ')
    const result = await d1
      .prepare(
        `SELECT ${spec.columns.join(', ')} FROM ${table} WHERE ${column} IN (${placeholders})`,
      )
      .bind(...keys)
      .all<ArtanisDomainRow>()
    const rows = result.results ?? []
    if (rows.length === 0) return
    await postgres.upsertRows(table, rows, updateColumns)
  } catch (error) {
    log('khala_sync_artanis_dual_write_failed', {
      messageSafe: safeMessage(error),
      op: table,
      refs,
    })
  }
}

// ---------------------------------------------------------------------------
// Flag-routed reads
// ---------------------------------------------------------------------------

const stableStringify = (value: unknown): string =>
  JSON.stringify(value, (_key, val: unknown) =>
    val !== null && typeof val === 'object' && !Array.isArray(val)
      ? Object.fromEntries(
          Object.entries(val as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : val,
  )

/**
 * Flag-routed read: d1 (default) | postgres (bounded retry + D1 fallback)
 * | compare (read both, SERVE D1, log mismatches). Reads with no Postgres
 * twin yet pass no `readPostgres` and stay on D1 regardless of the flag —
 * the cron ticks keep D1 authority until the read-cutover evidence lands.
 */
export const artanisRead = async <A>(
  db: ArtanisDatabase,
  op: string,
  refs: ReadonlyArray<string>,
  readD1: () => Promise<A>,
  readPostgres?: (postgres: PostgresArtanisDomainStore) => Promise<A>,
): Promise<A> => {
  if (!isArtanisDomainHandle(db)) return readD1()
  const { flags, log, postgres, wait } = db
  if (
    postgres === undefined ||
    readPostgres === undefined ||
    flags.reads === 'd1'
  ) {
    return readD1()
  }

  if (flags.reads === 'postgres') {
    for (let attempt = 0; ; attempt++) {
      try {
        return await readPostgres(postgres)
      } catch (error) {
        const delay = READ_RETRY_DELAYS_MS[attempt]
        if (delay === undefined) {
          log('khala_sync_artanis_postgres_read_fallback', {
            messageSafe: safeMessage(error),
            op,
            refs,
          })
          return readD1()
        }
        log('khala_sync_artanis_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op,
          refs,
        })
        await wait(delay)
      }
    }
  }

  // compare
  const d1Result = await readD1()
  try {
    const postgresResult = await readPostgres(postgres)
    if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
      log('khala_sync_artanis_read_compare_mismatch', {
        messageSafe: 'postgres read differs from d1 authority',
        op,
        refs,
      })
    }
  } catch (error) {
    log('khala_sync_artanis_postgres_read_failed', {
      messageSafe: safeMessage(error),
      op,
      refs,
    })
  }
  return d1Result
}

// ---------------------------------------------------------------------------
// Env factory (the index.ts drop-in)
// ---------------------------------------------------------------------------

export type ArtanisDomainStoreEnv = ArtanisDomainFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeArtanisDatabaseForEnvOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: ArtanisDomainLog | undefined
  /**
   * Override the D1 authority handle. Used to COMPOSE domain mirrors at
   * call sites whose writes span domains (KS-8.10 #8321: Artanis forum
   * publication delivery writes forum_posts — passing the forum content
   * mirroring database here keeps both domains' Postgres twins fresh
   * from one code path).
   */
  d1?: D1Database | undefined
}>

const defaultLog: ArtanisDomainLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

/**
 * The production `ArtanisDatabase` factory: D1 authority + flag-gated
 * Postgres dual-write/reads. Replaces bare `openAgentsDatabase(env)` at
 * the artanis_* Worker call sites, including the six cron ticks. With no
 * KHALA_SYNC_DB binding (or everything flagged off) it returns the plain
 * D1Database — behavior-identical to before this lane.
 */
export const makeArtanisDatabaseForEnv = (
  env: ArtanisDomainStoreEnv,
  options: MakeArtanisDatabaseForEnvOptions = {},
): ArtanisDatabase => {
  const d1 = options.d1 ?? openAgentsDatabase(env)
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  const flags = artanisDomainFlagsFromEnv(env)

  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    (!flags.dualWrite && flags.reads === 'd1')
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresArtanisDomainStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makeArtanisDomainHandle({
    d1,
    flags,
    log: options.log ?? defaultLog,
    postgres,
  })
}
