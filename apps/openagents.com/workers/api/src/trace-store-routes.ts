import { Effect, Match as M, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  sha256Hex,
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
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'
import {
  type TraceBlobRef,
  type TraceDemandKind,
  type TraceMediaBlobStore,
  type TraceRecord,
  type TraceStore,
  TraceStoreError,
  type TraceTrajectoryBlobStore,
  type TraceUploadSource,
  TRACE_DEMAND_KINDS,
  TRACE_INTERNAL_DEMAND_KINDS,
  parseTraceDemandKind,
  traceStoreErrorFromUnknown,
} from './trace-store-d1'

type HttpResponse = globalThis.Response

/**
 * Trace store + ingest/read API (openagents #6208, #6221, epic #6206). Serves
 * the shareable `/trace/{uuid}` surface and the trace upload data market.
 *
 * - POST /api/traces (authenticated + Idempotency-Key): validates, structurally
 *   checks, and TRIPWIRES the public-safe ATIF trajectory, then stores it and
 *   returns `{ uuid }`. Accepts EITHER a registered-agent bearer token OR an
 *   authenticated user web session (#6221) — a signed-in human owns the upload
 *   (`ownerUserId` from the session). Rejects leaky payloads
 *   (secrets/tokens/wallet/PII/raw provider model ids) before persistence;
 *   rejects unauthenticated callers.
 * - Data market (#6221): the uploader can grant `trainingConsent` (default
 *   WITHHELD) to use the trace as training/eval data for Khala, with an optional
 *   public-safe `license` label. A per-trace revshare reward marker is recorded
 *   INERT — eligible-only, amount TBD, flag-gated default-OFF, no money path.
 *   Anti-abuse: per-user upload rate limiting + per-owner content-digest dedup
 *   (a duplicate upload is rejected and never earns a second reward).
 * - GET /api/traces/{traceRef} (visibility-gated): public/unlisted need no auth;
 *   owner_only requires the owning browser session (or an admin). Returns the
 *   public-safe projection the page renders.
 * - GET /api/traces (requireBrowserSession): owner-scoped list of the caller's
 *   own traces.
 *
 * A trace is evidence only. These routes grant no accepted-work, payout,
 * settlement, or public-claim authority (#6208/#6212/#6221, INVARIANTS "Agent
 * Trace Store" + "Trace Upload Data Market"). The revshare marker is INERT.
 */


// Bounded ingest size + per-trajectory step cap (abuse controls, #6212/#6221).
// A real full agent session (e.g. a ~793-step Claude Code session ≈ 2.5MB
// redacted ATIF) is legitimately a few MB, so the body cap is 8MB. The step cap
// stays the real abuse bound. A trajectory too large to inline in a single D1
// value (~1MB) is offloaded to R2 with only a pointer kept in D1 (#6221).
const MAX_INGEST_BODY_BYTES = 8 * 1024 * 1024
const MAX_STEPS = 2_000
const MAX_BLOB_REFS = 200
const OWNER_LIST_LIMIT = 100

// Inline-vs-R2 threshold for the trajectory JSON. D1 caps a single value at
// ~1MB; we keep a conservative inline ceiling and offload anything larger to R2.
const MAX_INLINE_TRAJECTORY_BYTES = 768 * 1024

// Per-blob media upload cap (#6223). A QA recording is a few MB; the screenshots
// are small. The cap bounds a single blob's bytes (R2-backed, like the
// trajectory offload). Public-safe media only — no secrets to scan.
const MAX_MEDIA_BLOB_BYTES = 32 * 1024 * 1024

// Per-user upload rate limit (anti-abuse, #6221): at most this many stored
// traces per owner within the rolling window.
const UPLOAD_RATE_WINDOW_MS = 60 * 60 * 1000
const UPLOAD_RATE_MAX_PER_WINDOW = 120

// A trajectory model_name is verified by the tripwire; only `openagents/...`.
const TraceBlobRefSchema = S.Struct({
  kind: S.Literals(['video', 'screenshot', 'image']),
  r2Key: S.String,
  contentType: S.optionalKey(S.String),
  caption: S.optionalKey(S.String),
})

// A public-safe license label. Bounded; the tripwire still scans it for leaks.
const LicenseSchema = S.String.check(S.isMaxLength(120))

class IngestTraceRequest extends S.Class<IngestTraceRequest>(
  'IngestTraceRequest',
)({
  trajectory: AtifTrajectory,
  visibility: S.optionalKey(
    S.Literals(['public', 'unlisted', 'owner_only']),
  ),
  blobRefs: S.optionalKey(S.Array(TraceBlobRefSchema)),
  // Data market (#6221): the uploader's explicit grant to use this trace as
  // training/eval data for Khala. Omitted => WITHHELD (never assumed).
  trainingConsent: S.optionalKey(S.Boolean),
  license: S.optionalKey(LicenseSchema),
}) {}

class UpdateTraceVisibilityRequest extends S.Class<UpdateTraceVisibilityRequest>(
  'UpdateTraceVisibilityRequest',
)({
  visibility: S.Literals(['public', 'unlisted', 'owner_only']),
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
    /**
     * R2 store for large trajectory JSON (#6221). Optional: when absent, a
     * trajectory larger than the inline D1 ceiling is rejected (413) instead of
     * being truncated. When present, large trajectories are offloaded to R2 and
     * only a pointer is kept in D1.
     */
    trajectoryBlobStore?: (env: Bindings) => TraceTrajectoryBlobStore | undefined
    /**
     * R2 store for a trace's playable media blobs (#6223): the recording +
     * screenshots referenced from `blobRefs[]`. Optional: when absent, blob
     * upload (`POST /api/traces/{uuid}/blob/{r2Key}`) and serve
     * (`GET /api/traces/{uuid}/blob/{r2Key}`) report the media store as
     * unconfigured (404 on serve, 501 on upload) instead of guessing. When
     * present, the bytes are stored under `trace-blobs/{uuid}/{r2Key}` so the
     * `/trace/{uuid}` page plays them without any GitHub attachment.
     */
    mediaBlobStore?: (env: Bindings) => TraceMediaBlobStore | undefined
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
    /**
     * Owner-gated arming of the INERT data-market revshare stub (#6221).
     * Default OFF: when not armed, no reward-eligibility marker is recorded.
     * Even when armed, the marker is eligible-only with a TBD amount — it moves
     * no money and grants no payout/settlement/spend authority.
     */
    dataMarketRewardArmed?: (env: Bindings) => boolean
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

class TraceRateLimited extends S.TaggedErrorClass<TraceRateLimited>()(
  'TraceRateLimited',
  {},
) {}

class TraceDuplicate extends S.TaggedErrorClass<TraceDuplicate>()(
  'TraceDuplicate',
  { uuid: S.String },
) {}

class TracePayloadTooLarge extends S.TaggedErrorClass<TracePayloadTooLarge>()(
  'TracePayloadTooLarge',
  { reason: S.String },
) {}

class TraceMediaUnavailable extends S.TaggedErrorClass<TraceMediaUnavailable>()(
  'TraceMediaUnavailable',
  {},
) {}

type TraceRouteError =
  | TraceUnauthorized
  | TraceValidationError
  | TraceTripwireRejected
  | TraceNotFound
  | TraceForbidden
  | TraceRateLimited
  | TraceDuplicate
  | TracePayloadTooLarge
  | TraceMediaUnavailable
  | TraceStoreError

const routeErrorResponse = (error: TraceRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      TraceUnauthorized: () => unauthorized(),
      TraceForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      TraceRateLimited: () =>
        noStoreJsonResponse(
          {
            error: 'trace_rate_limited',
            message:
              'Trace upload rate limit reached for this account. Try again later.',
          },
          { status: 429 },
        ),
      TraceDuplicate: error =>
        // Dedup (#6221): the same owner already uploaded this exact payload. We
        // return the existing uuid (idempotent-ish) but never a second reward.
        noStoreJsonResponse(
          {
            error: 'trace_duplicate',
            message:
              'An identical trace was already uploaded by this account; it was not stored again.',
            uuid: error.uuid,
            url: `/trace/${error.uuid}`,
            duplicate: true,
          },
          { status: 409 },
        ),
      TraceNotFound: () =>
        noStoreJsonResponse({ error: 'trace_not_found' }, { status: 404 }),
      TracePayloadTooLarge: error =>
        noStoreJsonResponse(
          { error: 'trace_payload_too_large', reason: error.reason },
          { status: 413 },
        ),
      TraceMediaUnavailable: () =>
        noStoreJsonResponse(
          {
            error: 'trace_media_store_unconfigured',
            message:
              'Trace media blob storage is not configured on this deployment.',
          },
          { status: 501 },
        ),
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

/**
 * A read-only owner-scope token (mobile "Open traces in web", #6347). The
 * Khala mobile app opens `/traces?token=<oa_agent_…>` (the user's own Khala API
 * key from the device Keychain) so an owner can view THEIR OWN traces in the
 * browser WITHOUT a web login. The token is accepted from EITHER the
 * `Authorization: Bearer` header OR a `?token=` query parameter (the URL form
 * the app uses). It must be an `oa_agent_` key; any other shape is ignored.
 *
 * This token is a bearer for OWNER-SCOPED TRACE READ ONLY. It is never used to
 * authorize ingest, visibility writes, blob upload, admin, or any broader
 * account access — only the GET read/list paths consult it, and they scope
 * strictly to the resolved owner's own traces (see callers). We never log the
 * token value.
 */
const readScopeTokenFromRequest = (
  request: Request,
  url: URL,
): string | undefined => {
  const bearer = bearerTokenFromRequest(request)
  if (bearer !== undefined) {
    return bearer
  }
  const queryToken = url.searchParams.get('token')?.trim()
  return queryToken !== undefined &&
    queryToken !== '' &&
    queryToken.startsWith(AGENT_TOKEN_PREFIX)
    ? queryToken
    : undefined
}

/**
 * Resolve a READ-ONLY owner-scope identity from an `oa_agent_` token (header or
 * `?token=`). Linked Pylon/Codex traces are stored under the linked OpenAuth
 * owner id, while ordinary agent-owned traces are stored under the agent user
 * id, so the read scope mirrors that ownership choice. A missing/invalid token
 * resolves to `undefined` (no scope), never an error, so the caller can still
 * fall back to the browser session. Read-only: this grants NO write/admin
 * authority.
 */
const resolveReadScopeOwner = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  url: URL,
  env: Bindings,
): Effect.Effect<string | undefined> =>
  Effect.gen(function* () {
    const token = readScopeTokenFromRequest(request, url)
    if (token === undefined) {
      return undefined
    }
    const session = yield* Effect.tryPromise({
      catch: () => undefined,
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStore(env),
          token,
          dependencies.nowIso,
        ),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))
    return session === undefined
      ? undefined
      : (session.credential.openauthUserId?.trim() || session.user.id)
  })

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

/**
 * A resolved upload identity (#6221). Either a registered agent (bearer) or an
 * authenticated user web session. The uploader OWNS the resulting trace.
 */
type TraceUploader = Readonly<{
  ownerUserId: string
  agentRef: string
  uploadSource: TraceUploadSource
}>

/**
 * Resolve the uploader from EITHER a registered-agent bearer token OR an
 * authenticated user web session. The agent-bearer path takes precedence when a
 * bearer is present so existing agent ingest is unchanged; otherwise we fall
 * back to the signed-in browser session so a human can upload + own a trace.
 */
const requireUploader = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<TraceUploader, TraceUnauthorized> =>
  Effect.gen(function* () {
    if (bearerTokenFromRequest(request) !== undefined) {
      const session = yield* requireAgent(dependencies, request, env)
      return {
        ownerUserId: session.user.id,
        agentRef: `agent:${session.user.id}`,
        uploadSource: 'agent' as const,
      }
    }

    const session = yield* Effect.tryPromise({
      catch: () => new TraceUnauthorized({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))

    if (session === undefined) {
      return yield* new TraceUnauthorized({})
    }

    return {
      ownerUserId: session.user.userId,
      agentRef: `user:${session.user.userId}`,
      uploadSource: 'user_session' as const,
    }
  })

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
  // Data market (#6221), public-safe. The reward marker is INERT: eligibility
  // only, amount TBD; it moves no money and grants no payout authority.
  dataMarket: {
    trainingConsent: record.trainingConsent,
    ...(record.license === null ? {} : { license: record.license }),
    uploadSource: record.uploadSource,
    reward: {
      eligible: record.rewardEligible,
      amountSats: record.rewardAmountSats,
      status: 'tbd' as const,
    },
  },
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
  trainingConsent: record.trainingConsent,
  ...(record.license === null ? {} : { license: record.license }),
  uploadSource: record.uploadSource,
  rewardEligible: record.rewardEligible,
  // DEMAND-ORIGIN (#6298): a legacy null row is the unclassified real-user
  // default (`unlabeled`). Surfaced so the operator can see each trace's origin.
  demandKind: record.demandKind ?? 'unlabeled',
  ...(record.demandSource === null
    ? {}
    : { demandSource: record.demandSource }),
})

const requireTraceOwnerOrAdmin = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  record: TraceRecord,
): Effect.Effect<Session, TraceRouteError> =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      catch: () => new TraceUnauthorized({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))

    if (session === undefined) {
      return yield* new TraceUnauthorized({})
    }

    const isOwner = session.user.userId === record.ownerUserId
    const isAdmin =
      session.user.email !== undefined &&
      dependencies.isAdminEmail(session.user.email)

    if (!isOwner && !isAdmin) {
      return yield* new TraceNotFound({})
    }

    return session
  })

