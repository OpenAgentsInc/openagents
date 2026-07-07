// KS-8.17 (#8328): supervision long tail (Adjutant / Omni / Autopilot / ops)
// — D1 → Cloud SQL migration machinery, following the freshest KS-8 templates
// (`forge-domain-store.ts` #8327 for the store-factory wrap + read-back
// mirror, `agent-runtime-store.ts` #8316 for the row seam + flags).
//
// Domain tables (khala-sync migration `0024_supervision_longtail.sql`, 29
// tables): `adjutant_*` (10), `omni_*` (9), `autopilot_*` (6),
// `relay_health_*` (2), `backend_incident_events`, `hygiene_debt_receipts`.
//
// THE SEAM: this is a converge read-back mirror, not a statement-classifying
// database proxy. D1 stays the sole write authority; after a successful D1
// write the wrapper READS BACK the affected rows by their (composite) key and
// converge-upserts the byte-exact rows into Postgres. A mirror failure NEVER
// fails the request — it logs the typed drift diagnostic
// `khala_sync_supervision_dual_write_failed`.
//
// LIVE WIRING (this lane): the CLEAN typed-store seams whose writes are
// already funneled — the three re-homed crons and the funded-hygiene store:
//   * `makeRelayHealthStoreForEnv`         (RelayHealth.probeTick)
//   * `makeAutopilotContinuationStoreForEnv` (AutopilotContinuationPolicy.sweep)
//   * `makeAutopilotWorkStoreForEnv`       (AutopilotScheduledLaunches.dispatchDue)
//   * `makeHygieneDebtReceiptStoreForEnv`
// The `adjutant_*` / `omni_*` scattered raw-INSERT writers, the Effect-based
// onboarding store, `autopilot_token_usage` (omni-runs), and
// `backend_incident_events` keep their twins + backfill + verify from this
// lane but have their per-site live-mirror wiring filed to the decommission
// follow-up (they are read-only backfilled until then). The row-level store
// and `makeSupervisionLongtailMirrorForEnv` are the reusable seam those sites
// plug into.
//
// SECRETS (SPEC invariant 9): every column is a public-safe ref/path/digest/
// count or JSON of the same. Custody columns (`custodyColumns` in the shared
// registry — transcript/metadata/entries/result/receipt JSON) are mirrored as
// column values (the twin is column-for-column with D1) but NEVER appear in a
// diagnostic: log lines carry row KEYS only.
//
// Flags (per KS-8 convention):
//   KHALA_SYNC_SUPERVISION_DUAL_WRITE (default ON; '0'|'off'|'false'|'disabled'|'no')
//   KHALA_SYNC_SUPERVISION_READS      (default 'd1'; 'd1'|'compare'|'postgres')
// With no KHALA_SYNC_DB binding everything degrades to plain D1. What
// `reads !== 'd1'` turns on is the shadow-compare wiring below
// (`makeOmniPublicProofBundleCompareReader`) — a fail-soft, non-blocking
// read-back diff against the Postgres twin for the one public projection
// surface in this domain (`omni_public_proof_bundles`, read by both the
// redacted public handoff page and the operator JSON view). That reader
// itself NEVER serves Postgres, at any flag value — it still logs
// `khala_sync_supervision_postgres_reads_deferred` once under `postgres` so a
// premature flag flip can never silently believe IT is serving.
//
// KS-8.17 read-cutover follow-up (#8361, matching the KS-8.14 business-domain
// precedent, #8360): `KHALA_SYNC_SUPERVISION_READS=postgres` DOES unlock real
// serving through a SEPARATE reader, `makeOmniPublicProofBundlePostgresServerForEnv`
// — the bounded allowlist is this one table alone (the domain's only public,
// write-decision-free, already-shadow-compared read surface; every other
// comparable read in this domain has no reader wired at all). Fail-soft: a
// Postgres error (or `reads !== 'postgres'`, or no Postgres binding) returns
// `undefined` and the caller falls back to the normal D1-served
// `readOmniPublicProofBundleById` path. A genuine "not found" result from
// Postgres IS trusted and served directly (no D1 re-check) — that is the
// entire point of real serving; only a thrown error defers to D1.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Supervision long-tail cutover"):
// dual-write on → backfill
// (khala-sync-server scripts/backfill-supervision-longtail.ts) → second sweep
// → --verify (exact counts, tallies, idempotency-key-set equality, public
// proof-bundle digests, newest-N row hashes) → compare reads (this file's
// shadow-compare reader) → a genuinely silent soak → read/write cutover +
// D1 drop in a further follow-up.

