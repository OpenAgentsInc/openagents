// CFG D1 evacuation (epic #8515): forum Postgres READ-SERVING + WRITE
// cutover. The Cloudflare D1 `d1-http` bridge 401s account-wide (plan
// cancelled), so every forum read/write that still hits D1 is LIVE BROKEN
// (topic lists 500, threads 500, new posts fail). The forum content +
// remainder dual-write mirrors have kept Postgres current (1,313 posts,
// verified 2026-07-06), but `KHALA_SYNC_FORUM_READS=postgres` was INERT —
// the read-serving path was never built. This module builds it.
//
// THE SEAM (same shape as `business-domain-store.ts`'s `serveFromPostgres`,
// generalized to the WHOLE forum domain): the forum route feeds every
// repository read/write through ONE `db: D1Database`
// (`forumContentDatabaseForEnv`). This wrapper sits at that boundary and, in
// `postgres` mode:
//
//   READS  — a SELECT whose FROM/JOIN table refs are ALL forum-domain tables
//            (content ∪ remainder ∪ the five forum treasury-money tables) is
//            SERVED from Postgres via the shared `queryRows` seam, fail-soft
//            to the underlying D1 statement on any Postgres error. CTE names
//            declared in a `WITH [RECURSIVE] … AS` head are not treated as
//            base tables (e.g. `forum_post_ancestors`, a recursive CTE over
//            `forum_posts`, stays servable).
//   WRITES — an INSERT/UPDATE/DELETE targeting a forum-domain table is
//            EXECUTED on Postgres (authoritative — no D1 dependency), with
//            `INSERT OR IGNORE` translated to `… ON CONFLICT DO NOTHING`
//            (byte-identical dedup semantics). A write failure THROWS (unlike
//            a read, a silently-dropped write must never look like success).
//
// Everything else passes through to the underlying D1 handle untouched.
//
// ROW-SHAPE FIDELITY: postgres.js returns `bigint`/int8 columns (forum
// `post_count`, `post_number`, counts) as STRINGS, but the route decoders
// are strict `S.Number`. The forum serving client therefore overrides the
// int8 parser to `Number(...)` (all forum counters are small — no 2^53
// precision risk), so served rows are shape-identical to D1 rows and decode
// unchanged. Text timestamps (`created_at` etc. are `text` columns) already
// round-trip as strings.
//
// SQLite-only expressions that Postgres cannot evaluate (`json_extract` in
// the tip-stats aggregate + public search) live in reads that are DISPLAY
// overlays; their call sites are fail-soft at the Effect layer
// (`readForumPostTipStats` degrades to empty), so a Postgres serve failure
// there degrades the overlay instead of 500-ing the thread.

