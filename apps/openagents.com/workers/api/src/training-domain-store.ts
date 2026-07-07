// KS-8.15 (#8326): training domain CORE — D1 → Cloud SQL migration
// machinery for the seven `training_*` tables: `training_runs`,
// `training_windows`, `training_window_events`, `training_window_leases`,
// `training_verification_challenges`, `training_verification_events`,
// `training_trace_contributions` (khala-sync migration
// `0019_training_domain.sql`). Follows the freshest KS-8 templates
// (`agent-runtime-store.ts` #8316, `forum-content-store.ts` #8321,
// `khala-code-product-state-store.ts` #8324). The issue's remaining
// tables — `gym_*` (11), `mullet_*` (5), `blueprint_*` (3),
// `replay_clip_jobs`, `mirrorcode_runs` — move in the follow-up remainder
// lane; see MIGRATION_PLAN.md §3.12.
//
// THE SEAM: the training domain already has three typed store objects that
// own ALL writes to these tables — `TrainingAuthorityStore`
// (`training-run-window-authority.ts`), `TrainingVerificationStore`
// (`training-verification.ts`), and `TrainingTraceContributionStore`
// (`tassadar-trace-contribution-authority.ts`) — so the best-fit seam is
// a read-back mirror wrapped around those stores at their construction
// call sites (`make*ForEnv` drop-ins), NOT a statement classifier: the
// write surface is a closed set of methods, each of which knows the exact
// ref keys it touched. Every store keeps its authoritative D1 SQL
// byte-for-byte; after a successful D1 write the wrapper READS BACK the
// affected rows by their live ref arbiter (training_run_ref / window_ref /
// lease_ref / challenge_ref / contribution_ref; event ids for the two
// append-only ledgers) and converge-upserts the byte-exact rows into
// Postgres. Read-back mirroring is what keeps conditional transitions
// (`WHERE state = 'pending'`), batch UPDATE+INSERT transitions, and
// `INSERT OR IGNORE` idempotency hash-identical across stores — training
// receipts feed PUBLIC claims and must round-trip byte-exact. A mirror
// failure NEVER fails the request — it logs the typed drift diagnostic
// `khala_sync_training_dual_write_failed` (keys only).
//
// LEASE AUTHORITY: `claimLease` (the double-lease = double-payout risk)
// stays a D1-authoritative write in this lane; the Postgres twin is a
// mirror. At cutover the claim becomes a real `SELECT ... FOR UPDATE`
// row-lock transaction in Postgres — ported deliberately then (see
// RUNBOOK "Training domain cutover"), never emulated mid-migration.
//
// Pieces:
//
//  1. `TrainingDomainWriteStore` — the typed row-level seam
//     (`upsertRows`): converge upserts for the five state tables and
//     insert-if-absent for the two event ledgers, over the SAME arbiter
//     keys on both stores. Implementations:
//     `makeD1TrainingDomainWriteStore` (real D1/SQLite) and
//     `makePostgresTrainingDomainStore` (KHALA_SYNC_DB Hyperdrive,
//     sharing the column/key registry with the backfill via
//     `@openagentsinc/khala-sync-server` — one source of truth), plus
//     `makeDualWriteTrainingDomainWriteStore` (D1 authority + fail-soft
//     Postgres mirror). One behavioral contract suite runs against BOTH
//     concrete stores (`training-domain-repository.contract.test.ts`).
//
//  2. `makeTrainingDomainMirror` — the fail-soft read-back mirror
//     (`mirrorRowsByRef`).
//
//  3. `make*ForEnv` factories — drop-ins for the bare D1 factories at the
//     Worker write call sites: `makeTrainingAuthorityStoreForEnv`,
//     `makeTrainingVerificationStoreForEnv`,
//     `makeTrainingTraceContributionStoreForEnv`. Flags:
//       KHALA_SYNC_TRAINING_DUAL_WRITE (default ON; off|0|false|disabled)
//       KHALA_SYNC_TRAINING_READS     (default 'd1'; d1|compare|postgres)
//     With no KHALA_SYNC_DB binding everything degrades to plain D1.
//     Read routing in this lane covers exactly ONE scan:
//     `listClaimableWindows` — the read behind the
//     SelfServeWindowProducer.topUp cron this domain re-homes. `compare`
//     reads both, serves D1, logs
//     `khala_sync_training_read_compare_mismatch`; `postgres` serves
//     Postgres with bounded retry (50/150ms) and D1 fallback. All other
//     reads (public run summaries, proof replay bundles, activity
//     timelines) stay on D1 authority until the runbook cutover — public
//     projections never regress mid-cutover.
//
// PUBLIC-SAFETY: training rows are public-projection-bearing, but
// diagnostics still reference row KEYS only (refs/ids) — never
// projection payloads or receipt bodies.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Training domain cutover"):
// dual-write on → backfill (scripts/backfill-training.ts) → verify
// (exact counts, newest-N row hashes, window/verification event-chain
// fingerprints, lease-set fingerprint, state tallies) → compare reads →
// postgres reads → remainder lane + D1 retirement follow-up.

