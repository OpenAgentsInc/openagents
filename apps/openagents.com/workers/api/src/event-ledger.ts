import type { AgentDefinition } from '@openagentsinc/agent-runtime-schema'
import type { AgentDefinitionWebhookNormalizedEvent } from '@openagentsinc/agent-runtime-schema/webhooks'
import { Schema as S } from 'effect'

import type { DueAgentDefinitionTriggerRecord } from './agent-definition-trigger-store'
import {
  makeAgentRuntimeRemainderMirrorForEnv,
  type AgentRuntimeRemainderMirror,
  type AgentRuntimeRemainderStoreEnv,
} from './agent-runtime-remainder-store'
import {
  parseJsonRecord as parseBoundaryJsonRecord,
  parseJsonStringArray,
} from './json-boundary'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'

export const EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION =
  'openagents.event_ledger_ingest.v1' as const
export const EVENT_LEDGER_INGEST_OUTCOME_SCHEMA =
  'openagents.event_ledger_ingest_outcome.v1' as const
export const EVENT_LEDGER_GATEWAY_READ_SCHEMA =
  'openagents.event_ledger_gateway_read.v1' as const

export const EventLedgerHandledState = S.Literals([
  'open',
  'handled',
  'responded',
  'ignored',
])
export type EventLedgerHandledState = typeof EventLedgerHandledState.Type

const EventLedgerPayloadSummary = S.Record(S.String, S.Unknown)
export const EventLedgerSource = S.Literals(['github', 'slack'])
export type EventLedgerSource = typeof EventLedgerSource.Type

export class EventLedgerIngestQueueMessage extends S.Class<EventLedgerIngestQueueMessage>(
  'EventLedgerIngestQueueMessage',
)({
  schemaVersion: S.Literal(EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION),
  actorRef: S.String,
  contentRef: S.String,
  eventType: S.String,
  externalRef: S.String,
  occurredAt: S.String,
  ownerAgentUserId: S.String,
  ownerRef: S.String,
  payloadSummary: EventLedgerPayloadSummary,
  receivedAt: S.String,
  source: EventLedgerSource,
  sourceRefs: S.Array(S.String),
  subjectRef: S.String,
  trainingConsent: S.Literal(false),
}) {}

export type EventLedgerIngestOutcome = Readonly<{
  duplicate: boolean
  entryId: string
  orderingSequence: number
  ownerAgentUserId: string
  persisted: boolean
  schema: typeof EVENT_LEDGER_INGEST_OUTCOME_SCHEMA
  source: EventLedgerIngestQueueMessage['source']
}>

type EventLedgerEntryRow = Readonly<{
  actor_ref: string
  content_ref: string
  created_at: string
  entry_id: string
  event_type: string
  external_ref: string
  handled_at: string | null
  handled_by_definition_id: string | null
  handled_by_run_id: string | null
  handled_reason_ref: string | null
  handled_state: EventLedgerHandledState
  occurred_at: string
  ordering_key: string
  ordering_sequence: number
  owner_agent_user_id: string
  owner_ref: string
  payload_summary_json: string
  received_at: string
  source: EventLedgerSource
  source_refs_json: string
  subject_ref: string
  training_consent: number
  updated_at: string
}>

export type EventLedgerEntry = Readonly<{
  actorRef: string
  contentRef: string
  createdAt: string
  entryId: string
  eventType: string
  externalRef: string
  handledAt: string | null
  handledByDefinitionId: string | null
  handledByRunId: string | null
  handledReasonRef: string | null
  handledState: EventLedgerHandledState
  occurredAt: string
  orderingKey: string
  orderingSequence: number
  ownerAgentUserId: string
  ownerRef: string
  payloadSummary: Record<string, unknown>
  receivedAt: string
  source: EventLedgerSource
  sourceRefs: ReadonlyArray<string>
  subjectRef: string
  trainingConsent: false
  updatedAt: string
}>

export type EventLedgerEntryListInput = Readonly<{
  handledStates?: ReadonlyArray<EventLedgerHandledState> | undefined
  limit: number
  ownerAgentUserId: string
  subjectRef?: string | undefined
}>

