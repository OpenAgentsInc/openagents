import { Effect, Layer, Match as M, Schema as S } from 'effect'

import {
  expiredCookie,
  parseCookies,
} from '../auth-cookies'
import { readBearerToken } from '../auth/bearer-token'
import {
  type AgentRegistrationStore,
  makeD1AgentRegistrationStore,
} from '../agent-registration'
import {
  type CustomerOrderAgentContext,
  CustomerOrderAgentAuthFailure,
  type CustomerOrderAgentScope,
  authenticateCustomerOrderAgentRequest,
} from '../customer-order-agent-auth'
import {
  CreateCustomerOrderRequest,
  type CustomerOrderRuntime,
  CustomerOrderStorageError,
  CustomerOrderStore,
  SubmitCustomerSiteFeedbackRequest,
} from '../customer-orders'
import { businessDomainDatabaseForEnv } from '../business-domain-store'
import { methodNotAllowed, noStoreJsonResponse } from '../http/responses'
import { logWorkerRouteError } from '../observability'
import { openAgentsDatabase, scheduleBackgroundWork } from '../runtime'
import {
  PENDING_REFERRAL_COOKIE,
} from '../site-referrals'
import {
  type ReferralConsumptionResult,
  SiteReferralConsumptionStorageError,
  consumePendingReferralForUser,
  linkPendingReferralToOrder,
} from '../site-referral-attribution-consumption'
import type { OnboardingDripOrderState } from '../email-onboarding-drip'
import {
  GITHUB_REPOSITORY_DEFAULT_PER_PAGE,
  GITHUB_REPOSITORY_MAX_PER_PAGE,
  GitHubRepositoryListFailed,
  GitHubRepositoryReadFailed,
  GitHubRepositoryService,
  githubIdentityTokenKey,
} from './github'
import {
  OnboardingInvalidStep,
  type OnboardingRuntime,
  OnboardingStateStore,
  OnboardingStorageError,
  OnboardingUserNotFound,
  systemOnboardingRuntime,
} from './repository'
import {
  type OnboardingGitHubRepository,
  SelectOnboardingRepositoryRequest,
  SubmitOnboardingGoalRequest,
} from './schema'

type OnboardingEnv = Readonly<{
  AUTH_STORAGE: KVNamespace
  OPENAGENTS_DB: D1Database
}>

type OnboardingSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type CustomerOrderRouteActor<Session extends OnboardingSession> =
  | Readonly<{
      _tag: 'Agent'
      context: CustomerOrderAgentContext
      userId: string
    }>
  | Readonly<{
      _tag: 'BrowserSession'
      session: Session
      userId: string
    }>

type OnboardingRouteDependencies<
  Session extends OnboardingSession,
  RouteEnv extends OnboardingEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: Response,
    session: Session,
  ) => Response
  requireBrowserSession: (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  // Mobile-bearer session boundary (MM-B1, issue #8471) — the OpenAuth mobile
  // user bearer session verified the same way as `GET /api/mobile/auth/session`
  // (see `auth/mobile-session.ts`'s `makeUserBearerSessionBoundary`). Distinct
  // from `requireBrowserSession` above: no cookies are read or refreshed for
  // this boundary, since mobile clients carry the OpenAuth access token as a
  // bearer header, not a cookie.
  requireUserBearerSession: (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  customerOrderRuntime?: CustomerOrderRuntime
  runtime?: OnboardingRuntime
  // Test-only override of the real (network-calling) GitHub repository
  // service layer. Defaults to `GitHubRepositoryService.layer` (real GitHub
  // API calls) in production; tests inject a fake layer to exercise the
  // available/paginated/expired-token mobile-repo paths without a live
  // network call.
  githubRepositoryServiceLayer?: Layer.Layer<GitHubRepositoryService>
  siteReferralOnboarding?: (
    input: Readonly<{
      env: RouteEnv
      orderId: string | null
      orderState: OnboardingDripOrderState
      referralResult: ReferralConsumptionResult
      request: Request
      session: Session
    }>,
  ) => Promise<unknown>
  agentRegistrationStore?: AgentRegistrationStore
}>

class OnboardingUnauthorized extends S.TaggedErrorClass<OnboardingUnauthorized>()(
  'OnboardingUnauthorized',
  {},
) {}

class OnboardingBadRequest extends S.TaggedErrorClass<OnboardingBadRequest>()(
  'OnboardingBadRequest',
  {
    reason: S.String,
  },
) {}

class OnboardingSessionError extends S.TaggedErrorClass<OnboardingSessionError>()(
  'OnboardingSessionError',
  {
    error: S.Defect,
  },
) {}

class OnboardingGitHubTokenMissing extends S.TaggedErrorClass<OnboardingGitHubTokenMissing>()(
  'OnboardingGitHubTokenMissing',
  {},
) {}

// A stored GitHub token that GitHub itself now rejects (revoked, expired, or
// the user pulled the OAuth grant). Distinct from `OnboardingGitHubTokenMissing`
// (no token stored at all) so a mobile client can tell "never connected" apart
// from "connected once, needs to re-auth" — both still resolve to the same
// client action (prompt GitHub re-auth), which is why both map to 401 below.
class OnboardingGitHubTokenExpired extends S.TaggedErrorClass<OnboardingGitHubTokenExpired>()(
  'OnboardingGitHubTokenExpired',
  {},
) {}

class OnboardingRepositoryNotFound extends S.TaggedErrorClass<OnboardingRepositoryNotFound>()(
  'OnboardingRepositoryNotFound',
  {
    repositoryId: S.String,
  },
) {}

type OnboardingRouteError =
  | GitHubRepositoryListFailed
  | GitHubRepositoryReadFailed
  | CustomerOrderStorageError
  | CustomerOrderAgentAuthFailure
  | OnboardingBadRequest
  | OnboardingGitHubTokenExpired
  | OnboardingGitHubTokenMissing
  | OnboardingInvalidStep
  | OnboardingRepositoryNotFound
  | OnboardingSessionError
  | OnboardingStorageError
  | OnboardingUnauthorized
  | OnboardingUserNotFound
  | SiteReferralConsumptionStorageError

const routeErrorResponse = (error: OnboardingRouteError): Response => {
  if (error instanceof CustomerOrderStorageError) {
    logWorkerRouteError('customer_order_storage_error', error.error, {
      operation: error.operation,
    })
  }

  if (error instanceof OnboardingStorageError) {
    logWorkerRouteError('onboarding_storage_error', error.error, {
      operation: error.operation,
    })
  }

  if (error instanceof SiteReferralConsumptionStorageError) {
    logWorkerRouteError('site_referral_consumption_storage_error', error.error, {
      operation: error.operation,
    })
  }

  return M.value(error).pipe(
    M.tags({
      CustomerOrderStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      CustomerOrderAgentAuthFailure: ({ failureKind, reason }) =>
        noStoreJsonResponse(
          { error: failureKind === 'missing_credentials' ? 'unauthorized' : 'forbidden', reason },
          {
            status:
              failureKind === 'missing_credentials' ||
              failureKind === 'malformed_credentials' ||
              failureKind === 'expired_credentials'
                ? 401
                : 403,
          },
        ),
      GitHubRepositoryListFailed: () =>
        noStoreJsonResponse(
          { error: 'github_repository_list_failed' },
          { status: 502 },
        ),
      GitHubRepositoryReadFailed: () =>
        noStoreJsonResponse(
          { error: 'github_repository_lookup_failed' },
          { status: 502 },
        ),
      OnboardingBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      OnboardingGitHubTokenExpired: () =>
        noStoreJsonResponse({ error: 'github_token_expired' }, { status: 401 }),
      OnboardingGitHubTokenMissing: () =>
        noStoreJsonResponse({ error: 'github_token_missing' }, { status: 409 }),
      OnboardingInvalidStep: ({ step }) =>
        noStoreJsonResponse(
          { error: 'invalid_onboarding_step', step },
          { status: 409 },
        ),
      OnboardingRepositoryNotFound: ({ repositoryId }) =>
        noStoreJsonResponse(
          { error: 'repository_not_found', repositoryId },
          { status: 404 },
        ),
      OnboardingSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OnboardingStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      SiteReferralConsumptionStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      OnboardingUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      OnboardingUserNotFound: ({ userId }) =>
        noStoreJsonResponse(
          { error: 'user_not_found', userId },
          { status: 404 },
        ),
    }),
    M.exhaustive,
  )
}

const decodeJsonBody = <Schema extends S.Top>(
  request: Request,
  schema: Schema,
) =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: error =>
        new OnboardingBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
    })

    return yield* S.decodeUnknownEffect(schema)(payload)
  }).pipe(
    Effect.mapError(error =>
      error instanceof OnboardingBadRequest
        ? error
        : new OnboardingBadRequest({ reason: String(error) }),
    ),
  )

const requireSession = <
  Session extends OnboardingSession,
  RouteEnv extends OnboardingEnv,
>(
  dependencies: OnboardingRouteDependencies<Session, RouteEnv>,
  request: Request,
  env: RouteEnv,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireBrowserSession(request, env, ctx),
      catch: error => new OnboardingSessionError({ error }),
    })

    if (session === undefined) {
      return yield* new OnboardingUnauthorized({})
    }

    return session
  })