import {
  TRAINING_DOMAIN_TABLE_SPECS,
  upsertTrainingDomainRows,
  type SyncSql,
  type TrainingDomainRow,
  type TrainingDomainTable,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import {
  makeKhalaSyncWritesDatabase,
  parseKhalaSyncWritesMode,
  type KhalaSyncWritesMode,
  type MakeKhalaSyncWritesDatabaseOptions,
} from './khala-sync-domain-writes-database'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'
import {
  makeD1TrainingAuthorityStore,
  rowToTrainingRun,
  rowToTrainingWindow,
  rowToTrainingWindowLease,
  type TrainingAuthorityStore,
  type TrainingRunRecord,
  type TrainingRunRow,
  type TrainingWindowLeaseRecord,
  type TrainingWindowLeaseRow,
  type TrainingWindowRecord,
  type TrainingWindowRow,
} from './training-run-window-authority'
import {
  makeD1TrainingVerificationStore,
  rowToTrainingVerificationChallenge,
  type TrainingVerificationChallengeRecord,
  type TrainingVerificationRow,
  type TrainingVerificationStore,
} from './training-verification'
import {
  makeD1TrainingTraceContributionStore,
  type TrainingTraceContributionStore,
} from './tassadar-trace-contribution-authority'

export type { TrainingDomainRow, TrainingDomainTable }

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type TrainingReadsMode = 'd1' | 'postgres' | 'compare'

export type TrainingFlags = Readonly<{
  dualWrite: boolean
  reads: TrainingReadsMode
  /**
   * #8515 WRITE cutover: `postgres` (default) makes the Postgres-backed D1
   * adapter the authoritative training store — reads AND writes leave the
   * 401-dead D1 bridge. `d1` restores the pre-cutover D1-authority +
   * best-effort mirror path.
   */
  writes: KhalaSyncWritesMode
}>

export type TrainingFlagEnv = Readonly<{
  KHALA_SYNC_TRAINING_DUAL_WRITE?: string | undefined
  KHALA_SYNC_TRAINING_READS?: string | undefined
  KHALA_SYNC_TRAINING_WRITES?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.15 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding
 * exists); reads default to D1 authority until the runbook's cutover
 * sequence flips them. Unknown read values fall back to 'd1' — never
 * fail open into an unproven read path on a typo.
 *
 * `writes` defaults to 'postgres' (#8515): D1 is dead account-wide, so the
 * authoritative training reads AND writes ride the Postgres-backed D1
 * adapter. Only an explicit 'd1' restores D1 authority.
 */
export const trainingFlagsFromEnv = (env: TrainingFlagEnv): TrainingFlags => {
  const dualWriteRaw = env.KHALA_SYNC_TRAINING_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_TRAINING_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
    writes: parseKhalaSyncWritesMode(env.KHALA_SYNC_TRAINING_WRITES),
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type TrainingDiagnosticEvent =
  | 'khala_sync_training_dual_write_failed'
  | 'khala_sync_training_read_compare_mismatch'
  | 'khala_sync_training_postgres_read_failed'
  | 'khala_sync_training_postgres_read_fallback'

export type TrainingDiagnostic = Readonly<{
  /** The store operation, e.g. 'mirror:training_runs'. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (refs/ids). NEVER projection payloads or receipt bodies.
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type TrainingLog = (
  event: TrainingDiagnosticEvent,
  fields: TrainingDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

/**
 * The typed row-level write seam: converge upserts for the five state
 * tables (ref arbiters), insert-if-absent for the two event ledgers.
 * Returns how many rows were touched (state) or freshly inserted
 * (ledgers).
 */
export type TrainingDomainWriteStore = Readonly<{
  upsertRows: (
    table: TrainingDomainTable,
    rows: ReadonlyArray<TrainingDomainRow>,
  ) => Promise<number>
}>

/** The live ref arbiter used by the read-back mirror, per table. */
export const TRAINING_TABLE_MIRROR_KEY: Readonly<
  Record<TrainingDomainTable, string>
> = {
  training_runs: 'training_run_ref',
  training_trace_contributions: 'contribution_ref',
  training_verification_challenges: 'challenge_ref',
  training_verification_events: 'id',
  training_window_events: 'id',
  training_window_leases: 'lease_ref',
  training_windows: 'window_ref',
}

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresTrainingDomainStore = TrainingDomainWriteStore &
  Readonly<{
    /**
     * The claimable-windows scan twin (flag-routable read) — the read
     * behind the SelfServeWindowProducer.topUp cron this domain re-homes.
     * Same shape as the D1 scan: active, unarchived, no live lease, ranked
     * by homework kind → priority → planned_at.
     */
    listClaimableWindowRows: (
      nowIso: string,
      limit: number,
    ) => Promise<ReadonlyArray<TrainingWindowRow>>
    /**
     * CFG D1 evacuation (#8515): the public run-detail READ set behind
     * `GET /api/training/runs/:id` (and the Tassadar public run-summary
     * envelope) — `readRun` + the three per-run list reads. The Cloudflare D1
     * `d1-http` bridge 401s account-wide, so these still-live D1 reads throw
     * and 500 the run-detail route; served from Postgres they are safe.
     * Records are mapped through the SAME row mappers the D1 store uses, so a
     * served record is byte-identical to a D1-read record.
     */
    readRunRecord: (
      trainingRunRef: string,
    ) => Promise<TrainingRunRecord | undefined>
    listWindowRecordsForRun: (
      trainingRunRef: string,
      limit: number,
    ) => Promise<ReadonlyArray<TrainingWindowRecord>>
    listWindowLeaseRecordsForRun: (
      trainingRunRef: string,
      limit: number,
    ) => Promise<ReadonlyArray<TrainingWindowLeaseRecord>>
    listVerificationChallengeRecordsForRun: (
      trainingRunRef: string,
      limit: number,
    ) => Promise<ReadonlyArray<TrainingVerificationChallengeRecord>>
  }>

export type MakePostgresTrainingDomainStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the KS-8.1/8.2 stores.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

/**
 * Normalize one Postgres int8/bigint column back to the D1 number shape the
 * row mappers expect, PRESERVING null (so a mapper's `?? Default` still
 * fires). postgres.js hands int8 back as a string; `Number(...)` restores the
 * SQLite INTEGER shape. All training bigint columns are small counters — no
 * 2^53 precision risk.
 */
const bigintFieldOrNull = (value: unknown): number | null =>
  value === null || value === undefined ? null : Number(value)

/** Clamp a caller limit to the same bounds the D1 reads use (1..1000). */
const boundedLimit = (limit: number): number =>
  Math.max(1, Math.min(Math.trunc(limit), 1000))

export const makePostgresTrainingDomainStore = (
  deps: MakePostgresTrainingDomainStoreDependencies,
): PostgresTrainingDomainStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  return {
    listClaimableWindowRows: (nowIso, limit) =>
      withSql(async sql => {
        const rows: Array<TrainingWindowRow & { priority: unknown }> =
          await sql`
          SELECT w.*
            FROM training_windows w
           WHERE w.state = 'active'
             AND w.archived_at IS NULL
             AND NOT EXISTS (
               SELECT 1
                 FROM training_window_leases l
                WHERE l.window_ref = w.window_ref
                  AND l.state = 'active'
                  AND l.lease_expires_at > ${nowIso}
                  AND l.archived_at IS NULL
             )
           ORDER BY
             CASE w.homework_kind
               WHEN 'admin_dispatched_homework' THEN 3
               WHEN 'operator_planned_homework' THEN 2
               ELSE 1
             END DESC,
             w.priority DESC,
             w.planned_at ASC
           LIMIT ${Math.max(1, Math.min(limit, 100))}`
        return rows.map(row => ({
          ...row,
          // bigint columns come back driver-typed; normalize to D1 numbers.
          priority: Number(row.priority ?? 0),
        }))
      }),

    // CFG D1 evacuation (#8515): run-detail READ serving. postgres.js returns
    // int8/bigint columns as STRINGS while the row mappers (and the strict
    // `S.Number` route decoders behind them) expect numbers, so each read
    // normalizes exactly the bigint columns its mapper touches back to D1
    // number shape — null-preserving, so the mapper's `?? Default` still fires.
    readRunRecord: trainingRunRef =>
      withSql(async sql => {
        const rows = (await sql`
          SELECT * FROM training_runs
           WHERE training_run_ref = ${trainingRunRef}
             AND archived_at IS NULL
           LIMIT 1`) as Array<Record<string, unknown>>
        const row = rows[0]
        return row === undefined
          ? undefined
          : rowToTrainingRun({
              ...row,
              max_allowed_stale: bigintFieldOrNull(row['max_allowed_stale']),
              seal_publication_cadence_windows: bigintFieldOrNull(
                row['seal_publication_cadence_windows'],
              ),
            } as unknown as TrainingRunRow)
      }),

    listWindowRecordsForRun: (trainingRunRef, limit) =>
      withSql(async sql => {
        const rows = (await sql`
          SELECT * FROM training_windows
           WHERE training_run_ref = ${trainingRunRef}
             AND archived_at IS NULL
           ORDER BY planned_at DESC
           LIMIT ${boundedLimit(limit)}`) as Array<Record<string, unknown>>
        return rows.map(row =>
          rowToTrainingWindow({
            ...row,
            priority: Number(row['priority'] ?? 0),
          } as unknown as TrainingWindowRow),
        )
      }),

    listWindowLeaseRecordsForRun: (trainingRunRef, limit) =>
      withSql(async sql => {
        const rows = (await sql`
          SELECT * FROM training_window_leases
           WHERE training_run_ref = ${trainingRunRef}
             AND archived_at IS NULL
           ORDER BY claimed_at DESC
           LIMIT ${boundedLimit(limit)}`) as Array<Record<string, unknown>>
        return rows.map(row =>
          rowToTrainingWindowLease(row as unknown as TrainingWindowLeaseRow),
        )
      }),

    listVerificationChallengeRecordsForRun: (trainingRunRef, limit) =>
      withSql(async sql => {
        const rows = (await sql`
          SELECT * FROM training_verification_challenges
           WHERE training_run_ref = ${trainingRunRef}
             AND archived_at IS NULL
           ORDER BY updated_at DESC
           LIMIT ${boundedLimit(limit)}`) as Array<Record<string, unknown>>
        return rows.map(row =>
          rowToTrainingVerificationChallenge({
            ...row,
            max_attempts: bigintFieldOrNull(row['max_attempts']),
          } as unknown as TrainingVerificationRow),
        )
      }),

    upsertRows: async (table, rows) => {
      if (rows.length === 0) {
        return 0
      }
      return withSql(sql => upsertTrainingDomainRows(sql, table, rows))
    },
  }
}

// ---------------------------------------------------------------------------
// D1 implementation of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

/**
 * The D1 twin of the row-level seam (used by the contract suite and the
 * backfill parity checks). Same converge / insert-if-absent semantics
 * over the same arbiter key sets, from the shared registry.
 */
export const makeD1TrainingDomainWriteStore = (
  db: D1Database,
): TrainingDomainWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const spec = TRAINING_DOMAIN_TABLE_SPECS[table]
    let touched = 0
    for (const row of rows) {
      const values = spec.columns.map(column => {
        const value = row[column]
        return value === undefined ? null : value
      })
      const placeholders = spec.columns.map(() => '?').join(', ')
      if (spec.writeMode === 'insertIfAbsent') {
        const result = await db
          .prepare(
            `INSERT OR IGNORE INTO ${table} (${spec.columns.join(', ')}) VALUES (${placeholders})`,
          )
          .bind(...values)
          .run()
        touched += (result.meta?.changes ?? 0) > 0 ? 1 : 0
      } else {
        const setClauses = spec.columns
          .filter(column => !spec.keyColumns.includes(column))
          .map(column => `${column} = excluded.${column}`)
          .join(', ')
        await db
          .prepare(
            `INSERT INTO ${table} (${spec.columns.join(', ')}) VALUES (${placeholders})
             ON CONFLICT(${spec.keyColumns.join(', ')}) DO UPDATE SET ${setClauses}`,
          )
          .bind(...values)
          .run()
        touched += 1
      }
    }
    return touched
  },
})

