import { Effect } from 'effect'

import { noStoreJsonResponse } from './http/responses'
import { type RouteEffect, routeEffectOrResponse } from './http/route-effects'
import {
  OpenAgentsWorkerRequest,
  type OpenAgentsWorkerRequestShape,
} from './runtime'

type TeamChatRouteDependencies = Readonly<{
  handleTeamChatMessagesApi: (
    context: Pick<OpenAgentsWorkerRequestShape, 'ctx' | 'env' | 'request'>,
    teamId: string,
    projectId?: string,
  ) => RouteEffect
}>

const decodeRouteSegment = (value: string): string | undefined => {
  try {
    return decodeURIComponent(value)
  } catch {
    return undefined
  }
}

export const makeTeamChatRoutes = (
  dependencies: TeamChatRouteDependencies,
) => ({
  routeTeamChatRequest: (): Effect.Effect<
    Response | undefined,
    never,
    OpenAgentsWorkerRequest
  > =>
    Effect.gen(function* () {
      const workerRequest = yield* OpenAgentsWorkerRequest
      const { url } = workerRequest
      const teamProjectChatMessagesMatch =
        /^\/api\/teams\/([^/]+)\/projects\/([^/]+)\/chat\/messages$/.exec(
          url.pathname,
        )

      if (teamProjectChatMessagesMatch !== null) {
        const encodedTeamId = teamProjectChatMessagesMatch[1]
        const encodedProjectId = teamProjectChatMessagesMatch[2]

        if (encodedTeamId !== undefined && encodedProjectId !== undefined) {
          const teamId = decodeRouteSegment(encodedTeamId)
          const projectId = decodeRouteSegment(encodedProjectId)

          if (teamId === undefined || projectId === undefined) {
            return yield* Effect.succeed(
              noStoreJsonResponse(
                {
                  error: 'bad_request',
                  reason: 'teamId or projectId is malformed',
                },
                { status: 400 },
              ),
            )
          }

          return yield* routeEffectOrResponse(
            dependencies.handleTeamChatMessagesApi(
              workerRequest,
              teamId,
              projectId,
            ),
          )
        }
      }

      const teamChatMessagesMatch =
        /^\/api\/teams\/([^/]+)\/chat\/messages$/.exec(url.pathname)

      if (teamChatMessagesMatch !== null) {
        const encodedTeamId = teamChatMessagesMatch[1]

        if (encodedTeamId !== undefined) {
          const teamId = decodeRouteSegment(encodedTeamId)

          if (teamId === undefined) {
            return yield* Effect.succeed(
              noStoreJsonResponse(
                { error: 'bad_request', reason: 'teamId is malformed' },
                { status: 400 },
              ),
            )
          }

          return yield* routeEffectOrResponse(
            dependencies.handleTeamChatMessagesApi(workerRequest, teamId),
          )
        }
      }

      return undefined
    }),
})
