import { Effect, Layer, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { readJsonObject } from './json-boundary'
import {
  TokenUsageLedger,
  type TokenUsageLedgerError,
  type TokenUsageLedgerRuntime,
  TokenUsageLedgerValidationError,
  systemTokenUsageLedgerRuntime,
} from './token-usage-ledger'

type TokenUsageLedgerEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type HttpResponse = globalThis.Response

type TokenUsageLedgerSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type TokenUsageLedgerRouteDependencies<
  Session extends TokenUsageLedgerSession,
  Bindings extends TokenUsageLedgerEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  ledgerLayer?: (
    env: Bindings,
    runtime: TokenUsageLedgerRuntime,
  ) => Layer.Layer<TokenUsageLedger>
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  runtime?: TokenUsageLedgerRuntime
}>

class TokenUsageLedgerRouteUnauthorized extends S.TaggedErrorClass<TokenUsageLedgerRouteUnauthorized>()(
  'TokenUsageLedgerRouteUnauthorized',
  {},
) {}

class TokenUsageLedgerRouteForbidden extends S.TaggedErrorClass<TokenUsageLedgerRouteForbidden>()(
  'TokenUsageLedgerRouteForbidden',
  {},
) {}

class TokenUsageLedgerRouteSessionError extends S.TaggedErrorClass<TokenUsageLedgerRouteSessionError>()(
  'TokenUsageLedgerRouteSessionError',
  {
    error: S.Defect,
  },
) {}

class TokenUsageLedgerRouteAuthError extends S.TaggedErrorClass<TokenUsageLedgerRouteAuthError>()(
  'TokenUsageLedgerRouteAuthError',
  {
    error: S.Defect,
  },
) {}

class TokenUsageLedgerRouteBodyError extends S.TaggedErrorClass<TokenUsageLedgerRouteBodyError>()(
  'TokenUsageLedgerRouteBodyError',
  {
    error: S.Defect,
  },
) {}

type TokenUsageLedgerRouteError =
  | TokenUsageLedgerError
  | TokenUsageLedgerRouteAuthError
  | TokenUsageLedgerRouteBodyError
  | TokenUsageLedgerRouteForbidden
  | TokenUsageLedgerRouteSessionError
  | TokenUsageLedgerRouteUnauthorized

const routeErrorResponse = (
  error: TokenUsageLedgerRouteError,
): HttpResponse => {
  switch (error._tag) {
    case 'TokenUsageLedgerRouteUnauthorized':
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    case 'TokenUsageLedgerRouteForbidden':
      return noStoreJsonResponse({ error: 'forbidden' }, { status: 403 })
    case 'TokenUsageLedgerRouteBodyError':
    case 'TokenUsageLedgerUnsafePayload':
    case 'TokenUsageLedgerValidationError':
      return noStoreJsonResponse(
        { error: 'invalid_token_usage_event' },
        { status: 400 },
      )
    case 'TokenUsageLedgerRouteAuthError':
    case 'TokenUsageLedgerRouteSessionError':
    case 'TokenUsageLedgerStorageError':
      return noStoreJsonResponse(
        { error: 'internal_server_error' },
        { status: 500 },
      )
  }
}

const requireSignedInSession = <
  Session extends TokenUsageLedgerSession,
  Bindings extends TokenUsageLedgerEnv,
>(
  dependencies: TokenUsageLedgerRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireBrowserSession(request, env, ctx),
      catch: error => new TokenUsageLedgerRouteSessionError({ error }),
    })

    if (session === undefined) {
      return yield* new TokenUsageLedgerRouteUnauthorized()
    }

    return session
  })

const requireAdminSession = <
  Session extends TokenUsageLedgerSession,
  Bindings extends TokenUsageLedgerEnv,
>(
  dependencies: TokenUsageLedgerRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireSignedInSession(
      dependencies,
      request,
      env,
      ctx,
    )

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new TokenUsageLedgerRouteForbidden()
    }

    return session
  })

const requireTrustedProducer = <
  Session extends TokenUsageLedgerSession,
  Bindings extends TokenUsageLedgerEnv,
>(
  dependencies: TokenUsageLedgerRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.gen(function* () {
    const authorized = yield* Effect.tryPromise({
      try: () => dependencies.requireAdminApiToken(request, env),
      catch: error => new TokenUsageLedgerRouteAuthError({ error }),
    })

    if (!authorized) {
      return yield* new TokenUsageLedgerRouteUnauthorized()
    }
  })

const readJsonBody = (
  request: Request,
): Effect.Effect<Record<string, unknown>, TokenUsageLedgerRouteBodyError> =>
  Effect.tryPromise({
    try: () => readJsonObject(request),
    catch: error => new TokenUsageLedgerRouteBodyError({ error }),
  })

const tokenUsageFilters = (
  request: Request,
): Effect.Effect<
  Readonly<{
    accountRef?: string
    actorTeamId?: string
    actorUserId?: string
    leaderboardEligible?: string
    model?: string
    privacyOptOut?: string
    producerSystem?: string
    provider?: string
    since?: string
    sourceRoute?: string
    until?: string
    usageTruth?: string
  }>,
  TokenUsageLedgerValidationError
> =>
  Effect.sync(() => {
    const url = new URL(request.url)
    const maybe = (name: string): string | undefined => {
      const value = url.searchParams.get(name)?.trim()

      return value === undefined || value === '' ? undefined : value
    }
    const filters: Record<string, string> = {}
    const add = (name: string): void => {
      const value = maybe(name)

      if (value !== undefined) {
        filters[name] = value
      }
    }

    add('accountRef')
    add('actorTeamId')
    add('actorUserId')
    add('leaderboardEligible')
    add('model')
    add('privacyOptOut')
    add('producerSystem')
    add('provider')
    add('since')
    add('sourceRoute')
    add('until')
    add('usageTruth')

    return filters
  })

const tokenUsageLeaderboardFilters = (
  request: Request,
): Readonly<{ until?: string; window?: string }> => {
  const url = new URL(request.url)
  const maybe = (name: string): string | undefined => {
    const value = url.searchParams.get(name)?.trim()

    return value === undefined || value === '' ? undefined : value
  }
  const filters: Record<string, string> = {}

  for (const name of ['until', 'window']) {
    const value = maybe(name)

    if (value !== undefined) {
      filters[name] = value
    }
  }

  return filters
}

const inferenceAnalyticsFilters = (
  request: Request,
): Readonly<{ window?: string }> => {
  const url = new URL(request.url)
  const value = url.searchParams.get('window')?.trim()

  return value === undefined || value === '' ? {} : { window: value }
}

const runRoute = (
  effect: Effect.Effect<
    HttpResponse,
    TokenUsageLedgerRouteError,
    TokenUsageLedger
  >,
  layer: Layer.Layer<TokenUsageLedger>,
): Effect.Effect<HttpResponse> =>
  effect.pipe(
    Effect.provide(layer),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

export const makeTokenUsageLedgerRoutes = <
  Session extends TokenUsageLedgerSession,
  Bindings extends TokenUsageLedgerEnv,
>(
  dependencies: TokenUsageLedgerRouteDependencies<Session, Bindings>,
) => {
  const runtime = dependencies.runtime ?? systemTokenUsageLedgerRuntime
  const ledgerLayer =
    dependencies.ledgerLayer ??
    ((env: Bindings, nextRuntime: TokenUsageLedgerRuntime) =>
      TokenUsageLedger.layer(env, nextRuntime))

  return {
    handleTokenUsageAggregateApi: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> => {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return runRoute(
        Effect.gen(function* () {
          const session = yield* requireAdminSession(
            dependencies,
            request,
            env,
            ctx,
          )
          const filters = yield* tokenUsageFilters(request)
          const ledger = yield* TokenUsageLedger
          const aggregate = yield* ledger.readAggregates(filters)

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse(aggregate),
            session,
          )
        }),
        ledgerLayer(env, runtime),
      )
    },

    // OWNER-GATED inference cost / provider-lane analytics (#6232). Returns
    // aggregate token + cost rollups (byProvider, byModel, byRoute, byDay,
    // totals) over the requested window. Admin/owner session ONLY — provider ids
    // and cost are internal, not public. Aggregate-only; no per-user material.
    handleInferenceAnalyticsApi: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> => {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return runRoute(
        Effect.gen(function* () {
          const session = yield* requireAdminSession(
            dependencies,
            request,
            env,
            ctx,
          )
          const ledger = yield* TokenUsageLedger
          const analytics = yield* ledger.readInferenceAnalytics(
            inferenceAnalyticsFilters(request),
          )

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse(analytics),
            session,
          )
        }),
        ledgerLayer(env, runtime),
      )
    },

    handleTokenUsageLeaderboardPreferenceApi: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> => {
      if (request.method !== 'GET' && request.method !== 'PUT') {
        return Effect.succeed(methodNotAllowed(['GET', 'PUT']))
      }

      return runRoute(
        Effect.gen(function* () {
          const session = yield* requireSignedInSession(
            dependencies,
            request,
            env,
            ctx,
          )
          const ledger = yield* TokenUsageLedger
          const input = {
            actorUserId: session.user.userId,
            subjectKind: 'user' as const,
            subjectRef: session.user.userId,
          }
          const preference =
            request.method === 'GET'
              ? yield* ledger.readLeaderboardPreference(input)
              : yield* ledger.updateLeaderboardPreference(
                  input,
                  yield* readJsonBody(request),
                )

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse(preference),
            session,
          )
        }),
        ledgerLayer(env, runtime),
      )
    },

    handleTokenUsageLeaderboardsApi: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> => {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return runRoute(
        Effect.gen(function* () {
          const session = yield* requireAdminSession(
            dependencies,
            request,
            env,
            ctx,
          )
          const ledger = yield* TokenUsageLedger
          const leaderboards = yield* ledger.readLeaderboards(
            tokenUsageLeaderboardFilters(request),
          )

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse(leaderboards),
            session,
          )
        }),
        ledgerLayer(env, runtime),
      )
    },

    handleTokenUsageEventsApi: (
      request: Request,
      env: Bindings,
    ): Effect.Effect<HttpResponse> => {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return runRoute(
        Effect.gen(function* () {
          yield* requireTrustedProducer(dependencies, request, env)
          const body = yield* readJsonBody(request)
          const ledger = yield* TokenUsageLedger
          const result = yield* ledger.ingestEvent(body)

          return noStoreJsonResponse(result, {
            status: result.inserted ? 201 : 200,
          })
        }),
        ledgerLayer(env, runtime),
      )
    },
  }
}
