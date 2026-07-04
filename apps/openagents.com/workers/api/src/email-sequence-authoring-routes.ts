// COORDINATOR WIRING (integration deferred — do NOT wire on this branch)
//
// This module follows the makeOperatorEmailInspectionRoutes /
// makeAutopilotDecisionRoutes pattern: a factory returning a single
// `routeEmailSequenceAuthoringRequest(request, env, ctx) => Effect | undefined`
// that returns `undefined` when the path is not owned.
//
// 1) Construct it in workers/api/src/index.ts next to the other operator route
//    factories (e.g. near `const operatorEmailInspectionRoutes = ...`):
//
//      const emailSequenceAuthoringRoutes = makeEmailSequenceAuthoringRoutes({
//        appendRefreshedSessionCookies,
//        isOpenAgentsAdminEmail,
//        requireAdminApiToken,
//        requireBrowserSession,
//      })
//
//    (The factory only needs OPENAGENTS_DB on the bindings; it reads the DB via
//    makeCrmEmailDatabaseForEnv(env) like operator-email-inspection-routes.ts.)
//
// 2) Expose it on the worker-routes dependency object alongside the other
//    operator routes. Add to WorkerRouteDependencies in
//    workers/api/src/worker-routes.ts:
//
//      routeEmailSequenceAuthoringRequest: OptionalEffectRoute
//
//    and in index.ts where that object is built:
//
//      routeEmailSequenceAuthoringRequest:
//        emailSequenceAuthoringRoutes.routeEmailSequenceAuthoringRequest,
//
// 3) Chain it into routeOmniRequest's `??` fallthrough chain in index.ts next
//    to the other operator routes, e.g.:
//
//      operatorEmailInspectionRoutes.routeOperatorEmailInspectionRequest(
//        request, env, ctx,
//      ) ??
//      emailSequenceAuthoringRoutes.routeEmailSequenceAuthoringRequest(
//        request, env, ctx,
//      ) ??
//      ...
//
//    Order does not matter for correctness because the factory returns
//    `undefined` for non-owned paths; keep it grouped with the other
//    `/api/operator/*` routes for readability.
//
// Routes owned by this module (all operator-gated):
//   POST /api/operator/email-sequences
//     -> create/upsert a sequence (campaign) + ordered steps
//   POST /api/operator/email-sequences/:slug/status
//     -> update lifecycle status (draft|active|paused|archived)
//   POST /api/operator/email-sequences/:slug/enroll
//     -> enroll a subscriber and schedule per-step sends
//
// No new migration: reuses migration 0063 (email_campaigns,
// email_campaign_steps, email_campaign_enrollments, email_campaign_sends).

// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import { makeCrmEmailDatabaseForEnv } from './crm-email-domain-store'
import { Effect, Match as M, Schema as S } from 'effect'

import {
  type CreateEmailSequenceRequest,
  type EnrollSubscriberRequest,
  type UpdateEmailSequenceStatusRequest,
  createEmailSequence,
  decodeCreateEmailSequenceRequest,
  decodeEnrollSubscriberRequest,
  decodeUpdateEmailSequenceStatusRequest,
  enrollSubscriberInSequence,
  projectEmailSequenceDefinition,
  updateEmailSequenceStatus,
} from './email-sequence-authoring'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { readJsonObject } from './json-boundary'

type EmailSequenceAuthoringEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type EmailSequenceAuthoringSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type EmailSequenceAuthoringRouteDependencies<
  Session extends EmailSequenceAuthoringSession,
  Bindings extends EmailSequenceAuthoringEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

class EmailSequenceAuthoringUnauthorized extends S.TaggedErrorClass<EmailSequenceAuthoringUnauthorized>()(
  'EmailSequenceAuthoringUnauthorized',
  {},
) {}

class EmailSequenceAuthoringForbidden extends S.TaggedErrorClass<EmailSequenceAuthoringForbidden>()(
  'EmailSequenceAuthoringForbidden',
  {},
) {}

class EmailSequenceAuthoringSessionError extends S.TaggedErrorClass<EmailSequenceAuthoringSessionError>()(
  'EmailSequenceAuthoringSessionError',
  {
    error: S.Defect,
  },
) {}

class EmailSequenceAuthoringBadRequest extends S.TaggedErrorClass<EmailSequenceAuthoringBadRequest>()(
  'EmailSequenceAuthoringBadRequest',
  {
    reason: S.String,
  },
) {}

class EmailSequenceAuthoringNotFound extends S.TaggedErrorClass<EmailSequenceAuthoringNotFound>()(
  'EmailSequenceAuthoringNotFound',
  {},
) {}

class EmailSequenceAuthoringStorageError extends S.TaggedErrorClass<EmailSequenceAuthoringStorageError>()(
  'EmailSequenceAuthoringStorageError',
  {
    reason: S.String,
  },
) {}

type EmailSequenceAuthoringRouteError =
  | EmailSequenceAuthoringBadRequest
  | EmailSequenceAuthoringForbidden
  | EmailSequenceAuthoringNotFound
  | EmailSequenceAuthoringSessionError
  | EmailSequenceAuthoringStorageError
  | EmailSequenceAuthoringUnauthorized

