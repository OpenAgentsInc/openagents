// MM-G2 (#8486) push notify-event ingest + per-user preference routes.
//
// `POST /api/internal/push/notify-events` — the seam Lane 0's org cloud
// executor (#8473-#8477) and metering (#8479) will call once merged. PIN:
// none of those are on `main` yet as of this writing, so this route's real
// end-to-end trigger is not yet wired anywhere — it exists as a typed,
// tested, ready-to-call ingest point. AUTH PIN: gated on the existing admin
// API bearer token (the same `requireAdminApiToken` many other internal/
// operator routes use) as the safe interim default, since the real caller's
// identity (an internal service? an agent-scoped bearer? the executor's own
// credential?) is not yet defined by #8473. This should very likely become a
// dedicated scoped credential once that infra exists — flagged, not solved,
// here.
//
// `GET|PUT /api/mobile/notifications/preferences` — mobile-bearer-authorized
// (the SAME boundary as `/api/mobile/push-tokens`), a global on/off toggle.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse, unauthorized } from '../http/responses'
import { optionalBoolean, readJsonObject } from '../json-boundary'
import { currentIsoTimestamp } from '../runtime-primitives'
import type { VerifiedSession } from '../auth/session'
import {
  listActivePushDeviceTokensForUser,
  type PushDeviceTokenDb,
  type PushDeviceTokenRow,
} from './push-device-tokens'
import {
  readPushNotificationPreference,
  writePushNotificationPreference,
} from './push-notification-preferences'
import {
  buildNotificationPayload,
  type RuntimeNotifyEvent,
  type RuntimeNotifyEventKind,
} from './push-notify-events'
import { buildExpoPushMessage, sendExpoPushMessages, type FetchLike } from './push-sender'
import type { MobileAccessRevocationStore } from '../auth/mobile-session'

type HttpResponse = globalThis.Response

export const PUSH_NOTIFY_EVENTS_PATH = '/api/internal/push/notify-events'
export const PUSH_NOTIFICATION_PREFERENCES_PATH = '/api/mobile/notifications/preferences'

const NOTIFY_EVENT_KINDS: ReadonlySet<string> = new Set([
  'turn_completed',
  'turn_needs_input',
  'turn_failed',
  'credit_low',
])

const isNotifyEventKind = (value: unknown): value is RuntimeNotifyEventKind =>
  typeof value === 'string' && NOTIFY_EVENT_KINDS.has(value)

const notifyEventFromBody = (
  body: Record<string, unknown>,
): RuntimeNotifyEvent | undefined => {
  const kind = body.kind
  const ownerUserId = typeof body.ownerUserId === 'string' ? body.ownerUserId.trim() : ''
  const threadId = typeof body.threadId === 'string' ? body.threadId.trim() : ''

  if (!isNotifyEventKind(kind) || ownerUserId.length === 0 || threadId.length === 0) {
    return undefined
  }

  const turnId = typeof body.turnId === 'string' ? body.turnId : undefined
  const branchUrl = typeof body.branchUrl === 'string' ? body.branchUrl : undefined
  const prUrl = typeof body.prUrl === 'string' ? body.prUrl : undefined
  const exhausted = typeof body.exhausted === 'boolean' ? body.exhausted : undefined

  return {
    kind,
    ownerUserId,
    threadId,
    ...(turnId === undefined ? {} : { turnId }),
    ...(branchUrl === undefined ? {} : { branchUrl }),
    ...(prUrl === undefined ? {} : { prUrl }),
    ...(exhausted === undefined ? {} : { exhausted }),
  }
}

export type PushNotifyRouteDependencies<Bindings, User = unknown> = Readonly<{
  /** CFG-4 Domain 4 (#8519): the Postgres-authoritative push registry +
   * preferences handle (`paymentsLedgerDbForEnv`), never a raw D1 handle. */
  db: (env: Bindings) => PushDeviceTokenDb
  authStorage: (env: Bindings) => MobileAccessRevocationStore
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  requireUserBearerSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<VerifiedSession<User> | undefined>
  userIdFromSession: (session: VerifiedSession<User>) => string
  nowIso?: () => string
  fetchImpl?: FetchLike
}>

