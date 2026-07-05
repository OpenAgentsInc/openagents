import {
  ForgeCoordinationChangeState,
  ForgeCoordinationIssueState,
  ForgeCoordinationStatusState,
  ForgeGitHubMirrorReceipt,
  ForgeGitHubMirrorStatus,
  ForgeMergeQueueLedgerState,
  ForgePromotionDecisionReceipt,
  ForgeVerificationReceipt,
  decodeForgeControlPlaneScope,
  type ForgeControlPlaneScope,
} from '@openagentsinc/forge-protocol'
import { Effect, Schema as S } from 'effect'

import { timingSafeEqual } from './agent-registration'
import { readBearerToken } from './auth/bearer-token'
import {
  type ForgeCoordinationStore,
} from './forge-coordination-store'
import type {
  ForgeGitHubMirrorStore,
} from './forge-github-mirror-store'
import type {
  ForgeGitCanonicalStore,
} from './forge-git-canonical-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { logWorkerRouteError, unwrapEffectTryPromiseCause } from './observability'
import {
  decodeUnknownWithSchema,
  parseJsonStringArray,
  readJsonObject,
} from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

const FORGE_GIT_TOKEN_PREFIX = 'oa_forge_git_'
const OPENAGENTS_FORGE_TENANT_REF = 'tenant.openagents'
const OPENAGENTS_FORGE_REPOSITORY_REF = 'repo.openagents.openagents'
const OPENAGENTS_GITHUB_OWNER = 'OpenAgentsInc'
const OPENAGENTS_GITHUB_REPO = 'openagents'
const OPENAGENTS_DEFAULT_BRANCH_REF = 'refs/heads/main'

const OPENAGENTS_GITHUB_REPOSITORY = `${OPENAGENTS_GITHUB_OWNER}/${OPENAGENTS_GITHUB_REPO}`

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
  makeCanonicalStore: (env: Bindings) => ForgeGitCanonicalStore
  makeGitHubMirrorStore: (env: Bindings) => ForgeGitHubMirrorStore
  makeStore: (env: Bindings) => ForgeCoordinationStore
  mirrorGitHubToken?: (env: Bindings) => string | undefined
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

const ForgeMergeQueueDeriveRequest = S.Struct({
  tenantRef: S.String,
  queueRef: S.String,
  actualHead: S.String,
  sourceRefs: S.optionalKey(S.Array(S.String)),
})

const ForgeGitHubMirrorRunRequest = S.Struct({
  tenantRef: S.String,
  promotionRef: S.optionalKey(S.String),
})

const ForgeOpenAgentsImportRequest = S.Struct({
  tenantRef: S.String,
  repositoryRef: S.String,
})

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

const commitShaPattern = /^[a-f0-9]{40}$/i

type ForgeQueueCandidate = Readonly<{
  baseHead: string
  blockerRefs: ReadonlyArray<string>
  changeRef: string
  issueNumber: number
  issueOpen: boolean
  patchHead: string
  queuedAt: string
  verificationPassed: boolean
}>

type ForgeQueueReadyEntry = Readonly<{
  candidateRef: string
  issueNumber: number
  promotionRef: string
  virtualBaseCommit: string
  virtualHeadCommit: string
  waitsForActualHead: string | null
}>

type ForgeQueueBlockedEntry = Readonly<{
  blockedReasonRef: string
  candidateRef: string
  detail: string
  issueNumber: number
  status: 'blocked' | 'needs-rebase'
}>

const jsonStringArray = (value: string): ReadonlyArray<string> => {
  return parseJsonStringArray(value)
}

