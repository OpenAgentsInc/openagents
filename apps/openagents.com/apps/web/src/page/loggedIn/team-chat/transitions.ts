import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import { loggedInWorkroomAllowed } from '../../../product-policy'
import {
  appendTeamChatMessage,
  teamChatRoomKey,
  teamChatRouteContext,
} from '../chatState'
import { FocusChatComposer, ScrollChatTimelineToEnd } from '../commands/dom'
import { Message } from '../message'
import {
  ClosedRunMetadataDialog,
  FailedChatRun,
  LaunchingChatRun,
  Model,
  sidebarWithMissionItem,
  syncAgentRunScope,
} from '../model'
import { chatRunIsBusy, submitRunChatComposer } from '../runs/transitions'
import { LoadSyncSnapshot } from '../sync/commands'
import {
  activeChatRunFromResponse,
  launchResponseFromTeamPost,
  sidebarMissionFromRunResponse,
  syncSnapshotHref,
} from '../sync/projection'
import { type UpdateReturn, noUpdate } from '../transition'
import { LoadTeamChatMessages, PostTeamChatMessage } from './commands'

const TEAM_AUTOPILOT_COMMAND = '@autopilot'
const TEAM_ADJUTANT_COMMAND = '@adjutant'
const ADJUTANT_PROJECT_ID = 'project_adjutant'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const exactTeamCommandPrompt = (
  prompt: string,
  command: string,
): string | undefined => {
  const trimmed = prompt.trim()
  const lower = trimmed.toLowerCase()
  const prefix = `${command} `

  if (lower.startsWith(prefix)) {
    const body = trimmed.slice(command.length).trim()

    return body === '' ? undefined : body
  }

  const lines = prompt.split(/\r?\n/)
  const commandLineIndex = lines.findIndex(
    line => line.trim().toLowerCase() === command,
  )

  if (commandLineIndex !== -1) {
    const body = [
      ...lines.slice(0, commandLineIndex),
      ...lines.slice(commandLineIndex + 1),
    ]
      .join('\n')
      .trim()

    return body === '' ? undefined : body
  }

  const suffix = ` ${command}`

  if (lower.endsWith(suffix)) {
    const body = trimmed.slice(0, -command.length).trim()

    return body === '' ? undefined : body
  }

  return undefined
}

const exactTeamAutopilotPrompt = (prompt: string): string | undefined =>
  exactTeamCommandPrompt(prompt, TEAM_AUTOPILOT_COMMAND)

const exactTeamAdjutantPrompt = (prompt: string): string | undefined =>
  exactTeamCommandPrompt(prompt, TEAM_AUTOPILOT_COMMAND) ??
  exactTeamCommandPrompt(prompt, TEAM_ADJUTANT_COMMAND)

const submitTeamChatComposer = (model: Model, prompt: string): UpdateReturn => {
  const teamContext = teamChatRouteContext(model)

  if (teamContext === undefined) {
    return [
      evo(model, {
        chatRun: () =>
          FailedChatRun({
            error: 'This room is not available in the current session.',
          }),
      }),
      [ScrollChatTimelineToEnd(), FocusChatComposer()],
      Option.none(),
    ]
  }

  const isAdjutantProject = teamContext.projectId === ADJUTANT_PROJECT_ID
  const adjutantPrompt =
    isAdjutantProject || exactTeamAutopilotPrompt(prompt) === undefined
      ? exactTeamAdjutantPrompt(prompt)
      : undefined
  const autopilotPrompt =
    adjutantPrompt === undefined ? exactTeamAutopilotPrompt(prompt) : undefined

  if (autopilotPrompt !== undefined && chatRunIsBusy(model)) {
    return noUpdate(model)
  }

  const requestId = `team-chat-request-${(model.teamChatMessagesByTeam[teamContext.roomKey]?.length ?? 0) + 1}`
  const kind =
    autopilotPrompt !== undefined
      ? ('autopilot_intent' as const)
      : adjutantPrompt !== undefined
        ? ('adjutant_intent' as const)
        : ('message' as const)

  return [
    evo(model, {
      chatComposerValue: () => '',
      chatRun: chatRun =>
        autopilotPrompt === undefined
          ? chatRun
          : LaunchingChatRun({
              prompt: autopilotPrompt,
              requestId,
            }),
      runMetadataDialog: () => ClosedRunMetadataDialog(),
    }),
    [
      PostTeamChatMessage({
        body: prompt,
        kind,
        ...(autopilotPrompt === undefined && adjutantPrompt === undefined
          ? {}
          : { prompt: autopilotPrompt ?? adjutantPrompt ?? '' }),
        ...(teamContext.projectId === undefined
          ? {}
          : { projectId: teamContext.projectId }),
        requestId,
        roomKey: teamContext.roomKey,
        teamId: teamContext.teamId,
      }),
      ScrollChatTimelineToEnd(),
      FocusChatComposer(),
    ],
    Option.none(),
  ]
}

