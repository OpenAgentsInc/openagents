// KS-8.15 remainder (#8355): gym / mullet / blueprint / replay-clip /
// mirrorcode eval domain — D1 → Cloud SQL migration machinery for the 16
// tables the training CORE lane (#8326) left for this follow-up (khala-sync
// migration `0026_gym_evals_domain.sql`). Extends the training-domain-store.ts
// (#8326) seam exactly; see MIGRATION_PLAN.md §3.12.
//
// THE SEAM: each of these tables is written by a small typed store/repository
// factory that owns ALL its writes. The best-fit seam is a fail-soft
// read-back mirror: after the authoritative D1 write, READ BACK the affected
// rows by their live arbiter and converge-upsert the byte-exact rows into
// Postgres. Read-back mirroring keeps snapshot upserts, multi-row transitions,
// and insert-once idempotency hash-identical — gym / ladder / mirrorcode rows
// feed PUBLIC projections and must round-trip byte-exact. A mirror failure
// NEVER fails the request — it logs `khala_sync_gym_evals_dual_write_failed`
// (keys only).
//
// READS stay on D1 authority this lane (public gym projections never regress
// mid-cutover). `KHALA_SYNC_GYM_EVALS_READS` is parsed for the runbook cutover
// follow-up (where the derived-snapshot reads flip to Postgres), but every
// read stays on D1 here.
//
// Pieces:
//  1. `GymEvalsDomainWriteStore` — the typed row-level seam (`upsertRows`):
//     converge for the snapshot/state tables, insert-if-absent for the
//     insert-once tables, over the SAME arbiter keys on both stores.
//     Implementations: `makeD1GymEvalsDomainWriteStore` (real D1/SQLite) and
//     `makePostgresGymEvalsDomainStore` (KHALA_SYNC_DB Hyperdrive, sharing the
//     registry with the backfill via `@openagentsinc/khala-sync-server`), plus
//     `makeDualWriteGymEvalsDomainWriteStore` (D1 authority + fail-soft
//     Postgres mirror). One contract suite runs against BOTH concrete stores.
//  2. `makeGymEvalsDomainMirror` — the fail-soft read-back mirror
//     (`mirrorRowsByRef`).
//  3. `makeGymEvalsDomainMirrorForEnv` — the env-plumbing drop-in the wired
//     store factories use. Flag: KHALA_SYNC_GYM_EVALS_DUAL_WRITE (default ON;
//     off|0|false|disabled). With no KHALA_SYNC_DB binding everything degrades
//     to plain D1.
//
// PUBLIC-SAFETY: these rows are public-projection-bearing, but diagnostics
// reference row KEYS only (refs/ids) — never projection payloads.

