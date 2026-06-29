import {
  type WorkerBindings,
  agentRunScope,
  makeD1SyncOutboxRepository,
  personalWorkroomScope,
  publicAgentRunScope,
  publicAgentScope,
  publicGoalScope,
  threadScope as syncThreadScope,
  teamScope,
} from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  publicAgentGoalEventFromRecord,
} from './agent-goal-public-projection'
import type { AgentGoalEventRecord } from './agent-goal-runtime'
import {
  type AgentGoalRecord,
  publicAgentGoalRecord,
} from './agent-goals'
import { type AgentRunRecord, agentRunRouteId } from './omni-runs'
import { observedPromise } from './observability'
import {
  openAgentsDatabase,
  scheduleBackgroundWork,
  syncRoomNotifications,
  syncScopes,
} from './runtime'
import type { TeamChatMessage } from './team-chat'
import type { PublicThreadFile } from './thread-files'

type SyncEnv = Pick<WorkerBindings, 'OPENAGENTS_DB' | 'SYNC_ROOM'>
type SyncRoomEnv = Pick<WorkerBindings, 'SYNC_ROOM'>

export type SyncNotificationContext = Pick<ExecutionContext, 'waitUntil'>

export const TEAM_CHAT_MESSAGES_SYNC_COLLECTION = 'team_chat_messages'
export const THREAD_FILES_SYNC_COLLECTION = 'thread_files'
export const AGENT_GOALS_SYNC_COLLECTION = 'agent_goals'
export const AGENT_GOAL_EVENTS_SYNC_COLLECTION = 'agent_goal_events'
export const PUBLIC_AGENT_GOALS_SYNC_COLLECTION = 'public_agent_goals'
export const PUBLIC_AGENT_GOAL_EVENTS_SYNC_COLLECTION =
  'public_agent_goal_events'

export const syncScopeForAgentRun = (
  run: AgentRunRecord,
): ReadonlyArray<string> => {
  const routeId = agentRunRouteId(run.id)

  return [
    personalWorkroomScope(run.userId),
    ...(run.teamId === null ? [] : [teamScope(run.teamId)]),
    agentRunScope(run.id),
    syncThreadScope(routeId),
  ]
}

export const notifySyncScopes = async (
  env: SyncRoomEnv,
  scopes: ReadonlyArray<string>,
): Promise<void> => {
  await observedPromise('Sync.notifyScopes', () =>
    syncRoomNotifications(env).notifyScopesPromise(syncScopes(scopes)),
  )
}

const remainingTokens = (goal: AgentGoalRecord): number | null =>
  goal.tokenBudget === null
    ? null
    : Math.max(0, goal.tokenBudget - goal.tokensUsed)

export const agentGoalSyncValue = (goal: AgentGoalRecord) => ({
  id: goal.id,
  agentId: goal.agentId,
  userId: goal.userId,
  teamId: goal.teamId,
  projectId: goal.projectId,
  objective: goal.objective,
  status: goal.status,
  visibility: goal.visibility,
  currentRunId: goal.currentRunId,
  tokenBudget: goal.tokenBudget,
  tokensUsed: goal.tokensUsed,
  timeUsedSeconds: goal.timeUsedSeconds,
  remainingTokens: remainingTokens(goal),
  createdAt: goal.createdAt,
  updatedAt: goal.updatedAt,
  completedAt: goal.completedAt,
  pausedAt: goal.pausedAt,
  blockedAt: goal.blockedAt,
  canEdit: goal.archivedAt === null,
  canPause: goal.archivedAt === null && goal.status === 'active',
  canResume:
    goal.archivedAt === null &&
    (goal.status === 'paused' ||
      goal.status === 'blocked' ||
      goal.status === 'usage_limited' ||
      goal.status === 'budget_limited'),
  canMakePublic: goal.archivedAt === null && goal.visibility !== 'public',
  publicUrl: goal.visibility === 'public' ? `/api/public/goals/${goal.id}` : null,
})