// ---------------------------------------------------------------------------
// POST /api/traces — ingest
// ---------------------------------------------------------------------------

const routeIngest = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const uploader = yield* requireUploader(dependencies, request, env, ctx)
    const store = dependencies.makeStore(env)

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

    // Per-user upload rate limit (anti-abuse, #6221). An idempotent replay of an
    // already-stored trace is exempt — the create path returns it unchanged.
    const sinceIso = isoTimestampAfterIso(nowIso, -UPLOAD_RATE_WINDOW_MS)
    const recentCount = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => store.countTracesForOwnerSince(uploader.ownerUserId, sinceIso),
    })
    if (recentCount >= UPLOAD_RATE_MAX_PER_WINDOW) {
      const replayExempt =
        idempotencyKey !== undefined &&
        (yield* Effect.tryPromise({
          catch: traceStoreErrorFromUnknown,
          try: () => store.readTraceByUuid(idempotencyKey),
        }).pipe(Effect.catch(() => Effect.sync(() => undefined)))) !== undefined
      if (!replayExempt) {
        return yield* new TraceRateLimited({})
      }
    }

    // Content-digest dedup (#6221): a SHA-256 over the canonical public-safe
    // payload. A duplicate upload from the same owner is rejected (no double
    // reward). We dedup on the trajectory + blob refs, independent of the
    // Idempotency-Key (a different key with identical content still dedups).
    const contentDigest = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () =>
        sha256Hex(
          JSON.stringify({
            ownerUserId: uploader.ownerUserId,
            trajectory,
            blobRefs,
          }),
        ),
    })
    const duplicate = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () =>
        store.findTraceByOwnerDigest(uploader.ownerUserId, contentDigest),
    })
    if (duplicate !== undefined && duplicate.idempotencyKey !== idempotencyKey) {
      return yield* new TraceDuplicate({ uuid: duplicate.traceUuid })
    }

    // Consent: WITHHELD unless explicitly granted (#6221). Never assumed.
    const trainingConsent = body.trainingConsent === true
    const license = body.license ?? null

    // INERT revshare stub (#6221): a reward-eligibility marker, owner-gated by
    // the data-market arming flag (default OFF) and only when consent is granted
    // and this is a fresh (non-duplicate) upload. The amount stays null
    // ("reward TBD"); NO money moves and NO payout authority is granted.
    const rewardArmed = dependencies.dataMarketRewardArmed?.(env) ?? false
    const rewardEligible = rewardArmed && trainingConsent
    const rewardAmountSats = null

    const traceUuid = routeMakeId(dependencies)

    // Large-trajectory R2 offload (#6221). D1 caps a single value at ~1MB; a
    // real full agent session can be a few MB. When the serialized public-safe
    // trajectory exceeds the inline ceiling, store the JSON in R2 and keep only
    // a pointer (+ placeholder `{}`) in D1. The trajectory was already tripwired
    // above, so R2 holds nothing more sensitive than D1 would have.
    const trajectoryJson = JSON.stringify(trajectory)
    const trajectoryBytes = new TextEncoder().encode(trajectoryJson).length
    const blobStore = dependencies.trajectoryBlobStore?.(env)

    let inlineTrajectory: unknown = trajectory
    let trajectoryR2Key: string | null = null
    if (trajectoryBytes > MAX_INLINE_TRAJECTORY_BYTES) {
      if (blobStore === undefined) {
        return yield* new TracePayloadTooLarge({
          reason: `Trajectory is ${trajectoryBytes} bytes; the inline store limit is ${MAX_INLINE_TRAJECTORY_BYTES} bytes and no large-trace store is configured.`,
        })
      }
      trajectoryR2Key = yield* Effect.tryPromise({
        catch: traceStoreErrorFromUnknown,
        try: () => blobStore.putTrajectory(traceUuid, trajectoryJson),
      })
      // D1 keeps a placeholder; the real JSON is rehydrated from R2 on read.
      inlineTrajectory = {}
    }

    const stored = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () =>
        store.createTrace({
          traceUuid,
          ownerUserId: uploader.ownerUserId,
          agentRef: uploader.agentRef,
          schemaVersion: trajectory.schema_version,
          trajectoryId: trajectory.trajectory_id,
          sessionId: trajectory.session_id ?? null,
          visibility,
          stepCount: trajectory.steps.length,
          trajectory: inlineTrajectory,
          trajectoryR2Key,
          blobRefs,
          idempotencyKey,
          trainingConsent,
          license,
          contentDigest,
          rewardEligible,
          rewardAmountSats,
          uploadSource: uploader.uploadSource,
          // Direct user/agent uploads (#6221) carry no gateway demand header;
          // they are real external contributions, so demand origin is left
          // unclassified (null => `unlabeled` on read, kept in the default
          // real-user corpus). Demand tagging (#6298) applies to the gateway
          // CAPTURE path, where the chat route resolves the attribution.
          demandKind: null,
          demandSource: null,
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
        dataMarket: {
          trainingConsent: stored.record.trainingConsent,
          ...(stored.record.license === null
            ? {}
            : { license: stored.record.license }),
          uploadSource: stored.record.uploadSource,
          reward: {
            eligible: stored.record.rewardEligible,
            amountSats: stored.record.rewardAmountSats,
            status: 'tbd' as const,
          },
        },
      },
      { status: stored.created ? 201 : 200 },
    )
  })

