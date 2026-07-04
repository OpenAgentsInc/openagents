// KS-8.12 (#8323): Sites domain CORE — D1 → Cloud SQL migration
// machinery. Fifth KS-8 domain lane; follows the KS-8.10 template
// (`forum/forum-content-store.ts` #8321 — the freshest pattern), with
// KS-8.1/8.2/8.5/8.6 precedents behind it.
//
// Domain tables (khala-sync migration `0020_sites_core.sql`, FIFTEEN
// content/builder tables): `site_projects`, `site_versions`,
// `site_deployments`, `site_deployment_attempts`, `site_access_grants`,
// `site_events`, `site_builder_sessions`, `site_builder_messages`,
// `site_builder_events`, `site_builder_phase_runs`,
// `site_builder_file_snapshots`, `site_builder_previews`,
// `site_builder_artifacts`, `site_builder_repair_attempts`,
// `site_builder_saved_versions`. (The KS-8.12 issue's remaining ~36
// tables — content satellites, `site_environment_values` (secrets —
// SPEC invariant 9), the site COMMERCE/payment tables (money discipline:
// KS-8.7/KS-8.8 rails referenced by ID, never forked), `targeted_site_*`
// incl. the Analytics-Engine-candidate metric events,
// `tenant_custom_hostnames`, legacy `deployments`/`deployment_events` —
// move in the filed follow-up remainder lane #8357; see MIGRATION_PLAN.md
// §3.9.)
//
// THE SEAM: like the forum domain, sites writes are plain D1 SQL spread
// across repository-style modules whose functions all take
// `db: D1Database` (sites.ts, sites-builder-sessions.ts, site-library.ts,
// adjutant-run-lifecycle.ts, customer-orders.ts, operator route modules).
// That makes `D1Database` itself the existing repository interface, so
// this lane's production wiring is a MIRRORING D1Database
// (`sitesContentDatabaseForEnv`) dropped in at the `openAgentsDatabase(env)`
// call sites that feed sites writes. Every module keeps its authoritative
// D1 SQL byte-for-byte; after a successful D1 write to a scoped table the
// proxy READS BACK the affected rows and converge-upserts the exact D1
// rows into Postgres (read-back mirroring keeps status transitions and
// json_set() metadata edits hash-identical across stores). A mirror
// failure NEVER fails the request — it logs the typed drift diagnostic
// `khala_sync_sites_dual_write_failed`.
//
// SECONDARY MIRROR KEYS (the sites-specific extension over the forum
// classifier): several sites UPDATEs are keyed by a PARENT id, not the
// row PK — `UPDATE site_deployments … WHERE site_id = ? AND
// status = 'active'` (rollback/disable transitions) and the site-library
// archival batch (`UPDATE site_builder_sessions … WHERE site_id = ?`).
// `classifySitesContentStatement` first looks for PK equality in the
// WHERE clause; failing that, for a registered secondary key
// (`SITES_CONTENT_TABLE_MIRROR_KEYS`), and the mirror reads back ALL D1
// rows for that key value (bounded fan-out: one site's deployments /
// sessions). Anything else touching a scoped table logs
// `khala_sync_sites_write_unclassified` (a loud drift signal, still
// fail-soft) — it can never silently corrupt.
//
// Pieces:
//
//  1. `SitesContentWriteStore` — the typed row-level seam (`upsertRows`)
//     with `makeD1SitesContentWriteStore` (real D1/SQLite),
//     `makePostgresSitesContentStore` (KHALA_SYNC_DB Hyperdrive, sharing
//     the SAME column/PK registry as the backfill via
//     `@openagentsinc/khala-sync-server` — one source of truth), and
//     `makeDualWriteSitesContentWriteStore` (D1 authority + fail-soft
//     Postgres mirror). One behavioral contract suite runs against BOTH
//     concrete stores (`sites-content-repository.contract.test.ts`).
//
//  2. `makeSitesContentMirror` — fail-soft read-back mirror
//     (`mirrorRowsByKey`, PK or secondary key).
//
//  3. `sitesContentDatabaseForEnv` — the call-site drop-in for
//     `openAgentsDatabase(env)` on sites write paths. Flags:
//       KHALA_SYNC_SITES_DUAL_WRITE (default ON; off|0|false|disabled)
//       KHALA_SYNC_SITES_READS     (default 'd1'; d1|compare|postgres)
//     With no KHALA_SYNC_DB binding everything degrades to plain D1.
//     `compare` shadow-runs scoped-table SELECTs against Postgres, SERVES
//     D1, and logs `khala_sync_sites_read_compare_mismatch`. `postgres`
//     read serving is DEFERRED to the cutover follow-up (the sites read
//     surface is domain-wide, and live SITE SERVING reads must be
//     inventoried first per the KS-8.12 acceptance): in this lane the
//     flag behaves as `compare` and logs
//     `khala_sync_sites_postgres_reads_deferred` once, so a premature
//     flag flip can never serve an unproven read path.
//
// PUBLIC-SAFETY: site prompts, builder message bodies, and snapshot
// preview text are customer content — diagnostics reference row KEYS and
// statement heads only, never row values. `site_environment_values`
// (which may carry secrets) is deliberately OUTSIDE the scoped set.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Sites domain cutover"):
// dual-write on → backfill (scripts/backfill-sites-content.ts) → verify
// (exact counts, per-project version chains, deployment state census,
// builder sequence chains, newest-N row hashes) → compare reads → read
// cutover + remainder tables + D1 drop in the follow-up.

