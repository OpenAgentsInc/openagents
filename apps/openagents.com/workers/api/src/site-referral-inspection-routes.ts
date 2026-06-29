import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'
import {
  SiteReferralInspectionUnsafePayload,
  readOperatorConsumedReferralAttributions,
  readOperatorSiteReferralInspection,
  readSiteReferralOwnerOverview,
} from './site-referral-inspection'

type SiteReferralInspectionEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type SiteReferralInspectionSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type SiteReferralInspectionRouteDependencies<
  Session extends SiteReferralInspectionSession,
  Bindings extends SiteReferralInspectionEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

class SiteReferralInspectionUnauthorized extends S.TaggedErrorClass<SiteReferralInspectionUnauthorized>()(
  'SiteReferralInspectionUnauthorized',
  {},
) {}

class SiteReferralInspectionForbidden extends S.TaggedErrorClass<SiteReferralInspectionForbidden>()(
  'SiteReferralInspectionForbidden',
  {},
) {}

class SiteReferralInspectionSessionError extends S.TaggedErrorClass<SiteReferralInspectionSessionError>()(
  'SiteReferralInspectionSessionError',
  {
    error: S.Defect,
  },
) {}

class SiteReferralInspectionStorageError extends S.TaggedErrorClass<SiteReferralInspectionStorageError>()(
  'SiteReferralInspectionStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

type SiteReferralInspectionRouteError =
  | SiteReferralInspectionForbidden
  | SiteReferralInspectionSessionError
  | SiteReferralInspectionStorageError
  | SiteReferralInspectionUnauthorized
  | SiteReferralInspectionUnsafePayload

const routeErrorResponse = (
  error: SiteReferralInspectionRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      SiteReferralInspectionForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      SiteReferralInspectionSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      SiteReferralInspectionStorageError: () =>
        noStoreJsonResponse(
          { error: 'site_referral_inspection_storage_error' },
          { status: 500 },
        ),
      SiteReferralInspectionUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      SiteReferralInspectionUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_site_referral_inspection_payload', reason },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

const requireSession = <
  Session extends SiteReferralInspectionSession,
  Bindings extends SiteReferralInspectionEnv,
>(
  dependencies: SiteReferralInspectionRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      catch: error => new SiteReferralInspectionSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    return session === undefined
      ? yield* new SiteReferralInspectionUnauthorized({})
      : session
  })

const requireAdminSession = <
  Session extends SiteReferralInspectionSession,
  Bindings extends SiteReferralInspectionEnv,
>(
  dependencies: SiteReferralInspectionRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireSession(dependencies, request, env, ctx)

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new SiteReferralInspectionForbidden({})
    }

    return session
  })

const readLimit = (request: Request): number => {
  const raw = new URL(request.url).searchParams.get('limit')
  const value = raw === null ? 100 : Number.parseInt(raw, 10)

  return Number.isFinite(value) ? Math.max(1, Math.min(200, value)) : 100
}

const runRoute = (
  request: Request,
  allowedMethods: readonly string[],
  effect: Effect.Effect<HttpResponse, SiteReferralInspectionRouteError>,
): Effect.Effect<HttpResponse> =>
  allowedMethods.includes(request.method)
    ? effect.pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    : Effect.succeed(methodNotAllowed([...allowedMethods]))

export const makeSiteReferralInspectionRoutes = <
  Session extends SiteReferralInspectionSession,
  Bindings extends SiteReferralInspectionEnv,
>(
  dependencies: SiteReferralInspectionRouteDependencies<Session, Bindings>,
) => {
  const ownerOverview = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      request,
      ['GET'],
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const overview = yield* Effect.tryPromise({
          catch: error =>
            error instanceof SiteReferralInspectionUnsafePayload
              ? error
              : new SiteReferralInspectionStorageError({
                  error,
                  operation: 'siteReferralInspection.ownerOverview',
                }),
          try: () =>
            readSiteReferralOwnerOverview(
              openAgentsDatabase(env),
              session.user.userId,
              readLimit(request),
            ),
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ referralOverview: overview }),
          session,
        )
      }),
    )

  const operatorInspection = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      request,
      ['GET'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const inspection = yield* Effect.tryPromise({
          catch: error =>
            error instanceof SiteReferralInspectionUnsafePayload
              ? error
              : new SiteReferralInspectionStorageError({
                  error,
                  operation: 'siteReferralInspection.operatorInspection',
                }),
          try: () =>
            readOperatorSiteReferralInspection(
              openAgentsDatabase(env),
              readLimit(request),
            ),
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ referralInspection: inspection }),
          session,
        )
      }),
    )

  const operatorConsumedAttributions = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      request,
      ['GET'],
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const consumedAttributions = yield* Effect.tryPromise({
          catch: error =>
            error instanceof SiteReferralInspectionUnsafePayload
              ? error
              : new SiteReferralInspectionStorageError({
                  error,
                  operation:
                    'siteReferralInspection.operatorConsumedAttributions',
                }),
          try: () =>
            readOperatorConsumedReferralAttributions(
              openAgentsDatabase(env),
              readLimit(request),
            ),
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ consumedAttributions }),
          session,
        )
      }),
    )

  return {
    routeSiteReferralInspectionRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/sites/referrals/overview') {
        return ownerOverview(request, env, ctx)
      }

      if (url.pathname === '/api/operator/sites/referrals') {
        return operatorInspection(request, env, ctx)
      }

      if (url.pathname === '/api/operator/sites/referrals/consumed') {
        return operatorConsumedAttributions(request, env, ctx)
      }

      return undefined
    },
  }
}
