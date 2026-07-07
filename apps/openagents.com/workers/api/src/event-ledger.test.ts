import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { decodeAgentDefinition } from '@openagentsinc/agent-runtime-schema'
import {
  normalizeGitHubWebhookEvent,
  normalizeSlackWebhookEvent,
} from '@openagentsinc/agent-runtime-schema/webhooks'
import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { DueAgentDefinitionTriggerRecord } from './agent-definition-trigger-store'
import {
  EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
  EVENT_LEDGER_GATEWAY_READ_SCHEMA,
  EventLedgerIngestQueueMessage,
  type EventLedgerOwnerSequenceReservation,
  type EventLedgerOwnerSequenceStore,
  eventLedgerGatewayReadProjectionForDefinition,
  eventLedgerMessageForMatchedTrigger,
  makeD1EventLedgerStore,
  makeMirroredEventLedgerStore,
  makePostgresEventLedgerStore,
  recordEventLedgerIngestMessage,
} from './event-ledger'

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => value === undefined ? null : value)

    return this
  }

  async first<T = Record<string, unknown>>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }

  async all<T = Record<string, unknown>>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db
        .prepare(this.sql)
        .all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{
    readonly meta: { readonly changes: number }
    readonly results: []
    readonly success: true
  }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))

    return {
      meta: { changes: Number(result.changes ?? 0) },
      results: [],
      success: true,
    }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

class MemoryOwnerSequenceStore implements EventLedgerOwnerSequenceStore {
  private readonly reservations = new Map<
    string,
    EventLedgerOwnerSequenceReservation
  >()

  async markPersisted(orderingKey: string, _persistedAt: string): Promise<void> {
    const existing = this.reservations.get(orderingKey)

    if (existing !== undefined) {
      this.reservations.set(orderingKey, { ...existing, persisted: true })
    }
  }

  async reserve(
    message: EventLedgerIngestQueueMessage,
  ): Promise<EventLedgerOwnerSequenceReservation> {
    const orderingKey = `${message.source}:${message.externalRef}`
    const existing = this.reservations.get(orderingKey)

    if (existing !== undefined) {
      const duplicate = {
        ...existing,
        duplicate: true,
      }
      this.reservations.set(orderingKey, duplicate)

      return duplicate
    }

    const reservation = {
      duplicate: false,
      orderingKey,
      orderingSequence: this.reservations.size + 1,
      persisted: false,
    }
    this.reservations.set(orderingKey, reservation)

    return reservation
  }
}

const eventLedgerMigration = readFileSync(
  new URL('../migrations/0285_event_ledger.sql', import.meta.url),
  'utf8',
)
const eventLedgerHandledStateMigration = readFileSync(
  new URL('../migrations/0286_event_ledger_handled_state.sql', import.meta.url),
  'utf8',
)
const eventLedgerSlackSourceMigration = readFileSync(
  new URL('../migrations/0287_event_ledger_slack_source.sql', import.meta.url),
  'utf8',
)

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(eventLedgerMigration)
  raw.exec(eventLedgerHandledStateMigration)
  raw.exec(eventLedgerSlackSourceMigration)

  return new SqliteD1(raw) as unknown as D1Database
}

const githubMentionCommentPayload = {
  action: 'created',
  comment: {
    author_association: 'MEMBER',
    body: '@OpenAgents Secret-ish prose must not be stored in the ledger.',
    html_url:
      'https://github.com/OpenAgentsInc/openagents/issues/8212#issuecomment-1001',
    id: 1001,
    user: {
      id: 790,
      login: 'AtlantisPleb',
    },
  },
  issue: {
    html_url: 'https://github.com/OpenAgentsInc/openagents/issues/8212',
    number: 8212,
    state: 'open',
    title: 'Secret-ish title must not be stored in the ledger summary',
  },
  repository: {
    full_name: 'OpenAgentsInc/openagents',
    id: 123,
    name: 'openagents',
    owner: {
      id: 456,
      login: 'OpenAgentsInc',
    },
  },
  sender: {
    id: 789,
    login: 'AtlantisPleb',
  },
}

const slackMessagePayload = {
  api_app_id: 'A_BACKGROUND_AGENT',
  event: {
    channel: 'C_BACKGROUND',
    channel_type: 'channel',
    event_ts: '1783152000.000200',
    text:
      'Secret-ish Slack body must not be stored in the event ledger.',
    ts: '1783152000.000100',
    type: 'message',
    user: 'U_OWNER',
  },
  event_id: 'Ev_BACKGROUND_8214',
  event_time: 1783152000,
  team_id: 'T_OPENAGENTS',
  token: 'legacy-verification-token-is-not-authority',
  type: 'event_callback',
}

const triggerRecord = (
  ownerAgentUserId: string,
): DueAgentDefinitionTriggerRecord => ({
  schema: 'openagents.agent_definition_trigger.v1',
  consecutiveFailures: 0,
  createdAt: '2026-07-04T00:00:00.000Z',
  definitionId: 'agent_definition.public.event_ledger',
  ownerAgentUserId,
  ownerRef: `agent:${ownerAgentUserId}`,
  state: 'enabled',
  trigger: {
    conditions: [{ equals: 'issue_comment.created.mention', kind: 'event_type' }],
    kind: 'inbound_webhook',
    source: 'github',
    triggerRef: 'trigger.public.event_ledger.github_mentions',
  },
  triggerId:
    'agent_definition.public.event_ledger:trigger.public.event_ledger.github_mentions',
  triggerRef: 'trigger.public.event_ledger.github_mentions',
  updatedAt: '2026-07-04T00:00:00.000Z',
})

const slackTriggerRecord = (
  ownerAgentUserId: string,
): DueAgentDefinitionTriggerRecord => ({
  schema: 'openagents.agent_definition_trigger.v1',
  consecutiveFailures: 0,
  createdAt: '2026-07-04T00:00:00.000Z',
  definitionId: 'agent_definition.public.event_ledger',
  ownerAgentUserId,
  ownerRef: `agent:${ownerAgentUserId}`,
  state: 'enabled',
  trigger: {
    conditions: [{ equals: 'message', kind: 'event_type' }],
    kind: 'inbound_webhook',
    source: 'slack',
    triggerRef: 'trigger.public.event_ledger.slack_messages',
  },
  triggerId:
    'agent_definition.public.event_ledger:trigger.public.event_ledger.slack_messages',
  triggerRef: 'trigger.public.event_ledger.slack_messages',
  updatedAt: '2026-07-04T00:00:00.000Z',
})

const normalizedGitHubMentionEvent = () => {
  const event = normalizeGitHubWebhookEvent({
    deliveryId: 'delivery-8212',
    eventName: 'issue_comment',
    payload: githubMentionCommentPayload,
    receivedAt: '2026-07-04T00:05:00.000Z',
  })

  expect(event).toBeDefined()

  return event!
}

const normalizedSlackMessageEvent = () => {
  const event = normalizeSlackWebhookEvent({
    deliveryId: 'Ev_BACKGROUND_8214',
    payload: slackMessagePayload,
    receivedAt: '2026-07-04T02:00:00.000Z',
  })

  expect(event).toBeDefined()

  return event!
}

const definition = (
  secretPolicy: 'none' | 'owner_scoped_refs_only',
) =>
  decodeAgentDefinition({
    schema: 'openagents.agent_definition.v1',
    id: 'agent_definition.public.event_ledger',
    ownerRef: 'agent:agent_user_owner_a',
    name: 'Event Ledger Reader',
    slug: 'event-ledger-reader',
    goal: 'Read the private event ledger through a redacting gateway.',
    harness: { kind: 'codex' },
    toolset: {
      allow: ['tool.openagents.event_ledger.*'],
      deny: [],
      ask: [],
      networkPolicy: 'owner_scoped',
      secretPolicy,
    },
    triggers: [{ kind: 'manual', triggerRef: 'trigger.public.event_ledger.manual' }],
    lane: 'own_pylon',
    budget: { maxRunSeconds: 900, maxRunsPerDay: 3, maxCreditsPerDay: 0 },
    escalation: {
      channel: 'operator',
      askPolicy: {
        mode: 'operator_required',
        policyRef: 'policy.public.agent_definition.operator_required.v1',
      },
    },
    sourceRefs: ['github.issue.8213'],
    createdAt: '2026-07-04T00:00:00.000Z',
    updatedAt: '2026-07-04T00:00:00.000Z',
  })

describe('event ledger ingest', () => {
  // background_agents.inbox.event_ledger_owner_scoped_private.v1
  // background_agents.inbox.event_ledger_handled_gateway_redacted.v1
  test('builds a GitHub queue message with owner scope and without raw content', () => {
    const message = eventLedgerMessageForMatchedTrigger(
      normalizedGitHubMentionEvent(),
      triggerRecord('agent_user_owner_a'),
    )

    expect(message).toBeDefined()
    expect(message).toMatchObject({
      schemaVersion: EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
      actorRef: 'github.user.AtlantisPleb',
      contentRef: 'github.comment.OpenAgentsInc/openagents.1001',
      eventType: 'issue_comment.created.mention',
      externalRef: 'github.delivery.delivery-8212',
      ownerAgentUserId: 'agent_user_owner_a',
      source: 'github',
      subjectRef: 'github.repository.OpenAgentsInc/openagents.issue.8212',
      trainingConsent: false,
    })
    expect(
      S.decodeUnknownSync(EventLedgerIngestQueueMessage)(
        JSON.parse(JSON.stringify(message)),
      ),
    ).toMatchObject(message!)
    expect(JSON.stringify(message)).not.toContain('Secret-ish')
  })

  test('builds a Slack queue message with owner scope and without raw message text', () => {
    // background_agents.inbox.slack_event_ledger_ingest.v1
    const message = eventLedgerMessageForMatchedTrigger(
      normalizedSlackMessageEvent(),
      slackTriggerRecord('agent_user_owner_a'),
    )

    expect(message).toBeDefined()
    expect(message).toMatchObject({
      schemaVersion: EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
      actorRef: 'slack.user.U_OWNER',
      contentRef:
        'slack.message.T_OPENAGENTS.C_BACKGROUND.1783152000.000100',
      eventType: 'message',
      externalRef: 'slack.event.Ev_BACKGROUND_8214',
      ownerAgentUserId: 'agent_user_owner_a',
      source: 'slack',
      subjectRef:
        'slack.team.T_OPENAGENTS.channel.C_BACKGROUND.message.1783152000.000100',
      trainingConsent: false,
    })
    expect(
      S.decodeUnknownSync(EventLedgerIngestQueueMessage)(
        JSON.parse(JSON.stringify(message)),
      ),
    ).toMatchObject({
      actorRef: 'slack.user.U_OWNER',
      contentRef:
        'slack.message.T_OPENAGENTS.C_BACKGROUND.1783152000.000100',
      externalRef: 'slack.event.Ev_BACKGROUND_8214',
      payloadSummary: {
        actorUserId: 'U_OWNER',
        channelId: 'C_BACKGROUND',
        event: 'message',
        messageTs: '1783152000.000100',
        teamId: 'T_OPENAGENTS',
      },
      source: 'slack',
    })
    expect(JSON.stringify(message)).not.toContain('Secret-ish')
    expect(JSON.stringify(message)).not.toContain('legacy-verification-token')
  })

  test('persists D1 rows with per-owner ordering and idempotent dedup', async () => {
    const db = makeDb()
    const store = makeD1EventLedgerStore(db)
    const sequenceStore = new MemoryOwnerSequenceStore()
    const firstMessage = eventLedgerMessageForMatchedTrigger(
      normalizedGitHubMentionEvent(),
      triggerRecord('agent_user_owner_a'),
    )
    expect(firstMessage).toBeDefined()

    const first = await recordEventLedgerIngestMessage(
      {
        nowIso: () => '2026-07-04T00:06:00.000Z',
        sequenceStore,
        store,
      },
      firstMessage!,
    )
    const duplicate = await recordEventLedgerIngestMessage(
      {
        nowIso: () => '2026-07-04T00:07:00.000Z',
        sequenceStore,
        store,
      },
      firstMessage!,
    )

    expect(first).toMatchObject({
      duplicate: false,
      orderingSequence: 1,
      ownerAgentUserId: 'agent_user_owner_a',
      persisted: true,
      source: 'github',
    })
    expect(duplicate).toMatchObject({
      duplicate: true,
      orderingSequence: 1,
      ownerAgentUserId: 'agent_user_owner_a',
      persisted: true,
    })

    const ownerRows = await store.listOwnerEntries({
      limit: 10,
      ownerAgentUserId: 'agent_user_owner_a',
    })
    expect(ownerRows).toHaveLength(1)
    expect(ownerRows[0]).toMatchObject({
      actorRef: 'github.user.AtlantisPleb',
      contentRef: 'github.comment.OpenAgentsInc/openagents.1001',
      handledAt: null,
      handledByDefinitionId: null,
      handledByRunId: null,
      handledState: 'open',
      orderingSequence: 1,
      source: 'github',
      subjectRef: 'github.repository.OpenAgentsInc/openagents.issue.8212',
      trainingConsent: false,
    })
    expect(JSON.stringify(ownerRows[0]?.payloadSummary)).not.toContain(
      'Secret-ish',
    )

    const secondOwnerStore = new MemoryOwnerSequenceStore()
    const sameExternalRefSecondOwner = eventLedgerMessageForMatchedTrigger(
      normalizedGitHubMentionEvent(),
      triggerRecord('agent_user_owner_b'),
    )
    expect(sameExternalRefSecondOwner).toBeDefined()
    const secondOwner = await recordEventLedgerIngestMessage(
      {
        nowIso: () => '2026-07-04T00:08:00.000Z',
        sequenceStore: secondOwnerStore,
        store,
      },
      sameExternalRefSecondOwner!,
    )

    expect(secondOwner).toMatchObject({
      duplicate: false,
      orderingSequence: 1,
      ownerAgentUserId: 'agent_user_owner_b',
    })
    expect(
      await store.listOwnerEntries({
        limit: 10,
        ownerAgentUserId: 'agent_user_owner_b',
      }),
    ).toHaveLength(1)
  })

  test('mirrored store mirrors inserted and updated event-ledger rows by key', async () => {
    const db = makeDb()
    const mirrored: Array<{ table: string; ids: ReadonlyArray<string> }> = []
    const store = makeMirroredEventLedgerStore(makeD1EventLedgerStore(db), {
      mirrorRowsByPk: async (table, ids) => {
        mirrored.push({ ids, table })
      },
    })
    const sequenceStore = new MemoryOwnerSequenceStore()
    const message = eventLedgerMessageForMatchedTrigger(
      normalizedGitHubMentionEvent(),
      triggerRecord('agent_user_owner_a'),
    )
    expect(message).toBeDefined()

    const outcome = await recordEventLedgerIngestMessage(
      {
        nowIso: () => '2026-07-04T00:06:00.000Z',
        sequenceStore,
        store,
      },
      message!,
    )

    expect(mirrored).toEqual([
      { ids: [outcome.entryId], table: 'event_ledger_entries' },
    ])

    await store.updateHandledState({
      entryId: outcome.entryId,
      handledAt: '2026-07-04T00:10:00.000Z',
      handledByDefinitionId: 'definition-1',
      handledByRunId: 'run-1',
      handledReasonRef: 'event-ledger.test',
      handledState: 'handled',
      ownerAgentUserId: 'agent_user_owner_a',
    })

    expect(mirrored).toEqual([
      { ids: [outcome.entryId], table: 'event_ledger_entries' },
      { ids: [outcome.entryId], table: 'event_ledger_entries' },
    ])
  })

  test('persists Slack D1 rows under the same owner-scoped privacy contract', async () => {
    // background_agents.inbox.slack_event_ledger_ingest.v1
    const db = makeDb()
    const store = makeD1EventLedgerStore(db)
    const sequenceStore = new MemoryOwnerSequenceStore()
    const message = eventLedgerMessageForMatchedTrigger(
      normalizedSlackMessageEvent(),
      slackTriggerRecord('agent_user_owner_a'),
    )
    expect(message).toBeDefined()

    const outcome = await recordEventLedgerIngestMessage(
      {
        nowIso: () => '2026-07-04T02:01:00.000Z',
        sequenceStore,
        store,
      },
      message!,
    )

    expect(outcome).toMatchObject({
      duplicate: false,
      orderingSequence: 1,
      ownerAgentUserId: 'agent_user_owner_a',
      persisted: true,
      source: 'slack',
    })

    const ownerRows = await store.listOwnerEntries({
      limit: 10,
      ownerAgentUserId: 'agent_user_owner_a',
    })

    expect(ownerRows).toHaveLength(1)
    expect(ownerRows[0]).toMatchObject({
      actorRef: 'slack.user.U_OWNER',
      contentRef:
        'slack.message.T_OPENAGENTS.C_BACKGROUND.1783152000.000100',
      handledState: 'open',
      orderingSequence: 1,
      source: 'slack',
      subjectRef:
        'slack.team.T_OPENAGENTS.channel.C_BACKGROUND.message.1783152000.000100',
      trainingConsent: false,
    })
    expect(JSON.stringify(ownerRows[0])).not.toContain('Secret-ish')
    expect(JSON.stringify(ownerRows[0])).not.toContain(
      'legacy-verification-token',
    )
  })

  test('records handled-state with the touching run and definition', async () => {
    const db = makeDb()
    const store = makeD1EventLedgerStore(db)
    const message = eventLedgerMessageForMatchedTrigger(
      normalizedGitHubMentionEvent(),
      triggerRecord('agent_user_owner_a'),
    )
    expect(message).toBeDefined()

    await recordEventLedgerIngestMessage(
      {
        nowIso: () => '2026-07-04T00:06:00.000Z',
        sequenceStore: new MemoryOwnerSequenceStore(),
        store,
      },
      message!,
    )
    const [openEntry] = await store.listOwnerEntries({
      handledStates: ['open'],
      limit: 10,
      ownerAgentUserId: 'agent_user_owner_a',
    })

    expect(openEntry).toBeDefined()
    const handled = await store.updateHandledState({
      entryId: openEntry!.entryId,
      handledAt: '2026-07-04T00:09:00.000Z',
      handledByDefinitionId: 'agent_definition.public.event_ledger',
      handledByRunId: 'agent_definition_run.public.touch_1',
      handledReasonRef: 'reason.agent_definition.event_ledger.responded',
      handledState: 'responded',
      ownerAgentUserId: 'agent_user_owner_a',
    })

    expect(handled).toMatchObject({
      entryId: openEntry!.entryId,
      handledAt: '2026-07-04T00:09:00.000Z',
      handledByDefinitionId: 'agent_definition.public.event_ledger',
      handledByRunId: 'agent_definition_run.public.touch_1',
      handledReasonRef: 'reason.agent_definition.event_ledger.responded',
      handledState: 'responded',
      updatedAt: '2026-07-04T00:09:00.000Z',
    })
    expect(
      await store.listOwnerEntries({
        handledStates: ['open'],
        limit: 10,
        ownerAgentUserId: 'agent_user_owner_a',
      }),
    ).toHaveLength(0)
    expect(
      await store.listOwnerEntries({
        handledStates: ['responded'],
        limit: 10,
        ownerAgentUserId: 'agent_user_owner_a',
      }),
    ).toHaveLength(1)
    expect(
      await store.updateHandledState({
        entryId: openEntry!.entryId,
        handledAt: '2026-07-04T00:10:00.000Z',
        handledByDefinitionId: 'agent_definition.public.event_ledger',
        handledByRunId: 'agent_definition_run.public.other_owner',
        handledState: 'ignored',
        ownerAgentUserId: 'agent_user_owner_b',
      }),
    ).toBeUndefined()
  })

  test('redacts gateway reads according to the definition secret policy', async () => {
    const db = makeDb()
    const store = makeD1EventLedgerStore(db)
    const message = eventLedgerMessageForMatchedTrigger(
      normalizedGitHubMentionEvent(),
      triggerRecord('agent_user_owner_a'),
    )
    expect(message).toBeDefined()

    await recordEventLedgerIngestMessage(
      {
        nowIso: () => '2026-07-04T00:06:00.000Z',
        sequenceStore: new MemoryOwnerSequenceStore(),
        store,
      },
      message!,
    )
    const entries = await store.listOwnerEntries({
      limit: 10,
      ownerAgentUserId: 'agent_user_owner_a',
    })
    const refsOnly = eventLedgerGatewayReadProjectionForDefinition(
      definition('owner_scoped_refs_only'),
      entries,
    )
    const stateOnly = eventLedgerGatewayReadProjectionForDefinition(
      definition('none'),
      entries,
    )

    expect(refsOnly).toMatchObject({
      schema: EVENT_LEDGER_GATEWAY_READ_SCHEMA,
      count: 1,
      redaction: {
        policy: 'owner_scoped_refs_only',
        redactionClass: 'owner_scoped_refs',
      },
    })
    expect(refsOnly.entries[0]).toMatchObject({
      actorRef: 'github.user.AtlantisPleb',
      contentRef: 'github.comment.OpenAgentsInc/openagents.1001',
      externalRef: 'github.delivery.delivery-8212',
      handledState: 'open',
      redactionClass: 'owner_scoped_refs',
      subjectRef: 'github.repository.OpenAgentsInc/openagents.issue.8212',
    })
    expect(JSON.stringify(refsOnly)).not.toContain('Secret-ish')
    expect(JSON.stringify(refsOnly)).not.toContain('payloadSummary')

    expect(stateOnly).toMatchObject({
      count: 1,
      redaction: {
        policy: 'none',
        redactionClass: 'state_only',
      },
    })
    expect(stateOnly.entries[0]).toMatchObject({
      eventType: 'issue_comment.created.mention',
      handledState: 'open',
      redactionClass: 'state_only',
      source: 'github',
    })
    expect(stateOnly.entries[0]).not.toHaveProperty('actorRef')
    expect(stateOnly.entries[0]).not.toHaveProperty('contentRef')
    expect(stateOnly.entries[0]).not.toHaveProperty('externalRef')
    expect(stateOnly.entries[0]).not.toHaveProperty('sourceRefs')
    expect(stateOnly.entries[0]).not.toHaveProperty('subjectRef')
  })

  test('keeps the Durable Object binding registered and the queues block deleted', () => {
    const wrangler = readFileSync(
      new URL('../wrangler.jsonc', import.meta.url),
      'utf8',
    )

    // CFG-7 (#8522): the Cloudflare `queues` block is gone — event-ledger
    // ingest rides the oa-infra Postgres JobQueue. The ordering Durable
    // Object stays.
    expect(wrangler).not.toContain('"queues":')
    expect(wrangler).not.toContain('EVENT_LEDGER_INGEST_QUEUE')
    expect(wrangler).toContain('"name": "EVENT_LEDGER_OWNER"')
    expect(wrangler).toContain('"class_name": "EventLedgerOwnerDurableObject"')
    expect(wrangler).toContain(
      '"new_sqlite_classes": ["EventLedgerOwnerDurableObject"]',
    )
  })
})