export const updateTeamChat = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadTeamChatMessages: ({ href, roomKey, teamId }) => [
        model,
        [LoadTeamChatMessages({ href, roomKey, teamId })],
        Option.none(),
      ],
      SucceededLoadTeamChatMessages: ({ response, roomKey }) => [
        evo(model, {
          teamChatMessagesByTeam: messagesByTeam => ({
            ...messagesByTeam,
            [roomKey]: response.messages,
          }),
        }),
        [ScrollChatTimelineToEnd()],
        Option.none(),
      ],
      FailedLoadTeamChatMessages: ({ error, roomKey }) => [
        evo(model, {
          chatMessages: messages => [
            ...messages,
            {
              author: 'system',
              body: error,
              id: `team-chat-load-failed-${roomKey}`,
              label: 'OpenAgents',
              status: 'complete',
            },
          ],
        }),
        [ScrollChatTimelineToEnd()],
        Option.none(),
      ],
      SubmittedChatComposer: () => {
        if (!loggedInWorkroomAllowed(model.auth)) {
          return noUpdate(model)
        }

        const prompt = model.chatComposerValue.trim()

        if (prompt === '') {
          return noUpdate(model)
        }

        if (
          model.route._tag === 'TeamChat' ||
          model.route._tag === 'TeamProjectChat'
        ) {
          return submitTeamChatComposer(model, prompt)
        }

        return submitRunChatComposer(model, prompt)
      },
      SucceededPostTeamChatMessage: ({ requestId, response }) => {
        const launchResponse = launchResponseFromTeamPost(response)
        const roomKey = teamChatRoomKey(
          response.teamId,
          response.projectId ?? response.message.projectId ?? undefined,
        )
        const withMessage = evo(model, {
          teamChatMessagesByTeam: messagesByTeam => ({
            ...messagesByTeam,
            [roomKey]: appendTeamChatMessage(
              messagesByTeam[roomKey] ?? [],
              response.message,
            ),
          }),
        })

        if (launchResponse === undefined) {
          const launchError = response.launchError

          if (
            launchError !== undefined &&
            withMessage.chatRun._tag === 'Launching' &&
            withMessage.chatRun.requestId === requestId
          ) {
            return [
              evo(withMessage, {
                chatRun: () => FailedChatRun({ error: launchError }),
                runMetadataDialog: () => ClosedRunMetadataDialog(),
              }),
              [ScrollChatTimelineToEnd(), FocusChatComposer()],
              Option.none(),
            ]
          }

          return [withMessage, [ScrollChatTimelineToEnd()], Option.none()]
        }

        if (
          withMessage.chatRun._tag !== 'Launching' ||
          withMessage.chatRun.requestId !== requestId
        ) {
          return [withMessage, [ScrollChatTimelineToEnd()], Option.none()]
        }

        const sidebarMission = sidebarMissionFromRunResponse(launchResponse)

        return [
          evo(withMessage, {
            chatRun: () => activeChatRunFromResponse(launchResponse),
            runMetadataDialog: () => ClosedRunMetadataDialog(),
            sidebar: sidebar =>
              sidebarWithMissionItem(
                sidebar,
                sidebarMission,
                sidebarMission.href,
              ),
          }),
          [
            LoadSyncSnapshot({
              href: syncSnapshotHref(syncAgentRunScope(launchResponse.run.id)),
              scope: syncAgentRunScope(launchResponse.run.id),
            }),
            ScrollChatTimelineToEnd(),
          ],
          Option.none(),
        ]
      },
      FailedPostTeamChatMessage: ({ error, requestId, roomKey }) => {
        if (
          model.chatRun._tag === 'Launching' &&
          model.chatRun.requestId === requestId
        ) {
          return [
            evo(model, {
              chatRun: () => FailedChatRun({ error }),
              runMetadataDialog: () => ClosedRunMetadataDialog(),
            }),
            [ScrollChatTimelineToEnd()],
            Option.none(),
          ]
        }

        return [
          evo(model, {
            chatMessages: messages => [
              ...messages,
              {
                author: 'system',
                body: error,
                id: `team-chat-post-failed-${roomKey}-${requestId}`,
                label: 'OpenAgents',
                status: 'complete',
              },
            ],
          }),
          [ScrollChatTimelineToEnd(), FocusChatComposer()],
          Option.none(),
        ]
      },
    }),
    M.orElse(() => noUpdate(model)),
  )
