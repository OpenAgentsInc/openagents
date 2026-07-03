import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import {
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { fulfillmentLoopAgentDefinitionFixture } from '@openagentsinc/agent-runtime-schema/fixtures'
import { describe, expect, test } from 'vitest'

import { computeNextCronRunAt } from './agent-definition-cron'
import {
  makeD1AgentDefinitionTriggerStore,
  type AgentDefinitionTriggerStore,
} from './agent-definition-trigger-store'

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
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
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

const triggerMigration = readFileSync(
  new URL('../migrations/0281_agent_definition_triggers.sql', import.meta.url),
  'utf8',
)

const ownerAgentUserId = 'agent_user_owner'

const makeDefinition = (): AgentDefinition =>
  decodeAgentDefinition({
    ...fulfillmentLoopAgentDefinitionFixture,
    id: 'agent_definition.public.trigger_store_test',
    ownerRef: 'agent:agent_user_owner',
    triggers: [
      {
        kind: 'cron',
        triggerRef: 'trigger.public.trigger_store.quarter_hour',
        expr: '*/15 * * * *',
        tz: 'UTC',
      },
      {
        kind: 'inbound_webhook',
        triggerRef: 'trigger.public.trigger_store.github_issue',
        source: 'github',
        conditions: [
          {
            kind: 'event_type',
            equals: 'issues.opened',
          },
          {
            kind: 'json_path_equals',
            path: '$.repository.full_name',
            equals: 'OpenAgentsInc/openagents',
          },
        ],
      },
    ],
  })

const makeStore = (): AgentDefinitionTriggerStore => {
  const db = new DatabaseSync(':memory:')
  db.exec(triggerMigration)

  return makeD1AgentDefinitionTriggerStore(
    new SqliteD1(db) as unknown as D1Database,
  )
}

describe('agent definition cron utility', () => {
  test('computes the next UTC run for UTC and named timezones', () => {
    expect(computeNextCronRunAt({
      afterIso: '2026-07-03T15:07:00.000Z',
      expr: '*/15 * * * *',
      tz: 'UTC',
    })).toBe('2026-07-03T15:15:00.000Z')

    expect(computeNextCronRunAt({
      afterIso: '2026-07-03T15:00:00.000Z',
      expr: '0 9 * * *',
      tz: 'America/Chicago',
    })).toBe('2026-07-04T14:00:00.000Z')

    expect(() => computeNextCronRunAt({
      afterIso: '2026-07-03T15:00:00.000Z',
      expr: 'not a cron',
      tz: 'UTC',
    })).toThrow('five fields')
  })
})

describe('agent definition trigger D1 store', () => {
  test('persists cron and inbound webhook triggers with precomputed state', async () => {
    const store = makeStore()
    const definition = makeDefinition()
    const records = await store.replaceDefinitionTriggers(
      ownerAgentUserId,
      definition,
      '2026-07-03T15:07:00.000Z',
    )
    const cron = records.find(record =>
      record.triggerRef === 'trigger.public.trigger_store.quarter_hour'
    )
    const inboundWebhook = records.find(record =>
      record.triggerRef === 'trigger.public.trigger_store.github_issue'
    )

    expect(records).toHaveLength(2)
    expect(cron).toMatchObject({
      schema: 'openagents.agent_definition_trigger.v1',
      ownerRef: 'agent:agent_user_owner',
      definitionId: definition.id,
      state: 'enabled',
      consecutiveFailures: 0,
      nextRunAt: '2026-07-03T15:15:00.000Z',
      trigger: {
        kind: 'cron',
        expr: '*/15 * * * *',
        tz: 'UTC',
      },
    })
    expect(inboundWebhook).toMatchObject({
      state: 'enabled',
      consecutiveFailures: 0,
      trigger: {
        kind: 'inbound_webhook',
        source: 'github',
        conditions: [
          {
            kind: 'event_type',
            equals: 'issues.opened',
          },
          {
            kind: 'json_path_equals',
            path: '$.repository.full_name',
            equals: 'OpenAgentsInc/openagents',
          },
        ],
      },
    })
    expect(inboundWebhook?.nextRunAt).toBeUndefined()

    const inboundWebhookTriggers = await store.listInboundWebhookTriggers(
      'github',
      10,
    )
    expect(inboundWebhookTriggers).toHaveLength(1)
    expect(inboundWebhookTriggers[0]).toMatchObject({
      ownerAgentUserId,
      triggerRef: 'trigger.public.trigger_store.github_issue',
      trigger: {
        kind: 'inbound_webhook',
        source: 'github',
      },
    })
    expect(await store.listInboundWebhookTriggers('slack', 10)).toEqual([])
  })

  test('preserves state across definition edits and removes stale triggers', async () => {
    const store = makeStore()
    const definition = makeDefinition()
    const triggerRef = 'trigger.public.trigger_store.quarter_hour'

    await store.replaceDefinitionTriggers(
      ownerAgentUserId,
      definition,
      '2026-07-03T15:07:00.000Z',
    )
    expect(await store.pauseTrigger(
      ownerAgentUserId,
      triggerRef,
      '2026-07-03T15:10:00.000Z',
      'operator requested review',
    )).toBe(true)
    expect(await store.recordTriggerFailure(
      ownerAgentUserId,
      triggerRef,
      '2026-07-03T15:11:00.000Z',
    )).toBe(true)

    const editedDefinition = decodeAgentDefinition({
      ...definition,
      triggers: [
        {
          kind: 'cron',
          triggerRef,
          expr: '0 * * * *',
          tz: 'UTC',
        },
      ],
    })
    const records = await store.replaceDefinitionTriggers(
      ownerAgentUserId,
      editedDefinition,
      '2026-07-03T15:12:00.000Z',
    )

    expect(records).toHaveLength(1)
    expect(records[0]).toMatchObject({
      state: 'paused',
      consecutiveFailures: 1,
      nextRunAt: '2026-07-03T16:00:00.000Z',
      pausedAt: '2026-07-03T15:10:00.000Z',
      pauseReason: 'operator requested review',
      trigger: {
        kind: 'cron',
        expr: '0 * * * *',
      },
    })
  })

  test('updates failure, success, enable, and owner-scoped access state', async () => {
    const store = makeStore()
    const definition = makeDefinition()
    const triggerRef = 'trigger.public.trigger_store.quarter_hour'

    await store.replaceDefinitionTriggers(
      ownerAgentUserId,
      definition,
      '2026-07-03T15:07:00.000Z',
    )

    expect(await store.recordTriggerFailure(
      'agent_user_other',
      triggerRef,
      '2026-07-03T15:08:00.000Z',
    )).toBe(false)
    expect(await store.pauseTrigger(
      ownerAgentUserId,
      triggerRef,
      '2026-07-03T15:09:00.000Z',
      'transient failure',
    )).toBe(true)
    expect(await store.recordTriggerSuccess(
      ownerAgentUserId,
      triggerRef,
      '2026-07-03T15:30:00.000Z',
      '2026-07-03T15:20:00.000Z',
    )).toBe(true)
    expect(await store.enableTrigger(
      ownerAgentUserId,
      triggerRef,
      '2026-07-03T15:21:00.000Z',
    )).toBe(true)

    const ownerRecords = await store.listDefinitionTriggers(
      ownerAgentUserId,
      definition.id,
    )
    const ownerCronRecord = ownerRecords.find(record =>
      record.triggerRef === triggerRef
    )
    const otherRecords = await store.listDefinitionTriggers(
      'agent_user_other',
      definition.id,
    )

    expect(ownerCronRecord).toMatchObject({
      state: 'enabled',
      consecutiveFailures: 0,
      nextRunAt: '2026-07-03T15:30:00.000Z',
    })
    expect(ownerCronRecord?.pausedAt).toBeUndefined()
    expect(ownerCronRecord?.pauseReason).toBeUndefined()
    expect(otherRecords).toEqual([])

    expect(await store.listDueCronTriggers(
      '2026-07-03T15:29:59.000Z',
      10,
    )).toEqual([])

    const dueRecords = await store.listDueCronTriggers(
      '2026-07-03T15:30:00.000Z',
      10,
    )
    expect(dueRecords).toHaveLength(1)
    expect(dueRecords[0]).toMatchObject({
      ownerAgentUserId,
      triggerRef,
      nextRunAt: '2026-07-03T15:30:00.000Z',
    })

    expect(await store.recordTriggerDispatchFailure(
      ownerAgentUserId,
      triggerRef,
      '2026-07-03T15:45:00.000Z',
      '2026-07-03T15:30:01.000Z',
    )).toBe(true)
    const failedCronRecord = (
      await store.listDefinitionTriggers(ownerAgentUserId, definition.id)
    ).find(record => record.triggerRef === triggerRef)

    expect(failedCronRecord).toMatchObject({
      consecutiveFailures: 1,
      nextRunAt: '2026-07-03T15:45:00.000Z',
    })
  })
})