import {
  noopCompareSoakMetrics,
  normalizeSupervisionLongtailValue,
  requireSupervisionLongtailUnsafe,
  SUPERVISION_LONGTAIL_TABLE_SPECS,
  upsertSupervisionLongtailRows,
  type CompareSoakMetrics,
  type SupervisionLongtailRow,
  type SupervisionLongtailTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  makeD1AutopilotContinuationStore,
  type AutopilotContinuationStore,
} from './autopilot-continuation-policy'
import {
  makeD1AutopilotWorkStore,
  type AutopilotWorkStore,
} from './autopilot-work-routes'
import {
  makeD1HygieneDebtReceiptStore,
  type HygieneDebtReceiptStore,
} from './hygiene-debt-receipt-store'
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
import {
  rowToRecord as omniPublicProofBundleRowToRecord,
  type OmniPublicProofBundleRecord,
  type ProofBundleRow as OmniPublicProofBundleRow,
} from './omni-public-proof-bundles'
import {
  makeD1RelayHealthStore,
  type RelayHealthStore,
} from './relay-health'
import { openAgentsDatabase } from './runtime'

export type { SupervisionLongtailRow, SupervisionLongtailTable }

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type SupervisionLongtailReadsMode = 'd1' | 'postgres' | 'compare'

export type SupervisionLongtailFlags = Readonly<{
  dualWrite: boolean
  reads: SupervisionLongtailReadsMode
  /**
   * #8515 WRITE cutover: `postgres` (default) makes the Postgres-backed D1
   * adapter the authoritative supervision store — reads AND writes leave the
   * 401-dead D1 bridge. `d1` restores the D1-authority + best-effort mirror.
   */
  writes: KhalaSyncWritesMode
}>

