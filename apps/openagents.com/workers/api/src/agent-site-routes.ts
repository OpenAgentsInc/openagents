import { Effect, Schema as S } from 'effect'
import { Option } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  makeD1AgentRegistrationStore,
} from './agent-registration'
import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  decodeUnknownWithSchema,
  parseJsonRecord,
  readJsonObject,
} from './json-boundary'
import {
  type AutopilotSiteError,
  AutopilotSiteLaunchChecklistRequired,
  AutopilotSiteProjectNotFound,
  AutopilotSiteRuntimeNotDeployable,
  AutopilotSiteSlugUnavailable,
  AutopilotSiteSoftwareOrderNotFound,
  AutopilotSiteStorageError,
  AutopilotSiteStaticAssetsManifest as AutopilotSiteStaticAssetsManifestSchema,
  AutopilotSiteUnsafePayload,
  AutopilotSiteVersionNotDeployable,
  AutopilotSiteVersionNotFound,
  AutopilotSitesService,
  type RecordAutopilotSiteEventInput,
  systemAutopilotSitesRuntime,
} from './sites'
import { saveSiteBuilderVersion } from './sites-builder-saved-versions'
import {
  SiteLibraryForbidden,
  SiteLibraryNotFound,
  SiteLibraryStorageError,
  SiteLibraryValidationError,
  archiveSiteLibrarySite,
  deleteSiteLibrarySite,
  listSiteLibrary,
  siteIsVisibleForBuilderSession,
  systemSiteLibraryRuntime,
  updateSiteLibraryAccess,
} from './site-library'
import {
  type OperatorSiteBuilderSessionProjection,
  type PublicSiteBuilderSessionProjection,
  type SiteBuilderActorKind,
  type SiteBuilderEventRecord,
  type SiteBuilderFileSnapshotRecord,
  SiteBuilderSessionStorageError,
  SiteBuilderSessionValidationError,
  appendSiteBuilderEvent,
  appendSiteBuilderMessage,
  createSiteBuilderSession,
  listSiteBuilderEventsAfter,
  listSiteBuilderFileSnapshots,
  readLatestSiteBuilderFileSnapshot,
  readSiteBuilderSessionProjection,
  recordSiteBuilderPreview,
} from './sites-builder-sessions'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

type AgentSiteSession = Readonly<{
  user: Readonly<{
    email?: string | undefined
    userId: string
  }>
}>

type AgentSiteRouteDependencies<
  Session extends AgentSiteSession,
  Bindings,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  dbForEnv: (env: Bindings) => D1Database
  artifactsForEnv: (env: Bindings) => R2Bucket | undefined
  agentStoreForEnv?: (env: Bindings) => AgentRegistrationStore
  isAdminEmail: (email: string) => boolean
}>

type AgentSiteStorageDependencies<Bindings> = Readonly<{
  artifactsForEnv: (env: Bindings) => R2Bucket | undefined
  dbForEnv: (env: Bindings) => D1Database
}>

export const AgentSitesInternalGateHeader =
  'x-openagents-agent-sites-gate' as const
export const AgentSitesInternalGateValue = 'internal-preview' as const

export class CreateAgentSiteRequest extends S.Class<CreateAgentSiteRequest>(
  'CreateAgentSiteRequest',
)({
  customerOrderId: S.optionalKey(S.String),
  dryRun: S.optionalKey(S.Boolean),
  prompt: S.optionalKey(S.String),
  siteSlug: S.optionalKey(S.String),
  sourceRepositoryUrl: S.optionalKey(S.String),
  title: S.optionalKey(S.String),
}) {}

export class OpenAgentSiteBuilderSessionRequest extends S.Class<OpenAgentSiteBuilderSessionRequest>(
  'OpenAgentSiteBuilderSessionRequest',
)({
  agentRunId: S.optionalKey(S.String),
  dryRun: S.optionalKey(S.Boolean),
  goal: S.optionalKey(S.String),
}) {}

export class RequestAgentSitePreviewRequest extends S.Class<RequestAgentSitePreviewRequest>(
  'RequestAgentSitePreviewRequest',
)({
  agentRunId: S.optionalKey(S.String),
  artifactRef: S.optionalKey(S.String),
  description: S.optionalKey(S.String),
}) {}

export class SaveAgentSiteVersionRequest extends S.Class<SaveAgentSiteVersionRequest>(
  'SaveAgentSiteVersionRequest',
)({
  agentRunId: S.optionalKey(S.String),
  artifactRef: S.optionalKey(S.String),
  buildCommand: S.optionalKey(S.String),
  buildLogText: S.optionalKey(S.String),
  buildReceiptRef: S.optionalKey(S.String),
  d1BindingName: S.optionalKey(S.String),
  notes: S.optionalKey(S.String),
  previewId: S.optionalKey(S.String),
  r2BindingName: S.optionalKey(S.String),
  siteBuilderSessionId: S.optionalKey(S.String),
  siteMetadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  sourceCommitSha: S.optionalKey(S.String),
  sourceHash: S.optionalKey(S.String),
  staticAssetsManifest: S.optionalKey(AutopilotSiteStaticAssetsManifestSchema),
  workerModuleR2Key: S.optionalKey(S.String),
}) {}

export class RequestAgentSiteDeployRequest extends S.Class<RequestAgentSiteDeployRequest>(
  'RequestAgentSiteDeployRequest',
)({
  approvalRef: S.optionalKey(S.String),
  agentRunId: S.optionalKey(S.String),
  notes: S.optionalKey(S.String),
  siteBuilderSessionId: S.optionalKey(S.String),
  versionId: S.optionalKey(S.String),
}) {}

export class CreateSiteBuilderSessionApiRequest extends S.Class<CreateSiteBuilderSessionApiRequest>(
  'CreateSiteBuilderSessionApiRequest',
)({
  customerUserId: S.optionalKey(S.String),
  orderId: S.optionalKey(S.String),
  promptSummary: S.String,
  siteId: S.optionalKey(S.String),
  sourceRevisionId: S.optionalKey(S.String),
  sourceSiteVersionId: S.optionalKey(S.String),
  workroomId: S.optionalKey(S.String),
}) {}

export class AppendSiteBuilderMessageApiRequest extends S.Class<AppendSiteBuilderMessageApiRequest>(
  'AppendSiteBuilderMessageApiRequest',
)({
  body: S.String,
}) {}

export class AppendSiteBuilderEventApiRequest extends S.Class<AppendSiteBuilderEventApiRequest>(
  'AppendSiteBuilderEventApiRequest',
)({
  eventKind: S.Literals([
    'session_created',
    'message_added',
    'phase_started',
    'phase_updated',
    'phase_completed',
    'file_changed',
    'preview_created',
    'artifact_created',
    'build_failed',
    'build_repaired',
    'save_requested',
    'deploy_requested',
    'error',
  ]),
  payload: S.optionalKey(S.Record(S.String, S.Unknown)),
  phaseKind: S.optionalKey(
    S.Literals([
      'planning',
      'foundation',
      'core',
      'styling',
      'integration',
      'optimization',
      'preview',
      'save',
      'deploy',
    ]),
  ),
  sourceRef: S.optionalKey(S.String),
  status: S.optionalKey(
    S.Literals([
      'queued',
      'running',
      'succeeded',
      'failed',
      'blocked',
      'skipped',
    ]),
  ),
  summary: S.String,
  title: S.String,
  visibility: S.optionalKey(S.Literals(['customer', 'operator', 'internal'])),
}) {}

export class SaveSiteBuilderVersionApiRequest extends S.Class<SaveSiteBuilderVersionApiRequest>(
  'SaveSiteBuilderVersionApiRequest',
)({
  siteId: S.String,
  staticAssetsManifest: S.Struct({
    assets: S.Record(
      S.String,
      S.Struct({
        r2Key: S.String,
        cacheControl: S.optionalKey(S.String),
        contentType: S.optionalKey(S.String),
      }),
    ),
  }),
  actorRunId: S.optionalKey(S.String),
  artifactRef: S.optionalKey(S.String),
  buildCommand: S.optionalKey(S.String),
  buildLogText: S.optionalKey(S.String),
  buildReceiptRef: S.optionalKey(S.String),
  d1BindingName: S.optionalKey(S.String),
  notes: S.optionalKey(S.String),
  previewId: S.optionalKey(S.String),
  r2BindingName: S.optionalKey(S.String),
  siteMetadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  sourceArchiveText: S.optionalKey(S.String),
  sourceCommitSha: S.optionalKey(S.String),
  sourceHash: S.optionalKey(S.String),
  workerModuleR2Key: S.optionalKey(S.String),
  workerModuleText: S.optionalKey(S.String),
}) {}

export class UpdateSiteLibraryAccessApiRequest extends S.Class<UpdateSiteLibraryAccessApiRequest>(
  'UpdateSiteLibraryAccessApiRequest',
)({
  accessMode: S.Literals([
    'owner_admins',
    'openagents_core',
    'customer_owner',
    'custom_users',
    'public',
  ]),
  visibility: S.Literals(['private', 'team', 'public']),
}) {}

type AgentSiteAction =
  | 'builder_session_open'
  | 'deploy_version_request'
  | 'preview_request'
  | 'project_create'
  | 'save_version'

type AgentSiteScope =
  | 'sites:builder-session:create'
  | 'sites:deploy:request'
  | 'sites:preview:request'
  | 'sites:project:create'
  | 'sites:version:save'

type AgentSiteContract = Readonly<{
  action: AgentSiteAction
  bodySchema: S.Decoder<unknown>
  requiredScope: AgentSiteScope
}>

const AgentSiteGrant = S.Struct({
  expiresAt: S.NullOr(S.String),
  grantId: S.optionalKey(S.String),
  ownerUserId: S.optionalKey(S.String),
  scopes: S.Array(S.Literals([
    'sites:builder-session:create',
    'sites:deploy:request',
    'sites:preview:request',
    'sites:project:create',
    'sites:version:save',
  ])),
  siteId: S.optionalKey(S.String),
  status: S.Literals(['active', 'revoked']),
})

const decodeAgentSiteGrant = S.decodeUnknownOption(AgentSiteGrant)

type AgentSiteGrant = typeof AgentSiteGrant.Type

type AgentSiteContractActor = Readonly<{
  appendCookies: (response: HttpResponse) => HttpResponse
  actorRef: string
  ownerUserId: string
  scopeSatisfiedBy:
    | 'browser_session_plus_internal_preview_gate'
    | 'registered_agent_token_with_agent_site_grant'
  userId: string
}>

const contractResponse = (
  actor: AgentSiteContractActor,
  input: Readonly<{
    action: AgentSiteAction
    extra?: Readonly<Record<string, unknown>> | undefined
    idempotencyKey: string
    implementationState?:
      | 'builder_session_created'
      | 'contract_only'
      | 'deploy_review_requested'
      | 'operator_review_required'
      | 'preview_queued'
      | 'project_created'
      | 'version_saved'
    requiredScope: AgentSiteScope
    responseStatus?: number | undefined
    receiptStatus?:
      | 'accepted_contract'
      | 'created'
      | 'operator_review_required'
      | 'queued'
      | 'saved'
    siteId?: string | undefined
  }>,
): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      {
        agentSites: {
          action: input.action,
          authority: {
            deploy: 'request_only',
            saveVersion: 'available_with_builder_session_and_artifact_manifest',
          },
          implementationState: input.implementationState ?? 'contract_only',
          projection: {
            deployWillRun: false,
            previewWillRun: input.implementationState === 'preview_queued',
            projectWillBeCreated: input.implementationState === 'project_created',
            versionWillBeSaved: input.implementationState === 'version_saved',
          },
          receipt: {
            actorUserId: actor.userId,
            idempotencyKey: input.idempotencyKey,
            ownerUserId: actor.ownerUserId,
            requiredScope: input.requiredScope,
            scopeSatisfiedBy: actor.scopeSatisfiedBy,
            status: input.receiptStatus ?? 'accepted_contract',
          },
          requiredScope: input.requiredScope,
          ...(input.siteId === undefined ? {} : { siteId: input.siteId }),
          ...(input.extra ?? {}),
        },
      },
      { status: input.responseStatus ?? 202 },
    ),
  )

const readBearerToken = (request: Request): string | undefined => {
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

const agentSiteGrantsFromSession = (
  session: ProgrammaticAgentSession,
): ReadonlyArray<AgentSiteGrant> => {
  const metadata = parseJsonRecord(session.credential.profileMetadataJson)
  const grants = metadata?.agentSiteGrants

  return Array.isArray(grants)
    ? grants.flatMap(grant => {
        const decoded = Option.getOrUndefined(decodeAgentSiteGrant(grant))

        return decoded === undefined ? [] : [decoded]
      })
    : []
}

const agentSiteGrantAllows = (
  grant: AgentSiteGrant,
  input: Readonly<{
    nowIso: string
    requiredScope: AgentSiteScope
    siteId?: string | undefined
  }>,
): boolean =>
  grant.status === 'active' &&
  (grant.expiresAt === null || grant.expiresAt > input.nowIso) &&
  grant.scopes.includes(input.requiredScope) &&
  (input.siteId === undefined ||
    grant.siteId === undefined ||
    grant.siteId === input.siteId)

const forbiddenScopeResponse = (requiredScope: AgentSiteScope): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      {
        error: 'agent_sites_scope_required',
        message: 'Agent Sites API requires a registered agent token with scope.',
        requiredScope,
      },
      { status: 403 },
    ),
  )

const forbiddenGateResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      {
        error: 'agent_sites_internal_gate_required',
        message:
          'Agent Sites APIs require the internal preview gate or a registered agent bearer token with a matching agentSiteGrants scope.',
        requiredHeader: AgentSitesInternalGateHeader,
      },
      { status: 403 },
    ),
  )

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const idempotencyKey = request.headers.get('idempotency-key')?.trim()

  return idempotencyKey === '' ? undefined : idempotencyKey
}

const readRequestBody = async <A>(
  request: Request,
  schema: S.Decoder<A>,
): Promise<A> => {
  const body = await readJsonObject(request)

  return decodeUnknownWithSchema(schema, body)
}

const invalidBuilderSessionRequest = (message: string) =>
  noStoreJsonResponse(
    {
      error: 'invalid_site_builder_session_request',
      message,
    },
    { status: 400 },
  )

const builderSessionNotFound = () =>
  noStoreJsonResponse(
    {
      error: 'site_builder_session_not_found',
    },
    { status: 404 },
  )

const builderSessionStorageFailure = () =>
  noStoreJsonResponse(
    {
      error: 'site_builder_session_storage_failure',
    },
    { status: 500 },
  )

const siteLibraryNotFound = () =>
  noStoreJsonResponse({ error: 'site_not_found' }, { status: 404 })

const siteLibraryForbidden = () =>
  noStoreJsonResponse({ error: 'site_forbidden' }, { status: 403 })

const siteLibraryStorageFailure = () =>
  noStoreJsonResponse({ error: 'site_library_storage_failure' }, { status: 500 })