import {
  GYM_EVALS_DOMAIN_TABLE_SPECS,
  upsertGymEvalsDomainRows,
  type GymEvalsDomainRow,
  type GymEvalsDomainTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { Effect } from 'effect'

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
import {
  makeD1R2HarborFullTraceArchiveStore,
  type HarborFullTraceArchiveStore,
} from './inference/gym/harbor-full-trace-archive-store'
import {
  makeD1GymLadderStore,
  type GymLadderStore,
} from './inference/gym/ladder-store'
import {
  makeD1MirrorCodeRunStore,
  type MirrorCodeRunStore,
} from './inference/gym/mirrorcode-store'
import {
  makeD1MutaliskKhalaDelegationWorkflowStore,
  type MutaliskKhalaDelegationWorkflowStore,
} from './inference/gym/mutalisk-khala-delegation-store'
import {
  makeD1GymRunProgressStore,
  type GymRunProgressStore,
} from './inference/gym/run-progress-store'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'

export type { GymEvalsDomainRow, GymEvalsDomainTable }

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type GymEvalsReadsMode = 'd1' | 'postgres' | 'compare'

export type GymEvalsFlags = Readonly<{
  dualWrite: boolean
  reads: GymEvalsReadsMode
  /**
   * #8515 WRITE cutover: `postgres` (default) makes the Postgres-backed D1
   * adapter the authoritative gym/evals store — reads AND writes leave the
   * 401-dead D1 bridge. `d1` restores the D1-authority + best-effort mirror.
   */
  writes: KhalaSyncWritesMode
}>

export type GymEvalsFlagEnv = Readonly<{
  KHALA_SYNC_GYM_EVALS_DUAL_WRITE?: string | undefined
  KHALA_SYNC_GYM_EVALS_READS?: string | undefined
  KHALA_SYNC_GYM_EVALS_WRITES?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.15-remainder migration flags from Worker vars. Dual-write
 * defaults ON (this lane lands with the mirror active wherever the binding
 * exists); reads default to D1 authority and stay there this lane — the
 * `reads` value is reserved for the runbook's cutover follow-up. Unknown read
 * values fall back to 'd1' — never fail open into an unproven read path.
 */
export const gymEvalsFlagsFromEnv = (env: GymEvalsFlagEnv): GymEvalsFlags => {
  const dualWriteRaw =
    env.KHALA_SYNC_GYM_EVALS_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_GYM_EVALS_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
    writes: parseKhalaSyncWritesMode(env.KHALA_SYNC_GYM_EVALS_WRITES),
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type GymEvalsDiagnosticEvent = 'khala_sync_gym_evals_dual_write_failed'

export type GymEvalsDiagnostic = Readonly<{
  /** The store operation, e.g. 'mirror:gym_run_progress_snapshots'. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (refs/ids). NEVER projection payloads or receipt bodies.
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type GymEvalsLog = (
  event: GymEvalsDiagnosticEvent,
  fields: GymEvalsDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

/**
 * The single D1 column the read-back mirror filters by, per table. This is the
 * column that identifies the rows a given write touched — usually the single
 * arbiter key, but a parent ref for the composite-key child tables
 * (mutalisk progress, mullet children) so one write can re-mirror the full
 * affected set.
 */
export const GYM_EVALS_MIRROR_KEY: Readonly<
  Record<GymEvalsDomainTable, string>
> = {
  gym_harbor_full_trace_archives: 'archive_ref',
  gym_ladder_leaderboard_snapshots: 'ladder_ref',
  gym_mutalisk_khala_delegation_jobs: 'run_ref',
  gym_mutalisk_khala_delegation_progress: 'run_ref',
  gym_mutalisk_khala_delegation_summaries: 'run_ref',
  gym_run_progress_snapshots: 'run_ref',
  mullet_scenarios: 'id',
  mullet_simulation_runs: 'id',
  mullet_run_hourly_results: 'run_id',
  mullet_run_candidate_modes: 'run_id',
  mullet_run_exports: 'run_id',
  blueprint_program_runs: 'id',
  blueprint_action_submissions: 'id',
  blueprint_probe_contributions: 'id',
  replay_clip_jobs: 'job_ref',
  mirrorcode_runs: 'run_id',
}

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

/**
 * The typed row-level write seam: converge upserts for the snapshot/state
 * tables, insert-if-absent for the insert-once tables. Returns how many rows
 * were touched (converge) or freshly inserted (insert-if-absent).
 */
export type GymEvalsDomainWriteStore = Readonly<{
  upsertRows: (
    table: GymEvalsDomainTable,
    rows: ReadonlyArray<GymEvalsDomainRow>,
  ) => Promise<number>
}>

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type MakePostgresGymEvalsDomainStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production, a
   * direct local URL in tests). One client per store operation; always ended.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresGymEvalsDomainStore = (
  deps: MakePostgresGymEvalsDomainStoreDependencies,
): GymEvalsDomainWriteStore => {
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
    upsertRows: async (table, rows) => {
      if (rows.length === 0) {
        return 0
      }
      return withSql(sql => upsertGymEvalsDomainRows(sql, table, rows))
    },
  }
}

// ---------------------------------------------------------------------------
// D1 implementation of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

/**
 * The D1 twin of the row-level seam (used by the contract suite and the
 * backfill parity checks). Same converge / insert-if-absent semantics over the
 * same arbiter key sets, from the shared registry.
 */
export const makeD1GymEvalsDomainWriteStore = (
  db: D1Database,
): GymEvalsDomainWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const spec = GYM_EVALS_DOMAIN_TABLE_SPECS[table]
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

export type MakeDualWriteGymEvalsDomainWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store. */
  d1: GymEvalsDomainWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: GymEvalsDomainWriteStore | undefined
  flags: GymEvalsFlags
  log?: GymEvalsLog | undefined
}>

/**
 * D1 writes first (authority); the same rows then mirror to Postgres
 * best-effort. A mirror failure never fails the write — it emits
 * `khala_sync_gym_evals_dual_write_failed` (the drift metric).
 */
export const makeDualWriteGymEvalsDomainWriteStore = (
  deps: MakeDualWriteGymEvalsDomainWriteStoreDependencies,
): GymEvalsDomainWriteStore => {
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
        log('khala_sync_gym_evals_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `upsertRows:${table}`,
          refs: rows
            .slice(0, 10)
            .map(row => String(row[GYM_EVALS_MIRROR_KEY[table]] ?? '')),
        })
      }
      return outcome
    },
  }
}

