// KS-8.13 (#8324): Khala Code product-state migration + sync-scope adoption.
//
// This is the D1-shaped seam for thread/team/workspace state. Production
// writes still commit to D1 first. After a successful D1 write, this wrapper
// reads the accepted D1 row back, converge-upserts it into the Cloud SQL twin,
// and appends Khala Sync changelog entries for the thread/team scopes the row
// belongs to. Mirror failures are fail-soft diagnostics, never request
// failures; D1 remains authoritative until the issue closeout/runbook cutover.

import { identityDbForEnv, readIdentityUserProfiles, type IdentityDb } from './identity-db'
import {
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
  deleteKhalaCodeProductStateRows,
  isKhalaCodeProductStateTable,
  normalizeKhalaCodeProductStateValue,
  scopeChangesForKhalaCodeProductStateRow,
  scopeTombstonesForKhalaCodeProductStateRow,
  upsertKhalaCodeProductStateRows,
  withSyncTransaction,
  type KhalaCodeProductStateRow,
  type KhalaCodeProductStateTable,
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

export type KhalaCodeProductStateFlags = Readonly<{
  dualWrite: boolean
}>

export type KhalaCodeProductStateFlagEnv = Readonly<{
  KHALA_SYNC_KHALA_CODE_STATE_DUAL_WRITE?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

export const khalaCodeProductStateFlagsFromEnv = (
  env: KhalaCodeProductStateFlagEnv,
): KhalaCodeProductStateFlags => {
  const dualWriteRaw =
    env.KHALA_SYNC_KHALA_CODE_STATE_DUAL_WRITE?.trim().toLowerCase()
  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
  }
}

export type KhalaCodeProductStateDiagnosticEvent =
  | 'khala_sync_khala_code_state_dual_write_failed'
  | 'khala_sync_khala_code_state_write_unclassified'
  | 'khala_sync_khala_code_state_projection_skipped'

export type KhalaCodeProductStateDiagnostic = Readonly<{
  op: string
  refs: ReadonlyArray<string>
  messageSafe: string
}>

export type KhalaCodeProductStateLog = (
  event: KhalaCodeProductStateDiagnosticEvent,
  fields: KhalaCodeProductStateDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

const statementHead = (sql: string): string =>
  sql.replaceAll(/\s+/g, ' ').trim().slice(0, 80)

// ---------------------------------------------------------------------------
// Postgres mirror + changelog projection
// ---------------------------------------------------------------------------

export type KhalaCodeProductStateMirror = Readonly<{
  /**
   * Converge a hard-delete on the Postgres twin and append `op:"delete"`
   * tombstones for the removed rows. `deletedRows` are the rows READ from D1
   * BEFORE the delete executed (their scope/key columns are gone afterward);
   * the mirror resolves each row's scope targets and appends one tombstone per
   * scope so subscribers converge on the removal. Omitted/empty `deletedRows`
   * (a delete that matched nothing, or a scopeless table) append no tombstone.
   */
  deleteRows: (
    table: KhalaCodeProductStateTable,
    whereColumns: ReadonlyArray<string>,
    whereValues: ReadonlyArray<unknown>,
    deletedRows?: ReadonlyArray<KhalaCodeProductStateRow>,
  ) => Promise<void>
  upsertRows: (
    table: KhalaCodeProductStateTable,
    rows: ReadonlyArray<KhalaCodeProductStateRow>,
  ) => Promise<void>
}>

export type MakeKhalaCodeProductStateMirrorDependencies = Readonly<{
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
  /**
   * Fail-soft projection-skip diagnostic: called when a mirrored row cannot
   * be allowlist-mapped into its public-safe contract entity (or trips the
   * redaction guard). The Postgres row still converges; only the scope
   * changelog entry is withheld.
   */
  onProjectionSkip?:
    | ((table: KhalaCodeProductStateTable, reasonSafe: string) => void)
    | undefined
}>

export const makePostgresKhalaCodeProductStateMirror = (
  deps: MakeKhalaCodeProductStateMirrorDependencies,
): KhalaCodeProductStateMirror => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, matching the Khala Sync route stores.
      }
    }
  }

  return {
    deleteRows: (table, whereColumns, whereValues, deletedRows) =>
      withSql(sql =>
        withSyncTransaction(sql, async writer => {
          await deleteKhalaCodeProductStateRows(
            writer.sql,
            table,
            whereColumns,
            whereValues,
          )
          for (const row of deletedRows ?? []) {
            for (const tombstone of scopeTombstonesForKhalaCodeProductStateRow(
              table,
              row,
            )) {
              await writer.appendChange({
                entityId: tombstone.entityId,
                entityType: tombstone.entityType,
                mutationRef: `d1-shadow:ks-8.13:${table}`,
                op: 'delete',
                scope: tombstone.scope,
              })
            }
          }
        }),
      ),
    upsertRows: (table, rows) =>
      withSql(sql =>
        withSyncTransaction(sql, async writer => {
          await upsertKhalaCodeProductStateRows(writer.sql, table, rows)
          for (const row of rows) {
            for (const change of scopeChangesForKhalaCodeProductStateRow(
              table,
              row,
              deps.onProjectionSkip,
            )) {
              await writer.appendChange({
                entityId: change.entityId,
                entityType: change.entityType,
                mutationRef: `d1-shadow:ks-8.13:${table}`,
                op: 'upsert',
                postImage: change.postImage,
                scope: change.scope,
              })
            }
          }
        }),
      ),
  }
}

