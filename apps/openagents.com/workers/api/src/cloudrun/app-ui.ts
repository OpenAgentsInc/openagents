/** Authenticated TanStack Start document mount for the `/app` namespace. */
import type { Tokens } from '@openauthjs/openauth/client'

import { appendSessionCookies } from '../auth-cookies'

type AppSession = Readonly<{
  tokens?: Tokens
}>

type AppUiDependencies<Env> = Readonly<{
  renderStart: (
    request: Request,
    env: Readonly<Record<string, unknown>>,
    ctx: ExecutionContext,
  ) => Promise<Response | undefined>
  verifySession: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Promise<AppSession | undefined>
}>

const isAppDocumentPath = (pathname: string): boolean =>
  pathname === '/app' || pathname === '/app/'

const mergeVaryCookie = (headers: Headers): void => {
  const values = (headers.get('vary') ?? '')
    .split(',')
    .map(value => value.trim())
    .filter(Boolean)

  if (!values.some(value => value.toLowerCase() === 'cookie')) {
    values.push('Cookie')
  }

  headers.set('vary', values.join(', '))
}

const privateResponse = (response: Response): Response => {
  const headers = new Headers(response.headers)
  headers.set('cache-control', 'private, no-store')
  mergeVaryCookie(headers)

  return new Response(response.body, {
    headers,
    status: response.status,
    statusText: response.statusText,
  })
}

const loginRedirect = (): Response =>
  privateResponse(
    new Response(null, {
      status: 302,
      headers: { location: '/login?returnTo=%2Fapp' },
    }),
  )

export const handleAppUiRequest = async <Env>(
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  dependencies: AppUiDependencies<Env>,
): Promise<Response | undefined> => {
  if (!isAppDocumentPath(new URL(request.url).pathname)) {
    return undefined
  }

  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return privateResponse(
      Response.json(
        { error: 'method_not_allowed' },
        { status: 405, headers: { allow: 'GET, HEAD' } },
      ),
    )
  }

  const session = await dependencies
    .verifySession(request, env, ctx)
    .catch(() => undefined)

  if (session === undefined) {
    return loginRedirect()
  }

  const rendered = await dependencies.renderStart(
    request,
    env as Readonly<Record<string, unknown>>,
    ctx,
  )
  if (rendered === undefined) {
    return privateResponse(
      Response.json({ error: 'app_unavailable' }, { status: 503 }),
    )
  }

  const response = privateResponse(rendered)
  if (session.tokens !== undefined) {
    appendSessionCookies(response.headers, session.tokens)
  }

  return response
}
