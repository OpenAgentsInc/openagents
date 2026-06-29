import {
  ForgeCoordinationChangeState,
  ForgeCoordinationIssueState,
  ForgeCoordinationStatusState,
  ForgeGitHubMirrorReceipt,
  ForgeMergeQueueLedgerState,
  ForgePromotionDecisionReceipt,
  ForgeVerificationReceipt,
  decodeForgeControlPlaneScope,
  type ForgeControlPlaneScope,
  type ForgeGitHubMirrorStatus,
} from '@openagentsinc/forge-protocol'
import { Effect, Schema as S } from 'effect'

import { timingSafeEqual } from './agent-registration'
import {
  type ForgeCoordinationStore,
} from './forge-coordination-store'
import type {
  ForgeGitCanonicalStore,
} from './forge-git-canonical-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

const FORGE_GIT_TOKEN_PREFIX = 'oa_forge_git_'
const OPENAGENTS_FORGE_TENANT_REF = 'tenant.openagents'
const OPENAGENTS_FORGE_REPOSITORY_REF = 'repo.openagents.openagents'
const OPENAGENTS_GITHUB_OWNER = 'OpenAgentsInc'
const OPENAGENTS_GITHUB_REPO = 'openagents'
const OPENAGENTS_DEFAULT_BRANCH_REF = 'refs/heads/main'

export type ForgeControlPlaneAuth = Readonly<{
  mode: 'admin' | 'control_plane'
  subjectRef: string
  scopes: ReadonlyArray<ForgeControlPlaneScope>
  tenantRef: string | null
}>

export type ForgeControlPlaneAuthorize<Bindings> = (
  request: Request,
  env: Bindings,
  requiredScope: ForgeControlPlaneScope,
) => Promise<ForgeControlPlaneAuth | undefined>

type ForgeControlPlaneRouteDependencies<Bindings> = Readonly<{
  authorizeControlPlaneBearer?: ForgeControlPlaneAuthorize<Bindings> | undefined
  fetch?: typeof fetch
  githubMirrorRefUpdater?: ForgeGitHubMirrorRefUpdater | undefined
  makeCanonicalStore: (env: Bindings) => ForgeGitCanonicalStore
  makeStore: (env: Bindings) => ForgeCoordinationStore
  nowIso?: () => string
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  resolveGitHubMirrorToken?: (env: Bindings) => string | undefined
}>

type ForgeGitHubMirrorRefUpdate = Readonly<{
  commitId: string
  destinationRef: string
  destinationRepository: string
  token: string
}>

type ForgeGitHubMirrorRefUpdateResult = Readonly<{
  status: 'mirrored' | 'already_mirrored' | 'failed'
  errorReason?: string | undefined
}>

type ForgeGitHubMirrorRefUpdater = (
  update: ForgeGitHubMirrorRefUpdate,
) => Promise<ForgeGitHubMirrorRefUpdateResult>

class ForgeControlPlaneHttpError extends Error {
  constructor(
    readonly status: number,
    readonly errorCode: string,
    readonly reason?: string,
    readonly headers?: HeadersInit,
  ) {
    super(reason ?? errorCode)
    this.name = 'ForgeControlPlaneHttpError'
  }
}

const ForgeWorkRecordRequest = S.Struct({
  tenantRef: S.String,
  issueRef: S.String,
  githubIssueNumber: S.optionalKey(S.NullOr(S.Number)),
  title: S.String,
  state: ForgeCoordinationIssueState,
  priorityRef: S.optionalKey(S.NullOr(S.String)),
  sourceRefs: S.optionalKey(S.Array(S.String)),
})

