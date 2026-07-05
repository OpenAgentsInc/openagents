// COORDINATOR WIRING:
//
// 1. Import the factory near the other route imports in workers/api/src/index.ts:
//
//      import { makeOmniWorkroomLifecycleRoutes } from './omni-workroom-lifecycle-routes'
//
// 2. Construct the routes alongside the other `make*Routes` calls in index.ts
//    (e.g. next to `const autopilotDecisionRoutes = makeAutopilotDecisionRoutes...`):
//
//      const omniWorkroomLifecycleRoutes =
//        makeOmniWorkroomLifecycleRoutes<WorkerBindings>({
//          makeDb: env => openAgentsDatabase(env),
//          requireBrowserSession,
//          requireAdminApiToken: (request, env) =>
//            requireAdminApiToken(request, env),
//        })
//
// 3. Chain its dispatcher into the request router in index.ts. It returns
//    `undefined` when the path does not match, so chain with `??` ahead of the
//    catch-all, mirroring `routeAutopilotWorkRequest`:
//
//      routeOmniWorkroomLifecycleRequest: (request, env, ctx) =>
//        omniWorkroomLifecycleRoutes.routeOmniWorkroomLifecycleRequest(
//          request,
//          env,
//          ctx,
//        ),
//
//    and add the matching dispatch entry where the router consumes it, e.g.:
//
//      ?? omniWorkroomLifecycleRoutes.routeOmniWorkroomLifecycleRequest(
//           request,
//           env,
//           ctx,
//         )
//
// Do NOT edit workers/api/src/http/router.ts or workers/api/src/index.ts as part
// of this module's change set. These lines are the registration contract only.

import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown, readJsonObject } from './json-boundary'
import {
  type OmniWorkroomLifecycleDecisionRecord,
  type OmniWorkroomLifecycleError,
  type OmniWorkroomLifecycleRuntime,
  customerOmniWorkroomLifecycleProjection,
  operatorOmniWorkroomLifecycleProjection,
  publicOmniWorkroomLifecycleProjection,
  recordOmniWorkroomLifecycleDecision,
  systemOmniWorkroomLifecycleRuntime,
} from './omni-workroom-lifecycle'
import { OmniAcceptedOutcomeWorkKind } from './omni-accepted-outcome-contracts'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

type HttpResponse = globalThis.Response

type OmniWorkroomLifecycleRouteEnv = Readonly<Record<string, unknown>>

export type OmniWorkroomLifecycleAudience = 'public' | 'customer' | 'operator'

export type OmniWorkroomLifecycleRoutesDependencies<Bindings> = Readonly<{
  makeDb: (env: Bindings) => D1Database
  // KS-8.17 (#8361): optional read-back mirror factory for the
  // omni_workroom_lifecycle_decisions row this route writes. Coordinator
  // wiring should pass
  // `mirror: env => makeSupervisionLongtailMirrorForEnv(env, { db: dependencies.makeDb(env) })`
  // alongside `makeDb`; undefined stays a safe no-op.
  mirror?: (env: Bindings) => SupervisionLongtailMirror | undefined
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: Readonly<{ userId: string }> }> | undefined>
  requireAdminApiToken?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean> | boolean
  runtime?: OmniWorkroomLifecycleRuntime
}>

const OMNI_WORKROOM_LIFECYCLE_HISTORY_LIMIT = 200

const OmniWorkroomLifecycleDecisionRequest = S.Struct({
  actorKind: S.Literals(['customer', 'operator', 'system']),
  artifactRef: S.optionalKey(S.String),
  customerSafeExplanationRef: S.String,
  decisionKind: S.Literals([
    'accept',
    'reject',
    'provisionally_accept',
    'reopen',
    'request_revision',
    'mark_unavailable',
  ]),
  followupRequestRef: S.optionalKey(S.String),
  id: S.optionalKey(S.String),
  idempotencyKey: S.optionalKey(S.String),
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  receiptRef: S.String,
  siteRevisionFeedbackRef: S.optionalKey(S.String),
  workKind: OmniAcceptedOutcomeWorkKind,
})
type OmniWorkroomLifecycleDecisionRequest =
  typeof OmniWorkroomLifecycleDecisionRequest.Type

class OmniWorkroomLifecycleRouteError extends S.TaggedErrorClass<OmniWorkroomLifecycleRouteError>()(
  'OmniWorkroomLifecycleRouteError',
  {
    kind: S.Literals(['validation_error', 'storage_error']),
    reason: S.String,
  },
) {}