const routeErrorResponse = (
  error: EmailSequenceAuthoringRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      EmailSequenceAuthoringBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      EmailSequenceAuthoringForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      EmailSequenceAuthoringNotFound: () =>
        noStoreJsonResponse(
          { error: 'email_sequence_not_found' },
          { status: 404 },
        ),
      EmailSequenceAuthoringSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      EmailSequenceAuthoringStorageError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'email_sequence_storage_error', reason },
          { status: 500 },
        ),
      EmailSequenceAuthoringUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const requireAdminSession = <
  Session extends EmailSequenceAuthoringSession,
  Bindings extends EmailSequenceAuthoringEnv,
>(
  dependencies: EmailSequenceAuthoringRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new EmailSequenceAuthoringSessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken === true) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new EmailSequenceAuthoringSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new EmailSequenceAuthoringUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new EmailSequenceAuthoringForbidden({})
    }

    return session
  })

const decodeJsonBody = <A>(
  request: Request,
  decode: (input: unknown) => A,
): Effect.Effect<A, EmailSequenceAuthoringBadRequest> =>
  Effect.tryPromise({
    catch: error =>
      new EmailSequenceAuthoringBadRequest({
        reason: error instanceof Error ? error.message : 'invalid json',
      }),
    try: () => readJsonObject(request),
  }).pipe(
    Effect.flatMap(payload =>
      Effect.try({
        catch: error =>
          new EmailSequenceAuthoringBadRequest({
            reason: error instanceof Error ? error.message : String(error),
          }),
        try: () => decode(payload),
      }),
    ),
  )

const storage = <A>(
  reason: string,
  run: () => Promise<A>,
): Effect.Effect<A, EmailSequenceAuthoringStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new EmailSequenceAuthoringStorageError({
        reason: error instanceof Error ? error.message : reason,
      }),
    try: run,
  })

const SEQUENCE_STATUS_PATH = /^\/api\/operator\/email-sequences\/([^/]+)\/status$/
const SEQUENCE_ENROLL_PATH = /^\/api\/operator\/email-sequences\/([^/]+)\/enroll$/

const slugFromMatch = (
  pathname: string,
  pattern: RegExp,
): string | undefined => {
  const match = pattern.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const handleCreate = <
  Session extends EmailSequenceAuthoringSession,
  Bindings extends EmailSequenceAuthoringEnv,
>(
  dependencies: EmailSequenceAuthoringRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const body: CreateEmailSequenceRequest = yield* decodeJsonBody(
      request,
      decodeCreateEmailSequenceRequest,
    )
    const definition = yield* storage('create failed', () =>
      createEmailSequence(
        makeCrmEmailDatabaseForEnv(env),
        session.user.userId,
        body,
      ),
    )

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        { sequence: projectEmailSequenceDefinition(definition) },
        { status: 201 },
      ),
      session,
    )
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const handleStatus = <
  Session extends EmailSequenceAuthoringSession,
  Bindings extends EmailSequenceAuthoringEnv,
>(
  dependencies: EmailSequenceAuthoringRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  slug: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const body: UpdateEmailSequenceStatusRequest = yield* decodeJsonBody(
      request,
      decodeUpdateEmailSequenceStatusRequest,
    )
    const definition = yield* storage('status update failed', () =>
      updateEmailSequenceStatus(makeCrmEmailDatabaseForEnv(env), slug, body),
    )

    if (definition === null) {
      return yield* new EmailSequenceAuthoringNotFound({})
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        sequence: projectEmailSequenceDefinition(definition),
      }),
      session,
    )
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const handleEnroll = <
  Session extends EmailSequenceAuthoringSession,
  Bindings extends EmailSequenceAuthoringEnv,
>(
  dependencies: EmailSequenceAuthoringRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  slug: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const session = yield* requireAdminSession(dependencies, request, env, ctx)
    const body: EnrollSubscriberRequest = yield* decodeJsonBody(
      request,
      decodeEnrollSubscriberRequest,
    )
    const result = yield* storage('enroll failed', () =>
      enrollSubscriberInSequence(
        makeCrmEmailDatabaseForEnv(env),
        slug,
        body,
        session.user.userId,
      ),
    )

    if (result === null) {
      return yield* new EmailSequenceAuthoringNotFound({})
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        { enrollment: result },
        { status: result.status === 'enrolled' ? 201 : 200 },
      ),
      session,
    )
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

export const makeEmailSequenceAuthoringRoutes = <
  Session extends EmailSequenceAuthoringSession,
  Bindings extends EmailSequenceAuthoringEnv,
>(
  dependencies: EmailSequenceAuthoringRouteDependencies<Session, Bindings>,
) => ({
  routeEmailSequenceAuthoringRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/email-sequences') {
      return M.value(request.method).pipe(
        M.when('POST', () => handleCreate(dependencies, request, env, ctx)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const statusSlug = slugFromMatch(url.pathname, SEQUENCE_STATUS_PATH)

    if (statusSlug !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          handleStatus(dependencies, request, env, ctx, statusSlug),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const enrollSlug = slugFromMatch(url.pathname, SEQUENCE_ENROLL_PATH)

    if (enrollSlug !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          handleEnroll(dependencies, request, env, ctx, enrollSlug),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    return undefined
  },
})
