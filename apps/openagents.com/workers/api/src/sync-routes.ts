import { decodeSyncCommand } from '@openagentsinc/sync-schema'
import {
  SyncOutboxStore,
  SyncOutboxError as SyncOutboxStoreError,
  acceptedMutation,
  agentRunScope,
  badRequest,
  jsonResponse,
  notFound,
  personalWorkroomScope,
  publicAgentRunScope,
  publicAgentScope,
  publicGoalScope,
  publicGymRunProgressScope,
  publicKhalaTokensServedScope,
  publicSettledFeedScope,
  threadScope as syncThreadScope,
  teamScope,
} from '@openagentsinc/sync-worker'
import { Effect, Match as M, Schema as S } from 'effect'

import { routeAccessResponse } from './http/route-access-response'
import { noStoreJsonResponse } from './http/responses'
import type { Env } from './index'
import {
  syncOutboxStoreLayer,
  syncRoomNotifications,
  syncScope,
} from './runtime'
import { RouteAccessError } from './thread-access'

type SyncScopeKind =
  | 'agent-run'
  | 'public-agent'
  | 'public-agent-run'
  | 'public-goal'
  | 'public-gym-run-progress'
  | 'public-khala-tokens-served'
  | 'public-settled-feed'
  | 'team'
  | 'thread'
  | 'workspace'

export type ParsedSyncPath = Readonly<{
  action: 'mutate' | 'snapshot' | 'stream'
  id: string
  kind: SyncScopeKind
  scope: string
}>

class RequestDecodeError extends S.TaggedErrorClass<RequestDecodeError>()(
  'RequestDecodeError',
  {
    reason: S.String,
  },
) {}