const issueNumberFromRef = (
  issueRef: string,
  githubIssueNumber: number | null | undefined,
): number => {
  if (githubIssueNumber !== null && githubIssueNumber !== undefined) {
    return githubIssueNumber
  }
  const match = issueRef.match(/(?:^|[._#-])(\d+)$/u)
  return match === null ? 0 : Number(match[1])
}

const promotionRefForCandidate = async (
  candidate: ForgeQueueCandidate,
): Promise<string> =>
  `promotion.forge.next_actual.${(await sha256Hex(
    `${candidate.issueNumber}:${candidate.baseHead}:${candidate.patchHead}`,
  )).slice(0, 24)}`

const deletionPoisonBlocker = (
  blockerRefs: ReadonlyArray<string>,
): string | undefined =>
  blockerRefs.find(ref =>
    /(?:delete|deletion|mass_deletion|protected_path_deleted|deletion_poison)/iu.test(
      ref,
    ),
  )

const blockQueueCandidate = (
  candidate: ForgeQueueCandidate,
  blockedReasonRef: string,
  detail: string,
): ForgeQueueBlockedEntry => ({
  blockedReasonRef,
  candidateRef: candidate.changeRef,
  detail,
  issueNumber: candidate.issueNumber,
  status:
    blockedReasonRef === 'virtual_merge_queue.blocked.stale_base'
      ? 'needs-rebase'
      : 'blocked',
})

const projectForgeMergeQueue = async (
  actualHead: string,
  candidates: ReadonlyArray<ForgeQueueCandidate>,
) => {
  const ready: Array<ForgeQueueReadyEntry> = []
  const blocked: Array<ForgeQueueBlockedEntry> = []
  const seenIssues = new Set<number>()
  let virtualHead = actualHead

  if (!commitShaPattern.test(actualHead)) {
    return {
      actualHead,
      branchBaseForNextAssignment: actualHead,
      blocked: candidates.map(candidate =>
        blockQueueCandidate(
          candidate,
          'virtual_merge_queue.blocked.invalid_commit',
          'actual head is not a pinned 40-character commit',
        ),
      ),
      nextActualPromotion: null,
      ready,
      virtualHead: actualHead,
    }
  }

  for (const candidate of [...candidates].sort((left, right) =>
    `${left.queuedAt}\0${left.changeRef}`.localeCompare(
      `${right.queuedAt}\0${right.changeRef}`,
    ),
  )) {
    if (seenIssues.has(candidate.issueNumber)) {
      blocked.push(
        blockQueueCandidate(
          candidate,
          'virtual_merge_queue.blocked.duplicate_issue',
          `issue #${candidate.issueNumber} already has an earlier queue candidate`,
        ),
      )
      continue
    }
    seenIssues.add(candidate.issueNumber)

    if (
      candidate.issueNumber <= 0 ||
      !commitShaPattern.test(candidate.baseHead) ||
      !commitShaPattern.test(candidate.patchHead)
    ) {
      blocked.push(
        blockQueueCandidate(
          candidate,
          'virtual_merge_queue.blocked.invalid_commit',
          'candidate issue, base, or patch head is not pinned',
        ),
      )
      continue
    }

    if (!candidate.issueOpen) {
      blocked.push(
        blockQueueCandidate(
          candidate,
          'virtual_merge_queue.blocked.issue_closed',
          `issue #${candidate.issueNumber} is not open`,
        ),
      )
      continue
    }

    if (!candidate.verificationPassed) {
      blocked.push(
        blockQueueCandidate(
          candidate,
          'virtual_merge_queue.blocked.verification_not_passed',
          `issue #${candidate.issueNumber} has no passing verification receipt`,
        ),
      )
      continue
    }

    const deletionBlocker = deletionPoisonBlocker(candidate.blockerRefs)
    if (deletionBlocker !== undefined) {
      blocked.push(
        blockQueueCandidate(
          candidate,
          'virtual_merge_queue.blocked.protected_path_deleted',
          `candidate carries deletion-poison blocker ${deletionBlocker}`,
        ),
      )
      continue
    }

    if (candidate.baseHead.toLowerCase() !== virtualHead.toLowerCase()) {
      blocked.push(
        blockQueueCandidate(
          candidate,
          'virtual_merge_queue.blocked.stale_base',
          `candidate base ${candidate.baseHead} does not match virtual head ${virtualHead}`,
        ),
      )
      continue
    }

    const promotionRef = await promotionRefForCandidate(candidate)
    const waitsForActualHead =
      candidate.baseHead.toLowerCase() === actualHead.toLowerCase()
        ? null
        : candidate.baseHead
    ready.push({
      candidateRef: candidate.changeRef,
      issueNumber: candidate.issueNumber,
      promotionRef,
      virtualBaseCommit: candidate.baseHead,
      virtualHeadCommit: candidate.patchHead,
      waitsForActualHead,
    })
    virtualHead = candidate.patchHead
  }

  return {
    actualHead,
    branchBaseForNextAssignment: virtualHead,
    blocked,
    nextActualPromotion:
      ready.find(entry => entry.waitsForActualHead === null) ?? null,
    ready,
    virtualHead,
  }
}

const deriveForgeMergeQueueProjection = async (
  store: ForgeCoordinationStore,
  tenantRef: string,
  actualHead: string,
) => {
  const [changes, issues, verificationReceipts] = await Promise.all([
    store.listChanges(tenantRef, 100),
    store.listIssues(tenantRef, 100),
    store.listVerificationReceipts(tenantRef, 100),
  ])
  const issuesByRef = new Map(issues.map(issue => [issue.issue_ref, issue]))
  const passedVerificationRefs = new Set(
    verificationReceipts
      .filter(receipt => receipt.verdict === 'passed')
      .map(receipt => receipt.verification_ref),
  )
  const passedVerificationChangeRefs = new Set(
    verificationReceipts
      .filter(receipt => receipt.verdict === 'passed')
      .map(receipt => receipt.change_ref),
  )
  const candidates = changes
    .filter(change => change.state === 'ready' || change.state === 'open')
    .map((change): ForgeQueueCandidate => {
      const issue = issuesByRef.get(change.issue_ref)
      return {
        baseHead: change.base_head,
        blockerRefs: jsonStringArray(change.blocker_refs_json),
        changeRef: change.change_ref,
        issueNumber: issueNumberFromRef(
          change.issue_ref,
          issue?.github_issue_number,
        ),
        issueOpen: issue?.state !== 'closed',
        patchHead: change.patch_head,
        queuedAt: change.created_at,
        verificationPassed:
          (change.verification_ref !== null &&
            passedVerificationRefs.has(change.verification_ref)) ||
          passedVerificationChangeRefs.has(change.change_ref),
      }
    })

  return projectForgeMergeQueue(actualHead, candidates)
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
  const actualHead = optionalQuery(url, 'actualHead')
  const derived =
    actualHead === undefined
      ? undefined
      : await deriveForgeMergeQueueProjection(store, tenantRef, actualHead)

  return noStoreJsonResponse({
    ...(derived === undefined ? {} : { derived }),
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

const routeQueueDerive = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const body = await decodeBody(request, ForgeMergeQueueDeriveRequest)
  await requireScope(dependencies, request, env, 'forge:queue:write', body.tenantRef)
  const store = dependencies.makeStore(env)
  const derived = await deriveForgeMergeQueueProjection(
    store,
    body.tenantRef,
    body.actualHead,
  )
  const queueSnapshot = await store.recordMergeQueueLedger({
    tenantRef: body.tenantRef,
    queueRef: body.queueRef,
    baseHead: body.actualHead,
    actualHead: body.actualHead,
    virtualHead: derived.virtualHead,
    state: 'projected',
    nextPromotionRef: derived.nextActualPromotion?.promotionRef ?? null,
    ready: derived.ready,
    blocked: derived.blocked,
    sourceRefs: body.sourceRefs ?? [],
    nowIso: routeNowIso(dependencies),
  })

  return noStoreJsonResponse({ derived, queueSnapshot }, { status: 201 })
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

const forgeGitHubMirrorStatusFromQuery = (
  url: URL,
): typeof ForgeGitHubMirrorStatus.Type | undefined => {
  const status = optionalQuery(url, 'status')

  if (status === undefined) {
    return undefined
  }

  try {
    return decodeUnknownWithSchema(ForgeGitHubMirrorStatus, status)
  } catch {
    throw new ForgeControlPlaneHttpError(
      400,
      'forge_github_mirror_bad_status_filter',
    )
  }
}

const forgeGitHubMirrorAttention = (
  receipts: ReadonlyArray<ForgeGitHubMirrorReceipt>,
) => {
  const reasonRefs = receipts
    .filter(receipt => receipt.status === 'failed' || receipt.status === 'refused')
    .map(receipt => receipt.refusal_reason ?? receipt.error_reason)
    .filter((reason): reason is string => reason !== null)

  return {
    receiptRefs: receipts.map(receipt => receipt.mirror_ref),
    reasonRefs,
    state: reasonRefs.length > 0 ? 'needs_attention' : 'clear',
  }
}

const routeGitHubMirrorReceipts = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const tenantRef = tenantRefFromQuery(url)
  await requireScope(dependencies, request, env, 'forge:mirror:read', tenantRef)
  const mirrorReceipts = await dependencies
    .makeGitHubMirrorStore(env)
    .listReceipts(tenantRef, {
      limit: limitFromQuery(url),
      promotionRef: optionalQuery(url, 'promotionRef'),
      status: forgeGitHubMirrorStatusFromQuery(url),
    })

  return noStoreJsonResponse({
    attention: forgeGitHubMirrorAttention(mirrorReceipts),
    limit: limitFromQuery(url),
    mirrorReceipts,
    tenantRef,
  })
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

class ForgeGitHubMirrorApiError extends Error {
  constructor(readonly errorCode: string) {
    super(errorCode)
    this.name = 'ForgeGitHubMirrorApiError'
  }
}

type ForgeGitHubMirrorDestination = Readonly<{
  branchApiRef: string
  githubRepository: string
  githubRef: string
  owner: string
  repo: string
}>

const openAgentsGitHubMirrorDestination = (): ForgeGitHubMirrorDestination => ({
  branchApiRef: 'heads/main',
  githubRepository: OPENAGENTS_GITHUB_REPOSITORY,
  githubRef: OPENAGENTS_DEFAULT_BRANCH_REF,
  owner: OPENAGENTS_GITHUB_OWNER,
  repo: OPENAGENTS_GITHUB_REPO,
})

const githubMirrorHeaders = (
  token: string,
  contentType: boolean = false,
): HeadersInit => ({
  accept: 'application/vnd.github+json',
  authorization: `Bearer ${token}`,
  ...(contentType ? { 'content-type': 'application/json' } : {}),
  'user-agent': 'openagents-forge-github-mirror',
  'x-github-api-version': '2022-11-28',
})

const readGitHubDestinationHead = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  destination: ForgeGitHubMirrorDestination,
  token: string,
): Promise<string> => {
  const response = await (dependencies.fetch ?? fetch)(
    `https://api.github.com/repos/${encodeURIComponent(destination.owner)}/${encodeURIComponent(destination.repo)}/git/ref/${destination.branchApiRef}`,
    { headers: githubMirrorHeaders(token) },
  )

  if (!response.ok) {
    throw new ForgeGitHubMirrorApiError(
      `forge_github_mirror_destination_lookup_http_${response.status}`,
    )
  }

  const body = (await response.json()) as { object?: { sha?: unknown } }
  const sha = typeof body.object?.sha === 'string' ? body.object.sha : ''

  if (!commitShaPattern.test(sha)) {
    throw new ForgeGitHubMirrorApiError(
      'forge_github_mirror_destination_lookup_bad_ref',
    )
  }

  return sha.toLowerCase()
}

const updateGitHubDestinationHead = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  destination: ForgeGitHubMirrorDestination,
  token: string,
  commitId: string,
): Promise<void> => {
  const response = await (dependencies.fetch ?? fetch)(
    `https://api.github.com/repos/${encodeURIComponent(destination.owner)}/${encodeURIComponent(destination.repo)}/git/refs/${destination.branchApiRef}`,
    {
      body: JSON.stringify({ force: false, sha: commitId }),
      headers: githubMirrorHeaders(token, true),
      method: 'PATCH',
    },
  )

  if (!response.ok) {
    throw new ForgeGitHubMirrorApiError(
      `forge_github_mirror_ref_update_http_${response.status}`,
    )
  }
}

const mirrorRefForPromotion = async (
  promotionRef: string,
  destination: ForgeGitHubMirrorDestination,
): Promise<string> =>
  `mirror.github.openagents.main.${(await sha256Hex(
    `${promotionRef}:${destination.githubRepository}:${destination.githubRef}`,
  )).slice(0, 24)}`

const uniqueSourceRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(refs.filter(ref => ref.trim() !== ''))]

const repositoryRefForPromotion = async (
  store: ForgeCoordinationStore,
  promotion: ForgePromotionDecisionReceipt,
): Promise<string> => {
  if (promotion.verification_ref === null) {
    return OPENAGENTS_FORGE_REPOSITORY_REF
  }

  const verification = (
    await store.listVerificationReceipts(
      promotion.tenant_ref,
      100,
      promotion.change_ref,
    )
  ).find(receipt => receipt.verification_ref === promotion.verification_ref)

  return verification?.repository_ref ?? OPENAGENTS_FORGE_REPOSITORY_REF
}

const recordGitHubMirrorReceipt = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    commitId: string
    destination: ForgeGitHubMirrorDestination
    errorReason?: string | undefined
    nowIso: string
    promotion: ForgePromotionDecisionReceipt
    refusalReason?: string | undefined
    repositoryRef: string
    status: typeof ForgeGitHubMirrorStatus.Type
  }>,
): Promise<ForgeGitHubMirrorReceipt> =>
  dependencies.makeGitHubMirrorStore(env).recordReceipt({
    change_ref: input.promotion.change_ref,
    commit_id: input.commitId,
    completed_at: input.nowIso,
    destination_github_ref: input.destination.githubRef,
    destination_github_repository: input.destination.githubRepository,
    error_reason: input.errorReason ?? null,
    first_attempted_at: input.nowIso,
    last_attempted_at: input.nowIso,
    mirror_ref: await mirrorRefForPromotion(
      input.promotion.promotion_ref,
      input.destination,
    ),
    promotion_ref: input.promotion.promotion_ref,
    redacted: true,
    refusal_reason: input.refusalReason ?? null,
    repository_ref: input.repositoryRef,
    source_canonical_ref: input.promotion.target_ref,
    source_refs: uniqueSourceRefs([
      ...input.promotion.source_refs,
      input.promotion.promotion_ref,
      input.promotion.target_ref,
      `github:${input.destination.githubRepository}#${input.destination.githubRef}`,
    ]),
    status: input.status,
    tenant_ref: input.promotion.tenant_ref,
  })

const refusalReceipt = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    destination: ForgeGitHubMirrorDestination
    nowIso: string
    promotion: ForgePromotionDecisionReceipt
    reason: string
    repositoryRef: string
  }>,
): Promise<ForgeGitHubMirrorReceipt> =>
  recordGitHubMirrorReceipt(dependencies, env, {
    commitId: input.promotion.promoted_head ?? input.promotion.candidate_head,
    destination: input.destination,
    nowIso: input.nowIso,
    promotion: input.promotion,
    refusalReason: input.reason,
    repositoryRef: input.repositoryRef,
    status: 'refused',
  })

