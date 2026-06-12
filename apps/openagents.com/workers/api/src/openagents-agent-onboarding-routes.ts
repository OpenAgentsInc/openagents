import { notFound } from '@openagentsinc/sync-worker'
import { Effect, Match as M } from 'effect'

import {
  OpenAgentsAgentOnboardingUnsafe,
  openAgentsAgentOnboardingMarkdownEffect,
} from './openagents-agent-onboarding'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export type OpenAgentsCompanionFilePath =
  | '/AGENTS-CORE.md'
  | '/HEARTBEAT.md'
  | '/RULES.md'
  | '/skill.json'

const markdownResponse = (body: string) =>
  new Response(body, {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/markdown; charset=utf-8',
    },
  })

const companionContentType = (path: OpenAgentsCompanionFilePath): string =>
  path === '/skill.json'
    ? 'application/json; charset=utf-8'
    : 'text/markdown; charset=utf-8'

const assetRequestFor = (request: Request, pathname = '/AGENTS.md') => {
  const url = new URL(request.url)
  url.pathname = pathname
  url.search = ''
  url.hash = ''

  return new Request(url, { method: 'GET' })
}

const readAssetMarkdown = (
  request: Request,
  assets: Fetcher | undefined,
): Effect.Effect<string | null> =>
  assets === undefined
    ? Effect.succeed(null)
    : Effect.tryPromise({
        catch: () =>
          new OpenAgentsAgentOnboardingUnsafe({
            reason: 'agent onboarding asset fetch failed',
          }),
        try: async () => {
          const response = await assets.fetch(assetRequestFor(request))

          return response.ok ? response.text() : null
        },
      }).pipe(Effect.catch(() => Effect.succeed(null)))

const routeErrorResponse = (error: OpenAgentsAgentOnboardingUnsafe) =>
  M.value(error).pipe(
    M.tags({
      OpenAgentsAgentOnboardingUnsafe: () =>
        noStoreJsonResponse(
          { error: 'openagents_agent_onboarding_unsafe' },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

export const handleOpenAgentsAgentOnboarding = (
  request: Request,
  assets?: Fetcher,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : readAssetMarkdown(request, assets).pipe(
        Effect.flatMap(assetMarkdown =>
          assetMarkdown === null
            ? openAgentsAgentOnboardingMarkdownEffect()
            : Effect.succeed(assetMarkdown),
        ),
        Effect.map(markdown => markdownResponse(markdown)),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )

export const handleOpenAgentsCompanionFile = (
  request: Request,
  assets: Fetcher | undefined,
  path: OpenAgentsCompanionFilePath,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : assets === undefined
      ? Effect.succeed(notFound())
      : Effect.tryPromise({
          catch: () =>
            new OpenAgentsAgentOnboardingUnsafe({
              reason: 'companion asset fetch failed',
            }),
          try: async () => assets.fetch(assetRequestFor(request, path)),
        }).pipe(
          Effect.flatMap(response =>
            response.ok
              ? Effect.promise(async () => {
                  const body = await response.text()

                  return new Response(body, {
                    headers: {
                      'cache-control': 'no-store',
                      'content-type': companionContentType(path),
                    },
                  })
                })
              : Effect.succeed(notFound()),
          ),
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        )
