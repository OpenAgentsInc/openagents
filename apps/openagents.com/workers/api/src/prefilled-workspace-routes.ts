import { Effect, Match as M, Schema as S } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import { readJsonObject } from './json-boundary'
import {
  type CreatePrefilledWorkspaceInput,
  type PrefilledWorkspaceRecord,
  type PrefilledWorkspaceServiceShape,
  toPublicProjection,
} from './prefilled-workspace'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

type PrefilledWorkspaceRouteEnv = Readonly<Record<string, unknown>>

export type PrefilledWorkspaceRoutesDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => PrefilledWorkspaceServiceShape
  nowIso?: () => string
  // Resolves the signed-in holder's user id, or undefined for anonymous.
  requireHolderUserId: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<string | undefined>
  requireOperator: (request: Request, env: Bindings) => Promise<boolean>
}>

const SeededMemoryInput = S.Struct({
  label: S.String,
  value: S.String,
  publicSourceRef: S.String,
})

const StarterWorkflowInput = S.Struct({
  title: S.String,
  description: S.String,
  outcomeKind: S.String,
  status: S.optionalKey(
    S.Literals(['queued', 'ready', 'completed', 'dismissed']),
  ),
})

const IntroReceiptInput = S.Struct({
  summary: S.String,
  publicSourceRefs: S.Array(S.String),
})

const CreateWorkspaceRequest = S.Struct({
  accessMode: S.optionalKey(S.Literals(['public_safe', 'private_team'])),
  projectName: S.String,
  holderRef: S.optionalKey(S.String),
  holderUserId: S.optionalKey(S.Union([S.String, S.Null])),
  status: S.optionalKey(S.Literals(['draft', 'invited', 'active', 'archived'])),
  privateProjectId: S.optionalKey(S.Union([S.String, S.Null])),
  privateTeamId: S.optionalKey(S.Union([S.String, S.Null])),
  introReceipt: IntroReceiptInput,
  seededMemory: S.optionalKey(S.Array(SeededMemoryInput)),
  starterWorkflows: S.optionalKey(S.Array(StarterWorkflowInput)),
})
type CreateWorkspaceRequest = typeof CreateWorkspaceRequest.Type

const WorkspaceEngagementEventRequest = S.Struct({
  event: S.Literal('first_run'),
})
type WorkspaceEngagementEventRequest =
  typeof WorkspaceEngagementEventRequest.Type

const routeNowIso = <Bindings>(
  dependencies: PrefilledWorkspaceRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const badRequest = (reason: string): HttpResponse =>
  noStoreJsonResponse(
    { error: 'prefilled_workspace_validation_error', reason },
    { status: 400 },
  )

const notFound = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'prefilled_workspace_not_found',
      reason: 'Workspace not found.',
    },
    { status: 404 },
  )

const dependencyPromise = <A>(
  tryPromise: () => Promise<A>,
): Effect.Effect<A, HttpResponse> =>
  Effect.tryPromise({
    catch: () => serverError(),
    try: tryPromise,
  })

const workspaceIdFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/workspaces\/([^/]+)$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const workspaceEngagementIdFromPath = (
  pathname: string,
): string | undefined => {
  const match = /^\/api\/workspaces\/([^/]+)\/engagement$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const isWorkspacesCollectionPath = (pathname: string): boolean =>
  pathname === '/api/workspaces'

const inviteUrlForWorkspace = (request: Request, workspaceId: string): string => {
  const url = new URL(request.url)

  return `${url.origin}/workspaces/${encodeURIComponent(workspaceId)}`
}

const toCreateInput = (
  body: CreateWorkspaceRequest,
): CreatePrefilledWorkspaceInput => ({
  accessMode: body.accessMode,
  projectName: body.projectName,
  holderRef: body.holderRef,
  holderUserId: body.holderUserId,
  privateProjectId: body.privateProjectId,
  privateTeamId: body.privateTeamId,
  status: body.status,
  introReceipt: {
    summary: body.introReceipt.summary,
    publicSourceRefs: [...body.introReceipt.publicSourceRefs],
  },
  seededMemory: body.seededMemory?.map(entry => ({
    label: entry.label,
    value: entry.value,
    publicSourceRef: entry.publicSourceRef,
  })),
  starterWorkflows: body.starterWorkflows?.map(workflow => ({
    title: workflow.title,
    description: workflow.description,
    outcomeKind: workflow.outcomeKind,
    status: workflow.status ?? 'queued',
  })),
})

const decodeCreateRequest = (
  request: Request,
): Effect.Effect<CreateWorkspaceRequest, HttpResponse> =>
  Effect.tryPromise({
    catch: error =>
      badRequest(error instanceof Error ? error.message : String(error)),
    try: async () =>
      S.decodeUnknownSync(CreateWorkspaceRequest)(
        await readJsonObject(request),
      ),
  })

const decodeEngagementEventRequest = (
  request: Request,
): Effect.Effect<WorkspaceEngagementEventRequest, HttpResponse> =>
  Effect.tryPromise({
    catch: error =>
      badRequest(error instanceof Error ? error.message : String(error)),
    try: async () =>
      S.decodeUnknownSync(WorkspaceEngagementEventRequest)(
        await readJsonObject(request),
      ),
  })

const workspaceEngagementView = (record: PrefilledWorkspaceRecord) => ({
  invitedAt: record.engagement.invitedAt,
  firstViewedAt: record.engagement.firstViewedAt,
  firstClaimedAt: record.engagement.firstClaimedAt,
  firstRunAt: record.engagement.firstRunAt,
  lastViewedAt: record.engagement.lastViewedAt,
  revisitCount: record.engagement.revisitCount,
})

const operatorWorkspaceView = (
  request: Request,
  record: PrefilledWorkspaceRecord,
) => ({
  accessMode: record.accessMode,
  id: record.id,
  holderUserId: record.holderUserId,
  holderRef: record.holderRef,
  inviteUrl: inviteUrlForWorkspace(request, record.id),
  privateProjectId: record.privateProjectId,
  privateTeamId: record.privateTeamId,
  projectName: record.projectName,
  status: record.status,
  seededMemory: record.seededMemory,
  starterWorkflows: record.starterWorkflows,
  introReceipt: record.introReceipt,
  engagement: workspaceEngagementView(record),
  createdAt: record.createdAt,
  updatedAt: record.updatedAt,
})

// Operator create/seed: POST /api/workspaces
const createWorkspace = <Bindings extends PrefilledWorkspaceRouteEnv>(
  dependencies: PrefilledWorkspaceRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const authorized = yield* dependencyPromise(() =>
      dependencies.requireOperator(request, env),
    )

    if (!authorized) {
      return forbidden()
    }

    const body = yield* decodeCreateRequest(request)

    if (body.projectName.trim() === '') {
      return badRequest('A project name is required.')
    }

    if (
      body.accessMode === 'private_team' &&
      (body.privateTeamId === undefined || body.privateTeamId === null)
    ) {
      return badRequest('A private team workspace requires privateTeamId.')
    }

    const store = dependencies.makeStore(env)
    const record = yield* Effect.promise(() =>
      store.createWorkspace(toCreateInput(body)),
    )

    return noStoreJsonResponse(
      {
        generatedAt: nowIso,
        workspace: operatorWorkspaceView(request, record),
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(error)))

// Get a workspace: GET /api/workspaces/:workspaceId
//
// Operators (admin bearer) get the full operator view. Otherwise public_safe
// rows use the holder claim/bind path, while private_team rows require active
// team membership before returning the holder-facing projection.
const getWorkspace = <Bindings extends PrefilledWorkspaceRouteEnv>(
  dependencies: PrefilledWorkspaceRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workspaceId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)

    const isOperator = yield* dependencyPromise(() =>
      dependencies.requireOperator(request, env),
    )

    if (isOperator) {
      const record = yield* Effect.promise(() =>
        store.readWorkspace(workspaceId),
      )

      if (record === undefined) {
        return notFound()
      }

      return noStoreJsonResponse({
        generatedAt: nowIso,
        viewer: 'operator',
        workspace: operatorWorkspaceView(request, record),
      })
    }

    const holderUserId = yield* dependencyPromise(() =>
      dependencies.requireHolderUserId(request, env, ctx),
    )

    if (holderUserId === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const existingRecord = yield* Effect.promise(() =>
      store.readWorkspace(workspaceId),
    )

    if (existingRecord === undefined) {
      return notFound()
    }

    if (existingRecord.accessMode === 'private_team') {
      const record = yield* Effect.promise(() =>
        store.readPrivateWorkspaceForTeamMember(workspaceId, holderUserId),
      )

      if (record === undefined) {
        return forbidden()
      }

      return noStoreJsonResponse({
        generatedAt: nowIso,
        viewer: 'team_member',
        workspace: toPublicProjection(record),
      })
    }

    const record = yield* Effect.promise(() =>
      store.readOrClaimWorkspaceForHolder(workspaceId, holderUserId),
    )

    if (record === undefined) {
      return notFound()
    }

    return noStoreJsonResponse({
      generatedAt: nowIso,
      viewer: 'holder',
      workspace: toPublicProjection(record),
    })
  }).pipe(Effect.catch(error => Effect.succeed(error)))

const recordWorkspaceEngagement = <Bindings extends PrefilledWorkspaceRouteEnv>(
  dependencies: PrefilledWorkspaceRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workspaceId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const body = yield* decodeEngagementEventRequest(request)
    const isOperator = yield* dependencyPromise(() =>
      dependencies.requireOperator(request, env),
    )

    if (body.event !== 'first_run') {
      return badRequest('Unsupported workspace engagement event.')
    }

    if (isOperator) {
      const record = yield* Effect.promise(() =>
        store.recordFirstRunForOperator(workspaceId),
      )

      if (record === undefined) {
        return notFound()
      }

      return noStoreJsonResponse({
        generatedAt: nowIso,
        viewer: 'operator',
        workspace: operatorWorkspaceView(request, record),
      })
    }

    const holderUserId = yield* dependencyPromise(() =>
      dependencies.requireHolderUserId(request, env, ctx),
    )

    if (holderUserId === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const existingRecord = yield* Effect.promise(() =>
      store.readWorkspace(workspaceId),
    )

    if (existingRecord === undefined) {
      return notFound()
    }

    if (existingRecord.accessMode === 'private_team') {
      const record = yield* Effect.promise(() =>
        store.recordFirstRunForPrivateTeamMember(workspaceId, holderUserId),
      )

      if (record === undefined) {
        return forbidden()
      }

      return noStoreJsonResponse({
        generatedAt: nowIso,
        viewer: 'team_member',
        workspace: toPublicProjection(record),
      })
    }

    const record = yield* Effect.promise(() =>
      store.recordFirstRunForHolder(workspaceId, holderUserId),
    )

    if (record === undefined) {
      return notFound()
    }

    return noStoreJsonResponse({
      generatedAt: nowIso,
      viewer: 'holder',
      workspace: toPublicProjection(record),
    })
  }).pipe(Effect.catch(error => Effect.succeed(error)))

export const makePrefilledWorkspaceRoutes = <
  Bindings extends PrefilledWorkspaceRouteEnv,
>(
  dependencies: PrefilledWorkspaceRoutesDependencies<Bindings>,
) => ({
  routePrefilledWorkspaceRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (isWorkspacesCollectionPath(url.pathname)) {
      return M.value(request.method).pipe(
        M.when('POST', () => createWorkspace(dependencies, request, env)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const workspaceEngagementId = workspaceEngagementIdFromPath(url.pathname)

    if (workspaceEngagementId !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          recordWorkspaceEngagement(
            dependencies,
            request,
            env,
            ctx,
            workspaceEngagementId,
          ),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const workspaceId = workspaceIdFromPath(url.pathname)

    if (workspaceId !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          getWorkspace(dependencies, request, env, ctx, workspaceId),
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    return undefined
  },
})