const invalidSiteLibraryRequest = (message: string) =>
  noStoreJsonResponse(
    {
      error: 'invalid_site_library_request',
      message,
    },
    { status: 400 },
  )

const builderSessionIdempotencyRequired = () =>
  noStoreJsonResponse(
    {
      error: 'idempotency_key_required',
      message: 'Mutating Site builder session actions require Idempotency-Key.',
    },
    { status: 400 },
  )

const eventCursorFromRequest = (request: Request, url: URL): number => {
  const headerCursor = request.headers.get('last-event-id')?.trim()
  const queryCursor = url.searchParams.get('cursor')?.trim()
  const rawCursor =
    headerCursor !== undefined && headerCursor !== ''
      ? headerCursor
      : queryCursor

  if (rawCursor === null || rawCursor === undefined || rawCursor === '') {
    return 0
  }

  const cursor = Number(rawCursor)

  return Number.isSafeInteger(cursor) && cursor >= 0 ? cursor : 0
}

const siteBuilderEventStreamPayload = (
  events: ReadonlyArray<SiteBuilderEventRecord>,
): string => {
  const body = events
    .map(event =>
      [
        `id: ${event.sequence}`,
        `event: ${event.eventKind}`,
        `data: ${JSON.stringify({ event })}`,
        '',
      ].join('\n'),
    )
    .join('\n')

  return body === '' ? ': no events\n\n' : `${body}\n`
}

const siteBuilderEventStreamResponse = (
  events: Parameters<typeof siteBuilderEventStreamPayload>[0],
) =>
  new globalThis.Response(siteBuilderEventStreamPayload(events), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/event-stream; charset=utf-8',
      'x-accel-buffering': 'no',
    },
  })

const visibleBuilderFiles = (
  files: ReadonlyArray<SiteBuilderFileSnapshotRecord>,
  isAdmin: boolean,
) =>
  files.filter(
    file =>
      isAdmin || (file.visibility === 'customer' && file.previewText !== null),
  )

const latestBuilderFilesByPath = (
  files: ReadonlyArray<SiteBuilderFileSnapshotRecord>,
) => {
  const byPath = new Map<string, SiteBuilderFileSnapshotRecord>()

  for (const file of files) {
    const current = byPath.get(file.path)

    if (current === undefined || file.sequence > current.sequence) {
      byPath.set(file.path, file)
    }
  }

  return Array.from(byPath.values()).sort((left, right) =>
    left.path.localeCompare(right.path),
  )
}

const siteBuilderFileMetadata = (
  file: SiteBuilderFileSnapshotRecord,
  isAdmin: boolean,
) => ({
  byteSize: file.byteSize,
  contentHash: file.contentHash,
  createdAt: file.createdAt,
  hasPreview: file.previewText !== null,
  id: file.id,
  language: file.language,
  path: file.path,
  sequence: file.sequence,
  updatedAt: file.updatedAt,
  visibility: file.visibility,
  ...(isAdmin
    ? {
        artifactRef: file.artifactRef,
        sourceRef: file.sourceRef,
      }
    : {}),
})

const siteBuilderFileTree = (
  files: ReadonlyArray<SiteBuilderFileSnapshotRecord>,
  isAdmin: boolean,
) =>
  files.map(file => ({
    ...siteBuilderFileMetadata(file, isAdmin),
    segments: file.path.split('/'),
  }))

const siteBuilderFileReadProjection = (
  file: SiteBuilderFileSnapshotRecord,
  isAdmin: boolean,
) => ({
  ...siteBuilderFileMetadata(file, isAdmin),
  previewText: file.previewText,
})

const builderSessionFilePathFromRequest = (url: URL): string | undefined => {
  const path = url.searchParams.get('path')?.trim()

  return path === undefined || path === '' ? undefined : path
}

const adminSession = <Session extends AgentSiteSession>(
  dependencies: Pick<
    AgentSiteRouteDependencies<Session, unknown>,
    'isAdminEmail'
  >,
  session: Session,
): boolean =>
  session.user.email !== undefined &&
  dependencies.isAdminEmail(session.user.email)

const canReadBuilderSession = <Session extends AgentSiteSession>(
  dependencies: Pick<
    AgentSiteRouteDependencies<Session, unknown>,
    'isAdminEmail'
  >,
  session: Session,
  projection: Readonly<{
    operator: OperatorSiteBuilderSessionProjection
    public: PublicSiteBuilderSessionProjection
  }>,
): boolean =>
  projection.operator.ownerUserId === session.user.userId ||
  projection.operator.customerUserId === session.user.userId ||
  adminSession(dependencies, session)

const builderSessionProjection = (db: D1Database, sessionId: string) =>
  readSiteBuilderSessionProjection(db, sessionId)

const siteLibraryErrorResponse = (error: unknown) => {
  if (error instanceof SiteLibraryNotFound) {
    return siteLibraryNotFound()
  }

  if (error instanceof SiteLibraryForbidden) {
    return siteLibraryForbidden()
  }

  if (error instanceof SiteLibraryValidationError) {
    return invalidSiteLibraryRequest(error.reason)
  }

  if (error instanceof SiteLibraryStorageError) {
    return siteLibraryStorageFailure()
  }

  return siteLibraryStorageFailure()
}

const readSiteLibraryRequestBody = <A>(request: Request, schema: S.Decoder<A>) =>
  Effect.tryPromise({
    try: () => readRequestBody(request, schema),
    catch: () =>
      new SiteLibraryValidationError({
        reason: 'Request body does not match the Site library schema.',
      }),
  })

const builderSessionUnavailableResponse = <
  Session extends AgentSiteSession,
  Bindings,
>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  session: Session,
  db: D1Database,
  projection: Readonly<{
    operator: OperatorSiteBuilderSessionProjection
    public: PublicSiteBuilderSessionProjection
  }>,
): Effect.Effect<HttpResponse | null> =>
  Effect.gen(function* () {
    if (!canReadBuilderSession(dependencies, session, projection)) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionNotFound(),
        session,
      )
    }

    const siteVisible = yield* siteIsVisibleForBuilderSession(
      db,
      projection.public.siteId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (siteVisible instanceof Response) {
      return siteVisible
    }

    return siteVisible
      ? null
      : dependencies.appendRefreshedSessionCookies(
          builderSessionNotFound(),
          session,
        )
  })

const requireRouteSession = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.tryPromise({
    try: () => dependencies.requireBrowserSession(request, env, ctx),
    catch: error =>
      new SiteBuilderSessionStorageError({
        operation: 'requireRouteSession',
        reason: error instanceof Error ? error.message : String(error),
      }),
  }).pipe(Effect.catch(() => Effect.void))

const readBuilderRequestBody = <A>(request: Request, schema: S.Decoder<A>) =>
  Effect.tryPromise({
    try: () => readRequestBody(request, schema),
    catch: () =>
      new SiteBuilderSessionValidationError({
        reason: 'Request body does not match the Site builder session schema.',
      }),
  })

const siteBuilderErrorResponse = (error: unknown) => {
  if (error instanceof SiteBuilderSessionValidationError) {
    return invalidBuilderSessionRequest(error.reason)
  }

  if (error instanceof SiteBuilderSessionStorageError) {
    if (
      error.operation === 'assertSessionExists' ||
      error.reason.includes('not found')
    ) {
      return builderSessionNotFound()
    }

    return builderSessionStorageFailure()
  }

  return builderSessionStorageFailure()
}