import {
  requireForumContentUnsafe,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from '../khala-sync-push-routes'
import { acquireSharedPostgresClient } from '../khala-sync-postgres-pool'
import { logWorkerRouteWarning } from '../observability'

/**
 * Convert D1 `?` placeholders to Postgres `$n` (quote-aware). Duplicated from
 * `forum-content-store.ts` so this serving module has no import cycle with the
 * content store (which imports the serving wrapper for `postgres` mode).
 */
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
// The forum-domain served table set
// ---------------------------------------------------------------------------

/** Content-lane tables (khala-sync migration 0014). */
const FORUM_CONTENT_SERVED_TABLES = [
  'forum_boards',
  'forum_categories',
  'forum_forums',
  'forum_topics',
  'forum_posts',
  'forum_post_bodies',
  'forum_post_revisions',
  'forum_actor_follows',
  'forum_watches',
  'forum_bookmarks',
  'forum_reports',
  'forum_moderation_events',
  'forum_context_links',
] as const

/** Remainder-lane tables (khala-sync migration 0027). */
const FORUM_REMAINDER_SERVED_TABLES = [
  'forum_acl_grants',
  'forum_notification_reads',
  'forum_private_message_threads',
  'forum_private_messages',
  'forum_score_snapshots',
  'forum_work_requests',
  'forum_work_request_relay_links',
  'forum_work_request_offers',
  'forum_work_request_lifecycle_posts',
  'forum_work_request_acceptances',
  'forum_work_request_results',
] as const

/**
 * The five forum treasury-money tables (KS-8.8 treasury lane). They are read
 * through the SAME forum `db` handle (never the treasury handle) by the
 * public thread/tip reads, and they share the SAME Postgres database, so the
 * generic `queryRows` seam serves them with no cross-domain wiring. Serving
 * these is display/lookup only — no payout/settlement DECISION reads through
 * this handle (those go through the treasury seam with its own flag).
 */
const FORUM_TREASURY_SERVED_TABLES = [
  'forum_money_actions',
  'forum_payment_events',
  'forum_receipts',
  'forum_tip_recipient_wallets',
  'forum_tip_settlement_claims',
] as const

export const FORUM_POSTGRES_SERVED_TABLES: ReadonlySet<string> =
  new Set<string>([
    ...FORUM_CONTENT_SERVED_TABLES,
    ...FORUM_REMAINDER_SERVED_TABLES,
    ...FORUM_TREASURY_SERVED_TABLES,
  ])

export const isForumServedTable = (table: string): boolean =>
  FORUM_POSTGRES_SERVED_TABLES.has(table.toLowerCase())

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

export type ForumServingDiagnosticEvent =
  | 'khala_sync_forum_postgres_read_serve_failed'
  | 'khala_sync_forum_postgres_write_serve_failed'
  | 'khala_sync_forum_postgres_serving_active'

export type ForumServingLog = (
  event: ForumServingDiagnosticEvent,
  fields: Readonly<{ op: string; messageSafe: string }>,
) => void

const defaultLog: ForumServingLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
  })
}

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

const statementHead = (sql: string): string =>
  sql.replaceAll(/\s+/g, ' ').trim().slice(0, 80)

// ---------------------------------------------------------------------------
// Statement classification
// ---------------------------------------------------------------------------

export type ForumServeClass =
  | Readonly<{ kind: 'serve-select' }>
  | Readonly<{ kind: 'serve-write' }>
  | Readonly<{ kind: 'passthrough' }>

const withoutStringLiterals = (sql: string): string =>
  sql.replaceAll(/'(?:[^']|'')*'/g, "''")

