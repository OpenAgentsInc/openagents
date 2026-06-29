import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import { teamChatMessagesHref } from '../chatState'
import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedLoadTeamChatMessages,
  FailedPostTeamChatMessage,
  SucceededLoadTeamChatMessages,
  SucceededPostTeamChatMessage,
} from '../message'
import {
  TeamChatMessagesResponse,
  TeamChatPostResponse,
} from '../model'

export const LoadTeamChatMessages = Command.define(
  'LoadTeamChatMessages',
  { href: S.String, roomKey: S.String, teamId: S.String },
  SucceededLoadTeamChatMessages,
  FailedLoadTeamChatMessages,
)(({ href, roomKey, teamId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.teamChat.messages.load',
      request: href,
      schema: TeamChatMessagesResponse,
    })

    return SucceededLoadTeamChatMessages({ response, roomKey, teamId })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadTeamChatMessages({
          error: errorMessageFromUnknown(error),
          roomKey,
          teamId,
        }),
      ),
    ),
  ),
)

export const PostTeamChatMessage = Command.define(
  'PostTeamChatMessage',
  {
    body: S.String,
    kind: S.Literals(['message', 'autopilot_intent', 'adjutant_intent']),
    prompt: S.optionalKey(S.String),
    projectId: S.optionalKey(S.String),
    requestId: S.String,
    roomKey: S.String,
    teamId: S.String,
  },
  SucceededPostTeamChatMessage,
  FailedPostTeamChatMessage,
)(({ body, kind, prompt, projectId, requestId, roomKey, teamId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({
          body,
          kind,
          ...(prompt === undefined ? {} : { prompt }),
        }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.teamChat.message.post',
      request: teamChatMessagesHref(teamId, projectId),
      schema: TeamChatPostResponse,
    })

    return SucceededPostTeamChatMessage({ requestId, response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedPostTeamChatMessage({
          error: errorMessageFromUnknown(error),
          requestId,
          roomKey,
          teamId,
        }),
      ),
    ),
  ),
)
