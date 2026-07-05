// COORDINATOR WIRING: register these omni workroom HTTP routes during reconciliation.
// 1. import: import { makeOmniWorkroomRoutes } from './omni-workroom-routes'
// 2. construct (near other route modules):
//      const omniWorkroomRoutes = makeOmniWorkroomRoutes<WorkerBindings>({ db: env => openAgentsDatabase(env), requireBrowserSession })
// 3. dispatch (inside makeWorkerRouteRequest, before autopilotWorkRoutes in the catch-all `??` chain):
//      routeOmniWorkroomRequest: (request, env, ctx) => omniWorkroomRoutes.routeOmniWorkroomRequest(request, env, ctx),
//    then add `?? routeOmniWorkroomRequest(request, env, ctx)` to the shared fallthrough.

import { Effect, Match as M, Schema as S } from 'effect'

import {
  OmniAcceptedOutcomeWorkKind,
} from './omni-accepted-outcome-contracts'
import {
  type OmniDataClassification,
  OmniDataClassification as OmniDataClassificationSchema,
  type OmniTrustTier,
  OmniTrustTier as OmniTrustTierSchema,
} from './omni-data-classification'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { parseJsonUnknown, readJsonObject } from './json-boundary'
import {
  type OmniWorkroomError,
  type OmniWorkroomRecord,
  type OmniWorkroomStatus,
  type OmniWorkroomVisibility,
  OmniWorkroomStatus as OmniWorkroomStatusSchema,
  OmniWorkroomValidationError,
  OmniWorkroomVisibility as OmniWorkroomVisibilitySchema,
  promoteOmniWorkroom,
} from './omni-workrooms'
import {
  type OmniWorkroomProjectionSurface,
  OmniWorkroomProjectionSurface as OmniWorkroomProjectionSurfaceSchema,
  buildOmniWorkroomSurfaceProjection,
} from './omni-workroom-surface-projections'
import { buildOmniWorkroomSourceAuthorityDeliveryPlan } from './omni-workroom-business-object-delivery'
import type { OmniProjectionAudience } from './omni-data-classification'
import { currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

type HttpResponse = globalThis.Response

type OmniWorkroomRouteEnv = Readonly<Record<string, unknown>>

export type OmniWorkroomRoutesDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => D1Database
  // KS-8.17 (#8361): optional read-back mirror factory for the
  // omni_workrooms row this route's create path writes. Coordinator wiring
  // (see the header comment) should pass
  // `mirror: env => makeSupervisionLongtailMirrorForEnv(env, { db: dependencies.db(env) })`
  // alongside `db` when it registers these routes; undefined stays a safe
  // no-op (no Postgres binding / dual-write off).
  mirror?: (env: Bindings) => SupervisionLongtailMirror | undefined
  nowIso?: () => string
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: Readonly<{ userId: string }> }> | undefined>
}>

// REQUEST

const OmniWorkroomCreateRequest = S.Struct({
  acceptedOutcomeContractId: S.optionalKey(S.String),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  assignmentId: S.optionalKey(S.String),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  classificationCaveatRef: S.optionalKey(S.String),
  customerIntentRef: S.String,
  dataClassification: S.optionalKey(OmniDataClassificationSchema),
  emailRefs: S.optionalKey(S.Array(S.String)),
  id: S.optionalKey(S.String),
  idempotencyKey: S.String,
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  publicReceiptRef: S.optionalKey(S.String),
  receiptRefs: S.optionalKey(S.Array(S.String)),
  siteId: S.optionalKey(S.String),
  softwareOrderId: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
  status: S.optionalKey(OmniWorkroomStatusSchema),
  taskPacketRef: S.optionalKey(S.String),
  trustTier: S.optionalKey(OmniTrustTierSchema),
  visibility: S.optionalKey(OmniWorkroomVisibilitySchema),
  workKind: OmniAcceptedOutcomeWorkKind,
})
type OmniWorkroomCreateRequest = typeof OmniWorkroomCreateRequest.Type

class OmniWorkroomRequestError extends S.TaggedErrorClass<OmniWorkroomRequestError>()(
  'OmniWorkroomRequestError',
  { reason: S.String },
) {}