const siteLifecycleErrorResponse = (error: AutopilotSiteError): HttpResponse => {
  if (error instanceof AutopilotSiteSoftwareOrderNotFound) {
    return noStoreJsonResponse(
      {
        error: 'software_order_not_found',
        softwareOrderId: error.softwareOrderId,
      },
      { status: 404 },
    )
  }

  if (error instanceof AutopilotSiteProjectNotFound) {
    return noStoreJsonResponse(
      { error: 'site_not_found', siteId: error.siteId },
      { status: 404 },
    )
  }

  if (error instanceof AutopilotSiteVersionNotFound) {
    return noStoreJsonResponse(
      {
        error: 'site_version_not_found',
        siteId: error.siteId,
        versionId: error.versionId,
      },
      { status: 404 },
    )
  }

  if (error instanceof AutopilotSiteSlugUnavailable) {
    return noStoreJsonResponse(
      { error: 'site_slug_unavailable', slug: error.slug },
      { status: 409 },
    )
  }

  if (error instanceof AutopilotSiteVersionNotDeployable) {
    return noStoreJsonResponse(
      {
        buildStatus: error.buildStatus,
        error: 'site_version_not_deployable',
        versionId: error.versionId,
      },
      { status: 409 },
    )
  }

  if (error instanceof AutopilotSiteRuntimeNotDeployable) {
    return noStoreJsonResponse(
      {
        error: 'site_runtime_not_deployable',
        message: error.reason,
        siteId: error.siteId,
        versionId: error.versionId,
      },
      { status: 409 },
    )
  }

  if (error instanceof AutopilotSiteLaunchChecklistRequired) {
    return noStoreJsonResponse(
      {
        error: 'site_launch_checklist_required',
        message: error.reason,
        siteId: error.siteId,
      },
      { status: 403 },
    )
  }

  if (error instanceof AutopilotSiteUnsafePayload) {
    return noStoreJsonResponse(
      { error: 'unsafe_site_payload', message: error.reason },
      { status: 400 },
    )
  }

  if (error instanceof AutopilotSiteStorageError) {
    return siteLibraryStorageFailure()
  }

  return siteLibraryStorageFailure()
}

const previewUrlForSession = (sessionId: string): string =>
  `https://sites.openagents.com/previews/${encodeURIComponent(sessionId)}`

const agentSiteBuilderMetadata = (
  actor: AgentSiteContractActor,
  input: Readonly<{
    action: AgentSiteAction
    agentRunId?: string | undefined
    idempotencyKey: string
  }>,
): Readonly<Record<string, unknown>> => ({
  agentSiteAction: {
    action: input.action,
    actorRef: actor.actorRef,
    actorUserId: actor.userId,
    agentRunId: input.agentRunId ?? null,
    idempotencyKey: input.idempotencyKey,
    ownerUserId: actor.ownerUserId,
    scopeSatisfiedBy: actor.scopeSatisfiedBy,
  },
})

const createAgentSiteBuilderSession = (
  db: D1Database,
  actor: AgentSiteContractActor,
  input: Readonly<{
    action: AgentSiteAction
    agentRunId?: string | undefined
    goal?: string | undefined
    idempotencyKey: string
    siteId: string
    status?: 'draft' | 'planning' | 'building' | undefined
  }>,
) =>
  Effect.gen(function* () {
    const promptSummary =
      input.goal?.trim() === undefined || input.goal.trim() === ''
        ? `Agent Site action for ${input.siteId}.`
        : input.goal.trim()
    const session = yield* createSiteBuilderSession(db, {
      createdByActorRef: actor.actorRef,
      customerUserId: actor.ownerUserId,
      idempotencyKey: input.idempotencyKey,
      metadata: agentSiteBuilderMetadata(actor, input),
      ownerUserId: actor.ownerUserId,
      promptSummary,
      siteId: input.siteId,
      status: input.status ?? 'planning',
    })

    yield* appendSiteBuilderEvent(db, {
      eventKind: 'session_created',
      idempotencyKey: `${input.idempotencyKey}:event`,
      payload: agentSiteBuilderMetadata(actor, input),
      phaseKind: 'planning',
      sessionId: session.id,
      sourceRef: input.siteId,
      status: 'queued',
      summary: `Agent Site ${input.action} accepted for ${input.siteId}.`,
      title: 'Agent Site action accepted',
      visibility: 'customer',
    })

    return session
  })

const readAgentSiteProjectOrResponse = (
  db: D1Database,
  artifacts: R2Bucket | undefined,
  siteId: string,
) =>
  AutopilotSitesService.fromBindings(db, artifacts)
    .readProjectById(siteId)
    .pipe(
      Effect.flatMap(project =>
        project === null
          ? Effect.fail(new AutopilotSiteProjectNotFound({ siteId }))
          : Effect.succeed(project),
      ),
    )

const agentSiteProjectCreateResponse = <Bindings>(
  dependencies: AgentSiteStorageDependencies<Bindings>,
  actor: AgentSiteContractActor,
  env: Bindings,
  input: CreateAgentSiteRequest,
  idempotencyKey: string,
  requiredScope: AgentSiteScope,
) =>
  Effect.gen(function* () {
    if (
      input.dryRun === true ||
      input.customerOrderId === undefined ||
      input.siteSlug === undefined ||
      input.title === undefined
    ) {
      return contractResponse(actor, {
        action: 'project_create',
        extra: {
          executionState: 'operator_review_required',
          missing:
            input.customerOrderId === undefined ||
            input.siteSlug === undefined ||
            input.title === undefined
              ? ['customerOrderId', 'siteSlug', 'title']
              : [],
        },
        idempotencyKey,
        implementationState: 'operator_review_required',
        receiptStatus: 'operator_review_required',
        requiredScope,
      })
    }

    const project = yield* AutopilotSitesService.fromBindings(
      dependencies.dbForEnv(env),
      dependencies.artifactsForEnv(env),
      systemAutopilotSitesRuntime,
    ).createProjectFromSoftwareOrder({
      actorUserId: actor.userId,
      softwareOrderId: input.customerOrderId,
      slug: input.siteSlug,
      title: input.title,
    })

    return contractResponse(actor, {
      action: 'project_create',
      extra: {
        executionState: 'created',
        publicUrl: `https://sites.openagents.com/${project.slug}`,
        siteProject: project,
      },
      idempotencyKey,
      implementationState: 'project_created',
      receiptStatus: 'created',
      requiredScope,
      responseStatus: 201,
      siteId: project.id,
    })
  })

const agentSiteBuilderSessionCreateResponse = <Bindings>(
  dependencies: AgentSiteStorageDependencies<Bindings>,
  actor: AgentSiteContractActor,
  env: Bindings,
  input: OpenAgentSiteBuilderSessionRequest,
  idempotencyKey: string,
  requiredScope: AgentSiteScope,
  siteId: string,
) =>
  Effect.gen(function* () {
    const db = dependencies.dbForEnv(env)

    yield* readAgentSiteProjectOrResponse(
      db,
      dependencies.artifactsForEnv(env),
      siteId,
    )
    const session = yield* createAgentSiteBuilderSession(db, actor, {
      action: 'builder_session_open',
      agentRunId: input.agentRunId,
      goal: input.goal,
      idempotencyKey,
      siteId,
      status: 'planning',
    })
    const projection = yield* builderSessionProjection(db, session.id)

    return contractResponse(actor, {
      action: 'builder_session_open',
      extra: {
        executionState: 'created',
        siteBuilderSession: projection.public,
      },
      idempotencyKey,
      implementationState: 'builder_session_created',
      receiptStatus: 'created',
      requiredScope,
      responseStatus: 201,
      siteId,
    })
  })