const requireMobileBearerSession = <
  Session extends OnboardingSession,
  RouteEnv extends OnboardingEnv,
>(
  dependencies: OnboardingRouteDependencies<Session, RouteEnv>,
  request: Request,
  env: RouteEnv,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireUserBearerSession(request, env, ctx),
      catch: error => new OnboardingSessionError({ error }),
    })

    if (session === undefined) {
      return yield* new OnboardingUnauthorized({})
    }

    return session
  })

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('Idempotency-Key')?.trim()

  return value === undefined || value.length < 8 || value.length > 160
    ? undefined
    : value
}

const customerOrderAgentStore = (
  agentRegistrationStore: AgentRegistrationStore | undefined,
  db: D1Database,
): AgentRegistrationStore =>
  agentRegistrationStore ?? makeD1AgentRegistrationStore(db)

const requireCustomerOrderActor = <
  Session extends OnboardingSession,
  RouteEnv extends OnboardingEnv,
>(
  dependencies: OnboardingRouteDependencies<Session, RouteEnv>,
  request: Request,
  env: RouteEnv,
  ctx: ExecutionContext,
  requiredScope: CustomerOrderAgentScope,
): Effect.Effect<
  CustomerOrderRouteActor<Session>,
  CustomerOrderAgentAuthFailure | OnboardingSessionError | OnboardingUnauthorized
> => {
  const bearerToken = readBearerToken(request)

  if (bearerToken !== undefined) {
    return authenticateCustomerOrderAgentRequest(
      request,
      customerOrderAgentStore(
        dependencies.agentRegistrationStore,
        openAgentsDatabase(env),
      ),
      {
        nowIso:
          dependencies.customerOrderRuntime?.nowIso ??
          systemOnboardingRuntime.nowIso,
        requiredScope,
      },
    ).pipe(
      Effect.map(context => ({
        _tag: 'Agent' as const,
        context,
        userId: context.ownerUserId,
      })),
    )
  }

  return requireSession(dependencies, request, env, ctx).pipe(
    Effect.map(session => ({
      _tag: 'BrowserSession' as const,
      session,
      userId: session.user.userId,
    })),
  )
}

const appendCustomerOrderActorCookies = <
  Session extends OnboardingSession,
  RouteResponse,
>(
  appendRefreshedSessionCookies: (
    response: RouteResponse,
    session: Session,
  ) => RouteResponse,
  response: RouteResponse,
  actor: CustomerOrderRouteActor<Session>,
) =>
  actor._tag === 'BrowserSession'
    ? appendRefreshedSessionCookies(response, actor.session)
    : response

const readGitHubIdentityToken = (
  env: OnboardingEnv,
  userId: string,
): Effect.Effect<string | null, OnboardingStorageError> =>
  Effect.tryPromise({
    try: () => env.AUTH_STORAGE.get(githubIdentityTokenKey(userId)),
    catch: error =>
      new OnboardingStorageError({
        operation: 'onboarding.githubIdentityToken.read',
        error,
      }),
  })

const availableRepositories = (
  env: OnboardingEnv,
  userId: string,
): Effect.Effect<
  ReadonlyArray<OnboardingGitHubRepository>,
  | GitHubRepositoryListFailed
  | OnboardingGitHubTokenMissing
  | OnboardingStorageError,
  GitHubRepositoryService
> =>
  Effect.gen(function* () {
    const token = yield* readGitHubIdentityToken(env, userId)

    if (token === null) {
      return yield* new OnboardingGitHubTokenMissing({})
    }

    const github = yield* GitHubRepositoryService

    return yield* github.listRepositories(token).pipe(
      Effect.mapError(error =>
        error instanceof GitHubRepositoryListFailed
          ? error
          : new GitHubRepositoryListFailed({
              reason: 'GitHub repository response was invalid.',
              status: 502,
            }),
      ),
    )
  })

// The mobile-bearer GitHub token lookup (MM-B1, #8471). Unlike
// `availableRepositories` above (which the cookie-gated onboarding wizard
// uses and silently degrades to an empty list), the mobile routes surface a
// TYPED failure when the token is missing or GitHub has revoked/expired it —
// the mobile client needs to distinguish "nothing to show yet" from "please
// sign in with GitHub again" so it can prompt re-auth.
const requireMobileGitHubToken = (
  env: OnboardingEnv,
  userId: string,
): Effect.Effect<
  string,
  OnboardingGitHubTokenMissing | OnboardingStorageError
> =>
  Effect.gen(function* () {
    const token = yield* readGitHubIdentityToken(env, userId)

    if (token === null) {
      return yield* new OnboardingGitHubTokenMissing({})
    }

    return token
  })

const asMobileTokenExpiredWhenUnauthorized = <A, E>(
  effect: Effect.Effect<A, E, GitHubRepositoryService>,
  isUnauthorized: (error: E) => boolean,
): Effect.Effect<
  A,
  E | OnboardingGitHubTokenExpired,
  GitHubRepositoryService
> =>
  effect.pipe(
    Effect.mapError(error =>
      isUnauthorized(error) ? new OnboardingGitHubTokenExpired({}) : error,
    ),
  )

const parsePositiveIntQueryParam = (
  url: URL,
  name: string,
  fallback: number,
): number | undefined => {
  const raw = url.searchParams.get(name)

  if (raw === null || raw.trim() === '') {
    return fallback
  }

  const parsed = Number.parseInt(raw, 10)

  return Number.isFinite(parsed) && parsed >= 1 ? parsed : undefined
}

const manualRepository = (
  owner: string,
  name: string,
): OnboardingGitHubRepository => {
  const fullName = `${owner}/${name}`

  return {
    id: fullName,
    provider: 'github',
    owner,
    name,
    fullName,
    private: false,
    defaultBranch: 'main',
    htmlUrl: `https://github.com/${fullName}`,
    description: null,
  }
}

const routeLayer = (
  env: OnboardingEnv,
  runtime: OnboardingRuntime,
  customerOrderRuntime: CustomerOrderRuntime | undefined,
  githubRepositoryServiceLayer: Layer.Layer<GitHubRepositoryService>,
) =>
  Layer.merge(
    Layer.merge(
      OnboardingStateStore.layer(env, runtime),
      CustomerOrderStore.layer(env, customerOrderRuntime),
    ),
    githubRepositoryServiceLayer,
  )

const pendingReferralId = (request: Request): string | undefined =>
  parseCookies(request).get(PENDING_REFERRAL_COOKIE)

const clearPendingReferralCookie = (response: globalThis.Response) => {
  response.headers.append('set-cookie', expiredCookie(PENDING_REFERRAL_COOKIE))

  return response
}

const shouldClearPendingReferralCookie = (
  result: Awaited<ReturnType<typeof consumePendingReferralForUser>>,
): boolean => result._tag !== 'none'

const scheduleReferralOnboarding = <
  Session extends OnboardingSession,
  RouteEnv extends OnboardingEnv,
>(
  dependencies: OnboardingRouteDependencies<Session, RouteEnv>,
  input: Readonly<{
    ctx: ExecutionContext
    env: RouteEnv
    orderId: string | null
    orderState: OnboardingDripOrderState
    referralResult: ReferralConsumptionResult
    request: Request
    session: Session
  }>,
): void => {
  if (
    dependencies.siteReferralOnboarding === undefined ||
    input.referralResult._tag !== 'consumed'
  ) {
    return
  }

  scheduleBackgroundWork(
    input.ctx,
    dependencies
      .siteReferralOnboarding({
        env: input.env,
        orderId: input.orderId,
        orderState: input.orderState,
        referralResult: input.referralResult,
        request: input.request,
        session: input.session,
      })
      .catch(error =>
        logWorkerRouteError('site_referral_onboarding_route_failed', error, {
          orderId: input.orderId,
          userId: input.session.user.userId,
        }),
      ),
  )
}

export const makeOnboardingRoutes = <
  Session extends OnboardingSession,
  RouteEnv extends OnboardingEnv,
