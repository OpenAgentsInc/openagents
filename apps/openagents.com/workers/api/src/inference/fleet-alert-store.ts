import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from '../khala-sync-push-routes'
import { logWorkerRouteWarning } from '../observability'
import {
  pylonDispatchFlagsFromEnv,
  type PylonDispatchFlagEnv,
  type PylonDispatchLog,
} from '../pylon-dispatch-store'

export type FleetAlertWriteRecord = Readonly<{
  id: string
  alertRef: string
  detectedAt: string
  classification: string
  reasonRef: string
  burnTokensWindow: number
  windowMinutes: number
  stallThresholdTokens: number
  activeAssignments?: number | undefined
  queuedAssignments?: number | undefined
  recoveryActions: ReadonlyArray<string>
  recoveredLeaseCount?: number | undefined
  createdAt: string
}>

export type FleetAlertWriteStore = Readonly<{
  insertAlert: (record: FleetAlertWriteRecord) => Promise<void>
}>

const nonNegativeInt = (value: number | undefined): number =>
  Math.max(0, Math.trunc(value ?? 0))

const fleetAlertValues = (record: FleetAlertWriteRecord) => ({
  activeAssignments: nonNegativeInt(record.activeAssignments),
  burnTokensWindow: nonNegativeInt(record.burnTokensWindow),
  queuedAssignments: nonNegativeInt(record.queuedAssignments),
  recoveredLeaseCount: nonNegativeInt(record.recoveredLeaseCount),
  recoveryActionsJson: JSON.stringify(record.recoveryActions),
  stallThresholdTokens: nonNegativeInt(record.stallThresholdTokens),
  windowMinutes: nonNegativeInt(record.windowMinutes),
})

export const makeD1FleetAlertWriteStore = (
  db: D1Database,
): FleetAlertWriteStore => ({
  insertAlert: async record => {
    const values = fleetAlertValues(record)
    await db
      .prepare(
        `INSERT INTO fleet_alerts
          (id, alert_ref, detected_at, classification, reason_ref,
           burn_tokens_window, window_minutes, stall_threshold_tokens,
           active_assignments, queued_assignments, recovery_actions_json,
           recovered_lease_count, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.id,
        record.alertRef,
        record.detectedAt,
        record.classification,
        record.reasonRef,
        values.burnTokensWindow,
        values.windowMinutes,
        values.stallThresholdTokens,
        values.activeAssignments,
        values.queuedAssignments,
        values.recoveryActionsJson,
        values.recoveredLeaseCount,
        record.createdAt,
      )
      .run()
  },
})

export type MakePostgresFleetAlertWriteStoreDependencies = Readonly<{
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresFleetAlertWriteStore = (
  deps: MakePostgresFleetAlertWriteStoreDependencies,
): FleetAlertWriteStore => {
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
    insertAlert: record =>
      withSql(async sql => {
        const values = fleetAlertValues(record)
        await sql`
          INSERT INTO fleet_alerts
            (id, alert_ref, detected_at, classification, reason_ref,
             burn_tokens_window, window_minutes, stall_threshold_tokens,
             active_assignments, queued_assignments, recovery_actions_json,
             recovered_lease_count, created_at)
          VALUES
            (${record.id}, ${record.alertRef}, ${record.detectedAt},
             ${record.classification}, ${record.reasonRef},
             ${values.burnTokensWindow}, ${values.windowMinutes},
             ${values.stallThresholdTokens}, ${values.activeAssignments},
             ${values.queuedAssignments}, ${values.recoveryActionsJson},
             ${values.recoveredLeaseCount}, ${record.createdAt})
          ON CONFLICT (alert_ref) DO NOTHING`
      }),
  }
}

export type MakeDualWriteFleetAlertWriteStoreDependencies = Readonly<{
  d1: FleetAlertWriteStore
  flags: Readonly<{ dualWrite: boolean }>
  log?: PylonDispatchLog | undefined
  postgres: FleetAlertWriteStore | undefined
}>

const safeFleetAlertMirrorMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

export const makeDualWriteFleetAlertWriteStore = (
  deps: MakeDualWriteFleetAlertWriteStoreDependencies,
): FleetAlertWriteStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined) {
    return d1
  }

  return {
    insertAlert: async record => {
      await d1.insertAlert(record)
      if (!flags.dualWrite) {
        return
      }
      await postgres.insertAlert(record).catch((error: unknown) => {
        log('khala_sync_pylon_dual_write_failed', {
          messageSafe: safeFleetAlertMirrorMessage(error),
          op: 'insertFleetAlert',
          refs: [record.alertRef, record.classification],
        })
      })
    },
  }
}

export type FleetAlertWriteStoreEnv = PylonDispatchFlagEnv &
  Readonly<{
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeFleetAlertWriteStoreForEnvOptions = Readonly<{
  log?: PylonDispatchLog | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
}>

const defaultFleetAlertMirrorLog: PylonDispatchLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

export const makeFleetAlertWriteStoreForEnv = (
  db: D1Database,
  env: unknown,
  options: MakeFleetAlertWriteStoreForEnvOptions = {},
): FleetAlertWriteStore => {
  const d1 = makeD1FleetAlertWriteStore(db)
  const typedEnv = (env ?? {}) as FleetAlertWriteStoreEnv
  const connectionString = typedEnv.KHALA_SYNC_DB?.connectionString
  const flags = pylonDispatchFlagsFromEnv(typedEnv)

  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    !flags.dualWrite
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresFleetAlertWriteStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makeDualWriteFleetAlertWriteStore({
    d1,
    flags,
    log: options.log ?? defaultFleetAlertMirrorLog,
    postgres,
  })
}
