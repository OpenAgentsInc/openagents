import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedLoadPrefilledWorkspace,
  Message,
  SucceededLoadPrefilledWorkspace,
} from '../message'
import {
  Model,
  PrefilledWorkspaceFailed,
  PrefilledWorkspaceLoaded,
  PrefilledWorkspaceLoading,
  PrefilledWorkspaceResponse,
} from '../model'
import { type UpdateReturn, noUpdate } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const LoadPrefilledWorkspace = Command.define(
  'LoadPrefilledWorkspace',
  { workspaceId: S.String },
  SucceededLoadPrefilledWorkspace,
  FailedLoadPrefilledWorkspace,
)(({ workspaceId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.prefilledWorkspace.load',
      request: `/api/workspaces/${encodeURIComponent(workspaceId)}`,
      schema: PrefilledWorkspaceResponse,
    })

    return SucceededLoadPrefilledWorkspace({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPrefilledWorkspace({
          error: errorMessageFromUnknown(error),
          workspaceId,
        }),
      ),
    ),
  ),
)

export const updatePrefilledWorkspace = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadPrefilledWorkspace: ({ workspaceId }) => [
        evo(model, {
          prefilledWorkspace: () => PrefilledWorkspaceLoading({ workspaceId }),
        }),
        [LoadPrefilledWorkspace({ workspaceId })],
        Option.none(),
      ],
      SucceededLoadPrefilledWorkspace: ({ response }) => [
        evo(model, {
          prefilledWorkspace: () =>
            PrefilledWorkspaceLoaded({
              generatedAt: response.generatedAt,
              viewer: response.viewer,
              workspace: response.workspace,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadPrefilledWorkspace: ({ error, workspaceId }) => [
        evo(model, {
          prefilledWorkspace: () =>
            PrefilledWorkspaceFailed({ error, workspaceId }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )
