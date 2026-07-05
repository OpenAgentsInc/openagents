// KS-8.14 (#8325): business funnel / orders / referrals domain — D1 →
// Cloud SQL migration machinery. Follows the freshest KS-8 templates:
// the KS-8.10 mirroring-database seam (`forum/forum-content-store.ts`,
// #8321) and the KS-8.7 money discipline (`billing-store.ts`, #8318).
//
// Domain tables (khala-sync migration `0023_business_funnel.sql`,
// THIRTY-TWO live tables): business signup/fulfillment/pipeline/
// commitments/affiliates, funnel events, service promises + fulfillment
// loop receipts + escalation pages, checkout kickoffs, starter credits,
// software orders + triage + fulfillment artifacts/feedback + GitHub
// write-authority receipts, referral invites/attributions + the four
// consume-once attribution tables + workflow events, viral agent funnel,
// QA swarm engagements, promise transition receipts, buy-mode
// campaigns/jobs/alerts, customer-one cohort rows.
//
// THE SEAM: this domain's ~58 write statements are spread across ~22
// route/domain modules as plain `db.prepare(...)` SQL — there is no single
// store object to decorate. That makes `D1Database` itself the repository
// interface (the KS-8.10 conclusion), so the production wiring is a
// MIRRORING D1Database (`businessDomainDatabaseForEnv`) dropped in at the
// `openAgentsDatabase(env)` boundaries that feed the domain's writers.
// Every writer keeps its authoritative D1 SQL byte-for-byte; after a
// successful D1 write to a scoped table the proxy READS BACK the affected
// row(s) by the statement's addressed key and converge-upserts the exact
// D1 rows into Postgres. Read-back mirroring is what keeps in-place
// UPDATEs (pipeline stages, fulfillment status, buy-mode spend counters,
// attribution policy_state) hash-identical across stores, and it makes
// INSERT OR IGNORE dedupes mirror as no-ops (a discarded insert reads
// back the surviving row — or, addressed by its bound PK, zero rows).
//
// Two generalizations over the forum classifier, both driven by the
// shared registry in `@openagentsinc/khala-sync-server`:
//
//   1. LOOKUP COLUMNS. Some live statements address rows by a UNIQUE /
//      partial-unique-active column instead of the PK
//      (`order_triage_records … WHERE software_order_id = ?`,
//      `business_affiliate_attributions … WHERE
//      business_signup_request_id = ?`). The classifier accepts equality
//      on any registered lookup column and the mirror reads back by that
//      column; the converge upsert still lands on the true PK.
//
//   2. `INSERT … ON CONFLICT(col) DO UPDATE` (customer-one cohort rows,
//      business signup fulfillments). The mirror reads back by the
//      CONFLICT column — load-bearing for
//      `business_signup_fulfillments`, whose conflict target is the
//      UNIQUE `business_signup_request_id`: on conflict the surviving
//      row keeps its ORIGINAL id, so a PK read-back would find nothing
//      and the mirror would go stale.
//
// MONEY/ATTRIBUTION DISCIPLINE (MIGRATION_PLAN §3.11): D1 is the SOLE
// authority. Referral attribution uniqueness keys feed payouts (KS-8.8)
// — the mirror COPIES accepted D1 rows and never re-makes a consume-once
// or idempotency decision. The fulfillment-loop escalation pager and the
// starter-credit window-cap trigger evaluate against exactly ONE store
// (D1): nothing in Postgres feeds an evaluator, so dual-write can never
// double-page. A mirror failure NEVER fails the request — it logs the
// typed drift diagnostic `khala_sync_business_dual_write_failed`.
//
// Flags:
//   KHALA_SYNC_BUSINESS_DUAL_WRITE (default ON; off|0|false|disabled)
//   KHALA_SYNC_BUSINESS_READS      (default 'd1'; d1|compare|postgres)
// With no KHALA_SYNC_DB binding everything degrades to plain D1.
// `compare` shadow-runs scoped-table SELECTs against Postgres, SERVES D1,
// and logs `khala_sync_business_read_compare_mismatch`. `postgres` read
// serving is DEFERRED to the read-cutover follow-up (the domain read
// surface — funnel dashboards, pipeline queues, referral feeds, order
// lists — is wide, not one bounded scan): the flag behaves as `compare`
// and logs `khala_sync_business_postgres_reads_deferred` once, so a
// premature flag flip can never serve an unproven read path.
//
// PUBLIC-SAFETY: diagnostics reference row KEYS and statement heads only
// — never contact emails, request bodies, or receipt payloads.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Business funnel domain
// cutover"): dual-write on → backfill
// (scripts/backfill-business.ts) → catch-up sweep → --verify (attribution
// set equality + promise-receipt hash equality + funnel cohort counts +
// money sums) → compare reads → read cutover + remainder wiring + D1 drop
// in the follow-ups.