// ---------------------------------------------------------------------------
// Dual-write wrapper over the row seam
// ---------------------------------------------------------------------------

export type MakeDualWriteTrainingDomainWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store. */
  d1: TrainingDomainWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: TrainingDomainWriteStore | undefined
  flags: TrainingFlags
  log?: TrainingLog | undefined
}>

/**
 * D1 writes first (authority); the same rows then mirror to Postgres
 * best-effort. A mirror failure never fails the write — it emits
 * `khala_sync_training_dual_write_failed` (the drift metric).
 */
export const makeDualWriteTrainingDomainWriteStore = (
  deps: MakeDualWriteTrainingDomainWriteStoreDependencies,
): TrainingDomainWriteStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined || !flags.dualWrite) {
    return d1
  }

  return {
    upsertRows: async (table, rows) => {
      const outcome = await d1.upsertRows(table, rows)
      try {
        await postgres.upsertRows(table, rows)
      } catch (error) {
        log('khala_sync_training_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `upsertRows:${table}`,
          refs: rows
            .slice(0, 10)
            .map(row => String(row[TRAINING_TABLE_MIRROR_KEY[table]] ?? '')),
        })
      }
      return outcome
    },
  }
}

// ---------------------------------------------------------------------------
// The read-back mirror (production dual-write wiring)
// ---------------------------------------------------------------------------

