import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
// KS-8.12 (#8323): sites writes ride the dual-write mirror seam — the
// mirroring database is a passthrough for non-scoped statements and
// degrades to the raw D1 handle when no KHALA_SYNC_DB binding exists.
import { sitesContentDatabaseForEnv as openAgentsDatabase } from './sites-content-store'
import {
  AutopilotSiteAccessMode,
  AutopilotSiteAccessPrincipalKind,
  AutopilotSiteAccessRole,
  AutopilotSiteEnvironmentValueKind,
  type AutopilotSiteError,
  AutopilotSiteLaunchChecklist,
  AutopilotSiteProjectNotFound,
  AutopilotSiteStaticAssetsManifest,
  AutopilotSiteSourceRepository,
  AutopilotSiteVersionSourceKind,
  AutopilotSiteVisibility,
  AutopilotSitesService,
  type CreateAutopilotSiteFromOrderInput,
  type GrantAutopilotSiteAccessInput,
  type RequestAutopilotSiteGenerationInput,
  type SaveAutopilotSiteVersionInput,
  type UpdateAutopilotSiteAccessInput,
  type UpsertAutopilotSiteEnvironmentValueInput,
} from './sites'
import {
  type SiteCompatibilityError,
  SiteCompatibilityProjectFile,
  SiteCompatibilitySourceKind,
  makeSiteCompatibilityService,
} from './sites-compatibility'
import {
  type SiteBuildValidationError,
  SiteBuildValidationCompatibilityHint,
  makeSiteBuildValidationService,
} from './sites-build-validations'
import {
  SiteSourceExportDestination,
  type SiteSourceExportError,
  SiteSourceExportKind,
  SiteSourceExportSecretScan,
  recordSiteSourceExport,
} from './site-source-exports'