>(
  dependencies: OnboardingRouteDependencies<Session, RouteEnv>,
) => {
  const runtime = dependencies.runtime ?? systemOnboardingRuntime
  type RouteRuntimeEnv = RouteEnv

  const runRoute = (
    env: RouteEnv,
    effect: Effect.Effect<
      Response,
      OnboardingRouteError,
      CustomerOrderStore | OnboardingStateStore | GitHubRepositoryService
    >,
  ): Effect.Effect<Response> =>
    effect.pipe(
      Effect.provide(
        routeLayer(
          env,
          runtime,
          dependencies.customerOrderRuntime,
          dependencies.githubRepositoryServiceLayer ??
            GitHubRepositoryService.layer,
        ),
      ),
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )

  const statusResponse = (request: Request, env: RouteEnv, ctx: ExecutionContext) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const store = yield* OnboardingStateStore
        const onboarding = yield* store.readStatus(session.user.userId)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ onboarding }),
          session,
        )
      }),
    )

  const activeCustomerOrderResponse = (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.read',
        )
        const store = yield* CustomerOrderStore
        const order =
          actor._tag === 'BrowserSession'
            ? yield* store.readOrCreateActiveOrder(actor.userId)
            : (yield* store.listOrders(actor.userId))[0] ?? null

        if (actor._tag === 'Agent') {
          return noStoreJsonResponse({ order })
        }

        const orderState: OnboardingDripOrderState =
          order === null
            ? 'none'
            : order.status === 'delivered'
              ? 'delivered'
              : 'active'
        const pendingAttributionId = pendingReferralId(request)
        const referralResult =
          order === null
            ? yield* Effect.tryPromise({
                catch: error =>
                  error instanceof SiteReferralConsumptionStorageError
                    ? error
                    : new SiteReferralConsumptionStorageError({
                        error,
                        operation: 'siteReferralConsumption.userRoute.consume',
                      }),
                try: () =>
                  // KS-8.14 (#8359): referral consumption UPDATEs
                  // referral_attributions + INSERTs user_referral_attributions
                  // — ride the business funnel dual-write mirror seam.
                  consumePendingReferralForUser(
                    businessDomainDatabaseForEnv(env),
                    {
                      nowIso:
                        dependencies.customerOrderRuntime?.nowIso ??
                        systemOnboardingRuntime.nowIso,
                    },
                    {
                      pendingAttributionId,
                      userId: actor.userId,
                    },
                  ),
              })
            : yield* Effect.tryPromise({
                catch: error =>
                  error instanceof SiteReferralConsumptionStorageError
                    ? error
                    : new SiteReferralConsumptionStorageError({
                        error,
                        operation: 'siteReferralConsumption.orderRoute.link',
                      }),
                try: () =>
                  // KS-8.14 (#8359): order referral linkage writes the
                  // business-domain order_referral_attributions table —
                  // ride the business funnel dual-write mirror seam.
                  linkPendingReferralToOrder(
                    businessDomainDatabaseForEnv(env),
                    {
                      nowIso:
                        dependencies.customerOrderRuntime?.nowIso ??
                        systemOnboardingRuntime.nowIso,
                    },
                    {
                      orderId: order.id,
                      pendingAttributionId,
                      userId: actor.userId,
                    },
                  ),
              })

        if (actor._tag === 'BrowserSession') {
          yield* Effect.sync(() =>
            scheduleReferralOnboarding(dependencies, {
              ctx,
              env,
              orderId: order?.id ?? null,
              orderState,
              referralResult,
              request,
              session: actor.session,
            }),
          )
        }

        const response = appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ order }),
          actor,
        )

        return shouldClearPendingReferralCookie(referralResult)
          ? clearPendingReferralCookie(response)
          : response
      }),
    )

  const customerOrdersListResponse = (
    request: Request,
    env: RouteRuntimeEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.read',
        )
        const store = yield* CustomerOrderStore
        const orders = yield* store.listOrders(actor.userId)

        return appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ orders }),
          actor,
        )
      }),
    )

  const customerOrderCreateResponse = (
    request: Request,
    env: RouteRuntimeEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.write',
        )
        const input = yield* decodeJsonBody(request, CreateCustomerOrderRequest)
        const body = input.request.trim()

        if (body === '' || body.length > 4000) {
          return appendCustomerOrderActorCookies(
            dependencies.appendRefreshedSessionCookies,
            noStoreJsonResponse(
              { error: 'bad_request', reason: 'request is required' },
              { status: 400 },
            ),
            actor,
          )
        }

        const idempotencyKey =
          actor._tag === 'Agent' ? idempotencyKeyFromRequest(request) : undefined

        if (actor._tag === 'Agent' && idempotencyKey === undefined) {
          return noStoreJsonResponse(
            {
              error: 'idempotency_key_required',
              reason:
                'Agent customer order creation requires Idempotency-Key.',
            },
            { status: 400 },
          )
        }

        const store = yield* CustomerOrderStore
        const existingOrder =
          idempotencyKey === undefined
            ? null
            : yield* store.readOrderByAgentIdempotencyKey(
                actor.userId,
                idempotencyKey,
              )
        const order =
          existingOrder ??
          (yield* store.createOrder(actor.userId, body, idempotencyKey))
        const orderState: OnboardingDripOrderState =
          order.status === 'delivered' ? 'delivered' : 'active'

        if (actor._tag === 'Agent') {
          return noStoreJsonResponse(
            { idempotent: existingOrder !== null, order },
            { status: existingOrder === null ? 201 : 200 },
          )
        }

        const referralResult = yield* Effect.tryPromise({
          catch: error =>
            error instanceof SiteReferralConsumptionStorageError
              ? error
              : new SiteReferralConsumptionStorageError({
                  error,
                  operation: 'siteReferralConsumption.orderCreate.link',
                }),
          try: () =>
            // KS-8.14 (#8359): order referral linkage writes the
            // business-domain order_referral_attributions table — ride the
            // business funnel dual-write mirror seam.
            linkPendingReferralToOrder(
              businessDomainDatabaseForEnv(env),
              {
                nowIso:
                  dependencies.customerOrderRuntime?.nowIso ??
                  systemOnboardingRuntime.nowIso,
              },
              {
                orderId: order.id,
                pendingAttributionId: pendingReferralId(request),
                userId: actor.userId,
              },
            ),
        })

        yield* Effect.sync(() =>
          scheduleReferralOnboarding(dependencies, {
            ctx,
            env,
            orderId: order.id,
            orderState,
            referralResult,
            request,
            session: actor.session,
          }),
        )

        const response = appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ order }, { status: 201 }),
          actor,
        )

        return shouldClearPendingReferralCookie(referralResult)
          ? clearPendingReferralCookie(response)
          : response
      }),
    )

  const customerOrderDetailResponse = (
    orderId: string,
    request: Request,
    env: RouteRuntimeEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.read',
        )
        const store = yield* CustomerOrderStore
        const order = yield* store.readOrderById(actor.userId, orderId)

        return appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ order }),
          actor,
        )
      }),
    )

  const customerOrderSiteRevisionsResponse = (
    orderId: string,
    request: Request,
    env: RouteRuntimeEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.read',
        )
        const store = yield* CustomerOrderStore
        const revisions = yield* store.listSiteRevisions(
          actor.userId,
          orderId,
        )

        if (revisions === null) {
          return appendCustomerOrderActorCookies(
            dependencies.appendRefreshedSessionCookies,
            noStoreJsonResponse(
              { error: 'customer_order_not_found' },
              { status: 404 },
            ),
            actor,
          )
        }

        return appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ revisions }),
          actor,
        )
      }),
    )

  const customerOrderFulfillmentArtifactsResponse = (
    orderId: string,
    request: Request,
    env: RouteRuntimeEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.read',
        )
        const store = yield* CustomerOrderStore
        const artifacts = yield* store.listFulfillmentArtifacts(
          actor.userId,
          orderId,
        )

        if (artifacts === null) {
          return appendCustomerOrderActorCookies(
            dependencies.appendRefreshedSessionCookies,
            noStoreJsonResponse(
              { error: 'customer_order_not_found' },
              { status: 404 },
            ),
            actor,
          )
        }

        return appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ artifacts }),
          actor,
        )
      }),
    )

  const customerOrderSiteFeedbackListResponse = (
    orderId: string,
    request: Request,
    env: RouteRuntimeEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.read',
        )
        const store = yield* CustomerOrderStore
        const feedback = yield* store.listSiteFeedback(
          actor.userId,
          orderId,
        )

        if (feedback === null) {
          return appendCustomerOrderActorCookies(
            dependencies.appendRefreshedSessionCookies,
            noStoreJsonResponse(
              { error: 'customer_order_not_found' },
              { status: 404 },
            ),
            actor,
          )
        }

        return appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ feedback }),
          actor,
        )
      }),
    )

  const customerOrderSiteFeedbackSubmitResponse = (
    orderId: string,
    request: Request,
    env: RouteRuntimeEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const actor = yield* requireCustomerOrderActor(
          dependencies,
          request,
          env,
          ctx,
          'customer_orders.feedback',
        )
        const input = yield* decodeJsonBody(
          request,
          SubmitCustomerSiteFeedbackRequest,
        )
        const body = input.body.trim()

        if (body === '' || body.length > 4000) {
          return appendCustomerOrderActorCookies(
            dependencies.appendRefreshedSessionCookies,
            noStoreJsonResponse(
              { error: 'bad_request', reason: 'feedback body is required' },
              { status: 400 },
            ),
            actor,
          )
        }

        const store = yield* CustomerOrderStore
        const feedback = yield* store.submitSiteFeedback(
          actor.userId,
          orderId,
          body,
        )

        if (feedback === null) {
          return appendCustomerOrderActorCookies(
            dependencies.appendRefreshedSessionCookies,
            noStoreJsonResponse(
              { error: 'customer_order_not_found' },
              { status: 404 },
            ),
            actor,
          )
        }

        return appendCustomerOrderActorCookies(
          dependencies.appendRefreshedSessionCookies,
          noStoreJsonResponse({ feedback }, { status: 201 }),
          actor,
        )
      }),
    )

  const repositoriesResponse = (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const token = yield* readGitHubIdentityToken(env, session.user.userId)

        if (token === null) {
          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({
              repositories: [],
              tokenStatus: 'missing',
            }),
            session,
          )
        }

        const github = yield* GitHubRepositoryService
        const repositories = yield* github.listRepositories(token).pipe(
          Effect.mapError(error =>
            error instanceof GitHubRepositoryListFailed
              ? error
              : new GitHubRepositoryListFailed({
                  reason: 'GitHub repository response was invalid.',
                  status: 502,
                }),
          ),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            repositories,
            tokenStatus: 'available',
          }),
          session,
        )
      }),
    )

  // Mobile-bearer repo list API (MM-B1, #8471). GET /api/mobile/repos.
  // Paginated: ?page=&perPage= (GitHub-native paging via the Link header, see
  // github.ts's `listRepositoriesPage`). No cookies are set — mobile sessions
  // carry the OpenAuth bearer token, not a browser cookie.
  const mobileRepositoriesResponse = (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireMobileBearerSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const url = new URL(request.url)
        const page = parsePositiveIntQueryParam(url, 'page', 1)
        const perPage = parsePositiveIntQueryParam(
          url,
          'perPage',
          GITHUB_REPOSITORY_DEFAULT_PER_PAGE,
        )

        if (page === undefined) {
          return yield* new OnboardingBadRequest({
            reason: 'page must be a positive integer',
          })
        }

        if (perPage === undefined || perPage > GITHUB_REPOSITORY_MAX_PER_PAGE) {
          return yield* new OnboardingBadRequest({
            reason: `perPage must be an integer between 1 and ${GITHUB_REPOSITORY_MAX_PER_PAGE}`,
          })
        }

        const token = yield* requireMobileGitHubToken(env, session.user.userId)
        const github = yield* GitHubRepositoryService
        const result = yield* asMobileTokenExpiredWhenUnauthorized(
          github.listRepositoriesPage(token, { page, perPage }).pipe(
            Effect.mapError(error =>
              error instanceof GitHubRepositoryListFailed
                ? error
                : new GitHubRepositoryListFailed({
                    reason: 'GitHub repository response was invalid.',
                    status: 502,
                  }),
            ),
          ),
          error => error.status === 401,
        )

        return noStoreJsonResponse({
          repositories: result.repositories,
          page: result.page,
          perPage: result.perPage,
          hasNextPage: result.hasNextPage,
        })
      }),
    )

  // Mobile-bearer repo detail API (MM-B1, #8471). GET
  // /api/mobile/repos/{owner}/{name} — validates a specific repo is
  // reachable with the caller's stored GitHub token (e.g. before binding a
  // thread to it).
  const mobileRepositoryDetailResponse = (
    owner: string,
    name: string,
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireMobileBearerSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const token = yield* requireMobileGitHubToken(env, session.user.userId)
        const github = yield* GitHubRepositoryService
        const repository = yield* github.getRepository(token, owner, name).pipe(
          Effect.mapError(error => {
            const readFailed =
              error instanceof GitHubRepositoryReadFailed
                ? error
                : new GitHubRepositoryReadFailed({
                    reason: 'GitHub repository response was invalid.',
                    status: 502,
                  })

            if (readFailed.status === 401) {
              return new OnboardingGitHubTokenExpired({})
            }

            if (readFailed.status === 404) {
              return new OnboardingRepositoryNotFound({
                repositoryId: `${owner}/${name}`,
              })
            }

            return readFailed
          }),
        )

        return noStoreJsonResponse({ repository })
      }),
    )

  const saveRepositoryResponse = (
    mode: 'select' | 'update',
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const body = yield* decodeJsonBody(
          request,
          SelectOnboardingRepositoryRequest,
        )
        const repository =
          'repositoryId' in body
            ? yield* availableRepositories(env, session.user.userId).pipe(
                Effect.flatMap(repositories => {
                  const repository = repositories.find(
                    item => item.id === body.repositoryId,
                  )

                  return repository === undefined
                    ? Effect.fail(
                        new OnboardingRepositoryNotFound({
                          repositoryId: body.repositoryId,
                        }),
                      )
                    : Effect.succeed(repository)
                }),
              )
            : yield* Effect.gen(function* () {
                const owner = body.owner.trim()
                const name = body.name.trim()

                if (owner === '' || name === '') {
                  return yield* new OnboardingBadRequest({
                    reason: 'repository owner and name are required',
                  })
                }

                return manualRepository(owner, name)
              })

        const store = yield* OnboardingStateStore
        const onboarding =
          mode === 'select'
            ? yield* store.selectRepository(session.user.userId, repository)
            : yield* store.updateRepository(session.user.userId, repository)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ onboarding }),
          session,
        )
      }),
    )

  const skipRepositoryResponse = (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const store = yield* OnboardingStateStore
        const onboarding = yield* store.skipRepository(session.user.userId)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ onboarding }),
          session,
        )
      }),
    )

  const skipBillingResponse = (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const store = yield* OnboardingStateStore
        const onboarding = yield* store.skipBilling(session.user.userId)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ onboarding }),
          session,
        )
      }),
    )

  const submitGoalResponse = (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const body = yield* decodeJsonBody(request, SubmitOnboardingGoalRequest)
        const store = yield* OnboardingStateStore
        const onboarding = yield* store.submitGoal(
          session.user.userId,
          body.goal,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ onboarding }),
          session,
        )
      }),
    )

  return {
    routeOnboardingRequest: (
      request: Request,
      env: RouteEnv,
      ctx: ExecutionContext,
    ): Effect.Effect<Response> | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/onboarding') {
        return request.method === 'GET'
          ? statusResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      if (url.pathname === '/api/onboarding/repositories') {
        return request.method === 'GET'
          ? repositoriesResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      if (url.pathname === '/api/mobile/repos') {
        return request.method === 'GET'
          ? mobileRepositoriesResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      const mobileRepositoryDetailMatch =
        /^\/api\/mobile\/repos\/([^/]+)\/([^/]+)$/.exec(url.pathname)

      if (mobileRepositoryDetailMatch !== null) {
        return request.method === 'GET'
          ? mobileRepositoryDetailResponse(
              decodeURIComponent(mobileRepositoryDetailMatch[1] ?? ''),
              decodeURIComponent(mobileRepositoryDetailMatch[2] ?? ''),
              request,
              env,
              ctx,
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      if (url.pathname === '/api/customer-orders/active') {
        return request.method === 'GET'
          ? activeCustomerOrderResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      if (url.pathname === '/api/customer-orders') {
        if (request.method === 'GET') {
          return customerOrdersListResponse(request, env, ctx)
        }

        return request.method === 'POST'
          ? customerOrderCreateResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['GET', 'POST']))
      }

      const customerOrderMatch = /^\/api\/customer-orders\/([^/]+)$/.exec(
        url.pathname,
      )

      if (customerOrderMatch !== null) {
        return request.method === 'GET'
          ? customerOrderDetailResponse(
              customerOrderMatch[1] ?? '',
              request,
              env,
              ctx,
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      const customerOrderSiteRevisionsMatch =
        /^\/api\/customer-orders\/([^/]+)\/site-revisions$/.exec(url.pathname)

      if (customerOrderSiteRevisionsMatch !== null) {
        return request.method === 'GET'
          ? customerOrderSiteRevisionsResponse(
              customerOrderSiteRevisionsMatch[1] ?? '',
              request,
              env,
              ctx,
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      const customerOrderFulfillmentArtifactsMatch =
        /^\/api\/customer-orders\/([^/]+)\/fulfillment-artifacts$/.exec(
          url.pathname,
        )

      if (customerOrderFulfillmentArtifactsMatch !== null) {
        return request.method === 'GET'
          ? customerOrderFulfillmentArtifactsResponse(
              customerOrderFulfillmentArtifactsMatch[1] ?? '',
              request,
              env,
              ctx,
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      const customerOrderSiteFeedbackMatch =
        /^\/api\/customer-orders\/([^/]+)\/site-feedback$/.exec(url.pathname)

      if (customerOrderSiteFeedbackMatch !== null) {
        if (request.method === 'GET') {
          return customerOrderSiteFeedbackListResponse(
            customerOrderSiteFeedbackMatch[1] ?? '',
            request,
            env,
            ctx,
          )
        }

        return request.method === 'POST'
          ? customerOrderSiteFeedbackSubmitResponse(
              customerOrderSiteFeedbackMatch[1] ?? '',
              request,
              env,
              ctx,
            )
          : Effect.succeed(methodNotAllowed(['GET', 'POST']))
      }

      if (url.pathname === '/api/onboarding/repository/select') {
        return request.method === 'POST'
          ? saveRepositoryResponse('select', request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      if (url.pathname === '/api/onboarding/repository/update') {
        return request.method === 'POST'
          ? saveRepositoryResponse('update', request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      if (url.pathname === '/api/onboarding/repository/skip') {
        return request.method === 'POST'
          ? skipRepositoryResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      if (url.pathname === '/api/onboarding/billing/skip') {
        return request.method === 'POST'
          ? skipBillingResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      if (url.pathname === '/api/onboarding/goal') {
        return request.method === 'POST'
          ? submitGoalResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      return undefined
    },
  }
}
