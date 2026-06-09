import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { userFacingCopy } from '../../../display-copy'
import type { Team, TeamProject } from '../../../domain/session'
import { iconView } from '../../../icon'
import * as Ui from '../../../ui'
import { CHAT_PROMPT_ID } from '../chatDom'
import { artanisOperatorDock } from '../artanis-console/view'
import { agentGoalDock } from '../goals/view'
import {
  ClickedRunMetadataInfo,
  ClosedRunMetadataInfo,
  FailedUploadThreadFile,
  type Message,
  SubmittedChatComposer,
  SubmittedThreadFileUpload,
  UpdatedChatComposer,
} from '../message'
import type {
  ChatMessage,
  ChatRun,
  Model,
  TeamChatMessageRecord,
  ThreadFileRecord,
} from '../model'
import { teamProjectRouteRef, teamRouteRef } from '../model'
import {
  type ProviderConnectionState,
  type RunTimelineMessage,
  artifactNames,
  chatRunTimelineMessages,
  formatBytes,
  providerConnectionState,
  teamAutopilotRunCardParts,
} from '../run-timeline/projection'
import {
  chatRunIsBusy,
  personalChatThreadId,
  teamChatRoomKey,
  teamChatThreadId,
  teamProjectChatThreadId,
  threadFilesScopeKey,
} from '../update'

type TimelineMessage = RunTimelineMessage

type RoomContext =
  | Readonly<{ kind: 'personal' }>
  | Readonly<{ kind: 'team'; teamRef: string }>
  | Readonly<{ kind: 'project'; projectRef: string; teamRef: string }>

const chatMessageToTimelineMessage = (
  message: ChatMessage,
): TimelineMessage => ({
  author: message.author,
  id: message.id,
  label: message.label,
  parts: [
    {
      kind: 'text',
      body: [message.body],
    },
  ],
  status: message.status,
  time: 'submitted',
})

const hasVisibleTeamCommand = (body: string, command: string): boolean => {
  const lower = body.trim().toLowerCase()
  return (
    lower === command ||
    lower.startsWith(`${command} `) ||
    lower.endsWith(` ${command}`) ||
    body.split(/\r?\n/).some(line => line.trim().toLowerCase() === command)
  )
}

const teamChatMessageToTimelineMessages = (
  message: TeamChatMessageRecord,
  currentUserId: string,
  chatRun: ChatRun,
  providerState: ProviderConnectionState,
): ReadonlyArray<TimelineMessage> => {
  const command =
    message.kind === 'autopilot_intent' || message.kind === 'adjutant_intent'
      ? '@autopilot'
      : undefined
  const messageBody = userFacingCopy(message.body)
  const body =
    command !== undefined && !hasVisibleTeamCommand(messageBody, command)
      ? `${command} ${messageBody}`
      : messageBody
  const baseMessage = {
    author:
      message.kind === 'system'
        ? 'system'
        : message.author.userId === currentUserId
          ? 'user'
          : 'assistant',
    id: message.id,
    label: message.kind === 'system' ? 'Autopilot' : message.author.name,
    ...(message.kind === 'system' || message.author.avatarUrl === null
      ? {}
      : { avatarUrl: message.author.avatarUrl }),
    parts: [
      {
        kind: 'text',
        body: [body],
      },
    ],
    status: 'complete',
    time: message.createdAt,
  } satisfies TimelineMessage
  const runParts =
    message.kind === 'autopilot_intent'
      ? teamAutopilotRunCardParts(message, chatRun, providerState)
      : []

  if (runParts.length === 0) {
    return [baseMessage]
  }

  return [
    baseMessage,
    {
      author: 'system',
      id: `${message.id}-autopilot-run`,
      label: 'Autopilot',
      parts: runParts,
      status: 'complete',
      time: message.createdAt,
    },
  ]
}

const teamForRef = (model: Model, teamRef: string): Team | undefined =>
  model.auth.teams.find(team => teamRouteRef(team) === teamRef)

const projectForRef = (
  team: Team | undefined,
  projectRef: string,
): TeamProject | undefined =>
  team?.projects?.find(project => teamProjectRouteRef(project) === projectRef)

const projectForContext = (
  team: Team | undefined,
  context: RoomContext,
): TeamProject | undefined =>
  context.kind === 'project'
    ? projectForRef(team, context.projectRef)
    : undefined

const roomKeyForContext = (
  context: RoomContext,
  team: Team | undefined,
  project: TeamProject | undefined,
): string | undefined => {
  if (context.kind === 'personal') {
    return undefined
  }

  if (team === undefined) {
    return undefined
  }

  return context.kind === 'project' && project !== undefined
    ? teamChatRoomKey(team.id, project.id)
    : teamChatRoomKey(team.id)
}

const emptyTimelineMessage = (
  context: RoomContext,
  team: Team | undefined,
  project: TeamProject | undefined,
): TimelineMessage => {
  if (context.kind === 'team' || context.kind === 'project') {
    return {
      id: context.kind === 'project' ? 'project-room-empty' : 'team-room-empty',
      author: 'system',
      label: 'Autopilot',
      time: 'ready',
      parts: [
        {
          kind: 'text',
          tone: 'muted',
          body: [
            team === undefined
              ? 'This team room is not available in the current session.'
              : context.kind === 'project'
                ? project === undefined
                  ? 'This project is not available in the current session.'
                  : `No messages in ${project.name} yet.`
                : `No messages in ${team.name} yet.`,
          ],
        },
      ],
    }
  }

  return {
    id: 'personal-chat-empty',
    author: 'system',
    label: 'Autopilot',
    time: 'ready',
    parts: [
      {
        kind: 'text',
        tone: 'muted',
        body: ['Start a new Autopilot chat.'],
      },
    ],
  }
}

const timelineMessages = (
  model: Model,
  context: RoomContext,
  team: Team | undefined,
  project: TeamProject | undefined,
): ReadonlyArray<TimelineMessage> => {
  const roomKey = roomKeyForContext(context, team, project)
  const teamMessages =
    roomKey !== undefined
      ? (model.teamChatMessagesByTeam[roomKey] ?? []).flatMap(message =>
          teamChatMessageToTimelineMessages(
            message,
            model.session.userId,
            model.chatRun,
            providerConnectionState(model),
          ),
        )
      : []
  const runMessages =
    context.kind === 'team' || context.kind === 'project'
      ? []
      : chatRunTimelineMessages(model.chatRun, providerConnectionState(model))
  const localChatMessages =
    context.kind === 'team' || context.kind === 'project'
      ? []
      : model.chatMessages.map(chatMessageToTimelineMessage)
  const messages = [...teamMessages, ...localChatMessages, ...runMessages]

  return messages.length === 0
    ? [emptyTimelineMessage(context, team, project)]
    : messages
}

type ThreadFileUploadTarget = Readonly<{
  inputId: string
  scopeKey: string
  teamId: string | null
  threadId: string
}>

const threadFileUploadTarget = (
  model: Model,
  context: RoomContext,
  team: Team | undefined,
  project: TeamProject | undefined,
): ThreadFileUploadTarget | undefined => {
  if (
    (context.kind === 'team' || context.kind === 'project') &&
    team === undefined
  ) {
    return undefined
  }

  if (context.kind === 'project' && project === undefined) {
    return undefined
  }

  const threadId =
    model.route._tag === 'Thread'
      ? model.route.threadId
      : context.kind === 'project' &&
          team !== undefined &&
          project !== undefined
        ? teamProjectChatThreadId(team.id, project.id)
        : context.kind === 'team' && team !== undefined
          ? teamChatThreadId(team.id)
          : personalChatThreadId(model)
  const scopeKey = threadFilesScopeKey(threadId)

  return {
    inputId: `thread-file-upload-${scopeKey.replace(/[^a-zA-Z0-9_-]/g, '-')}`,
    scopeKey,
    teamId:
      (context.kind === 'team' || context.kind === 'project') &&
      team !== undefined
        ? team.id
        : null,
    threadId,
  }
}

const threadFileUploadMessage = (
  target: ThreadFileUploadTarget,
  files: ReadonlyArray<File>,
): Message => {
  const file = files[0]

  if (file === undefined) {
    return FailedUploadThreadFile({
      error: 'Choose a file to upload.',
      scopeKey: target.scopeKey,
    })
  }

  return SubmittedThreadFileUpload({
    file,
    inputId: target.inputId,
    scopeKey: target.scopeKey,
    teamId: target.teamId,
    threadId: target.threadId,
  })
}

const fileUploadActions = (
  model: Model,
  context: RoomContext,
  team: Team | undefined,
  project: TeamProject | undefined,
): ReadonlyArray<Html> => {
  const h = html<Message>()
  const target = threadFileUploadTarget(model, context, team, project)

  if (target === undefined) {
    return []
  }

  const isUploading =
    model.threadFileUpload._tag === 'ThreadFileUploading' &&
    model.threadFileUpload.scopeKey === target.scopeKey
  const uploadStatus = isUploading
    ? 'Uploading'
    : model.threadFileUpload._tag === 'ThreadFileUploadSucceeded'
      ? model.threadFileUpload.message
      : model.threadFileUpload._tag === 'ThreadFileUploadFailed'
        ? model.threadFileUpload.error
        : undefined
  const statusTone =
    model.threadFileUpload._tag === 'ThreadFileUploadFailed'
      ? 'text-[#ff6f00]'
      : 'text-white/35'

  return [
    h.input([
      h.Id(target.inputId),
      h.Name('file'),
      h.Type('file'),
      h.AriaLabel('Upload file'),
      h.Attribute(
        'accept',
        'image/*,.pdf,.txt,.md,.markdown,.json,.yaml,.yml,.xml,.csv,.docx,.xlsx',
      ),
      Ui.className<Message>('sr-only'),
      h.OnFileChange(files => threadFileUploadMessage(target, files)),
    ]),
    h.label(
      [
        h.For(target.inputId),
        h.AriaLabel('Upload file'),
        Ui.className<Message>(
          'mr-1 inline-flex size-[26px] cursor-pointer items-center justify-center border border-[#333] bg-[#080808] text-white/60 hover:bg-[#141414] hover:text-[#f1efe8]',
        ),
      ],
      [iconView<Message>('Plus', 'size-4')],
    ),
    uploadStatus === undefined
      ? null
      : h.span(
          [
            Ui.className<Message>(
              `mr-1 max-w-[180px] overflow-hidden text-ellipsis whitespace-nowrap text-[0.6875rem] ${statusTone}`,
            ),
          ],
          [uploadStatus],
        ),
  ]
}

const composer = (
  model: Model,
  context: RoomContext,
  team: Team | undefined,
  project: TeamProject | undefined,
): Html => {
  const h = html<Message>()
  const isBusy = chatRunIsBusy(model)
  const canSubmit = model.chatComposerValue.trim() !== '' && !isBusy
  const target = threadFileUploadTarget(model, context, team, project)

  return Ui.workroomComposer<Message>({
    textareaId: CHAT_PROMPT_ID,
    value: model.chatComposerValue,
    isStreaming: isBusy,
    canSubmit,
    onSubmit: h.OnSubmit(SubmittedChatComposer()),
    onInput: h.OnInput(inputValue =>
      UpdatedChatComposer({ value: inputValue }),
    ),
    onKeyDown: h.OnKeyDownPreventDefault((key, modifiers) =>
      key === 'Enter' && !modifiers.shiftKey
        ? Option.some(SubmittedChatComposer())
        : Option.none(),
    ),
    actions: [...fileUploadActions(model, context, team, project)],
    attrs: [
      h.DataAttribute('component', 'session-prompt-dock'),
      ...(target === undefined
        ? []
        : [
            h.AllowDrop(),
            h.OnDropFiles(files => threadFileUploadMessage(target, files)),
          ]),
    ],
  })
}

const roomPanel = (
  model: Model,
  context: RoomContext,
  team: Team | undefined,
  project: TeamProject | undefined,
): Html | undefined => {
  const h = html<Message>()
  const target = threadFileUploadTarget(model, context, team, project)
  const scopeKey =
    target?.scopeKey ?? threadFilesScopeKey(personalChatThreadId(model))
  const uploadedFiles = model.threadFilesByScope[scopeKey] ?? []
  const projectRows =
    project?.agent === undefined
      ? []
      : [
          {
            label: 'Agent',
            value: h.span([], [project.agent.name]),
          },
          {
            label: 'Status',
            value: h.span([], [project.agent.status]),
          },
          {
            label: 'Scope',
            value: h.span([], [project.agent.scope]),
          },
          {
            label: 'Runtime',
            value: h.span([], [project.agent.runtime]),
          },
          {
            label: 'Repo',
            value: h.span([], [project.agent.repository]),
          },
          {
            label: 'Focus',
            value: h.span([], [project.agent.focus]),
          },
        ]
  const runRows =
    model.chatRun._tag === 'Active'
      ? [
          {
            label: 'Run',
            value: h.span([], [model.chatRun.metadata.status]),
          },
          {
            label: 'Tokens',
            value: h.span(
              [],
              [
                model.chatRun.metadata.tokenTotal === 0
                  ? 'pending'
                  : String(model.chatRun.metadata.tokenTotal),
              ],
            ),
          },
        ]
      : []
  const artifacts =
    model.chatRun._tag === 'Active' ? artifactNames(model.chatRun) : []
  const docks = [agentGoalDock(model), artanisOperatorDock(model),
    ...(model.chatRun._tag === 'Active'
      ? [
          Ui.workroomPanelActionRow<Message>({
            label: 'Run diagnostics',
            action: Ui.compactButton<Message>({
              label: 'i',
              attrs: [
                h.AriaLabel('Open run metadata'),
                h.OnClick(ClickedRunMetadataInfo()),
              ],
            }),
          }),
        ]
      : []),
  ].filter((dock): dock is Html => dock !== null)

  if (
    projectRows.length === 0 &&
    runRows.length === 0 &&
    uploadedFiles.length === 0 &&
    artifacts.length === 0 &&
    docks.length === 0
  ) {
    return undefined
  }

  return Ui.workroomFilePanel<Message>({
    tabs: [],
    rows: [...projectRows, ...runRows],
    files: [
      ...uploadedFiles.map((file: ThreadFileRecord) => ({
        label: file.filename,
        meta: formatBytes(file.sizeBytes),
      })),
      ...artifacts.map(artifact => ({
        label: artifact,
        meta: 'artifact',
        depth: 1 as const,
      })),
    ],
    docks,
    attrs: [h.DataAttribute('component', 'session-side-panel')],
  })
}

const runMetadataJson = (
  chatRun: Extract<ChatRun, { _tag: 'Active' }>,
): string =>
  JSON.stringify(
    {
      run: chatRun.metadata,
      events: chatRun.events,
    },
    null,
    2,
  )

const metadataDialog = (model: Model): Html => {
  const h = html<Message>()

  if (
    model.runMetadataDialog._tag !== 'Open' ||
    model.chatRun._tag !== 'Active'
  ) {
    return h.div([], [])
  }

  return Ui.workroomMetadataDialog<Message>({
    ariaLabel: 'Run metadata',
    body: runMetadataJson(model.chatRun),
    eyebrow: 'Diagnostics',
    title: 'Run metadata',
    actions: [
      Ui.compactButton<Message>({
        label: 'Close',
        attrs: [
          h.AriaLabel('Close run metadata'),
          h.OnClick(ClosedRunMetadataInfo()),
        ],
      }),
    ],
  })
}

const roomView = (model: Model, context: RoomContext): Html => {
  const team =
    context.kind === 'team' || context.kind === 'project'
      ? teamForRef(model, context.teamRef)
      : undefined
  const project = projectForContext(team, context)
  const main = Ui.workroomContent<Message>([
    Ui.workroomTimeline<Message>({
      messages: timelineMessages(model, context, team, project),
      endMarker: Ui.workroomTimelineEndMarker<Message>([
        html<Message>().DataAttribute('chat-timeline-end', 'true'),
      ]),
    }),
    composer(model, context, team, project),
  ])
  const side = roomPanel(model, context, team, project)

  return Ui.workroomContent<Message>(
    [
      side === undefined ? main : Ui.workroomSplit<Message>(main, side),
      metadataDialog(model),
    ],
    [html<Message>().DataAttribute('component', 'chat-workroom')],
  )
}

export const view = (model: Model): Html =>
  roomView(model, { kind: 'personal' })

export const teamRoomView = (model: Model, teamRef: string): Html =>
  roomView(model, { kind: 'team', teamRef })

export const teamProjectView = (
  model: Model,
  teamRef: string,
  projectRef: string,
): Html => roomView(model, { kind: 'project', projectRef, teamRef })
