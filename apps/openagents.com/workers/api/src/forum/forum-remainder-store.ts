// KS-8.10 remainder (#8338): forum remainder domain — D1 → Cloud SQL
// migration machinery for the THIRTEEN remainder forum tables that finish
// the KS-8.10 family behind the parent content lane
// (`forum-content-store.ts`, #8321). Same seam, same mirror recipe.
//
// Domain tables (khala-sync migration `0026_forum_remainder.sql`):
// `forum_private_message_threads`, `forum_private_messages` (PRIVATE),
// `forum_acl_grants`, `forum_trust_edges`, `forum_actor_forum_trust`
// (DERIVED), `forum_score_snapshots` (DERIVED), `forum_notification_reads`,
// and the work-request lifecycle family (6) `forum_work_requests`,
// `forum_work_request_relay_links`, `forum_work_request_offers`,
// `forum_work_request_lifecycle_posts`, `forum_work_request_acceptances`,
// `forum_work_request_results`.
//
// THE SEAM: identical to the content lane. Every scoped write lives behind
// a `db: D1Database` first argument (forum `repository.ts`, `forum-work-
// requests.ts`, `forum-work-request-negotiation.ts`), all reached from the
// forum route with the SAME wrapped db. This module exports
// `wrapForumRemainderMirroring`, which the content lane's
// `forumContentDatabaseForEnv` COMPOSES around its own wrapper so the
// existing forum write call sites cover the remainder tables with NO new
// wiring. The content classifier treats remainder tables as passthrough
// (they were out of the #8321 scope), and this classifier treats content
// tables as passthrough — the two wrappers nest cleanly, each mirroring
// only its own tables. A mirror failure NEVER fails the request; it logs
// `khala_sync_forum_dual_write_failed`.
//
// PRIVACY: `forum_private_message_threads` / `forum_private_messages` carry
// sensitive content (behind `content_ref` / `participant_refs_json`). The
// read-back mirror only ever logs row KEYS (ids) on failure and never a
// subject, participant, or body — same discipline the diagnostics enforce
// for every table here.
//
// Flags are SHARED with the content lane (`KHALA_SYNC_FORUM_DUAL_WRITE` /
// `KHALA_SYNC_FORUM_READS`), read via `forumContentFlagsFromEnv`. Postgres
// read serving is deferred lane-wide by the content wrapper (which emits
// the single `khala_sync_forum_postgres_reads_deferred` diagnostic); this
// wrapper treats `postgres` as `compare` SILENTLY so the deferral is
// logged exactly once for the whole forum read surface.

import {
  FORUM_REMAINDER_TABLE_COLUMNS,
  FORUM_REMAINDER_TABLE_PK,
  isForumRemainderTable,
  normalizeForumContentValue,
  requireForumContentUnsafe,
  upsertForumRemainderRows,
  type ForumRemainderRow,
  type ForumRemainderTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from '../khala-sync-push-routes'
import { logWorkerRouteWarning } from '../observability'
import {
  forumContentFlagsFromEnv,
  toPostgresPlaceholders,
  type ForumContentFlags,
  type ForumContentStoreEnv,
} from './forum-content-store'

export type { ForumRemainderRow, ForumRemainderTable }

// ---------------------------------------------------------------------------
// Diagnostics — the SAME event vocabulary as the content lane
// ---------------------------------------------------------------------------

export type ForumRemainderDiagnosticEvent =
  | 'khala_sync_forum_dual_write_failed'
  | 'khala_sync_forum_write_unclassified'
  | 'khala_sync_forum_read_compare_mismatch'
  | 'khala_sync_forum_read_compare_failed'

export type ForumRemainderDiagnostic = Readonly<{
  /** Store op, e.g. 'mirror:forum_private_messages' or a statement head. */
  op: string
  /** Public-safe refs — row KEYS only (ids). NEVER subjects/bodies. */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no values). */
  messageSafe: string
}>

