import { getStartRequestContext, withStartRequestContext } from '@openagentsinc/effect-start'
import handler, { createServerEntry } from '@tanstack/react-start/server-entry'

import { type AiurEnv } from './auth/config'
import {
  applySecurityHeaders,
  routeAiurSharedSurfaceRequest,
  SECURITY_HEADERS,
} from './shared-surface'

export { applySecurityHeaders, SECURITY_HEADERS }

const currentEnv = (): AiurEnv =>
  getStartRequestContext<AiurEnv>()?.env ?? {}

/**
 * Worker-entry wrapper around `routeAiurSharedSurfaceRequest`
 * (`shared-surface.ts`) that reads `env` from the ambient Start request
 * context. The Cloud Run Bun entry (`cloudrun/server.ts`) calls the shared
 * router with an explicit `env` instead.
 */
export async function routeAiurSharedSurface(
  request: Request,
): Promise<Response | undefined> {
  return routeAiurSharedSurfaceRequest(request, currentEnv())
}

const server = createServerEntry({
  async fetch(request) {
    const sharedSurfaceResponse = await routeAiurSharedSurface(request)
    if (sharedSurfaceResponse !== undefined) {
      // A successful WebSocket upgrade (status 101) carries a
      // Workers-runtime `webSocket` pairing that a
      // `new Response(response.body, ...)` reconstruction (what
      // `applySecurityHeaders` does) would silently drop.
      return sharedSurfaceResponse.status === 101
        ? sharedSurfaceResponse
        : applySecurityHeaders(sharedSurfaceResponse)
    }

    const response = await handler.fetch(request, {
      responseLinkHeader: {
        filter: ({ phase }: { phase: 'static' | 'dynamic' }) =>
          phase === 'static',
      },
    })

    return applySecurityHeaders(response)
  },
})

type StartWorkerEnv = Record<string, unknown>

type StartExecutionContext = Readonly<{
  waitUntil(promise: Promise<unknown>): void
}>

export default {
  fetch(
    request: Request,
    env: StartWorkerEnv,
    executionCtx: StartExecutionContext,
  ) {
    return withStartRequestContext({ request, env, executionCtx }, () =>
      server.fetch(request),
    )
  },
}
