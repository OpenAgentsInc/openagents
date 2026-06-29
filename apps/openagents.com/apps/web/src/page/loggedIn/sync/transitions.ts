import type { SyncPatch } from '@openagentsinc/sync-schema'
import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import { threadRouter } from '../../../route'
import { applyTeamSyncCollections } from '../chatState'
import { ScrollChatTimelineToEnd } from '../commands/dom'
import {
  agentGoalScopeForRoute,
  syncScopeForAgentGoalRoute,
} from '../goals/scope'
import { modelWithAgentGoal } from '../goals/transitions'
import { Message } from '../message'
import {
  Model,
  type SidebarMissionReplacementGroup,
  SyncClientModel,
  missionItemsFromSnapshot,
  sidebarWithMissionItems,
  sidebarWithMissionPatch,
  syncTeamScope,
} from '../model'
import { FetchAutopilotRun } from '../runs/commands'
import {
  activeRunMatchesResponse,
  applySyncRunResponse,
  isThreadScopeForLoadingRun,
} from '../runs/transitions'
import { ThreadRouteState, authorizedThreadRoute } from '../thread-route'
import { type UpdateReturn, noUpdate } from '../transition'
import { LoadSyncSnapshot } from './commands'
import {
  activeChatRunWithSyncedEventPatch,
  activeChatRunWithSyncedRunPatch,
  agentGoalFromSyncCollections,
  agentRunResponseFromSyncCollections,
  syncScopeId,
  syncSnapshotHref,
  syncWithPatch,
  syncWithSnapshot,
} from './projection'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const authorizeResolvedThreadRoute = (
  threadRoute: ThreadRouteState,
  scope: string,
  response: NonNullable<ReturnType<typeof agentRunResponseFromSyncCollections>>,
): ThreadRouteState =>
  threadRoute._tag === 'ThreadRouteResolving' && threadRoute.scope === scope
    ? authorizedThreadRoute(threadRoute.routeId, response.run.id)
    : threadRoute

const modelWithSyncedAgentGoal = (
  model: Model,
  scope: string,
  collections: Record<string, Record<string, unknown>> | undefined,
): Model => {
  const goalScope = agentGoalScopeForRoute(model)
  const syncScope = syncScopeForAgentGoalRoute(model)

  if (goalScope === undefined || syncScope !== scope) {
    return model
  }

  const goal = agentGoalFromSyncCollections(collections, goalScope.scopeKey)

  return modelWithAgentGoal(
    model,
    goal === undefined ? Option.none() : Option.some(goal),
    goalScope.scopeKey,
  )
}

const isSidebarMissionScope = (model: Model, scope: string): boolean =>
  scope === model.sync.workspaceScope ||
  model.auth.teams.some(team => scope === syncTeamScope(team.id))

const sidebarMissionReplacementGroups = (
  model: Model,
  scope: string,
): ReadonlyArray<SidebarMissionReplacementGroup> =>
  scope === model.sync.workspaceScope ? ['personal'] : ['team-owned']

const activeThreadHref = (model: Model): string | undefined =>
  model.route._tag === 'Thread'
    ? threadRouter({ threadId: model.route.threadId })
    : undefined

const activeRunMatchesScope = (model: Model, scope: string): boolean => {
  if (model.chatRun._tag !== 'Active') {
    return false
  }

  const scopeId = syncScopeId(scope)

  return (
    scopeId === model.chatRun.metadata.runId ||
    scopeId === model.chatRun.metadata.displayRunId
  )
}

const modelWithIncrementalActiveRunPatch = (
  model: Model,
  patch: SyncPatch,
  collections: Record<string, Record<string, unknown>> | undefined,
): Model | undefined => {
  if (
    model.chatRun._tag !== 'Active' ||
    !activeRunMatchesScope(model, patch.scope)
  ) {
    return undefined
  }

  if (patch.collection === 'agent_run_events') {
    const nextChatRun = activeChatRunWithSyncedEventPatch(
      model.chatRun,
      patch,
      collections?.agent_run_events?.[patch.id],
    )

    return nextChatRun === undefined
      ? undefined
      : evo(model, { chatRun: () => nextChatRun })
  }

  if (patch.collection === 'agent_runs') {
    const nextChatRun = activeChatRunWithSyncedRunPatch(
      model.chatRun,
      patch,
      collections?.agent_runs?.[patch.id],
    )

    return nextChatRun === undefined
      ? undefined
      : evo(model, { chatRun: () => nextChatRun })
  }

  return undefined
}