const ForgeChangeRecordRequest = S.Struct({
  tenantRef: S.String,
  prRef: S.String,
  issueRef: S.String,
  changeRef: S.String,
  state: ForgeCoordinationChangeState,
  baseHead: S.String,
  patchHead: S.String,
  verificationRef: S.optionalKey(S.NullOr(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  sourceRefs: S.optionalKey(S.Array(S.String)),
})

const ForgeStatusTransitionRequest = S.Struct({
  tenantRef: S.String,
  statusRef: S.String,
  state: ForgeCoordinationStatusState,
  actorRef: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
})

const ForgeDispatchLeaseRequest = S.Struct({
  tenantRef: S.String,
  leaseRef: S.String,
  workRef: S.String,
  ownerAgentRef: S.String,
  idempotencyKeyHash: S.optionalKey(S.NullOr(S.String)),
  acquiredAt: S.optionalKey(S.String),
  expiresAt: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
})

const ForgeMergeQueueSnapshotRequest = S.Struct({
  tenantRef: S.String,
  queueRef: S.String,
  baseHead: S.String,
  actualHead: S.String,
  virtualHead: S.String,
  state: ForgeMergeQueueLedgerState,
  nextPromotionRef: S.optionalKey(S.NullOr(S.String)),
  ready: S.Unknown,
  blocked: S.Unknown,
  sourceRefs: S.optionalKey(S.Array(S.String)),
})

const ForgeOpenAgentsImportRequest = S.Struct({
  tenantRef: S.String,
  repositoryRef: S.String,
})

const ForgeGitHubMirrorRequest = S.Struct({
  tenantRef: S.String,
  mirrorRef: S.String,
  promotionRef: S.String,
  sourceCanonicalRef: S.String,
  destinationRepository: S.String,
  destinationRef: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
})

const readBearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  if (scheme?.toLowerCase() !== 'bearer' || token === undefined) {
    return undefined
  }

  return token
}

const readForgeScopeHeader = (
  request: Request,
): ReadonlyArray<ForgeControlPlaneScope> | undefined => {
  const raw =
    request.headers.get('x-openagents-forge-scopes') ??
    request.headers.get('x-openagents-forge-control-plane-scopes')

  if (raw === null || raw.trim() === '') {
    return []
  }

  try {
    return raw
      .split(/[,\s]+/u)
      .map(scope => scope.trim())
      .filter(scope => scope !== '')
      .map(scope => decodeForgeControlPlaneScope(scope))
  } catch {
    return undefined
  }
}

const readForgeTenantHeader = (request: Request): string | undefined => {
  const raw =
    request.headers.get('x-openagents-forge-tenant-ref') ??
    request.headers.get('x-openagents-forge-tenant')

  return raw === null || raw.trim() === '' ? undefined : raw.trim()
}

export const authorizeForgeControlPlaneBearer = async (
  request: Request,
  expectedToken: string | undefined,
  requiredScope: ForgeControlPlaneScope,
): Promise<ForgeControlPlaneAuth | undefined> => {
  const actual = readBearerToken(request)

  if (
    expectedToken === undefined ||
    expectedToken.trim() === '' ||
    actual === undefined ||
    !(await timingSafeEqual(actual, expectedToken))
  ) {
    return undefined
  }

  const scopes = readForgeScopeHeader(request)
  const tenantRef = readForgeTenantHeader(request)

  if (
    scopes === undefined ||
    tenantRef === undefined ||
    (!scopes.includes(requiredScope) && !scopes.includes('forge:admin'))
  ) {
    return undefined
  }

  return {
    mode: 'control_plane',
    scopes,
    subjectRef: 'forge.control-plane.service',
    tenantRef,
  }
}

const routeErrorResponse = (error: unknown) => {
  if (error instanceof ForgeControlPlaneHttpError) {
    return noStoreJsonResponse(
      {
        error: error.errorCode,
        ...(error.reason === undefined ? {} : { reason: error.reason }),
      },
      {
        ...(error.headers === undefined ? {} : { headers: error.headers }),
        status: error.status,
      },
    )
  }

  return noStoreJsonResponse(
    { error: 'forge_control_plane_storage_error' },
    { status: 500 },
  )
}

const routeEffect = (
  run: () => Promise<ReturnType<typeof routeErrorResponse>>,
) =>
  Effect.promise(async () => {
    try {
      return await run()
    } catch (error) {
      return routeErrorResponse(error)
    }
  })

const decodeBody = async <A>(
  request: Request,
  schema: S.Decoder<A>,
): Promise<A> => {
  try {
    return decodeUnknownWithSchema(schema, await readJsonObject(request))
  } catch (error) {
    throw new ForgeControlPlaneHttpError(
      400,
      'forge_control_plane_bad_request',
      error instanceof Error ? error.message : String(error),
    )
  }
}

const tenantRefFromQuery = (url: URL): string => {
  const tenantRef = url.searchParams.get('tenantRef')

  if (tenantRef === null || tenantRef.trim() === '') {
    throw new ForgeControlPlaneHttpError(
      400,
      'forge_control_plane_tenant_ref_required',
    )
  }

  return tenantRef.trim()
}

const optionalQuery = (url: URL, name: string): string | undefined => {
  const value = url.searchParams.get(name)
  return value === null || value.trim() === '' ? undefined : value.trim()
}

const limitFromQuery = (url: URL): number => {
  const raw = url.searchParams.get('limit')
  const parsed = raw === null ? 50 : Number(raw)

  if (!Number.isInteger(parsed)) {
    return 50
  }

  return Math.min(Math.max(parsed, 1), 100)
}

const requireScope = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  requiredScope: ForgeControlPlaneScope,
  tenantRef: string,
): Promise<ForgeControlPlaneAuth> => {
  const token = readBearerToken(request)

  if (token?.startsWith(FORGE_GIT_TOKEN_PREFIX) === true) {
    throw new ForgeControlPlaneHttpError(
      403,
      'forge_control_plane_git_token_rejected',
      'Forge smart-Git tokens are valid only for Git intake, not /api/forge control-plane routes.',
    )
  }

  if ((await dependencies.requireAdminApiToken?.(request, env)) === true) {
    return { mode: 'admin', scopes: ['forge:admin'], subjectRef: 'admin', tenantRef: null }
  }

  const controlPlaneAuth =
    await dependencies.authorizeControlPlaneBearer?.(
      request,
      env,
      requiredScope,
    )

  if (controlPlaneAuth !== undefined) {
    if (
      controlPlaneAuth.tenantRef !== null &&
      controlPlaneAuth.tenantRef !== tenantRef
    ) {
      throw new ForgeControlPlaneHttpError(
        403,
        'forge_control_plane_wrong_tenant',
        'Forge control-plane tokens are scoped to one tenant and cannot read or mutate another tenant.',
      )
    }

    return controlPlaneAuth
  }

  if (token === undefined) {
    throw new ForgeControlPlaneHttpError(
      401,
      'forge_control_plane_unauthorized',
      undefined,
      { 'www-authenticate': 'Bearer realm="OpenAgents Forge control plane"' },
    )
  }

  throw new ForgeControlPlaneHttpError(
    403,
    'forge_control_plane_forbidden',
    `Missing required Forge control-plane scope: ${requiredScope}.`,
  )
}

