import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { Schema as S } from 'effect'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import {
  pylonCodexRawEventChunkR2Key,
  pylonCodexRawEventChunkRef,
  type PylonCodexRawEventChunkStore,
  type PylonCodexRawEventChunkStoreInput,
  pylonCodexRawEventR2Key,
  pylonCodexRawEventRef,
  type PylonCodexRawEventStore,
  type PylonCodexRawEventStoreInput,
} from './pylon-codex-turn-ingest-routes'
import {
  pylonDispatchFlagsFromEnv,
  type PylonDispatchFlagEnv,
  type PylonDispatchLog,
} from './pylon-dispatch-store'
import { currentIsoTimestamp } from './runtime-primitives'

export const PYLON_CODEX_RAW_EVENT_METADATA_QUEUE_SCHEMA_VERSION =
  'openagents.pylon.codex_raw_event_metadata_queue.v1' as const

const PYLON_CODEX_RAW_EVENT_DEMAND_KIND = 'own_capacity' as const
const PYLON_CODEX_RAW_EVENT_DEMAND_SOURCE =
  'khala_coding_delegation' as const

export class PylonCodexRawEventMetadataQueueMessage extends S.Class<PylonCodexRawEventMetadataQueueMessage>(
  'PylonCodexRawEventMetadataQueueMessage',
)({
  schemaVersion: S.Literal(
    PYLON_CODEX_RAW_EVENT_METADATA_QUEUE_SCHEMA_VERSION,
  ),
  kind: S.Literals(['turn_events', 'event_chunk']),
  ref: S.String,
  assignmentRef: S.String,
  byteLength: S.Number,
  chunkIndex: S.NullOr(S.Number),
  contentDigest: S.String,
  createdAt: S.String,
  demandKind: S.Literal(PYLON_CODEX_RAW_EVENT_DEMAND_KIND),
  demandSource: S.Literal(PYLON_CODEX_RAW_EVENT_DEMAND_SOURCE),
  eventCount: S.Number,
  leaseRef: S.String,
  observedAt: S.String,
  ownerUserId: S.String,
  pylonRef: S.String,
  r2Key: S.String,
  runRef: S.NullOr(S.String),
  sessionRef: S.NullOr(S.String),
  turnIndex: S.Number,
  updatedAt: S.String,
  workspaceRef: S.NullOr(S.String),
}) {}

export type PylonCodexRawEventMetadataQueueProducer = Readonly<{
  send: (
    message: PylonCodexRawEventMetadataQueueMessage,
  ) => Promise<unknown>
}>

export type PylonCodexRawEventMetadataWriter = Readonly<{
  writeMetadata: (
    message: PylonCodexRawEventMetadataQueueMessage,
  ) => Promise<void>
}>

const rawEventByteLength = (eventsJson: string): number =>
  new TextEncoder().encode(eventsJson).byteLength

const rawEventQueueMessageBase = (
  input:
    | PylonCodexRawEventChunkStoreInput
    | PylonCodexRawEventStoreInput,
  fields: Readonly<{
    byteLength: number
    chunkIndex: number | null
    kind: 'event_chunk' | 'turn_events'
    r2Key: string
    ref: string
  }>,
): Omit<
  typeof PylonCodexRawEventMetadataQueueMessage.Type,
  'schemaVersion'
> => {
  const now = currentIsoTimestamp()
  return {
    assignmentRef: input.assignmentRef,
    byteLength: fields.byteLength,
    chunkIndex: fields.chunkIndex,
    contentDigest: input.digest,
    createdAt: now,
    demandKind: PYLON_CODEX_RAW_EVENT_DEMAND_KIND,
    demandSource: PYLON_CODEX_RAW_EVENT_DEMAND_SOURCE,
    eventCount: input.eventCount,
    kind: fields.kind,
    leaseRef: input.leaseRef,
    observedAt: input.observedAt,
    ownerUserId: input.ownerUserId,
    pylonRef: input.pylonRef,
    r2Key: fields.r2Key,
    ref: fields.ref,
    runRef: input.runRef,
    sessionRef: input.sessionRef,
    turnIndex: input.turnIndex,
    updatedAt: now,
    workspaceRef: input.workspaceRef,
  }
}