// ---------------------------------------------------------------------------
// Statement classification
// ---------------------------------------------------------------------------

export type ValueSource =
  | Readonly<{ kind: 'bind'; index: number }>
  | Readonly<{ kind: 'literal'; value: string }>

export type MirroredWhere = Readonly<{
  columns: ReadonlyArray<string>
  sources: ReadonlyArray<ValueSource>
}>

export type KhalaCodeProductStateStatementClass =
  | Readonly<{
      kind: 'mirrored-upsert'
      table: KhalaCodeProductStateTable
      where: MirroredWhere
    }>
  | Readonly<{
      kind: 'mirrored-delete'
      table: KhalaCodeProductStateTable
      where: MirroredWhere
    }>
  | Readonly<{ kind: 'unclassified-write'; table: KhalaCodeProductStateTable }>
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

const valueSourceFromItem = (
  item: string,
  bindIndex: number,
): ValueSource | undefined => {
  if (item === '?') {
    return { index: bindIndex, kind: 'bind' }
  }
  const literalMatch = /^'((?:[^']|'')*)'$/.exec(item)
  if (literalMatch !== null) {
    return {
      kind: 'literal',
      value: literalMatch[1]!.replaceAll("''", "'"),
    }
  }
  return undefined
}

const INSERT_VALUES_RE =
  /^\s*insert\s+(?:or\s+(?:ignore|replace)\s+)?into\s+([a-z_][a-z0-9_]*)\s*\(([^)]*)\)\s*values\s*\(([\s\S]*?)\)(?:\s+on\s+conflict[\s\S]*)?\s*;?\s*$/i

const INSERT_SELECT_RE =
  /^\s*insert\s+(?:or\s+(?:ignore|replace)\s+)?into\s+([a-z_][a-z0-9_]*)\s*\(([^)]*)\)\s*select\s+([\s\S]*?)\s+from\s+/i

const UPDATE_RE =
  /^\s*update\s+([a-z_][a-z0-9_]*)\s+set\s+([\s\S]*?)\s+where\s+([\s\S]*?);?\s*$/i

const DELETE_RE =
  /^\s*delete\s+from\s+([a-z_][a-z0-9_]*)\s+where\s+([\s\S]*?);?\s*$/i

const WRITE_HEAD_RE = /^\s*(insert|update|delete)\b/i

const whereForInsertedColumns = (
  table: KhalaCodeProductStateTable,
  columns: ReadonlyArray<string>,
  items: ReadonlyArray<string>,
  keyColumns: ReadonlyArray<string> = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[
    table
  ].keyColumns,
): MirroredWhere | undefined => {
  if (columns.length !== items.length) {
    return undefined
  }
  const sources: Array<ValueSource> = []
  for (const keyColumn of keyColumns) {
    const columnIndex = columns.indexOf(keyColumn)
    if (columnIndex === -1) {
      return undefined
    }
    const bindIndex = items.slice(0, columnIndex).filter(item => item === '?')
      .length
    const source = valueSourceFromItem(items[columnIndex]!, bindIndex)
    if (source === undefined) {
      return undefined
    }
    sources.push(source)
  }
  return { columns: keyColumns, sources }
}

const conflictTargetMatches = (
  sql: string,
  columns: ReadonlyArray<string>,
): boolean => {
  const normalized = withoutStringLiterals(sql)
    .replaceAll(/\s+/g, ' ')
    .toLowerCase()
  const body = columns.map(column => `\\s*${column}\\s*`).join(',')
  return new RegExp(`\\bon\\s+conflict\\s*\\(${body}\\)`).test(normalized)
}

const whereForInsertConflictTarget = (
  table: KhalaCodeProductStateTable,
  sql: string,
  columns: ReadonlyArray<string>,
  items: ReadonlyArray<string>,
): MirroredWhere | undefined => {
  if (table === 'teams' && conflictTargetMatches(sql, ['slug'])) {
    return whereForInsertedColumns(table, columns, items, ['slug'])
  }
  if (table === 'team_projects' && conflictTargetMatches(sql, ['team_id', 'slug'])) {
    return whereForInsertedColumns(table, columns, items, ['team_id', 'slug'])
  }
  return undefined
}

const THREAD_FILE_MESSAGE_REFS_INSERT_SELECT_RE =
  /^\s*insert\s+or\s+ignore\s+into\s+thread_file_message_refs\b/i

const classifyInsertSelect = (
  table: KhalaCodeProductStateTable,
  sql: string,
): KhalaCodeProductStateStatementClass => {
  if (
    table === 'thread_file_message_refs' &&
    THREAD_FILE_MESSAGE_REFS_INSERT_SELECT_RE.test(sql)
  ) {
    return {
      kind: 'mirrored-upsert',
      table,
      where: { columns: ['id'], sources: [{ index: 0, kind: 'bind' }] },
    }
  }
  return { kind: 'unclassified-write', table }
}

const sourceForWhereColumn = (
  whereClause: string,
  column: string,
  bindOffset: number,
): ValueSource | undefined => {
  const equality = new RegExp(
    `\\b${column}\\s*=\\s*(\\?|'(?:[^']|'')*')`,
    'i',
  ).exec(whereClause)
  if (equality === null) {
    return undefined
  }
  const matched = equality[1]!
  if (matched === '?') {
    return {
      index: bindOffset + countBinds(whereClause.slice(0, equality.index)),
      kind: 'bind',
    }
  }
  return {
    kind: 'literal',
    value: matched.slice(1, -1).replaceAll("''", "'"),
  }
}

const whereForColumns = (
  columns: ReadonlyArray<string>,
  whereClause: string,
  bindOffset: number,
): MirroredWhere | undefined => {
  const sources: Array<ValueSource> = []
  for (const column of columns) {
    const source = sourceForWhereColumn(whereClause, column, bindOffset)
    if (source === undefined) {
      return undefined
    }
    sources.push(source)
  }
  return { columns, sources }
}