const routeNowIso = <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeWorkRecords = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  const store = dependencies.makeStore(env)

  if (request.method === 'GET') {
    const tenantRef = tenantRefFromQuery(url)
    await requireScope(dependencies, request, env, 'forge:work:read', tenantRef)
    return noStoreJsonResponse({
      limit: limitFromQuery(url),
      tenantRef,
      workRecords: await store.listIssues(tenantRef, limitFromQuery(url)),
    })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const body = await decodeBody(request, ForgeWorkRecordRequest)
  await requireScope(dependencies, request, env, 'forge:work:write', body.tenantRef)
  const workRecord = await store.upsertIssue({
    tenantRef: body.tenantRef,
    issueRef: body.issueRef,
    title: body.title,
    state: body.state,
    sourceRefs: body.sourceRefs ?? [],
    nowIso: routeNowIso(dependencies),
    ...(body.githubIssueNumber === undefined
      ? {}
      : { githubIssueNumber: body.githubIssueNumber }),
    ...(body.priorityRef === undefined ? {} : { priorityRef: body.priorityRef }),
  })

  return noStoreJsonResponse({ workRecord }, { status: 201 })
}

const routeChanges = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  const store = dependencies.makeStore(env)

  if (request.method === 'GET') {
    const tenantRef = tenantRefFromQuery(url)
    await requireScope(dependencies, request, env, 'forge:change:read', tenantRef)
    const limit = limitFromQuery(url)
    return noStoreJsonResponse({
      changes: await store.listChanges(
        tenantRef,
        limit,
        optionalQuery(url, 'issueRef'),
      ),
      limit,
      tenantRef,
    })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const body = await decodeBody(request, ForgeChangeRecordRequest)
  await requireScope(dependencies, request, env, 'forge:change:write', body.tenantRef)
  const change = await store.upsertChange({
    tenantRef: body.tenantRef,
    prRef: body.prRef,
    issueRef: body.issueRef,
    changeRef: body.changeRef,
    state: body.state,
    baseHead: body.baseHead,
    patchHead: body.patchHead,
    blockerRefs: body.blockerRefs ?? [],
    sourceRefs: body.sourceRefs ?? [],
    nowIso: routeNowIso(dependencies),
    ...(body.verificationRef === undefined
      ? {}
      : { verificationRef: body.verificationRef }),
  })

  return noStoreJsonResponse({ change }, { status: 201 })
}