export type TrainingDomainMirror = Readonly<{
  /** Read the rows for `refValues` back from D1 and upsert into Postgres. */
  mirrorRowsByRef: (
    table: TrainingDomainTable,
    refValues: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeTrainingDomainMirrorDependencies = Readonly<{
  db: D1Database
  postgres: PostgresTrainingDomainStore
  log: TrainingLog
}>

/**
 * Fail-soft read-back mirror: reads the authoritative rows from D1 by
 * their live ref arbiter and converge-upserts them into Postgres; every
 * failure is logged (keys only) and swallowed. NEVER throws.
 */
export const makeTrainingDomainMirror = (
  deps: MakeTrainingDomainMirrorDependencies,
): TrainingDomainMirror => {
  const { db, log, postgres } = deps

  return {
    mirrorRowsByRef: async (table, refValues) => {
      try {
        if (refValues.length === 0) {
          return
        }
        const key = TRAINING_TABLE_MIRROR_KEY[table]
        const placeholders = refValues.map(() => '?').join(', ')
        const rows = await db
          .prepare(`SELECT * FROM ${table} WHERE ${key} IN (${placeholders})`)
          .bind(...refValues)
          .all<TrainingDomainRow>()
        await postgres.upsertRows(table, rows.results ?? [])
      } catch (error) {
        log('khala_sync_training_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `mirror:${table}`,
          refs: refValues.slice(0, 10),
        })
      }
    },
  }
}

// ---------------------------------------------------------------------------
// Env plumbing
// ---------------------------------------------------------------------------

export type TrainingStoreEnv = TrainingFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeTrainingStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /**
   * Injectable adapter client factory for the #8515 Postgres write authority
   * (tests). Default: the int8 postgres.js client in
   * `khala-sync-domain-writes-database`.
   */
  makeD1Client?: MakeKhalaSyncWritesDatabaseOptions['makeD1Client']
  log?: TrainingLog | undefined
  /** Bounded-retry backoff hook for routed reads (tests inject a no-op). */
  wait?: ((ms: number) => Promise<void>) | undefined
}>

