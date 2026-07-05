import {
  type AgentRegistrationStore,
  type OpenAuthAgentLinkRecord,
  authenticateProgrammaticAgent,
  sha256Hex,
} from './agent-registration'
import {
  readAgentBearerToken as bearerTokenFromRequest,
} from './auth/bearer-token'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { type RouteEffect, routeEffect } from './http/route-effects'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'

type HttpResponse = globalThis.Response

type PylonOpenAgentsAuthEnv = Readonly<{
  AUTH_STORAGE: KVNamespace
}>

type PylonOpenAgentsAuthSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type PylonOpenAgentsAuthDependencies<
  Session extends PylonOpenAgentsAuthSession,
  Bindings extends PylonOpenAgentsAuthEnv,
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

type PylonOpenAgentsAuthAttempt = Readonly<{
  schema: 'openagents.pylon.auth_link_attempt.v1'
  agentCredentialId: string
  agentDisplayName: string
  agentUserId: string
  attemptId: string
  createdAt: string
  expiresAt: string
  status: 'pending' | 'linked'
  tokenHash: string
  tokenPrefix: string
  userCode: string
}>

const ATTEMPT_TTL_SECONDS = 10 * 60
const POLL_INTERVAL_SECONDS = 2
const attemptKey = (attemptId: string) =>
  `pylon:openauth-agent-link-attempt:${attemptId}`

const makeUserCode = (seed: string): string => {
  // Derive the code from the RANDOM part of the attempt id. The attempt id is
  // always `pylon_openauth_<uuid>`, so slicing the literal prefix yielded the
  // constant "PYLO-NOPE" for every attempt. Strip the prefix + all dashes first
  // so the code comes from the uuid's entropy.
  const compact = seed
    .replace(/^pylon_openauth_/, '')
    .replace(/[^A-Za-z0-9]/g, '')
    .toUpperCase()
    .slice(0, 8)
  const padded = compact.padEnd(8, 'X')
  return `${padded.slice(0, 4)}-${padded.slice(4, 8)}`
}

const nowIso = <
  Session extends PylonOpenAgentsAuthSession,
  Bindings extends PylonOpenAgentsAuthEnv,
>(
  dependencies: PylonOpenAgentsAuthDependencies<Session, Bindings>,
) => dependencies.nowIso?.() ?? currentIsoTimestamp()

const makeId = <
  Session extends PylonOpenAgentsAuthSession,
  Bindings extends PylonOpenAgentsAuthEnv,
>(
  dependencies: PylonOpenAgentsAuthDependencies<Session, Bindings>,
) => (dependencies.makeId ?? randomUuid)()

const isExpired = (attempt: PylonOpenAgentsAuthAttempt, now: string): boolean =>
  Date.parse(attempt.expiresAt) <= Date.parse(now)

const readAttempt = async (
  kv: KVNamespace,
  attemptId: string,
): Promise<PylonOpenAgentsAuthAttempt | undefined> => {
  const raw = await kv.get(attemptKey(attemptId), 'json')
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    return undefined
  }
  const record = raw as Partial<PylonOpenAgentsAuthAttempt>
  return record.schema === 'openagents.pylon.auth_link_attempt.v1' &&
    record.attemptId === attemptId &&
    typeof record.agentCredentialId === 'string' &&
    typeof record.agentDisplayName === 'string' &&
    typeof record.agentUserId === 'string' &&
    typeof record.createdAt === 'string' &&
    typeof record.expiresAt === 'string' &&
    (record.status === 'pending' || record.status === 'linked') &&
    typeof record.tokenHash === 'string' &&
    typeof record.tokenPrefix === 'string' &&
    typeof record.userCode === 'string'
    ? (record as PylonOpenAgentsAuthAttempt)
    : undefined
}

const writeAttempt = (
  kv: KVNamespace,
  attempt: PylonOpenAgentsAuthAttempt,
): Promise<void> =>
  kv.put(attemptKey(attempt.attemptId), JSON.stringify(attempt), {
    expirationTtl: ATTEMPT_TTL_SECONDS,
  })

const originFor = (request: Request): string => new URL(request.url).origin

