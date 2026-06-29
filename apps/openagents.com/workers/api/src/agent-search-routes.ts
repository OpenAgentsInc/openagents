import { Effect } from 'effect'

import {
  type AgentRegistrationStore,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  AGENT_SEARCH_ENDPOINT,
  AGENT_SEARCH_ENTITLEMENT_HEADER,
  AGENT_SEARCH_PAYMENT_PREVIEW_ENDPOINT,
  AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT,
  AgentSearchPaymentRequired,
  AgentSearchProviderBudgetExceeded,
  AgentSearchQuotaExceeded,
  AgentSearchStorageError,
  type AgentSearchStore,
  AgentSearchValidationError,
  agentSearchErrorCode,
  executeAgentSearch,
  makeD1AgentSearchStore,
} from './agent-search'
import {
  type AgentSearchPaymentStore,
  makeD1AgentSearchPaymentStore,
  previewAgentSearchPayment,
  redeemAgentSearchPayment,
} from './agent-search-payments'
import {
  type OpenAgentsWorkerConfigEnv,
  getOpenAgentsWorkerConfig,
} from './config'
import {
  type ExaClientShape,
  ExaConfigurationDisabled,
  ExaProviderFetchError,
  ExaProviderHttpError,
  ExaProviderInvalidJson,
  ExaProviderSchemaError,
  ExaProviderTimeout,
  makeExaClient,
} from './exa'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
  unauthorized,
} from './http/responses'
import { readJsonObject } from './json-boundary'
import { openAgentsDatabase } from './runtime'
import {
  compactRandomId,
  currentEpochMillis,
  currentIsoTimestamp,
} from './runtime-primitives'

type HttpResponse = globalThis.Response

type AgentSearchRouteEnv = OpenAgentsWorkerConfigEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
  }>

type AgentSearchRouteDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  exaClient?: (
    env: Bindings,
    config: ReturnType<typeof getOpenAgentsWorkerConfig>,
  ) => ExaClientShape
  makeStore?: (env: Bindings) => AgentSearchStore
  makePaymentStore?: (env: Bindings) => AgentSearchPaymentStore
}>

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(/\s+/, 2)

  return scheme?.toLowerCase() === 'bearer' && token !== undefined
    ? token.trim()
    : undefined
}

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const key = request.headers.get('idempotency-key')?.trim()

  return key !== undefined && key.length >= 8 && key.length <= 200
    ? key
    : undefined
}

const badRequest = (reason: string): HttpResponse =>
  noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 })

const unprocessable = (reason: string): HttpResponse =>
  noStoreJsonResponse(
    { error: 'unprocessable_entity', reason },
    { status: 422 },
  )

const tooManyRequests = (error: AgentSearchQuotaExceeded): HttpResponse => {
  const headers = new Headers({
    'retry-after': Math.max(
      1,
      Math.ceil((Date.parse(error.resetAt) - currentEpochMillis()) / 1000),
    ).toString(),
    'x-openagents-rate-limit-reset': error.resetAt,
  })

  return noStoreJsonResponse(
    {
      error: 'agent_search_rate_limited',
      reason: error.message,
      resetAt: error.resetAt,
    },
    { headers, status: 429 },
  )
}

const unavailable = (reason: string, code = 'agent_search_unavailable') =>
  noStoreJsonResponse({ error: code, reason }, { status: 503 })

const paymentRequired = (error: AgentSearchPaymentRequired): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'payment_required',
      previewHref: error.previewHref,
      reason: error.message,
      requiredProductRefs: error.requiredProductRefs,
    },
    { status: 402 },
  )

const providerErrorResponse = (error: unknown): HttpResponse => {
  if (error instanceof ExaConfigurationDisabled) {
    return unavailable('Hosted search is not configured.', 'exa_disabled')
  }

  if (error instanceof ExaProviderHttpError) {
    return unavailable(
      error.status === 429
        ? 'Hosted search provider is rate limited.'
        : 'Hosted search provider returned an unavailable response.',
      'exa_provider_unavailable',
    )
  }

  if (error instanceof ExaProviderTimeout) {
    return unavailable('Hosted search provider timed out.', 'exa_timeout')
  }

  if (error instanceof ExaProviderFetchError) {
    return unavailable(
      'Hosted search provider could not be reached.',
      'exa_fetch_error',
    )
  }

  if (
    error instanceof ExaProviderInvalidJson ||
    error instanceof ExaProviderSchemaError
  ) {
    return unavailable(
      'Hosted search provider returned an invalid response.',
      'exa_invalid_response',
    )
  }

  return serverError()
}

const errorResponse = (error: unknown): HttpResponse => {
  if (error instanceof AgentSearchValidationError) {
    return unprocessable(error.message)
  }

  if (error instanceof AgentSearchPaymentRequired) {
    return paymentRequired(error)
  }

  if (error instanceof AgentSearchQuotaExceeded) {
    return tooManyRequests(error)
  }

  if (error instanceof AgentSearchProviderBudgetExceeded) {
    return unavailable(error.message, 'agent_search_provider_budget_exhausted')
  }

  if (error instanceof AgentSearchStorageError) {
    return serverError()
  }

  return providerErrorResponse(error)
}

