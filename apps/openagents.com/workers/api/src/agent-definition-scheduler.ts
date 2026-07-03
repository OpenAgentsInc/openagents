import type { AgentDefinition } from '@openagentsinc/agent-runtime-schema'

import { computeNextCronRunAt } from './agent-definition-cron'
import {
  type AgentDefinitionStore,
  makeD1AgentDefinitionStore,
} from './agent-definition-routes'
import {
  type AgentDefinitionRunDispatchDependencies,
  type AgentDefinitionRunDispatchOutcome,
  dispatchAgentDefinitionRun,
  makeD1AgentDefinitionRunStore,
} from './agent-definition-run-routes'
import {
  type AgentDefinitionTriggerStore,
  type DueAgentDefinitionTriggerRecord,
  makeD1AgentDefinitionTriggerStore,
} from './agent-definition-trigger-store'
import { makeD1ForgeCoordinationStore } from './forge-coordination-store'
import {
  type DurableStreamNamespace,
} from './inference/durable-inference-do-transport'
import { makeD1PylonApiStore } from './pylon-api'

export const AGENT_DEFINITION_SCHEDULER_SINGLETON_NAME =
  'agent-definition-scheduler'
export const AGENT_DEFINITION_SCHEDULER_DEFAULT_LIMIT = 25

type SchedulerDispatchDependencies = Omit<
  AgentDefinitionRunDispatchDependencies,
  'linkedAgents' | 'nowIso'
>

type SchedulerDispatch = (
  dependencies: AgentDefinitionRunDispatchDependencies,
  input: Readonly<{
    definition: AgentDefinition
    request: Parameters<typeof dispatchAgentDefinitionRun>[1]['request']
  }>,
) => Promise<AgentDefinitionRunDispatchOutcome>

export type AgentDefinitionSchedulerDependencies = Readonly<{
  definitionStore: Pick<AgentDefinitionStore, 'readDefinition'>
  dispatchDependencies: SchedulerDispatchDependencies
  dispatchRun?: SchedulerDispatch | undefined
  triggerStore: Pick<
    AgentDefinitionTriggerStore,
    | 'listDueCronTriggers'
    | 'recordTriggerDispatchFailure'
    | 'recordTriggerFailure'
    | 'recordTriggerSuccess'
  >
}>

export type AgentDefinitionSchedulerTickResult = Readonly<{
  backpressureCap: number
  backpressureCapHit: boolean
  dispatched: number
  failed: number
  oldestDueAt: string | null
  processed: number
  recoverySweepOverdue: number
  refused: number
  scheduledAt: string
  skipped: number
}>

const clampLimit = (limit: number | undefined): number =>
  Math.max(
    1,
    Math.min(limit ?? AGENT_DEFINITION_SCHEDULER_DEFAULT_LIMIT, 100),
  )

const cronTriggerNextRunAt = (
  record: DueAgentDefinitionTriggerRecord,
  nowIso: string,
): string | undefined => {
  if (record.trigger.kind !== 'cron') {
    return undefined
  }

  try {
    return computeNextCronRunAt({
      afterIso: nowIso,
      expr: record.trigger.expr,
      tz: record.trigger.tz,
    })
  } catch {
    return undefined
  }
}

const markFailure = async (
  triggerStore: AgentDefinitionSchedulerDependencies['triggerStore'],
  record: DueAgentDefinitionTriggerRecord,
  nextRunAt: string | undefined,
  nowIso: string,
): Promise<boolean> =>
  nextRunAt === undefined
    ? triggerStore.recordTriggerFailure(
        record.ownerAgentUserId,
        record.triggerRef,
        nowIso,
      )
    : triggerStore.recordTriggerDispatchFailure(
        record.ownerAgentUserId,
        record.triggerRef,
        nextRunAt,
        nowIso,
      )