export const makeQueuedR2PylonCodexRawEventStore = (
  bucket: R2Bucket,
  queue: PylonCodexRawEventMetadataQueueProducer,
): PylonCodexRawEventStore => ({
  putTurnEvents: async input => {
    const ref = pylonCodexRawEventRef(input.digest)
    const r2Key = pylonCodexRawEventR2Key(input)
    const existingObject = await bucket.head(r2Key)
    const byteLength = rawEventByteLength(input.eventsJson)
    if (existingObject === null) {
      await bucket.put(r2Key, input.eventsJson, {
        customMetadata: {
          assignmentRef: input.assignmentRef,
          demandKind: PYLON_CODEX_RAW_EVENT_DEMAND_KIND,
          demandSource: PYLON_CODEX_RAW_EVENT_DEMAND_SOURCE,
          ownerUserId: input.ownerUserId,
          rawEventRef: ref,
          turnIndex: String(input.turnIndex),
        },
        httpMetadata: { contentType: 'application/json; charset=utf-8' },
      })
    }

    await queue.send(
      new PylonCodexRawEventMetadataQueueMessage({
        schemaVersion: PYLON_CODEX_RAW_EVENT_METADATA_QUEUE_SCHEMA_VERSION,
        ...rawEventQueueMessageBase(input, {
          byteLength,
          chunkIndex: null,
          kind: 'turn_events',
          r2Key,
          ref,
        }),
      }),
    )

    return {
      byteLength,
      created: existingObject === null,
      ref,
      r2Key,
    }
  },
})

export const makeQueuedR2PylonCodexRawEventChunkStore = (
  bucket: R2Bucket,
  queue: PylonCodexRawEventMetadataQueueProducer,
): PylonCodexRawEventChunkStore => ({
  putEventChunk: async input => {
    const ref = pylonCodexRawEventChunkRef(input.digest)
    const r2Key = pylonCodexRawEventChunkR2Key(input)
    const existingObject = await bucket.head(r2Key)
    const byteLength = rawEventByteLength(input.eventsJson)
    if (existingObject === null) {
      await bucket.put(r2Key, input.eventsJson, {
        customMetadata: {
          assignmentRef: input.assignmentRef,
          chunkIndex: String(input.chunkIndex),
          demandKind: PYLON_CODEX_RAW_EVENT_DEMAND_KIND,
          demandSource: PYLON_CODEX_RAW_EVENT_DEMAND_SOURCE,
          ownerUserId: input.ownerUserId,
          rawEventChunkRef: ref,
          turnIndex: String(input.turnIndex),
        },
        httpMetadata: { contentType: 'application/json; charset=utf-8' },
      })
    }

    await queue.send(
      new PylonCodexRawEventMetadataQueueMessage({
        schemaVersion: PYLON_CODEX_RAW_EVENT_METADATA_QUEUE_SCHEMA_VERSION,
        ...rawEventQueueMessageBase(input, {
          byteLength,
          chunkIndex: input.chunkIndex,
          kind: 'event_chunk',
          r2Key,
          ref,
        }),
      }),
    )

    return {
      byteLength,
      created: existingObject === null,
      ref,
      r2Key,
    }
  },
})

export const makeD1PylonCodexRawEventMetadataWriter = (
  db: D1Database,
): PylonCodexRawEventMetadataWriter => ({
  writeMetadata: async message => {
    if (message.kind === 'turn_events') {
      await db
        .prepare(
          `
            INSERT OR IGNORE INTO pylon_codex_raw_events (
              raw_event_ref,
              assignment_ref,
              lease_ref,
              pylon_ref,
              owner_user_id,
              run_ref,
              session_ref,
              workspace_ref,
              turn_index,
              event_count,
              byte_length,
              content_digest,
              r2_key,
              observed_at,
              created_at,
              updated_at,
              demand_kind,
              demand_source
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          `,
        )
        .bind(
          message.ref,
          message.assignmentRef,
          message.leaseRef,
          message.pylonRef,
          message.ownerUserId,
          message.runRef,
          message.sessionRef,
          message.workspaceRef,
          message.turnIndex,
          message.eventCount,
          message.byteLength,
          message.contentDigest,
          message.r2Key,
          message.observedAt,
          message.createdAt,
          message.updatedAt,
          message.demandKind,
          message.demandSource,
        )
        .run()
      return
    }

    await db
      .prepare(
        `
          INSERT OR IGNORE INTO pylon_codex_raw_event_chunks (
            chunk_ref,
            assignment_ref,
            lease_ref,
            pylon_ref,
            owner_user_id,
            run_ref,
            session_ref,
            workspace_ref,
            turn_index,
            chunk_index,
            event_count,
            byte_length,
            content_digest,
            r2_key,
            observed_at,
            created_at,
            updated_at,
            demand_kind,
            demand_source
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `,
      )
      .bind(
        message.ref,
        message.assignmentRef,
        message.leaseRef,
        message.pylonRef,
        message.ownerUserId,
        message.runRef,
        message.sessionRef,
        message.workspaceRef,
        message.turnIndex,
        message.chunkIndex,
        message.eventCount,
        message.byteLength,
        message.contentDigest,
        message.r2Key,
        message.observedAt,
        message.createdAt,
        message.updatedAt,
        message.demandKind,
        message.demandSource,
      )
      .run()
  },
})