// ---------------------------------------------------------------------------
// GET /api/traces/{traceRef} — read (visibility-gated)
// ---------------------------------------------------------------------------

/**
 * Rehydrate a record's full trajectory from R2 when it was offloaded (#6221).
 * For inline traces this is a no-op. The public-safe read projection is
 * identical whether the trajectory lived inline in D1 or in R2.
 */
const rehydrateTrajectory = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  env: Bindings,
  record: TraceRecord,
): Effect.Effect<TraceRecord, TraceRouteError> =>
  Effect.gen(function* () {
    if (record.trajectoryR2Key === null) {
      return record
    }
    const blobStore = dependencies.trajectoryBlobStore?.(env)
    if (blobStore === undefined) {
      return record
    }
    const json = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => blobStore.getTrajectory(record.trajectoryR2Key as string),
    })
    if (json === null || json.trim() === '') {
      return record
    }
    const trajectory = yield* Effect.try({
      catch: () => new TraceNotFound({}),
      try: () => parseJsonUnknown(json),
    }).pipe(Effect.catch(() => Effect.succeed(record.trajectory)))
    return { ...record, trajectory }
  })

const routeRead = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  traceRef: string,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const storedRecord = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readTraceByUuid(traceRef),
    })

    if (storedRecord === undefined) {
      return yield* new TraceNotFound({})
    }

    // public + unlisted are readable by anyone with the link (no auth). Only
    // owner_only requires the owning browser session (or an admin).
    if (storedRecord.visibility !== 'owner_only') {
      const record = yield* rehydrateTrajectory(dependencies, env, storedRecord)
      return noStoreJsonResponse({ trace: publicTraceProjection(record) })
    }

    const record = storedRecord

    const session = yield* Effect.tryPromise({
      catch: () => new TraceForbidden({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))

    if (session === undefined) {
      // No web login: allow the OWNER to read THEIR OWN owner_only trace with an
      // `oa_agent_` read-scope token (header or `?token=`, mobile "Open traces
      // in web", #6347). Strictly owner-scoped: the token resolves to a single
      // owner id and may only see traces that owner owns; a cross-owner token is
      // indistinguishable from anonymous (still 404, existence not revealed).
      const tokenOwner = yield* resolveReadScopeOwner(
        dependencies,
        request,
        new URL(request.url),
        env,
      )
      if (tokenOwner !== undefined && tokenOwner === record.ownerUserId) {
        const rehydratedForToken = yield* rehydrateTrajectory(
          dependencies,
          env,
          record,
        )
        return noStoreJsonResponse({
          trace: publicTraceProjection(rehydratedForToken),
        })
      }
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

    const rehydrated = yield* rehydrateTrajectory(dependencies, env, record)
    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({ trace: publicTraceProjection(rehydrated) }),
      session,
    )
  })