import {
  SITES_CONTENT_TABLE_COLUMNS,
  SITES_CONTENT_TABLE_MIRROR_KEYS,
  SITES_CONTENT_TABLE_PK,
  isSitesContentTable,
  normalizeSitesContentValue,
  requireSitesContentUnsafe,
  upsertSitesContentRows,
  type SitesContentRow,
  type SitesContentTable,
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

export type { SitesContentRow, SitesContentTable }

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type SitesContentReadsMode = 'd1' | 'postgres' | 'compare'

export type SitesContentFlags = Readonly<{
  dualWrite: boolean
  reads: SitesContentReadsMode
}>

export type SitesContentFlagEnv = Readonly<{
  KHALA_SYNC_SITES_DUAL_WRITE?: string | undefined
  KHALA_SYNC_SITES_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.12 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding
 * exists); reads default to D1 authority until the runbook's cutover
 * sequence flips them. Unknown read values fall back to 'd1' — never
 * fail open into an unproven read path on a typo.
 */
export const sitesContentFlagsFromEnv = (
  env: SitesContentFlagEnv,
): SitesContentFlags => {
  const dualWriteRaw = env.KHALA_SYNC_SITES_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_SITES_READS?.trim().toLowerCase()

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

export type SitesContentDiagnosticEvent =
  | 'khala_sync_sites_dual_write_failed'
  | 'khala_sync_sites_write_unclassified'
  | 'khala_sync_sites_read_compare_mismatch'
  | 'khala_sync_sites_read_compare_failed'
  | 'khala_sync_sites_postgres_reads_deferred'

export type SitesContentDiagnostic = Readonly<{
  /** The store operation, e.g. 'mirror:site_deployments' or a statement head. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (ids/refs). NEVER prompts, message bodies, or preview text.
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no values). */
  messageSafe: string
}>

export type SitesContentLog = (
  event: SitesContentDiagnosticEvent,
  fields: SitesContentDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

/** The leading keywords of a statement — safe to log (SQL text is code). */
const statementHead = (sql: string): string =>
  sql.replaceAll(/\s+/g, ' ').trim().slice(0, 80)

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

/**
 * The typed row-level write seam: converge upserts (PK arbiter, D1
 * snapshot wins) for all fifteen tables. Returns how many rows were
 * touched.
 */
export type SitesContentWriteStore = Readonly<{
  upsertRows: (
    table: SitesContentTable,
    rows: ReadonlyArray<SitesContentRow>,
  ) => Promise<number>
}>

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresSitesContentStore = SitesContentWriteStore &
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

export type MakePostgresSitesContentStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the earlier KS-8
   * stores.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresSitesContentStore = (
  deps: MakePostgresSitesContentStoreDependencies,
): PostgresSitesContentStore => {
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
        requireSitesContentUnsafe(sql)(text, [...params]),
      ),
    upsertRows: (table, rows) =>
      withSql(sql => upsertSitesContentRows(sql, table, rows)),
  }
}

// ---------------------------------------------------------------------------
// D1 implementation of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

/**
 * The D1 twin of the row-level seam (used by the contract suite and
 * available as the write path at eventual full cutover). Same converge
 * semantics over the same PK arbiters, driven by the SAME shared registry.
 */
export const makeD1SitesContentWriteStore = (
  db: D1Database,
): SitesContentWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const columns = SITES_CONTENT_TABLE_COLUMNS[table]
    const pk = SITES_CONTENT_TABLE_PK[table]
    const setClauses = columns
      .filter(column => column !== pk)
      .map(column => `${column} = excluded.${column}`)
      .join(', ')
    let touched = 0
    for (const row of rows) {
      const values = columns.map(column =>
        normalizeSitesContentValue(row[column]),
      )
      const placeholders = columns.map(() => '?').join(', ')
      await db
        .prepare(
          `INSERT INTO ${table} (${columns.join(', ')}) VALUES (${placeholders})
           ON CONFLICT(${pk}) DO UPDATE SET ${setClauses}`,
        )
        .bind(...values)
        .run()
      touched += 1
    }
    return touched
  },
})