const failureReceipt = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    commitId: string
    destination: ForgeGitHubMirrorDestination
    nowIso: string
    promotion: ForgePromotionDecisionReceipt
    reason: string
    repositoryRef: string
  }>,
): Promise<ForgeGitHubMirrorReceipt> =>
  recordGitHubMirrorReceipt(dependencies, env, {
    commitId: input.commitId,
    destination: input.destination,
    errorReason: input.reason,
    nowIso: input.nowIso,
    promotion: input.promotion,
    repositoryRef: input.repositoryRef,
    status: 'failed',
  })

const mirrorPromotionToGitHub = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  env: Bindings,
  promotion: ForgePromotionDecisionReceipt,
): Promise<ForgeGitHubMirrorReceipt> => {
  const destination = openAgentsGitHubMirrorDestination()
  const nowIso = routeNowIso(dependencies)
  const store = dependencies.makeStore(env)
  const repositoryRef = await repositoryRefForPromotion(store, promotion)
  const existing = await dependencies
    .makeGitHubMirrorStore(env)
    .readReceiptForPromotion(
      promotion.tenant_ref,
      promotion.promotion_ref,
      destination.githubRepository,
      destination.githubRef,
    )

  if (
    existing?.status === 'mirrored' &&
    existing.commit_id === promotion.promoted_head
  ) {
    return existing
  }

  if (promotion.decision !== 'approved' || promotion.promoted_head === null) {
    return refusalReceipt(dependencies, env, {
      destination,
      nowIso,
      promotion,
      reason: 'forge_github_mirror_requires_approved_promotion',
      repositoryRef,
    })
  }

  if (promotion.target_ref !== destination.githubRef) {
    return refusalReceipt(dependencies, env, {
      destination,
      nowIso,
      promotion,
      reason: 'forge_github_mirror_target_ref_not_configured',
      repositoryRef,
    })
  }

  if (repositoryRef !== OPENAGENTS_FORGE_REPOSITORY_REF) {
    return refusalReceipt(dependencies, env, {
      destination,
      nowIso,
      promotion,
      reason: 'forge_github_mirror_repository_not_configured',
      repositoryRef,
    })
  }

  const canonicalRef = await dependencies
    .makeCanonicalStore(env)
    .readRef(promotion.tenant_ref, repositoryRef, promotion.target_ref)

  if (
    canonicalRef?.state !== 'active' ||
    canonicalRef.object_id?.toLowerCase() !== promotion.promoted_head.toLowerCase()
  ) {
    return refusalReceipt(dependencies, env, {
      destination,
      nowIso,
      promotion,
      reason: 'forge_github_mirror_source_canonical_ref_not_promoted',
      repositoryRef,
    })
  }

  const token = dependencies.mirrorGitHubToken?.(env)
  if (token === undefined || token.trim() === '') {
    return failureReceipt(dependencies, env, {
      commitId: promotion.promoted_head,
      destination,
      nowIso,
      promotion,
      reason: 'forge_github_mirror_token_missing',
      repositoryRef,
    })
  }

  try {
    const destinationHead = await readGitHubDestinationHead(
      dependencies,
      destination,
      token,
    )
    if (destinationHead !== promotion.promoted_head.toLowerCase()) {
      await updateGitHubDestinationHead(
        dependencies,
        destination,
        token,
        promotion.promoted_head,
      )
    }

    return recordGitHubMirrorReceipt(dependencies, env, {
      commitId: promotion.promoted_head,
      destination,
      nowIso,
      promotion,
      repositoryRef,
      status: 'mirrored',
    })
  } catch (error) {
    return failureReceipt(dependencies, env, {
      commitId: promotion.promoted_head,
      destination,
      nowIso,
      promotion,
      reason:
        error instanceof ForgeGitHubMirrorApiError
          ? error.errorCode
          : 'forge_github_mirror_fetch_failed',
      repositoryRef,
    })
  }
}