export type EventLedgerStore = Readonly<{
  insertEntry: (
    input: Readonly<{
      entryId: string
      message: EventLedgerIngestQueueMessage
      nowIso: string
      orderingSequence: number
    }>,
  ) => Promise<EventLedgerEntry>
  listOwnerEntries: (
    input: EventLedgerEntryListInput,
  ) => Promise<ReadonlyArray<EventLedgerEntry>>
  readOwnerEntry: (
    ownerAgentUserId: string,
    entryId: string,
  ) => Promise<EventLedgerEntry | undefined>
  updateHandledState: (
    input: Readonly<{
      entryId: string
      handledAt: string
      handledByDefinitionId: string
      handledByRunId: string
      handledReasonRef?: string | undefined
      handledState: EventLedgerHandledState
      ownerAgentUserId: string
    }>,
  ) => Promise<EventLedgerEntry | undefined>
}>

export type EventLedgerOwnerSequenceReservation = Readonly<{
  duplicate: boolean
  orderingKey: string
  orderingSequence: number
  persisted: boolean
}>

export type EventLedgerOwnerSequenceStore = Readonly<{
  markPersisted: (
    orderingKey: string,
    persistedAt: string,
  ) => Promise<void>
  reserve: (
    message: EventLedgerIngestQueueMessage,
  ) => Promise<EventLedgerOwnerSequenceReservation>
}>

const compactRefSegment = (value: string): string =>
  value.trim().replaceAll(/[^A-Za-z0-9_.:/=-]+/g, '_').slice(0, 180)

const recordValue = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const numberValue = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const githubDeliveryRef = (
  event: AgentDefinitionWebhookNormalizedEvent,
): string =>
  event.sourceRefs.find(ref => ref.startsWith('github.delivery.')) ??
  `github.delivery.${compactRefSegment(event.deliveryId)}`

const githubContentRef = (
  event: AgentDefinitionWebhookNormalizedEvent,
): string =>
  event.sourceRefs.find(ref => ref.startsWith('github.comment.')) ??
  event.sourceRefs.find(ref => ref.startsWith('github.issue.')) ??
  event.sourceRefs.find(ref => ref.startsWith('github.pull_request.')) ??
  event.subjectRef

const githubActorRef = (
  event: AgentDefinitionWebhookNormalizedEvent,
): string => {
  const sender = recordValue(event.payload.sender)
  const login = stringValue(sender?.login)

  return login === undefined
    ? 'github.user.unknown'
    : `github.user.${compactRefSegment(login)}`
}

const githubPayloadSummary = (
  event: AgentDefinitionWebhookNormalizedEvent,
): Record<string, unknown> => {
  const repository = recordValue(event.payload.repository)
  const subject = recordValue(event.payload.subject)
  const comment = recordValue(event.payload.comment)
  const sender = recordValue(event.payload.sender)
  const mention = recordValue(event.payload.mention)

  return {
    action: stringValue(event.payload.action),
    commentId: numberValue(comment?.id),
    event: stringValue(event.payload.event),
    mentionTarget: stringValue(mention?.target_login),
    repositoryFullName: stringValue(repository?.full_name),
    senderLogin: stringValue(sender?.login),
    subjectKind: stringValue(subject?.kind),
    subjectNumber: numberValue(subject?.number),
    subjectState: stringValue(subject?.state),
  }
}

const slackDeliveryRef = (
  event: AgentDefinitionWebhookNormalizedEvent,
): string =>
  event.sourceRefs.find(ref => ref.startsWith('slack.event.')) ??
  `slack.event.${compactRefSegment(event.deliveryId)}`

const slackContentRef = (
  event: AgentDefinitionWebhookNormalizedEvent,
): string =>
  event.sourceRefs.find(ref => ref.startsWith('slack.message.')) ??
  event.sourceRefs.find(ref => ref.startsWith('slack.thread.')) ??
  event.sourceRefs.find(ref => ref.startsWith('slack.channel.')) ??
  event.subjectRef

const slackActorRef = (
  event: AgentDefinitionWebhookNormalizedEvent,
): string => {
  const actor = recordValue(event.payload.actor)
  const userId = stringValue(actor?.user_id)
  const botId = stringValue(actor?.bot_id)

  if (userId !== undefined) {
    return `slack.user.${compactRefSegment(userId)}`
  }

  if (botId !== undefined) {
    return `slack.bot.${compactRefSegment(botId)}`
  }

  return 'slack.actor.unknown'
}