import {
  BUSINESS_DOMAIN_TABLE_SPECS,
  isBusinessDomainTable,
  normalizeBusinessValue,
  requireBusinessUnsafe,
  upsertBusinessRows,
  type BusinessDomainTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'

export type { BusinessDomainTable }

export type BusinessDomainRow = Readonly<Record<string, unknown>>

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type BusinessDomainReadsMode = 'd1' | 'postgres' | 'compare'

export type BusinessDomainFlags = Readonly<{
  dualWrite: boolean
  reads: BusinessDomainReadsMode
}>

export type BusinessDomainFlagEnv = Readonly<{
  KHALA_SYNC_BUSINESS_DUAL_WRITE?: string | undefined
  KHALA_SYNC_BUSINESS_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.14 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding
 * exists); reads default to D1 authority until the runbook's cutover
 * sequence flips them. Unknown read values fall back to 'd1' — never
 * fail open into an unproven read path on a typo.
 */
export const businessDomainFlagsFromEnv = (
  env: BusinessDomainFlagEnv,
): BusinessDomainFlags => {
  const dualWriteRaw = env.KHALA_SYNC_BUSINESS_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_BUSINESS_READS?.trim().toLowerCase()

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

export type BusinessDomainDiagnosticEvent =
  | 'khala_sync_business_dual_write_failed'
  | 'khala_sync_business_write_unclassified'
  | 'khala_sync_business_read_compare_mismatch'
  | 'khala_sync_business_read_compare_failed'
  | 'khala_sync_business_postgres_reads_deferred'

export type BusinessDomainDiagnostic = Readonly<{
  /** The store op, e.g. 'mirror:business_funnel_events' or a statement head. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (ids/refs). NEVER contact emails, request bodies, or payloads.
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no values). */
  messageSafe: string
}>

export type BusinessDomainLog = (
  event: BusinessDomainDiagnosticEvent,
  fields: BusinessDomainDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

/** The leading keywords of a statement — safe to log (SQL text is code). */
const statementHead = (sql: string): string =>
  sql.replaceAll(/\s+/g, ' ').trim().slice(0, 80)

// ---------------------------------------------------------------------------
// The row-level write seam (contract-suite twin pair)
// ---------------------------------------------------------------------------

/**
 * The typed row-level write seam: converge upserts (PK arbiter, D1
 * snapshot wins) for all thirty-two tables. Returns how many rows were
 * touched.
 */
export type BusinessDomainWriteStore = Readonly<{
  upsertRows: (
    table: BusinessDomainTable,
    rows: ReadonlyArray<BusinessDomainRow>,
  ) => Promise<number>
}>

export type PostgresBusinessDomainStore = BusinessDomainWriteStore &
  Readonly<{
    /**
     * Run one read-only statement on the Postgres twin (compare-mode
     * shadow reads and verification). `text` uses `$n` placeholders.
     */
    queryRows: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }>

export type MakePostgresBusinessDomainStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the KS-8.1/8.2/8.7/8.10 discipline.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresBusinessDomainStore = (
  deps: MakePostgresBusinessDomainStoreDependencies,
): PostgresBusinessDomainStore => {
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
      withSql(async sql => requireBusinessUnsafe(sql)(text, [...params])),
    upsertRows: (table, rows) =>
      withSql(sql => upsertBusinessRows(sql, table, rows)),
  }
}

/**
 * The D1 twin of the row-level seam (used by the contract suite and
 * available as the write path at eventual full cutover). Same converge
 * semantics over the same PK arbiters, driven by the SAME shared registry.
 */
export const makeD1BusinessDomainWriteStore = (
  db: D1Database,
): BusinessDomainWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const spec = BUSINESS_DOMAIN_TABLE_SPECS[table]
    const setClauses = spec.columns
      .filter(column => !spec.keyColumns.includes(column))
      .map(column => `${column} = excluded.${column}`)
      .join(', ')
    let touched = 0
    for (const row of rows) {
      const values = spec.columns.map(column =>
        normalizeBusinessValue(row[column]),
      )
      const placeholders = spec.columns.map(() => '?').join(', ')
      await db
        .prepare(
          `INSERT INTO ${table} (${spec.columns.join(', ')}) VALUES (${placeholders})
           ON CONFLICT(${spec.keyColumns.join(', ')}) DO UPDATE SET ${setClauses}`,
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

export type BusinessDomainMirror = Readonly<{
  /**
   * Read the rows matching `column IN (values)` back from D1 and
   * converge-upsert them into Postgres. `column` must be the PK or a
   * registered lookup/conflict column for `table`.
   */
  mirrorRowsBy: (
    table: BusinessDomainTable,
    column: string,
    values: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeBusinessDomainMirrorDependencies = Readonly<{
  db: D1Database
  postgres: BusinessDomainWriteStore
  log: BusinessDomainLog
}>

/**
 * Fail-soft read-back mirror: reads the authoritative rows from D1 and
 * converge-upserts them into Postgres; every failure is logged (keys
 * only) and swallowed. NEVER throws. A key that no longer/never matches a
 * D1 row (e.g. an `INSERT OR IGNORE` dedupe that discarded the new id)
 * mirrors zero rows — exactly right, the surviving row was mirrored when
 * it was first written.
 */
export const makeBusinessDomainMirror = (
  deps: MakeBusinessDomainMirrorDependencies,
): BusinessDomainMirror => {
  const { db, log, postgres } = deps

  return {
    mirrorRowsBy: async (table, column, values) => {
      if (values.length === 0) {
        return
      }
      try {
        const spec = BUSINESS_DOMAIN_TABLE_SPECS[table]
        if (
          !spec.keyColumns.includes(column) &&
          !spec.lookupColumns.includes(column) &&
          !spec.columns.includes(column)
        ) {
          throw new Error(`invalid mirror lookup column for ${table}`)
        }
        const placeholders = values.map(() => '?').join(', ')
        const rows = await db
          .prepare(
            `SELECT * FROM ${table} WHERE ${column} IN (${placeholders})`,
          )
          .bind(...values)
          .all<BusinessDomainRow>()
        await postgres.upsertRows(table, rows.results ?? [])
      } catch (error) {
        log('khala_sync_business_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `mirror:${table}`,
          refs: values.slice(0, 10).map(String),
        })
      }
    },
  }
}

// ---------------------------------------------------------------------------
// Statement classification (the scoped write set across the domain modules)
// ---------------------------------------------------------------------------

export type BusinessDomainKeySource =
  | Readonly<{ kind: 'bind'; index: number }>
  | Readonly<{ kind: 'literal'; value: string }>

export type BusinessDomainStatementClass =
  | Readonly<{
      kind: 'mirrored-write'
      table: BusinessDomainTable
      /** The column the statement addresses the row by (PK/lookup/conflict). */
      keyColumn: string
      keySource: BusinessDomainKeySource
    }>
  | Readonly<{ kind: 'unclassified-write'; table: BusinessDomainTable }>
  | Readonly<{ kind: 'comparable-select' }>
  | Readonly<{ kind: 'passthrough' }>

/** SQL text with string literals blanked, for placeholder counting. */
const withoutStringLiterals = (sql: string): string =>
  sql.replaceAll(/'(?:[^']|'')*'/g, "''")

const countBinds = (sql: string): number =>
  (withoutStringLiterals(sql).match(/\?/g) ?? []).length

/** Split a VALUES tuple body on top-level commas (quote/paren aware). */
const splitTupleItems = (body: string): ReadonlyArray<string> => {
  const items: Array<string> = []
  let depth = 0
  let inString = false
  let current = ''
  for (let i = 0; i < body.length; i++) {
    const ch = body[i]
    if (inString) {
      current += ch
      if (ch === "'") {
        if (body[i + 1] === "'") {
          current += "'"
          i += 1
        } else {
          inString = false
        }
      }
      continue
    }
    if (ch === "'") {
      inString = true
      current += ch
    } else if (ch === '(') {
      depth += 1
      current += ch
    } else if (ch === ')') {
      depth -= 1
      current += ch
    } else if (ch === ',' && depth === 0) {
      items.push(current.trim())
      current = ''
    } else {
      current += ch
    }
  }
  if (current.trim().length > 0) {
    items.push(current.trim())
  }
  return items
}

const INSERT_RE =
  /^\s*insert\s+(?:or\s+(?:ignore|replace)\s+)?into\s+([a-z_][a-z0-9_]*)\s*\(([^)]*)\)\s*values\s*\(([\s\S]*)\)\s*;?\s*$/i

const INSERT_ON_CONFLICT_RE =
  /^\s*insert\s+(?:or\s+(?:ignore|replace)\s+)?into\s+([a-z_][a-z0-9_]*)\s*\(([^)]*)\)\s*values\s*\(([\s\S]*?)\)\s*on\s+conflict\s*\(\s*([a-z_][a-z0-9_]*)\s*\)\s*do\s+(?:nothing|update\s+set[\s\S]*?)\s*;?\s*$/i

const UPDATE_RE =
  /^\s*update\s+([a-z_][a-z0-9_]*)\s+set\s+([\s\S]*?)\s+where\s+([\s\S]*?);?\s*$/i

const WRITE_HEAD_RE = /^\s*(insert|update|delete)\b/i

const SELECT_HEAD_RE = /^\s*select\b/i

const TABLE_REF_RE = /\b(?:from|join)\s+([a-z_][a-z0-9_]*)/gi

const classifyInsertTuple = (
  table: BusinessDomainTable,
  columnsRaw: string,
  tupleRaw: string,
  keyColumn: string,
): BusinessDomainStatementClass => {
  const columns = columnsRaw.split(',').map(column => column.trim().toLowerCase())
  const items = splitTupleItems(tupleRaw)
  const keyIndex = columns.indexOf(keyColumn)
  if (keyIndex === -1 || items.length !== columns.length) {
    return { kind: 'unclassified-write', table }
  }
  const keyItem = items[keyIndex]!
  if (keyItem === '?') {
    const bindIndex = items
      .slice(0, keyIndex)
      .filter(item => item === '?').length
    return {
      keyColumn,
      keySource: { index: bindIndex, kind: 'bind' },
      kind: 'mirrored-write',
      table,
    }
  }
  const literalMatch = /^'((?:[^']|'')*)'$/.exec(keyItem)
  if (literalMatch !== null) {
    return {
      keyColumn,
      keySource: {
        kind: 'literal',
        value: literalMatch[1]!.replaceAll("''", "'"),
      },
      kind: 'mirrored-write',
      table,
    }
  }
  return { kind: 'unclassified-write', table }
}

/**
 * Classify one prepared statement against the scoped table set.
 *
 *  - INSERT [OR IGNORE|REPLACE] INTO <scoped> (cols) VALUES (tuple)
 *    [ON CONFLICT(col) DO …]: without a conflict clause the addressed key
 *    is the PK's tuple item (`?` bind — index = number of `?` items
 *    before it, INSERT binds only appear in the tuple — or a quoted
 *    literal). WITH a conflict clause the addressed key is the CONFLICT
 *    column (the surviving row on conflict keeps its own PK).
 *  - UPDATE <scoped> SET … WHERE … <pk-or-lookup> = ?|'literal' …: bind
 *    index = binds in SET + binds in WHERE before the key equality. The
 *    PK is preferred; registered lookup columns are accepted when the PK
 *    is absent from the WHERE clause.
 *  - Any other INSERT/UPDATE/DELETE touching a scoped table →
 *    `unclassified-write` (loud diagnostic, still fail-soft; the contract
 *    suite pins the live domain write statements against this branch).
 *  - SELECTs whose from/join refs are ALL scoped tables →
 *    `comparable-select` (compare-mode shadow reads).
 */
export const classifyBusinessDomainStatement = (
  sql: string,
): BusinessDomainStatementClass => {
  const conflictMatch = INSERT_ON_CONFLICT_RE.exec(sql)
  if (conflictMatch !== null) {
    const table = conflictMatch[1]!.toLowerCase()
    if (!isBusinessDomainTable(table)) {
      return { kind: 'passthrough' }
    }
    return classifyInsertTuple(
      table,
      conflictMatch[2]!,
      conflictMatch[3]!,
      conflictMatch[4]!.toLowerCase(),
    )
  }

  const insertMatch = INSERT_RE.exec(sql)
  if (insertMatch !== null) {
    const table = insertMatch[1]!.toLowerCase()
    if (!isBusinessDomainTable(table)) {
      return { kind: 'passthrough' }
    }
    const pk = BUSINESS_DOMAIN_TABLE_SPECS[table].keyColumns[0]!
    return classifyInsertTuple(table, insertMatch[2]!, insertMatch[3]!, pk)
  }

  const updateMatch = UPDATE_RE.exec(sql)
  if (updateMatch !== null) {
    const table = updateMatch[1]!.toLowerCase()
    if (!isBusinessDomainTable(table)) {
      return { kind: 'passthrough' }
    }
    const spec = BUSINESS_DOMAIN_TABLE_SPECS[table]
    const whereClause = updateMatch[3]!
    for (const keyColumn of [...spec.keyColumns, ...spec.lookupColumns]) {
      const keyEquality = new RegExp(
        `\\b${keyColumn}\\s*=\\s*(\\?|'(?:[^']|'')*')`,
        'i',
      ).exec(whereClause)
      if (keyEquality === null) {
        continue
      }
      const matchedValue = keyEquality[1]!
      if (matchedValue === '?') {
        const bindsInSet = countBinds(updateMatch[2]!)
        const bindsBeforeKeyInWhere = countBinds(
          whereClause.slice(0, keyEquality.index),
        )
        return {
          keyColumn,
          keySource: {
            index: bindsInSet + bindsBeforeKeyInWhere,
            kind: 'bind',
          },
          kind: 'mirrored-write',
          table,
        }
      }
      return {
        keyColumn,
        keySource: {
          kind: 'literal',
          value: matchedValue.slice(1, -1).replaceAll("''", "'"),
        },
        kind: 'mirrored-write',
        table,
      }
    }
    return { kind: 'unclassified-write', table }
  }

  if (WRITE_HEAD_RE.test(sql)) {
    // DELETE or an unparsable INSERT/UPDATE: only loud if it touches a
    // scoped table.
    const touched = new Set<string>()
    for (const match of withoutStringLiterals(sql).matchAll(
      /\b(?:into|update|from)\s+([a-z_][a-z0-9_]*)/gi,
    )) {
      touched.add(match[1]!.toLowerCase())
    }
    for (const table of touched) {
      if (isBusinessDomainTable(table)) {
        return { kind: 'unclassified-write', table }
      }
    }
    return { kind: 'passthrough' }
  }

  if (SELECT_HEAD_RE.test(sql)) {
    const refs: Array<string> = []
    for (const match of withoutStringLiterals(sql).matchAll(TABLE_REF_RE)) {
      refs.push(match[1]!.toLowerCase())
    }
    if (refs.length > 0 && refs.every(ref => isBusinessDomainTable(ref))) {
      return { kind: 'comparable-select' }
    }
  }

  return { kind: 'passthrough' }
}

