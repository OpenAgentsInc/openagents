// KS-8.10 (#8321): forum content + trust domain — D1 → Cloud SQL migration
// machinery. Fourth KS-8 domain lane; follows the KS-8.1/8.2/8.5 templates
// (`pylon-dispatch-store.ts` #8307, `token-ledger-store.ts` #8308,
// `agent-runtime-store.ts` #8316 — the freshest pattern).
//
// Domain tables (khala-sync migration `0014_forum_content.sql`, THIRTEEN
// content-core tables): `forum_boards`, `forum_categories`,
// `forum_forums`, `forum_topics`, `forum_posts`, `forum_post_bodies`,
// `forum_post_revisions`, `forum_actor_follows`, `forum_watches`,
// `forum_bookmarks`, `forum_reports`, `forum_moderation_events`,
// `forum_context_links`. (The KS-8.10 issue's remaining tables — private
// messages, ACL grants, trust edges/scores, score snapshots, notification
// reads, work requests — move in the follow-up remainder lane; see
// MIGRATION_PLAN.md §3.7. The forum MONEY tables belong to KS-8.8 and are
// deliberately NOT touched here — money keeps D1 authority with its own
// lane's mirror discipline.)
//
// THE SEAM: unlike the KS-8.5 domain, the forum repository
// (`./repository.ts`) is not a store object — it is ~40 exported Effect
// functions that all take `db: D1Database` as their FIRST argument, and
// it is the ONLY writer of the thirteen scoped tables. That makes
// `D1Database` itself the existing repository interface, so this lane's
// production wiring is a MIRRORING D1Database (`forumContentDatabaseForEnv`)
// dropped in at the handful of `openAgentsDatabase(env)` call sites that
// feed forum writes. Every repository function keeps its authoritative D1
// SQL byte-for-byte; after a successful D1 write to a scoped table the
// proxy READS BACK the affected row by primary key and converge-upserts
// the exact D1 row into Postgres (read-back mirroring is what keeps
// counter bumps, CASE-free state transitions, and clamped decrements
// hash-identical across stores). A mirror failure NEVER fails the
// request — it logs the typed drift diagnostic
// `khala_sync_forum_dual_write_failed`.
//
// The proxy classifies statements with `classifyForumContentStatement`:
// the scoped write-statement set is CLOSED (all of it lives in
// repository.ts) and the contract suite pins every one of those
// statements against the classifier, so a new/changed write either
// classifies cleanly or logs `khala_sync_forum_write_unclassified` (a
// loud drift signal, still fail-soft) — it can never silently corrupt.
//
// Pieces:
//
//  1. `ForumContentWriteStore` — the typed row-level seam (`upsertRows`)
//     with `makeD1ForumContentWriteStore` (real D1/SQLite),
//     `makePostgresForumContentStore` (KHALA_SYNC_DB Hyperdrive, sharing
//     the SAME column/PK registry as the backfill via
//     `@openagentsinc/khala-sync-server` — one source of truth), and
//     `makeDualWriteForumContentWriteStore` (D1 authority + fail-soft
//     Postgres mirror). One behavioral contract suite runs against BOTH
//     concrete stores (`forum-content-repository.contract.test.ts`).
//
//  2. `makeForumContentMirror` — fail-soft read-back mirror
//     (`mirrorRowsByPk`).
//
//  3. `forumContentDatabaseForEnv` — the call-site drop-in for
//     `openAgentsDatabase(env)` on forum write paths. Flags:
//       KHALA_SYNC_FORUM_DUAL_WRITE (default ON; off|0|false|disabled)
//       KHALA_SYNC_FORUM_READS     (default 'd1'; d1|compare|postgres)
//     With no KHALA_SYNC_DB binding everything degrades to plain D1.
//     `compare` shadow-runs scoped-table SELECTs against Postgres, SERVES
//     D1, and logs `khala_sync_forum_read_compare_mismatch` — the
//     "public thread pages shadow-compared" cutover evidence. `postgres`
//     read serving is now BUILT (CFG D1 evacuation, #8515): the Cloudflare
//     D1 bridge is 401-dead, so `postgres` mode returns the forum Postgres
//     serving wrapper (`forum-postgres-serving.ts`) — every forum-domain
//     SELECT is served from Postgres and every forum-domain write is
//     executed on Postgres (authoritative), fail-soft to D1 only for a
//     Postgres read error. (The legacy `khala_sync_forum_postgres_reads_
//     deferred` diagnostic is retired — the read surface is served, not
//     deferred.)
//
// PUBLIC-SAFETY: forum content is a public projection surface, but
// diagnostics still reference row KEYS and statement heads only — never
// post bodies or projection payloads.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Forum content domain
// cutover"): dual-write on → backfill
// (scripts/backfill-forum-content.ts) → verify (exact counts, per-topic
// post chains, thread spot hashes, newest-N row hashes) → compare reads →
// read cutover + remainder tables + D1 drop in the follow-up.

import {
  FORUM_CONTENT_TABLE_COLUMNS,
  FORUM_CONTENT_TABLE_PK,
  isForumContentTable,
  normalizeForumContentValue,
  requireForumContentUnsafe,
  upsertForumContentRows,
  type ForumContentRow,
  type ForumContentTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from '../khala-sync-push-routes'
import { logWorkerRouteWarning } from '../observability'
import { openAgentsDatabase } from '../runtime'
import {
  forumServingStoreForEnv,
  makeForumPostgresServingDatabase,
} from './forum-postgres-serving'
import { wrapForumRemainderMirroring } from './forum-remainder-store'

export type { ForumContentRow, ForumContentTable }

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type ForumContentReadsMode = 'd1' | 'postgres' | 'compare'

export type ForumContentFlags = Readonly<{
  dualWrite: boolean
  reads: ForumContentReadsMode
}>

export type ForumContentFlagEnv = Readonly<{
  KHALA_SYNC_FORUM_DUAL_WRITE?: string | undefined
  KHALA_SYNC_FORUM_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.10 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding
 * exists); reads default to D1 authority until the runbook's cutover
 * sequence flips them. Unknown read values fall back to 'd1' — never
 * fail open into an unproven read path on a typo.
 */
export const forumContentFlagsFromEnv = (
  env: ForumContentFlagEnv,
): ForumContentFlags => {
  const dualWriteRaw = env.KHALA_SYNC_FORUM_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_FORUM_READS?.trim().toLowerCase()

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

export type ForumContentDiagnosticEvent =
  | 'khala_sync_forum_dual_write_failed'
  | 'khala_sync_forum_write_unclassified'
  | 'khala_sync_forum_read_compare_mismatch'
  | 'khala_sync_forum_read_compare_failed'
  | 'khala_sync_forum_postgres_reads_deferred'

export type ForumContentDiagnostic = Readonly<{
  /** The store operation, e.g. 'mirror:forum_posts' or a statement head. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (ids/refs). NEVER post bodies, titles, or projection payloads.
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no values). */
  messageSafe: string
}>

export type ForumContentLog = (
  event: ForumContentDiagnosticEvent,
  fields: ForumContentDiagnostic,
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
 * snapshot wins) for all thirteen tables. Returns how many rows were
 * touched.
 */
export type ForumContentWriteStore = Readonly<{
  upsertRows: (
    table: ForumContentTable,
    rows: ReadonlyArray<ForumContentRow>,
  ) => Promise<number>
}>

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresForumContentStore = ForumContentWriteStore &
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

export type MakePostgresForumContentStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the KS-8.1/8.2/8.5
   * stores.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresForumContentStore = (
  deps: MakePostgresForumContentStoreDependencies,
): PostgresForumContentStore => {
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
        requireForumContentUnsafe(sql)(text, [...params]),
      ),
    upsertRows: (table, rows) =>
      withSql(sql => upsertForumContentRows(sql, table, rows)),
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
export const makeD1ForumContentWriteStore = (
  db: D1Database,
): ForumContentWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const columns = FORUM_CONTENT_TABLE_COLUMNS[table]
    const pk = FORUM_CONTENT_TABLE_PK[table]
    const setClauses = columns
      .filter(column => column !== pk)
      .map(column => `${column} = excluded.${column}`)
      .join(', ')
    let touched = 0
    for (const row of rows) {
      const values = columns.map(column =>
        normalizeForumContentValue(row[column]),
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

export type MakeDualWriteForumContentWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store. */
  d1: ForumContentWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: ForumContentWriteStore | undefined
  flags: ForumContentFlags
  log?: ForumContentLog | undefined
}>

/**
 * D1 writes first (authority); the same rows then mirror to Postgres
 * best-effort. A mirror failure never fails the write — it emits
 * `khala_sync_forum_dual_write_failed` (the drift metric).
 */
export const makeDualWriteForumContentWriteStore = (
  deps: MakeDualWriteForumContentWriteStoreDependencies,
): ForumContentWriteStore => {
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
        log('khala_sync_forum_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `upsertRows:${table}`,
          refs: rows
            .slice(0, 10)
            .map(row => String(row[FORUM_CONTENT_TABLE_PK[table]] ?? '')),
        })
      }
      return outcome
    },
  }
}