export type SupervisionLongtailFlagEnv = Readonly<{
  KHALA_SYNC_SUPERVISION_DUAL_WRITE?: string | undefined
  KHALA_SYNC_SUPERVISION_READS?: string | undefined
  KHALA_SYNC_SUPERVISION_WRITES?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.17 migration flags from Worker vars. Dual-write defaults ON
 * (this lane lands with the mirror active wherever the binding exists); reads
 * default to D1 authority — read flips are EPIC-GATED ops decisions (#8282),
 * never a code default. Unknown read values fall back to 'd1'.
 */
export const supervisionLongtailFlagsFromEnv = (
  env: SupervisionLongtailFlagEnv,
): SupervisionLongtailFlags => {
  const dualWriteRaw =
    env.KHALA_SYNC_SUPERVISION_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_SUPERVISION_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
    writes: parseKhalaSyncWritesMode(env.KHALA_SYNC_SUPERVISION_WRITES),
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type SupervisionLongtailDiagnosticEvent =
  | 'khala_sync_supervision_dual_write_failed'
  | 'khala_sync_supervision_read_compare_mismatch'
  | 'khala_sync_supervision_read_compare_failed'
  | 'khala_sync_supervision_postgres_reads_deferred'
  | 'khala_sync_supervision_postgres_read_serve_failed'

export type SupervisionLongtailDiagnostic = Readonly<{
  /** The mirrored table or read operation, e.g. 'mirror:relay_health_probes'. */
  op: string
  /** Public-safe refs — row KEYS only (ids/refs). NEVER a custody column. */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no values). */
  messageSafe: string
}>

export type SupervisionLongtailLog = (
  event: SupervisionLongtailDiagnosticEvent,
  fields: SupervisionLongtailDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

/**
 * The typed row-level write seam: converge upserts (composite-PK arbiter, D1
 * snapshot wins) for all 29 tables. Returns how many rows were touched. One
 * behavioral contract suite runs against BOTH concrete implementations
 * (`supervision-longtail-domain-repository.contract.test.ts`).
 */
export type SupervisionLongtailWriteStore = Readonly<{
  upsertRows: (
    table: SupervisionLongtailTable,
    rows: ReadonlyArray<SupervisionLongtailRow>,
  ) => Promise<number>
}>

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresSupervisionLongtailStore = SupervisionLongtailWriteStore &
  Readonly<{
    /** Run one read-only statement on the twin (compare shadow + verify). */
    queryRows: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }>

export type MakePostgresSupervisionLongtailStoreDependencies = Readonly<{
  /** Acquire a transaction-mode-safe SQL client; always ended, even on error. */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresSupervisionLongtailStore = (
  deps: MakePostgresSupervisionLongtailStoreDependencies,
): PostgresSupervisionLongtailStore => {
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
    queryRows: (text, params) =>
      withSql(async sql =>
        requireSupervisionLongtailUnsafe(sql)(text, [...params]),
      ),
    upsertRows: (table, rows) =>
      withSql(sql => upsertSupervisionLongtailRows(sql, table, rows)),
  }
}

// ---------------------------------------------------------------------------
// D1 implementation of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

export const makeD1SupervisionLongtailWriteStore = (
  db: D1Database,
): SupervisionLongtailWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const spec = SUPERVISION_LONGTAIL_TABLE_SPECS[table]
    const setClauses = spec.columns
      .filter(column => !spec.keyColumns.includes(column))
      .map(column => `${column} = excluded.${column}`)
      .join(', ')
    const updateClause =
      setClauses.length === 0 ? 'DO NOTHING' : `DO UPDATE SET ${setClauses}`
    let touched = 0
    for (const row of rows) {
      const values = spec.columns.map(column =>
        normalizeSupervisionLongtailValue(row[column]),
      )
      const placeholders = spec.columns.map(() => '?').join(', ')
      await db
        .prepare(
          `INSERT INTO ${table} (${spec.columns.join(', ')}) VALUES (${placeholders})
           ON CONFLICT(${spec.keyColumns.join(', ')}) ${updateClause}`,
        )
        .bind(...values)
        .run()
      touched += 1
    }
    return touched
  },
})

// ---------------------------------------------------------------------------
// The read-back mirror
// ---------------------------------------------------------------------------

/** One composite key: values in the table's `keyColumns` order. */
export type SupervisionLongtailKey = ReadonlyArray<string>

export type SupervisionLongtailMirror = Readonly<{
  /** Read the rows for the composite keys back from D1 → Postgres. */
  mirrorRowsByKey: (
    table: SupervisionLongtailTable,
    keys: ReadonlyArray<SupervisionLongtailKey>,
  ) => Promise<void>
  /**
   * Read every row matching a bounded equality scan back from D1 → Postgres
   * (e.g. every row for a work_order_ref). `refs` overrides the diagnostic
   * refs.
   */
  mirrorRowsWhere: (
    table: SupervisionLongtailTable,
    whereColumns: ReadonlyArray<string>,
    values: ReadonlyArray<string>,
    refs?: ReadonlyArray<string>,
  ) => Promise<void>
  /**
   * Converge a retention prune onto the twin: DELETE rows whose `column` is
   * strictly less than `cutoff`. Fail-soft (retention lag on the twin never
   * fails the request). Used by the RelayHealth.probeTick prune.
   */
  pruneRowsOlderThan: (
    table: SupervisionLongtailTable,
    column: string,
    cutoff: string,
  ) => Promise<void>
}>

