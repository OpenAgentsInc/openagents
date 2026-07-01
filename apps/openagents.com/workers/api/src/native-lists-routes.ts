// COORDINATOR WIRING:
// In workers/api/src/index.ts, construct the routes alongside the other route
// factories (near makeAutopilotDecisionRoutes), e.g.:
//
//   const nativeListsRoutes = makeNativeListsRoutes<WorkerBindings>({
//     makeStore: env => makeNativeListsService(openAgentsDatabase(env)),
//     requireOperator: (request, env) => requireAdminApiToken(request, env),
//   })
//
// Then chain it into the omni dispatch chain (routeOmniRequest), e.g.:
//
//   routeOmniRequest: (request, env, ctx) =>
//     omniRoutes.routeOmniRequest(request, env, ctx) ??
//     omniWorkroomRoutes.routeOmniWorkroomRequest(request, env, ctx) ??
//     omniWorkroomLifecycleRoutes.routeOmniWorkroomLifecycleRequest(
//       request,
//       env,
//       ctx,
//     ) ??
//     omniBundleRoutes.routeOmniBundleRequest(request, env, ctx) ??
//     nativeListsRoutes.routeNativeListsRequest(request, env, ctx),
//
// Also add the import near the other route imports:
//   import { makeNativeListsRoutes } from './native-lists-routes'
//   import { makeNativeListsService } from './native-lists'

import { readRequestJsonEffect } from '@openagentsinc/effect-boundary'
import { Effect, Match as M, Schema as S } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
} from './http/responses'
import type { NativeListsServiceShape } from './native-lists'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

type NativeListsRouteEnv = Readonly<Record<string, unknown>>

export type NativeListsRoutesDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => NativeListsServiceShape
  nowIso?: () => string
  requireOperator: (request: Request, env: Bindings) => Promise<boolean>
}>

const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

const LeadCaptureRequest = S.Struct({
  email: S.String,
  metadata: S.optionalKey(
    S.Record(S.String, S.Union([S.String, S.Number, S.Boolean, S.Null])),
  ),
  sourceRef: S.optionalKey(S.String),
})
type LeadCaptureRequest = typeof LeadCaptureRequest.Type

const routeNowIso = <Bindings>(
  dependencies: NativeListsRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const badRequest = (reason: string): HttpResponse =>
  noStoreJsonResponse(
    { error: 'native_lists_validation_error', reason },
    { status: 400 },
  )

const notFound = (): HttpResponse =>
  noStoreJsonResponse(
    { error: 'native_lists_not_found', reason: 'Subscriber list not found.' },
    { status: 404 },
  )

const listIdFromSubscribersPath = (pathname: string): string | undefined => {
  const match = /^\/api\/lists\/([^/]+)\/subscribers$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const listIdFromListPath = (pathname: string): string | undefined => {
  const match = /^\/api\/lists\/([^/]+)$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const decodeLeadCaptureRequest = (
  request: Request,
): Effect.Effect<LeadCaptureRequest, HttpResponse> =>
  readRequestJsonEffect(
    LeadCaptureRequest,
    request,
    'native_lists.lead_capture.body',
  ).pipe(
    Effect.mapError(error =>
      badRequest(
        error.reasonRef === 'boundary.json.malformed'
          ? 'Malformed JSON request body.'
          : 'Lead capture request did not match the expected schema.',
      ),
    ),
  )

// Public lead-capture endpoint: POST /api/lists/:listId/subscribers
const captureLead = <Bindings extends NativeListsRouteEnv>(
  dependencies: NativeListsRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  listId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const list = yield* Effect.promise(() => store.readList(listId))

    if (list === undefined || list.status !== 'active') {
      return notFound()
    }

    const body = yield* decodeLeadCaptureRequest(request)
    const email = body.email.trim().toLowerCase()

    if (!emailPattern.test(email)) {
      return badRequest('A valid email address is required.')
    }

    const result = yield* Effect.promise(() =>
      store.addSubscriber({
        email,
        listId,
        metadata: body.metadata,
        sourceRef:
          body.sourceRef === undefined || body.sourceRef.trim() === ''
            ? 'lead_capture.public'
            : body.sourceRef,
      }),
    )

    return noStoreJsonResponse(
      {
        generatedAt: nowIso,
        idempotent: result.idempotent,
        subscriber: {
          email: result.subscriber.email,
          listId: result.subscriber.listId,
          status: result.subscriber.status,
        },
      },
      { status: result.idempotent ? 200 : 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(error)))

// Operator read: GET /api/lists/:listId/subscribers
const readSubscribers = <Bindings extends NativeListsRouteEnv>(
  dependencies: NativeListsRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  listId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const authorized = yield* Effect.promise(() =>
      dependencies.requireOperator(request, env),
    )

    if (!authorized) {
      return forbidden()
    }

    const store = dependencies.makeStore(env)
    const list = yield* Effect.promise(() => store.readList(listId))

    if (list === undefined) {
      return notFound()
    }

    const subscribers = yield* Effect.promise(() =>
      store.listSubscribers({ listId }),
    )

    return noStoreJsonResponse({
      generatedAt: nowIso,
      list: {
        id: list.id,
        name: list.name,
        slug: list.slug,
        status: list.status,
      },
      subscribers: subscribers.map(subscriber => ({
        email: subscriber.email,
        sourceRef: subscriber.sourceRef,
        status: subscriber.status,
      })),
    })
  })

// Operator read: GET /api/lists/:listId
const readList = <Bindings extends NativeListsRouteEnv>(
  dependencies: NativeListsRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  listId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const authorized = yield* Effect.promise(() =>
      dependencies.requireOperator(request, env),
    )

    if (!authorized) {
      return forbidden()
    }

    const store = dependencies.makeStore(env)
    const list = yield* Effect.promise(() => store.readList(listId))

    if (list === undefined) {
      return notFound()
    }

    return noStoreJsonResponse({
      generatedAt: nowIso,
      list: {
        id: list.id,
        name: list.name,
        ownerUserId: list.ownerUserId,
        slug: list.slug,
        status: list.status,
        teamId: list.teamId,
      },
    })
  })

export const makeNativeListsRoutes = <Bindings extends NativeListsRouteEnv>(
  dependencies: NativeListsRoutesDependencies<Bindings>,
) => ({
  routeNativeListsRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    const subscribersListId = listIdFromSubscribersPath(url.pathname)

    if (subscribersListId !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          captureLead(dependencies, request, env, subscribersListId),
        ),
        M.when('GET', () =>
          readSubscribers(dependencies, request, env, subscribersListId),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET', 'POST']))),
      )
    }

    const listId = listIdFromListPath(url.pathname)

    if (listId !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () => readList(dependencies, request, env, listId)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    return undefined
  },
})