export const classifyKhalaCodeProductStateStatement = (
  sql: string,
): KhalaCodeProductStateStatementClass => {
  const insertValuesMatch = INSERT_VALUES_RE.exec(sql)
  if (insertValuesMatch !== null) {
    const tableName = insertValuesMatch[1]!.toLowerCase()
    if (!isKhalaCodeProductStateTable(tableName)) {
      return { kind: 'passthrough' }
    }
    const columns = insertValuesMatch[2]!
      .split(',')
      .map(column => column.trim().toLowerCase())
    const items = splitTupleItems(insertValuesMatch[3]!)
    const where =
      whereForInsertConflictTarget(tableName, sql, columns, items) ??
      whereForInsertedColumns(tableName, columns, items)
    return where === undefined
      ? { kind: 'unclassified-write', table: tableName }
      : { kind: 'mirrored-upsert', table: tableName, where }
  }

  const insertSelectMatch = INSERT_SELECT_RE.exec(sql)
  if (insertSelectMatch !== null) {
    const tableName = insertSelectMatch[1]!.toLowerCase()
    if (!isKhalaCodeProductStateTable(tableName)) {
      return { kind: 'passthrough' }
    }
    return classifyInsertSelect(tableName, sql)
  }

  const updateMatch = UPDATE_RE.exec(sql)
  if (updateMatch !== null) {
    const tableName = updateMatch[1]!.toLowerCase()
    if (!isKhalaCodeProductStateTable(tableName)) {
      return { kind: 'passthrough' }
    }
    const spec = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[tableName]
    const where = whereForColumns(
      spec.keyColumns,
      updateMatch[3]!,
      countBinds(updateMatch[2]!),
    )
    return where === undefined
      ? { kind: 'unclassified-write', table: tableName }
      : { kind: 'mirrored-upsert', table: tableName, where }
  }

  const deleteMatch = DELETE_RE.exec(sql)
  if (deleteMatch !== null) {
    const tableName = deleteMatch[1]!.toLowerCase()
    if (!isKhalaCodeProductStateTable(tableName)) {
      return { kind: 'passthrough' }
    }
    const spec = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[tableName]
    const shareRecipientsByShare =
      tableName === 'share_projection_recipients'
        ? whereForColumns(['share_id'], deleteMatch[2]!, 0)
        : undefined
    const where =
      shareRecipientsByShare ??
      whereForColumns(spec.keyColumns, deleteMatch[2]!, 0)
    return where === undefined
      ? { kind: 'unclassified-write', table: tableName }
      : { kind: 'mirrored-delete', table: tableName, where }
  }

  if (WRITE_HEAD_RE.test(sql)) {
    const touched = new Set<string>()
    for (const match of withoutStringLiterals(sql).matchAll(
      /\b(?:into|update|from)\s+([a-z_][a-z0-9_]*)/gi,
    )) {
      touched.add(match[1]!.toLowerCase())
    }
    for (const table of touched) {
      if (isKhalaCodeProductStateTable(table)) {
        return { kind: 'unclassified-write', table }
      }
    }
  }

  return { kind: 'passthrough' }
}

export const resolveValueSource = (
  source: ValueSource,
  params: ReadonlyArray<unknown>,
): unknown | undefined =>
  source.kind === 'literal' ? source.value : params[source.index]

export const resolveWhereValues = (
  where: MirroredWhere,
  params: ReadonlyArray<unknown>,
): ReadonlyArray<unknown> | undefined => {
  const values = where.sources.map(source => resolveValueSource(source, params))
  return values.some(value => value === undefined) ? undefined : values
}

// ---------------------------------------------------------------------------
// D1Database wrapper
// ---------------------------------------------------------------------------

export type MakeKhalaCodeProductStateMirroringDatabaseDependencies = Readonly<{
  db: D1Database
  /** CFG-4 Domain 2 (#8519): Postgres identity handle for the
   * team_chat_messages author-field projection enrichment. */
  identityDb: IdentityDb
  log: KhalaCodeProductStateLog
  mirror: KhalaCodeProductStateMirror | undefined
}>

type BoundStatement = Readonly<{
  statement: D1PreparedStatement
  /**
   * Runs BEFORE the D1 write commits. Used by hard-delete mirroring to read
   * the rows the delete will remove while their scope/key columns still exist,
   * so tombstones can be resolved after the row is gone.
   */
  onBeforeWrite: (() => Promise<void>) | undefined
  onWriteSuccess: (() => Promise<void>) | undefined
}>