const agentSitePreviewRequestResponse = <Bindings>(
  dependencies: AgentSiteStorageDependencies<Bindings>,
  actor: AgentSiteContractActor,
  env: Bindings,
  input: RequestAgentSitePreviewRequest,
  idempotencyKey: string,
  requiredScope: AgentSiteScope,
  siteId: string,
) =>
  Effect.gen(function* () {
    const db = dependencies.dbForEnv(env)

    yield* readAgentSiteProjectOrResponse(
      db,
      dependencies.artifactsForEnv(env),
      siteId,
    )
    const session = yield* createAgentSiteBuilderSession(db, actor, {
      action: 'preview_request',
      agentRunId: input.agentRunId,
      goal: input.description,
      idempotencyKey: `${idempotencyKey}:session`,
      siteId,
      status: 'building',
    })
    const preview = yield* recordSiteBuilderPreview(db, {
      artifactRef: input.artifactRef,
      idempotencyKey,
      metadata: {
        agentRunId: input.agentRunId ?? null,
        description: input.description ?? null,
        requestedBy: actor.actorRef,
      },
      previewKind: 'static_r2',
      previewUrl: previewUrlForSession(session.id),
      sessionId: session.id,
      status: 'requested',
    })

    yield* appendSiteBuilderEvent(db, {
      eventKind: 'preview_created',
      idempotencyKey: `${idempotencyKey}:event`,
      payload: {
        artifactRef: preview.artifactRef,
        previewId: preview.id,
        previewUrl: preview.previewUrl,
        siteId,
      },
      phaseKind: 'preview',
      sessionId: session.id,
      sourceRef: preview.id,
      status: 'queued',
      summary: 'Queued an agent-requested Site preview.',
      title: 'Preview requested',
      visibility: 'customer',
    })

    const projection = yield* builderSessionProjection(db, session.id)

    return contractResponse(actor, {
      action: 'preview_request',
      extra: {
        executionState: 'queued',
        preview,
        siteBuilderSession: projection.public,
      },
      idempotencyKey,
      implementationState: 'preview_queued',
      receiptStatus: 'queued',
      requiredScope,
      siteId,
    })
  })

const agentSiteSaveVersionResponse = <Bindings>(
  dependencies: AgentSiteStorageDependencies<Bindings>,
  actor: AgentSiteContractActor,
  env: Bindings,
  input: SaveAgentSiteVersionRequest,
  idempotencyKey: string,
  requiredScope: AgentSiteScope,
  siteId: string,
) =>
  Effect.gen(function* () {
    const db = dependencies.dbForEnv(env)

    yield* readAgentSiteProjectOrResponse(
      db,
      dependencies.artifactsForEnv(env),
      siteId,
    )

    if (
      input.siteBuilderSessionId === undefined ||
      input.staticAssetsManifest === undefined
    ) {
      const session = yield* createAgentSiteBuilderSession(db, actor, {
        action: 'save_version',
        agentRunId: input.agentRunId,
        goal: input.notes,
        idempotencyKey: `${idempotencyKey}:session`,
        siteId,
        status: 'planning',
      })

      yield* appendSiteBuilderEvent(db, {
        eventKind: 'save_requested',
        idempotencyKey: `${idempotencyKey}:event`,
        payload: {
          buildReceiptRef: input.buildReceiptRef ?? null,
          missing: ['siteBuilderSessionId', 'staticAssetsManifest'],
          siteId,
        },
        phaseKind: 'save',
        sessionId: session.id,
        sourceRef: input.buildReceiptRef,
        status: 'blocked',
        summary:
          'Version save needs a builder session and static artifact manifest.',
        title: 'Save evidence required',
        visibility: 'customer',
      })

      const projection = yield* builderSessionProjection(db, session.id)

      return contractResponse(actor, {
        action: 'save_version',
        extra: {
          executionState: 'operator_review_required',
          requiredEvidence: ['siteBuilderSessionId', 'staticAssetsManifest'],
          siteBuilderSession: projection.public,
        },
        idempotencyKey,
        implementationState: 'operator_review_required',
        receiptStatus: 'operator_review_required',
        requiredScope,
        siteId,
      })
    }

    const result = yield* saveSiteBuilderVersion(
      db,
      dependencies.artifactsForEnv(env),
      {
        actorUserId: actor.userId,
        idempotencyKey,
        sessionId: input.siteBuilderSessionId,
        siteId,
        staticAssetsManifest: input.staticAssetsManifest,
        ...(input.agentRunId === undefined
          ? {}
          : { actorRunId: input.agentRunId }),
        ...(input.artifactRef === undefined
          ? {}
          : { artifactRef: input.artifactRef }),
        ...(input.buildCommand === undefined
          ? {}
          : { buildCommand: input.buildCommand }),
        ...(input.buildLogText === undefined
          ? {}
          : { buildLogText: input.buildLogText }),
        ...(input.buildReceiptRef === undefined
          ? {}
          : { buildReceiptRef: input.buildReceiptRef }),
        ...(input.d1BindingName === undefined
          ? {}
          : { d1BindingName: input.d1BindingName }),
        ...(input.notes === undefined ? {} : { notes: input.notes }),
        ...(input.previewId === undefined
          ? {}
          : { previewId: input.previewId }),
        ...(input.r2BindingName === undefined
          ? {}
          : { r2BindingName: input.r2BindingName }),
        ...(input.siteMetadata === undefined
          ? {}
          : { siteMetadata: input.siteMetadata }),
        ...(input.sourceCommitSha === undefined
          ? {}
          : { sourceCommitSha: input.sourceCommitSha }),
        ...(input.sourceHash === undefined
          ? {}
          : { sourceHash: input.sourceHash }),
        ...(input.workerModuleR2Key === undefined
          ? {}
          : { workerModuleR2Key: input.workerModuleR2Key }),
      },
    )
    const projection = yield* builderSessionProjection(
      db,
      input.siteBuilderSessionId,
    )

    return contractResponse(actor, {
      action: 'save_version',
      extra: {
        executionState: result.version === null ? 'idempotent_replay' : 'saved',
        savedVersion: result.savedVersion,
        siteBuilderSession: projection.public,
        siteVersion: result.version,
      },
      idempotencyKey,
      implementationState: 'version_saved',
      receiptStatus: 'saved',
      requiredScope,
      responseStatus: result.version === null ? 200 : 201,
      siteId,
    })
  })

const agentSiteDeployRequestResponse = <Bindings>(
  dependencies: AgentSiteStorageDependencies<Bindings>,
  actor: AgentSiteContractActor,
  env: Bindings,
  input: RequestAgentSiteDeployRequest,
  idempotencyKey: string,
  requiredScope: AgentSiteScope,
  siteId: string,
) =>
  Effect.gen(function* () {
    const db = dependencies.dbForEnv(env)

    yield* readAgentSiteProjectOrResponse(
      db,
      dependencies.artifactsForEnv(env),
      siteId,
    )
    const session =
      input.siteBuilderSessionId === undefined
        ? yield* createAgentSiteBuilderSession(db, actor, {
            action: 'deploy_version_request',
            agentRunId: input.agentRunId,
            goal: input.notes,
            idempotencyKey: `${idempotencyKey}:session`,
            siteId,
            status: 'planning',
          })
        : { id: input.siteBuilderSessionId }

    yield* appendSiteBuilderEvent(db, {
      eventKind: 'deploy_requested',
      idempotencyKey,
      payload: {
        approvalRef: input.approvalRef ?? null,
        notes: input.notes ?? null,
        siteId,
        versionId: input.versionId ?? null,
      },
      phaseKind: 'deploy',
      sessionId: session.id,
      sourceRef: input.versionId,
      status: 'queued',
      summary:
        'Agent requested deployment review. Production deployment remains owner/operator gated.',
      title: 'Deploy review requested',
      visibility: 'customer',
    })

    const eventInput: RecordAutopilotSiteEventInput = {
      actorUserId: actor.userId,
      siteId,
      summary: 'Agent requested Site deployment review.',
      type: 'site_deploy.requested',
      ...(input.agentRunId === undefined ? {} : { actorRunId: input.agentRunId }),
      ...(input.versionId === undefined ? {} : { versionId: input.versionId }),
      payload: {
        approvalRef: input.approvalRef ?? null,
        builderSessionId: session.id,
        deployWillRun: false,
        idempotencyKey,
      },
    }

    yield* AutopilotSitesService.fromBindings(
      db,
      dependencies.artifactsForEnv(env),
    ).recordEvent(eventInput)

    const projection = yield* builderSessionProjection(db, session.id)

    return contractResponse(actor, {
      action: 'deploy_version_request',
      extra: {
        deploymentAuthority: 'request_only',
        executionState: 'operator_review_required',
        siteBuilderSession: projection.public,
      },
      idempotencyKey,
      implementationState: 'deploy_review_requested',
      receiptStatus: 'queued',
      requiredScope,
      siteId,
    })
  })

const createBuilderSessionResponse = <
  Session extends AgentSiteSession,
  Bindings,
>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionIdempotencyRequired(),
        session,
      )
    }

    const input = yield* readBuilderRequestBody(
      request,
      CreateSiteBuilderSessionApiRequest,
    ).pipe(
      Effect.catch(() =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            invalidBuilderSessionRequest(
              'Request body does not match the Site builder session schema.',
            ),
            session,
          ),
        ),
      ),
    )

    if (input instanceof Response) {
      return input
    }

    const result = yield* createSiteBuilderSession(dependencies.dbForEnv(env), {
      createdByActorRef: `user:${session.user.userId}`,
      customerUserId: input.customerUserId ?? session.user.userId,
      idempotencyKey,
      orderId: input.orderId,
      ownerUserId: session.user.userId,
      promptSummary: input.promptSummary,
      siteId: input.siteId,
      sourceRevisionId: input.sourceRevisionId,
      sourceSiteVersionId: input.sourceSiteVersionId,
      workroomId: input.workroomId,
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (result instanceof Response) {
      return result
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      result.id,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          siteBuilderSession: projection.public,
        },
        { status: 201 },
      ),
      session,
    )
  })

const readBuilderSessionResponse = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  sessionId: string,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    const unavailable = yield* builderSessionUnavailableResponse(
      dependencies,
      session,
      dependencies.dbForEnv(env),
      projection,
    )

    if (unavailable !== null) {
      return unavailable
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        siteBuilderSession: projection.public,
        ...(adminSession(dependencies, session)
          ? { operatorSiteBuilderSession: projection.operator }
          : {}),
      }),
      session,
    )
  })

const listSiteLibraryResponse = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  url: URL,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const requestedScope = url.searchParams.get('scope')?.trim() ?? 'mine'
    const scope: 'mine' | 'public' | 'recent' =
      requestedScope === 'public' || requestedScope === 'recent'
        ? requestedScope
        : 'mine'
    const limitValue = Number(url.searchParams.get('limit') ?? '')
    const library = yield* listSiteLibrary(dependencies.dbForEnv(env), {
      actorUserId: session.user.userId,
      isAdmin: adminSession(dependencies, session),
      limit: Number.isSafeInteger(limitValue) ? limitValue : undefined,
      scope,
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteLibraryErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (library instanceof Response) {
      return library
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(library),
      session,
    )
  })

const updateSiteLibraryAccessResponse = <
  Session extends AgentSiteSession,
  Bindings,
>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  siteId: string,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const input = yield* readSiteLibraryRequestBody(
      request,
      UpdateSiteLibraryAccessApiRequest,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteLibraryErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (input instanceof Response) {
      return input
    }

    const site = yield* updateSiteLibraryAccess(
      dependencies.dbForEnv(env),
      systemSiteLibraryRuntime,
      {
        accessMode: input.accessMode,
        actorUserId: session.user.userId,
        isAdmin: adminSession(dependencies, session),
        siteId,
        visibility: input.visibility,
      },
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteLibraryErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (site instanceof Response) {
      return site
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({ site }),
      session,
    )
  })

const mutateSiteLibraryLifecycleResponse = <
  Session extends AgentSiteSession,
  Bindings,
>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  siteId: string,
  action: 'archive' | 'delete',
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            error: 'idempotency_key_required',
            message: 'Mutating Site library actions require Idempotency-Key.',
          },
          { status: 400 },
        ),
        session,
      )
    }

    const mutate =
      action === 'archive' ? archiveSiteLibrarySite : deleteSiteLibrarySite
    const site = yield* mutate(
      dependencies.dbForEnv(env),
      systemSiteLibraryRuntime,
      {
        actorUserId: session.user.userId,
        idempotencyKey,
        isAdmin: adminSession(dependencies, session),
        siteId,
      },
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteLibraryErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (site instanceof Response) {
      return site
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({ site }),
      session,
    )
  })

const streamBuilderEventsResponse = <
  Session extends AgentSiteSession,
  Bindings,
>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  sessionId: string,
  url: URL,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    const unavailable = yield* builderSessionUnavailableResponse(
      dependencies,
      session,
      dependencies.dbForEnv(env),
      projection,
    )

    if (unavailable !== null) {
      return unavailable
    }

    const cursor = eventCursorFromRequest(request, url)
    const events = yield* listSiteBuilderEventsAfter(
      dependencies.dbForEnv(env),
      {
        cursor,
        sessionId,
      },
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (events instanceof Response) {
      return events
    }

    const visibleEvents = adminSession(dependencies, session)
      ? events
      : events.filter(event => event.visibility === 'customer')

    return dependencies.appendRefreshedSessionCookies(
      siteBuilderEventStreamResponse(visibleEvents),
      session,
    )
  })

const listBuilderFilesResponse = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  sessionId: string,
  mode: 'export' | 'list' | 'tree',
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    const unavailable = yield* builderSessionUnavailableResponse(
      dependencies,
      session,
      dependencies.dbForEnv(env),
      projection,
    )

    if (unavailable !== null) {
      return unavailable
    }

    const isAdmin = adminSession(dependencies, session)
    const files = yield* listSiteBuilderFileSnapshots(
      dependencies.dbForEnv(env),
      {
        sessionId,
      },
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (files instanceof Response) {
      return files
    }

    const latestFiles = latestBuilderFilesByPath(
      visibleBuilderFiles(files, isAdmin),
    )

    if (mode === 'tree') {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          fileTree: siteBuilderFileTree(latestFiles, isAdmin),
          siteBuilderSessionId: sessionId,
        }),
        session,
      )
    }

    if (mode === 'export') {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          exportKind: 'customer_safe_preview_manifest',
          files: latestFiles.map(file =>
            siteBuilderFileReadProjection(file, isAdmin),
          ),
          fullSourceExport: 'future_artifact_token_required',
          siteBuilderSessionId: sessionId,
          sourceArchiveAvailable: false,
        }),
        session,
      )
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        files: latestFiles.map(file => siteBuilderFileMetadata(file, isAdmin)),
        siteBuilderSessionId: sessionId,
      }),
      session,
    )
  })