export type MakeSupervisionLongtailMirrorDependencies = Readonly<{
  db: D1Database
  postgres: PostgresSupervisionLongtailStore
  log: SupervisionLongtailLog
}>

/** Bounded read-back scan size (per-ref row families are small). */
const MIRROR_SCAN_LIMIT = 500

/**
 * Fail-soft read-back mirror: every method reads the authoritative rows from
 * D1 and converge-upserts them into Postgres; every failure is logged (keys
 * only) and swallowed. NEVER throws.
 */
export const makeSupervisionLongtailMirror = (
  deps: MakeSupervisionLongtailMirrorDependencies,
): SupervisionLongtailMirror => {
  const { db, log, postgres } = deps

  const guarded = async (
    op: string,
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> => {
    try {
      await run()
    } catch (error) {
      log('khala_sync_supervision_dual_write_failed', {
        messageSafe: safeMessage(error),
        op,
        refs: refs.slice(0, 10),
      })
    }
  }

  return {
    mirrorRowsByKey: (table, keys) =>
      keys.length === 0
        ? Promise.resolve()
        : guarded(
            `mirror:${table}`,
            keys.slice(0, 10).map(key => key.join('/')),
            async () => {
              const keyColumns = SUPERVISION_LONGTAIL_TABLE_SPECS[table].keyColumns
              const tuple = `(${keyColumns.map(column => `${column} = ?`).join(' AND ')})`
              const where = keys.map(() => tuple).join(' OR ')
              const rows = await db
                .prepare(`SELECT * FROM ${table} WHERE ${where}`)
                .bind(...keys.flat())
                .all<SupervisionLongtailRow>()
              await postgres.upsertRows(table, rows.results ?? [])
            },
          ),

    mirrorRowsWhere: (table, whereColumns, values, refs) =>
      guarded(
        `mirror:${table}:scan`,
        refs ?? values.map(String),
        async () => {
          const where = whereColumns
            .map(column => `${column} = ?`)
            .join(' AND ')
          const rows = await db
            .prepare(
              `SELECT * FROM ${table} WHERE ${where} LIMIT ${MIRROR_SCAN_LIMIT}`,
            )
            .bind(...values)
            .all<SupervisionLongtailRow>()
          await postgres.upsertRows(table, rows.results ?? [])
        },
      ),

    pruneRowsOlderThan: (table, column, cutoff) =>
      guarded(`prune:${table}`, [`${column}<${cutoff}`], async () => {
        await postgres.queryRows(`DELETE FROM ${table} WHERE ${column} < $1`, [
          cutoff,
        ])
      }),
  }
}

// ---------------------------------------------------------------------------
// Env plumbing
// ---------------------------------------------------------------------------

export type SupervisionLongtailStoreEnv = SupervisionLongtailFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeSupervisionLongtailStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable adapter client factory for the #8515 Postgres write
   * authority (tests). */
  makeD1Client?: MakeKhalaSyncWritesDatabaseOptions['makeD1Client']
  log?: SupervisionLongtailLog | undefined
  /** D1 handle override (call sites that already hold a proxied database). */
  db?: D1Database | undefined
  /** Compare-mode soak metrics override (tests inject a collector). */
  metrics?: CompareSoakMetrics | undefined
}>

const defaultLog: SupervisionLongtailLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions,
): PostgresSupervisionLongtailStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresSupervisionLongtailStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

type SupervisionLongtailRuntime = Readonly<{
  db: D1Database
  flags: SupervisionLongtailFlags
  log: SupervisionLongtailLog
  mirror: SupervisionLongtailMirror | undefined
  /**
   * The read-only twin handle for shadow-compare reads, present whenever a
   * Postgres binding exists AND `KHALA_SYNC_SUPERVISION_READS` is not `d1`
   * (independent of dual-write) — same gate as every other KS-8 domain
   * store's `compareStore` (see `forge-domain-store.ts`).
   */
  compareStore: PostgresSupervisionLongtailStore | undefined
  /** Compare-mode soak observability recorder (no-op when unbound). */
  metrics: CompareSoakMetrics
}>

const runtimeForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions,
): SupervisionLongtailRuntime => {
  const flags = supervisionLongtailFlagsFromEnv(env)
  // #8515 WRITE cutover: when writes=postgres (default) and the binding
  // exists, the authoritative handle is the Postgres-backed D1 adapter — reads
  // AND writes leave the dead D1 bridge. `options.db` overrides still win (call
  // sites holding a proxied handle). Every supervision column is a public-safe
  // ref/path/digest (SPEC invariant 9), so the full-row adapter path carries no
  // secret material.
  const postgresWritesDb =
    options.db === undefined && flags.writes === 'postgres'
      ? makeKhalaSyncWritesDatabase(env, { makeD1Client: options.makeD1Client })
      : undefined
  const db =
    options.db ??
    postgresWritesDb ??
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const log = options.log ?? defaultLog
  const postgres = postgresStoreForEnv(env, options)
  // The durable Analytics Engine soak sink was removed with the account-level
  // Analytics Engine feature (#8516); the default recorder is a no-op and the
  // per-call `khala_sync_*_compare_mismatch` diagnostics are unaffected.
  const metrics = options.metrics ?? noopCompareSoakMetrics
  return {
    compareStore:
      postgres !== undefined && flags.reads !== 'd1' ? postgres : undefined,
    db,
    flags,
    log,
    metrics,
    // When writes go straight to Postgres via the adapter, the D1 -> Postgres
    // read-back mirror is redundant AND would read the dead D1 bridge.
    mirror:
      postgres !== undefined &&
      flags.dualWrite &&
      postgresWritesDb === undefined
        ? makeSupervisionLongtailMirror({ db, log, postgres })
        : undefined,
  }
}

/**
 * The generic seam entry point for the not-yet-wired scattered writers
 * (adjutant_* / omni_* / autopilot_token_usage / backend_incident_events /
 * onboarding) — returns the read-back mirror, or undefined when no binding /
 * dual-write off. Those call sites (see the decommission follow-up) invoke
 * `mirror.mirrorRowsByKey(table, [key])` right after their D1 write.
 */
export const makeSupervisionLongtailMirrorForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions = {},
): SupervisionLongtailMirror | undefined =>
  runtimeForEnv(env, options).mirror

// ---------------------------------------------------------------------------
// Domain store factories (the call-site drop-ins for the wired clean seams)
// ---------------------------------------------------------------------------

/**
 * Drop-in for `makeD1RelayHealthStore(openAgentsDatabase(env))` — the
 * RelayHealth.probeTick cron store. insertProbe/insertTransition mirror the
 * new row by id; the retention prunes converge onto the twin.
 */
export const makeRelayHealthStoreForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions = {},
): RelayHealthStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1RelayHealthStore(runtime.db)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    insertProbe: async record => {
      await base.insertProbe(record)
      await mirror.mirrorRowsByKey('relay_health_probes', [[record.id]])
    },
    insertTransition: async event => {
      await base.insertTransition(event)
      await mirror.mirrorRowsByKey('relay_health_transitions', [[event.id]])
    },
    pruneProbesBefore: async beforeIso => {
      await base.pruneProbesBefore(beforeIso)
      await mirror.pruneRowsOlderThan(
        'relay_health_probes',
        'probed_at',
        beforeIso,
      )
    },
    pruneTransitionsBefore: async beforeIso => {
      await base.pruneTransitionsBefore(beforeIso)
      await mirror.pruneRowsOlderThan(
        'relay_health_transitions',
        'occurred_at',
        beforeIso,
      )
    },
  }
}