// KS-6.11 (#8422): `team_chat_messages` is the one product-state table whose
// scope-projected post-image needs a denormalized field the raw table row
// does not carry — the author's display name/avatar/GitHub username, which
// the legacy `readTeamChatMessageById` wire payload already sends
// (`team-chat.ts`). CFG-4 Domain 2 (#8519): `users`/`auth_identities` are
// Postgres-authoritative, so the old D1 LEFT JOIN is replaced by an
// identity-handle enrichment after the raw D1 read (same LEFT JOIN
// semantics: a message whose author user row is missing/deleted still
// mirrors and projects, just with null author-identity fields). The
// enriched columns are NOT part of `team_chat_messages`'s Postgres column
// spec (`khala-code-product-state-tables.ts`), so
// `upsertKhalaCodeProductStateRows` silently ignores them on the row it
// writes — only `scopeChangesForKhalaCodeProductStateRow`'s projection
// mapper reads them.
const enrichTeamChatMessageAuthorRows = async (
  identityDb: IdentityDb,
  rows: ReadonlyArray<KhalaCodeProductStateRow>,
): Promise<ReadonlyArray<KhalaCodeProductStateRow>> => {
  const profiles = await readIdentityUserProfiles(
    identityDb,
    rows.map(row => String(row.author_user_id ?? '')),
  )
  return rows.map(row => {
    const profile = profiles.get(String(row.author_user_id ?? ''))
    return {
      ...row,
      author_avatar_url: profile?.avatarUrl ?? null,
      author_github_username: profile?.githubUsername ?? null,
      author_name: profile?.displayName ?? null,
    }
  })
}

const readRowsByWhere = async (
  db: D1Database,
  identityDb: IdentityDb,
  table: KhalaCodeProductStateTable,
  columns: ReadonlyArray<string>,
  values: ReadonlyArray<unknown>,
): Promise<ReadonlyArray<KhalaCodeProductStateRow>> => {
  const normalized = values.map(normalizeKhalaCodeProductStateValue)

  const clauses = columns.map(column => `${column} IS ?`).join(' AND ')
  const rows = await db
    .prepare(`SELECT * FROM ${table} WHERE ${clauses}`)
    .bind(...normalized)
    .all<KhalaCodeProductStateRow>()

  if (table === 'team_chat_messages') {
    return enrichTeamChatMessageAuthorRows(identityDb, rows.results ?? [])
  }

  return rows.results ?? []
}