// ---------------------------------------------------------------------------
// The read-back mirror
// ---------------------------------------------------------------------------

export type ForumContentMirror = Readonly<{
  /** Read the rows for `pkValues` back from D1 and upsert into Postgres. */
  mirrorRowsByPk: (
    table: ForumContentTable,
    pkValues: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeForumContentMirrorDependencies = Readonly<{
  db: D1Database
  postgres: ForumContentWriteStore
  log: ForumContentLog
}>

/**
 * Fail-soft read-back mirror: reads the authoritative rows from D1 and
 * converge-upserts them into Postgres; every failure is logged (keys
 * only) and swallowed. NEVER throws. A PK that no longer/never matched a
 * D1 row (e.g. an `INSERT OR IGNORE` dedupe that discarded the new id)
 * mirrors zero rows — exactly right, the surviving row was mirrored when
 * it was first written.
 */
export const makeForumContentMirror = (
  deps: MakeForumContentMirrorDependencies,
): ForumContentMirror => {
  const { db, log, postgres } = deps

  return {
    mirrorRowsByPk: async (table, pkValues) => {
      if (pkValues.length === 0) {
        return
      }
      try {
        const pk = FORUM_CONTENT_TABLE_PK[table]
        const placeholders = pkValues.map(() => '?').join(', ')
        const rows = await db
          .prepare(`SELECT * FROM ${table} WHERE ${pk} IN (${placeholders})`)
          .bind(...pkValues)
          .all<ForumContentRow>()
        await postgres.upsertRows(table, rows.results ?? [])
      } catch (error) {
        log('khala_sync_forum_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `mirror:${table}`,
          refs: pkValues.slice(0, 10).map(String),
        })
      }
    },
  }
}

// ---------------------------------------------------------------------------
// Statement classification (the closed write set in ./repository.ts)
// ---------------------------------------------------------------------------

export type ForumContentPkSource =
  | Readonly<{ kind: 'bind'; index: number }>
  | Readonly<{ kind: 'literal'; value: string }>

export type ForumContentStatementClass =
  | Readonly<{
      kind: 'mirrored-write'
      table: ForumContentTable
      pkSource: ForumContentPkSource
    }>
  | Readonly<{ kind: 'unclassified-write'; table: ForumContentTable }>
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
 * Classify one prepared statement against the scoped table set.
 *
 *  - INSERT [OR IGNORE|REPLACE] INTO <scoped> (cols) VALUES (tuple):
 *    the PK's tuple item is either a `?` (bind index = number of `?`
 *    items before it — INSERT binds only appear in the tuple) or a
 *    quoted literal.
 *  - UPDATE <scoped> SET … WHERE … <pk> = ?|'literal' …: bind index =
 *    binds in SET + binds in WHERE before the PK equality.
 *  - Any other INSERT/UPDATE/DELETE touching a scoped table →
 *    `unclassified-write` (loud diagnostic; the contract suite keeps this
 *    branch unreachable for the live repository statements).
 *  - SELECTs whose from/join refs are ALL scoped tables →
 *    `comparable-select` (compare-mode shadow reads).
 */