// ---------------------------------------------------------------------------
// PATCH /api/traces/{uuid} — owner/admin visibility update
// ---------------------------------------------------------------------------

const routeUpdateVisibility = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  traceRef: string,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const store = dependencies.makeStore(env)
    const existing = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => store.readTraceByUuid(traceRef),
    })

    if (existing === undefined) {
      return yield* new TraceNotFound({})
    }

    const session = yield* requireTraceOwnerOrAdmin(
      dependencies,
      request,
      env,
      ctx,
      existing,
    )

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new TraceValidationError({ reason: 'Request body could not be read.' }),
      try: () => request.text(),
    })

    const body = yield* Effect.try({
      catch: error =>
        new TraceValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the trace visibility schema.',
        }),
      try: () => {
        const parsed: unknown =
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody)
        return decodeUnknownWithSchema(UpdateTraceVisibilityRequest, parsed)
      },
    })

    const updated = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () =>
        store.updateTraceVisibility(
          existing.traceUuid,
          existing.ownerUserId,
          body.visibility,
          routeNowIso(dependencies),
        ),
    })

    if (updated === undefined) {
      return yield* new TraceNotFound({})
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        trace: {
          uuid: updated.traceUuid,
          visibility: updated.visibility,
          updatedAt: updated.updatedAt,
        },
      }),
      session,
    )
  })