const routeNowIso = <Bindings>(
  dependencies: OmniWorkroomRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const decodeCreateRequest = (
  request: Request,
): Effect.Effect<OmniWorkroomCreateRequest, OmniWorkroomRequestError> =>
  Effect.tryPromise({
    catch: error =>
      new OmniWorkroomRequestError({
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      S.decodeUnknownSync(OmniWorkroomCreateRequest)(
        await readJsonObject(request),
      ),
  })

const surfaceFromUrl = (
  url: URL,
): Effect.Effect<OmniWorkroomProjectionSurface, OmniWorkroomRequestError> => {
  const raw = url.searchParams.get('surface') ?? 'public'

  return Effect.try({
    catch: () =>
      new OmniWorkroomRequestError({
        reason: `surface must be one of public, customer, team, agent, operator (got ${raw}).`,
      }),
    try: () => S.decodeUnknownSync(OmniWorkroomProjectionSurfaceSchema)(raw),
  })
}

const workroomErrorResponse = (error: OmniWorkroomError): HttpResponse =>
  M.value(error).pipe(
    M.tag('OmniWorkroomValidationError', err =>
      noStoreJsonResponse(
        { error: 'omni_workroom_validation_error', reason: err.reason },
        { status: 400 },
      ),
    ),
    M.tag('OmniWorkroomOrderNotFound', err =>
      noStoreJsonResponse(
        {
          error: 'omni_workroom_order_not_found',
          reason: `Software order ${err.softwareOrderId} was not found.`,
        },
        { status: 404 },
      ),
    ),
    M.tag('OmniWorkroomSiteNotFound', err =>
      noStoreJsonResponse(
        {
          error: 'omni_workroom_site_not_found',
          reason: `Site ${err.siteId} was not found.`,
        },
        { status: 404 },
      ),
    ),
    M.tag('OmniWorkroomAssignmentNotFound', err =>
      noStoreJsonResponse(
        {
          error: 'omni_workroom_assignment_not_found',
          reason: `Assignment ${err.assignmentId} was not found.`,
        },
        { status: 404 },
      ),
    ),
    M.tag('OmniWorkroomAcceptedOutcomeContractNotFound', err =>
      noStoreJsonResponse(
        {
          error: 'omni_workroom_accepted_outcome_contract_not_found',
          reason: `Accepted outcome contract ${err.acceptedOutcomeContractId} was not found.`,
        },
        { status: 404 },
      ),
    ),
    M.tag('OmniWorkroomStorageError', () =>
      noStoreJsonResponse(
        { error: 'omni_workroom_storage_error' },
        { status: 500 },
      ),
    ),
    M.exhaustive,
  )

const createInputFromRequest = (
  body: OmniWorkroomCreateRequest,
): Parameters<typeof promoteOmniWorkroom>[1] => ({
  acceptedOutcomeContractId: body.acceptedOutcomeContractId,
  artifactRefs: body.artifactRefs,
  assignmentId: body.assignmentId,
  blockerRefs: body.blockerRefs,
  classificationCaveatRef: body.classificationCaveatRef,
  customerIntentRef: body.customerIntentRef,
  dataClassification: body.dataClassification,
  emailRefs: body.emailRefs,
  id: body.id,
  idempotencyKey: body.idempotencyKey,
  metadata: body.metadata,
  publicReceiptRef: body.publicReceiptRef,
  receiptRefs: body.receiptRefs,
  siteId: body.siteId,
  softwareOrderId: body.softwareOrderId,
  sourceRefs: body.sourceRefs,
  status: body.status,
  taskPacketRef: body.taskPacketRef,
  trustTier: body.trustTier,
  visibility: body.visibility,
  workKind: body.workKind,
})

const requireOperatorSession = <Bindings extends OmniWorkroomRouteEnv>(
  dependencies: OmniWorkroomRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<Readonly<{ userId: string }>, OmniWorkroomRequestError> =>
  dependencies.requireBrowserSession === undefined
    ? Effect.fail(
        new OmniWorkroomRequestError({
          reason: 'No browser session resolver is configured.',
        }),
      )
    : Effect.flatMap(
        Effect.promise(() =>
          dependencies.requireBrowserSession!(request, env, ctx),
        ),
        session =>
          session === undefined
            ? Effect.fail(
                new OmniWorkroomRequestError({
                  reason: 'An authenticated operator session is required.',
                }),
              )
            : Effect.succeed(session.user),
      )

const createWorkroom = <Bindings extends OmniWorkroomRouteEnv>(
  dependencies: OmniWorkroomRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    yield* requireOperatorSession(dependencies, request, env, ctx)

    const nowIso = routeNowIso(dependencies)
    const body = yield* decodeCreateRequest(request)
    const db = dependencies.db(env)
    const existing = yield* readWorkroomByIdempotencyKey(db, body.idempotencyKey)
    const record: OmniWorkroomRecord = yield* promoteOmniWorkroom(
      db,
      createInputFromRequest(body),
      undefined,
      dependencies.mirror?.(env),
    )
    const idempotent = existing !== null

    return noStoreJsonResponse(
      {
        generatedAt: nowIso,
        workroom: buildOmniWorkroomSurfaceProjection({
          surface: 'operator',
          workroom: record,
        }),
      },
      { status: idempotent ? 200 : 201 },
    )
  }).pipe(
    Effect.catchTag('OmniWorkroomRequestError', error =>
      Effect.succeed(
        error.reason.includes('session') ||
          error.reason.includes('operator')
          ? unauthorized()
          : noStoreJsonResponse(
              { error: 'omni_workroom_request_error', reason: error.reason },
              { status: 400 },
            ),
      ),
    ),
    Effect.catch(error => Effect.succeed(workroomErrorResponse(error))),
    Effect.catchDefect(defect =>
      Effect.succeed(
        defect instanceof OmniWorkroomValidationError
          ? noStoreJsonResponse(
              {
                error: 'omni_workroom_validation_error',
                reason: defect.reason,
              },
              { status: 400 },
            )
          : noStoreJsonResponse(
              { error: 'omni_workroom_storage_error' },
              { status: 500 },
            ),
      ),
    ),
  )

const readWorkroom = <Bindings extends OmniWorkroomRouteEnv>(
  dependencies: OmniWorkroomRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workroomId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const url = new URL(request.url)
    const surface = yield* surfaceFromUrl(url)

    if (surface === 'operator' || surface === 'team') {
      yield* requireOperatorSession(dependencies, request, env, ctx)
    }

    const nowIso = routeNowIso(dependencies)
    const record = yield* readWorkroomById(dependencies.db(env), workroomId)

    if (record === null) {
      return noStoreJsonResponse(
        {
          error: 'omni_workroom_not_found',
          reason: `Omni workroom ${workroomId} was not found.`,
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse({
      generatedAt: nowIso,
      surface,
      workroom: buildOmniWorkroomSurfaceProjection({ surface, workroom: record }),
    })
  }).pipe(
    Effect.catchTag('OmniWorkroomRequestError', error =>
      Effect.succeed(
        error.reason.includes('session') || error.reason.includes('operator')
          ? unauthorized()
          : noStoreJsonResponse(
              { error: 'omni_workroom_request_error', reason: error.reason },
              { status: 400 },
            ),
      ),
    ),
    Effect.catchDefect(() =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'omni_workroom_projection_error' },
          { status: 500 },
        ),
      ),
    ),
  )

const audienceForSurface = (
  surface: OmniWorkroomProjectionSurface,
): OmniProjectionAudience => {
  switch (surface) {
    case 'agent':
      return 'customer'
    case 'customer':
      return 'customer'
    case 'operator':
      return 'operator'
    case 'public':
      return 'public'
    case 'team':
      return 'team'
  }
}

// Source-authority delivery projection for a live workroom.
//
// This is the seam that wires the source-authority + approval-gated write
// model (omni-source-authorized-business-objects.ts) onto the LIVE omni
// client-delivery workroom surface. It reads the workroom's metadata
// `sourceAuthority` bindings/writes/config and returns the delivery plan. When
// the owner-gated config is ready, approved source-backed writes are reported
// as applied business-object projections with closeout receipts. Operator/team
// surfaces require a session.
const readWorkroomSourceAuthority = <Bindings extends OmniWorkroomRouteEnv>(
  dependencies: OmniWorkroomRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workroomId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const url = new URL(request.url)
    const surface = yield* surfaceFromUrl(url)

    if (surface === 'operator' || surface === 'team') {
      yield* requireOperatorSession(dependencies, request, env, ctx)
    }

    const nowIso = routeNowIso(dependencies)
    const record = yield* readWorkroomById(dependencies.db(env), workroomId)

    if (record === null) {
      return noStoreJsonResponse(
        {
          error: 'omni_workroom_not_found',
          reason: `Omni workroom ${workroomId} was not found.`,
        },
        { status: 404 },
      )
    }

    const plan = buildOmniWorkroomSourceAuthorityDeliveryPlan({
      audience: audienceForSurface(surface),
      nowIso,
      workroom: record,
    })

    return noStoreJsonResponse({
      generatedAt: nowIso,
      sourceAuthorityDelivery: plan,
      surface,
      workroomId,
    })
  }).pipe(
    Effect.catchTag('OmniWorkroomRequestError', error =>
      Effect.succeed(
        error.reason.includes('session') || error.reason.includes('operator')
          ? unauthorized()
          : noStoreJsonResponse(
              { error: 'omni_workroom_request_error', reason: error.reason },
              { status: 400 },
            ),
      ),
    ),
    Effect.catchDefect(() =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'omni_workroom_source_authority_error' },
          { status: 500 },
        ),
      ),
    ),
  )

type WorkroomRow = Readonly<{
  accepted_outcome_contract_id: string | null
  archived_at: string | null
  artifact_refs_json: string
  assignment_id: string | null
  blocker_refs_json: string
  classification_caveat_ref: string
  created_at: string
  customer_intent_ref: string
  data_classification: OmniDataClassification
  email_refs_json: string
  id: string
  idempotency_key: string
  metadata_json: string
  public_receipt_ref: string
  receipt_refs_json: string
  site_id: string | null
  software_order_id: string
  source_refs_json: string
  status: OmniWorkroomStatus
  task_packet_ref: string | null
  trust_tier: OmniTrustTier
  updated_at: string
  visibility: OmniWorkroomVisibility
  work_kind: OmniAcceptedOutcomeWorkKind
}>

const parseJsonArray = (value: string): ReadonlyArray<string> => {
  try {
    const parsed = parseJsonUnknown(value)

    return Array.isArray(parsed) ? parsed.filter(item => typeof item === 'string') : []
  } catch {
    return []
  }
}

const parseJsonRecordSafe = (value: string): Readonly<Record<string, unknown>> => {
  try {
    const parsed = parseJsonUnknown(value)

    return typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

const workroomFromRow = (row: WorkroomRow): OmniWorkroomRecord => ({
  acceptedOutcomeContractId: row.accepted_outcome_contract_id,
  archivedAt: row.archived_at,
  artifactRefs: parseJsonArray(row.artifact_refs_json),
  assignmentId: row.assignment_id,
  blockerRefs: parseJsonArray(row.blocker_refs_json),
  classificationCaveatRef: row.classification_caveat_ref,
  createdAt: row.created_at,
  customerIntentRef: row.customer_intent_ref,
  dataClassification: row.data_classification,
  emailRefs: parseJsonArray(row.email_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecordSafe(row.metadata_json),
  publicReceiptRef: row.public_receipt_ref,
  receiptRefs: parseJsonArray(row.receipt_refs_json),
  siteId: row.site_id,
  softwareOrderId: row.software_order_id,
  sourceRefs: parseJsonArray(row.source_refs_json),
  status: row.status,
  taskPacketRef: row.task_packet_ref,
  trustTier: row.trust_tier,
  updatedAt: row.updated_at,
  visibility: row.visibility,
  workKind: row.work_kind,
})

const readWorkroomById = (
  db: D1Database,
  id: string,
): Effect.Effect<OmniWorkroomRecord | null, OmniWorkroomRequestError> =>
  Effect.tryPromise({
    catch: error =>
      new OmniWorkroomRequestError({
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: () =>
      db
        .prepare(
          `SELECT *
             FROM omni_workrooms
            WHERE id = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(id)
        .first<WorkroomRow>(),
  }).pipe(Effect.map(row => (row === null ? null : workroomFromRow(row))))

const readWorkroomByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<OmniWorkroomRecord | null, OmniWorkroomRequestError> =>
  Effect.tryPromise({
    catch: error =>
      new OmniWorkroomRequestError({
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: () =>
      db
        .prepare(
          `SELECT *
             FROM omni_workrooms
            WHERE idempotency_key = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKey)
        .first<WorkroomRow>(),
  }).pipe(Effect.map(row => (row === null ? null : workroomFromRow(row))))

const workroomIdFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/omni\/workrooms\/([^/]+)$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const workroomSourceAuthorityIdFromPath = (
  pathname: string,
): string | undefined => {
  const match = /^\/api\/omni\/workrooms\/([^/]+)\/source-authority$/.exec(
    pathname,
  )

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

export const makeOmniWorkroomRoutes = <Bindings extends OmniWorkroomRouteEnv>(
  dependencies: OmniWorkroomRoutesDependencies<Bindings>,
) => ({
  routeOmniWorkroomRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/omni/workrooms') {
      return M.value(request.method).pipe(
        M.when('POST', () => createWorkroom(dependencies, request, env, ctx)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const sourceAuthorityId = workroomSourceAuthorityIdFromPath(url.pathname)

    if (sourceAuthorityId !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkroomSourceAuthority(
            dependencies,
            request,
            env,
            ctx,
            sourceAuthorityId,
          ),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const workroomId = workroomIdFromPath(url.pathname)

    if (workroomId !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkroom(dependencies, request, env, ctx, workroomId),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    return undefined
  },
})