const routeChangeStatus = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  changeRef: string,
) => {
  if (request.method !== 'PATCH') {
    return methodNotAllowed(['PATCH'])
  }

  const body = await decodeBody(request, ForgeStatusTransitionRequest)
  await requireScope(dependencies, request, env, 'forge:status:write', body.tenantRef)
  const status = await dependencies.makeStore(env).recordStatus({
    tenantRef: body.tenantRef,
    statusRef: body.statusRef,
    subjectRef: decodeURIComponent(changeRef),
    state: body.state,
    actorRef: body.actorRef,
    sourceRefs: body.sourceRefs ?? [],
    createdAt: routeNowIso(dependencies),
  })

  return noStoreJsonResponse({ status }, { status: 201 })
}

const routeStatuses = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const tenantRef = tenantRefFromQuery(url)
  await requireScope(dependencies, request, env, 'forge:change:read', tenantRef)
  const limit = limitFromQuery(url)

  return noStoreJsonResponse({
    limit,
    statuses: await dependencies
      .makeStore(env)
      .listStatuses(tenantRef, limit, optionalQuery(url, 'subjectRef')),
    tenantRef,
  })
}

const routeLeases = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  const store = dependencies.makeStore(env)

  if (request.method === 'GET') {
    const tenantRef = tenantRefFromQuery(url)
    await requireScope(dependencies, request, env, 'forge:lease:write', tenantRef)
    const limit = limitFromQuery(url)
    return noStoreJsonResponse({
      leases: await store.listDispatchLeases(
        tenantRef,
        limit,
        optionalQuery(url, 'workRef'),
      ),
      limit,
      tenantRef,
    })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const body = await decodeBody(request, ForgeDispatchLeaseRequest)
  await requireScope(dependencies, request, env, 'forge:lease:write', body.tenantRef)
  const result = await store.acquireDispatchLease({
    tenantRef: body.tenantRef,
    leaseRef: body.leaseRef,
    workRef: body.workRef,
    ownerAgentRef: body.ownerAgentRef,
    acquiredAt: body.acquiredAt ?? routeNowIso(dependencies),
    expiresAt: body.expiresAt,
    sourceRefs: body.sourceRefs ?? [],
    ...(body.idempotencyKeyHash === undefined
      ? {}
      : { idempotencyKeyHash: body.idempotencyKeyHash }),
  })

  return noStoreJsonResponse(result, { status: result.acquired ? 201 : 409 })
}

const routeQueue = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const tenantRef = tenantRefFromQuery(url)
  await requireScope(dependencies, request, env, 'forge:queue:read', tenantRef)
  const limit = limitFromQuery(url)
  const store = dependencies.makeStore(env)

  return noStoreJsonResponse({
    latest: await store.readLatestMergeQueueLedger(tenantRef),
    limit,
    queueSnapshots: await store.listMergeQueueLedgers(tenantRef, limit),
    tenantRef,
  })
}

const routeQueueSnapshots = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const body = await decodeBody(request, ForgeMergeQueueSnapshotRequest)
  await requireScope(dependencies, request, env, 'forge:queue:write', body.tenantRef)
  const queueSnapshot = await dependencies.makeStore(env).recordMergeQueueLedger({
    tenantRef: body.tenantRef,
    queueRef: body.queueRef,
    baseHead: body.baseHead,
    actualHead: body.actualHead,
    virtualHead: body.virtualHead,
    state: body.state,
    ready: body.ready,
    blocked: body.blocked,
    sourceRefs: body.sourceRefs ?? [],
    nowIso: routeNowIso(dependencies),
    ...(body.nextPromotionRef === undefined
      ? {}
      : { nextPromotionRef: body.nextPromotionRef }),
  })

  return noStoreJsonResponse({ queueSnapshot }, { status: 201 })
}

const routeVerificationReceipts = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  const store = dependencies.makeStore(env)

  if (request.method === 'GET') {
    const tenantRef = tenantRefFromQuery(url)
    await requireScope(dependencies, request, env, 'forge:change:read', tenantRef)
    const limit = limitFromQuery(url)
    return noStoreJsonResponse({
      limit,
      tenantRef,
      verificationReceipts: await store.listVerificationReceipts(
        tenantRef,
        limit,
        optionalQuery(url, 'changeRef'),
      ),
    })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const receipt = await decodeBody(request, ForgeVerificationReceipt)
  await requireScope(dependencies, request, env, 'forge:receipt:write', receipt.tenant_ref)
  const verificationReceipt = await store.recordVerificationReceipt(
    receipt,
    routeNowIso(dependencies),
  )

  return noStoreJsonResponse({ verificationReceipt }, { status: 201 })
}

const routePromotionDecisions = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  const store = dependencies.makeStore(env)

  if (request.method === 'GET') {
    const tenantRef = tenantRefFromQuery(url)
    await requireScope(dependencies, request, env, 'forge:queue:read', tenantRef)
    const limit = limitFromQuery(url)
    return noStoreJsonResponse({
      limit,
      promotionDecisions: await store.listPromotionDecisionReceipts(
        tenantRef,
        limit,
        optionalQuery(url, 'changeRef'),
      ),
      tenantRef,
    })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const receipt = await decodeBody(request, ForgePromotionDecisionReceipt)
  await requireScope(
    dependencies,
    request,
    env,
    'forge:promotion:decide',
    receipt.tenant_ref,
  )
  const promotionDecision = await store.recordPromotionDecisionReceipt(
    receipt,
    routeNowIso(dependencies),
  )

  return noStoreJsonResponse({ promotionDecision }, { status: 201 })
}

const mirrorReceipt = (
  body: typeof ForgeGitHubMirrorRequest.Type,
  fields: Readonly<{
    attemptedAt: string
    commitId: string
    errorReason?: string | null
    mirroredAt?: string | null
    refusalReason?: string | null
    status: ForgeGitHubMirrorStatus
  }>,
): typeof ForgeGitHubMirrorReceipt.Type => ({
  schema: 'openagents.forge.github.mirror.receipt.v0.1',
  tenant_ref: body.tenantRef,
  mirror_ref: body.mirrorRef,
  promotion_ref: body.promotionRef,
  source_canonical_ref: body.sourceCanonicalRef,
  destination_repository: body.destinationRepository,
  destination_ref: body.destinationRef,
  commit_id: fields.commitId,
  status: fields.status,
  attempted_at: fields.attemptedAt,
  mirrored_at: fields.mirroredAt ?? null,
  refusal_reason: fields.refusalReason ?? null,
  error_reason: fields.errorReason ?? null,
  source_refs: body.sourceRefs ?? [],
  redacted: true,
})

const githubBranchNameFromRef = (destinationRef: string): string | undefined => {
  const trimmed = destinationRef.trim()
  if (!trimmed.startsWith('refs/heads/')) {
    return undefined
  }
  const branch = trimmed.slice('refs/heads/'.length)
  return branch === '' || branch.includes('..') ? undefined : branch
}