const verificationUrlFor = (
  request: Request,
  attempt: PylonOpenAgentsAuthAttempt,
): string => {
  const url = new URL(
    '/api/pylon/auth/openagents/device/verify',
    originFor(request),
  )
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

const linkedProjection = (tokenPrefix: string) => ({
  schema: 'openagents.pylon.auth.openagents.v1',
  status: 'linked' as const,
  linkedAgent: {
    tokenPrefix,
  },
})

export const makePylonOpenAgentsAuthHandlers = <
  Session extends PylonOpenAgentsAuthSession,
  Bindings extends PylonOpenAgentsAuthEnv,
>(
  dependencies: PylonOpenAgentsAuthDependencies<Session, Bindings>,
) => {
  const start = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const token = bearerTokenFromRequest(request)
    if (token === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const store = dependencies.agentStore(env)
    const session = await authenticateProgrammaticAgent(store, token)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    if (
      session.credential.openauthUserId !== null &&
      session.credential.openauthUserId !== undefined
    ) {
      return noStoreJsonResponse(
        linkedProjection(session.credential.tokenPrefix),
      )
    }

    const createdAt = nowIso(dependencies)
    const attemptId = `pylon_openauth_${makeId(dependencies)}`
    const expiresAt = isoTimestampAfterIso(
      createdAt,
      ATTEMPT_TTL_SECONDS * 1000,
    )
    const attempt: PylonOpenAgentsAuthAttempt = {
      schema: 'openagents.pylon.auth_link_attempt.v1',
      agentCredentialId: session.credential.id,
      agentDisplayName: session.user.displayName,
      agentUserId: session.user.id,
      attemptId,
      createdAt,
      expiresAt,
      status: 'pending',
      tokenHash: await sha256Hex(token),
      tokenPrefix: session.credential.tokenPrefix,
      userCode: makeUserCode(attemptId),
    }

    await writeAttempt(env.AUTH_STORAGE, attempt)

    return noStoreJsonResponse(
      {
        schema: 'openagents.pylon.auth.openagents.v1',
        status: 'pending',
        attemptId: attempt.attemptId,
        expiresAt: attempt.expiresAt,
        intervalSeconds: POLL_INTERVAL_SECONDS,
        userCode: attempt.userCode,
        verificationUrl: verificationUrlFor(request, attempt),
        linkedAgent: {
          tokenPrefix: attempt.tokenPrefix,
        },
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

    const token = bearerTokenFromRequest(request)
    if (token === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const store = dependencies.agentStore(env)
    const session = await authenticateProgrammaticAgent(store, token)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    if (
      session.credential.openauthUserId !== null &&
      session.credential.openauthUserId !== undefined
    ) {
      return noStoreJsonResponse(
        linkedProjection(session.credential.tokenPrefix),
      )
    }

    const attempt = await readAttempt(env.AUTH_STORAGE, attemptId)
    const tokenHash = await sha256Hex(token)
    if (attempt === undefined || attempt.tokenHash !== tokenHash) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const current = nowIso(dependencies)
    if (isExpired(attempt, current)) {
      return noStoreJsonResponse(
        {
          schema: 'openagents.pylon.auth.openagents.v1',
          status: 'expired',
          attemptId,
        },
        { status: 410 },
      )
    }

    return noStoreJsonResponse({
      schema: 'openagents.pylon.auth.openagents.v1',
      status: attempt.status,
      attemptId,
      expiresAt: attempt.expiresAt,
      intervalSeconds: POLL_INTERVAL_SECONDS,
      linkedAgent: {
        tokenPrefix: attempt.tokenPrefix,
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
      !/^pylon_openauth_[A-Za-z0-9_-]+$/.test(attemptId) ||
      !/^[A-Z0-9]{4}-[A-Z0-9]{4}$/.test(userCode)
    ) {
      return htmlResponse('<h1>Invalid Pylon link</h1>', { status: 400 })
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)
    if (session === undefined) {
      return loginRedirectFor(request)
    }

    const attempt = await readAttempt(env.AUTH_STORAGE, attemptId)
    const current = nowIso(dependencies)
    if (attempt === undefined || isExpired(attempt, current)) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Pylon link expired</h1>', { status: 410 }),
        session,
      )
    }

    if (attempt.userCode !== userCode) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Pylon code mismatch</h1>', { status: 403 }),
        session,
      )
    }

    const lookup = await dependencies
      .agentStore(env)
      .findAgentByTokenHash(attempt.tokenHash, current)
    if (lookup === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Pylon credential not found</h1>', { status: 404 }),
        session,
      )
    }

    if (
      lookup.openauthUserId !== null &&
      lookup.openauthUserId !== undefined &&
      lookup.openauthUserId !== session.user.userId
    ) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Pylon already linked</h1>', { status: 403 }),
        session,
      )
    }

    const agentStore = dependencies.agentStore(env)
    if (agentStore.linkOpenAuthAgent === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        htmlResponse('<h1>Pylon link unavailable</h1>', { status: 500 }),
        session,
      )
    }

    const linkRecord: OpenAuthAgentLinkRecord = {
      agentCredentialId: attempt.agentCredentialId,
      agentUserId: attempt.agentUserId,
      createdAt: current,
      id: `openauth_agent_link_${makeId(dependencies)}`,
      linkKind: 'credential_anchor',
      openauthUserId: session.user.userId,
      revokedAt: null,
      status: 'active',
      updatedAt: current,
    }

    await agentStore.linkOpenAuthAgent(linkRecord)
    await writeAttempt(env.AUTH_STORAGE, { ...attempt, status: 'linked' })

    return dependencies.appendRefreshedSessionCookies(
      htmlResponse(
        '<h1>Pylon connected</h1><p>You can return to your terminal.</p>',
      ),
      session,
    )
  }

  return {
    handlePylonOpenAgentsAuthStartApi: (
      request: Request,
      env: Bindings,
    ): RouteEffect =>
      routeEffect('pylon_openagents_auth_start', () => start(request, env)),
    handlePylonOpenAgentsAuthStatusApi: (
      request: Request,
      env: Bindings,
      attemptId: string,
    ): RouteEffect =>
      routeEffect('pylon_openagents_auth_status', () =>
        status(request, env, attemptId),
      ),
    handlePylonOpenAgentsAuthVerifyApi: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): RouteEffect =>
      routeEffect('pylon_openagents_auth_verify', () =>
        verify(request, env, ctx),
      ),
  }
}