/** Resolve the addressed row's key value from the statement's bound params. */
export const resolveBusinessDomainKey = (
  keySource: BusinessDomainKeySource,
  params: ReadonlyArray<unknown>,
): string | undefined => {
  if (keySource.kind === 'literal') {
    return keySource.value
  }
  const value = params[keySource.index]
  return value === undefined || value === null ? undefined : String(value)
}

/** Convert D1 `?` placeholders to Postgres `$n` (quote-aware). */
export const toPostgresPlaceholders = (sql: string): string => {
  let out = ''
  let inString = false
  let n = 0
  for (let i = 0; i < sql.length; i++) {
    const ch = sql[i]!
    if (inString) {
      out += ch
      if (ch === "'") {
        if (sql[i + 1] === "'") {
          out += "'"
          i += 1
        } else {
          inString = false
        }
      }
      continue
    }
    if (ch === "'") {
      inString = true
      out += ch
    } else if (ch === '?') {
      n += 1
      out += `$${n}`
    } else {
      out += ch
    }
  }
  return out
}

// ---------------------------------------------------------------------------
// The mirroring D1Database (production dual-write wiring)
// ---------------------------------------------------------------------------

const stableRowString = (row: Record<string, unknown>): string => {
  const entries = Object.entries(row)
    .map(([key, value]) => [key, normalizeBusinessValue(value)] as const)
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(
      ([key, value]) => `${key}=${value === null ? '<null>' : String(value)}`,
    )
  return entries.join('')
}