/**
 * #8515 WRITE cutover: the authoritative training `D1Database` handle. When
 * `KHALA_SYNC_TRAINING_WRITES=postgres` (default) AND the KHALA_SYNC_DB
 * binding exists, this is the Postgres-backed D1 adapter — reads AND writes
 * leave the 401-dead D1 bridge. Otherwise (explicit `d1`, or no binding) it is
 * plain D1, preserving the pre-cutover dual-write behavior.
 */
export const trainingWritesDatabaseForEnv = (
  env: TrainingStoreEnv,
  options: MakeTrainingStoreOptions = {},
): D1Database => {
  const flags = trainingFlagsFromEnv(env)
  if (flags.writes === 'postgres') {
    const postgresDb = makeKhalaSyncWritesDatabase(env, {
      makeD1Client: options.makeD1Client,
    })
    if (postgresDb !== undefined) {
      return postgresDb
    }
  }
  return openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
}

const defaultLog: TrainingLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: TrainingStoreEnv,
  options: MakeTrainingStoreOptions,
): PostgresTrainingDomainStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresTrainingDomainStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

const mirrorForEnv = (
  env: TrainingStoreEnv,
  options: MakeTrainingStoreOptions,
): TrainingDomainMirror | undefined => {
  const flags = trainingFlagsFromEnv(env)
  if (!flags.dualWrite) {
    return undefined
  }
  // #8515: when writes go straight to Postgres via the D1 adapter, the
  // D1 -> Postgres read-back mirror is redundant AND would read the dead D1
  // bridge. Disable it — the adapter `base` is the single Postgres authority.
  if (flags.writes === 'postgres') {
    return undefined
  }
  const postgres = postgresStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  return makeTrainingDomainMirror({
    db: openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
    log: options.log ?? defaultLog,
    postgres,
  })
}

