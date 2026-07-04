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

export type MakePylonAgentRunnerStatusMirrorDependencies = Readonly<{
  flags: Readonly<{ dualWrite: boolean }>
  log?: PylonDispatchLog | undefined
  postgres: PylonAgentRunnerStatusPostgresStore | undefined
}>

const safeRunnerStatusMirrorMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

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
            messageSafe: safeRunnerStatusMirrorMessage(error),
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
