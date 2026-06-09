import { Effect } from 'effect'

import { noStoreJsonResponse } from './http/responses'
import { type RouteEffect, routeEffectOrResponse } from './http/route-effects'
import type { Env } from './index'

type TeamChatRouteDependencies = Readonly<{
  handleTeamChatMessagesApi: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
    teamId: string,
    projectId?: string,
  ) => RouteEffect
}>

export const makeTeamChatRoutes = (
  dependencies: TeamChatRouteDependencies,
) => ({
  routeTeamChatRequest: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Effect.Effect<Response> | undefined => {
    const url = new URL(request.url)
    const teamProjectChatMessagesMatch =
      /^\/api\/teams\/([^/]+)\/projects\/([^/]+)\/chat\/messages$/.exec(
        url.pathname,
      )

    if (teamProjectChatMessagesMatch !== null) {
      const encodedTeamId = teamProjectChatMessagesMatch[1]
      const encodedProjectId = teamProjectChatMessagesMatch[2]

      if (encodedTeamId !== undefined && encodedProjectId !== undefined) {
        let teamId: string
        let projectId: string

        try {
          teamId = decodeURIComponent(encodedTeamId)
          projectId = decodeURIComponent(encodedProjectId)
        } catch {
          return Effect.succeed(
            noStoreJsonResponse(
              {
                error: 'bad_request',
                reason: 'teamId or projectId is malformed',
              },
              { status: 400 },
            ),
          )
        }

        return routeEffectOrResponse(
          dependencies.handleTeamChatMessagesApi(
            request,
            env,
            ctx,
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
        let teamId: string

        try {
          teamId = decodeURIComponent(encodedTeamId)
        } catch {
          return Effect.succeed(
            noStoreJsonResponse(
              { error: 'bad_request', reason: 'teamId is malformed' },
              { status: 400 },
            ),
          )
        }

        return routeEffectOrResponse(
          dependencies.handleTeamChatMessagesApi(request, env, ctx, teamId),
        )
      }
    }

    return undefined
  },
})
