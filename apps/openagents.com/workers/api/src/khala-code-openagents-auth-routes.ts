import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type OpenAuthAgentLinkRecord,
  createProgrammaticAgentRegistration,
  sha256Hex,
} from './agent-registration'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { type RouteEffect, routeEffect } from './http/route-effects'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'

type HttpResponse = globalThis.Response

type KhalaCodeOpenAgentsAuthEnv = Readonly<{
  AUTH_STORAGE: KVNamespace
}>

type KhalaCodeOpenAgentsAuthSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type KhalaCodeOpenAgentsAuthDependencies<
  Session extends KhalaCodeOpenAgentsAuthSession,
  Bindings extends KhalaCodeOpenAgentsAuthEnv,
> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  makeId?: () => string
  nowIso?: () => string
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

type KhalaCodeOpenAgentsAuthAttempt = Readonly<{
  schema: 'openagents.khala_code.desktop_auth_attempt.v1'
  agentCredentialId?: string
  agentToken?: string
  agentUserId?: string
  attemptId: string
  clientSecretHash: string
  createdAt: string
  expiresAt: string
  status: 'pending' | 'linked'
  tokenPrefix?: string
  userCode: string
}>

const ATTEMPT_TTL_SECONDS = 10 * 60
const POLL_INTERVAL_SECONDS = 2
const ATTEMPT_PREFIX = 'khala_code_desktop_openauth_'
const POLL_SECRET_PREFIX = 'khala_code_desktop_poll_'

export const KHALA_CODE_OPENAGENTS_AUTH_START_PATH =
  '/api/khala-code/auth/openagents/device/start'
export const KHALA_CODE_OPENAGENTS_AUTH_VERIFY_PATH =
  '/api/khala-code/auth/openagents/device/verify'

const attemptKey = (attemptId: string) =>
  `khala-code:desktop:openauth-agent-attempt:${attemptId}`

const makeUserCode = (seed: string): string => {
  const compact = seed
    .replace(new RegExp(`^${ATTEMPT_PREFIX}`), '')
    .replace(/[^A-Za-z0-9]/g, '')
    .toUpperCase()
    .slice(0, 8)
  const padded = compact.padEnd(8, 'X')
  return `${padded.slice(0, 4)}-${padded.slice(4, 8)}`
}

const nowIso = <
  Session extends KhalaCodeOpenAgentsAuthSession,
  Bindings extends KhalaCodeOpenAgentsAuthEnv,
>(
  dependencies: KhalaCodeOpenAgentsAuthDependencies<Session, Bindings>,
) => dependencies.nowIso?.() ?? currentIsoTimestamp()

const makeId = <
  Session extends KhalaCodeOpenAgentsAuthSession,
  Bindings extends KhalaCodeOpenAgentsAuthEnv,
>(
  dependencies: KhalaCodeOpenAgentsAuthDependencies<Session, Bindings>,
) => (dependencies.makeId ?? randomUuid)()

const isExpired = (
  attempt: KhalaCodeOpenAgentsAuthAttempt,
  now: string,
): boolean => Date.parse(attempt.expiresAt) <= Date.parse(now)

const readAttempt = async (
  kv: KVNamespace,
  attemptId: string,
): Promise<KhalaCodeOpenAgentsAuthAttempt | undefined> => {
  const raw = await kv.get(attemptKey(attemptId), 'json')
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    return undefined
  }
  const record = raw as Partial<KhalaCodeOpenAgentsAuthAttempt>
  return record.schema === 'openagents.khala_code.desktop_auth_attempt.v1' &&
    record.attemptId === attemptId &&
    typeof record.clientSecretHash === 'string' &&
    typeof record.createdAt === 'string' &&
    typeof record.expiresAt === 'string' &&
    (record.status === 'pending' || record.status === 'linked') &&
    typeof record.userCode === 'string' &&
    (record.agentCredentialId === undefined ||
      typeof record.agentCredentialId === 'string') &&
    (record.agentToken === undefined ||
      (typeof record.agentToken === 'string' &&
        record.agentToken.startsWith(AGENT_TOKEN_PREFIX))) &&
    (record.agentUserId === undefined || typeof record.agentUserId === 'string') &&
    (record.tokenPrefix === undefined || typeof record.tokenPrefix === 'string')
    ? (record as KhalaCodeOpenAgentsAuthAttempt)
    : undefined
}

const writeAttempt = (
  kv: KVNamespace,
  attempt: KhalaCodeOpenAgentsAuthAttempt,
): Promise<void> =>
  kv.put(attemptKey(attempt.attemptId), JSON.stringify(attempt), {
    expirationTtl: ATTEMPT_TTL_SECONDS,
  })

const originFor = (request: Request): string => new URL(request.url).origin

const verificationUrlFor = (
  request: Request,
  attempt: KhalaCodeOpenAgentsAuthAttempt,
): string => {
  const url = new URL(KHALA_CODE_OPENAGENTS_AUTH_VERIFY_PATH, originFor(request))
  url.searchParams.set('attempt', attempt.attemptId)
  url.searchParams.set('code', attempt.userCode)
  return url.toString()
}

const loginRedirectFor = (request: Request): HttpResponse => {
  const requestUrl = new URL(request.url)
  const loginUrl = new URL('/login/github', requestUrl.origin)
  loginUrl.searchParams.set(
    'returnTo',
    `${requestUrl.pathname}${requestUrl.search}`,
  )
  return Response.redirect(loginUrl.toString(), 302)
}

const htmlResponse = (body: string, init?: ResponseInit): HttpResponse =>
  new Response(`<!doctype html><html><body>${body}</body></html>`, {
    ...init,
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
      ...init?.headers,
    },
  })

const pollSecretFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('x-openagents-device-secret')?.trim()
  return value !== undefined &&
    value.length > POLL_SECRET_PREFIX.length &&
    value.startsWith(POLL_SECRET_PREFIX)
    ? value
    : undefined
}

const publicPendingProjection = (
  request: Request,
  attempt: KhalaCodeOpenAgentsAuthAttempt,
) => ({
  schema: 'openagents.khala_code.desktop_auth.openagents.v1',
  status: 'pending' as const,
  attemptId: attempt.attemptId,
  expiresAt: attempt.expiresAt,
  intervalSeconds: POLL_INTERVAL_SECONDS,
  userCode: attempt.userCode,
  verificationUrl: verificationUrlFor(request, attempt),
})

export const isKhalaCodeOpenAgentsAuthVerifyReturnPath = (
  url: URL,
): boolean => {
  if (url.pathname !== KHALA_CODE_OPENAGENTS_AUTH_VERIFY_PATH) return false
  const attempt = url.searchParams.get('attempt')?.trim()
  const code = url.searchParams.get('code')?.trim().toUpperCase()
  return attempt !== undefined &&
    code !== undefined &&
    new RegExp(`^${ATTEMPT_PREFIX}[A-Za-z0-9_-]+$`).test(attempt) &&
    /^[A-Z0-9]{4}-[A-Z0-9]{4}$/.test(code)
}

export const makeKhalaCodeOpenAgentsAuthHandlers = <
  Session extends KhalaCodeOpenAgentsAuthSession,
  Bindings extends KhalaCodeOpenAgentsAuthEnv,
