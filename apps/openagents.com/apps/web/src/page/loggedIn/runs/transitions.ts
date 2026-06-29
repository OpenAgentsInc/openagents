import { Match as M, Option } from 'effect'
import type { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { loggedInWorkroomAllowed } from '../../../product-policy'
import { ChatRoute } from '../../../route'
import {
  FocusChatComposer,
  ScrollChatTimelineToEnd,
  SetAutopilotThreadUrl,
} from '../commands/dom'
import { Message } from '../message'
import {
  AgentRunDetailResponse,
  AgentRunLaunchResponse,
  type AgentRunStatus,
  ChatMessage,
  ClosedRunMetadataDialog,
  FailedChatRun,
  IdleChatRun,
  IdleThreadFileUpload,
  LaunchingChatRun,
  LoadingChatRun,
  Model,
  OpenRunMetadataDialog,
  sidebarWithMissionItem,
  syncAgentRunScope,
  syncThreadScope,
} from '../model'
import { LoadSyncSnapshot } from '../sync/commands'
import {
  activeChatRunFromResponse,
  chatMessageFromRunResponse,
  displayRunId,
  runAuthorLabel,
  sidebarMissionFromRunResponse,
  syncSnapshotHref,
} from '../sync/projection'
import {
  ThreadRouteIdle,
  authorizedThreadRoute,
  resolvingThreadRoute,
  unavailableThreadRoute,
} from '../thread-route'
import { type UpdateReturn, noUpdate } from '../transition'
import { ingestSessionNotifications } from '../notifications/transitions'
import { FetchAutopilotRun, LaunchAutopilotRun } from './commands'

type AgentRunResponseWithOptionalUrls =
  | AgentRunLaunchResponse
  | AgentRunDetailResponse

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const activeStatuses: ReadonlyArray<AgentRunStatus> = [
  'queued',
  'running',
  'waiting_for_input',
]

export const chatRunIsBusy = (model: Model): boolean => {
  if (model.chatRun._tag === 'Loading') {
    return true
  }

  if (model.chatRun._tag === 'Launching') {
    return true
  }

  if (model.chatRun._tag !== 'Active') {
    return false
  }

  return activeStatuses.includes(model.chatRun.metadata.status)
}

export const activeRunMatchesResponse = (
  model: Model,
  response: AgentRunResponseWithOptionalUrls,
): boolean => {
  if (model.chatRun._tag === 'Loading') {
    return (
      model.chatRun.runId === response.run.id ||
      model.chatRun.runId === displayRunId(response.run.id)
    )
  }

  if (model.chatRun._tag !== 'Active') {
    return false
  }

  return (
    model.chatRun.metadata.runId === response.run.id ||
    model.chatRun.metadata.displayRunId === displayRunId(response.run.id)
  )
}

export const isThreadScopeForLoadingRun = (
  model: Model,
  scope: string,
): boolean =>
  scope.startsWith('thread:') &&
  model.chatRun._tag === 'Loading' &&
  model.threadRoute._tag === 'ThreadRouteResolving' &&
  model.threadRoute.scope === scope &&
  model.chatRun.runId === model.threadRoute.routeId

export const applySyncRunResponse = (
  model: Model,
  response: AgentRunDetailResponse,
): Model => {
  const previousMetadata =
    model.chatRun._tag === 'Active' ? model.chatRun.metadata : undefined
  const sidebarMission = sidebarMissionFromRunResponse(response)

  return evo(model, {
    chatMessages: messages =>
      messages.length === 0
        ? [
            chatMessageFromRunResponse(
              response,
              runAuthorLabel(response, model.session, model.auth.teams),
            ),
          ]
        : messages,
    chatRun: () => activeChatRunFromResponse(response, previousMetadata),
    runMetadataDialog: () => ClosedRunMetadataDialog(),
    sidebar: sidebar =>
      sidebarWithMissionItem(sidebar, sidebarMission, sidebarMission.href),
    threadRoute: threadRoute =>
      threadRoute._tag === 'ThreadRouteResolving'
        ? authorizedThreadRoute(threadRoute.routeId, response.run.id)
        : threadRoute,
  })
}

/**
 * Apply a polled run response, then derive Pylon-session notifications from the
 * resulting model via the shared `notificationsFromSessions` core. Returns the
 * notification commands (browser-notification raise) merged with `extraCommands`.
 */
const applyRunResponseWithNotifications = (
  model: Model,
  response: AgentRunDetailResponse,
  extraCommands: ReadonlyArray<Command.Command<Message>>,
): UpdateReturn => {
  const applied = applySyncRunResponse(model, response)
  const [ingestedModel, notificationCommands] =
    ingestSessionNotifications(applied)

  return [
    ingestedModel,
    [...extraCommands, ...notificationCommands],
    Option.none(),
  ]
}

export const submitRunChatComposer = (
  model: Model,
  prompt: string,
): UpdateReturn => {
  if (chatRunIsBusy(model)) {
    return noUpdate(model)
  }

  const index = model.chatMessages.length + 1
  const requestId = `chat-request-${index}`
  const userTurn: ChatMessage = {
    author: 'user',
    body: prompt,
    id: `user-turn-${index}`,
    label: model.session.name,
    status: 'complete',
  }

  return [
    evo(model, {
      chatComposerValue: () => '',
      chatMessages: messages => [...messages, userTurn],
      chatRun: () => LaunchingChatRun({ prompt, requestId }),
      runMetadataDialog: () => ClosedRunMetadataDialog(),
    }),
    [
      LaunchAutopilotRun({ prompt, requestId }),
      ScrollChatTimelineToEnd(),
      FocusChatComposer(),
    ],
    Option.none(),
  ]
}

export const updateRunState = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      ClickedNewChat: () =>
        loggedInWorkroomAllowed(model.auth)
          ? [
              evo(model, {
                chatComposerValue: () => '',
                chatMessages: () => [],
                chatRun: () => IdleChatRun(),
                runMetadataDialog: () => ClosedRunMetadataDialog(),
                threadRoute: () => ThreadRouteIdle(),
                threadFileUpload: () => IdleThreadFileUpload(),
                route: () => ChatRoute(),
              }),
              [FocusChatComposer()],
              Option.none(),
            ]
          : noUpdate(model),
      EnteredAutopilotRunRoute: ({ runId }) => {
        if (
          model.chatRun._tag === 'Active' &&
          (model.chatRun.metadata.runId === runId ||
            model.chatRun.metadata.displayRunId === runId)
        ) {
          const canonicalRunId = model.chatRun.metadata.runId

          return [
            evo(model, {
              threadRoute: () => authorizedThreadRoute(runId, canonicalRunId),
            }),
            [],
            Option.none(),
          ]
        }

        return [
          evo(model, {
            chatComposerValue: () => '',
            chatMessages: () => [],
            chatRun: () => LoadingChatRun({ runId }),
            runMetadataDialog: () => ClosedRunMetadataDialog(),
            threadRoute: () => resolvingThreadRoute(runId),
          }),
          [
            LoadSyncSnapshot({
              href: syncSnapshotHref(syncThreadScope(runId)),
              scope: syncThreadScope(runId),
            }),
            ScrollChatTimelineToEnd(),
          ],
          Option.none(),
        ]
      },
      SucceededLaunchAutopilotRun: ({ requestId, response }) => {
        if (
          model.chatRun._tag !== 'Launching' ||
          model.chatRun.requestId !== requestId
        ) {
          return noUpdate(model)
        }

        const sidebarMission = sidebarMissionFromRunResponse(response)

        return [
          evo(model, {
            chatRun: () => activeChatRunFromResponse(response),
            runMetadataDialog: () => ClosedRunMetadataDialog(),
            sidebar: sidebar =>
              sidebarWithMissionItem(
                sidebar,
                sidebarMission,
                sidebarMission.href,
              ),
          }),
          [
            SetAutopilotThreadUrl({
              threadId: displayRunId(response.run.id),
            }),
            LoadSyncSnapshot({
              href: syncSnapshotHref(syncAgentRunScope(response.run.id)),
              scope: syncAgentRunScope(response.run.id),
            }),
            ScrollChatTimelineToEnd(),
          ],
          Option.none(),
        ]
      },
      FailedLaunchAutopilotRun: ({ error, requestId }) => {
        if (
          model.chatRun._tag !== 'Launching' ||
          model.chatRun.requestId !== requestId
        ) {
          return noUpdate(model)
        }

        return [
          evo(model, {
            chatRun: () => FailedChatRun({ error }),
            runMetadataDialog: () => ClosedRunMetadataDialog(),
          }),
          [ScrollChatTimelineToEnd()],
          Option.none(),
        ]
      },
      RequestedPollAutopilotRun: ({ runId }) => {
        if (
          model.chatRun._tag !== 'Active' ||
          model.chatRun.metadata.runId !== runId ||
          !chatRunIsBusy(model)
        ) {
          return noUpdate(model)
        }

        return [model, [FetchAutopilotRun({ runId })], Option.none()]
      },
      SucceededFetchAutopilotRun: ({ response, runId }) => {
        if (
          !(
            (model.chatRun._tag === 'Active' &&
              model.chatRun.metadata.runId === runId) ||
            (model.chatRun._tag === 'Loading' && model.chatRun.runId === runId)
          )
        ) {
          return noUpdate(model)
        }

        return applyRunResponseWithNotifications(model, response, [
          ScrollChatTimelineToEnd(),
        ])
      },
      FailedFetchAutopilotRun: ({ error, runId }) => {
        if (
          !(
            (model.chatRun._tag === 'Active' &&
              model.chatRun.metadata.runId === runId) ||
            (model.chatRun._tag === 'Loading' && model.chatRun.runId === runId)
          )
        ) {
          return noUpdate(model)
        }

        return [
          evo(model, {
            chatRun: () => FailedChatRun({ error }),
            runMetadataDialog: () => ClosedRunMetadataDialog(),
            threadRoute: threadRoute =>
              threadRoute._tag === 'ThreadRouteResolving' &&
              threadRoute.routeId === runId
                ? unavailableThreadRoute(runId, error)
                : threadRoute,
          }),
          [ScrollChatTimelineToEnd()],
          Option.none(),
        ]
      },
      ClickedRunMetadataInfo: () => [
        evo(model, { runMetadataDialog: () => OpenRunMetadataDialog() }),
        [],
        Option.none(),
      ],
      ClosedRunMetadataInfo: () => [
        evo(model, { runMetadataDialog: () => ClosedRunMetadataDialog() }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )
