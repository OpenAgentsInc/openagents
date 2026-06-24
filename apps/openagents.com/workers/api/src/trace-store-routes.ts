import { Effect, Match as M, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifTrajectory,
  type TraceVisibility,
  atifTraceTripwire,
  validateAtifTrajectory,
} from './atif-trace-schema'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, parseJsonUnknown } from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  type TraceBlobRef,
  type TraceRecord,
  type TraceStore,
  TraceStoreError,
  traceStoreErrorFromUnknown,
} from './trace-store-d1'

type HttpResponse = globalThis.Response

/**
 * Trace store + ingest/read API (openagents #6208, epic #6206). Serves the
 * shareable `/trace/{uuid}` surface.
 *
 * - POST /api/traces (requireAgent + Idempotency-Key): validates, structurally
 *   checks, and TRIPWIRES the public-safe ATIF trajectory, then stores it and
 *   returns `{ uuid }`. Rejects leaky payloads (secrets/tokens/wallet/PII/raw
 *   provider model ids) before persistence; rejects unauthenticated callers.
 * - GET /api/traces/{traceRef} (visibility-gated): public/unlisted need no auth;
 *   owner_only requires the owning browser session (or an admin). Returns the
 *   public-safe projection the page renders.
 * - GET /api/traces (requireBrowserSession): owner-scoped list of the caller's
 *   own traces.
 *
 * A trace is evidence only. These routes grant no accepted-work, payout,
 * settlement, or public-claim authority (#6208/#6212, INVARIANTS "Agent Trace
 * Store").
 */

// Bounded ingest size + per-trajectory step cap (abuse controls, #6212).
const MAX_INGEST_BODY_BYTES = 512 * 1024
const MAX_STEPS = 2_000
const MAX_BLOB_REFS = 200
const OWNER_LIST_LIMIT = 100

// A trajectory model_name is verified by the tripwire; only `openagents/...`.
const TraceBlobRefSchema = S.Struct({
  kind: S.Literals(['video', 'screenshot', 'image']),
  r2Key: S.String,
  contentType: S.optionalKey(S.String),
  caption: S.optionalKey(S.String),
})

class IngestTraceRequest extends S.Class<IngestTraceRequest>(
  'IngestTraceRequest',
)({
  trajectory: AtifTrajectory,
  visibility: S.optionalKey(
    S.Literals(['public', 'unlisted', 'owner_only']),
  ),
  blobRefs: S.optionalKey(S.Array(TraceBlobRefSchema)),
}) {}

type TraceBrowserSession = Readonly<{
  user: Readonly<{
    email?: string | undefined
    userId: string
  }>
}>

type TraceStoreRouteDependencies<Bindings, Session extends TraceBrowserSession> =
  Readonly<{
    agentStore: (env: Bindings) => AgentRegistrationStore
    makeStore: (env: Bindings) => TraceStore
    requireBrowserSession: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ) => Promise<Session | undefined>
    appendRefreshedSessionCookies: (
      response: HttpResponse,
      session: Session,
    ) => HttpResponse
    isAdminEmail: (email: string) => boolean
    makeId?: () => string
    nowIso?: () => string
  }>

// ---------------------------------------------------------------------------
// Typed errors -> HTTP
// ---------------------------------------------------------------------------

class TraceUnauthorized extends S.TaggedErrorClass<TraceUnauthorized>()(
  'TraceUnauthorized',
  {},
) {}

class TraceValidationError extends S.TaggedErrorClass<TraceValidationError>()(
  'TraceValidationError',
  { reason: S.String },
) {}

class TraceTripwireRejected extends S.TaggedErrorClass<TraceTripwireRejected>()(
  'TraceTripwireRejected',
  { findings: S.Array(S.String) },
) {}

class TraceNotFound extends S.TaggedErrorClass<TraceNotFound>()(
  'TraceNotFound',
  {},
) {}

class TraceForbidden extends S.TaggedErrorClass<TraceForbidden>()(
  'TraceForbidden',
  {},
) {}

type TraceRouteError =
  | TraceUnauthorized
  | TraceValidationError
  | TraceTripwireRejected
  | TraceNotFound
  | TraceForbidden
  | TraceStoreError

const routeErrorResponse = (error: TraceRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      TraceUnauthorized: () => unauthorized(),
      TraceForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      TraceNotFound: () =>
        noStoreJsonResponse({ error: 'trace_not_found' }, { status: 404 }),
      TraceValidationError: error =>
        noStoreJsonResponse(
          { error: 'invalid_trace', reason: error.reason },
          { status: 400 },
        ),
      TraceTripwireRejected: error =>
        // Public-safety tripwire: the payload contained material that must never
        // be stored. We return the finding CODES only (never echo the offending
        // values back) so the producer can fix it without us re-emitting secrets.
        noStoreJsonResponse(
          {
            error: 'trace_public_safety_rejected',
            message:
              'Trace payload contains material that must not be stored (secrets, tokens, wallet/payment material, PII, local paths, or raw provider model ids).',
            findings: error.findings,
          },
          { status: 422 },
        ),
      TraceStoreError: error =>
        noStoreJsonResponse(
          { error: `trace_store_${error.kind}`, reason: error.reason },
          {
            status:
              error.kind === 'not_found'
                ? 404
                : error.kind === 'conflict'
                  ? 409
                  : 500,
          },
        ),
    }),
    M.exhaustive,
  )

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')
  if (authorization === null) {
    return undefined
  }
  const [scheme, token] = authorization.split(' ')
  return scheme?.toLowerCase() === 'bearer' &&
    token !== undefined &&
    token.startsWith(AGENT_TOKEN_PREFIX)
    ? token
    : undefined
}

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('idempotency-key')?.trim()
  return value === undefined || value === '' ? undefined : value
}

const routeNowIso = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
): string => (dependencies.makeId ?? randomUuid)()

const requireAgent = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
): Effect.Effect<ProgrammaticAgentSession, TraceUnauthorized> => {
  const token = bearerTokenFromRequest(request)
  if (token === undefined) {
    return Effect.fail(new TraceUnauthorized({}))
  }
  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new TraceUnauthorized({}),
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStore(env),
          token,
          dependencies.nowIso,
        ),
    }),
    session =>
      session === undefined
        ? Effect.fail(new TraceUnauthorized({}))
        : Effect.succeed(session),
  )
}

const publicTraceProjection = (record: TraceRecord) => ({
  uuid: record.traceUuid,
  schemaVersion: record.schemaVersion,
  trajectoryId: record.trajectoryId,
  ...(record.sessionId === null ? {} : { sessionId: record.sessionId }),
  visibility: record.visibility,
  agentRef: record.agentRef,
  stepCount: record.stepCount,
  trajectory: record.trajectory,
  blobRefs: record.blobRefs,
  createdAt: record.createdAt,
  authority: {
    acceptedWorkAuthority: false,
    payoutAuthority: false,
    publicClaimAuthority: false,
  },
})

const ownerTraceSummary = (record: TraceRecord) => ({
  uuid: record.traceUuid,
  trajectoryId: record.trajectoryId,
  visibility: record.visibility,
  agentRef: record.agentRef,
  stepCount: record.stepCount,
  createdAt: record.createdAt,
})

// ---------------------------------------------------------------------------
// POST /api/traces — ingest
// ---------------------------------------------------------------------------