// ---------------------------------------------------------------------------
// Dual-write wrapper over the row seam
// ---------------------------------------------------------------------------

export type MakeDualWriteSitesContentWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store. */
  d1: SitesContentWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: SitesContentWriteStore | undefined
  flags: SitesContentFlags
  log?: SitesContentLog | undefined
}>

/**
 * D1 writes first (authority); the same rows then mirror to Postgres
 * best-effort. A mirror failure never fails the write — it emits
 * `khala_sync_sites_dual_write_failed` (the drift metric).
 */
export const makeDualWriteSitesContentWriteStore = (
  deps: MakeDualWriteSitesContentWriteStoreDependencies,
): SitesContentWriteStore => {
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
        log('khala_sync_sites_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `upsertRows:${table}`,
          refs: rows
            .slice(0, 10)
            .map(row => String(row[SITES_CONTENT_TABLE_PK[table]] ?? '')),
        })
      }
      return outcome
    },
  }
}

// ---------------------------------------------------------------------------
// The read-back mirror
// ---------------------------------------------------------------------------

export type SitesContentMirror = Readonly<{
  /**
   * Read the rows where `column = value` back from D1 and upsert them
   * into Postgres. `column` is the table PK or a registered secondary
   * mirror key (bounded fan-out).
   */
  mirrorRowsByKey: (
    table: SitesContentTable,
    column: string,
    value: string,
  ) => Promise<void>
}>

export type MakeSitesContentMirrorDependencies = Readonly<{
  db: D1Database
  postgres: SitesContentWriteStore
  log: SitesContentLog
}>

/**
 * Fail-soft read-back mirror: reads the authoritative rows from D1 and
 * converge-upserts them into Postgres; every failure is logged (keys
 * only) and swallowed. NEVER throws. A key that no longer/never matched
 * a D1 row (e.g. an `INSERT OR IGNORE` dedupe that discarded the new id)
 * mirrors zero rows — exactly right, the surviving row was mirrored when
 * it was first written.
 */
export const makeSitesContentMirror = (
  deps: MakeSitesContentMirrorDependencies,
): SitesContentMirror => {
  const { db, log, postgres } = deps

  return {
    mirrorRowsByKey: async (table, column, value) => {
      try {
        const rows = await db
          .prepare(`SELECT * FROM ${table} WHERE ${column} = ?`)
          .bind(value)
          .all<SitesContentRow>()
        await postgres.upsertRows(table, rows.results ?? [])
      } catch (error) {
        log('khala_sync_sites_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `mirror:${table}:${column}`,
          refs: [value],
        })
      }
    },
  }
}

// ---------------------------------------------------------------------------
// Statement classification (the closed write set across the sites modules)
// ---------------------------------------------------------------------------

export type SitesContentKeySource =
  | Readonly<{ kind: 'bind'; column: string; index: number }>
  | Readonly<{ kind: 'literal'; column: string; value: string }>

export type SitesContentStatementClass =
  | Readonly<{
      kind: 'mirrored-write'
      table: SitesContentTable
      keySource: SitesContentKeySource
    }>
  | Readonly<{ kind: 'unclassified-write'; table: SitesContentTable }>
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

const UPDATE_RE =
  /^\s*update\s+([a-z_][a-z0-9_]*)\s+set\s+([\s\S]*?)\s+where\s+([\s\S]*?);?\s*$/i

const WRITE_HEAD_RE = /^\s*(insert|update|delete)\b/i

const SELECT_HEAD_RE = /^\s*select\b/i

const TABLE_REF_RE = /\b(?:from|join)\s+([a-z_][a-z0-9_]*)/gi

/**
 * Find `<column> = ?|'literal'` in a WHERE clause and resolve it into a
 * key source (bind index = binds in SET + binds in WHERE before the
 * equality). Column-name matching is word-bounded, so `id` never matches
 * inside `version_id`.
 */