const rowsEqual = (
  d1Rows: ReadonlyArray<Record<string, unknown>>,
  postgresRows: ReadonlyArray<Record<string, unknown>>,
): boolean => {
  if (d1Rows.length !== postgresRows.length) {
    return false
  }
  for (let i = 0; i < d1Rows.length; i++) {
    if (stableRowString(d1Rows[i]!) !== stableRowString(postgresRows[i]!)) {
      return false
    }
  }
  return true
}

export type MakeBusinessDomainMirroringDatabaseDependencies = Readonly<{
  db: D1Database
  /** The write mirror, or undefined when dual-write is off. */
  mirror: BusinessDomainMirror | undefined
  /**
   * The Postgres store for compare-mode shadow reads, or undefined when
   * reads stay on plain D1.
   */
  compareStore: PostgresBusinessDomainStore | undefined
  log: BusinessDomainLog
}>

type BoundStatement = Readonly<{
  statement: D1PreparedStatement
  onWriteSuccess: (() => Promise<void>) | undefined
}>

/**
 * Wrap one D1Database so that every successful write to a scoped business
 * domain table read-back-mirrors the affected row(s) into Postgres, and
 * (compare mode) every scoped-table SELECT is shadow-run against the
 * Postgres twin with D1 always served. All other statements pass through
 * untouched. Fail-soft everywhere: no mirror or compare outcome can fail
 * or alter the D1 result.
 */