export type MakePostgresPylonCodexRawEventMetadataWriterDependencies =
  Readonly<{
    acquireSql: () => Promise<KhalaSyncPushSqlClient>
  }>

export const makePostgresPylonCodexRawEventMetadataWriter = (
  deps: MakePostgresPylonCodexRawEventMetadataWriterDependencies,
): PylonCodexRawEventMetadataWriter => {
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
    writeMetadata: message =>
      withSql(async sql => {
        if (message.kind === 'turn_events') {
          await sql`
            INSERT INTO pylon_codex_raw_events
              (raw_event_ref, assignment_ref, lease_ref, pylon_ref,
               owner_user_id, run_ref, session_ref, workspace_ref, turn_index,
               event_count, byte_length, content_digest, r2_key, observed_at,
               created_at, updated_at, demand_kind, demand_source)
            VALUES
              (${message.ref}, ${message.assignmentRef}, ${message.leaseRef},
               ${message.pylonRef}, ${message.ownerUserId}, ${message.runRef},
               ${message.sessionRef}, ${message.workspaceRef},
               ${message.turnIndex}, ${message.eventCount},
               ${message.byteLength}, ${message.contentDigest},
               ${message.r2Key}, ${message.observedAt}, ${message.createdAt},
               ${message.updatedAt}, ${message.demandKind},
               ${message.demandSource})
            ON CONFLICT DO NOTHING`
          return
        }

        await sql`
          INSERT INTO pylon_codex_raw_event_chunks
            (chunk_ref, assignment_ref, lease_ref, pylon_ref, owner_user_id,
             run_ref, session_ref, workspace_ref, turn_index, chunk_index,
             event_count, byte_length, content_digest, r2_key, observed_at,
             created_at, updated_at, demand_kind, demand_source)
          VALUES
            (${message.ref}, ${message.assignmentRef}, ${message.leaseRef},
             ${message.pylonRef}, ${message.ownerUserId}, ${message.runRef},
             ${message.sessionRef}, ${message.workspaceRef},
             ${message.turnIndex}, ${message.chunkIndex}, ${message.eventCount},
             ${message.byteLength}, ${message.contentDigest}, ${message.r2Key},
             ${message.observedAt}, ${message.createdAt}, ${message.updatedAt},
             ${message.demandKind}, ${message.demandSource})
          ON CONFLICT DO NOTHING`
      }),
  }
}

export type MakePylonCodexRawEventMetadataQueueConsumerDependencies =
  Readonly<{
    d1: PylonCodexRawEventMetadataWriter
    log?: PylonDispatchLog | undefined
    postgres?: PylonCodexRawEventMetadataWriter | undefined
  }>

const safeRawEventQueueMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

export const makePylonCodexRawEventMetadataQueueConsumer = (
  deps: MakePylonCodexRawEventMetadataQueueConsumerDependencies,
): PylonCodexRawEventMetadataWriter => {
  const log = deps.log ?? (() => {})
  return {
    writeMetadata: async message => {
      await deps.d1.writeMetadata(message)
      await deps.postgres?.writeMetadata(message).catch((error: unknown) => {
        log('khala_sync_pylon_dual_write_failed', {
          messageSafe: safeRawEventQueueMessage(error),
          op: 'recordPylonCodexRawEventMetadata',
          refs: [message.ref, message.assignmentRef],
        })
      })
    },
  }
}

export type PylonCodexRawEventMetadataQueueEnv = PylonDispatchFlagEnv &
  Readonly<{
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakePylonCodexRawEventMetadataQueueConsumerForEnvOptions =
  Readonly<{
    log?: PylonDispatchLog | undefined
    makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  }>

const defaultRawEventMetadataQueueLog: PylonDispatchLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

export const makePylonCodexRawEventMetadataQueueConsumerForEnv = (
  env: unknown,
  db: D1Database,
  options: MakePylonCodexRawEventMetadataQueueConsumerForEnvOptions = {},
): PylonCodexRawEventMetadataWriter => {
  const typedEnv = (env ?? {}) as PylonCodexRawEventMetadataQueueEnv
  const connectionString = typedEnv.KHALA_SYNC_DB?.connectionString
  const flags = pylonDispatchFlagsFromEnv(typedEnv)
  const d1 = makeD1PylonCodexRawEventMetadataWriter(db)
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres =
    connectionString === undefined ||
    connectionString.length === 0 ||
    !flags.dualWrite
      ? undefined
      : makePostgresPylonCodexRawEventMetadataWriter({
          acquireSql: () => makeSqlClient(connectionString),
        })

  return makePylonCodexRawEventMetadataQueueConsumer({
    d1,
    log: options.log ?? defaultRawEventMetadataQueueLog,
    postgres,
  })
}