const SELECT_HEAD_RE = /^\s*(?:with\b[\s\S]*?\bselect\b|select\b)/i
const WRITE_HEAD_RE = /^\s*(insert|update|delete)\b/i
const TABLE_REF_RE = /\b(?:from|join)\s+([a-z_][a-z0-9_]*)/gi
const CTE_NAME_RE =
  /\b(?:with|,)\s+(?:recursive\s+)?([a-z_][a-z0-9_]*)\s*(?:\([^)]*\))?\s+as\s*\(/gi
const WRITE_TARGET_RE =
  /^\s*(?:insert(?:\s+or\s+(?:ignore|replace))?\s+into|update|delete\s+from)\s+([a-z_][a-z0-9_]*)/i

/** CTE aliases declared in a `WITH … AS (…)` head — not base tables. */
const cteNames = (sql: string): ReadonlySet<string> => {
  const names = new Set<string>()
  for (const match of withoutStringLiterals(sql).matchAll(CTE_NAME_RE)) {
    names.add(match[1]!.toLowerCase())
  }
  return names
}

/**
 * Classify one prepared statement for the forum serving seam:
 *  - SELECT (incl. `WITH … SELECT`) whose FROM/JOIN refs are all forum-domain
 *    tables (ignoring CTE self-references) → `serve-select`.
 *  - INSERT/UPDATE/DELETE targeting a forum-domain table → `serve-write`.
 *  - anything else → `passthrough`.
 */
export const classifyForumServeStatement = (sql: string): ForumServeClass => {
  if (WRITE_HEAD_RE.test(sql)) {
    const target = WRITE_TARGET_RE.exec(sql)
    if (target !== null && isForumServedTable(target[1]!)) {
      return { kind: 'serve-write' }
    }
    return { kind: 'passthrough' }
  }

  if (SELECT_HEAD_RE.test(sql)) {
    const ctes = cteNames(sql)
    const refs: Array<string> = []
    for (const match of withoutStringLiterals(sql).matchAll(TABLE_REF_RE)) {
      refs.push(match[1]!.toLowerCase())
    }
    const baseRefs = refs.filter(ref => !ctes.has(ref))
    if (baseRefs.length > 0 && baseRefs.every(ref => isForumServedTable(ref))) {
      return { kind: 'serve-select' }
    }
  }

  return { kind: 'passthrough' }
}

/**
 * Translate a D1/SQLite write to the Postgres dialect the serving seam runs:
 *  - `INSERT OR IGNORE INTO …` → `INSERT INTO … ON CONFLICT DO NOTHING`
 *    (SQLite `OR IGNORE` ignores conflicts on ANY unique/pk constraint, the
 *    exact semantics of a bare `ON CONFLICT DO NOTHING`).
 *  - `?` placeholders → `$n` (quote-aware, via `toPostgresPlaceholders`).
 * Plain INSERT/UPDATE/DELETE only get the placeholder rewrite. `col = col + 1`
 * counter bumps and `NULL`/literal VALUES tuples are already valid Postgres.
 */
export const translateForumWriteSql = (sql: string): string => {
  const orIgnore = /^(\s*)insert\s+or\s+ignore\s+into\b/i.exec(sql)
  if (orIgnore !== null) {
    const withoutOrIgnore = sql.replace(
      /^(\s*)insert\s+or\s+ignore\s+into\b/i,
      '$1INSERT INTO',
    )
    // Append ON CONFLICT DO NOTHING before any trailing semicolon, unless the
    // statement already declares its own ON CONFLICT clause.
    const hasOnConflict = /\bon\s+conflict\b/i.test(
      withoutStringLiterals(withoutOrIgnore),
    )
    const withConflict = hasOnConflict
      ? withoutOrIgnore
      : withoutOrIgnore.replace(/\s*;?\s*$/, ' ON CONFLICT DO NOTHING')
    return toPostgresPlaceholders(withConflict)
  }
  return toPostgresPlaceholders(sql)
}

// ---------------------------------------------------------------------------
// The serving D1Database proxy
// ---------------------------------------------------------------------------

/** The Postgres executor seam — runs one statement, returns its rows. */
export type ForumServeQueryRows = (
  text: string,
  params: ReadonlyArray<unknown>,
) => Promise<ReadonlyArray<Record<string, unknown>>>

export type MakeForumPostgresServingDatabaseDependencies = Readonly<{
  /** The underlying D1 handle (the compare/dual-write chain or raw D1). */
  db: D1Database
  /** The Postgres query executor (int8-coercing forum serving client). */
  queryRows: ForumServeQueryRows
  log?: ForumServingLog | undefined
}>

type BoundServe = Readonly<{
  statement: D1PreparedStatement
  classified: ForumServeClass
  sql: string
  params: ReadonlyArray<unknown>
}>

/**
 * Wrap one D1Database so forum-domain reads serve from Postgres (fail-soft to
 * D1) and forum-domain writes execute on Postgres (authoritative). All other
 * statements pass through untouched.
 */
export const makeForumPostgresServingDatabase = (
  deps: MakeForumPostgresServingDatabaseDependencies,
): D1Database => {
  const { db } = deps
  const log = deps.log ?? defaultLog

  const serveSelect = async <T>(
    bound: BoundServe,
    fallback: () => Promise<T>,
    shape: (rows: ReadonlyArray<Record<string, unknown>>) => T,
  ): Promise<T> => {
    try {
      const rows = await deps.queryRows(
        toPostgresPlaceholders(bound.sql),
        bound.params,
      )
      return shape(rows)
    } catch (error) {
      log('khala_sync_forum_postgres_read_serve_failed', {
        messageSafe: safeMessage(error),
        op: statementHead(bound.sql),
      })
      // Fail-soft: fall back to the underlying D1 statement (the caller's
      // Effect layer fail-softs the SQLite-only json_extract overlays).
      return fallback()
    }
  }

  const serveWrite = async <T>(bound: BoundServe): Promise<D1Result<T>> => {
    try {
      await deps.queryRows(translateForumWriteSql(bound.sql), bound.params)
      return {
        meta: {} as D1Meta,
        results: [] as T[],
        success: true,
      } as unknown as D1Result<T>
    } catch (error) {
      log('khala_sync_forum_postgres_write_serve_failed', {
        messageSafe: safeMessage(error),
        op: statementHead(bound.sql),
      })
      // A write must never silently look like success on failure.
      throw error
    }
  }

  const makeBound = (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): BoundServe => ({
    classified: classifyForumServeStatement(sql),
    params,
    sql,
    statement:
      params.length === 0 ? db.prepare(sql) : db.prepare(sql).bind(...params),
  })

  const wrapStatement = (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): D1PreparedStatement => {
    const bound = makeBound(sql, params)

    const wrapper = {
      __forumServeInner: bound,
      all: async <T>(...args: ReadonlyArray<unknown>) => {
        if (bound.classified.kind === 'serve-select') {
          return serveSelect<D1Result<T>>(
            bound,
            () =>
              (
                bound.statement.all as (
                  ...a: ReadonlyArray<unknown>
                ) => Promise<D1Result<T>>
              )(...args),
            rows =>
              ({
                meta: {} as D1Meta,
                results: rows as T[],
                success: true,
              }) as unknown as D1Result<T>,
          )
        }
        return (
          bound.statement.all as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<D1Result<T>>
        )(...args)
      },
      bind: (...values: ReadonlyArray<unknown>) => wrapStatement(sql, values),
      first: async <T>(...args: ReadonlyArray<unknown>) => {
        if (bound.classified.kind === 'serve-select' && args.length === 0) {
          return serveSelect<T | null>(
            bound,
            () =>
              (
                bound.statement.first as (
                  ...a: ReadonlyArray<unknown>
                ) => Promise<T | null>
              )(...args),
            rows => (rows[0] ?? null) as unknown as T | null,
          )
        }
        if (bound.classified.kind === 'serve-select') {
          // `first(column)` scalar reads: serve the row then project the
          // requested column, fail-soft to D1.
          const column = args[0] as string
          return serveSelect<T | null>(
            bound,
            () =>
              (
                bound.statement.first as (
                  ...a: ReadonlyArray<unknown>
                ) => Promise<T | null>
              )(...args),
            rows =>
              rows[0] === undefined
                ? null
                : ((rows[0]?.[column] ?? null) as unknown as T | null),
          )
        }
        return (
          bound.statement.first as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<T | null>
        )(...args)
      },
      raw: (...args: ReadonlyArray<unknown>) =>
        (
          bound.statement.raw as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<Array<Array<unknown>>>
        )(...args),
      run: async <T>(...args: ReadonlyArray<unknown>) => {
        if (bound.classified.kind === 'serve-write') {
          return serveWrite<T>(bound)
        }
        if (bound.classified.kind === 'serve-select') {
          return serveSelect<D1Result<T>>(
            bound,
            () =>
              (
                bound.statement.run as (
                  ...a: ReadonlyArray<unknown>
                ) => Promise<D1Result<T>>
              )(...args),
            rows =>
              ({
                meta: {} as D1Meta,
                results: rows as T[],
                success: true,
              }) as unknown as D1Result<T>,
          )
        }
        return (
          bound.statement.run as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<D1Result<T>>
        )(...args)
      },
    }

    return wrapper as unknown as D1PreparedStatement
  }

  const proxied = {
    batch: async <T>(statements: ReadonlyArray<D1PreparedStatement>) => {
      // Non-transactional in postgres mode (D1's batch atomicity is
      // unavailable when D1 is dead anyway). Execute each carried statement
      // against its resolved store, preserving request order.
      const results: Array<D1Result<T>> = []
      for (const statement of statements) {
        const carried = (
          statement as unknown as { __forumServeInner?: BoundServe }
        ).__forumServeInner
        if (carried === undefined) {
          results.push(
            await (
              statement.run as (
                ...a: ReadonlyArray<unknown>
              ) => Promise<D1Result<T>>
            )(),
          )
          continue
        }
        if (carried.classified.kind === 'serve-write') {
          results.push(await serveWrite<T>(carried))
        } else if (carried.classified.kind === 'serve-select') {
          results.push(
            await serveSelect<D1Result<T>>(
              carried,
              () =>
                (
                  carried.statement.run as (
                    ...a: ReadonlyArray<unknown>
                  ) => Promise<D1Result<T>>
                )(),
              rows =>
                ({
                  meta: {} as D1Meta,
                  results: rows as T[],
                  success: true,
                }) as unknown as D1Result<T>,
            ),
          )
        } else {
          results.push(
            await (
              carried.statement.run as (
                ...a: ReadonlyArray<unknown>
              ) => Promise<D1Result<T>>
            )(),
          )
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
// The int8-coercing forum serving Postgres client
// ---------------------------------------------------------------------------

/**
 * A postgres.js client factory that parses `bigint`/int8 (OID 20) columns to
 * JS `Number` so served forum rows (`post_count`, `post_number`, counts) are
 * shape-identical to D1 rows and decode against the strict `S.Number` route
 * schemas. All forum int8 columns are small counters — no 2^53 risk.
 */
export const makeForumServingSqlClient: MakeKhalaSyncPushSqlClient = async (
  connectionString,
) => {
  // On Cloud Run this reuses the shared int8→Number 'd1-bigint' pool instead
  // of a fresh connection per query.
  const { sql, end } = await acquireSharedPostgresClient({
    connectionString,
    options: {
      connect_timeout: 10,
      prepare: false,
      types: {
        // Override the built-in int8 parser: return Number, not string.
        bigint: {
          from: [20],
          parse: (value: string) => Number(value),
          serialize: (value: unknown) => String(value),
          to: 20,
        },
      },
    },
    variant: 'd1-bigint',
  })
  return {
    end,
    sql: sql as unknown as KhalaSyncPushSqlClient['sql'],
  }
}

// ---------------------------------------------------------------------------
// Env plumbing helper
// ---------------------------------------------------------------------------

export type ForumServingEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
}>

/** The forum Postgres serving store — just the read-only query executor. */
export type ForumServingStore = Readonly<{ queryRows: ForumServeQueryRows }>

/**
 * Build the Postgres serving store for the forum domain, or `undefined` when
 * there is no KHALA_SYNC_DB binding. Uses the int8-coercing serving client by
 * default; tests inject `makeSqlClient`. One client per query, always ended.
 */
export const forumServingStoreForEnv = (
  env: ForumServingEnv,
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined,
): ForumServingStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const factory =
    makeSqlClient ?? makeForumServingSqlClient ?? defaultMakeKhalaSyncSqlClient
  return {
    queryRows: async (text, params) => {
      const client = await factory(connectionString)
      try {
        return await requireForumContentUnsafe(client.sql as SyncSql)(text, [
          ...params,
        ])
      } finally {
        try {
          await client.end()
        } catch {
          // best-effort teardown, same discipline as the push route.
        }
      }
    },
  }
}