type OperatorSitesEnv = Readonly<{
  ARTIFACTS?: R2Bucket
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type OperatorSitesSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type OperatorSitesRouteDependencies<
  Session extends OperatorSitesSession,
  Bindings extends OperatorSitesEnv,
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

class OperatorSitesUnauthorized extends S.TaggedErrorClass<OperatorSitesUnauthorized>()(
  'OperatorSitesUnauthorized',
  {},
) {}

class OperatorSitesForbidden extends S.TaggedErrorClass<OperatorSitesForbidden>()(
  'OperatorSitesForbidden',
  {},
) {}

class OperatorSitesBadRequest extends S.TaggedErrorClass<OperatorSitesBadRequest>()(
  'OperatorSitesBadRequest',
  {
    reason: S.String,
  },
) {}

class OperatorSitesSessionError extends S.TaggedErrorClass<OperatorSitesSessionError>()(
  'OperatorSitesSessionError',
  {
    error: S.Defect,
  },
) {}

type OperatorSitesRouteError =
  | AutopilotSiteError
  | SiteSourceExportError
  | SiteBuildValidationError
  | SiteCompatibilityError
  | OperatorSitesBadRequest
  | OperatorSitesForbidden
  | OperatorSitesSessionError
  | OperatorSitesUnauthorized

export class CreateOperatorSiteFromOrderRequest extends S.Class<CreateOperatorSiteFromOrderRequest>(
  'CreateOperatorSiteFromOrderRequest',
)({
  softwareOrderId: S.String,
  slug: S.String,
  title: S.String,
  accessMode: S.optionalKey(AutopilotSiteAccessMode),
  projectId: S.optionalKey(S.String),
  teamId: S.optionalKey(S.String),
  visibility: S.optionalKey(AutopilotSiteVisibility),
}) {}

export class UpdateOperatorSiteAccessRequest extends S.Class<UpdateOperatorSiteAccessRequest>(
  'UpdateOperatorSiteAccessRequest',
)({
  accessMode: AutopilotSiteAccessMode,
  visibility: AutopilotSiteVisibility,
  launchChecklist: S.optionalKey(AutopilotSiteLaunchChecklist),
}) {}

export class SaveOperatorSiteVersionRequest extends S.Class<SaveOperatorSiteVersionRequest>(
  'SaveOperatorSiteVersionRequest',
)({
  sourceKind: AutopilotSiteVersionSourceKind,
  buildStatus: S.Literals(['build_failed', 'saved']),
  staticAssetsManifest: AutopilotSiteStaticAssetsManifest,
  actorRunId: S.optionalKey(S.String),
  buildCommand: S.optionalKey(S.String),
  buildLogText: S.optionalKey(S.String),
  d1BindingName: S.optionalKey(S.String),
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  r2BindingName: S.optionalKey(S.String),
  sourceArchiveText: S.optionalKey(S.String),
  sourceCommitSha: S.optionalKey(S.String),
  workerModuleText: S.optionalKey(S.String),
  workerModuleR2Key: S.optionalKey(S.String),
}) {}

export class UpsertOperatorSiteEnvironmentValueRequest extends S.Class<UpsertOperatorSiteEnvironmentValueRequest>(
  'UpsertOperatorSiteEnvironmentValueRequest',
)({
  key: S.String,
  kind: AutopilotSiteEnvironmentValueKind,
  plainValue: S.optionalKey(S.String),
  secretRef: S.optionalKey(S.String),
}) {}

export class GrantOperatorSiteAccessRequest extends S.Class<GrantOperatorSiteAccessRequest>(
  'GrantOperatorSiteAccessRequest',
)({
  principalKind: AutopilotSiteAccessPrincipalKind,
  principalRef: S.String,
  role: AutopilotSiteAccessRole,
}) {}

export class GenerateOperatorSiteRequest extends S.Class<GenerateOperatorSiteRequest>(
  'GenerateOperatorSiteRequest',
)({
  actorRunId: S.optionalKey(S.String),
  operatorNotes: S.optionalKey(S.String),
}) {}

export class CreateOperatorSiteSourceExportRequest extends S.Class<CreateOperatorSiteSourceExportRequest>(
  'CreateOperatorSiteSourceExportRequest',
)({
  destination: SiteSourceExportDestination,
  exportKind: SiteSourceExportKind,
  secretScan: SiteSourceExportSecretScan,
  approve: S.optionalKey(S.Boolean),
  expiresInSeconds: S.optionalKey(S.Number),
  receipt: S.optionalKey(S.Record(S.String, S.Unknown)),
  sourceArtifactRef: S.optionalKey(S.String),
}) {}

export class CheckOperatorSiteCompatibilityRequest extends S.Class<CheckOperatorSiteCompatibilityRequest>(
  'CheckOperatorSiteCompatibilityRequest',
)({
  files: S.Array(SiteCompatibilityProjectFile),
  sourceKind: S.optionalKey(SiteCompatibilitySourceKind),
  sourceRepository: S.optionalKey(S.NullOr(AutopilotSiteSourceRepository)),
}) {}

export class ValidateOperatorSiteBuildRequest extends S.Class<ValidateOperatorSiteBuildRequest>(
  'ValidateOperatorSiteBuildRequest',
)({
  buildLogText: S.optionalKey(S.String),
  compatibility: S.optionalKey(SiteBuildValidationCompatibilityHint),
  files: S.Array(SiteCompatibilityProjectFile),
  requestedBuildCommand: S.optionalKey(S.String),
  sourceCommitSha: S.optionalKey(S.String),
  sourceKind: S.optionalKey(AutopilotSiteVersionSourceKind),
  sourceRepository: S.optionalKey(S.NullOr(AutopilotSiteSourceRepository)),
}) {}

const routeErrorResponse = (error: OperatorSitesRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      AutopilotSiteProjectNotFound: ({ siteId }) =>
        noStoreJsonResponse(
          { error: 'site_not_found', siteId },
          { status: 404 },
        ),
      AutopilotSiteDeploymentNotFound: ({ deploymentId, siteId }) =>
        noStoreJsonResponse(
          { deploymentId, error: 'site_deployment_not_found', siteId },
          { status: 404 },
        ),
      AutopilotSiteLaunchChecklistRequired: ({ reason, siteId }) =>
        noStoreJsonResponse(
          { error: 'site_launch_checklist_required', reason, siteId },
          { status: 409 },
        ),
      AutopilotSiteRuntimeNotDeployable: ({ reason, siteId, versionId }) =>
        noStoreJsonResponse(
          { error: 'site_runtime_not_deployable', reason, siteId, versionId },
          { status: 409 },
        ),
      AutopilotSiteSlugUnavailable: ({ slug }) =>
        noStoreJsonResponse(
          { error: 'site_slug_unavailable', slug },
          { status: 409 },
        ),
      AutopilotSiteSoftwareOrderNotFound: ({ softwareOrderId }) =>
        noStoreJsonResponse(
          { error: 'software_order_not_found', softwareOrderId },
          { status: 404 },
        ),
      AutopilotSiteStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      AutopilotSiteUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_site_payload', reason },
          { status: 400 },
        ),
      AutopilotSiteVersionNotDeployable: ({ buildStatus, versionId }) =>
        noStoreJsonResponse(
          { buildStatus, error: 'site_version_not_deployable', versionId },
          { status: 409 },
        ),
      AutopilotSiteVersionNotFound: ({ siteId, versionId }) =>
        noStoreJsonResponse(
          { error: 'site_version_not_found', siteId, versionId },
          { status: 404 },
        ),
      SiteBuildValidationStorageError: () =>
        noStoreJsonResponse(
          { error: 'site_build_validation_storage_error' },
          { status: 500 },
        ),
      SiteBuildValidationUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_site_build_validation_payload', reason },
          { status: 400 },
        ),
      SiteBuildValidationValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'site_build_validation_error', reason },
          { status: 400 },
        ),
      SiteSourceExportStorageError: () =>
        noStoreJsonResponse(
          { error: 'site_source_export_storage_error' },
          { status: 500 },
        ),
      SiteSourceExportValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'site_source_export_validation_error', reason },
          { status: 400 },
        ),
      SiteCompatibilityStorageError: () =>
        noStoreJsonResponse(
          { error: 'site_compatibility_storage_error' },
          { status: 500 },
        ),
      SiteCompatibilityUnsafePayload: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'unsafe_site_compatibility_payload', reason },
          { status: 400 },
        ),
      SiteCompatibilityValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'site_compatibility_validation_error', reason },
          { status: 400 },
        ),
      OperatorSitesBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      OperatorSitesForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      OperatorSitesSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OperatorSitesUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const decodeJsonBody = <Schema extends S.Top>(
  request: Request,
  schema: Schema,
) =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      catch: error =>
        new OperatorSitesBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
      try: () => request.json(),
    })

    return yield* S.decodeUnknownEffect(schema)(payload)
  }).pipe(
    Effect.mapError(error =>
      error instanceof OperatorSitesBadRequest
        ? error
        : new OperatorSitesBadRequest({ reason: 'invalid request body' }),
    ),
  )

