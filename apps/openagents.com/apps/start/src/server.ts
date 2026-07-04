import { withStartRequestContext } from '@openagentsinc/effect-start'
import handler, { createServerEntry } from '@tanstack/react-start/server-entry'

type StartWorkerEnv = Record<string, unknown>

type StartExecutionContext = Readonly<{
  waitUntil(promise: Promise<unknown>): void
}>

export const SECURITY_HEADERS = {
  'X-Frame-Options': 'DENY',
  'X-Content-Type-Options': 'nosniff',
  'X-XSS-Protection': '1; mode=block',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'Permissions-Policy': 'camera=(), microphone=(), geolocation=()',
  'Strict-Transport-Security': 'max-age=31536000; includeSubDomains',
} as const

export function applySecurityHeaders(response: Response): Response {
  const headers = new Headers(response.headers)

  for (const [key, value] of Object.entries(SECURITY_HEADERS)) {
    headers.set(key, value)
  }

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}

const server = createServerEntry({
  async fetch(request) {
    const response = await handler.fetch(request, {
      responseLinkHeader: {
        filter: ({ phase }: { phase: 'static' | 'dynamic' }) =>
          phase === 'static',
      },
    })

    return applySecurityHeaders(response)
  },
})

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