const readBuilderFileResponse = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  sessionId: string,
  url: URL,
) =>
  Effect.gen(function* () {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const path = builderSessionFilePathFromRequest(url)

    if (path === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        invalidBuilderSessionRequest(
          'A file path query parameter is required.',
        ),
        session,
      )
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    const unavailable = yield* builderSessionUnavailableResponse(
      dependencies,
      session,
      dependencies.dbForEnv(env),
      projection,
    )

    if (unavailable !== null) {
      return unavailable
    }

    const file = yield* readLatestSiteBuilderFileSnapshot(
      dependencies.dbForEnv(env),
      {
        path,
        sessionId,
      },
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (file instanceof Response) {
      return file
    }

    const isAdmin = adminSession(dependencies, session)

    if (
      file === null ||
      (!isAdmin &&
        (file.visibility !== 'customer' || file.previewText === null))
    ) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionNotFound(),
        session,
      )
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        file: siteBuilderFileReadProjection(file, isAdmin),
        siteBuilderSessionId: sessionId,
      }),
      session,
    )
  })

const appendBuilderMessageResponse = <
  Session extends AgentSiteSession,
  Bindings,
>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  sessionId: string,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionIdempotencyRequired(),
        session,
      )
    }

    const input = yield* readBuilderRequestBody(
      request,
      AppendSiteBuilderMessageApiRequest,
    ).pipe(
      Effect.catch(() =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            invalidBuilderSessionRequest(
              'Request body does not match the Site builder message schema.',
            ),
            session,
          ),
        ),
      ),
    )

    if (input instanceof Response) {
      return input
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    const unavailable = yield* builderSessionUnavailableResponse(
      dependencies,
      session,
      dependencies.dbForEnv(env),
      projection,
    )

    if (unavailable !== null) {
      return unavailable
    }

    const message = yield* appendSiteBuilderMessage(
      dependencies.dbForEnv(env),
      {
        actorKind: 'customer' satisfies SiteBuilderActorKind,
        body: input.body,
        idempotencyKey,
        sessionId,
        visibility: 'customer',
      },
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (message instanceof Response) {
      return message
    }

    const nextProjection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (nextProjection instanceof Response) {
      return nextProjection
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          message,
          siteBuilderSession: nextProjection.public,
        },
        { status: 201 },
      ),
      session,
    )
  })

const appendBuilderEventResponse = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  sessionId: string,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    if (!adminSession(dependencies, session)) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionNotFound(),
        session,
      )
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionIdempotencyRequired(),
        session,
      )
    }

    const input = yield* readBuilderRequestBody(
      request,
      AppendSiteBuilderEventApiRequest,
    ).pipe(
      Effect.catch(() =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            invalidBuilderSessionRequest(
              'Request body does not match the Site builder event schema.',
            ),
            session,
          ),
        ),
      ),
    )

    if (input instanceof Response) {
      return input
    }

    const event = yield* appendSiteBuilderEvent(dependencies.dbForEnv(env), {
      eventKind: input.eventKind,
      idempotencyKey,
      payload: input.payload,
      phaseKind: input.phaseKind,
      sessionId,
      sourceRef: input.sourceRef,
      status: input.status,
      summary: input.summary,
      title: input.title,
      visibility: input.visibility ?? 'operator',
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (event instanceof Response) {
      return event
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          event,
          operatorSiteBuilderSession: projection.operator,
        },
        { status: 201 },
      ),
      session,
    )
  })

const saveBuilderVersionResponse = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  sessionId: string,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireRouteSession(dependencies, request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    if (!adminSession(dependencies, session)) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionNotFound(),
        session,
      )
    }

    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        builderSessionIdempotencyRequired(),
        session,
      )
    }

    const input = yield* readBuilderRequestBody(
      request,
      SaveSiteBuilderVersionApiRequest,
    ).pipe(
      Effect.catch(() =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            invalidBuilderSessionRequest(
              'Request body does not match the Site builder save-version schema.',
            ),
            session,
          ),
        ),
      ),
    )

    if (input instanceof Response) {
      return input
    }

    const result = yield* saveSiteBuilderVersion(
      dependencies.dbForEnv(env),
      dependencies.artifactsForEnv(env),
      {
        ...input,
        actorUserId: session.user.userId,
        idempotencyKey,
        sessionId,
      },
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (result instanceof Response) {
      return result
    }

    const projection = yield* builderSessionProjection(
      dependencies.dbForEnv(env),
      sessionId,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed(
          dependencies.appendRefreshedSessionCookies(
            siteBuilderErrorResponse(error),
            session,
          ),
        ),
      ),
    )

    if (projection instanceof Response) {
      return projection
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          savedVersion: result.savedVersion,
          siteBuilderSession: projection.public,
          siteVersion: result.version,
        },
        { status: result.version === null ? 200 : 201 },
      ),
      session,
    )
  })

const agentSiteResponse = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
  contract: AgentSiteContract,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  siteId?: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  let actor: AgentSiteContractActor | undefined

  if (
    request.headers.get(AgentSitesInternalGateHeader) ===
    AgentSitesInternalGateValue
  ) {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireBrowserSession(request, env, ctx),
      catch: error =>
        new SiteBuilderSessionStorageError({
          operation: 'agentSiteResponse.requireBrowserSession',
          reason: error instanceof Error ? error.message : String(error),
        }),
    }).pipe(Effect.catch(() => Effect.void))

    if (session === undefined) {
      return withAgentRateLimitHeaders(
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      )
    }

    actor = {
      appendCookies: response =>
        dependencies.appendRefreshedSessionCookies(response, session),
      actorRef: `user:${session.user.userId}`,
      ownerUserId: session.user.userId,
      scopeSatisfiedBy: 'browser_session_plus_internal_preview_gate',
      userId: session.user.userId,
    }
  } else {
    const bearerToken = readBearerToken(request)

    if (bearerToken === undefined) {
      return forbiddenGateResponse()
    }

    const session = yield* Effect.tryPromise({
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStoreForEnv?.(env) ??
            makeD1AgentRegistrationStore(dependencies.dbForEnv(env)),
          bearerToken,
        ),
      catch: error =>
        new SiteBuilderSessionStorageError({
          operation: 'agentSiteResponse.authenticateProgrammaticAgent',
          reason: error instanceof Error ? error.message : String(error),
        }),
    }).pipe(Effect.catch(() => Effect.void))

    if (session === undefined) {
      return withAgentRateLimitHeaders(
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      )
    }

    const grant = agentSiteGrantsFromSession(session).find(grant =>
      agentSiteGrantAllows(grant, {
        nowIso: currentIsoTimestamp(),
        requiredScope: contract.requiredScope,
        siteId,
      }),
    )

    if (grant === undefined) {
      return forbiddenScopeResponse(contract.requiredScope)
    }

    actor = {
      appendCookies: response => response,
      actorRef: `agent:${session.user.id}`,
      ownerUserId: grant.ownerUserId ?? session.user.id,
      scopeSatisfiedBy: 'registered_agent_token_with_agent_site_grant',
      userId: session.user.id,
    }
  }

  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey === undefined) {
    return actor.appendCookies(
      withAgentRateLimitHeaders(
        noStoreJsonResponse(
          {
            error: 'idempotency_key_required',
            message: 'Mutating agent Site actions require Idempotency-Key.',
          },
          { status: 400 },
        ),
      ),
    )
  }

  const body = yield* Effect.tryPromise({
    try: () => readRequestBody(request, contract.bodySchema),
    catch: () =>
      new SiteBuilderSessionValidationError({
        reason: 'Request body does not match the agent Site action schema.',
      }),
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        actor.appendCookies(
          withAgentRateLimitHeaders(
            noStoreJsonResponse(
              {
                error: 'invalid_request_body',
                message:
                  'Request body does not match the agent Site action schema.',
              },
              { status: 400 },
            ),
          ),
        ),
      ),
    ),
  )

  if (body instanceof Response) {
    return body
  }

  const execute = (() => {
    if (contract.action === 'project_create') {
      return agentSiteProjectCreateResponse(
        dependencies,
        actor,
        env,
        body as CreateAgentSiteRequest,
        idempotencyKey,
        contract.requiredScope,
      )
    }

    if (siteId === undefined) {
      return Effect.succeed(
        contractResponse(actor, {
          action: contract.action,
          idempotencyKey,
          requiredScope: contract.requiredScope,
        }),
      )
    }

    switch (contract.action) {
      case 'builder_session_open':
        return agentSiteBuilderSessionCreateResponse(
          dependencies,
          actor,
          env,
          body as OpenAgentSiteBuilderSessionRequest,
          idempotencyKey,
          contract.requiredScope,
          siteId,
        )
      case 'preview_request':
        return agentSitePreviewRequestResponse(
          dependencies,
          actor,
          env,
          body as RequestAgentSitePreviewRequest,
          idempotencyKey,
          contract.requiredScope,
          siteId,
        )
      case 'save_version':
        return agentSiteSaveVersionResponse(
          dependencies,
          actor,
          env,
          body as SaveAgentSiteVersionRequest,
          idempotencyKey,
          contract.requiredScope,
          siteId,
        )
      case 'deploy_version_request':
        return agentSiteDeployRequestResponse(
          dependencies,
          actor,
          env,
          body as RequestAgentSiteDeployRequest,
          idempotencyKey,
          contract.requiredScope,
          siteId,
        )
      default:
        return Effect.succeed(
          contractResponse(actor, {
            action: contract.action,
            idempotencyKey,
            requiredScope: contract.requiredScope,
            siteId,
          }),
        )
    }
  })()

  const executed = yield* execute.pipe(
    Effect.catch(error =>
      Effect.succeed(
        error instanceof SiteBuilderSessionStorageError ||
          error instanceof SiteBuilderSessionValidationError
          ? siteBuilderErrorResponse(error)
          : siteLifecycleErrorResponse(error as AutopilotSiteError),
      ),
    ),
  )

  return actor.appendCookies(withAgentRateLimitHeaders(executed))

  })

