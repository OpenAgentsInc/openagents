import { Effect, Match as M, Schema as S } from 'effect'

import {
  PylonMarketplaceCreateJobIntakeRequest,
  type PylonMarketplaceJobStore,
  PylonMarketplaceStoreError,
  PylonMarketplaceTriageJobIntakeRequest,
  createPylonMarketplaceJobIntake,
  listPylonMarketplaceJobs,
  pylonMarketplaceStoreErrorFromUnknown,
  triagePylonMarketplaceJobIntake,
} from './pylon-marketplace-service'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
  randomUuid,
} from './runtime-primitives'

type HttpResponse = globalThis.Response

type OperatorPylonMarketplaceSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type OperatorPylonMarketplaceDependencies<
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  currentEpochMillis?: () => number
  isOpenAgentsAdminEmail: (email: string) => boolean
  makeId?: () => string
  makeStore: (env: Bindings) => PylonMarketplaceJobStore
  requireAdminApiToken?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

class OperatorPylonMarketplaceUnauthorized extends S.TaggedErrorClass<OperatorPylonMarketplaceUnauthorized>()(
  'OperatorPylonMarketplaceUnauthorized',
  {},
) {}

class OperatorPylonMarketplaceForbidden extends S.TaggedErrorClass<OperatorPylonMarketplaceForbidden>()(
  'OperatorPylonMarketplaceForbidden',
  {},
) {}

class OperatorPylonMarketplaceSessionError extends S.TaggedErrorClass<OperatorPylonMarketplaceSessionError>()(
  'OperatorPylonMarketplaceSessionError',
  {
    error: S.Defect,
  },
) {}

type OperatorPylonMarketplaceRouteError =
  | OperatorPylonMarketplaceForbidden
  | OperatorPylonMarketplaceSessionError
  | OperatorPylonMarketplaceUnauthorized
  | PylonMarketplaceStoreError

const routeErrorResponse = (
  error: OperatorPylonMarketplaceRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      OperatorPylonMarketplaceForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      OperatorPylonMarketplaceSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OperatorPylonMarketplaceUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      PylonMarketplaceStoreError: storeError =>
        noStoreJsonResponse(
          {
            error: `pylon_marketplace_${storeError.kind}`,
            reason: storeError.reason,
          },
          {
            status: storeError.kind === 'conflict'
              ? 409
              : storeError.kind === 'not_found'
                ? 404
                : storeError.kind === 'storage_error'
                  ? 500
                  : 400,
          },
        ),
    }),
    M.exhaustive,
  )

const requireAdminSession = <
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
>(
  dependencies: OperatorPylonMarketplaceDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new OperatorPylonMarketplaceSessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new OperatorPylonMarketplaceSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorPylonMarketplaceUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorPylonMarketplaceForbidden({})
    }

    return session
  })

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('Idempotency-Key')?.trim()

  return value === undefined || value === '' ? undefined : value
}

const decodeBody = <A>(
  request: Request,
  schema: S.Decoder<A>,
): Effect.Effect<A, PylonMarketplaceStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new PylonMarketplaceStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () => decodeUnknownWithSchema(schema, await readJsonObject(request)),
  })

const storeFor = <
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
>(
  dependencies: OperatorPylonMarketplaceDependencies<Session, Bindings>,
  env: Bindings,
): PylonMarketplaceJobStore =>
  dependencies.makeStore(env)

const nowIsoFor = <
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
>(
  dependencies: OperatorPylonMarketplaceDependencies<Session, Bindings>,
): string =>
  epochMillisToIsoTimestamp(
    (dependencies.currentEpochMillis ?? currentEpochMillis)(),
  )

const routeList = <
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
>(
  dependencies: OperatorPylonMarketplaceDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const projection = yield* Effect.tryPromise({
      catch: pylonMarketplaceStoreErrorFromUnknown,
      try: () =>
        listPylonMarketplaceJobs(storeFor(dependencies, env), {
          audience: 'operator',
          limit: 100,
          nowIso: nowIsoFor(dependencies),
        }),
    })

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        authority: {
          buyerChargeMutationAllowed: false,
          paidAssignmentDispatchAllowed: false,
          payoutMutationAllowed: false,
          settlementMutationAllowed: false,
        },
        liveDispatchAllowed: false,
        projection: projection.projection,
      }),
      session,
    )
  })

const routeCreate = <
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
>(
  dependencies: OperatorPylonMarketplaceDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return yield* new PylonMarketplaceStoreError({
        kind: 'validation_error',
        reason: 'Idempotency-Key header is required.',
      })
    }

    const body = yield* decodeBody(
      request,
      PylonMarketplaceCreateJobIntakeRequest,
    )
    const response = yield* Effect.tryPromise({
      catch: pylonMarketplaceStoreErrorFromUnknown,
      try: () =>
        createPylonMarketplaceJobIntake(
          storeFor(dependencies, env),
          body,
          {
            idempotencyKey,
            makeId: dependencies.makeId ?? randomUuid,
            nowIso: nowIsoFor(dependencies),
          },
        ),
    })

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(response, { status: response.idempotent ? 200 : 201 }),
      session,
    )
  })

const routeTriage = <
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
>(
  dependencies: OperatorPylonMarketplaceDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  intakeRef: string,
) =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return yield* new PylonMarketplaceStoreError({
        kind: 'validation_error',
        reason: 'Idempotency-Key header is required.',
      })
    }

    const body = yield* decodeBody(
      request,
      PylonMarketplaceTriageJobIntakeRequest,
    )
    const response = yield* Effect.tryPromise({
      catch: pylonMarketplaceStoreErrorFromUnknown,
      try: () =>
        triagePylonMarketplaceJobIntake(
          storeFor(dependencies, env),
          intakeRef,
          body,
          {
            idempotencyKey,
            makeId: dependencies.makeId ?? randomUuid,
            nowIso: nowIsoFor(dependencies),
          },
        ),
    })

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(response),
      session,
    )
  })

export const makeOperatorPylonMarketplaceRoutes = <
  Session extends OperatorPylonMarketplaceSession,
  Bindings,
>(
  dependencies: OperatorPylonMarketplaceDependencies<Session, Bindings>,
) => ({
  routeOperatorPylonMarketplaceRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/artanis/pylon-marketplace/jobs') {
      if (request.method === 'GET') {
        return routeList(dependencies, request, env, ctx).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        )
      }

      if (request.method === 'POST') {
        return routeCreate(dependencies, request, env, ctx).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        )
      }

      return Effect.succeed(methodNotAllowed(['GET', 'POST']))
    }

    const triageMatch =
      /^\/api\/operator\/artanis\/pylon-marketplace\/jobs\/([^/]+)\/triage$/
        .exec(url.pathname)

    if (triageMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeTriage(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(triageMatch[1]!),
      ).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    return undefined
  },
})