>(
  dependencies: KhalaCodeOpenAgentsAuthDependencies<Session, Bindings>,
) => {
  const start = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const createdAt = nowIso(dependencies)
    const attemptId = `${ATTEMPT_PREFIX}${makeId(dependencies)}`
    const pollSecret = `${POLL_SECRET_PREFIX}${makeId(dependencies)}`
    const expiresAt = isoTimestampAfterIso(
      createdAt,
      ATTEMPT_TTL_SECONDS * 1000,
    )
    const attempt: KhalaCodeOpenAgentsAuthAttempt = {
      schema: 'openagents.khala_code.desktop_auth_attempt.v1',
      attemptId,
      clientSecretHash: await sha256Hex(pollSecret),
      createdAt,
      expiresAt,
      status: 'pending',
      userCode: makeUserCode(attemptId),
    }

    await writeAttempt(env.AUTH_STORAGE, attempt)

    return noStoreJsonResponse(
      {
        ...publicPendingProjection(request, attempt),
        pollSecret,
      },
      { status: 201 },
    )
  }

  const status = async (
    request: Request,
    env: Bindings,
    attemptId: string,
  ): Promise<HttpResponse> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const pollSecret = pollSecretFromRequest(request)
    if (pollSecret === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const attempt = await readAttempt(env.AUTH_STORAGE, attemptId)
    const secretHash = await sha256Hex(pollSecret)
    if (attempt === undefined || attempt.clientSecretHash !== secretHash) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const current = nowIso(dependencies)
    if (isExpired(attempt, current)) {
      return noStoreJsonResponse(
        {
          schema: 'openagents.khala_code.desktop_auth.openagents.v1',
          status: 'expired',
          attemptId,
        },
        { status: 410 },
      )
    }

    if (attempt.status !== 'linked') {
      return noStoreJsonResponse(publicPendingProjection(request, attempt))
    }

    if (
      attempt.agentToken === undefined ||
      attempt.tokenPrefix === undefined ||
      attempt.agentCredentialId === undefined ||
      attempt.agentUserId === undefined
    ) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse({
      schema: 'openagents.khala_code.desktop_auth.openagents.v1',
      status: 'linked',
      attemptId,
      agentToken: attempt.agentToken,
      linkedAgent: {
        credentialId: attempt.agentCredentialId,
        tokenPrefix: attempt.tokenPrefix,
        userId: attempt.agentUserId,
      },
    })
  }

  const verify = async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const url = new URL(request.url)
    const attemptId = url.searchParams.get('attempt')?.trim() ?? ''
    const userCode = url.searchParams.get('code')?.trim().toUpperCase() ?? ''
    if (
      !new RegExp(`^${ATTEMPT_PREFIX}[A-Za-z0-9_-]+$`).test(attemptId) ||
      !/^[A-Z0-9]{4}-[A-Z0-9]{4}$/.test(userCode)
    ) {
      return htmlResponse('<h1>Invalid Khala Code link</h1>', { status: 400 })
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)
    if (session === undefined) {
      return loginRedirectFor(request)
    }

    const attempt = await readAttempt(env.AUTH_STORAGE, attemptId)
    const current = nowIso(dependencies)
    if (attempt === undefined || isExpired(attempt, current)) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Khala Code link expired</h1>', { status: 410 }),
        session,
      )
    }

    if (attempt.userCode !== userCode) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Khala Code code mismatch</h1>', { status: 403 }),
        session,
      )
    }

    if (attempt.status === 'linked') {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse(
          '<h1>Khala Code connected</h1><p>You can return to Khala Code.</p>',
        ),
        session,
      )
    }

    const agentStore = dependencies.agentStore(env)
    if (agentStore.linkOpenAuthAgent === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Khala Code connect unavailable</h1>', {
          status: 500,
        }),
        session,
      )
    }

    const registration = await createProgrammaticAgentRegistration(
      agentStore,
      {
        displayName: 'Khala Code Desktop',
        externalId: `khala-code-desktop:${attempt.attemptId}`,
        metadata: {
          authFlow: 'desktop_device_auth',
          product: 'khala-code-desktop',
        },
      },
      {
        makeUuid: () => makeId(dependencies),
        now: () => current,
      },
    )

    const linkRecord: OpenAuthAgentLinkRecord = {
      agentCredentialId: registration.credential.id,
      agentUserId: registration.user.id,
      createdAt: current,
      id: `openauth_agent_link_${makeId(dependencies)}`,
      linkKind: 'credential_anchor',
      openauthUserId: session.user.userId,
      revokedAt: null,
      status: 'active',
      updatedAt: current,
    }

    await agentStore.linkOpenAuthAgent(linkRecord)
    await writeAttempt(env.AUTH_STORAGE, {
      ...attempt,
      agentCredentialId: registration.credential.id,
      agentToken: registration.credential.token,
      agentUserId: registration.user.id,
      status: 'linked',
      tokenPrefix: registration.credential.tokenPrefix,
    })

    return dependencies.appendRefreshedSessionCookies(
      htmlResponse(
        '<h1>Khala Code connected</h1><p>You can return to Khala Code.</p>',
      ),
      session,
    )
  }

  return {
    handleKhalaCodeOpenAgentsAuthStartApi: (
      request: Request,
      env: Bindings,
    ): RouteEffect =>
      routeEffect('khala_code_openagents_auth_start', () =>
        start(request, env),
      ),
    handleKhalaCodeOpenAgentsAuthStatusApi: (
      request: Request,
      env: Bindings,
      attemptId: string,
    ): RouteEffect =>
      routeEffect('khala_code_openagents_auth_status', () =>
        status(request, env, attemptId),
      ),
    handleKhalaCodeOpenAgentsAuthVerifyApi: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): RouteEffect =>
      routeEffect('khala_code_openagents_auth_verify', () =>
        verify(request, env, ctx),
      ),
  }
}