const requireAdminSession = <
  Session extends OperatorSitesSession,
  Bindings extends OperatorSitesEnv,
>(
  dependencies: OperatorSitesRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      catch: error => new OperatorSitesSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorSitesUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorSitesForbidden({})
    }

    return session
  })

const runRoute = (
  env: OperatorSitesEnv,
  effect: Effect.Effect<
    HttpResponse,
    OperatorSitesRouteError,
    AutopilotSitesService
  >,
): Effect.Effect<HttpResponse> =>
  effect.pipe(
    Effect.provide(AutopilotSitesService.layer(env)),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const createInput = (
  session: OperatorSitesSession,
  body: CreateOperatorSiteFromOrderRequest,
): CreateAutopilotSiteFromOrderInput => ({
  actorUserId: session.user.userId,
  softwareOrderId: body.softwareOrderId,
  slug: body.slug,
  title: body.title,
  ...(body.accessMode === undefined ? {} : { accessMode: body.accessMode }),
  ...(body.projectId === undefined ? {} : { projectId: body.projectId }),
  ...(body.teamId === undefined ? {} : { teamId: body.teamId }),
  ...(body.visibility === undefined ? {} : { visibility: body.visibility }),
})

const updateAccessInput = (
  session: OperatorSitesSession,
  siteId: string,
  body: UpdateOperatorSiteAccessRequest,
): UpdateAutopilotSiteAccessInput => ({
  accessMode: body.accessMode,
  actorUserId: session.user.userId,
  siteId,
  visibility: body.visibility,
  ...(body.launchChecklist === undefined
    ? {}
    : { launchChecklist: body.launchChecklist }),
})

const saveVersionInput = (
  session: OperatorSitesSession,
  siteId: string,
  body: SaveOperatorSiteVersionRequest,
): SaveAutopilotSiteVersionInput => ({
  actorUserId: session.user.userId,
  buildStatus: body.buildStatus,
  siteId,
  sourceKind: body.sourceKind,
  staticAssetsManifest: body.staticAssetsManifest,
  ...(body.actorRunId === undefined ? {} : { actorRunId: body.actorRunId }),
  ...(body.buildCommand === undefined
    ? {}
    : { buildCommand: body.buildCommand }),
  ...(body.buildLogText === undefined
    ? {}
    : { buildLogText: body.buildLogText }),
  ...(body.d1BindingName === undefined
    ? {}
    : { d1BindingName: body.d1BindingName }),
  ...(body.metadata === undefined ? {} : { metadata: body.metadata }),
  ...(body.r2BindingName === undefined
    ? {}
    : { r2BindingName: body.r2BindingName }),
  ...(body.sourceArchiveText === undefined
    ? {}
    : { sourceArchiveText: body.sourceArchiveText }),
  ...(body.sourceCommitSha === undefined
    ? {}
    : { sourceCommitSha: body.sourceCommitSha }),
  ...(body.workerModuleR2Key === undefined
    ? {}
    : { workerModuleR2Key: body.workerModuleR2Key }),
  ...(body.workerModuleText === undefined
    ? {}
    : { workerModuleText: body.workerModuleText }),
})

const upsertEnvironmentValueInput = (
  session: OperatorSitesSession,
  siteId: string,
  body: UpsertOperatorSiteEnvironmentValueRequest,
): UpsertAutopilotSiteEnvironmentValueInput => ({
  actorUserId: session.user.userId,
  key: body.key,
  kind: body.kind,
  siteId,
  ...(body.plainValue === undefined ? {} : { plainValue: body.plainValue }),
  ...(body.secretRef === undefined ? {} : { secretRef: body.secretRef }),
})

const grantAccessInput = (
  session: OperatorSitesSession,
  siteId: string,
  body: GrantOperatorSiteAccessRequest,
): GrantAutopilotSiteAccessInput => ({
  actorUserId: session.user.userId,
  principalKind: body.principalKind,
  principalRef: body.principalRef,
  role: body.role,
  siteId,
})

const generationInput = (
  session: OperatorSitesSession,
  siteId: string,
  body: GenerateOperatorSiteRequest,
): RequestAutopilotSiteGenerationInput => ({
  actorUserId: session.user.userId,
  siteId,
  ...(body.actorRunId === undefined ? {} : { actorRunId: body.actorRunId }),
  ...(body.operatorNotes === undefined
    ? {}
    : { operatorNotes: body.operatorNotes }),
})

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const key = request.headers.get('idempotency-key')?.trim()

  return key === undefined || key === '' ? undefined : key
}