const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

// ---------------------------------------------------------------------------
// Domain store factories (the call-site drop-ins)
// ---------------------------------------------------------------------------

const windowRecordRefs = (
  records: ReadonlyArray<TrainingWindowRecord>,
): string => records.map(record => record.windowRef).join(',')

/**
 * Drop-in for `makeD1TrainingAuthorityStore(openAgentsDatabase(env))`:
 * every write method mirrors its affected rows fail-soft after the
 * authoritative D1 write; `listClaimableWindows` routes per
 * KHALA_SYNC_TRAINING_READS (d1 | compare | postgres with bounded retry +
 * D1 fallback) — the SelfServeWindowProducer.topUp read this domain
 * re-homes. All other reads stay on D1 authority.
 */
export const makeTrainingAuthorityStoreForEnv = (
  env: TrainingStoreEnv,
  options: MakeTrainingStoreOptions = {},
): TrainingAuthorityStore => {
  const base = makeD1TrainingAuthorityStore(
    trainingWritesDatabaseForEnv(env, options),
  )
  const flags = trainingFlagsFromEnv(env)
  const postgres = postgresStoreForEnv(env, options)
  const log = options.log ?? defaultLog
  const wait =
    options.wait ??
    ((ms: number) => new Promise(resolve => setTimeout(resolve, ms)))
  const mirror = mirrorForEnv(env, options)

  if (postgres === undefined || (mirror === undefined && flags.reads === 'd1')) {
    return base
  }

  const mirrored: TrainingAuthorityStore =
    mirror === undefined
      ? base
      : {
          ...base,
          attachRunEvidence: async run => {
            const stored = await base.attachRunEvidence(run)
            await mirror.mirrorRowsByRef('training_runs', [run.trainingRunRef])
            return stored
          },
          beginRunSealBarrier: async (trainingRunRef, nowIso) => {
            await base.beginRunSealBarrier(trainingRunRef, nowIso)
            await mirror.mirrorRowsByRef('training_runs', [trainingRunRef])
          },
          claimLease: async (lease, nowIso) => {
            const stored = await base.claimLease(lease, nowIso)
            await mirror.mirrorRowsByRef('training_window_leases', [
              lease.leaseRef,
            ])
            return stored
          },
          clearRunSealBarrier: async trainingRunRef => {
            await base.clearRunSealBarrier(trainingRunRef)
            await mirror.mirrorRowsByRef('training_runs', [trainingRunRef])
          },
          planRun: async run => {
            const stored = await base.planRun(run)
            await mirror.mirrorRowsByRef('training_runs', [run.trainingRunRef])
            return stored
          },
          planWindow: async window => {
            const stored = await base.planWindow(window)
            await mirror.mirrorRowsByRef('training_windows', [window.windowRef])
            return stored
          },
          transitionRun: async run => {
            const stored = await base.transitionRun(run)
            await mirror.mirrorRowsByRef('training_runs', [run.trainingRunRef])
            return stored
          },
          transitionWindow: async (window, event) => {
            const stored = await base.transitionWindow(window, event)
            await mirror.mirrorRowsByRef('training_windows', [window.windowRef])
            await mirror.mirrorRowsByRef('training_window_events', [event.id])
            return stored
          },
        }

  if (flags.reads === 'd1') {
    return mirrored
  }

  const postgresClaimableWindows = async (
    nowIso: string,
    limit: number,
  ): Promise<ReadonlyArray<TrainingWindowRecord>> => {
    const rows = await postgres.listClaimableWindowRows(nowIso, limit)
    return rows.map(rowToTrainingWindow)
  }

  /**
   * Route one flag-controlled read: `compare` serves D1 and logs a
   * fingerprint mismatch against Postgres; `postgres` serves Postgres with
   * bounded retry and D1 fallback (fail-soft — a run-detail read must never
   * 500 just because the Postgres serve hiccuped). Same shape as the
   * `listClaimableWindows` router above, generalized over the run-detail
   * reads (#8515).
   */
  const routePostgresRead = async <A>(
    op: string,
    fingerprint: (value: A) => string,
    servePostgres: () => Promise<A>,
    serveD1: () => Promise<A>,
  ): Promise<A> => {
    if (flags.reads === 'compare') {
      const d1Result = await serveD1()
      try {
        const postgresResult = await servePostgres()
        if (fingerprint(postgresResult) !== fingerprint(d1Result)) {
          log('khala_sync_training_read_compare_mismatch', {
            messageSafe: `fingerprint mismatch`,
            op,
            refs: [],
          })
        }
      } catch (error) {
        log('khala_sync_training_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op: `${op}:compare`,
          refs: [],
        })
      }
      return d1Result
    }

    // reads === 'postgres': bounded retry, then D1 fallback.
    for (let attempt = 0; ; attempt += 1) {
      try {
        return await servePostgres()
      } catch (error) {
        const delay = READ_RETRY_DELAYS_MS[attempt]
        if (delay === undefined) {
          log('khala_sync_training_postgres_read_fallback', {
            messageSafe: safeMessage(error),
            op,
            refs: [],
          })
          return serveD1()
        }
        log('khala_sync_training_postgres_read_failed', {
          messageSafe: safeMessage(error),
          op: `${op}:attempt${String(attempt)}`,
          refs: [],
        })
        await wait(delay)
      }
    }
  }

  const refsFingerprint = (
    records: ReadonlyArray<{ readonly [k: string]: unknown }>,
    key: string,
  ): string => records.map(record => String(record[key] ?? '')).join(',')

  return {
    ...mirrored,
    listClaimableWindows: (nowIso, limit) =>
      routePostgresRead(
        'listClaimableWindows',
        records => windowRecordRefs(records),
        () => postgresClaimableWindows(nowIso, limit),
        () => mirrored.listClaimableWindows(nowIso, limit),
      ),
    // CFG D1 evacuation (#8515): the public run-detail READ set —
    // `GET /api/training/runs/:id` and the Tassadar run-summary envelope —
    // now serves from Postgres so a dead D1 `d1-http` bridge no longer 500s it.
    readRun: trainingRunRef =>
      routePostgresRead(
        'readRun',
        run => (run === undefined ? 'none' : run.trainingRunRef),
        () => postgres.readRunRecord(trainingRunRef),
        () => mirrored.readRun(trainingRunRef),
      ),
    listWindowsForRun: (trainingRunRef, limit) =>
      routePostgresRead(
        'listWindowsForRun',
        records => refsFingerprint(records, 'windowRef'),
        () => postgres.listWindowRecordsForRun(trainingRunRef, limit),
        () => mirrored.listWindowsForRun(trainingRunRef, limit),
      ),
    listWindowLeasesForRun: (trainingRunRef, limit) =>
      routePostgresRead(
        'listWindowLeasesForRun',
        records => refsFingerprint(records, 'leaseRef'),
        () => postgres.listWindowLeaseRecordsForRun(trainingRunRef, limit),
        () => mirrored.listWindowLeasesForRun(trainingRunRef, limit),
      ),
    listVerificationChallengesForRun: (trainingRunRef, limit) =>
      routePostgresRead(
        'listVerificationChallengesForRun',
        records => refsFingerprint(records, 'challengeRef'),
        () =>
          postgres.listVerificationChallengeRecordsForRun(trainingRunRef, limit),
        () => mirrored.listVerificationChallengesForRun(trainingRunRef, limit),
      ),
  }
}