const slackPayloadSummary = (
  event: AgentDefinitionWebhookNormalizedEvent,
): Record<string, unknown> => {
  const team = recordValue(event.payload.team)
  const channel = recordValue(event.payload.channel)
  const actor = recordValue(event.payload.actor)
  const message = recordValue(event.payload.message)

  return {
    actorBotId: stringValue(actor?.bot_id),
    actorUserId: stringValue(actor?.user_id),
    channelId: stringValue(channel?.id),
    channelType: stringValue(channel?.type),
    event: stringValue(event.payload.event),
    eventTime: numberValue(event.payload.event_time),
    eventTs: stringValue(message?.event_ts),
    messageTs: stringValue(message?.ts),
    subtype: stringValue(event.payload.subtype),
    teamId: stringValue(team?.id),
    threadTs: stringValue(message?.thread_ts),
    type: stringValue(event.payload.type),
  }
}

const eventLedgerGithubMessageForMatchedTrigger = (
  event: AgentDefinitionWebhookNormalizedEvent,
  trigger: DueAgentDefinitionTriggerRecord,
): EventLedgerIngestQueueMessage =>
  new EventLedgerIngestQueueMessage({
    schemaVersion: EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
    actorRef: githubActorRef(event),
    contentRef: githubContentRef(event),
    eventType: event.eventType,
    externalRef: githubDeliveryRef(event),
    occurredAt: event.receivedAt,
    ownerAgentUserId: trigger.ownerAgentUserId,
    ownerRef: trigger.ownerRef,
    payloadSummary: githubPayloadSummary(event),
    receivedAt: event.receivedAt,
    source: 'github',
    sourceRefs: event.sourceRefs,
    subjectRef: event.subjectRef,
    trainingConsent: false,
  })

const eventLedgerSlackMessageForMatchedTrigger = (
  event: AgentDefinitionWebhookNormalizedEvent,
  trigger: DueAgentDefinitionTriggerRecord,
): EventLedgerIngestQueueMessage =>
  new EventLedgerIngestQueueMessage({
    schemaVersion: EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
    actorRef: slackActorRef(event),
    contentRef: slackContentRef(event),
    eventType: event.eventType,
    externalRef: slackDeliveryRef(event),
    occurredAt: event.receivedAt,
    ownerAgentUserId: trigger.ownerAgentUserId,
    ownerRef: trigger.ownerRef,
    payloadSummary: slackPayloadSummary(event),
    receivedAt: event.receivedAt,
    source: 'slack',
    sourceRefs: event.sourceRefs,
    subjectRef: event.subjectRef,
    trainingConsent: false,
  })

export const eventLedgerMessageForMatchedTrigger = (
  event: AgentDefinitionWebhookNormalizedEvent,
  trigger: DueAgentDefinitionTriggerRecord,
): EventLedgerIngestQueueMessage | undefined =>
  event.source === 'github'
    ? eventLedgerGithubMessageForMatchedTrigger(event, trigger)
    : event.source === 'slack'
      ? eventLedgerSlackMessageForMatchedTrigger(event, trigger)
      : undefined

const entryIdFor = (
  message: EventLedgerIngestQueueMessage,
  orderingSequence: number,
): string =>
  `event_ledger.${message.source}.${compactRefSegment(
    message.ownerAgentUserId,
  )}.${String(orderingSequence).padStart(12, '0')}`

const rowToEntry = (row: EventLedgerEntryRow): EventLedgerEntry => ({
  actorRef: row.actor_ref,
  contentRef: row.content_ref,
  createdAt: row.created_at,
  entryId: row.entry_id,
  eventType: row.event_type,
  externalRef: row.external_ref,
  handledAt: row.handled_at,
  handledByDefinitionId: row.handled_by_definition_id,
  handledByRunId: row.handled_by_run_id,
  handledReasonRef: row.handled_reason_ref,
  handledState: row.handled_state,
  occurredAt: row.occurred_at,
  orderingKey: row.ordering_key,
  orderingSequence: row.ordering_sequence,
  ownerAgentUserId: row.owner_agent_user_id,
  ownerRef: row.owner_ref,
  payloadSummary: parseBoundaryJsonRecord(row.payload_summary_json) ?? {},
  receivedAt: row.received_at,
  source: row.source,
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  subjectRef: row.subject_ref,
  trainingConsent: false,
  updatedAt: row.updated_at,
})

