import {
  SyncPatch,
  SyncSnapshot,
  extractAutopilotTokenUsageFromJson,
} from '@openagentsinc/sync-schema'

import {
  ActiveChatRun,
  type AgentGoalApiGoal,
  type AgentGoalStatus,
  type AgentRunApiEvent,
  type AgentRunApiRepository,
  type AgentRunApiRun,
  type AgentRunDetailResponse,
  type AgentRunLaunchResponse,
  type AgentRunStatus,
  type ChatMessage,
  type ChatRun,
  type ChatRunEvent,
  type ChatRunMetadata,
  type SidebarSessionItem,
  SyncClientModel,
  type SyncClientModel as SyncClientModelType,
  type TeamChatPostResponse,
  agentRunExternalRefFromNullable,
  optionFromNullableString,
} from '../model'

type AgentRunResponseWithOptionalUrls =
  | AgentRunLaunchResponse
  | AgentRunDetailResponse

type ActiveChatRunModel = Extract<ChatRun, { readonly _tag: 'Active' }>

const recordFromUnknown = (
  value: unknown,
): Record<string, unknown> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value))
    : undefined

const textFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const numberFromUnknown = (value: unknown): number => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value))
  }

  if (typeof value === 'string' && value.trim() !== '') {
    const parsed = Number(value)

    return Number.isFinite(parsed) ? Math.max(0, Math.trunc(parsed)) : 0
  }

  return 0
}

const stringArrayFromUnknown = (value: unknown): ReadonlyArray<string> =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : []

const agentRunStatusFromUnknown = (
  value: unknown,
): AgentRunStatus | undefined =>
  value === 'queued' ||
  value === 'running' ||
  value === 'waiting_for_input' ||
  value === 'completed' ||
  value === 'failed' ||
  value === 'canceled'
    ? value
    : undefined

const agentGoalStatusFromUnknown = (
  value: unknown,
): AgentGoalStatus | undefined => {
  if (value === 'complete') {
    return 'completed'
  }

  return value === 'pending' ||
    value === 'active' ||
    value === 'paused' ||
    value === 'blocked' ||
    value === 'completed' ||
    value === 'failed' ||
    value === 'usage_limited' ||
    value === 'budget_limited' ||
    value === 'archived'
    ? value
    : undefined
}

const agentGoalFromSyncRecord = (
  value: unknown,
): AgentGoalApiGoal | undefined => {
  const record = recordFromUnknown(value)

  if (record === undefined) {
    return undefined
  }

  const id = textFromUnknown(record.id)
  const agentId = textFromUnknown(record.agentId)
  const objective = textFromUnknown(record.objective)
  const status = agentGoalStatusFromUnknown(record.status)
  const visibility = textFromUnknown(record.visibility)
  const createdAt = textFromUnknown(record.createdAt)
  const updatedAt = textFromUnknown(record.updatedAt)

  if (
    id === undefined ||
    agentId === undefined ||
    objective === undefined ||
    status === undefined ||
    (visibility !== 'private' &&
      visibility !== 'team' &&
      visibility !== 'public') ||
    createdAt === undefined ||
    updatedAt === undefined
  ) {
    return undefined
  }

  return {
    id,
    agentId,
    userId: textFromUnknown(record.userId) ?? null,
    teamId: textFromUnknown(record.teamId) ?? null,
    projectId: textFromUnknown(record.projectId) ?? null,
    objective,
    status,
    visibility,
    currentRunId: textFromUnknown(record.currentRunId) ?? null,
    tokenBudget:
      record.tokenBudget === null
        ? null
        : numberFromUnknown(record.tokenBudget),
    tokensUsed: numberFromUnknown(record.tokensUsed),
    timeUsedSeconds: numberFromUnknown(record.timeUsedSeconds),
    remainingTokens:
      record.remainingTokens === null
        ? null
        : numberFromUnknown(record.remainingTokens),
    createdAt,
    updatedAt,
    completedAt: textFromUnknown(record.completedAt) ?? null,
    pausedAt: textFromUnknown(record.pausedAt) ?? null,
    blockedAt: textFromUnknown(record.blockedAt) ?? null,
    canEdit: record.canEdit === true,
    canPause: record.canPause === true,
    canResume: record.canResume === true,
    canMakePublic: record.canMakePublic === true,
    publicUrl: textFromUnknown(record.publicUrl) ?? null,
  }
}

export const tokenUsageFromPayloadJson = extractAutopilotTokenUsageFromJson