const authenticateAgent = <Bindings extends AgentSearchRouteEnv>(
  dependencies: AgentSearchRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.gen(function* () {
    const token = bearerTokenFromRequest(request)

    if (token === undefined) {
      return undefined
    }

    return yield* Effect.tryPromise({
      catch: error => new AgentSearchStorageError('search.auth', error),
      try: () =>
        authenticateProgrammaticAgent(dependencies.agentStore(env), token),
    }).pipe(Effect.catch(() => Effect.void))
  })

const recordFailureMetric = (
  store: AgentSearchStore,
  input: Readonly<{
    actorRef: string
    credentialId: string
    error: unknown
  }>,
): Effect.Effect<void> =>
  Effect.tryPromise({
    catch: error => new AgentSearchStorageError('search.metric.failed', error),
    try: () =>
      store.recordMetric({
        actorRef: input.actorRef,
        cacheStatus: null,
        createdAt: currentIsoTimestamp(),
        credentialId: input.credentialId,
        durationMs: null,
        eventName: 'agent_search.failed',
        id: compactRandomId('agent_search_metric'),
        mode: 'basic',
        providerCostDollars: null,
        providerStatus: agentSearchErrorCode(input.error as never),
        resultCount: null,
      }),
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )

const handleAgentSearch = <Bindings extends AgentSearchRouteEnv>(
  dependencies: AgentSearchRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* authenticateAgent(dependencies, request, env)

    if (session === undefined) {
      return unauthorized()
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest(
        'Idempotency-Key header of 8-200 characters is required',
      )
    }

    const body = yield* Effect.tryPromise({
      catch: () => new AgentSearchValidationError('Request body must be JSON.'),
      try: () => readJsonObject(request),
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof AgentSearchValidationError
            ? unprocessable(error.message)
            : serverError(),
        ),
      ),
    )

    if (body instanceof Response) {
      return body
    }

    const config = getOpenAgentsWorkerConfig(env)
    const store =
      dependencies.makeStore?.(env) ??
      makeD1AgentSearchStore(openAgentsDatabase(env))
    const exaClient =
      dependencies.exaClient?.(env, config) ?? makeExaClient(config.exa)
    return yield* executeAgentSearch({
      body,
      config,
      exaClient,
      idempotencyKey,
      paidEntitlementRef:
        request.headers.get(AGENT_SEARCH_ENTITLEMENT_HEADER)?.trim() ||
        undefined,
      session,
      store,
    }).pipe(
      Effect.map(projection => noStoreJsonResponse(projection)),
      Effect.catch(error =>
        recordFailureMetric(store, {
          actorRef: `agent:${session.user.id}`,
          credentialId: session.credential.id,
          error,
        }).pipe(Effect.map(() => errorResponse(error))),
      ),
    )
  })

const handleAgentSearchPaymentPreview = <Bindings extends AgentSearchRouteEnv>(
  dependencies: AgentSearchRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* authenticateAgent(dependencies, request, env)

    if (session === undefined) {
      return unauthorized()
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest(
        'Idempotency-Key header of 8-200 characters is required',
      )
    }

    const body = yield* Effect.tryPromise({
      catch: () => new AgentSearchValidationError('Request body must be JSON.'),
      try: () => readJsonObject(request),
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof AgentSearchValidationError
            ? unprocessable(error.message)
            : serverError(),
        ),
      ),
    )

    if (body instanceof Response) {
      return body
    }

    const config = getOpenAgentsWorkerConfig(env)
    const store =
      dependencies.makePaymentStore?.(env) ??
      makeD1AgentSearchPaymentStore(openAgentsDatabase(env))

    return yield* Effect.tryPromise({
      catch: error =>
        error instanceof AgentSearchValidationError
          ? error
          : new AgentSearchStorageError('search.payment.preview', error),
      try: () =>
        previewAgentSearchPayment(store, {
          body,
          config,
          idempotencyKey,
          session,
        }),
    }).pipe(
      Effect.map(preview => noStoreJsonResponse({ preview })),
      Effect.catch(error => Effect.succeed(errorResponse(error))),
    )
  })

const handleAgentSearchPaymentRedeem = <Bindings extends AgentSearchRouteEnv>(
  dependencies: AgentSearchRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* authenticateAgent(dependencies, request, env)

    if (session === undefined) {
      return unauthorized()
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest(
        'Idempotency-Key header of 8-200 characters is required',
      )
    }

    const body = yield* Effect.tryPromise({
      catch: () => new AgentSearchValidationError('Request body must be JSON.'),
      try: () => readJsonObject(request),
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof AgentSearchValidationError
            ? unprocessable(error.message)
            : serverError(),
        ),
      ),
    )

    if (body instanceof Response) {
      return body
    }

    const store =
      dependencies.makePaymentStore?.(env) ??
      makeD1AgentSearchPaymentStore(openAgentsDatabase(env))

    return yield* Effect.tryPromise({
      catch: error =>
        error instanceof AgentSearchValidationError
          ? error
          : new AgentSearchStorageError('search.payment.redeem', error),
      try: () =>
        redeemAgentSearchPayment(store, {
          body,
          idempotencyKey,
          session,
        }),
    }).pipe(
      Effect.map(redemption => noStoreJsonResponse({ redemption })),
      Effect.catch(error => Effect.succeed(errorResponse(error))),
    )
  })

export const makeAgentSearchRoutes = <Bindings extends AgentSearchRouteEnv>(
  dependencies: AgentSearchRouteDependencies<Bindings>,
) => ({
  routeAgentSearchRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === AGENT_SEARCH_PAYMENT_PREVIEW_ENDPOINT) {
      return handleAgentSearchPaymentPreview(dependencies, request, env)
    }

    if (url.pathname === AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT) {
      return handleAgentSearchPaymentRedeem(dependencies, request, env)
    }

    if (url.pathname === AGENT_SEARCH_ENDPOINT) {
      return handleAgentSearch(dependencies, request, env)
    }

    return undefined
  },
})