// ---------------------------------------------------------------------------
// CFG-17 (#8533): makePostgresEventLedgerStore — the D1 evacuation target.
//
// Backed by an in-memory Postgres double over the same tagged-template `sql`
// seam postgres.js exposes. Proves the store's four operations against
// event_ledger_entries: idempotent insert + read-back, per-owner listing with
// subject/handled-state filters, single-entry read, and handled-state update.
// ---------------------------------------------------------------------------

const makeFakeEventLedgerEntriesSql = () => {
  const rows: Array<Record<string, unknown>> = []

  const sql = (async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ): Promise<Array<Record<string, unknown>>> => {
    const q = strings.join(' ')
    const v = values as Array<unknown>

    if (q.includes('INSERT INTO event_ledger_entries')) {
      const [
        entry_id,
        owner_agent_user_id,
        owner_ref,
        source,
        external_ref,
        actor_ref,
        content_ref,
        subject_ref,
        event_type,
        source_refs_json,
        payload_summary_json,
        occurred_at,
        received_at,
        ordering_key,
        ordering_sequence,
        created_at,
        updated_at,
      ] = v
      const duplicate = rows.some(
        r =>
          r.owner_agent_user_id === owner_agent_user_id &&
          r.source === source &&
          r.external_ref === external_ref,
      )
      if (!duplicate) {
        rows.push({
          entry_id,
          owner_agent_user_id,
          owner_ref,
          source,
          external_ref,
          actor_ref,
          content_ref,
          subject_ref,
          event_type,
          source_refs_json,
          payload_summary_json,
          occurred_at,
          received_at,
          ordering_key,
          ordering_sequence: String(ordering_sequence),
          handled_state: 'open',
          handled_by_run_id: null,
          handled_by_definition_id: null,
          handled_at: null,
          handled_reason_ref: null,
          training_consent: 0,
          created_at,
          updated_at,
        })
      }
      return []
    }

    if (q.includes('UPDATE event_ledger_entries')) {
      const [
        handled_state,
        handled_by_run_id,
        handled_by_definition_id,
        handled_at,
        handled_reason_ref,
        updated_at,
        owner_agent_user_id,
        entry_id,
      ] = v
      const row = rows.find(
        r =>
          r.owner_agent_user_id === owner_agent_user_id &&
          r.entry_id === entry_id,
      )
      if (row !== undefined) {
        Object.assign(row, {
          handled_state,
          handled_by_run_id,
          handled_by_definition_id,
          handled_at,
          handled_reason_ref,
          updated_at,
        })
      }
      return []
    }

    if (q.includes('SELECT * FROM event_ledger_entries')) {
      if (q.includes('external_ref =')) {
        const [owner, source, external] = v as [string, string, string]
        return rows
          .filter(
            r =>
              r.owner_agent_user_id === owner &&
              r.source === source &&
              r.external_ref === external,
          )
          .slice(0, 1)
      }
      if (q.includes('entry_id =')) {
        const [owner, entry] = v as [string, string]
        return rows
          .filter(
            r => r.owner_agent_user_id === owner && r.entry_id === entry,
          )
          .slice(0, 1)
      }
      // listOwnerEntries: [owner, subjectRef, subjectRef, states, states, limit]
      const owner = v[0]
      const subjectRef = v[1] as string | null
      const handledStates = v[3] as ReadonlyArray<string> | null
      const limit = v[5] as number
      let out = rows.filter(r => r.owner_agent_user_id === owner)
      if (subjectRef !== null && subjectRef !== undefined) {
        out = out.filter(r => r.subject_ref === subjectRef)
      }
      if (handledStates !== null && handledStates !== undefined) {
        out = out.filter(r =>
          handledStates.includes(r.handled_state as string),
        )
      }
      return out
        .sort(
          (a, b) =>
            Number(a.ordering_sequence) - Number(b.ordering_sequence),
        )
        .slice(0, limit)
    }

    return []
  }) as never

  return { sql, rows }
}