/**
 * Drop-in for `makeD1AutopilotContinuationStore(openAgentsDatabase(env))` —
 * the AutopilotContinuationPolicy.sweep cron store.
 */
export const makeAutopilotContinuationStoreForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions = {},
): AutopilotContinuationStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1AutopilotContinuationStore(runtime.db)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    claimContinuationAttempt: async record => {
      const result = await base.claimContinuationAttempt(record)
      // The dedupe (run_id, attempt) miss inserts no row under this id; the
      // read-back then mirrors zero rows — the winning row was mirrored when
      // it was first written.
      await mirror.mirrorRowsByKey('autopilot_continuation_events', [
        [record.id],
      ])
      return result
    },
    markContinuationAttemptFailed: async (id, reasonRef) => {
      await base.markContinuationAttemptFailed(id, reasonRef)
      await mirror.mirrorRowsByKey('autopilot_continuation_events', [[id]])
    },
    upsertPolicy: async record => {
      const result = await base.upsertPolicy(record)
      await mirror.mirrorRowsByKey('autopilot_continuation_policies', [
        [result.userId],
      ])
      return result
    },
  }
}

/**
 * Drop-in for `makeD1AutopilotWorkStore(openAgentsDatabase(env))` — the
 * AutopilotScheduledLaunches.dispatchDue cron store (dispatchDue calls
 * recordScheduledLaunchTransition / recordPylonAssignmentDispatch). Every
 * write method carries `workOrderRef`, so one bounded scan mirrors every
 * touched row for that order; the closeout receipt mirrors by closeout_ref.
 */
export const makeAutopilotWorkStoreForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions = {},
): AutopilotWorkStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1AutopilotWorkStore(runtime.db)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  const mirrorOrder = (workOrderRef: string) =>
    mirror.mirrorRowsWhere(
      'autopilot_work_orders',
      ['work_order_ref'],
      [workOrderRef],
    )
  const wrapped: AutopilotWorkStore = {
    ...base,
    createWorkOrder: async record => {
      const result = await base.createWorkOrder(record)
      await mirrorOrder(result.record.workOrderRef)
      return result
    },
    recordBuyerPaymentProof: async input => {
      const result = await base.recordBuyerPaymentProof(input)
      await mirrorOrder(input.workOrderRef)
      return result
    },
    recordExecutionCloseout: async input => {
      const result = await base.recordExecutionCloseout(input)
      await mirrorOrder(input.workOrderRef)
      return result
    },
    recordPylonAssignmentDispatch: async input => {
      const result = await base.recordPylonAssignmentDispatch(input)
      await mirrorOrder(input.workOrderRef)
      return result
    },
    recordReviewDecision: async input => {
      const result = await base.recordReviewDecision(input)
      await mirrorOrder(input.workOrderRef)
      return result
    },
    recordScheduledLaunchTransition: async input => {
      const result = await base.recordScheduledLaunchTransition(input)
      await mirrorOrder(input.workOrderRef)
      return result
    },
  }
  if (base.recordDecisionCloseoutReceipt !== undefined) {
    const recordDecisionCloseoutReceipt = base.recordDecisionCloseoutReceipt
    return {
      ...wrapped,
      recordDecisionCloseoutReceipt: async receipt => {
        await recordDecisionCloseoutReceipt(receipt)
        await mirror.mirrorRowsByKey('autopilot_decision_closeout_receipts', [
          [receipt.closeoutRef],
        ])
        await mirrorOrder(receipt.workOrderRef)
      },
    }
  }
  return wrapped
}

/**
 * Drop-in for `makeD1HygieneDebtReceiptStore(openAgentsDatabase(env))` — the
 * funded-hygiene payable-receipt store. create/markRetired mirror by
 * debt_receipt_key.
 */
export const makeHygieneDebtReceiptStoreForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions = {},
): HygieneDebtReceiptStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1HygieneDebtReceiptStore(runtime.db)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    create: async input => {
      const result = await base.create(input)
      await mirror.mirrorRowsByKey('hygiene_debt_receipts', [
        [result.record.debtReceiptKey],
      ])
      return result
    },
    markRetired: async (debtReceiptKeyRef, settlementReceiptRef, nowIso) => {
      const result = await base.markRetired(
        debtReceiptKeyRef,
        settlementReceiptRef,
        nowIso,
      )
      await mirror.mirrorRowsByKey('hygiene_debt_receipts', [
        [debtReceiptKeyRef],
      ])
      return result
    },
  }
}

// ---------------------------------------------------------------------------
// Read-compare shadow (the KS-8.17 follow-up piece deferred by the parent
// lane, #8361): D1 stays the ONLY store that ever serves a response. When
// `KHALA_SYNC_SUPERVISION_READS` is `compare` or `postgres`, this fires a
// fail-soft, non-blocking shadow read against the Postgres twin after the
// real D1-served read and diffs it column-for-column, logging
// `khala_sync_supervision_read_compare_mismatch` /
// `khala_sync_supervision_read_compare_failed`. A `postgres` flag value NEVER
// serves Postgres here — real serving stays deferred to a follow-up with a
// genuinely silent soak; the flag only widens which values trigger the shadow
// compare (`khala_sync_supervision_postgres_reads_deferred`, logged once).
// ---------------------------------------------------------------------------

/** Column-for-column equality using the same normalizer the mirror uses. */
const supervisionLongtailRowsEqual = (
  table: SupervisionLongtailTable,
  left: Record<string, unknown>,
  right: Record<string, unknown>,
): boolean => {
  const spec = SUPERVISION_LONGTAIL_TABLE_SPECS[table]
  for (const column of spec.columns) {
    if (
      String(normalizeSupervisionLongtailValue(left[column])) !==
      String(normalizeSupervisionLongtailValue(right[column]))
    ) {
      return false
    }
  }
  return true
}

/**
 * The `omni_public_proof_bundles` shadow-compare reader — the one public
 * projection surface in this domain (the redacted handoff page and the
 * operator JSON view both key off `readOmniPublicProofBundleById`).
 * `undefined` when no Postgres binding or `KHALA_SYNC_SUPERVISION_READS=d1`
 * (the default). The returned function is fail-soft (NEVER throws/rejects —
 * every error is caught and logged as a diagnostic) but call sites MUST
 * `await` it inline before returning their response (same discipline as
 * `forge-domain-store.ts`'s `compareListRefs`): a Cloudflare Worker may
 * cancel an un-awaited async tail after the response is sent, so a bare
 * fire-and-forget call here could silently never run.
 */
export const makeOmniPublicProofBundleCompareReader = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions = {},
): ((id: string) => Promise<void>) | undefined => {
  const runtime = runtimeForEnv(env, options)
  const { compareStore, db, flags, log, metrics } = runtime
  if (compareStore === undefined) {
    return undefined
  }
  let deferredLogged = false
  const op = 'omni_public_proof_bundles:readById'
  return async (id: string): Promise<void> => {
    if (flags.reads === 'postgres' && !deferredLogged) {
      deferredLogged = true
      log('khala_sync_supervision_postgres_reads_deferred', {
        messageSafe:
          'this shadow-compare reader itself never serves Postgres (by design); the SEPARATE makeOmniPublicProofBundlePostgresServerForEnv reader (#8361 follow-up) is what actually serves this table under postgres mode',
        op,
        refs: [id],
      })
    }
    try {
      const d1Row = await db
        .prepare(
          `SELECT * FROM omni_public_proof_bundles WHERE id = ? AND archived_at IS NULL LIMIT 1`,
        )
        .bind(id)
        .first<Record<string, unknown>>()
      const pgRows = await compareStore.queryRows(
        `SELECT * FROM omni_public_proof_bundles WHERE id = $1 AND archived_at IS NULL LIMIT 1`,
        [id],
      )
      const pgRow = pgRows[0]
      const bothMissing = d1Row === null && pgRow === undefined
      const rowsMatch =
        bothMissing ||
        (d1Row !== null &&
          pgRow !== undefined &&
          supervisionLongtailRowsEqual(
            'omni_public_proof_bundles',
            d1Row,
            pgRow,
          ))
      if (!rowsMatch) {
        log('khala_sync_supervision_read_compare_mismatch', {
          messageSafe: `public proof bundle differs: d1=${d1Row === null ? 'missing' : 'present'} postgres=${pgRow === undefined ? 'missing' : 'present'}`,
          op,
          refs: [id],
        })
        metrics.record({ domain: 'supervision', outcome: 'mismatch', readKind: op })
      } else {
        metrics.record({ domain: 'supervision', outcome: 'match', readKind: op })
      }
    } catch (error) {
      log('khala_sync_supervision_read_compare_failed', {
        messageSafe: safeMessage(error),
        op,
        refs: [id],
      })
      metrics.record({ domain: 'supervision', outcome: 'error', readKind: op })
    }
  }
}

