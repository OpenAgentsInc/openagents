// COORDINATOR WIRING:
// Add to the main fetch router (mirroring makeAutopilotDecisionRoutes /
// makeOmniRoutes / makeOmniBundleRoutes wiring). Do NOT let this lane edit
// index.ts or worker-routes.ts.
//
//   import { makeOmniHandoffRoutes } from '../omni-handoff-routes'
//
//   const omniHandoffRoutes = makeOmniHandoffRoutes<Env>({
//     db: env => env.DB,
//     requireOperator: (request, env) => requireAdminApiToken(request, env),
//   })
//
//   Then, inside the main fetch router (alongside the existing
//   routeAutopilotDecisionRequest / routeOmniRequest / routeOmniBundleRequest
//   checks), the coordinator chains into routeOmniRequest:
//
//     const omniHandoffResponse = omniHandoffRoutes.routeOmniHandoffRequest(
//       request,
//       env,
//       ctx,
//     )
//     if (omniHandoffResponse !== undefined) {
//       return await runEffectProgram(omniHandoffResponse)
//     }

import { readRequestJsonEffect } from '@openagentsinc/effect-boundary'
import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { OmniAcceptedOutcomeWorkKind } from './omni-accepted-outcome-contracts'
import {
  type OmniHandoffInput,
  OmniHandoffValidationError,
  customerOmniHandoffProjection,
  runOmniWorkroomHandoff,
} from './omni-handoff'

type HttpResponse = globalThis.Response

type OmniHandoffRouteEnv = Readonly<Record<string, unknown>>

export type OmniHandoffRoutesDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => D1Database
  requireOperator: (request: Request, env: Bindings) => Promise<boolean>
  nowIso?: () => string
}>

const handoffPattern = /^\/api\/omni\/workrooms\/([^/]+)\/handoff$/

// REQUEST SCHEMA

const OmniEvidenceEntryRequest = S.Struct({
  caveatRef: S.NullOr(S.String),
  entryKind: S.Literals([
    'exa_source_card',
    'research_brief',
    'source_commit',
    'generated_source',
    'build_log',
    'screenshot',
    'deployment_url',
    'diff',
    'test_report',
    'email_receipt',
    'receipt',
    'redaction_report',
  ]),
  publicSafe: S.Boolean,
  redactionState: S.Literals([
    'not_needed',
    'redacted',
    'private_only',
    'blocked',
  ]),
  ref: S.String,
  required: S.Boolean,
  sourceAuthority: S.Literals([
    'agent_generated',
    'customer_supplied',
    'operator_reviewed',
    'public_web',
    'github',
    'system_receipt',
  ]),
  summaryRef: S.String,
  visibility: S.Literals(['private', 'team', 'customer', 'public']),
})

const OmniHandoffRequest = S.Struct({
  evidence: S.Struct({
    entries: S.Array(OmniEvidenceEntryRequest),
    idempotencyKey: S.String,
    legalSensitive: S.optionalKey(S.Boolean),
    metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
    sourceAuthorityCaveatRef: S.optionalKey(S.String),
    summaryRef: S.String,
  }),
  proof: S.Struct({
    acceptanceStateRef: S.String,
    economicsCaveatRef: S.String,
    extraReceiptRefs: S.optionalKey(S.Array(S.String)),
    legalCaveatRef: S.optionalKey(S.String),
    privacyCaveatRef: S.String,
    reviewStateRef: S.String,
  }),
  proofIdempotencyKey: S.String,
  workroomState: S.Literals([
    'accepted',
    'provisionally_accepted',
    'completed',
  ]),
  workKind: OmniAcceptedOutcomeWorkKind,
})
type OmniHandoffRequest = typeof OmniHandoffRequest.Type

// ERRORS

class OmniHandoffRequestError extends S.TaggedErrorClass<OmniHandoffRequestError>()(
  'OmniHandoffRequestError',
  { reason: S.String, status: S.Number },
) {}

const requestError = (status: number, reason: string): OmniHandoffRequestError =>
  new OmniHandoffRequestError({ reason, status })

const requestErrorResponse = (error: OmniHandoffRequestError): HttpResponse =>
  noStoreJsonResponse(
    { error: 'omni_handoff_request_error', reason: error.reason },
    { status: error.status },
  )

// Maps the typed orchestration failures onto HTTP responses. Workroom-not-found
// surfaces as 404; the rest are validation (400) or storage (500) failures.
const handoffErrorResponse = (error: {
  _tag: string
  reason?: string
  workroomId?: string
}): HttpResponse => {
  switch (error._tag) {
    case 'OmniEvidenceBundleWorkroomNotFound':
    case 'OmniPublicProofBundleWorkroomNotFound':
      return noStoreJsonResponse(
        {
          error: 'omni_handoff_workroom_not_found',
          reason: `Workroom ${error.workroomId ?? ''} was not found.`,
        },
        { status: 404 },
      )
    case 'OmniHandoffValidationError':
    case 'OmniEvidenceBundleValidationError':
    case 'OmniPublicProofBundleValidationError':
      return noStoreJsonResponse(
        {
          error: 'omni_handoff_validation_error',
          reason: error.reason ?? 'Handoff input was invalid.',
        },
        { status: 400 },
      )
    default:
      return noStoreJsonResponse(
        {
          error: 'omni_handoff_storage_error',
          reason: error.reason ?? 'Handoff orchestration failed.',
        },
        { status: 500 },
      )
  }
}

// `assertWorkroomReadyForHandoff` and the bundle services' `assertValidInput`
// run synchronously inside their `Effect.gen` bodies, so a redaction violation
// surfaces as a defect rather than a typed failure. Convert the known
// validation defects into a 400 so redaction enforcement is reported as a
// request error, not a 500 crash.
const handoffDefectResponse = (defect: unknown): HttpResponse => {
  if (
    defect instanceof OmniHandoffValidationError ||
    (defect !== null &&
      typeof defect === 'object' &&
      '_tag' in defect &&
      (defect._tag === 'OmniEvidenceBundleValidationError' ||
        defect._tag === 'OmniPublicProofBundleValidationError'))
  ) {
    const reason =
      defect !== null && typeof defect === 'object' && 'reason' in defect
        ? String((defect as { reason: unknown }).reason)
        : 'Handoff input was invalid.'

    return noStoreJsonResponse(
      { error: 'omni_handoff_validation_error', reason },
      { status: 400 },
    )
  }

  return noStoreJsonResponse(
    {
      error: 'omni_handoff_storage_error',
      reason: defect instanceof Error ? defect.message : String(defect),
    },
    { status: 500 },
  )
}

const decodeBody = (
  request: Request,
): Effect.Effect<OmniHandoffRequest, OmniHandoffRequestError> =>
  readRequestJsonEffect(
    OmniHandoffRequest,
    request,
    'omni_handoff.body',
  ).pipe(
    Effect.mapError(error =>
      requestError(
        400,
        error.reasonRef === 'boundary.json.malformed'
          ? 'Malformed JSON request body.'
          : 'Omni handoff request did not match the expected schema.',
      ),
    ),
  )

const requireOperatorAuth = <Bindings extends OmniHandoffRouteEnv>(
  dependencies: OmniHandoffRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, OmniHandoffRequestError> =>
  Effect.flatMap(
    Effect.promise(() => dependencies.requireOperator(request, env)),
    isOperator =>
      isOperator
        ? Effect.void
        : Effect.fail(requestError(401, 'Operator authorization is required.')),
  )

const handoffInput = (
  body: OmniHandoffRequest,
  workroomId: string,
): OmniHandoffInput => ({
  evidence: {
    entries: body.evidence.entries,
    idempotencyKey: body.evidence.idempotencyKey,
    legalSensitive: body.evidence.legalSensitive,
    metadata: body.evidence.metadata,
    sourceAuthorityCaveatRef: body.evidence.sourceAuthorityCaveatRef,
    summaryRef: body.evidence.summaryRef,
  },
  proof: {
    acceptanceStateRef: body.proof.acceptanceStateRef,
    economicsCaveatRef: body.proof.economicsCaveatRef,
    extraReceiptRefs: body.proof.extraReceiptRefs,
    legalCaveatRef: body.proof.legalCaveatRef,
    privacyCaveatRef: body.proof.privacyCaveatRef,
    reviewStateRef: body.proof.reviewStateRef,
  },
  proofIdempotencyKey: body.proofIdempotencyKey,
  workroom: {
    state: body.workroomState,
    workKind: body.workKind,
    workroomId,
  },
})

const performHandoff = <Bindings extends OmniHandoffRouteEnv>(
  dependencies: OmniHandoffRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  workroomId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    yield* requireOperatorAuth(dependencies, request, env)
    const body = yield* decodeBody(request)
    const result = yield* runOmniWorkroomHandoff(
      dependencies.db(env),
      handoffInput(body, workroomId),
    )

    return noStoreJsonResponse(
      {
        handoff: customerOmniHandoffProjection(result),
        proofBundleId: result.proofBundle.id,
      },
      { status: 201 },
    )
  }).pipe(
    Effect.catchTag('OmniHandoffRequestError', error =>
      Effect.succeed(requestErrorResponse(error)),
    ),
    Effect.catch(error => Effect.succeed(handoffErrorResponse(error))),
    Effect.catchDefect(defect => Effect.succeed(handoffDefectResponse(defect))),
  )

export const makeOmniHandoffRoutes = <Bindings extends OmniHandoffRouteEnv>(
  dependencies: OmniHandoffRoutesDependencies<Bindings>,
) => ({
  routeOmniHandoffRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const match = handoffPattern.exec(url.pathname)

    if (match?.[1] === undefined) {
      return undefined
    }

    const workroomId = decodeURIComponent(match[1])

    return M.value(request.method).pipe(
      M.when('POST', () =>
        performHandoff(dependencies, request, env, workroomId),
      ),
      M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
    )
  },
})
