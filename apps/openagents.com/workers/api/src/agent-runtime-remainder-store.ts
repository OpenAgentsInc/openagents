// KS-8.5 follow-up (#8334): runtime mirror seam for the agent-runtime
// remainder tables. This slice wires the event ledger first because its
// per-owner ordering_sequence density is the load-bearing invariant.

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'

export type AgentRuntimeRemainderTable = 'event_ledger_entries'

export type AgentRuntimeRemainderRow = Readonly<Record<string, unknown>>

export type AgentRuntimeRemainderWriteStore = Readonly<{
  upsertRows: (
    table: AgentRuntimeRemainderTable,
    rows: ReadonlyArray<AgentRuntimeRemainderRow>,
  ) => Promise<number>
}>

const EVENT_LEDGER_COLUMNS = [
  'entry_id',
  'owner_agent_user_id',
  'owner_ref',
  'source',
  'external_ref',
  'actor_ref',
  'content_ref',
  'subject_ref',
  'event_type',
  'source_refs_json',
  'payload_summary_json',
  'occurred_at',
  'received_at',
  'ordering_key',
  'ordering_sequence',
  'handled_state',
  'handled_by_run_id',
  'handled_by_definition_id',
  'handled_at',
  'handled_reason_ref',
  'training_consent',
  'created_at',
  'updated_at',
] as const

const TABLE_COLUMNS: Readonly<
  Record<AgentRuntimeRemainderTable, ReadonlyArray<string>>
> = {
  event_ledger_entries: EVENT_LEDGER_COLUMNS,
}

const TABLE_PK: Readonly<Record<AgentRuntimeRemainderTable, string>> = {
  event_ledger_entries: 'entry_id',
}

const TABLE_CONFLICT: Readonly<
  Record<AgentRuntimeRemainderTable, ReadonlyArray<string>>
> = {
  event_ledger_entries: ['entry_id'],
}

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) {
    return null
  }
  if (typeof value === 'number') {
    return value
  }
  if (typeof value === 'bigint') {
    return Number(value)
  }
  if (typeof value === 'boolean') {
    return value ? 1 : 0
  }
  return String(value)
}

type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== 'function') {
    return async () => {
      throw new TypeError(
        'agent runtime remainder mirror requires a SQL client with unsafe(text, params)',
      )
    }
  }
  return unsafe
}

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

export type AgentRuntimeRemainderDiagnosticEvent =
  'khala_sync_agent_runtime_remainder_dual_write_failed'

export type AgentRuntimeRemainderLog = (
  event: AgentRuntimeRemainderDiagnosticEvent,
  fields: Readonly<{
    op: string
    refs: ReadonlyArray<string>
    messageSafe: string
  }>,
) => void

const defaultLog: AgentRuntimeRemainderLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

export type AgentRuntimeRemainderFlags = Readonly<{ dualWrite: boolean }>

export type AgentRuntimeRemainderFlagEnv = Readonly<{
  KHALA_SYNC_AGENT_RUNTIME_REMAINDER_DUAL_WRITE?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

export const agentRuntimeRemainderFlagsFromEnv = (
  env: AgentRuntimeRemainderFlagEnv,
): AgentRuntimeRemainderFlags => {
  const raw =
    env.KHALA_SYNC_AGENT_RUNTIME_REMAINDER_DUAL_WRITE?.trim().toLowerCase()
  return { dualWrite: raw === undefined || !FLAG_OFF_VALUES.has(raw) }
}

export type MakePostgresAgentRuntimeRemainderStoreDependencies = Readonly<{
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresAgentRuntimeRemainderStore = (
  deps: MakePostgresAgentRuntimeRemainderStoreDependencies,
): AgentRuntimeRemainderWriteStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort client teardown, same as the KS-8.5 core store.
      }
    }
  }

  return {
    upsertRows: async (table, rows) => {
      if (rows.length === 0) {
        return 0
      }
      const columns = TABLE_COLUMNS[table]
      const keyColumns = TABLE_CONFLICT[table]
      const setClauses = columns
        .filter(column => !keyColumns.includes(column))
        .map(column => `${column} = EXCLUDED.${column}`)
        .join(', ')
      return withSql(async sql => {
        const unsafe = requireUnsafe(sql)
        const insertResults = await Promise.all(
          rows.map(row => {
            const values = columns.map(column => normalizeValue(row[column]))
            const placeholders = values.map((_, index) => `$${index + 1}`).join(', ')
            return unsafe(
              `INSERT INTO ${table} (${columns.join(', ')}) VALUES (${placeholders}) ON CONFLICT (${keyColumns.join(', ')}) DO UPDATE SET ${setClauses} RETURNING 1 AS touched`,
              values as Array<unknown>,
            )
          }),
        )
        return insertResults.reduce((total, result) => total + result.length, 0)
      })
    },
  }
}

export type AgentRuntimeRemainderMirror = Readonly<{
  mirrorRowsByPk: (
    table: AgentRuntimeRemainderTable,
    pkValues: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeAgentRuntimeRemainderMirrorDependencies = Readonly<{
  db: D1Database
  postgres: AgentRuntimeRemainderWriteStore
  log?: AgentRuntimeRemainderLog | undefined
}>

export const makeAgentRuntimeRemainderMirror = (
  deps: MakeAgentRuntimeRemainderMirrorDependencies,
): AgentRuntimeRemainderMirror => {
  const log = deps.log ?? defaultLog

  const guarded = async (
    op: string,
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> => {
    try {
      await run()
    } catch (error) {
      log('khala_sync_agent_runtime_remainder_dual_write_failed', {
        messageSafe: safeMessage(error),
        op,
        refs: refs.slice(0, 10),
      })
    }
  }

  return {
    mirrorRowsByPk: (table, pkValues) =>
      guarded(`mirror:${table}`, pkValues, async () => {
        if (pkValues.length === 0) {
          return
        }
        const pk = TABLE_PK[table]
        const placeholders = pkValues.map(() => '?').join(', ')
        const rows = await deps.db
          .prepare(`SELECT * FROM ${table} WHERE ${pk} IN (${placeholders})`)
          .bind(...pkValues)
          .all<AgentRuntimeRemainderRow>()
        await deps.postgres.upsertRows(table, rows.results ?? [])
      }),
  }
}

export type AgentRuntimeRemainderStoreEnv = AgentRuntimeRemainderFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeAgentRuntimeRemainderStoreOptions = Readonly<{
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: AgentRuntimeRemainderLog | undefined
}>

export const makeAgentRuntimeRemainderMirrorForEnv = (
  env: AgentRuntimeRemainderStoreEnv,
  options: MakeAgentRuntimeRemainderStoreOptions = {},
): AgentRuntimeRemainderMirror | undefined => {
  const flags = agentRuntimeRemainderFlagsFromEnv(env)
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (
    !flags.dualWrite ||
    connectionString === undefined ||
    connectionString.length === 0
  ) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makeAgentRuntimeRemainderMirror({
    db: openAgentsDatabase(env),
    log: options.log,
    postgres: makePostgresAgentRuntimeRemainderStore({
      acquireSql: () => makeSqlClient(connectionString),
    }),
  })
}