export const makeBusinessDomainMirroringDatabase = (
  deps: MakeBusinessDomainMirroringDatabaseDependencies,
): D1Database => {
  const { compareStore, db, log, mirror } = deps

  const compareSelect =
    compareStore === undefined
      ? undefined
      : async (
          sql: string,
          params: ReadonlyArray<unknown>,
          d1Rows: ReadonlyArray<Record<string, unknown>>,
        ): Promise<void> => {
          try {
            const postgresRows = await compareStore.queryRows(
              toPostgresPlaceholders(sql),
              params,
            )
            if (!rowsEqual(d1Rows, postgresRows)) {
              log('khala_sync_business_read_compare_mismatch', {
                messageSafe: `d1=${d1Rows.length} postgres=${postgresRows.length} rows differ`,
                op: statementHead(sql),
                refs: [],
              })
            }
          } catch (error) {
            log('khala_sync_business_read_compare_failed', {
              messageSafe: safeMessage(error),
              op: statementHead(sql),
              refs: [],
            })
          }
        }

  const makeBound = (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): BoundStatement => {
    const statement =
      params.length === 0 ? db.prepare(sql) : db.prepare(sql).bind(...params)
    const classified = classifyBusinessDomainStatement(sql)

    if (classified.kind === 'unclassified-write') {
      return {
        onWriteSuccess: () => {
          log('khala_sync_business_write_unclassified', {
            messageSafe:
              'scoped business table write did not classify; postgres twin may drift until the next backfill sweep',
            op: statementHead(sql),
            refs: [],
          })
          return Promise.resolve()
        },
        statement,
      }
    }

    if (classified.kind === 'mirrored-write' && mirror !== undefined) {
      return {
        onWriteSuccess: () => {
          const key = resolveBusinessDomainKey(classified.keySource, params)
          return key === undefined
            ? Promise.resolve()
            : mirror.mirrorRowsBy(classified.table, classified.keyColumn, [
                key,
              ])
        },
        statement,
      }
    }

    return { onWriteSuccess: undefined, statement }
  }

  const wrapStatement = (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): D1PreparedStatement => {
    const bound = makeBound(sql, params)
    const classified = classifyBusinessDomainStatement(sql)
    const comparable =
      compareSelect !== undefined && classified.kind === 'comparable-select'

    const wrapper = {
      all: async <T>(...args: ReadonlyArray<unknown>) => {
        const result = await (
          bound.statement.all as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<D1Result<T>>
        )(...args)
        if (comparable) {
          await compareSelect(
            sql,
            params,
            (result.results ?? []) as ReadonlyArray<Record<string, unknown>>,
          )
        }
        return result
      },
      bind: (...values: ReadonlyArray<unknown>) => wrapStatement(sql, values),
      first: async <T>(...args: ReadonlyArray<unknown>) => {
        const result = await (
          bound.statement.first as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<T | null>
        )(...args)
        // Compare only whole-row `first()` reads; `first(column)` scalar
        // reads ride the same statement shape and are covered by `all`
        // call sites.
        if (comparable && args.length === 0) {
          await compareSelect(
            sql,
            params,
            result === null
              ? []
              : [result as unknown as Record<string, unknown>],
          )
        }
        return result
      },
      raw: (...args: ReadonlyArray<unknown>) =>
        (
          bound.statement.raw as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<Array<Array<unknown>>>
        )(...args),
      run: async <T>(...args: ReadonlyArray<unknown>) => {
        const result = await (
          bound.statement.run as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<D1Result<T>>
        )(...args)
        if (bound.onWriteSuccess !== undefined) {
          await bound.onWriteSuccess()
        }
        return result
      },
      // The proxy carries the inner statement so `batch` can unwrap it.
      __businessDomainInner: bound,
    }

    return wrapper as unknown as D1PreparedStatement
  }

  const proxied = {
    batch: async <T>(statements: ReadonlyArray<D1PreparedStatement>) => {
      const inners = statements.map(statement => {
        const carried = (
          statement as unknown as {
            __businessDomainInner?: BoundStatement
          }
        ).__businessDomainInner
        return carried ?? { onWriteSuccess: undefined, statement }
      })
      const results = await db.batch<T>(inners.map(inner => inner.statement))
      for (const inner of inners) {
        if (inner.onWriteSuccess !== undefined) {
          await inner.onWriteSuccess()
        }
      }
      return results
    },
    dump: () => db.dump(),
    exec: (sql: string) => db.exec(sql),
    prepare: (sql: string) => wrapStatement(sql, []),
    withSession: (
      ...args: ReadonlyArray<unknown>
    ): ReturnType<D1Database['withSession']> =>
      (
        db.withSession as (
          ...a: ReadonlyArray<unknown>
        ) => ReturnType<D1Database['withSession']>
      )(...args),
  }

  return proxied as unknown as D1Database
}

// ---------------------------------------------------------------------------
// Env plumbing (the call-site drop-in)
// ---------------------------------------------------------------------------

export type BusinessDomainStoreEnv = BusinessDomainFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeBusinessDomainStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: BusinessDomainLog | undefined
}>

const defaultLog: BusinessDomainLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: BusinessDomainStoreEnv,
  options: MakeBusinessDomainStoreOptions,
): PostgresBusinessDomainStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresBusinessDomainStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * The drop-in for `openAgentsDatabase(env)` at business-domain write entry
 * points: the same D1Database, wrapped so scoped business table writes
 * read-back mirror into Postgres (dual-write flag) and scoped SELECTs
 * shadow-compare (reads flag). With no KHALA_SYNC_DB binding, dual-write
 * off AND reads 'd1', the RAW database is returned — zero overhead.
 */
export const businessDomainDatabaseForEnv = (
  env: BusinessDomainStoreEnv,
  options: MakeBusinessDomainStoreOptions = {},
): D1Database => {
  const db = openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const flags = businessDomainFlagsFromEnv(env)
  const log = options.log ?? defaultLog
  const postgres = postgresStoreForEnv(env, options)

  if (postgres === undefined || (!flags.dualWrite && flags.reads === 'd1')) {
    return db
  }

  if (flags.reads === 'postgres') {
    // Serving business-domain reads from Postgres is the cutover
    // follow-up (the read surface is domain-wide). Never fail open into
    // an unproven path: behave as compare and say so once.
    log('khala_sync_business_postgres_reads_deferred', {
      messageSafe:
        'KHALA_SYNC_BUSINESS_READS=postgres is deferred to the read-cutover follow-up; serving d1 with compare shadow reads',
      op: 'businessDomainDatabaseForEnv',
      refs: [],
    })
  }

  return makeBusinessDomainMirroringDatabase({
    compareStore: flags.reads === 'd1' ? undefined : postgres,
    db,
    log,
    mirror: flags.dualWrite
      ? makeBusinessDomainMirror({ db, log, postgres })
      : undefined,
  })
}