// ---------------------------------------------------------------------------
// Trace media blobs (#6223): self-hosted recording + screenshots.
//
//   POST /api/traces/{uuid}/blob/{r2Key}  (agent/uploader auth, size-capped)
//   GET  /api/traces/{uuid}/blob/{r2Key}  (visibility-gated EXACTLY like read)
//
// These make `/trace/{uuid}` play its own media so the page never depends on a
// GitHub attachment. The bytes are PUBLIC-SAFE (video/screenshots of a public
// QA session); the trajectory TEXT is separately tripwired on ingest.
// ---------------------------------------------------------------------------

/**
 * Visibility-gate a trace exactly like the JSON read (`routeRead`): public and
 * unlisted are readable by anyone with the link (no auth); owner_only requires
 * the owning browser session (or an admin) and otherwise 404s (never reveals
 * existence). Returns the record (caller streams the blob) on success.
 */
const authorizeTraceForBlobRead = <
  Bindings,
  Session extends TraceBrowserSession,
>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  traceRef: string,
): Effect.Effect<TraceRecord, TraceRouteError> =>
  Effect.gen(function* () {
    const record = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readTraceByUuid(traceRef),
    })

    if (record === undefined) {
      return yield* new TraceNotFound({})
    }

    if (record.visibility !== 'owner_only') {
      return record
    }

    const session = yield* Effect.tryPromise({
      catch: () => new TraceForbidden({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))

    if (session === undefined) {
      // No web login: allow the OWNER to read THEIR OWN owner_only trace's media
      // with an `oa_agent_` read-scope token (header or `?token=`, #6347),
      // exactly mirroring the JSON read. Strictly owner-scoped; a cross-owner or
      // missing token is indistinguishable from anonymous (404, no existence
      // disclosure).
      const tokenOwner = yield* resolveReadScopeOwner(
        dependencies,
        request,
        new URL(request.url),
        env,
      )
      if (tokenOwner !== undefined && tokenOwner === record.ownerUserId) {
        return record
      }
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

    return record
  })

/** The content type to serve a blob with: the stored blobRef wins, else R2. */
const blobContentType = (
  record: TraceRecord,
  r2Key: string,
  storedContentType: string | undefined,
): string => {
  const ref = record.blobRefs.find(blob => blob.r2Key === r2Key)
  return ref?.contentType ?? storedContentType ?? 'application/octet-stream'
}

/**
 * POST /api/traces/{uuid}/blob/{r2Key} — upload one media blob's bytes for a
 * trace. Restricted to the UPLOADER of the trace (the owning agent bearer or
 * user session): only the owner of a trace can attach its recording. The
 * `r2Key` must be one declared on the trace's `blobRefs[]` (we never accept an
 * arbitrary key). Size-capped; env-armed (501 when no media store configured).
 */
const routeBlobUpload = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  traceRef: string,
  r2Key: string,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const uploader = yield* requireUploader(dependencies, request, env, ctx)

    const mediaStore = dependencies.mediaBlobStore?.(env)
    if (mediaStore === undefined) {
      return yield* new TraceMediaUnavailable({})
    }

    const record = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readTraceByUuid(traceRef),
    })
    if (record === undefined) {
      return yield* new TraceNotFound({})
    }

    // Only the trace owner may attach its media.
    if (record.ownerUserId !== uploader.ownerUserId) {
      return yield* new TraceNotFound({})
    }

    // The key must be one the trace already declared (no arbitrary writes).
    const declaredRef = record.blobRefs.find(blob => blob.r2Key === r2Key)
    if (declaredRef === undefined) {
      return yield* new TraceValidationError({
        reason: `r2Key "${r2Key}" is not declared on this trace's blobRefs.`,
      })
    }

    const bytes = yield* Effect.tryPromise({
      catch: () =>
        new TraceValidationError({ reason: 'Blob body could not be read.' }),
      try: () => request.arrayBuffer(),
    })
    if (bytes.byteLength === 0) {
      return yield* new TraceValidationError({ reason: 'Blob body was empty.' })
    }
    if (bytes.byteLength > MAX_MEDIA_BLOB_BYTES) {
      return yield* new TracePayloadTooLarge({
        reason: `Blob is ${bytes.byteLength} bytes; the per-blob media limit is ${MAX_MEDIA_BLOB_BYTES} bytes.`,
      })
    }

    const contentType =
      declaredRef.contentType ??
      request.headers.get('content-type') ??
      undefined

    const storedKey = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => mediaStore.putBlob(record.traceUuid, r2Key, bytes, contentType),
    })

    return noStoreJsonResponse(
      {
        uuid: record.traceUuid,
        r2Key,
        storedKey,
        bytes: bytes.byteLength,
        ...(contentType === undefined ? {} : { contentType }),
        url: `/api/traces/${record.traceUuid}/blob/${r2Key}`,
      },
      { status: 201 },
    )
  })

/**
 * GET /api/traces/{uuid}/blob/{r2Key} — stream a trace's media blob from R2.
 * Visibility-gated EXACTLY like the JSON read. Streams the R2 object with the
 * stored content type (blobRef wins), a cache-control header, and a strong
 * ETag. 404 when the trace or the blob does not exist. Accept-Ranges is
 * advertised so the recording can seek (R2 honors a ranged `get`).
 */
const routeBlobServe = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  traceRef: string,
  r2Key: string,
): Effect.Effect<HttpResponse, TraceRouteError> =>
  Effect.gen(function* () {
    const record = yield* authorizeTraceForBlobRead(
      dependencies,
      request,
      env,
      ctx,
      traceRef,
    )

    const mediaStore = dependencies.mediaBlobStore?.(env)
    if (mediaStore === undefined) {
      return yield* new TraceNotFound({})
    }

    const object = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => mediaStore.getBlob(record.traceUuid, r2Key),
    })
    if (object === null) {
      return yield* new TraceNotFound({})
    }

    const headers = new Headers()
    headers.set(
      'content-type',
      blobContentType(record, r2Key, object.contentType),
    )
    // public/unlisted media is link-shareable and immutable per uuid+key.
    headers.set(
      'cache-control',
      record.visibility === 'owner_only'
        ? 'private, max-age=60'
        : 'public, max-age=31536000, immutable',
    )
    if (object.httpEtag !== undefined) {
      headers.set('etag', object.httpEtag)
    }
    headers.set('accept-ranges', 'bytes')
    if (Number.isFinite(object.size)) {
      headers.set('content-length', String(object.size))
    }

    const response = new Response(object.body, { headers })

    // owner_only blobs refresh the session cookies (mirrors routeRead's owner
    // path); public/unlisted are anonymous and need none.
    if (record.visibility !== 'owner_only') {
      return response
    }
    const session = yield* Effect.tryPromise({
      catch: () => new TraceForbidden({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))
    return session === undefined
      ? response
      : dependencies.appendRefreshedSessionCookies(response, session)
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
    const url = new URL(request.url)

    const session = yield* Effect.tryPromise({
      catch: () => new TraceUnauthorized({}),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))

    // Resolve the owner scope: the browser session if signed in, otherwise an
    // `oa_agent_` read-scope token (header or `?token=`, mobile "Open traces in
    // web", #6347). The token is read-only and owner-scoped: it can only list
    // the traces owned by the user id it resolves to, never another owner's and
    // never with write/admin authority. No session AND no valid token => 401.
    const tokenOwner =
      session === undefined
        ? yield* resolveReadScopeOwner(dependencies, request, url, env)
        : undefined
    const ownerUserId =
      session !== undefined ? session.user.userId : tokenOwner

    if (ownerUserId === undefined) {
      return yield* new TraceUnauthorized({})
    }

    // DEMAND-ORIGIN CORPUS FILTER (#6298). The owner/corpus list segments by
    // demand origin. By DEFAULT it EXCLUDES internal-dogfood (internal +
    // own_capacity: heartbeat / canary / Terminal-Bench / coding-delegation) so
    // the default view is the genuine external real-user corpus. An explicit
    // `?demand_kind=` (repeatable / comma-separated) overrides that to the named
    // kinds; `?demand_kind=all` returns every kind including internal.
    const store = dependencies.makeStore(env)
    const requested = url.searchParams
      .getAll('demand_kind')
      .flatMap(value => value.split(','))
      .map(value => value.trim().toLowerCase())
      .filter(value => value !== '')
    const wantsAll = requested.includes('all')
    const parsedRequested = requested
      .map(value => parseTraceDemandKind(value))
      .filter((value): value is TraceDemandKind => value !== undefined)
    // Default real-user corpus = every kind EXCEPT internal-dogfood.
    const defaultCorpusKinds = TRACE_DEMAND_KINDS.filter(
      kind => !TRACE_INTERNAL_DEMAND_KINDS.includes(kind),
    )
    const demandFilter: ReadonlyArray<TraceDemandKind> | undefined = wantsAll
      ? undefined
      : parsedRequested.length > 0
        ? parsedRequested
        : defaultCorpusKinds

    const traces = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () =>
        store.listTracesForOwnerByDemand(
          ownerUserId,
          OWNER_LIST_LIMIT,
          demandFilter,
        ),
    })

    // Segmented operator count over ALL of the owner's traces (external vs
    // internal vs own_capacity vs unlabeled), independent of the applied filter.
    const demandCounts = yield* Effect.tryPromise({
      catch: traceStoreErrorFromUnknown,
      try: () => store.countTracesForOwnerByDemand(ownerUserId),
    })

    const listResponse = noStoreJsonResponse({
      traces: traces.map(ownerTraceSummary),
      demandSegments: demandCounts,
      // Echo the applied filter so the operator UI knows internal was excluded
      // by default. `null` means "all kinds" (?demand_kind=all).
      appliedDemandKinds: demandFilter === undefined ? null : demandFilter,
    })

    // Only the browser-session path refreshes cookies; the token path is a
    // read-only owner scope and issues no web session.
    return session === undefined
      ? listResponse
      : dependencies.appendRefreshedSessionCookies(listResponse, session)
  })

// ---------------------------------------------------------------------------
// GET /traces — owner-scoped HTML list page (mobile "Open traces in web", #6347)
//
// The Khala mobile app opens `https://openagents.com/traces?token=<oa_agent_…>`
// in the system browser so an OWNER can view THEIR OWN traces WITHOUT a web
// login. This is a self-contained, read-only, server-rendered page: it resolves
// the owner from the token (header or `?token=`), lists that owner's recent
// traces, and links each to `/trace/{uuid}?token=` so owner_only deep-links also
// open without a login. With no valid token it renders the honest sign-in
// prompt (the logged-in web list lives in the SPA). The page never persists or
// echoes the token outside the owner's own trace links, and grants read only.
// ---------------------------------------------------------------------------

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

// Styled after the openagents.com homepage: black, white, monospace.
const tracesPageShell = (title: string, body: string): string =>
  `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<meta name="robots" content="noindex" />
<title>${escapeHtml(title)} | OpenAgents</title>
<style>
body { background:#000; color:#fff; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; display:flex; justify-content:center; padding:48px 16px; margin:0; }
main { max-width: 720px; width:100%; }
h1 { font-size: 18px; color:#fff; text-transform: uppercase; letter-spacing: 0.08em; margin:0 0 4px; }
p { line-height: 1.6; font-size: 14px; color:#a1a1aa; }
a { color:#fff; }
ul { list-style:none; padding:0; margin:16px 0 0; }
li { border-bottom:1px solid #18181b; padding:12px 0; }
li a { display:block; text-decoration:none; }
.title { color:#e4e4e7; font-size:14px; word-break:break-all; }
.meta { color:#71717a; font-size:12px; margin-top:4px; }
.badge { display:inline-block; border:1px solid #27272a; border-radius:4px; padding:1px 6px; margin-right:6px; color:#a1a1aa; font-size:11px; text-transform:uppercase; letter-spacing:0.04em; }
.muted { color:#71717a; font-size:12px; }
</style>
</head>
<body><main>${body}</main></body>
</html>`

const tracesHtmlResponse = (
  title: string,
  body: string,
  status = 200,
): HttpResponse =>
  new Response(tracesPageShell(title, body), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
    },
    status,
  })

const traceListItemHtml = (
  record: TraceRecord,
  tokenQuery: string,
): string => {
  const label = record.sessionId ?? record.trajectoryId
  const href = `/trace/${encodeURIComponent(record.traceUuid)}${tokenQuery}`
  return `<li><a href="${escapeHtml(href)}"><span class="title">${escapeHtml(
    label,
  )}</span><span class="meta"><span class="badge">${escapeHtml(
    record.visibility,
  )}</span>${record.stepCount} step${
    record.stepCount === 1 ? '' : 's'
  } &middot; ${escapeHtml(record.createdAt)}</span></a></li>`
}