const promotionCandidatesForMirrorRun = async (
  store: ForgeCoordinationStore,
  tenantRef: string,
  promotionRef: string | undefined,
): Promise<ReadonlyArray<ForgePromotionDecisionReceipt>> => {
  const promotions = await store.listPromotionDecisionReceipts(tenantRef, 100)

  if (promotionRef === undefined) {
    return promotions.filter(promotion => promotion.decision === 'approved')
  }

  const promotion = promotions.find(
    candidate => candidate.promotion_ref === promotionRef,
  )

  if (promotion === undefined) {
    throw new ForgeControlPlaneHttpError(
      404,
      'forge_github_mirror_promotion_not_found',
    )
  }

  return [promotion]
}

const routeGitHubMirrorRun = async <Bindings>(
  dependencies: ForgeControlPlaneRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const body = await decodeBody(request, ForgeGitHubMirrorRunRequest)
  await requireScope(dependencies, request, env, 'forge:mirror:write', body.tenantRef)
  const store = dependencies.makeStore(env)
  const promotions = await promotionCandidatesForMirrorRun(
    store,
    body.tenantRef,
    body.promotionRef,
  )
  // Each promotion's GitHub mirror attempt is independent. Isolate them with
  // Effect structured concurrency (`Effect.forEach` + `Effect.result`) rather
  // than a bare `Promise.all`: one promotion's mirror throwing (e.g. a
  // transient GitHub API error not already caught inside
  // `mirrorPromotionToGitHub`) must not discard the already-computed mirror
  // receipts for every OTHER promotion in the same batch.
  const mirrorOutcomes = await Effect.runPromise(
    Effect.forEach(
      promotions,
      promotion =>
        Effect.result(
          Effect.tryPromise(() => mirrorPromotionToGitHub(dependencies, env, promotion)),
        ).pipe(
          Effect.map(outcome => ({ outcome, promotion })),
        ),
      { concurrency: 'unbounded' },
    ),
  )

  const mirrorReceipts: Array<ForgeGitHubMirrorReceipt> = []
  const erroredPromotionRefs: Array<string> = []

  for (const { outcome, promotion } of mirrorOutcomes) {
    if (outcome._tag === 'Success') {
      mirrorReceipts.push(outcome.success)
      continue
    }

    erroredPromotionRefs.push(promotion.promotion_ref)
    logWorkerRouteError(
      'forge_github_mirror_promotion_failed',
      unwrapEffectTryPromiseCause(outcome.failure),
      {
        promotionRef: promotion.promotion_ref,
        tenantRef: body.tenantRef,
      },
    )
  }

  return noStoreJsonResponse({
    attention: forgeGitHubMirrorAttention(mirrorReceipts),
    mirrorReceipts,
    mirroredCount: mirrorReceipts.filter(receipt => receipt.status === 'mirrored')
      .length,
    refusedCount: mirrorReceipts.filter(receipt => receipt.status === 'refused')
      .length,
    failedCount: mirrorReceipts.filter(receipt => receipt.status === 'failed')
      .length,
    erroredCount: erroredPromotionRefs.length,
    erroredPromotionRefs,
    tenantRef: body.tenantRef,
  })
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

    if (route.length === 2 && route[0] === 'queue' && route[1] === 'derive') {
      return routeEffect(() => routeQueueDerive(dependencies, request, env))
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

    if (route.length === 1 && route[0] === 'github-mirror') {
      return routeEffect(() =>
        routeGitHubMirrorReceipts(dependencies, request, env, url),
      )
    }

    if (
      route.length === 2 &&
      route[0] === 'github-mirror' &&
      route[1] === 'run'
    ) {
      return routeEffect(() =>
        routeGitHubMirrorRun(dependencies, request, env),
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