export const classifyForumContentStatement = (
  sql: string,
): ForumContentStatementClass => {
  const insertMatch = INSERT_RE.exec(sql)
  if (insertMatch !== null) {
    const table = insertMatch[1]!.toLowerCase()
    if (!isForumContentTable(table)) {
      return { kind: 'passthrough' }
    }
    const columns = insertMatch[2]!
      .split(',')
      .map(column => column.trim().toLowerCase())
    const items = splitTupleItems(insertMatch[3]!)
    const pk = FORUM_CONTENT_TABLE_PK[table]
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
        kind: 'mirrored-write',
        pkSource: { index: bindIndex, kind: 'bind' },
        table,
      }
    }
    const literalMatch = /^'((?:[^']|'')*)'$/.exec(pkItem)
    if (literalMatch !== null) {
      return {
        kind: 'mirrored-write',
        pkSource: {
          kind: 'literal',
          value: literalMatch[1]!.replaceAll("''", "'"),
        },
        table,
      }
    }
    return { kind: 'unclassified-write', table }
  }

  const updateMatch = UPDATE_RE.exec(sql)
  if (updateMatch !== null) {
    const table = updateMatch[1]!.toLowerCase()
    if (!isForumContentTable(table)) {
      return { kind: 'passthrough' }
    }
    const pk = FORUM_CONTENT_TABLE_PK[table]
    const whereClause = updateMatch[3]!
    const pkEquality = new RegExp(
      `\\b${pk}\\s*=\\s*(\\?|'(?:[^']|'')*')`,
      'i',
    ).exec(whereClause)
    if (pkEquality === null) {
      return { kind: 'unclassified-write', table }
    }
    const matchedValue = pkEquality[1]!
    if (matchedValue === '?') {
      const bindsInSet = countBinds(updateMatch[2]!)
      const bindsBeforePkInWhere = countBinds(
        whereClause.slice(0, pkEquality.index),
      )
      return {
        kind: 'mirrored-write',
        pkSource: {
          index: bindsInSet + bindsBeforePkInWhere,
          kind: 'bind',
        },
        table,
      }
    }
    return {
      kind: 'mirrored-write',
      pkSource: {
        kind: 'literal',
        value: matchedValue.slice(1, -1).replaceAll("''", "'"),
      },
      table,
    }
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
      if (isForumContentTable(table)) {
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
    if (refs.length > 0 && refs.every(ref => isForumContentTable(ref))) {
      return { kind: 'comparable-select' }
    }
  }

  return { kind: 'passthrough' }
}

/** Resolve the affected row's PK value from the statement's bound params. */
export const resolveForumContentPk = (
  pkSource: ForumContentPkSource,
  params: ReadonlyArray<unknown>,
): string | undefined => {
  if (pkSource.kind === 'literal') {
    return pkSource.value
  }
  const value = params[pkSource.index]
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
        [key, normalizeForumContentValue(value)] as const,
    )
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([key, value]) => `${key}=${value === null ? '<null>' : String(value)}`)
  return entries.join('')
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

export type MakeForumContentMirroringDatabaseDependencies = Readonly<{
  db: D1Database
  /** The write mirror, or undefined when dual-write is off. */
  mirror: ForumContentMirror | undefined
  /**
   * The Postgres store for compare-mode shadow reads, or undefined when
   * reads stay on plain D1.
   */
  compareStore: PostgresForumContentStore | undefined
  log: ForumContentLog
}>

type BoundStatement = Readonly<{
  statement: D1PreparedStatement
  onWriteSuccess: (() => Promise<void>) | undefined
}>

/**
 * Wrap one D1Database so that every successful write to a scoped forum
 * content table read-back-mirrors the affected row into Postgres, and
 * (compare mode) every scoped-table SELECT is shadow-run against the
 * Postgres twin with D1 always served. All other statements pass through
 * untouched. Fail-soft everywhere: no mirror or compare outcome can fail
 * or alter the D1 result.
 */