const routeTracesPage = <Bindings, Session extends TraceBrowserSession>(
  dependencies: TraceStoreRouteDependencies<Bindings, Session>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const url = new URL(request.url)

    // Owner scope: browser session if signed in, else the read-scope token.
    const session = yield* Effect.tryPromise({
      catch: () => undefined,
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    }).pipe(Effect.catch(() => Effect.sync(() => undefined)))

    const tokenOwner =
      session === undefined
        ? yield* resolveReadScopeOwner(dependencies, request, url, env)
        : undefined
    const ownerUserId =
      session !== undefined ? session.user.userId : tokenOwner

    if (ownerUserId === undefined) {
      return tracesHtmlResponse(
        'Traces',
        `<h1>Your traces</h1><p>Open this page from the Khala app's "Open traces in web" menu, or <a href="/login">sign in</a> to view your traces on the web.</p>`,
        200,
      )
    }

    // Preserve the token (if any) for the per-trace deep-links so owner_only
    // traces also open without a web login. A signed-in session needs no token.
    const rawToken =
      session === undefined ? readScopeTokenFromRequest(request, url) : undefined
    const tokenQuery =
      rawToken === undefined
        ? ''
        : `?token=${encodeURIComponent(rawToken)}`

    // The default real-user corpus view (excludes internal dogfood), mirroring
    // the JSON owner list default.
    const defaultCorpusKinds = TRACE_DEMAND_KINDS.filter(
      kind => !TRACE_INTERNAL_DEMAND_KINDS.includes(kind),
    )
    const traces = yield* Effect.tryPromise({
      catch: () => [] as ReadonlyArray<TraceRecord>,
      try: () =>
        dependencies
          .makeStore(env)
          .listTracesForOwnerByDemand(
            ownerUserId,
            OWNER_LIST_LIMIT,
            defaultCorpusKinds,
          ),
    }).pipe(Effect.catch(() => Effect.succeed([] as ReadonlyArray<TraceRecord>)))

    const list =
      traces.length === 0
        ? '<p class="muted">No traces yet. Run a Khala chat and your session traces will appear here.</p>'
        : `<ul>${traces
            .map(record => traceListItemHtml(record, tokenQuery))
            .join('')}</ul>`

    return tracesHtmlResponse(
      'Your traces',
      `<h1>Your traces</h1><p>Read-only view of your own agent traces.</p>${list}`,
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

    // GET /traces — owner-scoped HTML list page (mobile "Open traces in web",
    // #6347). Served HERE (before the SPA app-shell fallback) so a token-bearing
    // owner sees their own traces with no web login.
    if (url.pathname === '/traces') {
      if (request.method === 'GET') {
        return routeTracesPage(dependencies, request, env, ctx)
      }
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    if (url.pathname === '/api/traces') {
      if (request.method === 'POST') {
        return routeIngest(dependencies, request, env, ctx).pipe(
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

    // POST /api/traces/upload — explicit user-upload alias (#6221). Same ingest
    // path (agent bearer OR user web session); friendlier for human uploads.
    if (url.pathname === '/api/traces/upload') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }
      return routeIngest(dependencies, request, env, ctx).pipe(
        Effect.map(withAgentRateLimitHeaders),
        Effect.catch(error =>
          Effect.succeed(withAgentRateLimitHeaders(routeErrorResponse(error))),
        ),
      )
    }

    // Media blob: POST upload + GET serve. The `r2Key` may contain slashes
    // (e.g. `shots/00-login.png`), so it captures the rest of the path. Matched
    // BEFORE the single-segment read route below.
    const blobMatch = /^\/api\/traces\/([^/]+)\/blob\/(.+)$/.exec(url.pathname)
    if (blobMatch !== null) {
      const traceUuid = decodeURIComponent(blobMatch[1] ?? '')
      const r2Key = (blobMatch[2] ?? '')
        .split('/')
        .map(decodeURIComponent)
        .join('/')
      if (request.method === 'GET') {
        return routeBlobServe(
          dependencies,
          request,
          env,
          ctx,
          traceUuid,
          r2Key,
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
      }
      if (request.method === 'POST') {
        return routeBlobUpload(
          dependencies,
          request,
          env,
          ctx,
          traceUuid,
          r2Key,
        ).pipe(
          Effect.map(withAgentRateLimitHeaders),
          Effect.catch(error =>
            Effect.succeed(
              withAgentRateLimitHeaders(routeErrorResponse(error)),
            ),
          ),
        )
      }
      return Effect.succeed(methodNotAllowed(['GET', 'POST']))
    }

    const readMatch = /^\/api\/traces\/([^/]+)$/.exec(url.pathname)
    if (readMatch !== null) {
      const traceUuid = decodeURIComponent(readMatch[1] ?? '')
      if (request.method === 'GET') {
        return routeRead(
          dependencies,
          request,
          env,
          ctx,
          traceUuid,
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
      }
      if (request.method === 'PATCH') {
        return routeUpdateVisibility(
          dependencies,
          request,
          env,
          ctx,
          traceUuid,
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
      }
      return Effect.succeed(methodNotAllowed(['GET', 'PATCH']))
    }

    return undefined
  },
})
