import {
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { fulfillmentLoopAgentDefinitionFixture } from '@openagentsinc/agent-runtime-schema/fixtures'
import { describe, expect, test } from 'vitest'

import {
  type AgentDefinitionSchedulerDependencies,
  runAgentDefinitionSchedulerTick,
} from './agent-definition-scheduler'
import type {
  AgentDefinitionRunDispatchDependencies,
  AgentDefinitionRunDispatchOutcome,
} from './agent-definition-run-routes'
import type { DueAgentDefinitionTriggerRecord } from './agent-definition-trigger-store'

const ownerAgentUserId = 'agent_user_scheduler_owner'
const nowIso = '2026-07-03T15:30:00.000Z'

const makeDefinition = (): AgentDefinition =>
  decodeAgentDefinition({
    ...fulfillmentLoopAgentDefinitionFixture,
    id: 'agent_definition.public.scheduler_test',
    ownerRef: `agent:${ownerAgentUserId}`,
    lane: 'own_pylon',
    triggers: [
      {
        kind: 'cron',
        triggerRef: 'trigger.public.scheduler.quarter_hour',
        expr: '*/15 * * * *',
        tz: 'UTC',
      },
    ],
  })

const dueRecord = (
  input: Readonly<{
    definition: AgentDefinition
    nextRunAt: string
    triggerRef?: string | undefined
  }>,
): DueAgentDefinitionTriggerRecord => {
  const triggerRef =
    input.triggerRef ?? 'trigger.public.scheduler.quarter_hour'
  const trigger = input.definition.triggers.find(candidate =>
    candidate.triggerRef === triggerRef
  )

  return {
    schema: 'openagents.agent_definition_trigger.v1',
    consecutiveFailures: 0,
    createdAt: '2026-07-03T15:00:00.000Z',
    definitionId: input.definition.id,
    nextRunAt: input.nextRunAt,
    ownerAgentUserId,
    ownerRef: input.definition.ownerRef,
    state: 'enabled',
    trigger: trigger ?? {
      kind: 'cron',
      triggerRef,
      expr: '*/15 * * * *',
      tz: 'UTC',
    },
    triggerId: `${input.definition.id}:${triggerRef}`,
    triggerRef,
    updatedAt: '2026-07-03T15:00:00.000Z',
  }
}

class MemoryTriggerStore {
  readonly dispatchFailures: Array<{
    readonly nextRunAt: string
    readonly ownerAgentUserId: string
    readonly triggerRef: string
    readonly updatedAt: string
  }> = []
  readonly failures: Array<{
    readonly ownerAgentUserId: string
    readonly triggerRef: string
    readonly updatedAt: string
  }> = []
  readonly successes: Array<{
    readonly nextRunAt: string | undefined
    readonly ownerAgentUserId: string
    readonly triggerRef: string
    readonly updatedAt: string
  }> = []

  constructor(private readonly rows: ReadonlyArray<DueAgentDefinitionTriggerRecord>) {}

  listDueCronTriggers(
    tickNowIso: string,
    limit: number,
  ): Promise<ReadonlyArray<DueAgentDefinitionTriggerRecord>> {
    return Promise.resolve(
      this.rows
        .filter(record =>
          record.trigger.kind === 'cron' &&
          record.state === 'enabled' &&
          record.nextRunAt !== undefined &&
          record.nextRunAt <= tickNowIso
        )
        .sort((left, right) =>
          (left.nextRunAt ?? '').localeCompare(right.nextRunAt ?? '')
        )
        .slice(0, limit),
    )
  }

  recordTriggerSuccess(
    owner: string,
    triggerRef: string,
    nextRunAt: string | undefined,
    updatedAt: string,
  ): Promise<boolean> {
    this.successes.push({
      nextRunAt,
      ownerAgentUserId: owner,
      triggerRef,
      updatedAt,
    })

    return Promise.resolve(true)
  }

  recordTriggerDispatchFailure(
    owner: string,
    triggerRef: string,
    nextRunAt: string,
    updatedAt: string,
  ): Promise<boolean> {
    this.dispatchFailures.push({
      nextRunAt,
      ownerAgentUserId: owner,
      triggerRef,
      updatedAt,
    })

    return Promise.resolve(true)
  }

  recordTriggerFailure(
    owner: string,
    triggerRef: string,
    updatedAt: string,
  ): Promise<boolean> {
    this.failures.push({
      ownerAgentUserId: owner,
      triggerRef,
      updatedAt,
    })

    return Promise.resolve(true)
  }
}

const schedulerDependencies = (
  input: Readonly<{
    definition: AgentDefinition | undefined
    dispatchRun: AgentDefinitionSchedulerDependencies['dispatchRun']
    triggerStore: MemoryTriggerStore
  }>,
): AgentDefinitionSchedulerDependencies => ({
  definitionStore: {
    readDefinition: (_owner, _definitionId) =>
      Promise.resolve(input.definition),
  },
  dispatchDependencies: {
    forgeStore: {} as never,
    pylonStore: {} as never,
    runStore: {} as never,
  },
  dispatchRun: input.dispatchRun,
  triggerStore: input.triggerStore,
})

const dispatchedOutcome = (): AgentDefinitionRunDispatchOutcome => ({
  assignmentRef: 'assignment.background.scheduler',
  durableStreamUrl: 'https://openagents.com/v1/stream/scheduler',
  kind: 'dispatched',
  record: {} as never,
  seeded: false,
})

describe('agent definition scheduler', () => {
  test('dispatches due cron triggers through the shared run helper under a cap', async () => {
    const definition = makeDefinition()
    const triggerStore = new MemoryTriggerStore([
      dueRecord({
        definition,
        nextRunAt: '2026-07-03T15:15:00.000Z',
      }),
      dueRecord({
        definition,
        nextRunAt: '2026-07-03T15:30:00.000Z',
        triggerRef: 'trigger.public.scheduler.second',
      }),
    ])
    const dispatches: Array<{
      readonly dependencies: AgentDefinitionRunDispatchDependencies
      readonly triggerPayload: Record<string, unknown> | undefined
      readonly triggerRef: string | undefined
    }> = []

    const result = await runAgentDefinitionSchedulerTick(
      schedulerDependencies({
        definition,
        dispatchRun: (dependencies, input) => {
          dispatches.push({
            dependencies,
            triggerPayload: input.request.triggerPayload,
            triggerRef: input.request.triggerRef,
          })

          return Promise.resolve(dispatchedOutcome())
        },
        triggerStore,
      }),
      { limit: 1, nowIso },
    )

    expect(result).toMatchObject({
      backpressureCap: 1,
      backpressureCapHit: true,
      dispatched: 1,
      failed: 0,
      oldestDueAt: '2026-07-03T15:15:00.000Z',
      processed: 1,
      recoverySweepOverdue: 1,
      refused: 0,
      skipped: 0,
    })
    expect(dispatches).toHaveLength(1)
    expect(dispatches[0]?.dependencies.linkedAgents).toEqual([
      { agentUserId: ownerAgentUserId },
    ])
    expect(dispatches[0]?.triggerRef).toBe(
      'trigger.public.scheduler.quarter_hour',
    )
    expect(dispatches[0]?.triggerPayload).toMatchObject({
      dueAt: '2026-07-03T15:15:00.000Z',
      scheduledAt: nowIso,
      schema: 'openagents.background_agent.cron_trigger.v1',
    })
    expect(triggerStore.successes).toEqual([
      {
        nextRunAt: '2026-07-03T15:45:00.000Z',
        ownerAgentUserId,
        triggerRef: 'trigger.public.scheduler.quarter_hour',
        updatedAt: nowIso,
      },
    ])
  })

  test('advances refused dispatches while preserving the failure streak', async () => {
    const definition = makeDefinition()
    const triggerStore = new MemoryTriggerStore([
      dueRecord({
        definition,
        nextRunAt: '2026-07-03T15:30:00.000Z',
      }),
    ])
    const result = await runAgentDefinitionSchedulerTick(
      schedulerDependencies({
        definition,
        dispatchRun: () =>
          Promise.resolve({
            evidenceRefs: ['evidence.no_capacity'],
            error: 'target_pylon_unavailable',
            kind: 'refused',
            reason: 'No owner Pylon was available.',
            record: {} as never,
            requestedPylonRef: null,
            seeded: false,
            statusCode: 503,
          }),
        triggerStore,
      }),
      { nowIso },
    )

    expect(result).toMatchObject({
      dispatched: 0,
      failed: 0,
      processed: 1,
      refused: 1,
    })
    expect(triggerStore.dispatchFailures).toEqual([
      {
        nextRunAt: '2026-07-03T15:45:00.000Z',
        ownerAgentUserId,
        triggerRef: 'trigger.public.scheduler.quarter_hour',
        updatedAt: nowIso,
      },
    ])
  })

  test('records a trigger failure when the owner-scoped definition is missing', async () => {
    const definition = makeDefinition()
    const triggerStore = new MemoryTriggerStore([
      dueRecord({
        definition,
        nextRunAt: '2026-07-03T15:30:00.000Z',
      }),
    ])
    const result = await runAgentDefinitionSchedulerTick(
      schedulerDependencies({
        definition: undefined,
        dispatchRun: () => Promise.resolve(dispatchedOutcome()),
        triggerStore,
      }),
      { nowIso },
    )

    expect(result).toMatchObject({
      dispatched: 0,
      failed: 1,
      processed: 1,
    })
    expect(triggerStore.dispatchFailures).toHaveLength(1)
    expect(triggerStore.successes).toEqual([])
  })
})