const whereKeyEquality = (
  column: string,
  setClause: string,
  whereClause: string,
): SitesContentKeySource | undefined => {
  const equality = new RegExp(
    `(?<![a-z0-9_.])${column}\\s*=\\s*(\\?|'(?:[^']|'')*')`,
    'i',
  ).exec(whereClause)
  if (equality === null) {
    return undefined
  }
  const matched = equality[1]!
  if (matched === '?') {
    return {
      column,
      index:
        countBinds(setClause) +
        countBinds(whereClause.slice(0, equality.index)),
      kind: 'bind',
    }
  }
  return {
    column,
    kind: 'literal',
    value: matched.slice(1, -1).replaceAll("''", "'"),
  }
}

/**
 * Classify one prepared statement against the scoped table set.
 *
 *  - INSERT [OR IGNORE|REPLACE] INTO <scoped> (cols) VALUES (tuple):
 *    the PK's tuple item is either a `?` (bind index = number of `?`
 *    items before it — INSERT binds only appear in the tuple) or a
 *    quoted literal.
 *  - UPDATE <scoped> SET … WHERE …: PK equality first
 *    (`id = ?|'literal'`), then the registered secondary mirror keys
 *    (`site_id` / `session_id`) — the deployment/session transitions
 *    keyed by parent id.
 *  - Any other INSERT/UPDATE/DELETE touching a scoped table →
 *    `unclassified-write` (loud diagnostic; the contract suite keeps
 *    this branch unreachable for the live module statements).
 *  - SELECTs whose from/join refs are ALL scoped tables →
 *    `comparable-select` (compare-mode shadow reads).
 */