const EVENT_LEDGER_SELECT_COLUMNS = `entry_id, owner_agent_user_id, owner_ref,
        source, external_ref, actor_ref, content_ref, subject_ref, event_type,
        source_refs_json, payload_summary_json, occurred_at, received_at,
        ordering_key, ordering_sequence, handled_state, handled_by_run_id,
        handled_by_definition_id, handled_at, handled_reason_ref,
        training_consent, created_at, updated_at`

const safeLimit = (limit: number): number =>
  Number.isFinite(limit) ? Math.max(1, Math.min(Math.trunc(limit), 500)) : 50

export const makeD1EventLedgerStore = (db: D1Database): EventLedgerStore => ({
  insertEntry: async ({ entryId, message, nowIso, orderingSequence }) => {
    const orderingKey = `${message.source}:${message.externalRef}`

    await db
      .prepare(
        `INSERT OR IGNORE INTO event_ledger_entries
          (entry_id, owner_agent_user_id, owner_ref, source, external_ref,
           actor_ref, content_ref, subject_ref, event_type, source_refs_json,
           payload_summary_json, occurred_at, received_at, ordering_key,
           ordering_sequence, training_consent, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)`,
      )
      .bind(
        entryId,
        message.ownerAgentUserId,
        message.ownerRef,
        message.source,
        message.externalRef,
        message.actorRef,
        message.contentRef,
        message.subjectRef,
        message.eventType,
        JSON.stringify(message.sourceRefs),
        JSON.stringify(message.payloadSummary),
        message.occurredAt,
        message.receivedAt,
        orderingKey,
        orderingSequence,
        nowIso,
        nowIso,
      )
      .run()

    const row = await db
      .prepare(
        `SELECT ${EVENT_LEDGER_SELECT_COLUMNS}
           FROM event_ledger_entries
          WHERE owner_agent_user_id = ?
            AND source = ?
            AND external_ref = ?
          LIMIT 1`,
      )
      .bind(message.ownerAgentUserId, message.source, message.externalRef)
      .first<EventLedgerEntryRow>()

    if (row === null) {
      throw { error: 'event_ledger_entry_not_persisted' }
    }

    return rowToEntry(row)
  },
  listOwnerEntries: async input => {
    const clauses = ['owner_agent_user_id = ?']
    const params: Array<string | number> = [input.ownerAgentUserId]

    if (input.subjectRef !== undefined) {
      clauses.push('subject_ref = ?')
      params.push(input.subjectRef)
    }

    if (
      input.handledStates !== undefined &&
      input.handledStates.length > 0
    ) {
      clauses.push(
        `handled_state IN (${input.handledStates.map(() => '?').join(', ')})`,
      )
      params.push(...input.handledStates)
    }

    const rows = await db
      .prepare(
        `SELECT ${EVENT_LEDGER_SELECT_COLUMNS}
           FROM event_ledger_entries
          WHERE ${clauses.join(' AND ')}
          ORDER BY ordering_sequence ASC
          LIMIT ?`,
      )
      .bind(...params, safeLimit(input.limit))
      .all<EventLedgerEntryRow>()

    return (rows.results ?? []).map(rowToEntry)
  },
  readOwnerEntry: async (ownerAgentUserId, entryId) => {
    const row = await db
      .prepare(
        `SELECT ${EVENT_LEDGER_SELECT_COLUMNS}
           FROM event_ledger_entries
          WHERE owner_agent_user_id = ?
            AND entry_id = ?
          LIMIT 1`,
      )
      .bind(ownerAgentUserId, entryId)
      .first<EventLedgerEntryRow>()

    return row === null ? undefined : rowToEntry(row)
  },
  updateHandledState: async input => {
    await db
      .prepare(
        `UPDATE event_ledger_entries
            SET handled_state = ?,
                handled_by_run_id = ?,
                handled_by_definition_id = ?,
                handled_at = ?,
                handled_reason_ref = ?,
                updated_at = ?
          WHERE owner_agent_user_id = ?
            AND entry_id = ?`,
      )
      .bind(
        input.handledState,
        input.handledByRunId,
        input.handledByDefinitionId,
        input.handledAt,
        input.handledReasonRef ?? null,
        input.handledAt,
        input.ownerAgentUserId,
        input.entryId,
      )
      .run()

    return makeD1EventLedgerStore(db).readOwnerEntry(
      input.ownerAgentUserId,
      input.entryId,
    )
  },
})

export const makeMirroredEventLedgerStore = (
  d1: EventLedgerStore,
  mirror: AgentRuntimeRemainderMirror | undefined,
): EventLedgerStore => {
  if (mirror === undefined) {
    return d1
  }

  const mirrorEntry = (entry: EventLedgerEntry | undefined): Promise<void> =>
    entry === undefined
      ? Promise.resolve()
      : mirror.mirrorRowsByPk('event_ledger_entries', [entry.entryId])

  return {
    insertEntry: async input => {
      const entry = await d1.insertEntry(input)
      await mirrorEntry(entry)
      return entry
    },
    listOwnerEntries: input => d1.listOwnerEntries(input),
    readOwnerEntry: (ownerAgentUserId, entryId) =>
      d1.readOwnerEntry(ownerAgentUserId, entryId),
    updateHandledState: async input => {
      const entry = await d1.updateHandledState(input)
      await mirrorEntry(entry)
      return entry
    },
  }
}

export const makeEventLedgerStoreForEnv = (
  env: AgentRuntimeRemainderStoreEnv & Readonly<{ OPENAGENTS_DB: D1Database }>,
): EventLedgerStore => {
  const d1 = makeD1EventLedgerStore(openAgentsDatabase(env))
  return makeMirroredEventLedgerStore(
    d1,
    makeAgentRuntimeRemainderMirrorForEnv(env),
  )
}

export type EventLedgerGatewayEntry = Readonly<{
  actorRef?: string
  contentRef?: string
  entryId: string
  eventType: string
  externalRef?: string
  handledAt: string | null
  handledByDefinitionId: string | null
  handledByRunId: string | null
  handledReasonRef: string | null
  handledState: EventLedgerHandledState
  occurredAt: string
  orderingSequence: number
  receivedAt: string
  redactionClass: 'owner_scoped_refs' | 'state_only'
  source: EventLedgerSource
  sourceRefs?: ReadonlyArray<string>
  subjectRef?: string
}>

export type EventLedgerGatewayReadProjection = Readonly<{
  count: number
  definitionId: string
  entries: ReadonlyArray<EventLedgerGatewayEntry>
  redaction: Readonly<{
    policy: AgentDefinition['toolset']['secretPolicy']
    redactionClass: EventLedgerGatewayEntry['redactionClass']
  }>
  schema: typeof EVENT_LEDGER_GATEWAY_READ_SCHEMA
}>

const gatewayEntryForDefinition = (
  definition: AgentDefinition,
  entry: EventLedgerEntry,
): EventLedgerGatewayEntry => {
  const base = {
    entryId: entry.entryId,
    eventType: entry.eventType,
    handledAt: entry.handledAt,
    handledByDefinitionId: entry.handledByDefinitionId,
    handledByRunId: entry.handledByRunId,
    handledReasonRef: entry.handledReasonRef,
    handledState: entry.handledState,
    occurredAt: entry.occurredAt,
    orderingSequence: entry.orderingSequence,
    receivedAt: entry.receivedAt,
    source: entry.source,
  }

  return definition.toolset.secretPolicy === 'owner_scoped_refs_only'
    ? {
        ...base,
        actorRef: entry.actorRef,
        contentRef: entry.contentRef,
        externalRef: entry.externalRef,
        redactionClass: 'owner_scoped_refs',
        sourceRefs: entry.sourceRefs,
        subjectRef: entry.subjectRef,
      }
    : {
        ...base,
        redactionClass: 'state_only',
      }
}

export const eventLedgerGatewayReadProjectionForDefinition = (
  definition: AgentDefinition,
  entries: ReadonlyArray<EventLedgerEntry>,
): EventLedgerGatewayReadProjection => {
  const projectedEntries = entries.map(entry =>
    gatewayEntryForDefinition(definition, entry),
  )
  const redactionClass =
    definition.toolset.secretPolicy === 'owner_scoped_refs_only'
      ? 'owner_scoped_refs'
      : 'state_only'

  return {
    count: projectedEntries.length,
    definitionId: definition.id,
    entries: projectedEntries,
    redaction: {
      policy: definition.toolset.secretPolicy,
      redactionClass,
    },
    schema: EVENT_LEDGER_GATEWAY_READ_SCHEMA,
  }
}