const pgMessage = (
  overrides: Partial<{ externalRef: string; subjectRef: string }> = {},
) =>
  new EventLedgerIngestQueueMessage({
    schemaVersion: EVENT_LEDGER_INGEST_QUEUE_SCHEMA_VERSION,
    actorRef: 'github.user.octocat',
    contentRef: 'github.issue.7',
    eventType: 'issues.opened',
    externalRef: overrides.externalRef ?? 'github.delivery.pg',
    occurredAt: '2026-07-06T00:00:00.000Z',
    ownerAgentUserId: 'owner-pg',
    ownerRef: 'agent.owner-pg',
    payloadSummary: { action: 'opened' },
    receivedAt: '2026-07-06T00:00:01.000Z',
    source: 'github',
    sourceRefs: ['github.delivery.pg'],
    subjectRef: overrides.subjectRef ?? 'github.issue.7',
    trainingConsent: false,
  })

describe('makePostgresEventLedgerStore (CFG-17 #8533)', () => {
  test('insertEntry writes the row and reads it back with coerced sequence', async () => {
    const fake = makeFakeEventLedgerEntriesSql()
    const store = makePostgresEventLedgerStore(fake.sql)

    const entry = await store.insertEntry({
      entryId: 'entry-1',
      message: pgMessage({ externalRef: 'github.delivery.a' }),
      nowIso: '2026-07-06T00:00:02.000Z',
      orderingSequence: 1,
    })

    expect(entry.entryId).toBe('entry-1')
    expect(entry.orderingSequence).toBe(1)
    expect(typeof entry.orderingSequence).toBe('number')
    expect(entry.orderingKey).toBe('github:github.delivery.a')
    expect(entry.handledState).toBe('open')
    expect(entry.payloadSummary).toEqual({ action: 'opened' })
    expect(entry.trainingConsent).toBe(false)
    expect(fake.rows).toHaveLength(1)
  })

  test('insertEntry is idempotent on (owner, source, external_ref)', async () => {
    const fake = makeFakeEventLedgerEntriesSql()
    const store = makePostgresEventLedgerStore(fake.sql)

    await store.insertEntry({
      entryId: 'entry-1',
      message: pgMessage({ externalRef: 'github.delivery.dup' }),
      nowIso: '2026-07-06T00:00:02.000Z',
      orderingSequence: 1,
    })
    const second = await store.insertEntry({
      entryId: 'entry-1',
      message: pgMessage({ externalRef: 'github.delivery.dup' }),
      nowIso: '2026-07-06T00:00:03.000Z',
      orderingSequence: 1,
    })

    expect(second.entryId).toBe('entry-1')
    expect(fake.rows).toHaveLength(1)
  })

  test('listOwnerEntries filters by handled state and orders by sequence', async () => {
    const fake = makeFakeEventLedgerEntriesSql()
    const store = makePostgresEventLedgerStore(fake.sql)

    await store.insertEntry({
      entryId: 'entry-1',
      message: pgMessage({ externalRef: 'd.1' }),
      nowIso: '2026-07-06T00:00:02.000Z',
      orderingSequence: 2,
    })
    await store.insertEntry({
      entryId: 'entry-2',
      message: pgMessage({ externalRef: 'd.2' }),
      nowIso: '2026-07-06T00:00:03.000Z',
      orderingSequence: 1,
    })

    const all = await store.listOwnerEntries({
      limit: 50,
      ownerAgentUserId: 'owner-pg',
    })
    expect(all.map(e => e.orderingSequence)).toEqual([1, 2])

    const open = await store.listOwnerEntries({
      handledStates: ['open'],
      limit: 50,
      ownerAgentUserId: 'owner-pg',
    })
    expect(open).toHaveLength(2)

    const handled = await store.listOwnerEntries({
      handledStates: ['handled'],
      limit: 50,
      ownerAgentUserId: 'owner-pg',
    })
    expect(handled).toHaveLength(0)
  })

  test('updateHandledState mutates the row and returns the updated entry', async () => {
    const fake = makeFakeEventLedgerEntriesSql()
    const store = makePostgresEventLedgerStore(fake.sql)

    await store.insertEntry({
      entryId: 'entry-1',
      message: pgMessage({ externalRef: 'd.1' }),
      nowIso: '2026-07-06T00:00:02.000Z',
      orderingSequence: 1,
    })

    const updated = await store.updateHandledState({
      entryId: 'entry-1',
      handledAt: '2026-07-06T00:01:00.000Z',
      handledByDefinitionId: 'def-1',
      handledByRunId: 'run-1',
      handledState: 'handled',
      ownerAgentUserId: 'owner-pg',
    })

    expect(updated?.handledState).toBe('handled')
    expect(updated?.handledByRunId).toBe('run-1')
    expect(updated?.handledByDefinitionId).toBe('def-1')

    const read = await store.readOwnerEntry('owner-pg', 'entry-1')
    expect(read?.handledState).toBe('handled')
  })
})