const defaultGitHubMirrorRefUpdater =
  (fetchImpl: typeof fetch = fetch): ForgeGitHubMirrorRefUpdater =>
  async update => {
    const branch = githubBranchNameFromRef(update.destinationRef)
    if (branch === undefined) {
      return {
        errorReason: 'destination_ref_must_be_branch_ref',
        status: 'failed',
      }
    }

    const refUrl = `https://api.github.com/repos/${update.destinationRepository}/git/ref/heads/${encodeURIComponent(branch)}`
    const headers = {
      accept: 'application/vnd.github+json',
      authorization: `Bearer ${update.token}`,
      'content-type': 'application/json',
      'user-agent': 'openagents-forge-github-mirror',
    }
    const currentResponse = await fetchImpl(refUrl, { headers })
    if (currentResponse.ok) {
      const current = (await currentResponse.json()) as {
        object?: { sha?: unknown }
      }
      if (
        typeof current.object?.sha === 'string' &&
        current.object.sha.toLowerCase() === update.commitId.toLowerCase()
      ) {
        return { status: 'already_mirrored' }
      }
    }

    const response = await fetchImpl(refUrl, {
      body: JSON.stringify({ force: false, sha: update.commitId }),
      headers,
      method: 'PATCH',
    })

    return response.ok
      ? { status: 'mirrored' }
      : {
          errorReason: `github_ref_update_http_${response.status}`,
          status: 'failed',
        }
  }

const routeGitHubMirrors = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  const store = dependencies.makeStore(env)

  if (request.method === 'GET') {
    const tenantRef = tenantRefFromQuery(url)
    await requireScope(dependencies, request, env, 'forge:mirror:read', tenantRef)
    const limit = limitFromQuery(url)
    return noStoreJsonResponse({
      githubMirrorReceipts: await store.listGitHubMirrorReceipts(
        tenantRef,
        limit,
        optionalQuery(url, 'promotionRef'),
      ),
      limit,
      tenantRef,
    })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const body = await decodeBody(request, ForgeGitHubMirrorRequest)
  await requireScope(dependencies, request, env, 'forge:mirror:write', body.tenantRef)
  const now = routeNowIso(dependencies)
  const existing = await store.readGitHubMirrorReceipt(body.tenantRef, body.mirrorRef)
  if (
    existing?.status === 'mirrored' ||
    existing?.status === 'already_mirrored'
  ) {
    return noStoreJsonResponse(
      { githubMirrorReceipt: existing, idempotent: true },
      { status: 200 },
    )
  }

  const promotion = await store.readPromotionDecisionReceipt(
    body.tenantRef,
    body.promotionRef,
  )
  if (promotion === undefined) {
    const githubMirrorReceipt = await store.recordGitHubMirrorReceipt(
      mirrorReceipt(body, {
        attemptedAt: now,
        commitId: 'unknown',
        refusalReason: 'promotion_receipt_missing',
        status: 'refused',
      }),
      now,
    )
    return noStoreJsonResponse({ githubMirrorReceipt }, { status: 201 })
  }

  if (promotion.decision !== 'approved' || promotion.promoted_head === null) {
    const githubMirrorReceipt = await store.recordGitHubMirrorReceipt(
      mirrorReceipt(body, {
        attemptedAt: now,
        commitId: promotion.promoted_head ?? promotion.candidate_head,
        refusalReason: `promotion_${promotion.decision}`,
        status: 'refused',
      }),
      now,
    )
    return noStoreJsonResponse({ githubMirrorReceipt }, { status: 201 })
  }

  const token = dependencies.resolveGitHubMirrorToken?.(env)?.trim()
  if (token === undefined || token === '') {
    const githubMirrorReceipt = await store.recordGitHubMirrorReceipt(
      mirrorReceipt(body, {
        attemptedAt: now,
        commitId: promotion.promoted_head,
        errorReason: 'github_mirror_token_unconfigured',
        status: 'failed',
      }),
      now,
    )
    return noStoreJsonResponse({ githubMirrorReceipt }, { status: 201 })
  }

  const result = await (
    dependencies.githubMirrorRefUpdater ??
    defaultGitHubMirrorRefUpdater(dependencies.fetch)
  )({
    commitId: promotion.promoted_head,
    destinationRef: body.destinationRef,
    destinationRepository: body.destinationRepository,
    token,
  })
  const githubMirrorReceipt = await store.recordGitHubMirrorReceipt(
    mirrorReceipt(body, {
      attemptedAt: now,
      commitId: promotion.promoted_head,
      errorReason: result.errorReason ?? null,
      mirroredAt: result.status === 'failed' ? null : now,
      status: result.status,
    }),
    now,
  )

  return noStoreJsonResponse({ githubMirrorReceipt }, { status: 201 })
}

