import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import {
  pylonDispatchFlagsFromEnv,
  type PylonDispatchFlagEnv,
  type PylonDispatchLog,
} from './pylon-dispatch-store'

export type PylonAgentRunnerStatusEventRecord = Readonly<{
  eventRef: string
  ownerAgentUserId: string
  runnerRef: string
  runnerKind: string
  pylonRef: string | null
  assignmentRef: string | null
  state: string
  stateStartedAt: string
  updatedAt: string
  retentionState: string
  eventJson: string
  createdAt: string
  retainedAt: string | null
}>

export type PylonAgentRunnerStatusRow = Readonly<{
  event_ref: string
  owner_agent_user_id: string
  runner_ref: string
  runner_kind: string
  pylon_ref: string | null
  assignment_ref: string | null
  state: string
  state_started_at: string
  updated_at: string
  retention_state: 'live' | 'retained'
  event_json: string
}>

export type PylonAgentRunnerStatusReadScope =
  | Readonly<{ kind: 'admin' }>
  | Readonly<{ kind: 'agent'; userId: string }>

export type PylonAgentRunnerStatusReadInput = Readonly<{
  limit: number
  scope: PylonAgentRunnerStatusReadScope
}>

export type PylonAgentRunnerStatusReadResult = Readonly<{
  rows: ReadonlyArray<PylonAgentRunnerStatusRow>
  sourceRefs: ReadonlyArray<string>
}>

export type PylonAgentRunnerStatusReadStore = Readonly<{
  listStatusRows: (
    input: PylonAgentRunnerStatusReadInput,
  ) => Promise<PylonAgentRunnerStatusReadResult>
}>

export type PylonAgentRunnerStatusRetainInput = Readonly<{
  ownerAgentUserId: string
  runnerRef: string
  eventRef: string
  retainedAt: string
}>

export type PylonAgentRunnerStatusMirrorInput = Readonly<{
  retain: PylonAgentRunnerStatusRetainInput
  record: PylonAgentRunnerStatusEventRecord
}>

export type PylonAgentRunnerStatusMirror = Readonly<{
  recordStatusEvent: (
    input: PylonAgentRunnerStatusMirrorInput,
  ) => Promise<void>
}>

export type PylonAgentRunnerStatusPostgresStore = Readonly<{
  retainLiveRunnerEvents: (
    input: PylonAgentRunnerStatusRetainInput,
  ) => Promise<void>
  upsertStatusEvent: (
    record: PylonAgentRunnerStatusEventRecord,
  ) => Promise<void>
}>

export type MakePostgresPylonAgentRunnerStatusStoreDependencies = Readonly<{
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

const RUNNER_STATUS_D1_SOURCE_REF = 'd1:pylon_agent_runner_status_events'
const RUNNER_STATUS_POSTGRES_SOURCE_REF =
  'postgres:pylon_agent_runner_status_events'
const RUNNER_STATUS_POSTGRES_SHADOW_SOURCE_REF =
  'postgres-shadow:pylon_agent_runner_status_events'
const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

const boundedRunnerStatusLimit = (limit: number): number =>
  Number.isFinite(limit) ? Math.max(1, Math.min(Math.trunc(limit), 200)) : 200

const stableStringify = (value: unknown): string =>
  JSON.stringify(value, (_key, val: unknown) =>
    val !== null && typeof val === 'object' && !Array.isArray(val)
      ? Object.fromEntries(
          Object.entries(val as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : val,
  )

const safeRunnerStatusMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

export const makeD1PylonAgentRunnerStatusReadStore = (
  db: D1Database,
): PylonAgentRunnerStatusReadStore => ({
  listStatusRows: async input => {
    const limit = boundedRunnerStatusLimit(input.limit)
    const ownerClause =
      input.scope.kind === 'admin' ? '' : 'AND owner_agent_user_id = ?'
    const ownerBindings =
      input.scope.kind === 'admin' ? [] : [input.scope.userId]
    try {
      const result = await db
        .prepare(
          `SELECT event_ref, owner_agent_user_id, runner_ref, runner_kind,
                  pylon_ref, assignment_ref, state, state_started_at,
                  updated_at, retention_state, event_json
             FROM pylon_agent_runner_status_events
            WHERE archived_at IS NULL
              ${ownerClause}
            ORDER BY updated_at DESC
            LIMIT ?`,
        )
        .bind(...ownerBindings, limit)
        .all<PylonAgentRunnerStatusRow>()
      return {
        rows: result.results ?? [],
        sourceRefs: [RUNNER_STATUS_D1_SOURCE_REF],
      }
    } catch {
      return { rows: [], sourceRefs: [RUNNER_STATUS_D1_SOURCE_REF] }
    }
  },
})

export const makePostgresPylonAgentRunnerStatusReadStore = (
  deps: MakePostgresPylonAgentRunnerStatusStoreDependencies,
): PylonAgentRunnerStatusReadStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the sync push route.
      }
    }
  }

  return {
    listStatusRows: input =>
      withSql(async sql => {
        const limit = boundedRunnerStatusLimit(input.limit)
        const rows: Array<PylonAgentRunnerStatusRow> =
          input.scope.kind === 'admin'
            ? await sql`
                SELECT event_ref, owner_agent_user_id, runner_ref, runner_kind,
                       pylon_ref, assignment_ref, state, state_started_at,
                       updated_at, retention_state, event_json
                  FROM pylon_agent_runner_status_events
                 WHERE archived_at IS NULL
                 ORDER BY updated_at DESC
                 LIMIT ${limit}`
            : await sql`
                SELECT event_ref, owner_agent_user_id, runner_ref, runner_kind,
                       pylon_ref, assignment_ref, state, state_started_at,
                       updated_at, retention_state, event_json
                  FROM pylon_agent_runner_status_events
                 WHERE archived_at IS NULL
                   AND owner_agent_user_id = ${input.scope.userId}
                 ORDER BY updated_at DESC
                 LIMIT ${limit}`
        return {
          rows,
          sourceRefs: [RUNNER_STATUS_POSTGRES_SOURCE_REF],
        }
      }),
  }
}

