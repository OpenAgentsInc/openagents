import {
  assertAgentRuntimePublicEventSafe,
  decodeAgentRuntimeEvent,
  projectAgentRuntimeSurfaceStatus,
  type AgentRuntimeEvent as AgentRuntimeEventShape,
  type AgentRuntimeRunId,
  type AgentRuntimeSurfaceStatusRow,
} from '@openagentsinc/agent-runtime-schema'
import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  rebuiltOnTransitionStaleness,
} from './public-projection-staleness'

export const AGENT_RUNTIME_PUBLIC_PROJECTION_STALENESS =
  rebuiltOnTransitionStaleness(0, [
    'agent_runtime_event_ingested',
    'agent_runtime_run_state_transition',
  ])

export const AgentRuntimePublicRunProjection = S.Struct({
  schema: S.Literal('openagents.agent_runtime.public_run_projection.v1'),
  runId: S.String,
  state: S.Literals([
    'running',
    'paused',
    'interrupted',
    'cancelled',
    'completed',
    'failed',
  ]),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  eventCount: S.Number,
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  latestEventId: S.optional(S.String),
  authority: S.Struct({
    acceptedWorkAuthority: S.Literal(false),
    payoutAuthority: S.Literal(false),
    publicClaimAuthority: S.Literal(false),
  }),
  visibilitySplit: S.Struct({
    storedEventVisibilities: S.Array(S.String),
    projectedVisibility: S.Literal('public'),
  }),
})
export type AgentRuntimePublicRunProjection =
  typeof AgentRuntimePublicRunProjection.Type

export type AgentRuntimeIngestionResult = {
  readonly accepted: boolean
  readonly runId: AgentRuntimeRunId
  readonly eventId: string
  readonly sequence: number
}

export interface AgentRuntimeEventRepository {
  append(event: AgentRuntimeEventShape): Promise<void>
  eventsForRun(runId: AgentRuntimeRunId): Promise<ReadonlyArray<AgentRuntimeEventShape>>
}

export class AgentRuntimeIngestionError extends Error {
  override readonly name = 'AgentRuntimeIngestionError'
}

export class MemoryAgentRuntimeEventRepository implements AgentRuntimeEventRepository {
  readonly eventsByRun = new Map<AgentRuntimeRunId, ReadonlyArray<AgentRuntimeEventShape>>()

  async append(event: AgentRuntimeEventShape): Promise<void> {
    const previous = this.eventsByRun.get(event.runId) ?? []
    this.eventsByRun.set(event.runId, [...previous, event])
  }

  async eventsForRun(runId: AgentRuntimeRunId): Promise<ReadonlyArray<AgentRuntimeEventShape>> {
    return this.eventsByRun.get(runId) ?? []
  }
}

export function decodeAgentRuntimeIngressEvent(value: unknown): AgentRuntimeEventShape {
  const event = decodeAgentRuntimeEvent(value)
  return assertAgentRuntimePublicEventSafe(event)
}

export async function ingestAgentRuntimeEvent(
  repository: AgentRuntimeEventRepository,
  value: unknown,
): Promise<AgentRuntimeIngestionResult> {
  const event = decodeAgentRuntimeIngressEvent(value)
  const existing = await repository.eventsForRun(event.runId)
  const duplicate = existing.some(candidate => candidate.eventId === event.eventId)

  if (duplicate) {
    throw new AgentRuntimeIngestionError('Agent runtime event is already persisted.')
  }

  const expectedSequence = existing.length + 1

  if (event.sequence !== expectedSequence) {
    throw new AgentRuntimeIngestionError(
      `Agent runtime event sequence must append at ${expectedSequence}.`,
    )
  }

  await repository.append(event)

  return {
    accepted: true,
    runId: event.runId,
    eventId: event.eventId,
    sequence: event.sequence,
  }
}

export async function ingestAgentRuntimeEvents(
  repository: AgentRuntimeEventRepository,
  events: ReadonlyArray<unknown>,
): Promise<ReadonlyArray<AgentRuntimeIngestionResult>> {
  const results: Array<AgentRuntimeIngestionResult> = []

  for (const event of events) {
    results.push(await ingestAgentRuntimeEvent(repository, event))
  }

  return results
}

export async function projectPublicAgentRuntimeRun(
  repository: AgentRuntimeEventRepository,
  runId: AgentRuntimeRunId,
  nowIso: string,
): Promise<AgentRuntimePublicRunProjection> {
  const events = await repository.eventsForRun(runId)
  const publicEvents = events.filter(event => event.visibility === 'public')
  const artifactRefs = new Set<string>()
  const blockerRefs = new Set<string>()
  const storedEventVisibilities = new Set<string>()
  let state: AgentRuntimePublicRunProjection['state'] = 'running'
  let latestEventId: string | undefined

  for (const event of events) {
    storedEventVisibilities.add(event.visibility)
  }

  for (const event of publicEvents) {
    latestEventId = event.eventId
    for (const blockerRef of event.blockerRefs) {
      blockerRefs.add(blockerRef)
    }
    if (event.artifact?.artifactRef !== undefined) {
      artifactRefs.add(event.artifact.artifactRef)
    }
    for (const artifactRef of event.externalInvocation?.artifactRefs ?? []) {
      artifactRefs.add(artifactRef)
    }
    if (event.tag === 'run.paused') {
      state = 'paused'
    }
    if (event.tag === 'run.interrupted') {
      state = 'interrupted'
    }
    if (event.tag === 'run.cancelled') {
      state = 'cancelled'
    }
    if (event.tag === 'run.completed') {
      state = 'completed'
    }
    if (event.tag === 'run.failed') {
      state = 'failed'
    }
  }

  const projection: AgentRuntimePublicRunProjection = {
    schema: 'openagents.agent_runtime.public_run_projection.v1',
    runId,
    state,
    generatedAt: nowIso,
    staleness: AGENT_RUNTIME_PUBLIC_PROJECTION_STALENESS,
    eventCount: publicEvents.length,
    artifactRefs: [...artifactRefs],
    blockerRefs: [...blockerRefs],
    ...(latestEventId === undefined ? {} : { latestEventId }),
    authority: {
      acceptedWorkAuthority: false,
      payoutAuthority: false,
      publicClaimAuthority: false,
    },
    visibilitySplit: {
      storedEventVisibilities: [...storedEventVisibilities],
      projectedVisibility: 'public',
    },
  }

  return S.decodeUnknownSync(AgentRuntimePublicRunProjection)(projection)
}

export const agentRuntimeProjectionHasPrivateMaterial = (
  projection: AgentRuntimePublicRunProjection,
): boolean => /raw[_-]?(prompt|log|provider|payload)|\/Users\/|\/home\/|secret|bearer|sk-[a-z0-9]|provider[_-]?payload/i
  .test(JSON.stringify(projection))

export const projectAgentRuntimeWorkroomStatus = (
  projection: AgentRuntimePublicRunProjection,
): AgentRuntimeSurfaceStatusRow =>
  projectAgentRuntimeSurfaceStatus(
    {
      runId: projection.runId,
      state: projection.state,
      generatedAt: projection.generatedAt,
      eventCount: projection.eventCount,
      artifactRefs: projection.artifactRefs,
      blockerRefs: projection.blockerRefs,
      ...(projection.latestEventId === undefined
        ? {}
        : { latestEventId: projection.latestEventId }),
      staleness: {
        maxStalenessSeconds: projection.staleness.maxStalenessSeconds,
        transitionRefs: projection.staleness.rebuildsOn,
      },
    },
  )