export type ForumRemainderLog = (
  event: ForumRemainderDiagnosticEvent,
  fields: ForumRemainderDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

const statementHead = (sql: string): string =>
  sql.replaceAll(/\s+/g, ' ').trim().slice(0, 80)

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

export type ForumRemainderWriteStore = Readonly<{
  upsertRows: (
    table: ForumRemainderTable,
    rows: ReadonlyArray<ForumRemainderRow>,
  ) => Promise<number>
}>

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresForumRemainderStore = ForumRemainderWriteStore &
  Readonly<{
    queryRows: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }>

export type MakePostgresForumRemainderStoreDependencies = Readonly<{
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresForumRemainderStore = (
  deps: MakePostgresForumRemainderStoreDependencies,
): PostgresForumRemainderStore => {
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
      withSql(sql => upsertForumRemainderRows(sql, table, rows)),
  }
}

// ---------------------------------------------------------------------------
// D1 twin of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

export const makeD1ForumRemainderWriteStore = (
  db: D1Database,
): ForumRemainderWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const columns = FORUM_REMAINDER_TABLE_COLUMNS[table]
    const pk = FORUM_REMAINDER_TABLE_PK[table]
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

export type MakeDualWriteForumRemainderWriteStoreDependencies = Readonly<{
  d1: ForumRemainderWriteStore
  postgres: ForumRemainderWriteStore | undefined
  flags: ForumContentFlags
  log?: ForumRemainderLog | undefined
}>

export const makeDualWriteForumRemainderWriteStore = (
  deps: MakeDualWriteForumRemainderWriteStoreDependencies,
): ForumRemainderWriteStore => {
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
            .map(row => String(row[FORUM_REMAINDER_TABLE_PK[table]] ?? '')),
        })
      }
      return outcome
    },
  }
}

// ---------------------------------------------------------------------------
// The read-back mirror
// ---------------------------------------------------------------------------

