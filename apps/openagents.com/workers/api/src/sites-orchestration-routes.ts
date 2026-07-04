import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
// KS-8.12 (#8323): sites writes ride the dual-write mirror seam — the
// mirroring database is a passthrough for non-scoped statements and
// degrades to the raw D1 handle when no KHALA_SYNC_DB binding exists.
import { sitesContentDatabaseForEnv as openAgentsDatabase } from './sites-content-store'
import { SiteBuilderPhaseKind } from './sites-builder-sessions'
import { SiteBuilderRepairFailureKind } from './sites-builder-repair-loop'
import {
  type AdvanceSiteOrchestrationInput,
  type SiteOrchestrationError,
  advanceSiteBuilderOrchestration,
  readSiteBuilderOrchestrationState,
} from './sites-orchestration'

/**
 * Operator-gated routes for the native Sites prompt -> build -> deploy
 * orchestration core (WS-D1, #4981).
 *
 *   POST /api/operator/sites/orchestration/:sessionId/advance
 *     -> advance a single phase (start a build session, drive a phase forward,
 *        feed a preview candidate, or report a build failure for repair)
 *   GET  /api/operator/sites/orchestration/:sessionId
 *     -> read the current typed orchestration / session-state projection
 *
 * Follows the autopilot-decision-routes pattern: `make...Routes(...)` returns a
 * router whose `route...Request` returns `Effect|undefined`. No new tables — it
 * reuses the existing site_builder_* tables through the orchestration core.
 */

type SiteOrchestrationEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type SiteOrchestrationSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

export type SiteOrchestrationRoutesDependencies<
  Session extends SiteOrchestrationSession,
  Bindings extends SiteOrchestrationEnv,
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

class SiteOrchestrationUnauthorized extends S.TaggedErrorClass<SiteOrchestrationUnauthorized>()(
  'SiteOrchestrationUnauthorized',
  {},
) {}

class SiteOrchestrationForbidden extends S.TaggedErrorClass<SiteOrchestrationForbidden>()(
  'SiteOrchestrationForbidden',
  {},
) {}

class SiteOrchestrationBadRequest extends S.TaggedErrorClass<SiteOrchestrationBadRequest>()(
  'SiteOrchestrationBadRequest',
  {
    reason: S.String,
  },
) {}

class SiteOrchestrationSessionError extends S.TaggedErrorClass<SiteOrchestrationSessionError>()(
  'SiteOrchestrationSessionError',
  {
    error: S.Defect,
  },
) {}

type SiteOrchestrationRouteError =
  | SiteOrchestrationBadRequest
  | SiteOrchestrationForbidden
  | SiteOrchestrationSessionError
  | SiteOrchestrationUnauthorized
  | SiteOrchestrationError

const AdvanceSiteOrchestrationRequest = S.Struct({
  idempotencyKey: S.String,
  phaseKind: SiteBuilderPhaseKind,
  summary: S.String,
  title: S.String,
  failure: S.optionalKey(
    S.Struct({
      attemptNumber: S.optionalKey(S.Number),
      failureKind: SiteBuilderRepairFailureKind,
      failureSummary: S.String,
      retryBudget: S.optionalKey(S.Number),
    }),
  ),
  previewCandidate: S.optionalKey(
    S.Struct({
      artifactRef: S.optionalKey(S.String),
      candidateKind: S.Literals([
        'static_assets',
        'worker_module',
        'needs_build',
        'dev_server',
        'unknown',
      ]),
      healthRef: S.optionalKey(S.String),
      previewUrl: S.optionalKey(S.String),
      runtimeNeeds: S.optionalKey(
        S.Struct({
          buildExecution: S.optionalKey(S.Boolean),
          dependencyHeavy: S.optionalKey(S.Boolean),
          dependencyInstall: S.optionalKey(S.Boolean),
          devServer: S.optionalKey(S.Boolean),
          runtimeRepair: S.optionalKey(S.Boolean),
          ssrRuntime: S.optionalKey(S.Boolean),
        }),
      ),
      versionRef: S.optionalKey(S.String),
      workerModulePath: S.optionalKey(S.String),
    }),
  ),
  resultRef: S.optionalKey(S.String),
})
type AdvanceSiteOrchestrationRequest =
  typeof AdvanceSiteOrchestrationRequest.Type