const routeIngest = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const session = yield* requireAgent(dependencies, request, env)

    const idempotencyKey = idempotencyKeyFromRequest(request)
    if (idempotencyKey === undefined) {
      return yield* new TraceValidationError({
        reason: 'Trace ingest requires an Idempotency-Key header.',
      })
    }

    // Size cap before parsing (abuse control). Falls back to byte-length of the
    // body text when Content-Length is absent.
    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new TraceValidationError({ reason: 'Request body could not be read.' }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_INGEST_BODY_BYTES) {
      return yield* new TraceValidationError({
        reason: `Trace payload exceeds the ${MAX_INGEST_BODY_BYTES}-byte ingest limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new TraceValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the trace ingest schema.',
        }),
      try: () => {
        const parsed: unknown =
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody)
        return decodeUnknownWithSchema(IngestTraceRequest, parsed)
      },
    })

    const trajectory = body.trajectory

    if (trajectory.schema_version !== ATIF_PINNED_SCHEMA_VERSION) {
      return yield* new TraceValidationError({
        reason: `Unsupported schema_version; expected ${ATIF_PINNED_SCHEMA_VERSION}.`,
      })
    }

    if (trajectory.steps.length > MAX_STEPS) {
      return yield* new TraceValidationError({
        reason: `Trace exceeds the ${MAX_STEPS}-step limit.`,
      })
    }

    if ((body.blobRefs?.length ?? 0) > MAX_BLOB_REFS) {
      return yield* new TraceValidationError({
        reason: `Trace exceeds the ${MAX_BLOB_REFS}-blob-ref limit.`,
      })
    }

    const structuralIssues = validateAtifTrajectory(trajectory)
    if (structuralIssues.length > 0) {
      return yield* new TraceValidationError({
        reason: structuralIssues.map(issue => issue.message).join(' '),
      })
    }

    // Public-safety tripwire: reject (do not redact) leaky payloads BEFORE
    // persistence. We never store secrets/tokens/wallet/PII/raw provider ids.
    const tripwireFindings = atifTraceTripwire(trajectory)
    if (tripwireFindings.length > 0) {
      return yield* new TraceTripwireRejected({
        findings: tripwireFindings.map(finding => finding.code),
      })
    }

    // Visibility: explicit request override wins; else the trajectory's own
    // `visibility`; else default unlisted.
    const visibility: TraceVisibility =
      body.visibility ?? trajectory.visibility ?? 'unlisted'

    const traceUuid = routeMakeId(dependencies)
    const nowIso = routeNowIso(dependencies)
    const blobRefs: ReadonlyArray<TraceBlobRef> = (body.blobRefs ?? []).map(
      ref => ({
        kind: ref.kind,
        r2Key: ref.r2Key,
        ...(ref.contentType === undefined
          ? {}
          : { contentType: ref.contentType }),
        ...(ref.caption === undefined ? {} : { caption: ref.caption }),
      }),
    )

    const stored = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () =>
        dependencies.makeStore(env).createTrace({
          traceUuid,
          ownerUserId: session.user.id,
          agentRef: `agent:${session.user.id}`,
          schemaVersion: trajectory.schema_version,
          trajectoryId: trajectory.trajectory_id,
          sessionId: trajectory.session_id ?? null,
          visibility,
          stepCount: trajectory.steps.length,
          trajectory,
          blobRefs,
          idempotencyKey,
          nowIso,
        }),
    })

    // 201 on first store, 200 on idempotent replay.
    return noStoreJsonResponse(
      {
        uuid: stored.record.traceUuid,
        url: `/trace/${stored.record.traceUuid}`,
        visibility: stored.record.visibility,
        replay: !stored.created,
      },
      { status: stored.created ? 201 : 200 },
    )
  })

// ---------------------------------------------------------------------------
// GET /api/traces/{traceRef} — read (visibility-gated)
// ---------------------------------------------------------------------------

const routeRead = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  traceRef: string,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const record = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readTraceByUuid(traceRef),
    })

    if (record === undefined) {
      return yield* new TraceNotFound({})
    }

    // public + unlisted are readable by anyone with the link (no auth). Only
    // owner_only requires the owning browser session (or an admin).
    if (record.visibility !== 'owner_only') {
      return noStoreJsonResponse({ trace: publicTraceProjection(record) })
    }

    const session = yield* Effect.tryPromise({
      catch: () => new TraceForbidden({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.succeed(undefined)))

    if (session === undefined) {
      // Do not reveal existence of an owner_only trace to anonymous callers.
      return yield* new TraceNotFound({})
    }

    const isOwner = session.user.userId === record.ownerUserId
    const isAdmin =
      session.user.email !== undefined &&
      dependencies.isAdminEmail(session.user.email)

    if (!isOwner && !isAdmin) {
      return yield* new TraceNotFound({})
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({ trace: publicTraceProjection(record) }),
      session,
    )
  })

// ---------------------------------------------------------------------------
// GET /api/traces — owner-scoped list (requireBrowserSession)
// ---------------------------------------------------------------------------

const routeOwnerList = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      catch: () => new TraceUnauthorized({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.succeed(undefined)))

    if (session === undefined) {
      return yield* new TraceUnauthorized({})
    }

    const traces = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () =>
        dependencies
          .makeStore(env)
          .listTracesForOwner(session.user.userId, OWNER_LIST_LIMIT),
    })

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({ traces: traces.map(ownerTraceSummary) }),
      session,
    )
  })

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

export const makeTraceStoreRoutes = <
  Bindings,
  Session extends TraceBrowserSession,
>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
) => ({
  routeTraceRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/traces') {
      if (request.method === 'POST') {
        return routeIngest(dependencies, request, env).pipe(
          Effect.map(withAgentRateLimitHeaders),
          Effect.catch(error =>
            Effect.succeed(
              withAgentRateLimitHeaders(routeErrorResponse(error)),
            ),
          ),
        )
      }
      if (request.method === 'GET') {
        return routeOwnerList(dependencies, request, env, ctx).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        )
      }
      return Effect.succeed(methodNotAllowed(['GET', 'POST']))
    }

    const readMatch = /^\/api\/traces\/([^/]+)$/.exec(url.pathname)
    if (readMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }
      return routeRead(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(readMatch[1] ?? ''),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    return undefined
  },
})