export const recordEventLedgerIngestMessage = async (
  dependencies: Readonly<{
    nowIso?: (() => string) | undefined
    sequenceStore: EventLedgerOwnerSequenceStore
    store: EventLedgerStore
  }>,
  message: EventLedgerIngestQueueMessage,
): Promise<EventLedgerIngestOutcome> => {
  const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
  const reservation = await dependencies.sequenceStore.reserve(message)
  const entryId = entryIdFor(message, reservation.orderingSequence)

  await dependencies.store.insertEntry({
    entryId,
    message,
    nowIso,
    orderingSequence: reservation.orderingSequence,
  })
  await dependencies.sequenceStore.markPersisted(
    reservation.orderingKey,
    nowIso,
  )

  return {
    duplicate: reservation.duplicate,
    entryId,
    orderingSequence: reservation.orderingSequence,
    ownerAgentUserId: message.ownerAgentUserId,
    persisted: true,
    schema: EVENT_LEDGER_INGEST_OUTCOME_SCHEMA,
    source: message.source,
  }
}

type EventLedgerOwnerSequenceRow = Readonly<{
  ordering_sequence: number
  persisted_at: string | null
}>

const makeDurableObjectSequenceStore = (
  storage: DurableObjectStorage,
): EventLedgerOwnerSequenceStore => ({
  markPersisted: async (orderingKey, persistedAt) => {
    storage.sql.exec(
      `UPDATE event_ledger_owner_order
          SET persisted_at = ?
        WHERE ordering_key = ?`,
      persistedAt,
      orderingKey,
    )
  },
  reserve: async message => {
    const orderingKey = `${message.source}:${message.externalRef}`
    const existing = storage.sql
      .exec<EventLedgerOwnerSequenceRow>(
        `SELECT ordering_sequence, persisted_at
           FROM event_ledger_owner_order
          WHERE ordering_key = ?`,
        orderingKey,
      )
      .toArray()[0]

    if (existing !== undefined) {
      return {
        duplicate: true,
        orderingKey,
        orderingSequence: Number(existing.ordering_sequence),
        persisted: existing.persisted_at !== null,
      }
    }

    const sequenceRow = storage.sql
      .exec<{ next_sequence: number }>(
        `SELECT COALESCE(MAX(ordering_sequence), 0) + 1 AS next_sequence
           FROM event_ledger_owner_order`,
      )
      .toArray()[0]
    const orderingSequence = Number(sequenceRow?.next_sequence ?? 1)

    storage.sql.exec(
      `INSERT INTO event_ledger_owner_order
        (ordering_key, ordering_sequence, first_seen_at, persisted_at)
       VALUES (?, ?, ?, NULL)`,
      orderingKey,
      orderingSequence,
      message.receivedAt,
    )

    return {
      duplicate: false,
      orderingKey,
      orderingSequence,
      persisted: false,
    }
  },
})

type EventLedgerOwnerDurableObjectEnv = AgentRuntimeRemainderStoreEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
  }>

export class EventLedgerOwnerDurableObject {
  constructor(
    private readonly state: DurableObjectState,
    private readonly env: EventLedgerOwnerDurableObjectEnv,
  ) {
    this.state.blockConcurrencyWhile(async () => {
      this.state.storage.sql.exec(
        `CREATE TABLE IF NOT EXISTS event_ledger_owner_order (
          ordering_key TEXT PRIMARY KEY,
          ordering_sequence INTEGER NOT NULL UNIQUE,
          first_seen_at TEXT NOT NULL,
          persisted_at TEXT
        )`,
      )
    })
  }

  async record(
    message: EventLedgerIngestQueueMessage,
  ): Promise<EventLedgerIngestOutcome> {
    const decoded = S.decodeUnknownSync(EventLedgerIngestQueueMessage)(message)

    return recordEventLedgerIngestMessage(
      {
        sequenceStore: makeDurableObjectSequenceStore(this.state.storage),
        store: makeEventLedgerStoreForEnv(this.env),
      },
      decoded,
    )
  }
}

export const recordEventLedgerMessageWithOwnerObject = (
  namespace: DurableObjectNamespace,
  message: EventLedgerIngestQueueMessage,
): Promise<EventLedgerIngestOutcome> => {
  const stub = namespace.getByName(message.ownerAgentUserId) as unknown as {
    record: (
      input: EventLedgerIngestQueueMessage,
    ) => Promise<EventLedgerIngestOutcome>
  }

  return stub.record(message)
}