export const makePostgresPylonAgentRunnerStatusStore = (
  deps: MakePostgresPylonAgentRunnerStatusStoreDependencies,
): PylonAgentRunnerStatusPostgresStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the sync push route.
      }
    }
  }

  return {
    retainLiveRunnerEvents: input =>
      withSql(async sql => {
        await sql`
          UPDATE pylon_agent_runner_status_events
             SET retention_state = 'retained',
                 retained_at = COALESCE(retained_at, ${input.retainedAt})
           WHERE owner_agent_user_id = ${input.ownerAgentUserId}
             AND runner_ref = ${input.runnerRef}
             AND event_ref <> ${input.eventRef}
             AND retention_state = 'live'
             AND archived_at IS NULL`
      }),

    upsertStatusEvent: record =>
      withSql(async sql => {
        await sql`
          INSERT INTO pylon_agent_runner_status_events
            (event_ref, owner_agent_user_id, runner_ref, runner_kind, pylon_ref,
             assignment_ref, state, state_started_at, updated_at,
             retention_state, event_json, created_at, retained_at, archived_at)
          VALUES
            (${record.eventRef}, ${record.ownerAgentUserId}, ${record.runnerRef},
             ${record.runnerKind}, ${record.pylonRef}, ${record.assignmentRef},
             ${record.state}, ${record.stateStartedAt}, ${record.updatedAt},
             ${record.retentionState}, ${record.eventJson}, ${record.createdAt},
             ${record.retainedAt}, NULL)
          ON CONFLICT (event_ref) DO UPDATE SET
            runner_kind = EXCLUDED.runner_kind,
            pylon_ref = EXCLUDED.pylon_ref,
            assignment_ref = EXCLUDED.assignment_ref,
            state = EXCLUDED.state,
            state_started_at = EXCLUDED.state_started_at,
            updated_at = EXCLUDED.updated_at,
            retention_state = EXCLUDED.retention_state,
            event_json = EXCLUDED.event_json,
            retained_at = EXCLUDED.retained_at
          WHERE pylon_agent_runner_status_events.owner_agent_user_id =
            ${record.ownerAgentUserId}`
      }),
  }
}

export type MakeReadRoutedPylonAgentRunnerStatusReadStoreDependencies =
  Readonly<{
    d1: PylonAgentRunnerStatusReadStore
    flags: Readonly<{ reads: 'd1' | 'postgres' | 'compare' }>
    log?: PylonDispatchLog | undefined
    postgres: PylonAgentRunnerStatusReadStore | undefined
    wait?: ((ms: number) => Promise<void>) | undefined
  }>