// ---------------------------------------------------------------------------
// The read-back mirror (production dual-write wiring)
// ---------------------------------------------------------------------------

export type GymEvalsDomainMirror = Readonly<{
  /** Read the rows for `refValues` back from D1 and upsert into Postgres. */
  mirrorRowsByRef: (
    table: GymEvalsDomainTable,
    refValues: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeGymEvalsDomainMirrorDependencies = Readonly<{
  db: D1Database
  postgres: GymEvalsDomainWriteStore
  log: GymEvalsLog
}>

/**
 * Fail-soft read-back mirror: reads the authoritative rows from D1 by their
 * live mirror-ref column and converge-upserts them into Postgres; every
 * failure is logged (keys only) and swallowed. NEVER throws.
 */
export const makeGymEvalsDomainMirror = (
  deps: MakeGymEvalsDomainMirrorDependencies,
): GymEvalsDomainMirror => {
  const { db, log, postgres } = deps

  return {
    mirrorRowsByRef: async (table, refValues) => {
      try {
        if (refValues.length === 0) {
          return
        }
        const key = GYM_EVALS_MIRROR_KEY[table]
        const placeholders = refValues.map(() => '?').join(', ')
        const rows = await db
          .prepare(`SELECT * FROM ${table} WHERE ${key} IN (${placeholders})`)
          .bind(...refValues)
          .all<GymEvalsDomainRow>()
        await postgres.upsertRows(table, rows.results ?? [])
      } catch (error) {
        log('khala_sync_gym_evals_dual_write_failed', {
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

export type GymEvalsStoreEnv = GymEvalsFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeGymEvalsStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable adapter client factory for the #8515 Postgres write
   * authority (tests). */
  makeD1Client?: MakeKhalaSyncWritesDatabaseOptions['makeD1Client']
  log?: GymEvalsLog | undefined
}>

/**
 * #8515 WRITE cutover: the authoritative gym/evals `D1Database` handle. When
 * `KHALA_SYNC_GYM_EVALS_WRITES=postgres` (default) AND the KHALA_SYNC_DB
 * binding exists, this is the Postgres-backed D1 adapter; otherwise plain D1.
 */
const gymEvalsWritesDatabaseForEnv = (
  env: GymEvalsStoreEnv,
  options: MakeGymEvalsStoreOptions,
): D1Database => {
  const flags = gymEvalsFlagsFromEnv(env)
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

const defaultLog: GymEvalsLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: GymEvalsStoreEnv,
  options: MakeGymEvalsStoreOptions,
): GymEvalsDomainWriteStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresGymEvalsDomainStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * The env-plumbing drop-in the wired store factories use: returns a fail-soft
 * read-back mirror when dual-write is on AND a KHALA_SYNC_DB binding exists,
 * otherwise `undefined` (the wired factory then returns its plain D1 store
 * unchanged — identical behavior, so tests without a binding are unaffected).
 */
export const makeGymEvalsDomainMirrorForEnv = (
  env: GymEvalsStoreEnv,
  options: MakeGymEvalsStoreOptions = {},
): GymEvalsDomainMirror | undefined => {
  const flags = gymEvalsFlagsFromEnv(env)
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
  return makeGymEvalsDomainMirror({
    db: openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
    log: options.log ?? defaultLog,
    postgres,
  })
}

// ---------------------------------------------------------------------------
// Store-factory drop-ins (the call-site wiring)
// ---------------------------------------------------------------------------
//
// Each `make*ForEnv` is a drop-in for the bare D1 factory at its Worker
// construction site: it wraps every write method so the affected rows mirror
// fail-soft into Postgres after the authoritative D1 write. With no
// KHALA_SYNC_DB binding (or dual-write off) the mirror is undefined and the
// plain D1 store is returned unchanged — identical behavior, so tests without
// a binding are unaffected. Mullet + blueprint + replay-clip writers are
// transactional/functional and route-threaded; their call-site wiring lands
// with the read-cutover follow-up (RUNBOOK "Gym/evals domain cutover") while
// their twins + backfill + contract coverage ship here.

/** Drop-in for `makeD1GymRunProgressStore(openAgentsDatabase(env))`. */
export const makeGymRunProgressStoreForEnv = (
  env: GymEvalsStoreEnv,
  options: MakeGymEvalsStoreOptions = {},
): GymRunProgressStore => {
  const base = makeD1GymRunProgressStore(
    gymEvalsWritesDatabaseForEnv(env, options),
  )
  const mirror = makeGymEvalsDomainMirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    upsertRunProgress: progress =>
      base.upsertRunProgress(progress).pipe(
        Effect.tap(() =>
          Effect.promise(() =>
            mirror.mirrorRowsByRef('gym_run_progress_snapshots', [
              progress.runRef,
            ]),
          ),
        ),
      ),
  }
}

/** Drop-in for `makeD1MirrorCodeRunStore(openAgentsDatabase(env))`. */
export const makeMirrorCodeRunStoreForEnv = (
  env: GymEvalsStoreEnv,
  options: MakeGymEvalsStoreOptions = {},
): MirrorCodeRunStore => {
  const base = makeD1MirrorCodeRunStore(
    gymEvalsWritesDatabaseForEnv(env, options),
  )
  const mirror = makeGymEvalsDomainMirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    upsertRun: (run, updatedAtIso) =>
      base.upsertRun(run, updatedAtIso).pipe(
        Effect.tap(() =>
          Effect.promise(() =>
            mirror.mirrorRowsByRef('mirrorcode_runs', [run.runId]),
          ),
        ),
      ),
  }
}

/** Drop-in for `makeD1GymLadderStore(openAgentsDatabase(env))`. */
export const makeGymLadderStoreForEnv = (
  env: GymEvalsStoreEnv,
  options: MakeGymEvalsStoreOptions = {},
): GymLadderStore => {
  const base = makeD1GymLadderStore(
    gymEvalsWritesDatabaseForEnv(env, options),
  )
  const mirror = makeGymEvalsDomainMirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    upsertLadder: (ladder, publishedAtIso) =>
      base.upsertLadder(ladder, publishedAtIso).pipe(
        Effect.tap(() =>
          Effect.promise(() =>
            mirror.mirrorRowsByRef('gym_ladder_leaderboard_snapshots', [
              ladder.ladderRef,
            ]),
          ),
        ),
      ),
  }
}

/**
 * Drop-in for `makeD1MutaliskKhalaDelegationWorkflowStore(...)`. Each write
 * spans the jobs + progress (+ summaries) tables keyed by run_ref, so the
 * mirror re-reads the full affected set by run_ref after the D1 write.
 */
export const makeMutaliskKhalaDelegationWorkflowStoreForEnv = (
  env: GymEvalsStoreEnv,
  options: MakeGymEvalsStoreOptions = {},
): MutaliskKhalaDelegationWorkflowStore => {
  const base = makeD1MutaliskKhalaDelegationWorkflowStore(
    gymEvalsWritesDatabaseForEnv(env, options),
  )
  const mirror = makeGymEvalsDomainMirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  const mirrorRun = (runRef: string): Effect.Effect<void> =>
    Effect.promise(async () => {
      await mirror.mirrorRowsByRef('gym_mutalisk_khala_delegation_jobs', [
        runRef,
      ])
      await mirror.mirrorRowsByRef('gym_mutalisk_khala_delegation_progress', [
        runRef,
      ])
      await mirror.mirrorRowsByRef('gym_mutalisk_khala_delegation_summaries', [
        runRef,
      ])
    })
  return {
    ...base,
    appendProgress: progress =>
      base
        .appendProgress(progress)
        .pipe(Effect.tap(() => mirrorRun(progress.runRef))),
    createRun: (job, initialProgress) =>
      base
        .createRun(job, initialProgress)
        .pipe(Effect.tap(() => mirrorRun(job.runRef))),
    saveBridgeOutput: output =>
      base
        .saveBridgeOutput(output)
        .pipe(Effect.tap(() => mirrorRun(output.job.runRef))),
  }
}

/** Drop-in for `makeD1R2HarborFullTraceArchiveStore(openAgentsDatabase(env), bucket)`. */
export const makeHarborFullTraceArchiveStoreForEnv = (
  env: GymEvalsStoreEnv,
  bucket: R2Bucket,
  options: MakeGymEvalsStoreOptions = {},
): HarborFullTraceArchiveStore => {
  const base = makeD1R2HarborFullTraceArchiveStore(
    gymEvalsWritesDatabaseForEnv(env, options),
    bucket,
  )
  const mirror = makeGymEvalsDomainMirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    putArchive: async input => {
      const result = await base.putArchive(input)
      await mirror.mirrorRowsByRef('gym_harbor_full_trace_archives', [
        result.record.archiveRef,
      ])
      return result
    },
  }
}