export type NotifyEventOutcome = Readonly<{
  ok: true
  sent: number
  suppressedByPreference: boolean
  invalidatedTokens: ReadonlyArray<string>
}>

/** The reusable core: given an event + the active device rows for its
 * owner + that owner's preference, decide whether/what to send. Exported
 * standalone so a future direct in-process caller (once #8473 lands) can
 * skip the HTTP hop entirely. */
export const dispatchNotifyEvent = async (
  db: PushDeviceTokenDb,
  input: Readonly<{
    event: RuntimeNotifyEvent
    devices: ReadonlyArray<PushDeviceTokenRow>
    pushEnabled: boolean
    fetchImpl?: FetchLike
  }>,
): Promise<NotifyEventOutcome> => {
  if (!input.pushEnabled || input.devices.length === 0) {
    return { invalidatedTokens: [], ok: true, sent: 0, suppressedByPreference: !input.pushEnabled }
  }

  const payload = buildNotificationPayload(input.event)
  const messages = input.devices.map(device => buildExpoPushMessage(device.expoPushToken, payload))
  const result = await sendExpoPushMessages(db, messages, input.fetchImpl)
  const sentCount = result.tickets.filter(ticket => ticket.status === 'ok').length

  return {
    invalidatedTokens: result.invalidatedTokens,
    ok: true,
    sent: sentCount,
    suppressedByPreference: false,
  }
}

const routeNotifyEvents = async <Bindings, User>(
  dependencies: PushNotifyRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (!(await dependencies.requireAdminApiToken(request, env))) return unauthorized()

  let body: Record<string, unknown>
  try {
    body = await readJsonObject(request)
  } catch {
    return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
  }

  const event = notifyEventFromBody(body)
  if (event === undefined) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'kind, ownerUserId, and threadId are required' },
      { status: 400 },
    )
  }

  const db = dependencies.db(env)
  const authStorage = dependencies.authStorage(env)
  const [devices, preference] = await Promise.all([
    listActivePushDeviceTokensForUser(db, authStorage, event.ownerUserId),
    readPushNotificationPreference(db, event.ownerUserId),
  ])

  const outcome = await dispatchNotifyEvent(db, {
    devices,
    event,
    pushEnabled: preference.pushEnabled,
    ...(dependencies.fetchImpl === undefined ? {} : { fetchImpl: dependencies.fetchImpl }),
  })

  return noStoreJsonResponse(outcome, { status: 200 })
}

const routeReadPreference = async <Bindings, User>(
  dependencies: PushNotifyRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<HttpResponse> => {
  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) return unauthorized()

  const preference = await readPushNotificationPreference(
    dependencies.db(env),
    dependencies.userIdFromSession(session),
  )
  return noStoreJsonResponse({ ok: true, preference: { pushEnabled: preference.pushEnabled } })
}

const routeWritePreference = async <Bindings, User>(
  dependencies: PushNotifyRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<HttpResponse> => {
  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) return unauthorized()

  let body: Record<string, unknown>
  try {
    body = await readJsonObject(request)
  } catch {
    return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
  }

  const pushEnabled = optionalBoolean(body.pushEnabled)
  if (pushEnabled === undefined) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'pushEnabled (boolean) is required' },
      { status: 400 },
    )
  }

  const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
  const preference = await writePushNotificationPreference(dependencies.db(env), {
    nowIso,
    pushEnabled,
    userId: dependencies.userIdFromSession(session),
  })
  return noStoreJsonResponse({ ok: true, preference: { pushEnabled: preference.pushEnabled } })
}

export const handlePushNotifyEventsRequest = <Bindings, User>(
  dependencies: PushNotifyRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'POST') return methodNotAllowed(['POST'])
    return routeNotifyEvents(dependencies, request, env)
  })

export const handlePushNotificationPreferencesRequest = <Bindings, User>(
  dependencies: PushNotifyRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method === 'GET') return routeReadPreference(dependencies, request, env, ctx)
    if (request.method === 'PUT') return routeWritePreference(dependencies, request, env, ctx)
    return methodNotAllowed(['GET', 'PUT'])
  })