export const makeKhalaCodeProductStateMirroringDatabase = (
  deps: MakeKhalaCodeProductStateMirroringDatabaseDependencies,
): D1Database => {
  const { db, identityDb, log, mirror } = deps

  const makeBound = (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): BoundStatement => {
    const statement =
      params.length === 0
        ? db.prepare(sql)
        : db.prepare(sql).bind(...params)
    const classified = classifyKhalaCodeProductStateStatement(sql)

    if (classified.kind === 'unclassified-write') {
      return {
        onBeforeWrite: undefined,
        onWriteSuccess: () => {
          log('khala_sync_khala_code_state_write_unclassified', {
            messageSafe:
              'Khala Code product-state write did not classify; Postgres twin may drift until the next backfill sweep',
            op: statementHead(sql),
            refs: [classified.table],
          })
          return Promise.resolve()
        },
        statement,
      }
    }

    if (mirror === undefined) {
      return { onBeforeWrite: undefined, onWriteSuccess: undefined, statement }
    }

    if (classified.kind === 'mirrored-delete') {
      // Read the rows the delete will remove BEFORE it commits, so their
      // scope/key columns are still available to resolve delete tombstones.
      let capturedRows: ReadonlyArray<KhalaCodeProductStateRow> = []
      return {
        onBeforeWrite: async () => {
          const values = resolveWhereValues(classified.where, params)
          if (values === undefined) {
            return
          }
          try {
            capturedRows = await readRowsByWhere(
              db,
              identityDb,
              classified.table,
              classified.where.columns,
              values,
            )
          } catch {
            // Best-effort tombstone capture: a failed pre-read still lets the
            // D1 delete + Postgres converge proceed; only the scope tombstone
            // is withheld (fail-soft, like the projection skip path).
            capturedRows = []
          }
        },
        onWriteSuccess: async () => {
          const values = resolveWhereValues(classified.where, params)
          if (values === undefined) {
            return
          }
          try {
            await mirror.deleteRows(
              classified.table,
              classified.where.columns,
              values,
              capturedRows,
            )
          } catch (error) {
            log('khala_sync_khala_code_state_dual_write_failed', {
              messageSafe: safeMessage(error),
              op: `${classified.kind}:${classified.table}`,
              refs: values.slice(0, 10).map(String),
            })
          }
        },
        statement,
      }
    }

    if (classified.kind === 'mirrored-upsert') {
      return {
        onBeforeWrite: undefined,
        onWriteSuccess: async () => {
          const values = resolveWhereValues(classified.where, params)
          if (values === undefined) {
            return
          }
          try {
            const rows = await readRowsByWhere(
              db,
              identityDb,
              classified.table,
              classified.where.columns,
              values,
            )
            await mirror.upsertRows(classified.table, rows)
          } catch (error) {
            log('khala_sync_khala_code_state_dual_write_failed', {
              messageSafe: safeMessage(error),
              op: `${classified.kind}:${classified.table}`,
              refs: values.slice(0, 10).map(String),
            })
          }
        },
        statement,
      }
    }

    return { onBeforeWrite: undefined, onWriteSuccess: undefined, statement }
  }

  const wrapStatement = (
    sql: string,
    params: ReadonlyArray<unknown>,
  ): D1PreparedStatement => {
    const bound = makeBound(sql, params)
    const wrapper = {
      bind: (...values: ReadonlyArray<unknown>) =>
        wrapStatement(sql, values),
      all: <T>(...args: ReadonlyArray<unknown>) =>
        (
          bound.statement.all as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<D1Result<T>>
        )(...args),
      first: <T>(...args: ReadonlyArray<unknown>) =>
        (
          bound.statement.first as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<T | null>
        )(...args),
      raw: (...args: ReadonlyArray<unknown>) =>
        (
          bound.statement.raw as (
            ...a: ReadonlyArray<unknown>
          ) => Promise<Array<Array<unknown>>>
        )(...args),
      run: async <T>(...args: ReadonlyArray<unknown>) => {
        if (bound.onBeforeWrite !== undefined) {
          await bound.onBeforeWrite()
        }
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
      __khalaCodeProductStateInner: bound,
    }
    return wrapper as unknown as D1PreparedStatement
  }

  const proxied = {
    batch: async <T>(statements: ReadonlyArray<D1PreparedStatement>) => {
      const inners = statements.map(statement => {
        const carried = (
          statement as unknown as {
            __khalaCodeProductStateInner?: BoundStatement
          }
        ).__khalaCodeProductStateInner
        return (
          carried ?? {
            onBeforeWrite: undefined,
            onWriteSuccess: undefined,
            statement,
          }
        )
      })
      for (const inner of inners) {
        if (inner.onBeforeWrite !== undefined) {
          await inner.onBeforeWrite()
        }
      }
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
// Env plumbing
// ---------------------------------------------------------------------------

export type KhalaCodeProductStateStoreEnv = KhalaCodeProductStateFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeKhalaCodeProductStateStoreOptions = Readonly<{
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: KhalaCodeProductStateLog | undefined
  /** CFG-4 Domain 2 (#8519): identity-handle override (tests). */
  identityDb?: IdentityDb | undefined
}>

const defaultLog: KhalaCodeProductStateLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresMirrorForEnv = (
  env: KhalaCodeProductStateStoreEnv,
  options: MakeKhalaCodeProductStateStoreOptions,
): KhalaCodeProductStateMirror | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const log = options.log ?? defaultLog
  return makePostgresKhalaCodeProductStateMirror({
    acquireSql: () => makeSqlClient(connectionString),
    onProjectionSkip: (table, reasonSafe) => {
      log('khala_sync_khala_code_state_projection_skipped', {
        messageSafe: reasonSafe,
        op: `project:${table}`,
        refs: [table],
      })
    },
  })
}

export const khalaCodeProductStateDatabaseForEnv = (
  env: KhalaCodeProductStateStoreEnv,
  options: MakeKhalaCodeProductStateStoreOptions = {},
): D1Database => {
  const db = openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const flags = khalaCodeProductStateFlagsFromEnv(env)
  if (!flags.dualWrite) {
    return db
  }
  const mirror = postgresMirrorForEnv(env, options)
  if (mirror === undefined) {
    return db
  }
  return makeKhalaCodeProductStateMirroringDatabase({
    db,
    identityDb: options.identityDb ?? identityDbForEnv(env),
    log: options.log ?? defaultLog,
    mirror,
  })
}