export const agentGoalEventSyncValue = (event: AgentGoalEventRecord) => ({
  id: event.id,
  goalId: event.goalId,
  runId: event.runId,
  eventType: event.eventType,
  status: event.status,
  tokenDelta: event.tokenDelta,
  timeDeltaSeconds: event.timeDeltaSeconds,
  createdAt: event.createdAt,
})

const privateGoalScopes = (goal: AgentGoalRecord): ReadonlyArray<string> => [
  ...(goal.userId === null ? [] : [personalWorkroomScope(goal.userId)]),
  ...(goal.teamId === null ? [] : [teamScope(goal.teamId)]),
  ...(goal.currentRunId === null
    ? []
    : [
        agentRunScope(goal.currentRunId),
        syncThreadScope(agentRunRouteId(goal.currentRunId)),
      ]),
]

const publicGoalScopes = (goal: AgentGoalRecord): ReadonlyArray<string> => [
  publicAgentScope(goal.agentId),
  publicGoalScope(goal.id),
  ...(goal.currentRunId === null
    ? []
    : [publicAgentRunScope(goal.currentRunId)]),
]

const uniqueScopes = (scopes: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(scopes)]

export const publicAgentGoalSyncValue = (goal: AgentGoalRecord) =>
  publicAgentGoalRecord(goal)

export const publicAgentGoalEventSyncValue = async (
  event: AgentGoalEventRecord,
) => Effect.runSync(publicAgentGoalEventFromRecord(event))

export const publishAgentGoalSyncIfBound = async (
  env: Pick<WorkerBindings, 'OPENAGENTS_DB'> &
    Partial<Pick<WorkerBindings, 'SYNC_ROOM'>>,
  ctx: SyncNotificationContext,
  goal: AgentGoalRecord,
  actorId: string,
): Promise<void> => {
  await publishAgentGoalSync(
    env as Pick<WorkerBindings, 'OPENAGENTS_DB' | 'SYNC_ROOM'>,
    ctx,
    goal,
    actorId,
  )
}

export const publishAgentGoalSync = async (
  env: SyncEnv,
  ctx: SyncNotificationContext,
  goal: AgentGoalRecord,
  actorId: string,
): Promise<void> => {
  await observedPromise('Sync.publishAgentGoal', async () => {
    const store = makeD1SyncOutboxRepository(openAgentsDatabase(env))
    const privateScopes = privateGoalScopes(goal)
    const publicScopes = publicGoalScopes(goal)
    const privateOp = goal.archivedAt === null ? 'put' : 'delete'
    const publicOp =
      goal.archivedAt === null && goal.visibility === 'public'
        ? 'put'
        : 'delete'

    await Promise.all([
      ...privateScopes.map(scope =>
        store.appendChange({
          actorId,
          collection: AGENT_GOALS_SYNC_COLLECTION,
          id: goal.id,
          op: privateOp,
          scope,
          value: privateOp === 'put' ? agentGoalSyncValue(goal) : undefined,
        }),
      ),
      ...publicScopes.map(scope =>
        store.appendChange({
          actorId,
          collection: PUBLIC_AGENT_GOALS_SYNC_COLLECTION,
          id: goal.id,
          op: publicOp,
          scope,
          value:
            publicOp === 'put' ? publicAgentGoalSyncValue(goal) : undefined,
        }),
      ),
    ])

    scheduleBackgroundWork(
      ctx,
      notifySyncScopes(env, uniqueScopes([...privateScopes, ...publicScopes])),
    )
  })
}

export const publishAgentGoalEventSync = async (
  env: SyncEnv,
  ctx: SyncNotificationContext,
  goal: AgentGoalRecord,
  event: AgentGoalEventRecord,
  actorId: string,
): Promise<void> => {
  await observedPromise('Sync.publishAgentGoalEvent', async () => {
    const store = makeD1SyncOutboxRepository(openAgentsDatabase(env))
    const privateScopes = privateGoalScopes(goal)
    const publicScopes =
      goal.archivedAt === null && goal.visibility === 'public'
        ? publicGoalScopes(goal)
        : []
    const publicEvent =
      publicScopes.length === 0
        ? undefined
        : await publicAgentGoalEventSyncValue(event)

    await Promise.all([
      ...privateScopes.map(scope =>
        store.appendChange({
          actorId,
          collection: AGENT_GOAL_EVENTS_SYNC_COLLECTION,
          id: event.id,
          op: 'put',
          scope,
          value: agentGoalEventSyncValue(event),
        }),
      ),
      ...publicScopes.map(scope =>
        store.appendChange({
          actorId,
          collection: PUBLIC_AGENT_GOAL_EVENTS_SYNC_COLLECTION,
          id: event.id,
          op: 'put',
          scope,
          value: publicEvent,
        }),
      ),
    ])

    scheduleBackgroundWork(
      ctx,
      notifySyncScopes(env, uniqueScopes([...privateScopes, ...publicScopes])),
    )
  })
}

export const teamChatMessageSyncValue = (
  message: TeamChatMessage,
): TeamChatMessage => message

export const threadFileSyncValue = (file: PublicThreadFile): PublicThreadFile =>
  file

export const publishTeamChatMessageSync = async (
  env: SyncEnv,
  ctx: SyncNotificationContext,
  message: TeamChatMessage,
  actorId: string,
): Promise<void> => {
  await observedPromise('Sync.publishTeamChatMessage', async () => {
    const scope = teamScope(message.teamId)

    await makeD1SyncOutboxRepository(openAgentsDatabase(env)).appendChange({
      actorId,
      collection: TEAM_CHAT_MESSAGES_SYNC_COLLECTION,
      id: message.id,
      op: 'put',
      scope,
      value: teamChatMessageSyncValue(message),
    })
    scheduleBackgroundWork(ctx, notifySyncScopes(env, [scope]))
  })
}

export const publishTeamThreadFileSync = async (
  env: SyncEnv,
  ctx: SyncNotificationContext,
  file: PublicThreadFile,
  actorId: string,
): Promise<void> => {
  await observedPromise('Sync.publishTeamThreadFile', async () => {
    if (file.teamId === null) {
      return
    }

    const scope = teamScope(file.teamId)

    await makeD1SyncOutboxRepository(openAgentsDatabase(env)).appendChange({
      actorId,
      collection: THREAD_FILES_SYNC_COLLECTION,
      id: file.id,
      op: 'put',
      scope,
      value: threadFileSyncValue(file),
    })
    scheduleBackgroundWork(ctx, notifySyncScopes(env, [scope]))
  })
}

export const readAgentRunSyncScopes = async (
  env: Pick<WorkerBindings, 'OPENAGENTS_DB'>,
  runId: string,
): Promise<ReadonlyArray<string>> => {
  const row = await openAgentsDatabase(env)
    .prepare(`SELECT id, user_id, team_id FROM agent_runs WHERE id = ?`)
    .bind(runId)
    .first<Readonly<{ id: string; team_id: string | null; user_id: string }>>()
  const routeId = agentRunRouteId(row?.id ?? runId)

  return row === null
    ? [agentRunScope(runId), syncThreadScope(routeId)]
    : [
        personalWorkroomScope(row.user_id),
        ...(row.team_id === null ? [] : [teamScope(row.team_id)]),
        agentRunScope(row.id),
        syncThreadScope(routeId),
      ]
}

export const notifyAgentRunSyncScopes = async (
  env: SyncEnv,
  runId: string,
): Promise<void> => {
  await observedPromise('Sync.notifyAgentRunSyncScopes', async () => {
    await notifySyncScopes(env, await readAgentRunSyncScopes(env, runId))
  })
}
