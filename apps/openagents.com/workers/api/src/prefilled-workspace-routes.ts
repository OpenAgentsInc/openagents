// COORDINATOR WIRING:
// In workers/api/src/index.ts, construct the routes alongside the other route
// factories (near makeNativeListsRoutes), e.g.:
//
//   const prefilledWorkspaceRoutes = makePrefilledWorkspaceRoutes<WorkerBindings>({
//     makeStore: env => makePrefilledWorkspaceService(openAgentsDatabase(env)),
//     requireHolderUserId: async (request, env, ctx) => {
//       const session = await requireBrowserSession(request, env, ctx)
//       return session?.user.userId
//     },
//     requireOperator: (request, env) => requireAdminApiToken(request, env),
//   })
//
// Then chain it into the omni dispatch chain (routeOmniRequest), e.g.:
//
//   routeOmniRequest: (request, env, ctx) =>
//     omniRoutes.routeOmniRequest(request, env, ctx) ??
//     ...
//     nativeListsRoutes.routeNativeListsRequest(request, env, ctx) ??
//     prefilledWorkspaceRoutes.routePrefilledWorkspaceRequest(request, env, ctx),
//
// Also add the imports near the other route imports:
//   import { makePrefilledWorkspaceService } from './prefilled-workspace'
//   import { makePrefilledWorkspaceRoutes } from './prefilled-workspace-routes'

import { Effect, Match as M, Schema as S } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
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
  projectName: S.String,
  holderRef: S.optionalKey(S.String),
  holderUserId: S.optionalKey(S.Union([S.String, S.Null])),
  status: S.optionalKey(S.Literals(['draft', 'invited', 'active', 'archived'])),
  introReceipt: IntroReceiptInput,
  seededMemory: S.optionalKey(S.Array(SeededMemoryInput)),
  starterWorkflows: S.optionalKey(S.Array(StarterWorkflowInput)),
})
type CreateWorkspaceRequest = typeof CreateWorkspaceRequest.Type

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

const workspaceIdFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/workspaces\/([^/]+)$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const isWorkspacesCollectionPath = (pathname: string): boolean =>
  pathname === '/api/workspaces'

const toCreateInput = (
  body: CreateWorkspaceRequest,
): CreatePrefilledWorkspaceInput => ({
  projectName: body.projectName,
  holderRef: body.holderRef,
  holderUserId: body.holderUserId,
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

const operatorWorkspaceView = (record: PrefilledWorkspaceRecord) => ({
  id: record.id,
  holderUserId: record.holderUserId,
  holderRef: record.holderRef,
  projectName: record.projectName,
  status: record.status,
  seededMemory: record.seededMemory,
  starterWorkflows: record.starterWorkflows,
  introReceipt: record.introReceipt,
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
    const authorized = yield* Effect.promise(() =>
      dependencies.requireOperator(request, env),
    )

    if (!authorized) {
      return forbidden()
    }

    const body = yield* decodeCreateRequest(request)

    if (body.projectName.trim() === '') {
      return badRequest('A project name is required.')
    }

    const store = dependencies.makeStore(env)
    const record = yield* Effect.promise(() =>
      store.createWorkspace(toCreateInput(body)),
    )

    return noStoreJsonResponse(
      {
        generatedAt: nowIso,
        workspace: operatorWorkspaceView(record),
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(error)))

// Get a workspace: GET /api/workspaces/:workspaceId
//
// Operators (admin bearer) get the full operator view. Otherwise the signed-in
// holder bound to the workspace gets the public-safe holder projection.
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

    const isOperator = yield* Effect.promise(() =>
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
        workspace: operatorWorkspaceView(record),
      })
    }

    const holderUserId = yield* Effect.promise(() =>
      dependencies.requireHolderUserId(request, env, ctx),
    )

    if (holderUserId === undefined) {
      return noStoreJsonResponse(
        { error: 'unauthorized' },
        { status: 401 },
      )
    }

    const record = yield* Effect.promise(() =>
      store.readWorkspaceForHolder(workspaceId, holderUserId),
    )

    if (record === undefined) {
      return notFound()
    }

    return noStoreJsonResponse({
      generatedAt: nowIso,
      viewer: 'holder',
      workspace: toPublicProjection(record),
    })
  })

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