const workroomIdFromDecisionPath = (
  pathname: string,
): string | undefined => {
  const match = /^\/api\/omni\/workrooms\/([^/]+)\/lifecycle-decisions$/.exec(
    pathname,
  )

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const audienceFromRequest = (request: Request): OmniWorkroomLifecycleAudience =>
  M.value(new URL(request.url).searchParams.get('audience')).pipe(
    M.when('operator', () => 'operator' as const),
    M.when('customer', () => 'customer' as const),
    M.orElse(() => 'public' as const),
  )

const projectForAudience = (
  audience: OmniWorkroomLifecycleAudience,
  decision: OmniWorkroomLifecycleDecisionRecord,
) =>
  M.value(audience).pipe(
    M.when('operator', () =>
      operatorOmniWorkroomLifecycleProjection(decision),
    ),
    M.when('customer', () =>
      customerOmniWorkroomLifecycleProjection(decision),
    ),
    M.orElse(() => publicOmniWorkroomLifecycleProjection(decision)),
  )

const idempotencyKeyFromRequest = (
  request: Request,
  body: OmniWorkroomLifecycleDecisionRequest,
): string | undefined => {
  const header = request.headers.get('Idempotency-Key')?.trim()

  if (header !== undefined && header !== '') {
    return header
  }

  return body.idempotencyKey
}

const lifecycleRouteErrorResponse = (
  error: OmniWorkroomLifecycleRouteError,
): HttpResponse =>
  noStoreJsonResponse(
    { error: `omni_workroom_lifecycle_${error.kind}`, reason: error.reason },
    { status: error.kind === 'storage_error' ? 500 : 400 },
  )

const serviceErrorResponse = (
  error: OmniWorkroomLifecycleError,
): HttpResponse =>
  M.value(error).pipe(
    M.tag('OmniWorkroomLifecycleWorkroomNotFound', notFound =>
      noStoreJsonResponse(
        {
          error: 'omni_workroom_lifecycle_workroom_not_found',
          reason: `Workroom ${notFound.workroomId} was not found.`,
        },
        { status: 404 },
      ),
    ),
    M.tag('OmniWorkroomLifecycleValidationError', validation =>
      noStoreJsonResponse(
        {
          error: 'omni_workroom_lifecycle_validation_error',
          reason: validation.reason,
        },
        { status: 400 },
      ),
    ),
    M.tag('OmniWorkroomLifecycleStorageError', storage =>
      noStoreJsonResponse(
        {
          error: 'omni_workroom_lifecycle_storage_error',
          reason: storage.reason,
        },
        { status: 500 },
      ),
    ),
    M.exhaustive,
  )

const decodeDecisionRequest = (
  request: Request,
): Effect.Effect<
  OmniWorkroomLifecycleDecisionRequest,
  OmniWorkroomLifecycleRouteError
> =>
  Effect.tryPromise({
    catch: error =>
      new OmniWorkroomLifecycleRouteError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      S.decodeUnknownSync(OmniWorkroomLifecycleDecisionRequest)(
        await readJsonObject(request),
      ),
  })

type LifecycleHistoryRow = Readonly<{
  actor_kind: OmniWorkroomLifecycleDecisionRecord['actorKind']
  archived_at: string | null
  artifact_ref: string | null
  created_at: string
  customer_safe_explanation_ref: string
  decision_kind: OmniWorkroomLifecycleDecisionRecord['decisionKind']
  followup_request_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  no_settlement_implication: number
  receipt_ref: string
  resulting_state: OmniWorkroomLifecycleDecisionRecord['resultingState']
  site_revision_feedback_ref: string | null
  work_kind: OmniWorkroomLifecycleDecisionRecord['workKind']
  workroom_id: string
}>

const decisionFromHistoryRow = (
  row: LifecycleHistoryRow,
): OmniWorkroomLifecycleDecisionRecord => ({
  actorKind: row.actor_kind,
  archivedAt: row.archived_at,
  artifactRef: row.artifact_ref,
  createdAt: row.created_at,
  customerSafeExplanationRef: row.customer_safe_explanation_ref,
  decisionKind: row.decision_kind,
  followupRequestRef: row.followup_request_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: ((): Readonly<Record<string, unknown>> => {
    try {
      const parsed: unknown = parseJsonUnknown(row.metadata_json)

      return parsed !== null && typeof parsed === 'object'
        ? (parsed as Readonly<Record<string, unknown>>)
        : {}
    } catch {
      return {}
    }
  })(),
  noSettlementImplication: row.no_settlement_implication === 1,
  receiptRef: row.receipt_ref,
  resultingState: row.resulting_state,
  siteRevisionFeedbackRef: row.site_revision_feedback_ref,
  workKind: row.work_kind,
  workroomId: row.workroom_id,
})

const readDecisionHistory = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<
  ReadonlyArray<OmniWorkroomLifecycleDecisionRecord>,
  OmniWorkroomLifecycleRouteError
> =>
  Effect.tryPromise({
    catch: error =>
      new OmniWorkroomLifecycleRouteError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: () =>
      db
        .prepare(
          `SELECT *
             FROM omni_workroom_lifecycle_decisions
            WHERE workroom_id = ?
              AND archived_at IS NULL
            ORDER BY created_at ASC, id ASC
            LIMIT ?`,
        )
        .bind(workroomId, OMNI_WORKROOM_LIFECYCLE_HISTORY_LIMIT)
        .all<LifecycleHistoryRow>(),
  }).pipe(
    Effect.map(result => (result.results ?? []).map(decisionFromHistoryRow)),
  )

const postLifecycleDecision = <
  Bindings extends OmniWorkroomLifecycleRouteEnv,
>(
  dependencies: OmniWorkroomLifecycleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  workroomId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const body = yield* decodeDecisionRequest(request)
    const idempotencyKey = idempotencyKeyFromRequest(request, body)

    if (idempotencyKey === undefined || idempotencyKey.trim() === '') {
      return yield* new OmniWorkroomLifecycleRouteError({
        kind: 'validation_error',
        reason:
          'Idempotency-Key header or idempotencyKey body field is required.',
      })
    }

    const db = dependencies.makeDb(env)
    const decision = yield* recordOmniWorkroomLifecycleDecision(
      db,
      {
        actorKind: body.actorKind,
        artifactRef: body.artifactRef,
        customerSafeExplanationRef: body.customerSafeExplanationRef,
        decisionKind: body.decisionKind,
        followupRequestRef: body.followupRequestRef,
        id: body.id,
        idempotencyKey,
        metadata: body.metadata,
        receiptRef: body.receiptRef,
        siteRevisionFeedbackRef: body.siteRevisionFeedbackRef,
        workKind: body.workKind,
        workroomId,
      },
      dependencies.runtime ?? systemOmniWorkroomLifecycleRuntime,
      dependencies.mirror?.(env),
    )

    return noStoreJsonResponse({
      decision: operatorOmniWorkroomLifecycleProjection(decision),
      directEffectPermitted: false,
    })
  }).pipe(
    Effect.catchTag('OmniWorkroomLifecycleRouteError', error =>
      Effect.succeed(lifecycleRouteErrorResponse(error)),
    ),
    Effect.catchTags({
      OmniWorkroomLifecycleStorageError: error =>
        Effect.succeed(serviceErrorResponse(error)),
      OmniWorkroomLifecycleValidationError: error =>
        Effect.succeed(serviceErrorResponse(error)),
      OmniWorkroomLifecycleWorkroomNotFound: error =>
        Effect.succeed(serviceErrorResponse(error)),
    }),
  )

const getLifecycleHistory = <
  Bindings extends OmniWorkroomLifecycleRouteEnv,
>(
  dependencies: OmniWorkroomLifecycleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  workroomId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const audience = audienceFromRequest(request)
    const db = dependencies.makeDb(env)
    const decisions = yield* readDecisionHistory(db, workroomId)

    return noStoreJsonResponse({
      audience,
      decisions: decisions.map(decision =>
        projectForAudience(audience, decision),
      ),
      directEffectPermitted: false,
      workroomId,
    })
  }).pipe(
    Effect.catchTag('OmniWorkroomLifecycleRouteError', error =>
      Effect.succeed(lifecycleRouteErrorResponse(error)),
    ),
  )

export const makeOmniWorkroomLifecycleRoutes = <
  Bindings extends OmniWorkroomLifecycleRouteEnv,
>(
  dependencies: OmniWorkroomLifecycleRoutesDependencies<Bindings>,
) => ({
  routeOmniWorkroomLifecycleRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const workroomId = workroomIdFromDecisionPath(url.pathname)

    if (workroomId === undefined) {
      return undefined
    }

    return M.value(request.method).pipe(
      M.when('POST', () =>
        postLifecycleDecision(dependencies, request, env, workroomId),
      ),
      M.when('GET', () =>
        getLifecycleHistory(dependencies, request, env, workroomId),
      ),
      M.orElse(() => Effect.succeed(methodNotAllowed(['GET', 'POST']))),
    )
  },
})
