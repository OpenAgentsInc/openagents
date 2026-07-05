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
//     compareProofBundleRead: env => makeOmniPublicProofBundleCompareReader(env),
//     serveProofBundleFromPostgres: env => makeOmniPublicProofBundlePostgresServerForEnv(env),
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
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

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
  // KS-8.17 (#8361): optional read-back mirror factory for the
  // omni_evidence_bundles / omni_public_proof_bundles rows these routes
  // write. Coordinator wiring (see the header comment) should pass
  // `mirror: env => makeSupervisionLongtailMirrorForEnv(env, { db: dependencies.db(env) })`
  // alongside `db`; undefined stays a safe no-op.
  mirror?: (env: Bindings) => SupervisionLongtailMirror | undefined
  // KS-8.17 read-cutover follow-up (#8361): optional, fail-soft (never
  // throws/rejects) shadow-compare hook for the public proof-bundle read —
  // pass `compareProofBundleRead: env => makeOmniPublicProofBundleCompareReader(env)`.
  // Undefined (the default) or a no-Postgres-binding/`reads=d1` environment
  // is a safe no-op; D1 always serves the actual response either way. MUST
  // be awaited inline (not fire-and-forget) — a Worker may cancel an
  // un-awaited async tail once the response is sent.
  compareProofBundleRead?: (
    env: Bindings,
  ) => ((id: string) => Promise<void>) | undefined
  // KS-8.17 read-cutover follow-up (#8361): optional, fail-soft bounded
  // real-Postgres-serve reader for the public proof-bundle read — pass
  // `serveProofBundleFromPostgres: env => makeOmniPublicProofBundlePostgresServerForEnv(env)`.
  // Undefined (the default), a non-`postgres` reads mode, or a Postgres
  // query error all resolve to `undefined`, and the route reads D1 exactly
  // as before (`readProofBundle`). A DEFINED result (even `{ record: null }`
  // — a genuine "not found") is trusted and served directly with no D1
  // fallback; that is the entire point of real serving.
  serveProofBundleFromPostgres?: (
    env: Bindings,
  ) => (
    | ((
        id: string,
      ) => Promise<Readonly<{ record: OmniPublicProofBundleRecord | null }> | undefined>)
    | undefined
  )
  readEvidenceBundle: OmniEvidenceBundleReader<D1Database>
  readProofBundle: OmniPublicProofBundleReader<D1Database>
  requireOperator: (request: Request, env: Bindings) => Promise<boolean>
  nowIso?: () => string
}>

const EVIDENCE_BUNDLES_PATH = '/api/omni/evidence-bundles'
const PROOF_BUNDLES_PATH = '/api/omni/public-proof-bundles'

const evidenceDetailPattern = /^\/api\/omni\/evidence-bundles\/([^/]+)$/
const proofDetailPattern = /^\/api\/omni\/public-proof-bundles\/([^/]+)$/
const proofHandoffPagePattern = /^\/handoff\/([^/]+)$/

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

const escapeHtml = (value: string): string =>
  value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;')

const htmlResponse = (html: string, init: ResponseInit = {}): HttpResponse => {
  const headers = new Headers(init.headers)
  headers.set('cache-control', 'no-store')
  headers.set('content-type', 'text/html; charset=utf-8')

  return new Response(html, { ...init, headers })
}

const humanize = (value: string): string => value.replaceAll('_', ' ')

const refListHtml = (
  title: string,
  refs: ReadonlyArray<string>,
  emptyText: string,
): string => {
  const items =
    refs.length === 0
      ? `<p class="empty">${escapeHtml(emptyText)}</p>`
      : `<ul>${refs.map(ref => `<li><code>${escapeHtml(ref)}</code></li>`).join('')}</ul>`

  return `<section class="pane"><h2>${escapeHtml(title)}</h2>${items}</section>`
}

const renderPublicProofBundleHandoffHtml = (
  bundle: ReturnType<typeof publicOmniProofBundleProjection>,
  id: string,
  request: Request,
): string => {
  const apiPath = `/api/omni/public-proof-bundles/${encodeURIComponent(id)}`
  const canonicalUrl = new URL(request.url)
  canonicalUrl.search = ''
  const updatedLabel = bundle.status === 'ready' ? 'Ready' : humanize(bundle.status)
  const hasLegalCaveat = bundle.legalCaveatRef !== null && bundle.legalCaveatRef !== ''

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>OpenAgents handoff ${escapeHtml(bundle.workroomId)}</title>
<style>
:root{color-scheme:dark;--bg:#000;--panel:#080808;--line:#222;--line-strong:#333;--text:#f1efe8;--muted:rgba(255,255,255,.62);--faint:rgba(255,255,255,.42);--accent:#ffb400;--good:#00c853;--warn:#ff6f00}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font-family:"Berkeley Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:14px;line-height:1.55}a{color:var(--text);text-decoration-color:var(--accent);text-underline-offset:3px}main{width:min(1120px,calc(100vw - 32px));margin:0 auto;padding:32px 0 44px}.top{display:grid;gap:18px;border-bottom:1px solid var(--line);padding-bottom:20px}.kicker{margin:0;color:var(--faint);font-size:11px;text-transform:uppercase;letter-spacing:.08em}.headline{display:grid;gap:10px}.headline h1{max-width:760px;margin:0;color:var(--text);font-size:28px;font-weight:650;line-height:1.16;letter-spacing:0;text-wrap:balance}.headline p{max-width:74ch;margin:0;color:var(--muted)}.status{display:flex;flex-wrap:wrap;gap:8px}.pill{display:inline-flex;min-height:28px;align-items:center;border:1px solid var(--line-strong);padding:4px 8px;color:var(--muted);font-size:12px}.pill.ready{border-color:rgba(0,200,83,.45);color:#bbf7ce}.pill.warn{border-color:rgba(255,111,0,.5);color:#ffd6a8}.grid{display:grid;grid-template-columns:minmax(0,1.1fr) minmax(300px,.9fr);gap:16px;margin-top:18px}.pane{border:1px solid var(--line);background:var(--panel);padding:16px}.pane h2{margin:0 0 10px;color:rgba(255,255,255,.86);font-size:14px;font-weight:650}.pane p{margin:0;color:var(--muted)}.facts{display:grid;border-top:1px solid var(--line)}.fact{display:grid;grid-template-columns:150px minmax(0,1fr);gap:12px;border-bottom:1px solid var(--line);padding:10px 0}.fact dt{color:var(--faint);font-size:11px;text-transform:uppercase;letter-spacing:.08em}.fact dd{min-width:0;margin:0;color:var(--muted);overflow-wrap:anywhere}.stack{display:grid;gap:12px}ul{display:grid;gap:8px;margin:0;padding:0;list-style:none}li{min-width:0;border-top:1px solid var(--line);padding-top:8px;color:var(--muted);overflow-wrap:anywhere}code{font:inherit;color:rgba(255,255,255,.78);overflow-wrap:anywhere}.empty{color:var(--faint)}.notice{border-color:rgba(255,180,0,.4)}.notice p{color:#ead49a}.footer{margin-top:18px;border-top:1px solid var(--line);padding-top:16px;color:var(--faint);font-size:12px}@media (max-width:820px){main{width:min(100% - 24px,1120px);padding-top:22px}.grid{grid-template-columns:1fr}.fact{grid-template-columns:1fr;gap:2px}.headline h1{font-size:23px}}@media (prefers-reduced-motion:reduce){*,*::before,*::after{scroll-behavior:auto!important}}
</style>
</head>
<body>
<main>
  <header class="top">
    <p class="kicker">OpenAgents public handoff</p>
    <div class="headline">
      <h1>Redacted deliverables and receipts for ${escapeHtml(humanize(bundle.workKind))}</h1>
      <p>This page contains only customer-shareable proof refs from the public proof bundle. Private prompts, provider payloads, raw logs, wallet material, settlement details, and client-identifying data are not exposed here.</p>
    </div>
    <div class="status">
      <span class="pill ${bundle.status === 'ready' ? 'ready' : 'warn'}">${escapeHtml(updatedLabel)}</span>
      <span class="pill">${escapeHtml(humanize(bundle.workKind))}</span>
      <span class="pill">No settlement implication</span>
      ${hasLegalCaveat ? '<span class="pill warn">Legal caveat present</span>' : ''}
    </div>
  </header>
  <div class="grid">
    <div class="stack">
      ${refListHtml('Redacted deliverables', bundle.artifactRefs, 'No public deliverable refs are attached yet.')}
      ${refListHtml('Receipts', bundle.receiptRefs, 'No public receipt refs are attached yet.')}
      ${refListHtml('Evidence sources', bundle.sourceRefs, 'No public source refs are attached yet.')}
    </div>
    <aside class="stack">
      <section class="pane">
        <h2>Bundle facts</h2>
        <dl class="facts">
          <div class="fact"><dt>Workroom</dt><dd><code>${escapeHtml(bundle.workroomId)}</code></dd></div>
          <div class="fact"><dt>Public receipt</dt><dd><code>${escapeHtml(bundle.publicReceiptRef)}</code></dd></div>
          <div class="fact"><dt>Review state</dt><dd><code>${escapeHtml(bundle.reviewStateRef)}</code></dd></div>
          <div class="fact"><dt>Acceptance</dt><dd><code>${escapeHtml(bundle.acceptanceStateRef)}</code></dd></div>
          <div class="fact"><dt>Privacy</dt><dd><code>${escapeHtml(bundle.privacyCaveatRef)}</code></dd></div>
          <div class="fact"><dt>Economics</dt><dd><code>${escapeHtml(bundle.economicsCaveatRef)}</code></dd></div>
          <div class="fact"><dt>Legal</dt><dd><code>${escapeHtml(bundle.legalCaveatRef ?? 'not_applicable')}</code></dd></div>
        </dl>
      </section>
      <section class="pane notice">
        <h2>Share boundary</h2>
        <p>Use this link for client-facing handoff. It is evidence-only and does not authorize work, payout, settlement, provider access, or registry state changes.</p>
      </section>
      <section class="pane">
        <h2>Machine-readable source</h2>
        <p><a href="${escapeHtml(apiPath)}">${escapeHtml(apiPath)}</a></p>
      </section>
    </aside>
  </div>
  <p class="footer">Canonical handoff URL: <a href="${escapeHtml(canonicalUrl.toString())}">${escapeHtml(canonicalUrl.toString())}</a></p>
</main>
</body>
</html>`
}

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
      undefined,
      dependencies.mirror?.(env),
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
      undefined,
      dependencies.mirror?.(env),
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

/**
 * KS-8.17 read-compare shadow (#8361): awaited inline (a Worker may cancel
 * an un-awaited async tail once the response is sent), but fail-soft — it
 * never throws/rejects and never changes what is served. D1 always serves
 * this route regardless of `KHALA_SYNC_SUPERVISION_READS`.
 */
const runProofBundleCompare = async <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  env: Bindings,
  id: string,
): Promise<void> => {
  try {
    await dependencies.compareProofBundleRead?.(env)?.(id)
  } catch {
    // Shadow-compare wiring itself must never affect a served response.
  }
}

/**
 * KS-8.17 read-cutover follow-up (#8361): the bounded real-Postgres-serve
 * path, tried before the normal D1 read. `serveProofBundleFromPostgres` is
 * `undefined` unless `KHALA_SYNC_SUPERVISION_READS=postgres`; the returned
 * reader is itself fail-soft (never throws), and this wrapper also swallows
 * any unexpected throw so a broken wiring can never break the served
 * response. ANY defined result (even `{ record: null }` — a genuine "not
 * found" from Postgres) is trusted and returned with no D1 fallback; only
 * `undefined` (ineligible or a caught Postgres error) falls back to the
 * unchanged D1-served `readProofBundle` path.
 */
const resolveProofBundle = async <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  env: Bindings,
  id: string,
): Promise<OmniPublicProofBundleRecord | null> => {
  let served:
    | Readonly<{ record: OmniPublicProofBundleRecord | null }>
    | undefined
  try {
    served = await dependencies.serveProofBundleFromPostgres?.(env)?.(id)
  } catch {
    served = undefined
  }
  if (served !== undefined) {
    return served.record
  }
  return dependencies.readProofBundle(dependencies.db(env), id)
}

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

    yield* Effect.promise(() => runProofBundleCompare(dependencies, env, id))

    const record = yield* Effect.promise(() =>
      resolveProofBundle(dependencies, env, id),
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

const readProofBundleHandoffPage = <Bindings extends OmniBundleRouteEnv>(
  dependencies: OmniBundleRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  id: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    yield* Effect.promise(() => runProofBundleCompare(dependencies, env, id))

    const record = yield* Effect.tryPromise(() =>
      resolveProofBundle(dependencies, env, id),
    )

    if (record === null) {
      return htmlResponse(
        `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Handoff unavailable | OpenAgents</title></head><body style="margin:0;background:#000;color:#f1efe8;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;display:grid;min-height:100vh;place-items:center"><main style="width:min(460px,calc(100vw - 32px));border:1px solid #222;background:#080808;padding:24px"><p style="margin:0 0 10px;color:rgba(255,255,255,.42);font-size:11px;text-transform:uppercase;letter-spacing:.08em">OpenAgents public handoff</p><h1 style="margin:0 0 12px;font-size:22px;line-height:1.2">Handoff unavailable</h1><p style="margin:0;color:rgba(255,255,255,.62);line-height:1.55">The requested public proof bundle was not found.</p></main></body></html>`,
        { status: 404 },
      )
    }

    return htmlResponse(
      renderPublicProofBundleHandoffHtml(
        publicOmniProofBundleProjection(record),
        id,
        request,
      ),
    )
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        htmlResponse(
          `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Handoff unavailable | OpenAgents</title></head><body style="margin:0;background:#000;color:#f1efe8;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;display:grid;min-height:100vh;place-items:center"><main style="width:min(460px,calc(100vw - 32px));border:1px solid #222;background:#080808;padding:24px"><p style="margin:0 0 10px;color:rgba(255,255,255,.42);font-size:11px;text-transform:uppercase;letter-spacing:.08em">OpenAgents public handoff</p><h1 style="margin:0 0 12px;font-size:22px;line-height:1.2">Handoff unavailable</h1><p style="margin:0;color:rgba(255,255,255,.62);line-height:1.55">The handoff page could not read the public proof bundle.</p></main></body></html>`,
          { status: 500 },
        ),
      ),
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

    const proofHandoffPage = proofHandoffPagePattern.exec(url.pathname)

    if (proofHandoffPage?.[1] !== undefined) {
      const id = decodeURIComponent(proofHandoffPage[1])

      return M.value(request.method).pipe(
        M.when('GET', () =>
          readProofBundleHandoffPage(dependencies, request, env, id),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

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