const routeErrorResponse = (
  error: SiteOrchestrationRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      SiteBuilderSessionStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      SiteBuilderSessionValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'validation_error', reason },
          { status: 400 },
        ),
      SiteOrchestrationBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      SiteOrchestrationForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      SiteOrchestrationSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      SiteOrchestrationUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const requireAdminSession = <
  Session extends SiteOrchestrationSession,
  Bindings extends SiteOrchestrationEnv,
>(
  dependencies: SiteOrchestrationRoutesDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      catch: error => new SiteOrchestrationSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new SiteOrchestrationUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new SiteOrchestrationForbidden({})
    }

    return session
  })

const decodeAdvanceRequest = (
  request: Request,
): Effect.Effect<
  AdvanceSiteOrchestrationRequest,
  SiteOrchestrationBadRequest
> =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      catch: error =>
        new SiteOrchestrationBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
      try: () => request.json(),
    })

    return yield* S.decodeUnknownEffect(AdvanceSiteOrchestrationRequest)(
      payload,
    )
  }).pipe(
    Effect.mapError(error =>
      error instanceof SiteOrchestrationBadRequest
        ? error
        : new SiteOrchestrationBadRequest({ reason: 'invalid request body' }),
    ),
  )

const advanceInput = (
  sessionId: string,
  body: AdvanceSiteOrchestrationRequest,
): AdvanceSiteOrchestrationInput => ({
  idempotencyKey: body.idempotencyKey,
  phaseKind: body.phaseKind,
  sessionId,
  summary: body.summary,
  title: body.title,
  ...(body.failure === undefined ? {} : { failure: body.failure }),
  ...(body.previewCandidate === undefined
    ? {}
    : { previewCandidate: body.previewCandidate }),
  ...(body.resultRef === undefined ? {} : { resultRef: body.resultRef }),
})

const sessionIdFromPath = (pathname: string): string | undefined => {
  const advanceMatch =
    /^\/api\/operator\/sites\/orchestration\/([^/]+)\/advance$/.exec(pathname)

  if (advanceMatch?.[1] !== undefined) {
    return decodeURIComponent(advanceMatch[1])
  }

  const readMatch = /^\/api\/operator\/sites\/orchestration\/([^/]+)$/.exec(
    pathname,
  )

  return readMatch?.[1] === undefined
    ? undefined
    : decodeURIComponent(readMatch[1])
}

export const makeSitesOrchestrationRoutes = <
  Session extends SiteOrchestrationSession,
  Bindings extends SiteOrchestrationEnv,
