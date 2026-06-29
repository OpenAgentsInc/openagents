// COORDINATOR WIRING:
// Add to workers/api/src/http/router.ts (mirroring makeAutopilotDecisionRoutes /
// makeOmniRoutes wiring). Do NOT let this lane edit router.ts or index.ts.
//
//   import { makeOmniBundleRoutes } from '../omni-bundle-routes'
//   import {
//     createOmniEvidenceBundle,
//     readOmniEvidenceBundleById,
//   } from '../omni-evidence-bundles'
//   import {
//     createOmniPublicProofBundle,
//     readOmniPublicProofBundleById,
//   } from '../omni-public-proof-bundles'
//
//   const omniBundleRoutes = makeOmniBundleRoutes<Env>({
//     db: env => env.DB,
//     requireOperator: (request, env) => requireAdminApiToken(request, env),
//     // createOmniEvidenceBundle / createOmniPublicProofBundle are imported
//     // EXISTING services; the read* helpers below are thin id lookups that the
//     // coordinator can either export from those service modules or pass inline.
//     readEvidenceBundle: (db, id) => readOmniEvidenceBundleById(db, id),
//     readProofBundle: (db, id) => readOmniPublicProofBundleById(db, id),
//   })
//
//   Then, inside the main fetch router (alongside the existing
//   routeAutopilotDecisionRequest / routeOmniRequest checks):
//
//     const omniBundleResponse = omniBundleRoutes.routeOmniBundleRequest(
//       request,
//       env,
//       ctx,
//     )
//     if (omniBundleResponse !== undefined) {
//       return await runEffectProgram(omniBundleResponse)
//     }
//
// NOTE: the GET read path needs an id-keyed lookup. The existing services only
// expose idempotency-key reads internally, so this module accepts injected
// `readEvidenceBundle` / `readProofBundle` reader functions. If the coordinator
// prefers, it can add `readOmniEvidenceBundleById` / `readOmniPublicProofBundleById`
// exports to the service modules and wire them as shown above. This lane does
// NOT edit those service modules.

import { Effect, Match as M, Schema as S } from 'effect'

import {
  type CreateOmniEvidenceBundleInput,
  type OmniEvidenceBundleError,
  type OmniEvidenceBundleRecord,
  OmniEvidenceBundleValidationError,
  createOmniEvidenceBundle,
  customerOmniEvidenceBundleProjection,
  operatorOmniEvidenceBundleProjection,
} from './omni-evidence-bundles'
import {
  type CreateOmniPublicProofBundleInput,
  type OmniPublicProofBundleError,
  type OmniPublicProofBundleRecord,
  OmniPublicProofBundleValidationError,
  createOmniPublicProofBundle,
  operatorOmniProofBundleProjection,
  publicOmniProofBundleProjection,
} from './omni-public-proof-bundles'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { readJsonObject } from './json-boundary'
import { OmniAcceptedOutcomeWorkKind } from './omni-accepted-outcome-contracts'

type HttpResponse = globalThis.Response

type OmniBundleRouteEnv = Readonly<Record<string, unknown>>

export type OmniEvidenceBundleReader<Db> = (
  db: Db,
  id: string,
) => Promise<OmniEvidenceBundleRecord | null>

export type OmniPublicProofBundleReader<Db> = (
  db: Db,
  id: string,
) => Promise<OmniPublicProofBundleRecord | null>

export type OmniBundleRoutesDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => D1Database
  readEvidenceBundle: OmniEvidenceBundleReader<D1Database>
  readProofBundle: OmniPublicProofBundleReader<D1Database>
  requireOperator: (request: Request, env: Bindings) => Promise<boolean>
  nowIso?: () => string
}>

const EVIDENCE_BUNDLES_PATH = '/api/omni/evidence-bundles'
const PROOF_BUNDLES_PATH = '/api/omni/public-proof-bundles'

const evidenceDetailPattern = /^\/api\/omni\/evidence-bundles\/([^/]+)$/
const proofDetailPattern = /^\/api\/omni\/public-proof-bundles\/([^/]+)$/

// REQUEST SCHEMAS

const OptionalRefArray = S.optionalKey(S.Array(S.String))

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
  redactionState: S.Literals(['not_needed', 'redacted', 'private_only', 'blocked']),
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

const CreateEvidenceBundleRequest = S.Struct({
  entries: S.Array(OmniEvidenceEntryRequest),
  id: S.optionalKey(S.String),
  idempotencyKey: S.String,
  legalSensitive: S.optionalKey(S.Boolean),
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  publicReceiptRef: S.optionalKey(S.String),
  sourceAuthorityCaveatRef: S.optionalKey(S.String),
  status: S.optionalKey(
    S.Literals(['draft', 'ready', 'redaction_required', 'superseded', 'archived']),
  ),
  summaryRef: S.String,
  workKind: OmniAcceptedOutcomeWorkKind,
  workroomId: S.String,
})
type CreateEvidenceBundleRequest = typeof CreateEvidenceBundleRequest.Type

const CreateProofBundleRequest = S.Struct({
  acceptanceStateRef: S.String,
  artifactRefs: OptionalRefArray,
  economicsCaveatRef: S.String,
  id: S.optionalKey(S.String),
  idempotencyKey: S.String,
  legalCaveatRef: S.optionalKey(S.String),
  legalSensitive: S.optionalKey(S.Boolean),
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  privacyCaveatRef: S.String,
  publicReceiptRef: S.optionalKey(S.String),
  receiptRefs: OptionalRefArray,
  reviewStateRef: S.String,
  sourceRefs: OptionalRefArray,
  status: S.optionalKey(
    S.Literals(['draft', 'ready', 'blocked', 'superseded', 'archived']),
  ),
  workKind: OmniAcceptedOutcomeWorkKind,
  workroomId: S.String,
})
type CreateProofBundleRequest = typeof CreateProofBundleRequest.Type

// ERRORS

class OmniBundleRequestError extends S.TaggedErrorClass<OmniBundleRequestError>()(
  'OmniBundleRequestError',
  { reason: S.String, status: S.Number },
) {}

const requestError = (status: number, reason: string): OmniBundleRequestError =>
  new OmniBundleRequestError({ reason, status })

const bundleErrorResponse = (
  error: OmniEvidenceBundleError | OmniPublicProofBundleError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      OmniEvidenceBundleWorkroomNotFound: workroom =>
        noStoreJsonResponse(
          {
            error: 'omni_bundle_workroom_not_found',
            reason: `Workroom ${workroom.workroomId} was not found.`,
          },
          { status: 404 },
        ),
      OmniPublicProofBundleWorkroomNotFound: workroom =>
        noStoreJsonResponse(
          {
            error: 'omni_bundle_workroom_not_found',
            reason: `Workroom ${workroom.workroomId} was not found.`,
          },
          { status: 404 },
        ),
      OmniEvidenceBundleValidationError: validation =>
        noStoreJsonResponse(
          { error: 'omni_bundle_validation_error', reason: validation.reason },
          { status: 400 },
        ),
      OmniPublicProofBundleValidationError: validation =>
        noStoreJsonResponse(
          { error: 'omni_bundle_validation_error', reason: validation.reason },
          { status: 400 },
        ),
      OmniEvidenceBundleStorageError: storage =>
        noStoreJsonResponse(
          { error: 'omni_bundle_storage_error', reason: storage.reason },
          { status: 500 },
        ),
      OmniPublicProofBundleStorageError: storage =>
        noStoreJsonResponse(
          { error: 'omni_bundle_storage_error', reason: storage.reason },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

const requestErrorResponse = (error: OmniBundleRequestError): HttpResponse =>
  noStoreJsonResponse({ error: 'omni_bundle_request_error', reason: error.reason }, {
    status: error.status,
  })

// The bundle services run `assertValidInput` synchronously inside their
// `Effect.gen` body, so a public-safety/redaction violation surfaces as a defect
// rather than a typed failure. Convert the known validation defects into a 400
// so redaction enforcement is reported as a request error, not a 500 crash.
const createBundleDefectResponse = (defect: unknown): HttpResponse => {
  if (
    defect instanceof OmniEvidenceBundleValidationError ||
    defect instanceof OmniPublicProofBundleValidationError
  ) {
    return noStoreJsonResponse(
      { error: 'omni_bundle_validation_error', reason: defect.reason },
      { status: 400 },
    )
  }

  return noStoreJsonResponse(
    {
      error: 'omni_bundle_storage_error',
      reason: defect instanceof Error ? defect.message : String(defect),
    },
    { status: 500 },
  )
}

const decodeBody = <A>(
  decode: (raw: unknown) => A,
  request: Request,
): Effect.Effect<A, OmniBundleRequestError> =>
  Effect.tryPromise({
    catch: error =>
      requestError(400, error instanceof Error ? error.message : String(error)),
    try: async () => decode(await readJsonObject(request)),
  })

const requireOperatorAuth = <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, OmniBundleRequestError> =>
  Effect.flatMap(
    Effect.promise(() => dependencies.requireOperator(request, env)),
    isOperator =>
      isOperator
        ? Effect.void
        : Effect.fail(requestError(401, 'Operator authorization is required.')),
  )

const evidenceBundleCreateInput = (
  body: CreateEvidenceBundleRequest,
): CreateOmniEvidenceBundleInput => ({
  entries: body.entries,
  id: body.id,
  idempotencyKey: body.idempotencyKey,
  legalSensitive: body.legalSensitive,
  metadata: body.metadata,
  publicReceiptRef: body.publicReceiptRef,
  sourceAuthorityCaveatRef: body.sourceAuthorityCaveatRef,
  status: body.status,
  summaryRef: body.summaryRef,
  workKind: body.workKind,
  workroomId: body.workroomId,
})

const proofBundleCreateInput = (
  body: CreateProofBundleRequest,
): CreateOmniPublicProofBundleInput => ({
  acceptanceStateRef: body.acceptanceStateRef,
  artifactRefs: body.artifactRefs,
  economicsCaveatRef: body.economicsCaveatRef,
  id: body.id,
  idempotencyKey: body.idempotencyKey,
  legalCaveatRef: body.legalCaveatRef,
  legalSensitive: body.legalSensitive,
  metadata: body.metadata,
  privacyCaveatRef: body.privacyCaveatRef,
  publicReceiptRef: body.publicReceiptRef,
  receiptRefs: body.receiptRefs,
  reviewStateRef: body.reviewStateRef,
  sourceRefs: body.sourceRefs,
  status: body.status,
  workKind: body.workKind,
  workroomId: body.workroomId,
})

// HANDLERS

const createEvidenceBundle = <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    yield* requireOperatorAuth(dependencies, request, env)
    const body = yield* decodeBody(
      S.decodeUnknownSync(CreateEvidenceBundleRequest),
      request,
    )
    const record = yield* createOmniEvidenceBundle(
      dependencies.db(env),
      evidenceBundleCreateInput(body),
    )

    return noStoreJsonResponse(
      { bundle: operatorOmniEvidenceBundleProjection(record) },
      { status: 201 },
    )
  }).pipe(
    Effect.catchTag('OmniBundleRequestError', error =>
      Effect.succeed(requestErrorResponse(error)),
    ),
    Effect.catch(error => Effect.succeed(bundleErrorResponse(error))),
    Effect.catchDefect(defect =>
      Effect.succeed(createBundleDefectResponse(defect)),
    ),
  )

const createProofBundle = <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    yield* requireOperatorAuth(dependencies, request, env)
    const body = yield* decodeBody(
      S.decodeUnknownSync(CreateProofBundleRequest),
      request,
    )
    const record = yield* createOmniPublicProofBundle(
      dependencies.db(env),
      proofBundleCreateInput(body),
    )

    return noStoreJsonResponse(
      { bundle: operatorOmniProofBundleProjection(record) },
      { status: 201 },
    )
  }).pipe(
    Effect.catchTag('OmniBundleRequestError', error =>
      Effect.succeed(requestErrorResponse(error)),
    ),
    Effect.catch(error => Effect.succeed(bundleErrorResponse(error))),
    Effect.catchDefect(defect =>
      Effect.succeed(createBundleDefectResponse(defect)),
    ),
  )

const wantsOperatorView = (request: Request): boolean =>
  new URL(request.url).searchParams.get('view') === 'operator'

const readEvidenceBundle = <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  id: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const operatorRequested = wantsOperatorView(request)

    if (operatorRequested) {
      yield* requireOperatorAuth(dependencies, request, env)
    }

    const record = yield* Effect.promise(() =>
      dependencies.readEvidenceBundle(dependencies.db(env), id),
    )

    if (record === null) {
      return noStoreJsonResponse(
        { error: 'omni_bundle_not_found', reason: 'Evidence bundle was not found.' },
        { status: 404 },
      )
    }

    return noStoreJsonResponse({
      bundle: operatorRequested
        ? operatorOmniEvidenceBundleProjection(record)
        : customerOmniEvidenceBundleProjection(record),
      view: operatorRequested ? 'operator' : 'customer',
    })
  }).pipe(
    Effect.catchTag('OmniBundleRequestError', error =>
      Effect.succeed(requestErrorResponse(error)),
    ),
  )

const readProofBundle = <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  id: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const operatorRequested = wantsOperatorView(request)

    if (operatorRequested) {
      yield* requireOperatorAuth(dependencies, request, env)
    }

    const record = yield* Effect.promise(() =>
      dependencies.readProofBundle(dependencies.db(env), id),
    )

    if (record === null) {
      return noStoreJsonResponse(
        {
          error: 'omni_bundle_not_found',
          reason: 'Public proof bundle was not found.',
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse({
      bundle: operatorRequested
        ? operatorOmniProofBundleProjection(record)
        : publicOmniProofBundleProjection(record),
      view: operatorRequested ? 'operator' : 'public',
    })
  }).pipe(
    Effect.catchTag('OmniBundleRequestError', error =>
      Effect.succeed(requestErrorResponse(error)),
    ),
  )

export const makeOmniBundleRoutes = <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
) => ({
  routeOmniBundleRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === EVIDENCE_BUNDLES_PATH) {
      return M.value(request.method).pipe(
        M.when('POST', () => createEvidenceBundle(dependencies, request, env)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    if (url.pathname === PROOF_BUNDLES_PATH) {
      return M.value(request.method).pipe(
        M.when('POST', () => createProofBundle(dependencies, request, env)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const evidenceDetail = evidenceDetailPattern.exec(url.pathname)

    if (evidenceDetail?.[1] !== undefined) {
      const id = decodeURIComponent(evidenceDetail[1])

      return M.value(request.method).pipe(
        M.when('GET', () => readEvidenceBundle(dependencies, request, env, id)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const proofDetail = proofDetailPattern.exec(url.pathname)

    if (proofDetail?.[1] !== undefined) {
      const id = decodeURIComponent(proofDetail[1])

      return M.value(request.method).pipe(
        M.when('GET', () => readProofBundle(dependencies, request, env, id)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    return undefined
  },
})