export const classifySitesContentStatement = (
  sql: string,
): SitesContentStatementClass => {
  const insertMatch = INSERT_RE.exec(sql)
  if (insertMatch !== null) {
    const table = insertMatch[1]!.toLowerCase()
    if (!isSitesContentTable(table)) {
      return { kind: 'passthrough' }
    }
    const columns = insertMatch[2]!
      .split(',')
      .map(column => column.trim().toLowerCase())
    const items = splitTupleItems(insertMatch[3]!)
    const pk = SITES_CONTENT_TABLE_PK[table]
    const pkIndex = columns.indexOf(pk)
    if (pkIndex === -1 || items.length !== columns.length) {
      return { kind: 'unclassified-write', table }
    }
    const pkItem = items[pkIndex]!
    if (pkItem === '?') {
      const bindIndex = items
        .slice(0, pkIndex)
        .filter(item => item === '?').length
      return {
        keySource: { column: pk, index: bindIndex, kind: 'bind' },
        kind: 'mirrored-write',
        table,
      }
    }
    const literalMatch = /^'((?:[^']|'')*)'$/.exec(pkItem)
    if (literalMatch !== null) {
      return {
        keySource: {
          column: pk,
          kind: 'literal',
          value: literalMatch[1]!.replaceAll("''", "'"),
        },
        kind: 'mirrored-write',
        table,
      }
    }
    return { kind: 'unclassified-write', table }
  }

  const updateMatch = UPDATE_RE.exec(sql)
  if (updateMatch !== null) {
    const table = updateMatch[1]!.toLowerCase()
    if (!isSitesContentTable(table)) {
      return { kind: 'passthrough' }
    }
    const setClause = updateMatch[2]!
    const whereClause = updateMatch[3]!
    const candidates = [
      SITES_CONTENT_TABLE_PK[table],
      ...SITES_CONTENT_TABLE_MIRROR_KEYS[table],
    ]
    for (const column of candidates) {
      const keySource = whereKeyEquality(column, setClause, whereClause)
      if (keySource !== undefined) {
        return { keySource, kind: 'mirrored-write', table }
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
      if (isSitesContentTable(table)) {
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
    if (refs.length > 0 && refs.every(ref => isSitesContentTable(ref))) {
      return { kind: 'comparable-select' }
    }
  }

  return { kind: 'passthrough' }
}

/** Resolve the affected rows' key value from the statement's bound params. */
export const resolveSitesContentKey = (
  keySource: SitesContentKeySource,
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
    .map(
      ([key, value]) =>
        [key, normalizeSitesContentValue(value)] as const,
    )
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([key, value]) => `${key}=${value === null ? '<null>' : String(value)}`)
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

export type MakeSitesContentMirroringDatabaseDependencies = Readonly<{
  db: D1Database
  /** The write mirror, or undefined when dual-write is off. */
  mirror: SitesContentMirror | undefined
  /**
   * The Postgres store for compare-mode shadow reads, or undefined when
   * reads stay on plain D1.
   */
  compareStore: PostgresSitesContentStore | undefined
  log: SitesContentLog
}>

type BoundStatement = Readonly<{
  statement: D1PreparedStatement
  onWriteSuccess: (() => Promise<void>) | undefined
}>

/**
 * Wrap one D1Database so that every successful write to a scoped sites
 * table read-back-mirrors the affected rows into Postgres, and (compare
 * mode) every scoped-table SELECT is shadow-run against the Postgres
 * twin with D1 always served. All other statements pass through
 * untouched. Fail-soft everywhere: no mirror or compare outcome can fail
 * or alter the D1 result.
 */
export const makeSitesContentMirroringDatabase = (
  deps: MakeSitesContentMirroringDatabaseDependencies,
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
              log('khala_sync_sites_read_compare_mismatch', {
                messageSafe: `d1=${d1Rows.length} postgres=${postgresRows.length} rows differ`,
                op: statementHead(sql),
                refs: [],
              })
            }
          } catch (error) {
            log('khala_sync_sites_read_compare_failed', {
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
      params.length === 0
        ? db.prepare(sql)
        : db.prepare(sql).bind(...params)
    const classified = classifySitesContentStatement(sql)

    if (classified.kind === 'unclassified-write') {
      return {
        onWriteSuccess: () => {
          log('khala_sync_sites_write_unclassified', {
            messageSafe:
              'scoped sites table write did not classify; postgres twin may drift until the next backfill sweep',
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
          const key = resolveSitesContentKey(classified.keySource, params)
          return key === undefined
            ? Promise.resolve()
            : mirror.mirrorRowsByKey(
                classified.table,
                classified.keySource.column,
                key,
              )
        },
        statement,
      }
    }

    return { statement, onWriteSuccess: undefined }
  }

  const wrapStatement = (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): D1PreparedStatement => {
    const bound = makeBound(sql, params)
    const classified = classifySitesContentStatement(sql)
    const comparable =
      compareSelect !== undefined && classified.kind === 'comparable-select'

    const wrapper = {
      bind: (...values: ReadonlyArray<unknown>) =>
        wrapStatement(sql, values),
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
      __sitesContentInner: bound,
    }

    return wrapper as unknown as D1PreparedStatement
  }

  const proxied = {
    batch: async <T>(statements: ReadonlyArray<D1PreparedStatement>) => {
      const inners = statements.map(statement => {
        const carried = (
          statement as unknown as {
            __sitesContentInner?: BoundStatement
          }
        ).__sitesContentInner
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

export type SitesContentStoreEnv = SitesContentFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeSitesContentStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: SitesContentLog | undefined
}>

const defaultLog: SitesContentLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: SitesContentStoreEnv,
  options: MakeSitesContentStoreOptions,
): PostgresSitesContentStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresSitesContentStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * The drop-in for `openAgentsDatabase(env)` at sites write entry points:
 * the same D1Database, wrapped so scoped sites writes read-back mirror
 * into Postgres (dual-write flag) and scoped SELECTs shadow-compare
 * (reads flag). With no KHALA_SYNC_DB binding, dual-write off AND reads
 * 'd1', the RAW database is returned — zero overhead.
 */
export const sitesContentDatabaseForEnv = (
  env: SitesContentStoreEnv,
  options: MakeSitesContentStoreOptions = {},
): D1Database => {
  const db = openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const flags = sitesContentFlagsFromEnv(env)
  const log = options.log ?? defaultLog
  const postgres = postgresStoreForEnv(env, options)

  if (postgres === undefined || (!flags.dualWrite && flags.reads === 'd1')) {
    return db
  }

  if (flags.reads === 'postgres') {
    // Serving sites reads from Postgres is the cutover follow-up (the
    // read surface is domain-wide and live SITE SERVING reads must be
    // inventoried first). Never fail open into an unproven path: behave
    // as compare and say so once.
    log('khala_sync_sites_postgres_reads_deferred', {
      messageSafe:
        'KHALA_SYNC_SITES_READS=postgres is deferred to the read-cutover follow-up; serving d1 with compare shadow reads',
      op: 'sitesContentDatabaseForEnv',
      refs: [],
    })
  }

  return makeSitesContentMirroringDatabase({
    compareStore: flags.reads === 'd1' ? undefined : postgres,
    db,
    log,
    mirror: flags.dualWrite
      ? makeSitesContentMirror({ db, log, postgres })
      : undefined,
  })
}