const cronTriggerPayload = (
  record: DueAgentDefinitionTriggerRecord,
  nowIso: string,
): Record<string, unknown> => ({
  schema: 'openagents.background_agent.cron_trigger.v1',
  dueAt: record.nextRunAt ?? nowIso,
  scheduledAt: nowIso,
  triggerId: record.triggerId,
  triggerRef: record.triggerRef,
})

export const runAgentDefinitionSchedulerTick = async (
  dependencies: AgentDefinitionSchedulerDependencies,
  input: Readonly<{
    limit?: number | undefined
    nowIso: string
  }>,
): Promise<AgentDefinitionSchedulerTickResult> => {
  const limit = clampLimit(input.limit)
  const fetchedDue = await dependencies.triggerStore.listDueCronTriggers(
    input.nowIso,
    limit >= 100 ? limit : limit + 1,
  )
  const due = fetchedDue.slice(0, limit)
  const oldestDueAt = due[0]?.nextRunAt ?? null
  let dispatched = 0
  let failed = 0
  let refused = 0
  let skipped = 0

  for (const record of due) {
    const nextRunAt = cronTriggerNextRunAt(record, input.nowIso)

    if (record.trigger.kind !== 'cron' || nextRunAt === undefined) {
      skipped += 1
      try {
        const marked = await markFailure(
          dependencies.triggerStore,
          record,
          nextRunAt,
          input.nowIso,
        )
        if (!marked) failed += 1
      } catch {
        failed += 1
      }
      continue
    }

    const definition = await dependencies.definitionStore
      .readDefinition(record.ownerAgentUserId, record.definitionId)
      .catch(() => undefined)

    if (definition === undefined) {
      failed += 1
      await markFailure(
        dependencies.triggerStore,
        record,
        nextRunAt,
        input.nowIso,
      ).catch(() => undefined)
      continue
    }

    const dispatch = await (
      dependencies.dispatchRun ?? dispatchAgentDefinitionRun
    )({
      ...dependencies.dispatchDependencies,
      linkedAgents: [{ agentUserId: record.ownerAgentUserId }],
      nowIso: () => input.nowIso,
    }, {
      definition,
      request: {
        triggerPayload: cronTriggerPayload(record, input.nowIso),
        triggerRef: record.triggerRef,
      },
    }).catch((): AgentDefinitionRunDispatchOutcome => ({ kind: 'storage_error' }))

    if (dispatch.kind === 'dispatched') {
      const marked = await dependencies.triggerStore
        .recordTriggerSuccess(
          record.ownerAgentUserId,
          record.triggerRef,
          nextRunAt,
          input.nowIso,
        )
        .catch(() => false)
      if (marked) {
        dispatched += 1
      } else {
        failed += 1
      }
      continue
    }

    const marked = await markFailure(
      dependencies.triggerStore,
      record,
      nextRunAt,
      input.nowIso,
    ).catch(() => false)

    if (!marked) {
      failed += 1
    } else if (dispatch.kind === 'refused') {
      refused += 1
    } else {
      failed += 1
    }
  }

  return {
    backpressureCap: limit,
    backpressureCapHit: fetchedDue.length > due.length,
    dispatched,
    failed,
    oldestDueAt,
    processed: due.length,
    recoverySweepOverdue: due.filter(record =>
      record.nextRunAt === undefined ? false : record.nextRunAt < input.nowIso
    ).length,
    refused,
    scheduledAt: input.nowIso,
    skipped,
  }
}

export const makeAgentDefinitionSchedulerDependencies = (
  input: Readonly<{
    db: D1Database
    durableStreamNamespace?: DurableStreamNamespace | undefined
  }>,
): AgentDefinitionSchedulerDependencies => {
  return {
    definitionStore: makeD1AgentDefinitionStore(input.db),
    dispatchDependencies: {
      durableStreamNamespace: input.durableStreamNamespace,
      forgeStore: makeD1ForgeCoordinationStore(input.db),
      pylonStore: makeD1PylonApiStore(input.db),
      runStore: makeD1AgentDefinitionRunStore(input.db),
    },
    triggerStore: makeD1AgentDefinitionTriggerStore(input.db),
  }
}