export type ForumRemainderMirror = Readonly<{
  mirrorRowsByPk: (
    table: ForumRemainderTable,
    pkValues: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeForumRemainderMirrorDependencies = Readonly<{
  db: D1Database
  postgres: ForumRemainderWriteStore
  log: ForumRemainderLog
}>

export const makeForumRemainderMirror = (
  deps: MakeForumRemainderMirrorDependencies,
): ForumRemainderMirror => {
  const { db, log, postgres } = deps

  return {
    mirrorRowsByPk: async (table, pkValues) => {
      if (pkValues.length === 0) {
        return
      }
      try {
        const pk = FORUM_REMAINDER_TABLE_PK[table]
        const placeholders = pkValues.map(() => '?').join(', ')
        const rows = await db
          .prepare(`SELECT * FROM ${table} WHERE ${pk} IN (${placeholders})`)
          .bind(...pkValues)
          .all<ForumRemainderRow>()
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
// Statement classification (the closed remainder write set)
// ---------------------------------------------------------------------------

export type ForumRemainderPkSource =
  | Readonly<{ kind: 'bind'; index: number }>
  | Readonly<{ kind: 'literal'; value: string }>

export type ForumRemainderStatementClass =
  | Readonly<{
      kind: 'mirrored-write'
      table: ForumRemainderTable
      pkSource: ForumRemainderPkSource
    }>
  | Readonly<{ kind: 'unclassified-write'; table: ForumRemainderTable }>
  | Readonly<{ kind: 'comparable-select' }>
  | Readonly<{ kind: 'passthrough' }>

const withoutStringLiterals = (sql: string): string =>
  sql.replaceAll(/'(?:[^']|'')*'/g, "''")

const countBinds = (sql: string): number =>
  (withoutStringLiterals(sql).match(/\?/g) ?? []).length

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
 * Classify one prepared statement against the remainder table set. Same
 * parser as the content lane, scoped to `isForumRemainderTable`:
 *  - INSERT [OR IGNORE|REPLACE]: the PK's tuple item is a `?` (bind index =
 *    `?` items before it) or a quoted literal.
 *  - UPDATE … WHERE … <pk> = ?|'literal': bind index = binds in SET + binds
 *    in WHERE before the PK equality.
 *  - other scoped INSERT/UPDATE/DELETE → `unclassified-write` (loud, still
 *    fail-soft).
 *  - SELECTs whose from/join refs are ALL remainder tables →
 *    `comparable-select`.
 */
export const classifyForumRemainderStatement = (
  sql: string,
): ForumRemainderStatementClass => {
  const insertMatch = INSERT_RE.exec(sql)
  if (insertMatch !== null) {
    const table = insertMatch[1]!.toLowerCase()
    if (!isForumRemainderTable(table)) {
      return { kind: 'passthrough' }
    }
    const columns = insertMatch[2]!
      .split(',')
      .map(column => column.trim().toLowerCase())
    const items = splitTupleItems(insertMatch[3]!)
    const pk = FORUM_REMAINDER_TABLE_PK[table]
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
    if (!isForumRemainderTable(table)) {
      return { kind: 'passthrough' }
    }
    const pk = FORUM_REMAINDER_TABLE_PK[table]
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
    const touched = new Set<string>()
    for (const match of withoutStringLiterals(sql).matchAll(
      /\b(?:into|update|from)\s+([a-z_][a-z0-9_]*)/gi,
    )) {
      touched.add(match[1]!.toLowerCase())
    }
    for (const table of touched) {
      if (isForumRemainderTable(table)) {
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
    if (refs.length > 0 && refs.every(ref => isForumRemainderTable(ref))) {
      return { kind: 'comparable-select' }
    }
  }

  return { kind: 'passthrough' }
}

export const resolveForumRemainderPk = (
  pkSource: ForumRemainderPkSource,
  params: ReadonlyArray<unknown>,
): string | undefined => {
  if (pkSource.kind === 'literal') {
    return pkSource.value
  }
  const value = params[pkSource.index]
  return value === undefined || value === null ? undefined : String(value)
}

// ---------------------------------------------------------------------------
// The mirroring D1Database (production dual-write wiring)
// ---------------------------------------------------------------------------

const stableRowString = (row: Record<string, unknown>): string => {
  const entries = Object.entries(row)
    .map(
      ([key, value]) => [key, normalizeForumContentValue(value)] as const,
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

export type MakeForumRemainderMirroringDatabaseDependencies = Readonly<{
  db: D1Database
  mirror: ForumRemainderMirror | undefined
  compareStore: PostgresForumRemainderStore | undefined
  log: ForumRemainderLog
}>

type BoundStatement = Readonly<{
  statement: D1PreparedStatement
  onWriteSuccess: (() => Promise<void>) | undefined
}>

/**
 * Wrap one D1Database so every successful write to a scoped remainder table
 * read-back-mirrors the affected row into Postgres, and (compare mode)
 * every scoped-table SELECT shadow-runs against the twin with D1 always
 * served. All other statements pass through untouched — including forum
 * CONTENT writes, which the inner content wrapper mirrors. Fail-soft
 * everywhere.
 */
export const makeForumRemainderMirroringDatabase = (
  deps: MakeForumRemainderMirroringDatabaseDependencies,
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
    const classified = classifyForumRemainderStatement(sql)

    if (classified.kind === 'unclassified-write') {
      return {
        onWriteSuccess: () => {
          log('khala_sync_forum_write_unclassified', {
            messageSafe:
              'scoped forum remainder table write did not classify; postgres twin may drift until the next backfill sweep',
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
          const pk = resolveForumRemainderPk(classified.pkSource, params)
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
    const classified = classifyForumRemainderStatement(sql)
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
      __forumRemainderInner: bound,
    }

    return wrapper as unknown as D1PreparedStatement
  }

  const proxied = {
    batch: async <T>(statements: ReadonlyArray<D1PreparedStatement>) => {
      const inners = statements.map(statement => {
        const carried = (
          statement as unknown as {
            __forumRemainderInner?: BoundStatement
          }
        ).__forumRemainderInner
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
// The composition entry point (wraps the content-lane db)
// ---------------------------------------------------------------------------

export type MakeForumRemainderStoreOptions = Readonly<{
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: ForumRemainderLog | undefined
}>

const defaultLog: ForumRemainderLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: ForumContentStoreEnv,
  options: MakeForumRemainderStoreOptions,
): PostgresForumRemainderStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresForumRemainderStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * Compose the remainder mirror around an already-resolved forum D1Database
 * (the content-lane wrapper or a raw db). Returns `base` unchanged when
 * there is no KHALA_SYNC_DB binding or when dual-write is off AND reads are
 * 'd1' — so the fast path stays zero-overhead. `reads=postgres` is treated
 * as `compare` SILENTLY: the content lane already logs the single
 * lane-wide `khala_sync_forum_postgres_reads_deferred` diagnostic.
 */
export const wrapForumRemainderMirroring = (
  base: D1Database,
  env: ForumContentStoreEnv,
  options: MakeForumRemainderStoreOptions = {},
): D1Database => {
  const flags = forumContentFlagsFromEnv(env)
  const postgres = postgresStoreForEnv(env, options)
  if (postgres === undefined || (!flags.dualWrite && flags.reads === 'd1')) {
    return base
  }
  const log = options.log ?? defaultLog

  return makeForumRemainderMirroringDatabase({
    compareStore: flags.reads === 'd1' ? undefined : postgres,
    db: base,
    log,
    mirror: flags.dualWrite
      ? makeForumRemainderMirror({ db: base, log, postgres })
      : undefined,
  })
}