const legacyAgentRunUuid = (runId: string): string | undefined => {
  const match = /^agent_run_([0-9a-fA-F]{32})$/.exec(runId)
  const hex = match?.[1]?.toLowerCase()

  return hex === undefined
    ? undefined
    : `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

export const displayRunId = (runId: string): string =>
  legacyAgentRunUuid(runId) ?? runId

export const chatMessageFromRunResponse = (
  response: AgentRunResponseWithOptionalUrls,
  label: string,
): ChatMessage => ({
  author: 'user',
  body: response.run.goal,
  id: `run-goal-${response.run.id}`,
  label,
  status: 'complete',
})

export const runAuthorLabel = (
  response: AgentRunResponseWithOptionalUrls,
  session: { readonly name: string; readonly userId: string },
  teams: ReadonlyArray<{
    readonly id: string
    readonly members: ReadonlyArray<{
      readonly name: string
      readonly userId: string
    }>
  }>,
): string => {
  const runUserId = response.run.userId

  if (
    runUserId === null ||
    runUserId === undefined ||
    runUserId === session.userId
  ) {
    return session.name
  }

  const matchingTeams =
    response.run.teamId === null || response.run.teamId === undefined
      ? teams
      : teams.filter(team => team.id === response.run.teamId)
  const member = matchingTeams
    .flatMap(team => team.members)
    .find(member => member.userId === runUserId)

  return member?.name ?? runUserId
}

export const normalizeEvent = (event: AgentRunApiEvent): ChatRunEvent => {
  const tokenUsage = tokenUsageFromPayloadJson(event.payloadJson)

  return {
    artifactRefs: event.artifactRefs,
    createdAt: event.createdAt,
    externalEventId: optionFromNullableString(event.externalEventId),
    id: event.id,
    payloadJson: optionFromNullableString(event.payloadJson),
    sequence: event.sequence,
    source: event.source,
    status: optionFromNullableString(event.status),
    summary: event.summary,
    tokenModel: optionFromNullableString(tokenUsage?.model),
    tokenProvider: optionFromNullableString(tokenUsage?.provider),
    tokenTotal: tokenUsage?.totalTokens ?? 0,
    type: event.type,
  }
}

const metadataFromResponse = (
  response: AgentRunResponseWithOptionalUrls,
  previous: ChatRunMetadata | undefined,
): ChatRunMetadata => {
  const events = response.events.map(normalizeEvent)
  const tokenEvents = events.filter(event => event.tokenTotal > 0)

  return {
    backend: response.run.backend,
    createdAt: response.run.createdAt,
    displayRunId: displayRunId(response.run.id),
    eventCursor: response.run.eventCursor,
    externalRunRef: agentRunExternalRefFromNullable(response.run.externalRunId),
    goal: response.run.goal,
    repository: `${response.run.repository.owner}/${response.run.repository.repo}@${response.run.repository.ref}`,
    runId: response.run.id,
    runnerId: response.run.runnerId,
    runtime: response.run.runtime,
    status: response.run.status,
    statusUrl:
      'statusUrl' in response
        ? response.statusUrl
        : (previous?.statusUrl ??
          `/api/omni/agent-runs/${encodeURIComponent(displayRunId(response.run.id))}`),
    streamUrl:
      'streamUrl' in response
        ? response.streamUrl
        : (previous?.streamUrl ??
          `/api/omni/agent-runs/${encodeURIComponent(displayRunId(response.run.id))}/events`),
    tokenTotal: tokenEvents.reduce(
      (total, event) => total + event.tokenTotal,
      0,
    ),
    tokenUsageEvents: tokenEvents.length,
    updatedAt: response.run.updatedAt,
  }
}

export const activeChatRunFromResponse = (
  response: AgentRunResponseWithOptionalUrls,
  previous: ChatRunMetadata | undefined = undefined,
) =>
  ActiveChatRun({
    events: response.events.map(normalizeEvent),
    metadata: metadataFromResponse(response, previous),
  })

const tokenTotal = (event: ChatRunEvent | undefined): number =>
  event?.tokenTotal ?? 0

const tokenEventCount = (event: ChatRunEvent | undefined): number =>
  event !== undefined && event.tokenTotal > 0 ? 1 : 0

const metadataWithEventDelta = (
  metadata: ChatRunMetadata,
  oldEvent: ChatRunEvent | undefined,
  nextEvent: ChatRunEvent | undefined,
  updatedAt: string,
  eventCursor: number,
): ChatRunMetadata => ({
  ...metadata,
  eventCursor: Math.max(metadata.eventCursor, eventCursor),
  tokenTotal: Math.max(
    0,
    metadata.tokenTotal + tokenTotal(nextEvent) - tokenTotal(oldEvent),
  ),
  tokenUsageEvents: Math.max(
    0,
    metadata.tokenUsageEvents +
      tokenEventCount(nextEvent) -
      tokenEventCount(oldEvent),
  ),
  updatedAt,
})

const activeChatRunWithoutEvent = (
  chatRun: ActiveChatRunModel,
  eventId: string,
  updatedAt: string,
): ActiveChatRunModel => {
  const oldEvent = chatRun.events.find(event => event.id === eventId)

  if (oldEvent === undefined) {
    return ActiveChatRun({
      events: chatRun.events,
      metadata: {
        ...chatRun.metadata,
        updatedAt,
      },
    })
  }

  return ActiveChatRun({
    events: chatRun.events.filter(event => event.id !== eventId),
    metadata: metadataWithEventDelta(
      chatRun.metadata,
      oldEvent,
      undefined,
      updatedAt,
      oldEvent.sequence,
    ),
  })
}

const activeChatRunWithEvent = (
  chatRun: ActiveChatRunModel,
  nextEvent: ChatRunEvent,
): ActiveChatRunModel => {
  const oldEventIndex = chatRun.events.findIndex(
    event => event.id === nextEvent.id,
  )
  const oldEvent =
    oldEventIndex === -1 ? undefined : chatRun.events[oldEventIndex]
  const mergedEvents =
    oldEventIndex === -1
      ? [...chatRun.events, nextEvent]
      : chatRun.events.map(event =>
          event.id === nextEvent.id ? nextEvent : event,
        )
  const lastEvent = chatRun.events[chatRun.events.length - 1]
  const appendIsOrdered =
    oldEventIndex === -1 &&
    (lastEvent === undefined || nextEvent.sequence >= lastEvent.sequence)
  const replacementKeepsOrder =
    oldEvent !== undefined && oldEvent.sequence === nextEvent.sequence
  const events =
    appendIsOrdered || replacementKeepsOrder
      ? mergedEvents
      : [...mergedEvents].sort((left, right) => left.sequence - right.sequence)

  return ActiveChatRun({
    events,
    metadata: metadataWithEventDelta(
      chatRun.metadata,
      oldEvent,
      nextEvent,
      nextEvent.createdAt,
      nextEvent.sequence,
    ),
  })
}

export const activeChatRunWithSyncedEventPatch = (
  chatRun: ActiveChatRunModel,
  patch: SyncPatch,
  value: unknown,
): ActiveChatRunModel | undefined => {
  if (patch.op === 'delete' || patch.op === 'invalidate') {
    return activeChatRunWithoutEvent(chatRun, patch.id, patch.serverTime)
  }

  const event = agentRunEventFromSyncRecord(value)

  if (event === undefined) {
    return undefined
  }

  if (
    event.parentId !== chatRun.metadata.runId &&
    event.parentId !== chatRun.metadata.displayRunId
  ) {
    return activeChatRunWithoutEvent(chatRun, patch.id, event.createdAt)
  }

  return activeChatRunWithEvent(chatRun, normalizeEvent(event))
}

export const activeChatRunWithSyncedRunPatch = (
  chatRun: ActiveChatRunModel,
  patch: SyncPatch,
  value: unknown,
): ActiveChatRunModel | undefined => {
  if (patch.op === 'delete' || patch.op === 'invalidate') {
    return undefined
  }

  const run = agentRunFromSyncRecord(value)

  if (
    run === undefined ||
    (run.id !== chatRun.metadata.runId &&
      displayRunId(run.id) !== chatRun.metadata.displayRunId)
  ) {
    return undefined
  }

  return ActiveChatRun({
    events: chatRun.events,
    metadata: {
      ...chatRun.metadata,
      backend: run.backend,
      createdAt: run.createdAt,
      displayRunId: displayRunId(run.id),
      eventCursor: run.eventCursor,
      externalRunRef: agentRunExternalRefFromNullable(run.externalRunId),
      goal: run.goal,
      repository: `${run.repository.owner}/${run.repository.repo}@${run.repository.ref}`,
      runId: run.id,
      runnerId: run.runnerId,
      runtime: run.runtime,
      status: run.status,
      updatedAt: run.updatedAt,
    },
  })
}

export const launchResponseFromTeamPost = (
  response: TeamChatPostResponse,
): AgentRunLaunchResponse | undefined =>
  response.run === undefined ||
  response.events === undefined ||
  response.statusUrl === undefined ||
  response.streamUrl === undefined
    ? undefined
    : {
        run: response.run,
        events: response.events,
        statusUrl: response.statusUrl,
        streamUrl: response.streamUrl,
      }

export const syncScopeId = (scope: string): string => {
  const [_kind, ...idParts] = scope.split(':')

  return idParts.join(':')
}

const repositoryFromSyncRecord = (
  value: unknown,
): AgentRunApiRepository | undefined => {
  const record = recordFromUnknown(value)

  if (record === undefined) {
    return undefined
  }

  const owner = textFromUnknown(record.owner)
  const repo = textFromUnknown(record.repo)
  const ref = textFromUnknown(record.ref)

  if (owner === undefined || repo === undefined || ref === undefined) {
    return undefined
  }

  return {
    provider: textFromUnknown(record.provider) ?? 'github',
    owner,
    repo,
    ref,
  }
}

const agentRunFromSyncRecord = (value: unknown): AgentRunApiRun | undefined => {
  const record = recordFromUnknown(value)

  if (record === undefined) {
    return undefined
  }

  const id = textFromUnknown(record.id)
  const runtime = textFromUnknown(record.runtime)
  const backend = textFromUnknown(record.backend)
  const runnerId = textFromUnknown(record.runnerId)
  const repository = repositoryFromSyncRecord(record.repository)
  const goal = textFromUnknown(record.goal)
  const status = agentRunStatusFromUnknown(record.status)
  const createdAt = textFromUnknown(record.createdAt)
  const updatedAt = textFromUnknown(record.updatedAt)

  if (
    id === undefined ||
    runtime === undefined ||
    backend === undefined ||
    runnerId === undefined ||
    repository === undefined ||
    goal === undefined ||
    status === undefined ||
    createdAt === undefined ||
    updatedAt === undefined
  ) {
    return undefined
  }

  return {
    id,
    runtime,
    backend,
    runnerId,
    userId: textFromUnknown(record.userId) ?? null,
    teamId: textFromUnknown(record.teamId) ?? null,
    projectId: textFromUnknown(record.projectId) ?? null,
    repository,
    goal,
    externalRunId: textFromUnknown(record.externalRunId) ?? null,
    status,
    eventCursor: numberFromUnknown(record.eventCursor),
    createdAt,
    updatedAt,
  }
}

const agentRunEventFromSyncRecord = (
  value: unknown,
): AgentRunApiEvent | undefined => {
  const record = recordFromUnknown(value)

  if (record === undefined) {
    return undefined
  }

  const id = textFromUnknown(record.id)
  const parentId = textFromUnknown(record.runId)
  const type = textFromUnknown(record.type)
  const summary = textFromUnknown(record.summary)
  const source = textFromUnknown(record.source)
  const createdAt = textFromUnknown(record.createdAt)

  if (
    id === undefined ||
    parentId === undefined ||
    type === undefined ||
    summary === undefined ||
    source === undefined ||
    createdAt === undefined
  ) {
    return undefined
  }

  return {
    id,
    parentId,
    sequence: numberFromUnknown(record.sequence),
    type,
    summary,
    status: textFromUnknown(record.status) ?? null,
    source,
    payloadJson: textFromUnknown(record.payloadJson) ?? null,
    artifactRefs: stringArrayFromUnknown(record.artifactRefs),
    externalEventId: textFromUnknown(record.externalEventId) ?? null,
    createdAt,
  }
}

const syncRunMatchesRequest = (
  record: Record<string, unknown>,
  requestedId: string | undefined,
): boolean => {
  if (requestedId === undefined) {
    return true
  }

  return (
    textFromUnknown(record.id) === requestedId ||
    textFromUnknown(record.routeId) === requestedId ||
    displayRunId(textFromUnknown(record.id) ?? '') === requestedId
  )
}

export const agentRunResponseFromSyncCollections = (
  collections: Record<string, Record<string, unknown>> | undefined,
  requestedId: string | undefined,
): AgentRunDetailResponse | undefined => {
  const runs = collections?.agent_runs ?? {}
  const selectedRun = Object.values(runs).find(value => {
    const record = recordFromUnknown(value)

    return record !== undefined && syncRunMatchesRequest(record, requestedId)
  })
  const run = agentRunFromSyncRecord(selectedRun)

  if (run === undefined) {
    return undefined
  }

  const events = Object.values(collections?.agent_run_events ?? {})
    .flatMap(value => {
      const event = agentRunEventFromSyncRecord(value)

      return event === undefined || event.parentId !== run.id ? [] : [event]
    })
    .sort((left, right) => left.sequence - right.sequence)

  return {
    run,
    events,
  }
}

export const agentGoalFromSyncCollections = (
  collections: Record<string, Record<string, unknown>> | undefined,
  scopeKey: string,
): AgentGoalApiGoal | undefined => {
  const goals = collections?.agent_goals ?? {}

  return Object.values(goals)
    .flatMap(value => {
      const goal = agentGoalFromSyncRecord(value)

      return goal === undefined ? [] : [goal]
    })
    .find(goal => {
      const key = [
        goal.agentId,
        goal.teamId ?? 'personal',
        goal.projectId ?? 'room',
      ].join(':')

      return key === scopeKey
    })
}

export const syncSnapshotHref = (scope: string): string => {
  const [kind, ...idParts] = scope.split(':')
  const id = idParts.join(':')

  return `/api/sync/${kind}/${encodeURIComponent(id)}/snapshot`
}

export const syncWithSnapshot = (
  sync: SyncClientModelType,
  scope: string,
  snapshot: SyncSnapshot,
): SyncClientModelType =>
  SyncClientModel({
    ...sync,
    collectionByScope: {
      ...sync.collectionByScope,
      [scope]: snapshot.collections,
    },
    cursors: {
      ...sync.cursors,
      [scope]: snapshot.cursor,
    },
  })

export const syncWithPatch = (
  sync: SyncClientModelType,
  patch: SyncPatch,
): SyncClientModelType => {
  const scopeCollections = sync.collectionByScope[patch.scope] ?? {}
  const collection = scopeCollections[patch.collection] ?? {}
  const previousRecord = recordFromUnknown(collection[patch.id]) ?? {}
  const patchRecord = recordFromUnknown(patch.patch) ?? {}
  const nextCollection =
    patch.op === 'delete' || patch.op === 'invalidate'
      ? Object.fromEntries(
          Object.entries(collection).filter(([key]) => key !== patch.id),
        )
      : {
          ...collection,
          [patch.id]:
            patch.op === 'patch'
              ? { ...previousRecord, ...patchRecord }
              : patch.value,
        }

  return SyncClientModel({
    ...sync,
    collectionByScope: {
      ...sync.collectionByScope,
      [patch.scope]: {
        ...scopeCollections,
        [patch.collection]: nextCollection,
      },
    },
    cursors: {
      ...sync.cursors,
      [patch.scope]: patch.seq,
    },
    pendingMutations:
      patch.mutationId === undefined
        ? sync.pendingMutations
        : Object.fromEntries(
            Object.entries(sync.pendingMutations).filter(
              ([mutationId]) => mutationId !== patch.mutationId,
            ),
          ),
  })
}

const missionStatusFromRunStatus = (
  status: AgentRunStatus,
): SidebarSessionItem['status'] => {
  if (status === 'running' || status === 'waiting_for_input') {
    return 'active'
  }

  if (status === 'completed') {
    return 'complete'
  }

  if (status === 'failed' || status === 'canceled') {
    return 'failed'
  }

  return 'queued'
}

export const sidebarMissionFromRunResponse = (
  response: AgentRunResponseWithOptionalUrls,
): SidebarSessionItem => {
  const routeId = displayRunId(response.run.id)
  const repository = response.run.repository.repo
  const status = missionStatusFromRunStatus(response.run.status)
  const projectId = response.run.projectId ?? undefined
  const teamId = response.run.teamId ?? undefined
  const owner =
    projectId === undefined
      ? teamId === undefined
        ? 'personal'
        : 'team'
      : 'project'

  return {
    active: false,
    attention: status === 'active',
    detail: `${repository} - ${response.run.status}`,
    href: `/t/${encodeURIComponent(routeId)}`,
    owner,
    ...(response.run.userId === null || response.run.userId === undefined
      ? {}
      : { ownerUserId: response.run.userId }),
    ...(projectId === undefined ? {} : { projectId }),
    status,
    ...(teamId === undefined ? {} : { teamId }),
    title: response.run.goal,
    updatedAt: response.run.updatedAt,
  }
}