export const makeForumContentMirroringDatabase = (
  deps: MakeForumContentMirroringDatabaseDependencies,
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
              log('khala_sync_forum_read_compare_mismatch', {
                messageSafe: `d1=${d1Rows.length} postgres=${postgresRows.length} rows differ`,
                op: statementHead(sql),
                refs: [],
              })
            }
          } catch (error) {
            log('khala_sync_forum_read_compare_failed', {
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
    const classified = classifyForumContentStatement(sql)

    if (classified.kind === 'unclassified-write') {
      return {
        onWriteSuccess: () => {
          log('khala_sync_forum_write_unclassified', {
            messageSafe:
              'scoped forum table write did not classify; postgres twin may drift until the next backfill sweep',
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
          const pk = resolveForumContentPk(classified.pkSource, params)
          return pk === undefined
            ? Promise.resolve()
            : mirror.mirrorRowsByPk(classified.table, [pk])
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
    const classified = classifyForumContentStatement(sql)
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
      __forumContentInner: bound,
    }

    return wrapper as unknown as D1PreparedStatement
  }

  const proxied = {
    batch: async <T>(statements: ReadonlyArray<D1PreparedStatement>) => {
      const inners = statements.map(statement => {
        const carried = (
          statement as unknown as {
            __forumContentInner?: BoundStatement
          }
        ).__forumContentInner
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

export type ForumContentStoreEnv = ForumContentFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeForumContentStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: ForumContentLog | undefined
}>

const defaultLog: ForumContentLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: ForumContentStoreEnv,
  options: MakeForumContentStoreOptions,
): PostgresForumContentStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresForumContentStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * The drop-in for `openAgentsDatabase(env)` at forum write entry points:
 * the same D1Database, wrapped so scoped forum content writes read-back
 * mirror into Postgres (dual-write flag) and scoped SELECTs shadow-compare
 * (reads flag). With no KHALA_SYNC_DB binding, dual-write off AND reads
 * 'd1', the RAW database is returned — zero overhead.
 */
export const forumContentDatabaseForEnv = (
  env: ForumContentStoreEnv,
  options: MakeForumContentStoreOptions = {},
): D1Database => {
  const db = openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const flags = forumContentFlagsFromEnv(env)
  const log = options.log ?? defaultLog
  const postgres = postgresStoreForEnv(env, options)

  // The KS-8.10 remainder lane (#8338) composes its own mirror around this
  // one so the SAME forum write call sites cover the remainder tables with
  // no extra wiring. It shares the flags/binding and is a no-op in exactly
  // the cases below, preserving the zero-overhead identity fast path.
  const wrapRemainder = (base: D1Database): D1Database =>
    wrapForumRemainderMirroring(base, env, {
      log,
      makeSqlClient: options.makeSqlClient,
    })

  if (postgres === undefined || (!flags.dualWrite && flags.reads === 'd1')) {
    return wrapRemainder(db)
  }

  if (flags.reads === 'postgres') {
    // CFG D1 evacuation (#8515): the Cloudflare D1 bridge is 401-dead, so the
    // whole forum read/write surface must move OFF D1. `postgres` mode serves
    // every forum-domain SELECT from Postgres and executes every forum-domain
    // write on Postgres (authoritative) — see `forum-postgres-serving.ts`.
    // The serving wrapper sits at the same `db` boundary the repository reads
    // and writes through, so no repository call site changes. When the
    // KHALA_SYNC_DB binding is present (guaranteed here — `postgres` above is
    // defined), this replaces the compare/dual-write chain entirely: Postgres
    // is the authority, D1 is only the fail-soft fallback for a Postgres read
    // error (never reached for writes).
    const servingStore = forumServingStoreForEnv(env, options.makeSqlClient)
    if (servingStore !== undefined) {
      return makeForumPostgresServingDatabase({
        db,
        queryRows: servingStore.queryRows,
      })
    }
    // No serving store (should not happen when `postgres` is defined) —
    // fall through to the compare chain below rather than fail open.
  }

  return wrapRemainder(
    makeForumContentMirroringDatabase({
      compareStore: flags.reads === 'd1' ? undefined : postgres,
      db,
      log,
      mirror: flags.dualWrite
        ? makeForumContentMirror({ db, log, postgres })
        : undefined,
    }),
  )
}