const sha256Hex = async (value: string): Promise<string> => {
  const bytes = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )
  return [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const readGitHubMainTip = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
): Promise<Readonly<{ objectId: string; sourceUrl: string }>> => {
  const url = `https://api.github.com/repos/${OPENAGENTS_GITHUB_OWNER}/${OPENAGENTS_GITHUB_REPO}/git/ref/heads/main`
  const response = await (dependencies.fetch ?? fetch)(url, {
    headers: {
      accept: 'application/vnd.github+json',
      'user-agent': 'openagents-forge-import',
    },
  })
  if (!response.ok) {
    throw new ForgeControlPlaneHttpError(
      502,
      'forge_openagents_import_upstream_failed',
      `GitHub ref lookup failed with HTTP ${response.status}.`,
    )
  }
  const body = (await response.json()) as {
    object?: { sha?: unknown; url?: unknown }
  }
  const objectId =
    typeof body.object?.sha === 'string' ? body.object.sha.toLowerCase() : ''
  if (!/^[0-9a-f]{40}$/u.test(objectId)) {
    throw new ForgeControlPlaneHttpError(
      502,
      'forge_openagents_import_bad_upstream_ref',
      'GitHub main ref did not include a SHA-1 object id.',
    )
  }
  return {
    objectId,
    sourceUrl: typeof body.object?.url === 'string' ? body.object.url : url,
  }
}

const routeRefs = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }
  const tenantRef = tenantRefFromQuery(url)
  await requireScope(dependencies, request, env, 'forge:change:read', tenantRef)
  const repositoryRef = optionalQuery(url, 'repositoryRef')
  if (repositoryRef === undefined) {
    throw new ForgeControlPlaneHttpError(
      400,
      'forge_control_plane_repository_ref_required',
    )
  }
  const refs = await dependencies
    .makeCanonicalStore(env)
    .listRefs(tenantRef, repositoryRef, {
      limit: limitFromQuery(url),
      state: optionalQuery(url, 'state') === 'deleted' ? 'deleted' : 'active',
    })
  return noStoreJsonResponse({
    defaultBranch: refs.find(ref => ref.ref_name === OPENAGENTS_DEFAULT_BRANCH_REF),
    defaultBranchRef: OPENAGENTS_DEFAULT_BRANCH_REF,
    limit: limitFromQuery(url),
    refs,
    repository: {
      defaultBranchRef: OPENAGENTS_DEFAULT_BRANCH_REF,
      github: `${OPENAGENTS_GITHUB_OWNER}/${OPENAGENTS_GITHUB_REPO}`,
      repositoryRef,
      tenantRef,
    },
    repositoryRef,
    tenantRef,
  })
}

