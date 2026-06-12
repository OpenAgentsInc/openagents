import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Context, Effect, Layer, Schema as S } from 'effect'

import {
  AgentGoalAccessService,
  AgentGoalPublicProjectionUnsafe,
  AgentGoalRepository,
  PublicAgentGoalRecord,
  type AgentGoalError,
} from './agent-goals'
import {
  AgentGoalEventRecord,
  AgentGoalEventRepository,
} from './agent-goal-runtime'
import { parseJsonRecord, stringArrayFromUnknown } from './json-boundary'

export class PublicAgentGoalStreamEvent extends S.Class<PublicAgentGoalStreamEvent>(
  'PublicAgentGoalStreamEvent',
)({
  id: S.String,
  goalId: S.String,
  runId: S.NullOr(S.String),
  type: S.String,
  status: S.NullOr(S.String),
  summary: S.String,
  tokenDelta: S.Int,
  timeDeltaSeconds: S.Int,
  artifactRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  commitRefs: S.Array(S.String),
  createdAt: S.String,
}) {}

export class PublicAgentGoalSnapshot extends S.Class<PublicAgentGoalSnapshot>(
  'PublicAgentGoalSnapshot',
)({
  agentId: S.String,
  goal: S.NullOr(PublicAgentGoalRecord),
  events: S.Array(PublicAgentGoalStreamEvent),
}) {}

export type AgentPublicProjectionServiceShape = Readonly<{
  currentAgentGoal: (
    agentId: string,
  ) => Effect.Effect<PublicAgentGoalSnapshot, AgentGoalError>
  goalSnapshot: (
    goalId: string,
  ) => Effect.Effect<PublicAgentGoalSnapshot, AgentGoalError>
}>

export class AgentPublicProjectionService extends Context.Service<
  AgentPublicProjectionService,
  AgentPublicProjectionServiceShape
>()('@openagentsinc/AgentPublicProjectionService') {}

const eventSummary = (event: AgentGoalEventRecord): string => {
  if (event.eventType === 'GoalCreated') {
    return 'Goal created.'
  }

  if (event.eventType === 'ExternalSet') {
    return 'Goal updated.'
  }

  if (event.eventType === 'ExternalClear') {
    return 'Goal cleared.'
  }

  if (event.eventType === 'RunAccepted') {
    return 'Run accepted.'
  }

  if (event.eventType === 'RunStarted') {
    return 'Run started.'
  }

  if (event.eventType === 'RunCompleted') {
    return 'Run completed.'
  }

  if (event.eventType === 'RunFailed') {
    return 'Run failed.'
  }

  if (event.eventType === 'UsageAccounted') {
    return 'Usage accounted.'
  }

  if (event.eventType === 'BudgetLimitReached') {
    return 'Budget limit reached.'
  }

  if (event.eventType === 'UsageLimitReached') {
    return 'Usage limit reached.'
  }

  if (event.eventType === 'WorkerResumed') {
    return 'Continuation scheduled.'
  }

  if (event.eventType === 'ArtifactPublished') {
    return 'Artifact published.'
  }

  if (event.eventType === 'CheckpointPersisted') {
    return 'Checkpoint saved.'
  }

  if (event.eventType === 'ToolCompleted') {
    return 'Agent action completed.'
  }

  return 'Goal activity recorded.'
}

const safePublicRefPattern =
  /^(?:[a-f0-9]{7,64}|sha256:[a-f0-9]{16,64}|artifact_[a-z0-9_-]{1,96}|receipt_[a-z0-9_-]{1,96}|https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\/(?:commit|pull)\/[A-Za-z0-9_.\/-]{1,96})$/

const unsafeRefPattern =
  /(?:token|secret|credential|callback|provider|grant|payload|prompt|private|auth|bearer|key)/i

const safePublicRef = (value: string): string | undefined => {
  const text = value.trim()

  if (text === '' || text.length > 180 || unsafeRefPattern.test(text)) {
    return undefined
  }

  return safePublicRefPattern.test(text) ? text : undefined
}

const safePublicRefs = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  values.flatMap(value => {
    const ref = safePublicRef(value)

    return ref === undefined ? [] : [ref]
  })

const publicEventMetadata = (event: AgentGoalEventRecord) => {
  const payload = parseJsonRecord(event.payloadJson)

  return {
    artifactRefs: safePublicRefs(stringArrayFromUnknown(payload?.artifactRefs)),
    receiptRefs: safePublicRefs(stringArrayFromUnknown(payload?.receiptRefs)),
    commitRefs: safePublicRefs(stringArrayFromUnknown(payload?.commitRefs)),
  }
}

export const publicAgentGoalEventFromRecord = (
  event: AgentGoalEventRecord,
): Effect.Effect<
  PublicAgentGoalStreamEvent,
  AgentGoalPublicProjectionUnsafe
> => {
  const metadata = publicEventMetadata(event)
  const projected = new PublicAgentGoalStreamEvent({
    id: event.id,
    goalId: event.goalId,
    runId: event.runId,
    type: event.eventType,
    status: event.status,
    summary: eventSummary(event),
    tokenDelta: event.tokenDelta,
    timeDeltaSeconds: event.timeDeltaSeconds,
    artifactRefs: metadata.artifactRefs,
    receiptRefs: metadata.receiptRefs,
    commitRefs: metadata.commitRefs,
    createdAt: event.createdAt,
  })

  return containsProviderSecretMaterial(JSON.stringify(projected))
    ? Effect.fail(new AgentGoalPublicProjectionUnsafe({ goalId: event.goalId }))
    : Effect.succeed(projected)
}

export const makeAgentPublicProjectionService = (
  dependencies: Readonly<{
    access: typeof AgentGoalAccessService.Service
    events: typeof AgentGoalEventRepository.Service
    repository: typeof AgentGoalRepository.Service
  }>,
): AgentPublicProjectionServiceShape => {
  const snapshotForGoal = Effect.fn(
    'AgentPublicProjectionService.snapshotForGoal',
  )(function* (goal: PublicAgentGoalRecord) {
    const events = yield* dependencies.events.listByGoal(goal.id)
    const publicEvents = yield* Effect.forEach(
      events,
      publicAgentGoalEventFromRecord,
    )

    return new PublicAgentGoalSnapshot({
      agentId: goal.agentId,
      goal,
      events: publicEvents,
    })
  })

  return {
    currentAgentGoal: Effect.fn(
      'AgentPublicProjectionService.currentAgentGoal',
    )(function* (agentId: string) {
      const goal = yield* dependencies.repository.getPublicCurrentByAgentId(
        agentId,
      )

      if (goal === undefined) {
        return new PublicAgentGoalSnapshot({
          agentId,
          goal: null,
          events: [],
        })
      }

      return yield* snapshotForGoal(
        yield* dependencies.access.publicProjection(goal),
      )
    }),
    goalSnapshot: Effect.fn('AgentPublicProjectionService.goalSnapshot')(
      function* (goalId: string) {
        const goal = yield* dependencies.repository.getById(goalId)
        const publicGoal = yield* dependencies.access.publicProjection(goal)

        return yield* snapshotForGoal(publicGoal)
      },
    ),
  }
}

export const AgentPublicProjectionServiceLive = Layer.effect(
  AgentPublicProjectionService,
  Effect.gen(function* () {
    const access = yield* AgentGoalAccessService
    const events = yield* AgentGoalEventRepository
    const repository = yield* AgentGoalRepository

    return makeAgentPublicProjectionService({ access, events, repository })
  }),
)
