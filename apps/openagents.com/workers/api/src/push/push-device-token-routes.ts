// `POST|DELETE /api/mobile/push-tokens` — mobile-bearer-authorized push
// device-token register/unregister (MM-G1, #8485). Reuses the SAME mobile
// user bearer-session boundary as `GET /api/mobile/auth/session`
// (`auth/mobile-session.ts`'s `makeUserBearerSessionBoundary`) — never a
// browser session or agent token, since this is a per-device client
// capability, not something a server-side agent should ever register on a
// user's behalf.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse, unauthorized } from '../http/responses'
import { optionalString, readJsonObject } from '../json-boundary'
import { currentIsoTimestamp } from '../runtime-primitives'
import type { VerifiedSession } from '../auth/session'
import {
  registerPushDeviceToken,
  unregisterPushDeviceToken,
  type PushPlatform,
} from './push-device-tokens'

type HttpResponse = globalThis.Response

export const PUSH_DEVICE_TOKENS_PATH = '/api/mobile/push-tokens'
export const PUSH_DEVICE_TOKENS_ROUTE_REF = 'route.khala_mobile.push_device_tokens.v0_1'

export type PushDeviceTokenRouteDependencies<Bindings, User = unknown> = Readonly<{
  db: (env: Bindings) => D1Database
  nowIso?: () => string
  requireUserBearerSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<VerifiedSession<User> | undefined>
  /** Resolve the caller's stable OpenAgents user id from the verified
   * session's decoded subject shape. */
  userIdFromSession: (session: VerifiedSession<User>) => string
  /** The raw bearer access token from the request, so registration can
   * compute its revocation-lookup key (see `push-device-tokens.ts`). */
  readBearerToken: (request: Request) => string | undefined
}>

const isPlatform = (value: unknown): value is PushPlatform =>
  value === 'ios' || value === 'android'

const routeRegister = async <Bindings, User>(
  dependencies: PushDeviceTokenRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<HttpResponse> => {
  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) return unauthorized()

  const accessToken = dependencies.readBearerToken(request)
  if (accessToken === undefined) return unauthorized()

  let body: Record<string, unknown>
  try {
    body = await readJsonObject(request)
  } catch {
    return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
  }

  const deviceId = optionalString(body.deviceId)?.trim()
  const expoPushToken = optionalString(body.expoPushToken)?.trim()
  const platform = body.platform

  if (
    deviceId === undefined ||
    deviceId.length === 0 ||
    expoPushToken === undefined ||
    expoPushToken.length === 0 ||
    !isPlatform(platform)
  ) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'deviceId, expoPushToken, and platform (ios|android) are required' },
      { status: 400 },
    )
  }

  const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
  const row = await registerPushDeviceToken(dependencies.db(env), {
    accessToken,
    deviceId,
    expoPushToken,
    nowIso,
    platform,
    userId: dependencies.userIdFromSession(session),
  })

  return noStoreJsonResponse(
    {
      ok: true,
      registration: {
        deviceId: row.deviceId,
        platform: row.platform,
        updatedAt: row.updatedAt,
      },
    },
    { status: 200 },
  )
}

const routeUnregister = async <Bindings, User>(
  dependencies: PushDeviceTokenRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<HttpResponse> => {
  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) return unauthorized()

  const deviceId = new URL(request.url).searchParams.get('deviceId')?.trim()
  if (deviceId === undefined || deviceId === null || deviceId.length === 0) {
    return noStoreJsonResponse({ error: 'invalid_request', reason: 'deviceId query param is required' }, { status: 400 })
  }

  const outcome = await unregisterPushDeviceToken(dependencies.db(env), {
    deviceId,
    userId: dependencies.userIdFromSession(session),
  })

  return noStoreJsonResponse({ ok: true, removed: outcome.removed }, { status: 200 })
}

export const handlePushDeviceTokensRequest = <Bindings, User>(
  dependencies: PushDeviceTokenRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method === 'POST') return routeRegister(dependencies, request, env, ctx)
    if (request.method === 'DELETE') return routeUnregister(dependencies, request, env, ctx)
    return methodNotAllowed(['POST', 'DELETE'])
  })