>(
  dependencies: SiteOrchestrationRoutesDependencies<Session, Bindings>,
) => {
  const runRoute = (
    effect: Effect.Effect<HttpResponse, SiteOrchestrationRouteError>,
  ): Effect.Effect<HttpResponse> =>
    effect.pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

  const advance = (
    sessionId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> =>
    runRoute(
      Effect.gen(function* () {
        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeAdvanceRequest(request)
        const state = yield* advanceSiteBuilderOrchestration(
          openAgentsDatabase(env),
          advanceInput(sessionId, body),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(
            {
              nextPhase: state.nextPhase,
              outcome: state.outcome,
              phaseKind: state.phaseKind,
              preview: state.preview,
              public: state.public,
              repairAttempt: state.repairAttempt,
              sessionStatus: state.sessionStatus,
            },
            { status: 201 },
          ),
          session,
        )
      }),
    )

  const readState = (
    sessionId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> =>
    runRoute(
      Effect.gen(function* () {
        if (request.method !== 'GET') {
          return methodNotAllowed(['GET'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const projection = yield* readSiteBuilderOrchestrationState(
          openAgentsDatabase(env),
          sessionId,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            operator: projection.operator,
            public: projection.public,
          }),
          session,
        )
      }),
    )

  return {
    routeSitesOrchestrationRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)

      if (
        /^\/api\/operator\/sites\/orchestration\/[^/]+\/advance$/.test(
          url.pathname,
        )
      ) {
        const sessionId = sessionIdFromPath(url.pathname)

        return sessionId === undefined
          ? undefined
          : advance(sessionId, request, env, ctx)
      }

      if (
        /^\/api\/operator\/sites\/orchestration\/[^/]+$/.test(url.pathname)
      ) {
        const sessionId = sessionIdFromPath(url.pathname)

        return sessionId === undefined
          ? undefined
          : readState(sessionId, request, env, ctx)
      }

      return undefined
    },
  }
}

/*
 * ============================================================================
 * COORDINATOR WIRING (deferred — do NOT apply in this lane; see #4981 WS-D1)
 * ============================================================================
 *
 * INTEGRATION IS DEFERRED. The block below documents exactly how a coordinator
 * lane should construct and chain this router. None of it has been applied to
 * the shared files (index.ts, worker-routes.ts) in this worktree.
 *
 * 1) Construct the router in `index.ts` next to `makeOperatorSitesRoutes`
 *    (search for `const operatorSitesRoutes = makeOperatorSitesRoutes({`,
 *    currently ~line 5936). The dependency shape is identical to the operator
 *    sites router minus `notifyCustomerSiteDeployed`:
 *
 *      import { makeSitesOrchestrationRoutes } from './sites-orchestration-routes'
 *
 *      const sitesOrchestrationRoutes = makeSitesOrchestrationRoutes({
 *        appendRefreshedSessionCookies,
 *        isOpenAgentsAdminEmail,
 *        requireBrowserSession,
 *      })
 *
 * 2) Expose it on the worker-routes dependency object in `index.ts` (the same
 *    object literal, ~line 7134, that already lists
 *    `routeOperatorSitesRequest: operatorSitesRoutes.routeOperatorSitesRequest`):
 *
 *      routeSitesOrchestrationRequest:
 *        sitesOrchestrationRoutes.routeSitesOrchestrationRequest,
 *
 * 3) Add the field to the worker-routes dependency type in `worker-routes.ts`
 *    (next to `routeOperatorSitesRequest: OptionalEffectRoute`, ~line 61):
 *
 *      routeSitesOrchestrationRequest: OptionalEffectRoute
 *
 * 4) Chain it inside `routeOmniRequest` in `worker-routes.ts`, immediately
 *    AFTER the existing operator-sites block (~line 428) so the more specific
 *    `/api/operator/sites/orchestration/...` paths are tried first, and BEFORE
 *    the generic operator-sites matcher would otherwise swallow them. Since
 *    operator-sites only matches `/api/operator/sites` and
 *    `/api/operator/sites/<id>` (no `orchestration` segment) ordering relative
 *    to it is safe either way; placing it just before the operator-sites block
 *    is the cleanest:
 *
 *      const sitesOrchestrationResponse =
 *        dependencies.routeSitesOrchestrationRequest(request, env, ctx)
 *
 *      if (sitesOrchestrationResponse !== undefined) {
 *        return yield* sitesOrchestrationResponse
 *      }
 *
 * 5) No migration is required. This core reuses the existing site_builder_*
 *    tables (sessions, events, phase_runs, previews, repair_attempts) through
 *    the orchestration core in `sites-orchestration.ts`.
 *
 * Routes exposed once wired:
 *   POST /api/operator/sites/orchestration/:sessionId/advance
 *   GET  /api/operator/sites/orchestration/:sessionId
 * Both are operator-gated via requireBrowserSession + isOpenAgentsAdminEmail.
 * ============================================================================
 */