export const makeAgentSiteRoutes = <Session extends AgentSiteSession, Bindings>(
  dependencies: AgentSiteRouteDependencies<Session, Bindings>,
) => ({
  routeAgentSiteRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/agent/sites') {
      return agentSiteResponse(
        dependencies,
        {
          action: 'project_create',
          bodySchema: CreateAgentSiteRequest,
          requiredScope: 'sites:project:create',
        },
        request,
        env,
        ctx,
      )
    }

    if (url.pathname === '/api/sites') {
      return listSiteLibraryResponse(dependencies, request, env, ctx, url)
    }

    const siteLibraryActionMatch =
      /^\/api\/sites\/([^/]+)\/(access|archive|delete)$/.exec(url.pathname)

    if (siteLibraryActionMatch !== null) {
      const siteId = decodeURIComponent(siteLibraryActionMatch[1] ?? '')
      const action = siteLibraryActionMatch[2]

      return action === 'access'
        ? updateSiteLibraryAccessResponse(
            dependencies,
            request,
            env,
            ctx,
            siteId,
          )
        : mutateSiteLibraryLifecycleResponse(
            dependencies,
            request,
            env,
            ctx,
            siteId,
            action === 'archive' ? 'archive' : 'delete',
          )
    }

    if (url.pathname === '/api/sites/builder-sessions') {
      return createBuilderSessionResponse(dependencies, request, env, ctx)
    }

    const builderSessionMatch =
      /^\/api\/sites\/builder-sessions\/([^/]+)$/.exec(url.pathname)

    if (builderSessionMatch !== null) {
      return readBuilderSessionResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(builderSessionMatch[1] ?? ''),
      )
    }

    const builderMessageMatch =
      /^\/api\/sites\/builder-sessions\/([^/]+)\/messages$/.exec(url.pathname)

    if (builderMessageMatch !== null) {
      return appendBuilderMessageResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(builderMessageMatch[1] ?? ''),
      )
    }

    const builderEventsMatch =
      /^\/api\/sites\/builder-sessions\/([^/]+)\/events$/.exec(url.pathname)

    if (builderEventsMatch !== null) {
      return streamBuilderEventsResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(builderEventsMatch[1] ?? ''),
        url,
      )
    }

    const builderFilesMatch =
      /^\/api\/sites\/builder-sessions\/([^/]+)\/files$/.exec(url.pathname)

    if (builderFilesMatch !== null) {
      return listBuilderFilesResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(builderFilesMatch[1] ?? ''),
        'list',
      )
    }

    const builderFileTreeMatch =
      /^\/api\/sites\/builder-sessions\/([^/]+)\/files\/tree$/.exec(
        url.pathname,
      )

    if (builderFileTreeMatch !== null) {
      return listBuilderFilesResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(builderFileTreeMatch[1] ?? ''),
        'tree',
      )
    }

    const builderFileReadMatch =
      /^\/api\/sites\/builder-sessions\/([^/]+)\/files\/read$/.exec(
        url.pathname,
      )

    if (builderFileReadMatch !== null) {
      return readBuilderFileResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(builderFileReadMatch[1] ?? ''),
        url,
      )
    }

    const builderFileExportMatch =
      /^\/api\/sites\/builder-sessions\/([^/]+)\/files\/export$/.exec(
        url.pathname,
      )

    if (builderFileExportMatch !== null) {
      return listBuilderFilesResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(builderFileExportMatch[1] ?? ''),
        'export',
      )
    }

    const operatorBuilderEventMatch =
      /^\/api\/operator\/sites\/builder-sessions\/([^/]+)\/events$/.exec(
        url.pathname,
      )

    if (operatorBuilderEventMatch !== null) {
      return appendBuilderEventResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(operatorBuilderEventMatch[1] ?? ''),
      )
    }

    const operatorBuilderVersionMatch =
      /^\/api\/operator\/sites\/builder-sessions\/([^/]+)\/versions$/.exec(
        url.pathname,
      )

    if (operatorBuilderVersionMatch !== null) {
      return saveBuilderVersionResponse(
        dependencies,
        request,
        env,
        ctx,
        decodeURIComponent(operatorBuilderVersionMatch[1] ?? ''),
      )
    }

    const match = /^\/api\/agent\/sites\/([^/]+)\/([^/]+)$/.exec(url.pathname)

    if (match === null) {
      return undefined
    }

    const siteId = decodeURIComponent(match[1] ?? '')
    const actionPath = match[2]
    const contracts: Readonly<Record<string, AgentSiteContract>> = {
      'builder-sessions': {
        action: 'builder_session_open',
        bodySchema: OpenAgentSiteBuilderSessionRequest,
        requiredScope: 'sites:builder-session:create',
      },
      'deploy-requests': {
        action: 'deploy_version_request',
        bodySchema: RequestAgentSiteDeployRequest,
        requiredScope: 'sites:deploy:request',
      },
      previews: {
        action: 'preview_request',
        bodySchema: RequestAgentSitePreviewRequest,
        requiredScope: 'sites:preview:request',
      },
      versions: {
        action: 'save_version',
        bodySchema: SaveAgentSiteVersionRequest,
        requiredScope: 'sites:version:save',
      },
    }
    const contract =
      actionPath === undefined ? undefined : contracts[actionPath]

    if (contract === undefined) {
      return undefined
    }

    return agentSiteResponse(dependencies, contract, request, env, ctx, siteId)
  },
})