/** Drop-in for `makeD1TrainingVerificationStore(openAgentsDatabase(env))`. */
export const makeTrainingVerificationStoreForEnv = (
  env: TrainingStoreEnv,
  options: MakeTrainingStoreOptions = {},
): TrainingVerificationStore => {
  const base = makeD1TrainingVerificationStore(
    trainingWritesDatabaseForEnv(env, options),
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }

  const mirrorChallengeAndEvent = async (
    challengeRef: string,
    eventId: string,
  ): Promise<void> => {
    await mirror.mirrorRowsByRef('training_verification_challenges', [
      challengeRef,
    ])
    await mirror.mirrorRowsByRef('training_verification_events', [eventId])
  }

  return {
    ...base,
    createChallenge: async (challenge, event) => {
      const stored = await base.createChallenge(challenge, event)
      await mirrorChallengeAndEvent(challenge.challengeRef, event.id)
      return stored
    },
    leaseChallenge: async (challenge, event) => {
      const stored = await base.leaseChallenge(challenge, event)
      await mirrorChallengeAndEvent(challenge.challengeRef, event.id)
      return stored
    },
    transitionChallenge: async (challenge, event) => {
      const stored = await base.transitionChallenge(challenge, event)
      await mirrorChallengeAndEvent(challenge.challengeRef, event.id)
      return stored
    },
  }
}

/** Drop-in for `makeD1TrainingTraceContributionStore(openAgentsDatabase(env))`. */
export const makeTrainingTraceContributionStoreForEnv = (
  env: TrainingStoreEnv,
  options: MakeTrainingStoreOptions = {},
): TrainingTraceContributionStore => {
  const base = makeD1TrainingTraceContributionStore(
    trainingWritesDatabaseForEnv(env, options),
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }

  return {
    ...base,
    pairValidatorVerdict: async input => {
      const stored = await base.pairValidatorVerdict(input)
      await mirror.mirrorRowsByRef('training_trace_contributions', [
        stored.contributionRef,
      ])
      return stored
    },
    recordWorkerContribution: async record => {
      // Mirror the STORED row (the INSERT OR IGNORE winner), not the
      // attempted record — idempotent retries re-mirror the original.
      const stored = await base.recordWorkerContribution(record)
      await mirror.mirrorRowsByRef('training_trace_contributions', [
        stored.contributionRef,
      ])
      return stored
    },
  }
}
