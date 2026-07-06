import {
  AIUR_AUTH_STATE_COOKIE,
  AIUR_AUTH_STATE_MAX_AGE_SECONDS,
  appendClearSessionCookies,
  appendSessionCookies,
  expiredCookie,
  parseCookies,
  serializeCookie,
} from './cookies'
import { type AiurEnv } from './config'
import { type AiurAuthClientDeps, makeAiurAuthClient } from './session'

export const AIUR_LOGIN_START_PATH = '/auth/github/start'
export const AIUR_CALLBACK_PATH = '/auth/callback'
export const AIUR_LOGOUT_PATH = '/auth/logout'

const noStore = (response: Response): Response => {
  response.headers.set('cache-control', 'no-store')
  return response
}

const redirectResponse = (
  location: string,
  cookies: ReadonlyArray<string> = [],
): Response => {
  const headers = new Headers({ location })
  for (const cookie of cookies) {
    headers.append('set-cookie', cookie)
  }
  return new Response(null, { status: 302, headers })
}

const appOrigin = (request: Request): string => new URL(request.url).origin

export const handleAiurLoginStart = async (
  request: Request,
  env: AiurEnv,
  deps: AiurAuthClientDeps = {},
): Promise<Response> => {
  const redirectUri = `${appOrigin(request)}${AIUR_CALLBACK_PATH}`
  const { challenge, url } = await makeAiurAuthClient(env, deps).authorize(
    redirectUri,
    'code',
    { provider: 'github' },
  )

  return redirectResponse(url, [
    serializeCookie(
      AIUR_AUTH_STATE_COOKIE,
      challenge.state,
      AIUR_AUTH_STATE_MAX_AGE_SECONDS,
      '/auth',
    ),
  ])
}

export const handleAiurCallback = async (
  request: Request,
  env: AiurEnv,
  deps: AiurAuthClientDeps = {},
): Promise<Response> => {
  const url = new URL(request.url)
  const cookies = parseCookies(request)
  const cleanupCookies = [expiredCookie(AIUR_AUTH_STATE_COOKIE, '/auth')]

  if (url.searchParams.get('error') !== null) {
    return redirectResponse('/', cleanupCookies)
  }

  const code = url.searchParams.get('code')
  const state = url.searchParams.get('state')
  const expectedState = cookies.get(AIUR_AUTH_STATE_COOKIE)

  if (
    code === null ||
    state === null ||
    expectedState === undefined ||
    state !== expectedState
  ) {
    return redirectResponse('/', cleanupCookies)
  }

  const redirectUri = `${appOrigin(request)}${AIUR_CALLBACK_PATH}`
  const exchanged = await makeAiurAuthClient(env, deps)
    .exchange(code, redirectUri)
    .catch(() => undefined)

  if (exchanged === undefined || exchanged.err !== false) {
    return redirectResponse('/', cleanupCookies)
  }

  const response = redirectResponse('/', cleanupCookies)
  // Cookies are set for ANY successful OpenAuth exchange, owner or not —
  // the allowlist gate (owner-gate.ts) runs fresh on every subsequent
  // request and fails closed regardless of whether a session cookie is
  // present. Persisting the session here proves the real GitHub/OpenAuth
  // round trip works end to end without special-casing the deny path.
  appendSessionCookies(response.headers, exchanged.tokens)

  return response
}

export const handleAiurLogout = (): Response => {
  const response = redirectResponse('/')
  appendClearSessionCookies(response.headers)
  return response
}

export const routeAiurAuthRequest = (
  request: Request,
  env: AiurEnv,
  deps: AiurAuthClientDeps = {},
): Promise<Response> | undefined => {
  const path = new URL(request.url).pathname

  if (path === AIUR_LOGIN_START_PATH) {
    return handleAiurLoginStart(request, env, deps).then(noStore)
  }

  if (path === AIUR_CALLBACK_PATH) {
    return handleAiurCallback(request, env, deps).then(noStore)
  }

  if (path === AIUR_LOGOUT_PATH) {
    return Promise.resolve(noStore(handleAiurLogout()))
  }

  return undefined
}