export const updateSync = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadSyncSnapshot: ({ href, scope }) => [
        evo(model, {
          sync: sync =>
            SyncClientModel({
              ...sync,
              connectionByScope: {
                ...sync.connectionByScope,
                [scope]: { status: 'connecting' },
              },
            }),
        }),
        [LoadSyncSnapshot({ href, scope })],
        Option.none(),
      ],
      SucceededLoadSyncSnapshot: ({ scope, snapshot }) => {
        const nextSync = SyncClientModel({
          ...syncWithSnapshot(model.sync, scope, snapshot),
          connectionByScope: {
            ...model.sync.connectionByScope,
            [scope]: { status: 'open' },
          },
        })
        const response = agentRunResponseFromSyncCollections(
          snapshot.collections,
          syncScopeId(scope),
        )
        const modelWithSync = applyTeamSyncCollections(
          evo(model, { sync: () => nextSync }),
          scope,
          nextSync.collectionByScope[scope],
        )
        const modelWithGoal = modelWithSyncedAgentGoal(
          modelWithSync,
          scope,
          nextSync.collectionByScope[scope],
        )
        const nextModel =
          response !== undefined && activeRunMatchesResponse(model, response)
            ? applySyncRunResponse(modelWithGoal, response)
            : evo(modelWithGoal, {
                sidebar: sidebar =>
                  isSidebarMissionScope(model, scope)
                    ? sidebarWithMissionItems(
                        sidebar,
                        missionItemsFromSnapshot(
                          snapshot,
                          activeThreadHref(model),
                        ),
                        activeThreadHref(model),
                        sidebarMissionReplacementGroups(model, scope),
                      )
                    : sidebar,
              })

        const authorizedModel =
          response !== undefined && scope.startsWith('thread:')
            ? evo(nextModel, {
                threadRoute: threadRoute =>
                  authorizeResolvedThreadRoute(threadRoute, scope, response),
              })
            : nextModel

        return [
          authorizedModel,
          response === undefined && isThreadScopeForLoadingRun(model, scope)
            ? [FetchAutopilotRun({ runId: syncScopeId(scope) })]
            : response !== undefined &&
                activeRunMatchesResponse(model, response)
              ? [ScrollChatTimelineToEnd()]
              : [],
          Option.none(),
        ]
      },
      FailedLoadSyncSnapshot: ({ error, scope }) => {
        const isLoadingThreadScope = isThreadScopeForLoadingRun(model, scope)
        const runId = syncScopeId(scope)

        return [
          evo(model, {
            sync: sync =>
              SyncClientModel({
                ...sync,
                connectionByScope: {
                  ...sync.connectionByScope,
                  [scope]: { error, status: 'failed' },
                },
              }),
          }),
          isLoadingThreadScope ? [FetchAutopilotRun({ runId })] : [],
          Option.none(),
        ]
      },
      OpenedSyncStream: ({ scope }) => [
        evo(model, {
          sync: sync =>
            SyncClientModel({
              ...sync,
              connectionByScope: {
                ...sync.connectionByScope,
                [scope]: { status: 'open' },
              },
            }),
        }),
        [],
        Option.none(),
      ],
      ClosedSyncStream: ({ scope }) => [
        evo(model, {
          sync: sync =>
            SyncClientModel({
              ...sync,
              connectionByScope: {
                ...sync.connectionByScope,
                [scope]: { status: 'closed' },
              },
            }),
        }),
        [],
        Option.none(),
      ],
      FailedSyncStream: ({ error, scope }) => [
        evo(model, {
          sync: sync =>
            SyncClientModel({
              ...sync,
              connectionByScope: {
                ...sync.connectionByScope,
                [scope]: { error, status: 'failed' },
              },
            }),
        }),
        [],
        Option.none(),
      ],
      ReceivedSyncPatch: ({ patch }) => {
        const nextSync = syncWithPatch(model.sync, patch)
        const baseModel = modelWithSyncedAgentGoal(
          applyTeamSyncCollections(
            evo(model, {
              sidebar: sidebar =>
                isSidebarMissionScope(model, patch.scope)
                  ? sidebarWithMissionPatch(
                      sidebar,
                      patch,
                      activeThreadHref(model),
                    )
                  : sidebar,
              sync: () => nextSync,
            }),
            patch.scope,
            nextSync.collectionByScope[patch.scope],
          ),
          patch.scope,
          nextSync.collectionByScope[patch.scope],
        )
        const incrementalModel = modelWithIncrementalActiveRunPatch(
          baseModel,
          patch,
          nextSync.collectionByScope[patch.scope],
        )

        if (incrementalModel !== undefined) {
          return [incrementalModel, [], Option.none()]
        }

        const response =
          patch.scope === model.sync.workspaceScope
            ? undefined
            : agentRunResponseFromSyncCollections(
                nextSync.collectionByScope[patch.scope],
                syncScopeId(patch.scope),
              )
        const nextModel =
          response !== undefined && activeRunMatchesResponse(model, response)
            ? applySyncRunResponse(baseModel, response)
            : baseModel

        return [
          nextModel,
          response !== undefined && activeRunMatchesResponse(model, response)
            ? [ScrollChatTimelineToEnd()]
            : [],
          Option.none(),
        ]
      },
      ReceivedSyncCursorGap: ({ gap }) => [
        evo(model, {
          sync: sync =>
            SyncClientModel({
              ...sync,
              connectionByScope: {
                ...sync.connectionByScope,
                [gap.scope]: { error: 'cursor gap', status: 'failed' },
              },
            }),
        }),
        [
          LoadSyncSnapshot({
            href: syncSnapshotHref(gap.scope),
            scope: gap.scope,
          }),
        ],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )
