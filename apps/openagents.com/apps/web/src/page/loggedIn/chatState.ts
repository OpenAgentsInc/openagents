import { evo } from 'foldkit/struct'

import { decodeUnknownWithSchema } from '../../json-boundary'
import { teamProjectWorkroomAllowed } from '../../product-policy'
import {
  Model,
  TeamChatMessageRecord,
  ThreadFileApiRecord,
  ThreadFileRecord,
  teamProjectRouteRef,
  teamRouteRef,
  threadFileOwnershipTeamId,
  threadFileRecordFromDto,
} from './model'
import { syncScopeId } from './sync/projection'

type TeamChatRouteContext = Readonly<{
  projectId?: string
  roomKey: string
  teamId: string
}>

export const personalChatThreadId = (model: Model): string =>
  `personal:${model.session.userId}:chat`

export const teamChatThreadId = (teamId: string): string =>
  `team:${teamId}:chat`

export const teamProjectChatThreadId = (
  teamId: string,
  projectId: string,
): string => `team:${teamId}:project:${projectId}:chat`

export const threadFilesScopeKey = (threadId: string): string =>
  `thread:${threadId}`

export const teamFilesScopeKey = (teamId: string): string =>
  `team-files:${teamId}`

const teamIdForRouteRef = (model: Model, teamRef: string): string | undefined =>
  model.auth.teams.find(team => teamRouteRef(team) === teamRef)?.id

const projectIdForRouteRef = (
  model: Model,
  teamId: string,
  projectRef: string,
): string | undefined =>
  model.auth.teams
    .find(team => team.id === teamId)
    ?.projects?.find(project => teamProjectRouteRef(project) === projectRef)?.id

export const teamChatRoomKey = (teamId: string, projectId?: string): string =>
  projectId === undefined ? teamId : `${teamId}:project:${projectId}`

export const teamChatMessagesHref = (
  teamId: string,
  projectId?: string,
): string =>
  projectId === undefined
    ? `/api/teams/${encodeURIComponent(teamId)}/chat/messages`
    : `/api/teams/${encodeURIComponent(teamId)}/projects/${encodeURIComponent(projectId)}/chat/messages`

export const teamChatRouteContext = (
  model: Model,
): TeamChatRouteContext | undefined => {
  if (model.route._tag === 'TeamChat') {
    const teamId = teamIdForRouteRef(model, model.route.teamRef)

    return teamId === undefined
      ? undefined
      : { roomKey: teamChatRoomKey(teamId), teamId }
  }

  if (model.route._tag === 'TeamProjectChat') {
    if (!teamProjectWorkroomAllowed(model.route)) {
      return undefined
    }

    const teamId = teamIdForRouteRef(model, model.route.teamRef)

    if (teamId === undefined) {
      return undefined
    }

    const projectId = projectIdForRouteRef(
      model,
      teamId,
      model.route.projectRef,
    )

    return projectId === undefined
      ? undefined
      : { projectId, roomKey: teamChatRoomKey(teamId, projectId), teamId }
  }

  return undefined
}

export const teamChatMessagesRequestForRoute = (
  model: Model,
): Readonly<{ href: string; roomKey: string; teamId: string }> | undefined => {
  const context = teamChatRouteContext(model)

  return context === undefined
    ? undefined
    : {
        href: teamChatMessagesHref(context.teamId, context.projectId),
        roomKey: context.roomKey,
        teamId: context.teamId,
      }
}

export const threadFilesRequestForRoute = (
  model: Model,
): Readonly<{ href: string; scopeKey: string }> | undefined => {
  if (model.route._tag === 'Chat') {
    const threadId = personalChatThreadId(model)

    return {
      href: `/api/thread-files?threadId=${encodeURIComponent(threadId)}`,
      scopeKey: threadFilesScopeKey(threadId),
    }
  }

  if (model.route._tag === 'Thread') {
    return {
      href: `/api/thread-files?threadId=${encodeURIComponent(model.route.threadId)}`,
      scopeKey: threadFilesScopeKey(model.route.threadId),
    }
  }

  if (
    model.route._tag === 'TeamChat' ||
    model.route._tag === 'TeamProjectChat'
  ) {
    const context = teamChatRouteContext(model)

    if (context === undefined) {
      return undefined
    }

    const threadId =
      context.projectId === undefined
        ? teamChatThreadId(context.teamId)
        : teamProjectChatThreadId(context.teamId, context.projectId)

    return {
      href: `/api/thread-files?teamId=${encodeURIComponent(context.teamId)}&threadId=${encodeURIComponent(threadId)}`,
      scopeKey: threadFilesScopeKey(threadId),
    }
  }

  if (model.route._tag === 'TeamFiles') {
    const teamId = teamIdForRouteRef(model, model.route.teamRef)

    return teamId === undefined
      ? undefined
      : {
          href: `/api/teams/${encodeURIComponent(teamId)}/files`,
          scopeKey: teamFilesScopeKey(teamId),
        }
  }

  return undefined
}

export const threadFileDetailRequestForRoute = (
  model: Model,
): Readonly<{ fileId: string; href: string }> | undefined => {
  if (model.route._tag === 'PersonalFile') {
    return {
      fileId: model.route.fileId,
      href: `/api/thread-files/${encodeURIComponent(model.route.fileId)}`,
    }
  }

  if (model.route._tag === 'TeamFile') {
    const teamId = teamIdForRouteRef(model, model.route.teamRef)

    if (teamId === undefined) {
      return undefined
    }

    return {
      fileId: model.route.fileId,
      href: `/api/thread-files/${encodeURIComponent(model.route.fileId)}?teamId=${encodeURIComponent(teamId)}`,
    }
  }

  return undefined
}

export const appendTeamChatMessage = (
  messages: ReadonlyArray<TeamChatMessageRecord>,
  message: TeamChatMessageRecord,
): ReadonlyArray<TeamChatMessageRecord> =>
  [...messages.filter(existing => existing.id !== message.id), message].sort(
    (left, right) => left.createdAt.localeCompare(right.createdAt),
  )

const appendThreadFile = (
  files: ReadonlyArray<ThreadFileRecord>,
  file: ThreadFileRecord,
): ReadonlyArray<ThreadFileRecord> =>
  [file, ...files.filter(existing => existing.id !== file.id)].sort(
    (left, right) => right.createdAt.localeCompare(left.createdAt),
  )

export const replaceThreadFileInScopes = (
  filesByScope: Record<string, ReadonlyArray<ThreadFileRecord>>,
  file: ThreadFileRecord,
): Record<string, ReadonlyArray<ThreadFileRecord>> =>
  Object.fromEntries(
    Object.entries(filesByScope).map(([scopeKey, files]) => [
      scopeKey,
      files.map(existing => (existing.id === file.id ? file : existing)),
    ]),
  )

const decodeTeamChatMessageSyncRecord = (
  value: unknown,
): TeamChatMessageRecord | undefined => {
  try {
    return decodeUnknownWithSchema(TeamChatMessageRecord, value)
  } catch {
    return undefined
  }
}

const decodeThreadFileSyncRecord = (
  value: unknown,
): ThreadFileRecord | undefined => {
  try {
    return threadFileRecordFromDto(
      decodeUnknownWithSchema(ThreadFileApiRecord, value),
    )
  } catch {
    return undefined
  }
}

const teamChatMessagesFromSyncCollections = (
  collections: Record<string, Record<string, unknown>> | undefined,
  teamId: string,
): ReadonlyArray<TeamChatMessageRecord> =>
  Object.values(collections?.team_chat_messages ?? {})
    .flatMap(value => {
      const message = decodeTeamChatMessageSyncRecord(value)

      return message === undefined || message.teamId !== teamId ? [] : [message]
    })
    .sort((left, right) => left.createdAt.localeCompare(right.createdAt))

const teamChatMessagesByRoomFromSyncCollections = (
  collections: Record<string, Record<string, unknown>> | undefined,
  teamId: string,
): Record<string, ReadonlyArray<TeamChatMessageRecord>> =>
  teamChatMessagesFromSyncCollections(collections, teamId).reduce<
    Record<string, ReadonlyArray<TeamChatMessageRecord>>
  >((byRoom, message) => {
    const roomKey = teamChatRoomKey(
      message.teamId,
      message.projectId ?? undefined,
    )

    return {
      ...byRoom,
      [roomKey]: appendTeamChatMessage(byRoom[roomKey] ?? [], message),
    }
  }, {})

const threadFilesFromSyncCollections = (
  collections: Record<string, Record<string, unknown>> | undefined,
  teamId: string,
): ReadonlyArray<ThreadFileRecord> =>
  Object.values(collections?.thread_files ?? {})
    .flatMap(value => {
      const file = decodeThreadFileSyncRecord(value)

      if (file === undefined) {
        return []
      }

      return threadFileOwnershipTeamId(file.ownership) !== teamId ? [] : [file]
    })
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt))

const threadFileScopesFromTeamFiles = (
  files: ReadonlyArray<ThreadFileRecord>,
): Record<string, ReadonlyArray<ThreadFileRecord>> =>
  files.reduce<Record<string, ReadonlyArray<ThreadFileRecord>>>(
    (scopes, file) => ({
      ...scopes,
      [threadFilesScopeKey(file.threadId)]: appendThreadFile(
        scopes[threadFilesScopeKey(file.threadId)] ?? [],
        file,
      ),
    }),
    {},
  )

export const applyTeamSyncCollections = (
  model: Model,
  scope: string,
  collections: Record<string, Record<string, unknown>> | undefined,
): Model => {
  if (!scope.startsWith('team:')) {
    return model
  }

  const teamId = syncScopeId(scope)
  const hasChatMessages = collections?.team_chat_messages !== undefined
  const hasThreadFiles = collections?.thread_files !== undefined
  const chatMessagesByRoom = teamChatMessagesByRoomFromSyncCollections(
    collections,
    teamId,
  )
  const threadFiles = threadFilesFromSyncCollections(collections, teamId)
  const threadFileScopes = threadFileScopesFromTeamFiles(threadFiles)

  return evo(model, {
    teamChatMessagesByTeam: messagesByTeam =>
      hasChatMessages
        ? {
            ...messagesByTeam,
            [teamId]: chatMessagesByRoom[teamId] ?? [],
            ...chatMessagesByRoom,
          }
        : messagesByTeam,
    threadFilesByScope: filesByScope =>
      hasThreadFiles
        ? {
            ...filesByScope,
            [teamFilesScopeKey(teamId)]: threadFiles,
            ...threadFileScopes,
          }
        : filesByScope,
  })
}