const routeOpenAgentsImport = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }
  await requireScope(dependencies, request, env, 'forge:admin', OPENAGENTS_FORGE_TENANT_REF)
  const body = await decodeBody(request, ForgeOpenAgentsImportRequest)
  if (
    body.tenantRef !== OPENAGENTS_FORGE_TENANT_REF ||
    body.repositoryRef !== OPENAGENTS_FORGE_REPOSITORY_REF
  ) {
    throw new ForgeControlPlaneHttpError(
      403,
      'forge_openagents_import_target_forbidden',
      `OpenAgents dogfood import is pinned to ${OPENAGENTS_FORGE_TENANT_REF}/${OPENAGENTS_FORGE_REPOSITORY_REF}.`,
    )
  }
  const tip = await readGitHubMainTip(dependencies)
  const sourceRefs = [
    'github:OpenAgentsInc/openagents',
    'github:OpenAgentsInc/openagents#6793',
    `github:OpenAgentsInc/openagents@${tip.objectId}`,
    'docs/forge/2026-06-28-forge-openagents-import-runbook.md',
  ]
  const digest = await sha256Hex(
    JSON.stringify({
      github: `${OPENAGENTS_GITHUB_OWNER}/${OPENAGENTS_GITHUB_REPO}`,
      objectId: tip.objectId,
      ref: OPENAGENTS_DEFAULT_BRANCH_REF,
      sourceUrl: tip.sourceUrl,
    }),
  )
  const shortTip = tip.objectId.slice(0, 16)
  const result = await dependencies.makeCanonicalStore(env).importExternalRef({
    changeRef: `change.forge.import.openagents.${shortTip}`,
    objectFormat: 'sha1',
    objectId: tip.objectId,
    packfileRef: `packfile.forge.github.openagents.main.${shortTip}`,
    receivePackRef: `receive-pack.forge.import.openagents.main.${shortTip}`,
    refName: OPENAGENTS_DEFAULT_BRANCH_REF,
    repositoryRef: body.repositoryRef,
    sourceDigestSha256: digest,
    sourceRefs,
    tenantRef: body.tenantRef,
    nowIso: routeNowIso(dependencies),
  })
  return noStoreJsonResponse(
    {
      changed: result.changed,
      defaultBranch: result.ref,
      import: {
        defaultBranchRef: OPENAGENTS_DEFAULT_BRANCH_REF,
        github: `${OPENAGENTS_GITHUB_OWNER}/${OPENAGENTS_GITHUB_REPO}`,
        objectId: tip.objectId,
        repositoryRef: body.repositoryRef,
        sourceRefs,
        tenantRef: body.tenantRef,
      },
      object: result.object,
    },
    { status: result.changed ? 201 : 200 },
  )
}

export const makeForgeControlPlaneRoutes = <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
) => ({
  routeForgeControlPlaneRequest(
    request: Request,
    env: Bindings,
  ) {
    const url = new URL(request.url)

    if (
      url.pathname !== '/api/forge' &&
      !url.pathname.startsWith('/api/forge/')
    ) {
      return undefined
    }

    const segments = url.pathname.split('/').filter(Boolean)
    const route = segments.slice(2)

    if (route.length === 1 && route[0] === 'work-records') {
      return routeEffect(() => routeWorkRecords(dependencies, request, env, url))
    }

    if (route.length === 1 && route[0] === 'changes') {
      return routeEffect(() => routeChanges(dependencies, request, env, url))
    }

    if (
      route.length === 3 &&
      route[0] === 'changes' &&
      route[2] === 'status'
    ) {
      return routeEffect(() =>
        routeChangeStatus(dependencies, request, env, route[1] ?? ''),
      )
    }

    if (route.length === 1 && route[0] === 'statuses') {
      return routeEffect(() => routeStatuses(dependencies, request, env, url))
    }

    if (route.length === 1 && route[0] === 'leases') {
      return routeEffect(() => routeLeases(dependencies, request, env, url))
    }

    if (route.length === 1 && route[0] === 'queue') {
      return routeEffect(() => routeQueue(dependencies, request, env, url))
    }

    if (route.length === 1 && route[0] === 'refs') {
      return routeEffect(() => routeRefs(dependencies, request, env, url))
    }

    if (route.length === 2 && route[0] === 'queue' && route[1] === 'snapshots') {
      return routeEffect(() => routeQueueSnapshots(dependencies, request, env))
    }

    if (route.length === 1 && route[0] === 'verification-receipts') {
      return routeEffect(() =>
        routeVerificationReceipts(dependencies, request, env, url),
      )
    }

    if (route.length === 1 && route[0] === 'promotion-decisions') {
      return routeEffect(() =>
        routePromotionDecisions(dependencies, request, env, url),
      )
    }

    if (route.length === 1 && route[0] === 'github-mirrors') {
      return routeEffect(() =>
        routeGitHubMirrors(dependencies, request, env, url),
      )
    }

    if (
      route.length === 2 &&
      route[0] === 'admin' &&
      route[1] === 'import-openagents'
    ) {
      return routeEffect(() =>
        routeOpenAgentsImport(dependencies, request, env),
      )
    }

    return Effect.succeed(
      noStoreJsonResponse(
        { error: 'forge_control_plane_route_not_found' },
        { status: 404 },
      ),
    )
  },
})