export const makeOperatorSitesRoutes = <
  Session extends OperatorSitesSession,
  Bindings extends OperatorSitesEnv,
>(
  dependencies: OperatorSitesRouteDependencies<Session, Bindings>,
) => {
  const listOrCreateSites = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const sites = yield* AutopilotSitesService

        if (request.method === 'GET') {
          const projects = yield* sites.listProjects(100)

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({ sites: projects }),
            session,
          )
        }

        if (request.method === 'POST') {
          const body = yield* decodeJsonBody(
            request,
            CreateOperatorSiteFromOrderRequest,
          )
          const site = yield* sites.createProjectFromSoftwareOrder(
            createInput(session, body),
          )

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({ site }, { status: 201 }),
            session,
          )
        }

        return methodNotAllowed(['GET', 'POST'])
      }),
    )

  const readSite = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    runRoute(
      env,
      Effect.gen(function* () {
        const siteId = new URL(request.url).pathname.split('/').at(-1) ?? ''

        if (request.method !== 'GET') {
          return methodNotAllowed(['GET'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const sites = yield* AutopilotSitesService
        const site = yield* sites.readProjectById(siteId)

        if (site === null) {
          return yield* new AutopilotSiteProjectNotFound({ siteId })
        }

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ site }),
          session,
        )
      }),
    )

  const updateSiteAccess = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'PATCH') {
          return methodNotAllowed(['PATCH'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          UpdateOperatorSiteAccessRequest,
        )
        const sites = yield* AutopilotSitesService
        const site = yield* sites.updateProjectAccess(
          updateAccessInput(session, siteId, body),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ site }),
          session,
        )
      }),
    )

  const saveSiteVersion = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const body = yield* decodeJsonBody(
          request,
          SaveOperatorSiteVersionRequest,
        )
        const sites = yield* AutopilotSitesService
        const version = yield* sites.saveVersion(
          saveVersionInput(session, siteId, body),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ version }, { status: 201 }),
          session,
        )
      }),
    )

  const generateSite = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const body = yield* decodeJsonBody(request, GenerateOperatorSiteRequest)
        const sites = yield* AutopilotSitesService
        const generation = yield* sites.requestGeneration(
          generationInput(session, siteId, body),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ generation }, { status: 202 }),
          session,
        )
      }),
    )

  const createSourceExport = (
    siteId: string,
    versionId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const idempotencyKey = idempotencyKeyFromRequest(request)

        if (idempotencyKey === undefined) {
          return yield* new OperatorSitesBadRequest({
            reason: 'idempotency key is required',
          })
        }

        const body = yield* decodeJsonBody(
          request,
          CreateOperatorSiteSourceExportRequest,
        )
        const sourceExport = yield* recordSiteSourceExport(
          openAgentsDatabase(env),
          {
            actorUserId: session.user.userId,
            approve: body.approve,
            destination: body.destination,
            exportKind: body.exportKind,
            expiresInSeconds: body.expiresInSeconds,
            idempotencyKey,
            receipt: body.receipt,
            secretScan: body.secretScan,
            siteId,
            sourceArtifactRef: body.sourceArtifactRef,
            versionId,
          },
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ sourceExport }, { status: 201 }),
          session,
        )
      }),
    )

  const readCompatibility = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const sites = yield* AutopilotSitesService
        const site = yield* sites.readProjectById(siteId)

        if (site === null) {
          return yield* new AutopilotSiteProjectNotFound({ siteId })
        }

        const compatibility =
          yield* makeSiteCompatibilityService(
            openAgentsDatabase(env),
          ).latestReceipt(siteId)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ compatibility, site }),
          session,
        )
      }),
    )

  const checkCompatibility = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const body = yield* decodeJsonBody(
          request,
          CheckOperatorSiteCompatibilityRequest,
        )
        const sites = yield* AutopilotSitesService
        const site = yield* sites.readProjectById(siteId)

        if (site === null) {
          return yield* new AutopilotSiteProjectNotFound({ siteId })
        }

        const compatibility =
          yield* makeSiteCompatibilityService(
            openAgentsDatabase(env),
          ).checkCompatibility({
            actorUserId: session.user.userId,
            files: body.files,
            site,
            ...(body.sourceKind === undefined
              ? {}
              : { sourceKind: body.sourceKind }),
            ...(body.sourceRepository === undefined
              ? {}
              : { sourceRepository: body.sourceRepository }),
          })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ compatibility, site }, { status: 201 }),
          session,
        )
      }),
    )

  const readLatestBuildValidation = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const sites = yield* AutopilotSitesService
        const site = yield* sites.readProjectById(siteId)

        if (site === null) {
          return yield* new AutopilotSiteProjectNotFound({ siteId })
        }

        const buildValidation =
          yield* makeSiteBuildValidationService(
            openAgentsDatabase(env),
          ).latestReceipt(siteId)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ buildValidation, site }),
          session,
        )
      }),
    )

  const validateBuild = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const body = yield* decodeJsonBody(
          request,
          ValidateOperatorSiteBuildRequest,
        )
        const sites = yield* AutopilotSitesService
        const site = yield* sites.readProjectById(siteId)

        if (site === null) {
          return yield* new AutopilotSiteProjectNotFound({ siteId })
        }

        const buildValidation =
          yield* makeSiteBuildValidationService(
            openAgentsDatabase(env),
          ).validateBuild({
            actorUserId: session.user.userId,
            files: body.files,
            site,
            ...(body.buildLogText === undefined
              ? {}
              : { buildLogText: body.buildLogText }),
            ...(body.compatibility === undefined
              ? {}
              : { compatibility: body.compatibility }),
            ...(body.requestedBuildCommand === undefined
              ? {}
              : { requestedBuildCommand: body.requestedBuildCommand }),
            ...(body.sourceCommitSha === undefined
              ? {}
              : { sourceCommitSha: body.sourceCommitSha }),
            ...(body.sourceKind === undefined
              ? {}
              : { sourceKind: body.sourceKind }),
            ...(body.sourceRepository === undefined
              ? {}
              : { sourceRepository: body.sourceRepository }),
          })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ buildValidation, site }, { status: 201 }),
          session,
        )
      }),
    )

  const upsertEnvironmentValue = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'PUT' && request.method !== 'POST') {
          return methodNotAllowed(['PUT', 'POST'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          UpsertOperatorSiteEnvironmentValueRequest,
        )
        const sites = yield* AutopilotSitesService
        const value = yield* sites.upsertEnvironmentValue(
          upsertEnvironmentValueInput(session, siteId, body),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ value }),
          session,
        )
      }),
    )

  const grantSiteAccess = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const body = yield* decodeJsonBody(
          request,
          GrantOperatorSiteAccessRequest,
        )
        const sites = yield* AutopilotSitesService
        const grant = yield* sites.grantAccess(
          grantAccessInput(session, siteId, body),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ grant }, { status: 201 }),
          session,
        )
      }),
    )

  const listSiteEvents = (
    siteId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
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
        const sites = yield* AutopilotSitesService
        const events = yield* sites.listEvents(siteId, 100)

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ events }),
          session,
        )
      }),
    )

  return {
    routeOperatorSitesRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/operator/sites') {
        return listOrCreateSites(request, env, ctx)
      }

      const generateMatch = /^\/api\/operator\/sites\/([^/]+)\/generate$/.exec(
        url.pathname,
      )

      if (generateMatch !== null) {
        return generateSite(generateMatch[1] ?? '', request, env, ctx)
      }

      const eventsMatch = /^\/api\/operator\/sites\/([^/]+)\/events$/.exec(
        url.pathname,
      )

      if (eventsMatch !== null) {
        return listSiteEvents(eventsMatch[1] ?? '', request, env, ctx)
      }

      const compatibilityCheckMatch =
        /^\/api\/operator\/sites\/([^/]+)\/compatibility\/check$/.exec(
          url.pathname,
        )

      if (compatibilityCheckMatch !== null) {
        return checkCompatibility(
          compatibilityCheckMatch[1] ?? '',
          request,
          env,
          ctx,
        )
      }

      const compatibilityMatch =
        /^\/api\/operator\/sites\/([^/]+)\/compatibility$/.exec(url.pathname)

      if (compatibilityMatch !== null) {
        return readCompatibility(
          compatibilityMatch[1] ?? '',
          request,
          env,
          ctx,
        )
      }

      const buildValidationLatestMatch =
        /^\/api\/operator\/sites\/([^/]+)\/build-validations\/latest$/.exec(
          url.pathname,
        )

      if (buildValidationLatestMatch !== null) {
        return readLatestBuildValidation(
          buildValidationLatestMatch[1] ?? '',
          request,
          env,
          ctx,
        )
      }

      const buildValidationsMatch =
        /^\/api\/operator\/sites\/([^/]+)\/build-validations$/.exec(
          url.pathname,
        )

      if (buildValidationsMatch !== null) {
        return validateBuild(
          buildValidationsMatch[1] ?? '',
          request,
          env,
          ctx,
        )
      }

      const environmentValuesMatch =
        /^\/api\/operator\/sites\/([^/]+)\/environment-values$/.exec(
          url.pathname,
        )

      if (environmentValuesMatch !== null) {
        return upsertEnvironmentValue(
          environmentValuesMatch[1] ?? '',
          request,
          env,
          ctx,
        )
      }

      const accessGrantsMatch =
        /^\/api\/operator\/sites\/([^/]+)\/access-grants$/.exec(url.pathname)

      if (accessGrantsMatch !== null) {
        return grantSiteAccess(accessGrantsMatch[1] ?? '', request, env, ctx)
      }

      const saveVersionMatch =
        /^\/api\/operator\/sites\/([^/]+)\/versions$/.exec(url.pathname)

      if (saveVersionMatch !== null) {
        return saveSiteVersion(saveVersionMatch[1] ?? '', request, env, ctx)
      }

      const sourceExportMatch =
        /^\/api\/operator\/sites\/([^/]+)\/versions\/([^/]+)\/source-exports$/.exec(
          url.pathname,
        )

      if (sourceExportMatch !== null) {
        return createSourceExport(
          sourceExportMatch[1] ?? '',
          sourceExportMatch[2] ?? '',
          request,
          env,
          ctx,
        )
      }

      const accessMatch = /^\/api\/operator\/sites\/([^/]+)\/access$/.exec(
        url.pathname,
      )

      if (accessMatch !== null) {
        return updateSiteAccess(accessMatch[1] ?? '', request, env, ctx)
      }

      if (/^\/api\/operator\/sites\/[^/]+$/.test(url.pathname)) {
        return readSite(request, env, ctx)
      }

      return undefined
    },
  }
}
