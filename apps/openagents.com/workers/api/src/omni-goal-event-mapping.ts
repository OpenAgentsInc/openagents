import { extractAutopilotTokenUsageFromJson } from '@openagentsinc/sync-schema'

import {
  AgentGoalRuntimeEvent,
  type AgentGoalRuntimeEventType,
} from './agent-goal-runtime'
import type { OmniEventRecord } from './omni-runs'

const tokenUsageFromEvent = (event: OmniEventRecord) =>
  extractAutopilotTokenUsageFromJson(event.payloadJson)

const sourceRefForTokenUsageEvent = (event: OmniEventRecord): string =>
  event.externalEventId ?? `${event.parentId}:${event.sequence}`

const goalRuntimeEventTypeFromRunEvent = (
  event: OmniEventRecord,
): AgentGoalRuntimeEventType | undefined => {
  const type = event.type.toLowerCase()
  const status = (event.status ?? '').toLowerCase()

  if (status === 'failed' || type.includes('failed')) {
    return type.includes('usage') || type.includes('provider')
      ? 'UsageLimitReached'
      : 'RunFailed'
  }

  if (status === 'completed' || type.includes('completed')) {
    return 'RunCompleted'
  }

  if (tokenUsageFromEvent(event) !== undefined) {
    return 'UsageAccounted'
  }

  if (type === 'agent_run.accepted') {
    return 'RunAccepted'
  }

  if (
    status === 'running' ||
    type.includes('started') ||
    type.includes('start') ||
    type.includes('dispatched')
  ) {
    return 'RunStarted'
  }

  if (
    event.artifactRefs.length > 0 ||
    type.includes('artifact') ||
    type.includes('file') ||
    type.includes('result')
  ) {
    return 'ArtifactPublished'
  }

  return type.includes('tool') ||
    type.includes('shell') ||
    type.includes('command')
    ? 'ToolCompleted'
    : undefined
}

const externalGoalEventId = (
  event: OmniEventRecord,
  eventType: AgentGoalRuntimeEventType,
): string =>
  eventType === 'UsageAccounted'
    ? `usage:${sourceRefForTokenUsageEvent(event)}`
    : (event.externalEventId ??
      `${event.parentId}:${event.sequence}:${event.type}`)

export const goalRuntimeEventFromRunEvent = (
  goalId: string | null,
  event: OmniEventRecord,
): AgentGoalRuntimeEvent | undefined => {
  if (goalId === null) {
    return undefined
  }

  const eventType = goalRuntimeEventTypeFromRunEvent(event)

  if (eventType === undefined) {
    return undefined
  }

  const usage = tokenUsageFromEvent(event)

  return new AgentGoalRuntimeEvent({
    type: eventType,
    goalId,
    expectedGoalId: goalId,
    runId: event.parentId,
    externalEventId: externalGoalEventId(event, eventType),
    ...(usage === undefined ? {} : { tokenDelta: usage.totalTokens }),
    payload: {
      artifactRefs: event.artifactRefs,
      eventId: event.id,
      source: event.source,
      status: event.status,
      summary: event.summary,
      type: event.type,
    },
  })
}

export const goalRuntimeEventFromRunStatus = (
  goalId: string | null,
  runId: string,
  status: string | undefined,
  events: ReadonlyArray<OmniEventRecord>,
  timeDeltaSeconds?: number | undefined,
): AgentGoalRuntimeEvent | undefined => {
  if (goalId === null || status === undefined) {
    return undefined
  }

  const eventType =
    status === 'running'
      ? 'RunStarted'
      : status === 'completed'
        ? 'RunCompleted'
        : status === 'failed' || status === 'canceled'
          ? 'RunFailed'
          : undefined

  if (eventType === undefined) {
    return undefined
  }

  const maxSequence = Math.max(0, ...events.map(event => event.sequence))

  return new AgentGoalRuntimeEvent({
    type: eventType,
    goalId,
    expectedGoalId: goalId,
    runId,
    externalEventId: `run-status:${runId}:${status}:${maxSequence}`,
    ...(timeDeltaSeconds === undefined ? {} : { timeDeltaSeconds }),
    payload: {
      eventCount: events.length,
      status,
    },
  })
}