// ---------------------------------------------------------------------------
// Real Postgres read serving (KS-8.17 read-cutover follow-up, #8361) — the
// bounded, single-table allowlist analog of the KS-8.14 business-domain
// precedent (#8360, `BUSINESS_DOMAIN_POSTGRES_SERVED_READ_TABLES`). This
// domain has exactly one public, write-decision-free, already-shadow-compared
// read surface (`omni_public_proof_bundles`), so the "allowlist" is simply:
// this reader exists, every other comparable read in this domain has none.
// ---------------------------------------------------------------------------

/** The genuinely-final answer from Postgres: `null` IS a real "not found". */
export type OmniPublicProofBundlePostgresServeResult = Readonly<{
  record: OmniPublicProofBundleRecord | null
}>

/**
 * The `omni_public_proof_bundles` real-serve reader — `undefined` unless a
 * Postgres binding exists AND `KHALA_SYNC_SUPERVISION_READS=postgres`.
 * Distinct from `makeOmniPublicProofBundleCompareReader` (the shadow-compare
 * diagnostic above, which never serves and keeps its own well-tested
 * behavior untouched by this addition). Fail-soft: a Postgres query error
 * returns `undefined` so the caller falls back to the normal
 * `readOmniPublicProofBundleById` (D1) path — serving can never fail a
 * request, it can only fail to happen. A successful Postgres query that
 * finds no row is NOT a failure: it returns `{ record: null }`, a genuine,
 * final "not found" the caller should serve directly (no D1 re-check) —
 * that is the entire point of real serving.
 */
export const makeOmniPublicProofBundlePostgresServerForEnv = (
  env: SupervisionLongtailStoreEnv,
  options: MakeSupervisionLongtailStoreOptions = {},
): ((id: string) => Promise<OmniPublicProofBundlePostgresServeResult | undefined>) | undefined => {
  const runtime = runtimeForEnv(env, options)
  const { compareStore, flags, log } = runtime
  if (flags.reads !== 'postgres' || compareStore === undefined) {
    return undefined
  }
  const op = 'omni_public_proof_bundles:readById'
  return async (
    id: string,
  ): Promise<OmniPublicProofBundlePostgresServeResult | undefined> => {
    try {
      const rows = await compareStore.queryRows(
        `SELECT * FROM omni_public_proof_bundles WHERE id = $1 AND archived_at IS NULL LIMIT 1`,
        [id],
      )
      const row = rows[0]
      return {
        record:
          row === undefined
            ? null
            : omniPublicProofBundleRowToRecord(row as OmniPublicProofBundleRow),
      }
    } catch (error) {
      log('khala_sync_supervision_postgres_read_serve_failed', {
        messageSafe: safeMessage(error),
        op,
        refs: [id],
      })
      return undefined
    }
  }
}
