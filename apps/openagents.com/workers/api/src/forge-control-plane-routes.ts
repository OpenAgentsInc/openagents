import {
  ForgeCoordinationChangeState,
  ForgeCoordinationIssueState,
  ForgeCoordinationStatusState,
  ForgeMergeQueueLedgerState,
  ForgePromotionDecisionReceipt,
  ForgeVerificationReceipt,
  decodeForgeControlPlaneScope,
  type ForgeControlPlaneScope,
} from '@openagentsinc/forge-protocol'
import { Effect, Schema as S } from 'effect'

import { timingSafeEqual } from './agent-registration'
import {
  type ForgeCoordinationStore,
} from './forge-coordination-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

const FORGE_GIT_TOKEN_PREFIX = 'oa_forge_git_'

export type ForgeControlPlaneAuth = Readonly<{
  mode: 'admin' | 'control_plane'
  subjectRef: string
  scopes: ReadonlyArray<ForgeControlPlaneScope>
}>

export type ForgeControlPlaneAuthorize<Bindings> = (
  request: Request,
  env: Bindings,
  requiredScope: ForgeControlPlaneScope,
) => Promise<ForgeControlPlaneAuth | undefined>

type ForgeControlPlaneRouteDependencies<Bindings> = Readonly<{
  authorizeControlPlaneBearer?: ForgeControlPlaneAuthorize<Bindings> | undefined
  makeStore: (env: Bindings) => ForgeCoordinationStore
  nowIso?: () => string
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
}>

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

  if (
    scopes === undefined ||
    (!scopes.includes(requiredScope) && !scopes.includes('forge:admin'))
  ) {
    return undefined
  }

  return {
    mode: 'control_plane',
    scopes,
    subjectRef: 'forge.control-plane.service',
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
    return { mode: 'admin', scopes: ['forge:admin'], subjectRef: 'admin' }
  }

  const controlPlaneAuth =
    await dependencies.authorizeControlPlaneBearer?.(
      request,
      env,
      requiredScope,
    )

  if (controlPlaneAuth !== undefined) {
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
    await requireScope(dependencies, request, env, 'forge:work:read')
    const tenantRef = tenantRefFromQuery(url)
    return noStoreJsonResponse({
      limit: limitFromQuery(url),
      tenantRef,
      workRecords: await store.listIssues(tenantRef, limitFromQuery(url)),
    })
  }

  if (request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  await requireScope(dependencies, request, env, 'forge:work:write')
  const body = await decodeBody(request, ForgeWorkRecordRequest)
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
    await requireScope(dependencies, request, env, 'forge:change:read')
    const tenantRef = tenantRefFromQuery(url)
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

  await requireScope(dependencies, request, env, 'forge:change:write')
  const body = await decodeBody(request, ForgeChangeRecordRequest)
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

  await requireScope(dependencies, request, env, 'forge:status:write')
  const body = await decodeBody(request, ForgeStatusTransitionRequest)
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

  await requireScope(dependencies, request, env, 'forge:change:read')
  const tenantRef = tenantRefFromQuery(url)
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
    await requireScope(dependencies, request, env, 'forge:lease:write')
    const tenantRef = tenantRefFromQuery(url)
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

  await requireScope(dependencies, request, env, 'forge:lease:write')
  const body = await decodeBody(request, ForgeDispatchLeaseRequest)
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

  await requireScope(dependencies, request, env, 'forge:queue:read')
  const tenantRef = tenantRefFromQuery(url)
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

  await requireScope(dependencies, request, env, 'forge:queue:write')
  const body = await decodeBody(request, ForgeMergeQueueSnapshotRequest)
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
    await requireScope(dependencies, request, env, 'forge:change:read')
    const tenantRef = tenantRefFromQuery(url)
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

  await requireScope(dependencies, request, env, 'forge:receipt:write')
  const receipt = await decodeBody(request, ForgeVerificationReceipt)
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
    await requireScope(dependencies, request, env, 'forge:queue:read')
    const tenantRef = tenantRefFromQuery(url)
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

  await requireScope(dependencies, request, env, 'forge:promotion:decide')
  const receipt = await decodeBody(request, ForgePromotionDecisionReceipt)
  const promotionDecision = await store.recordPromotionDecisionReceipt(
    receipt,
    routeNowIso(dependencies),
  )

  return noStoreJsonResponse({ promotionDecision }, { status: 201 })
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

    return Effect.succeed(
      noStoreJsonResponse(
        { error: 'forge_control_plane_route_not_found' },
        { status: 404 },
      ),
    )
  },
})