class SyncRouteDependencyError extends S.TaggedErrorClass<SyncRouteDependencyError>()(
  'SyncRouteDependencyError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

class SyncRouteUnauthorized extends S.TaggedErrorClass<SyncRouteUnauthorized>()(
  'SyncRouteUnauthorized',
  {},
) {}

class SyncRouteAccessDenied extends S.TaggedErrorClass<SyncRouteAccessDenied>()(
  'SyncRouteAccessDenied',
  {
    error: RouteAccessError,
  },
) {}

class SyncStreamDispatchError extends S.TaggedErrorClass<SyncStreamDispatchError>()(
  'SyncStreamDispatchError',
  {
    error: S.Defect,
    scope: S.String,
  },
) {}

export const SyncRouteError = S.Union([
  RequestDecodeError,
  SyncRouteDependencyError,
  SyncRouteUnauthorized,
  SyncRouteAccessDenied,
  SyncStreamDispatchError,
  SyncOutboxStoreError,
])
export type SyncRouteError = typeof SyncRouteError.Type

type BrowserSessionShape = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type SyncRouteDependencies<Session extends BrowserSessionShape> = Readonly<{
  appendRefreshedSessionCookies: (
    response: Response,
    session: Session,
  ) => Response
  authorizeSyncPath: (
    env: Env,
    session: Session,
    syncPath: ParsedSyncPath,
  ) => Effect.Effect<RouteAccessError | undefined, unknown>
  requireBrowserSession: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Effect.Effect<Session | undefined, unknown>
}>

const syncScopeForPath = (
  kind: SyncScopeKind,
  id: string,
): string | undefined => {
  if (kind === 'workspace') {
    return personalWorkroomScope(id)
  }

  if (kind === 'team') {
    return teamScope(id)
  }

  if (kind === 'thread') {
    return syncThreadScope(id)
  }

  if (kind === 'agent-run') {
    return agentRunScope(id)
  }

  if (kind === 'public-agent') {
    return publicAgentScope(id)
  }

  if (kind === 'public-goal') {
    return publicGoalScope(id)
  }

  if (kind === 'public-agent-run') {
    return publicAgentRunScope(id)
  }

  if (kind === 'public-settled-feed') {
    return publicSettledFeedScope(id)
  }

  if (kind === 'public-khala-tokens-served') {
    return publicKhalaTokensServedScope(id)
  }

  if (kind === 'public-gym-run-progress') {
    return publicGymRunProgressScope(id)
  }

  return undefined
}

const optionalSyncScopeKind = (value: string): SyncScopeKind | undefined =>
  value === 'workspace' ||
  value === 'team' ||
  value === 'thread' ||
  value === 'agent-run' ||
  value === 'public-agent' ||
  value === 'public-goal' ||
  value === 'public-agent-run' ||
  value === 'public-gym-run-progress' ||
  value === 'public-khala-tokens-served' ||
  value === 'public-settled-feed'
    ? value
    : undefined

const optionalSyncAction = (
  value: string,
): ParsedSyncPath['action'] | undefined =>
  value === 'snapshot' || value === 'stream' || value === 'mutate'
    ? value
    : undefined

const parseSyncPath = (url: URL): ParsedSyncPath | undefined => {
  const match = /^\/api\/sync\/([^/]+)\/([^/]+)\/([^/]+)$/.exec(url.pathname)

  if (match === null) {
    return undefined
  }

  const [, rawKind, encodedId, rawAction] = match

  if (
    rawKind === undefined ||
    encodedId === undefined ||
    rawAction === undefined
  ) {
    return undefined
  }

  const kind = optionalSyncScopeKind(rawKind)
  const action = optionalSyncAction(rawAction)

  if (kind === undefined || action === undefined) {
    return undefined
  }

  let id: string

  try {
    id = decodeURIComponent(encodedId)
  } catch {
    return undefined
  }

  const scope = syncScopeForPath(kind, id)

  return scope === undefined ? undefined : { action, id, kind, scope }
}

const decodeRequestCommand = (request: Request) =>
  Effect.gen(function* () {
    const body = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: error =>
        new RequestDecodeError({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
    })

    return yield* decodeSyncCommand(body).pipe(
      Effect.mapError(
        error => new RequestDecodeError({ reason: String(error) }),
      ),
    )
  })

const syncRouteDependency = <A>(
  operation: string,
  effect: Effect.Effect<A, unknown>,
): Effect.Effect<A, SyncRouteDependencyError> =>
  Effect.mapError(
    effect,
    error => new SyncRouteDependencyError({ error, operation }),
  )

const syncRouteServerError = () =>
  noStoreJsonResponse({ error: 'sync_failed' }, { status: 500 })

const syncRouteConflictResponse = () =>
  noStoreJsonResponse({ error: 'sync_conflict' }, { status: 409 })

const syncRouteErrorResponse = (error: SyncRouteError) =>
  M.value(error).pipe(
    M.tagsExhaustive({
      RequestDecodeError: ({ reason }) => badRequest(reason),
      SyncChangeMissing: syncRouteServerError,
      SyncMutationAlreadyAccepted: syncRouteConflictResponse,
      SyncMutationAlreadyRejected: syncRouteConflictResponse,
      SyncOutboxStorageError: syncRouteServerError,
      SyncPayloadDecodeError: syncRouteServerError,
      SyncPayloadEncodeError: syncRouteServerError,
      SyncRouteAccessDenied: ({ error }) =>
        routeAccessResponse(error, { surface: 'api' }),
      SyncRouteDependencyError: syncRouteServerError,
      SyncRouteUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      SyncScopeMismatch: ({ expectedScope }) =>
        jsonResponse(
          { error: 'scope_mismatch', expectedScope },
          { status: 409 },
        ),
      SyncSequenceAllocationFailed: syncRouteServerError,
      SyncSnapshotMissing: syncRouteServerError,
      SyncStreamDispatchError: syncRouteServerError,
    }),
  )

const handleSnapshot = (scope: string) =>
  Effect.gen(function* () {
    const store = yield* SyncOutboxStore
    const snapshot = yield* store.readSnapshot(scope)

    return jsonResponse(snapshot)
  })

const handleStream = (request: Request, env: Env, scope: string) =>
  Effect.tryPromise({
    catch: error => new SyncStreamDispatchError({ error, scope }),
    try: () => {
      const headers = new Headers(request.headers)
      headers.set('x-openagents-sync-scope', scope)
      const syncRequest = new Request(request, { headers })
      const notifications = syncRoomNotifications(env)
      const id = syncScope(scope)
      const room = notifications.roomForScope(id)

      return room.fetch(syncRequest)
    },
  })

const handleMutation = (request: Request, scope: string, actorId: string) =>
  Effect.gen(function* () {
    const command = yield* decodeRequestCommand(request)
    const store = yield* SyncOutboxStore

    yield* store.acceptMutationForScope(scope, command, actorId)

    return jsonResponse(acceptedMutation(command), { status: 202 })
  })

const isPublicSyncPath = (syncPath: ParsedSyncPath): boolean =>
  syncPath.kind === 'public-agent' ||
  syncPath.kind === 'public-goal' ||
  syncPath.kind === 'public-agent-run' ||
  syncPath.kind === 'public-gym-run-progress' ||
  syncPath.kind === 'public-khala-tokens-served' ||
  syncPath.kind === 'public-settled-feed'

const handlePublicSyncRequest = (
  request: Request,
  env: Parameters<typeof handleStream>[1],
  syncPath: ParsedSyncPath,
) =>
  Effect.gen(function* () {
    if (request.method === 'GET' && syncPath.action === 'snapshot') {
      return yield* handleSnapshot(syncPath.scope)
    }

    if (request.method === 'GET' && syncPath.action === 'stream') {
      return yield* handleStream(request, env, syncPath.scope)
    }

    return notFound()
  })

const handleAuthorizedSyncRequest = <Session extends BrowserSessionShape>(
  dependencies: SyncRouteDependencies<Session>,
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  syncPath: ParsedSyncPath,
) =>
  Effect.gen(function* () {
    const session = yield* syncRouteDependency(
      'sync.require_browser_session',
      dependencies.requireBrowserSession(request, env, ctx),
    )

    if (session === undefined) {
      return yield* new SyncRouteUnauthorized()
    }

    const accessError = yield* syncRouteDependency(
      'sync.authorize_path',
      dependencies.authorizeSyncPath(env, session, syncPath),
    )

    if (accessError !== undefined) {
      return yield* new SyncRouteAccessDenied({ error: accessError })
    }

    if (request.method === 'GET' && syncPath.action === 'snapshot') {
      const snapshotResponse = yield* handleSnapshot(syncPath.scope)

      return dependencies.appendRefreshedSessionCookies(
        snapshotResponse,
        session,
      )
    }

    if (request.method === 'GET' && syncPath.action === 'stream') {
      return yield* handleStream(request, env, syncPath.scope)
    }

    if (request.method === 'POST' && syncPath.action === 'mutate') {
      const mutationResponse = yield* handleMutation(
        request,
        syncPath.scope,
        session.user.userId,
      )

      return dependencies.appendRefreshedSessionCookies(
        mutationResponse,
        session,
      )
    }

    return notFound()
  })

export const makeSyncRoutes = <Session extends BrowserSessionShape>(
  dependencies: SyncRouteDependencies<Session>,
) => ({
  routeSyncRequest: (request: Request, env: Env, ctx: ExecutionContext) => {
    const url = new URL(request.url)

    if (url.pathname === '/api/health') {
      return Effect.succeed(jsonResponse({ ok: true }))
    }

    const syncPath = parseSyncPath(url)

    if (syncPath === undefined) {
      return Effect.succeed(notFound())
    }

    if (isPublicSyncPath(syncPath)) {
      return handlePublicSyncRequest(request, env, syncPath).pipe(
        Effect.provide(syncOutboxStoreLayer(env)),
        Effect.match({
          onFailure: syncRouteErrorResponse,
          onSuccess: response => response,
        }),
      )
    }

    return handleAuthorizedSyncRequest(
      dependencies,
      request,
      env,
      ctx,
      syncPath,
    ).pipe(
      Effect.provide(syncOutboxStoreLayer(env)),
      Effect.match({
        onFailure: syncRouteErrorResponse,
        onSuccess: response => response,
      }),
    )
  },
})