export const makeReadRoutedPylonAgentRunnerStatusReadStore = (
  deps: MakeReadRoutedPylonAgentRunnerStatusReadStoreDependencies,
): PylonAgentRunnerStatusReadStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})
  const wait =
    deps.wait ?? ((ms: number) => new Promise(resolve => setTimeout(resolve, ms)))

  if (postgres === undefined || flags.reads === 'd1') {
    return d1
  }

  return {
    listStatusRows: async input => {
      if (flags.reads === 'postgres') {
        for (let attempt = 0; ; attempt++) {
          try {
            return await postgres.listStatusRows(input)
          } catch (error) {
            const delay = READ_RETRY_DELAYS_MS[attempt]
            if (delay === undefined) {
              log('khala_sync_pylon_postgres_read_fallback', {
                messageSafe: safeRunnerStatusMessage(error),
                op: 'listAgentRunnerStatusRows',
                refs:
                  input.scope.kind === 'admin'
                    ? []
                    : [`agent:${input.scope.userId}`],
              })
              return d1.listStatusRows(input)
            }
            log('khala_sync_pylon_postgres_read_failed', {
              messageSafe: safeRunnerStatusMessage(error),
              op: 'listAgentRunnerStatusRows',
              refs:
                input.scope.kind === 'admin'
                  ? []
                  : [`agent:${input.scope.userId}`],
            })
            await wait(delay)
          }
        }
      }

      const d1Result = await d1.listStatusRows(input)
      try {
        const postgresResult = await postgres.listStatusRows(input)
        if (stableStringify(d1Result.rows) !== stableStringify(postgresResult.rows)) {
          log('khala_sync_pylon_read_compare_mismatch', {
            messageSafe: 'postgres read differs from d1 authority',
            op: 'listAgentRunnerStatusRows',
            refs:
              input.scope.kind === 'admin'
                ? []
                : [`agent:${input.scope.userId}`],
          })
        }
        return {
          rows: d1Result.rows,
          sourceRefs: [
            ...d1Result.sourceRefs,
            RUNNER_STATUS_POSTGRES_SHADOW_SOURCE_REF,
          ],
        }
      } catch (error) {
        log('khala_sync_pylon_postgres_read_failed', {
          messageSafe: safeRunnerStatusMessage(error),
          op: 'listAgentRunnerStatusRows',
          refs:
            input.scope.kind === 'admin'
              ? []
              : [`agent:${input.scope.userId}`],
        })
        return d1Result
      }
    },
  }
}

export type MakePylonAgentRunnerStatusMirrorDependencies = Readonly<{
  flags: Readonly<{ dualWrite: boolean }>
  log?: PylonDispatchLog | undefined
  postgres: PylonAgentRunnerStatusPostgresStore | undefined
}>

export const makePylonAgentRunnerStatusMirror = (
  deps: MakePylonAgentRunnerStatusMirrorDependencies,
): PylonAgentRunnerStatusMirror => {
  const { flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined || !flags.dualWrite) {
    return { recordStatusEvent: () => Promise.resolve() }
  }

  return {
    recordStatusEvent: input =>
      postgres
        .retainLiveRunnerEvents(input.retain)
        .then(() => postgres.upsertStatusEvent(input.record))
        .catch((error: unknown) => {
          log('khala_sync_pylon_dual_write_failed', {
            messageSafe: safeRunnerStatusMessage(error),
            op: 'recordAgentRunnerStatusEvent',
            refs: [input.record.eventRef, input.record.runnerRef],
          })
        }),
  }
}

export type PylonAgentRunnerStatusMirrorEnv = PylonDispatchFlagEnv &
  Readonly<{
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakePylonAgentRunnerStatusMirrorForEnvOptions = Readonly<{
  log?: PylonDispatchLog | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
}>

export type MakePylonAgentRunnerStatusReadStoreForEnvOptions =
  MakePylonAgentRunnerStatusMirrorForEnvOptions &
    Readonly<{
      wait?: ((ms: number) => Promise<void>) | undefined
    }>

const defaultRunnerStatusMirrorLog: PylonDispatchLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

export const makePylonAgentRunnerStatusMirrorForEnv = (
  env: unknown,
  options: MakePylonAgentRunnerStatusMirrorForEnvOptions = {},
): PylonAgentRunnerStatusMirror => {
  const typedEnv = (env ?? {}) as PylonAgentRunnerStatusMirrorEnv
  const connectionString = typedEnv.KHALA_SYNC_DB?.connectionString
  const flags = pylonDispatchFlagsFromEnv(typedEnv)

  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    !flags.dualWrite
  ) {
    return makePylonAgentRunnerStatusMirror({
      flags,
      postgres: undefined,
    })
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresPylonAgentRunnerStatusStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makePylonAgentRunnerStatusMirror({
    flags,
    log: options.log ?? defaultRunnerStatusMirrorLog,
    postgres,
  })
}

export const makePylonAgentRunnerStatusReadStoreForEnv = (
  env: PylonAgentRunnerStatusMirrorEnv,
  db: D1Database,
  options: MakePylonAgentRunnerStatusReadStoreForEnvOptions = {},
): PylonAgentRunnerStatusReadStore => {
  const d1 = makeD1PylonAgentRunnerStatusReadStore(db)
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  const flags = pylonDispatchFlagsFromEnv(env)

  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    flags.reads === 'd1'
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresPylonAgentRunnerStatusReadStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makeReadRoutedPylonAgentRunnerStatusReadStore({
    d1,
    flags,
    log: options.log ?? defaultRunnerStatusMirrorLog,
    postgres,
    wait: options.wait,
  })
}
