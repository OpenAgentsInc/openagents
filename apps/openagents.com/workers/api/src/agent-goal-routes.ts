import { Effect, Layer, Match as M, Schema as S } from 'effect'

import {
  AgentGoalCurrentExists,
  AgentGoalEventRepository,
  AgentGoalRuntimeService,
  AgentGoalRuntimeServiceLive,
  AgentGoalTerminalStatus,
  AgentGoalToolMutationForbidden,
  type AgentGoalToolResult,
} from './agent-goal-runtime'
import {
  AgentPublicProjectionService,
  AgentPublicProjectionServiceLive,
} from './agent-goal-public-projection'
import {
  AgentGoalAccessDenied,
  AgentGoalAccessService,
  AgentGoalAccessServiceLayer,
  type AgentGoalAccessServiceShape,
  type AgentGoalActor,
  AgentGoalNotFound,
  AgentGoalPublicProjectionUnsafe,
  type AgentGoalRecord,
  AgentGoalRepository,
  type AgentGoalScope,
  AgentGoalStaleUpdate,
  AgentGoalStorageError,
  AgentGoalValidationError,
  AgentGoalVisibility,
  type PublicAgentGoalRecord,
} from './agent-goals'
import {
  makeAgentGoalEventRepositoryLayerForEnv,
  makeAgentGoalRepositoryLayerForEnv,
} from './agent-runtime-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'
import { publishAgentGoalSyncIfBound } from './sync-notifier'
import type { TeamRole, UserTeamProject } from './team-repository'

type GoalRouteEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type WorkerEnv<Env extends GoalRouteEnv> = Env
type HttpResponse = globalThis.Response
type GoalRouteEffect = Effect.Effect<HttpResponse>

type GoalRouteSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type GoalRouteAuthenticatedActor =
  | Readonly<{
      kind: 'human'
      user: Readonly<{ userId: string }>
    }>
  | Readonly<{
      kind: 'agent'
      agent: Readonly<{
        user: Readonly<{ id: string }>
      }>
    }>

type GoalRouteDependencies<
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  authenticateRequestActor: (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
  ) => Promise<GoalRouteAuthenticatedActor | undefined>
  makeEventRepositoryLayer?:
    | ((env: WorkerEnv<Env>) => Layer.Layer<AgentGoalEventRepository>)
    | undefined
  makeRepositoryLayer?:
    | ((env: WorkerEnv<Env>) => Layer.Layer<AgentGoalRepository>)
    | undefined
  readActiveTeamMembershipRole: (
    db: D1Database,
    teamId: string,
    userId: string,
  ) => Promise<TeamRole | undefined>
  readActiveTeamProject: (
    db: D1Database,
    teamId: string,
    projectId: string,
  ) => Promise<UserTeamProject | undefined>
  requireAdminApiToken: (
    request: Request,
    env: WorkerEnv<Env>,
  ) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

const publishGoalSync = <Env extends GoalRouteEnv>(
  env: WorkerEnv<Env>,
  ctx: ExecutionContext,
  goal: AgentGoalRecord,
  actorId: string,
): Effect.Effect<void> =>
  Effect.tryPromise({
    try: () => publishAgentGoalSyncIfBound(env, ctx, goal, actorId),
    catch: () => undefined,
  }).pipe(Effect.catch(() => Effect.void))

const DEFAULT_BROWSER_AGENT_ID = 'autopilot'

export class GoalScopeSelector extends S.Class<GoalScopeSelector>(
  'GoalScopeSelector',
)({
  agentId: S.optionalKey(S.String),
  teamId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
}) {}

export class CreateGoalRequest extends S.Class<CreateGoalRequest>(
  'CreateGoalRequest',
)({
  agentId: S.optionalKey(S.String),
  objective: S.String,
  teamId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
  tokenBudget: S.optionalKey(S.NullOr(S.Int)),
  visibility: S.optionalKey(AgentGoalVisibility),
}) {}

export class AgentCreateGoalRequest extends S.Class<AgentCreateGoalRequest>(
  'AgentCreateGoalRequest',
)({
  agentId: S.optionalKey(S.String),
  explicitRequest: S.Boolean,
  objective: S.String,
  runId: S.optionalKey(S.String),
  teamId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
  tokenBudget: S.optionalKey(S.Int),
}) {}

export class PatchGoalRequest extends S.Class<PatchGoalRequest>(
  'PatchGoalRequest',
)({
  objective: S.optionalKey(S.String),
  tokenBudget: S.optionalKey(S.NullOr(S.Int)),
  visibility: S.optionalKey(AgentGoalVisibility),
}) {}

export class VisibilityGoalRequest extends S.Class<VisibilityGoalRequest>(
  'VisibilityGoalRequest',
)({
  visibility: AgentGoalVisibility,
}) {}

export class AgentTerminalGoalRequest extends S.Class<AgentTerminalGoalRequest>(
  'AgentTerminalGoalRequest',
)({
  expectedGoalId: S.optionalKey(S.String),
  runId: S.optionalKey(S.String),
  timeDeltaSeconds: S.optionalKey(S.Int),
  tokenDelta: S.optionalKey(S.Int),
}) {}

export class AgentUpdateGoalRequest extends S.Class<AgentUpdateGoalRequest>(
  'AgentUpdateGoalRequest',
)({
  expectedGoalId: S.optionalKey(S.String),
  runId: S.optionalKey(S.String),
  status: AgentGoalTerminalStatus,
  timeDeltaSeconds: S.optionalKey(S.Int),
  tokenDelta: S.optionalKey(S.Int),
}) {}

export class OperatorCreateGoalRequest extends S.Class<OperatorCreateGoalRequest>(
  'OperatorCreateGoalRequest',
)({
  agentId: S.optionalKey(S.String),
  objective: S.String,
  userId: S.String,
  teamId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
  tokenBudget: S.optionalKey(S.NullOr(S.Int)),
  visibility: S.optionalKey(AgentGoalVisibility),
}) {}

export class OperatorScopeSelector extends S.Class<OperatorScopeSelector>(
  'OperatorScopeSelector',
)({
  agentId: S.optionalKey(S.String),
  userId: S.String,
  teamId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
}) {}

export class AgentGoalApiGoal extends S.Class<AgentGoalApiGoal>(
  'AgentGoalApiGoal',
)({
  id: S.String,
  agentId: S.String,
  userId: S.NullOr(S.String),
  teamId: S.NullOr(S.String),
  projectId: S.NullOr(S.String),
  objective: S.String,
  status: S.String,
  visibility: AgentGoalVisibility,
  currentRunId: S.NullOr(S.String),
  tokenBudget: S.NullOr(S.Int),
  tokensUsed: S.Int,
  timeUsedSeconds: S.Int,
  remainingTokens: S.NullOr(S.Int),
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  pausedAt: S.NullOr(S.String),
  blockedAt: S.NullOr(S.String),
  canEdit: S.Boolean,
  canPause: S.Boolean,
  canResume: S.Boolean,
  canMakePublic: S.Boolean,
  publicUrl: S.NullOr(S.String),
}) {}

export class PublicAgentGoalApiGoal extends S.Class<PublicAgentGoalApiGoal>(
  'PublicAgentGoalApiGoal',
)({
  id: S.String,
  agentId: S.String,
  objective: S.String,
  status: S.String,
  currentRunId: S.NullOr(S.String),
  tokenBudget: S.NullOr(S.Int),
  tokensUsed: S.Int,
  timeUsedSeconds: S.Int,
  remainingTokens: S.NullOr(S.Int),
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  publicUrl: S.String,
}) {}

class GoalUnauthorized extends S.TaggedErrorClass<GoalUnauthorized>()(
  'GoalUnauthorized',
  {},
) {}

class GoalForbidden extends S.TaggedErrorClass<GoalForbidden>()(
  'GoalForbidden',
  {},
) {}

class GoalBadRequest extends S.TaggedErrorClass<GoalBadRequest>()(
  'GoalBadRequest',
  {
    reason: S.String,
  },
) {}

class GoalSessionError extends S.TaggedErrorClass<GoalSessionError>()(
  'GoalSessionError',
  {
    error: S.Defect,
  },
) {}

class GoalTeamAccessError extends S.TaggedErrorClass<GoalTeamAccessError>()(
  'GoalTeamAccessError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

type GoalRouteError =
  | AgentGoalAccessDenied
  | AgentGoalCurrentExists
  | AgentGoalNotFound
  | AgentGoalPublicProjectionUnsafe
  | AgentGoalStaleUpdate
  | AgentGoalStorageError
  | AgentGoalToolMutationForbidden
  | AgentGoalValidationError
  | GoalBadRequest
  | GoalForbidden
  | GoalSessionError
  | GoalTeamAccessError
  | GoalUnauthorized

const queryObject = (request: Request): Record<string, string> =>
  Object.fromEntries(new URL(request.url).searchParams.entries())

const decodeUnknown = <Schema extends S.Top>(schema: Schema, value: unknown) =>
  S.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(error => new GoalBadRequest({ reason: String(error) })),
  )

const decodeJsonBody = <Schema extends S.Top>(
  request: Request,
  schema: Schema,
) =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: error =>
        new GoalBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
    })

    return yield* decodeUnknown(schema, payload)
  })

const decodeOptionalJsonBody = <Schema extends S.Top>(
  request: Request,
  schema: Schema,
  fallback: unknown,
) => {
  const contentType = request.headers.get('content-type') ?? ''

  return contentType.toLowerCase().includes('application/json')
    ? decodeJsonBody(request, schema)
    : decodeUnknown(schema, fallback)
}

const routeErrorResponse = (error: GoalRouteError) =>
  M.value(error).pipe(
    M.tags({
      AgentGoalAccessDenied: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      AgentGoalCurrentExists: ({ goalId }) =>
        noStoreJsonResponse(
          { error: 'goal_already_exists', goalId },
          { status: 409 },
        ),
      AgentGoalNotFound: () =>
        noStoreJsonResponse({ error: 'goal_not_found' }, { status: 404 }),
      AgentGoalPublicProjectionUnsafe: () =>
        noStoreJsonResponse(
          { error: 'public_projection_unsafe' },
          { status: 409 },
        ),
      AgentGoalStaleUpdate: () =>
        noStoreJsonResponse({ error: 'stale_goal_update' }, { status: 409 }),
      AgentGoalStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      AgentGoalToolMutationForbidden: ({ operation, reason }) =>
        noStoreJsonResponse(
          { error: 'goal_tool_mutation_forbidden', operation, reason },
          { status: 400 },
        ),
      AgentGoalValidationError: ({ field, message }) =>
        noStoreJsonResponse(
          { error: 'validation_error', field, message },
          { status: 400 },
        ),
      GoalBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      GoalForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      GoalSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      GoalTeamAccessError: () =>
        noStoreJsonResponse({ error: 'team_access_error' }, { status: 500 }),
      GoalUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const routeLayer = <Env extends GoalRouteEnv>(
  env: WorkerEnv<Env>,
  dependencies: GoalRouteDependencies<GoalRouteSession, Env>,
) => {
  const repositoryLayer =
    dependencies.makeRepositoryLayer?.(env) ??
    makeAgentGoalRepositoryLayerForEnv(env)
  const eventRepositoryLayer =
    dependencies.makeEventRepositoryLayer?.(env) ??
    makeAgentGoalEventRepositoryLayerForEnv(env)
  const runtimeDependencies = Layer.merge(repositoryLayer, eventRepositoryLayer)

  return Layer.mergeAll(
    runtimeDependencies,
    AgentGoalAccessServiceLayer,
    AgentGoalRuntimeServiceLive.pipe(Layer.provide(runtimeDependencies)),
    AgentPublicProjectionServiceLive.pipe(
      Layer.provide(
        Layer.merge(runtimeDependencies, AgentGoalAccessServiceLayer),
      ),
    ),
  )
}

const runRoute = <Session extends GoalRouteSession, Env extends GoalRouteEnv>(
  env: WorkerEnv<Env>,
  dependencies: GoalRouteDependencies<Session, Env>,
  effect: Effect.Effect<
    HttpResponse,
    GoalRouteError,
    | AgentGoalAccessService
    | AgentGoalEventRepository
    | AgentGoalRepository
    | AgentGoalRuntimeService
    | AgentPublicProjectionService
  >,
): GoalRouteEffect =>
  effect.pipe(
    Effect.provide(
      routeLayer(
        env,
        dependencies as GoalRouteDependencies<GoalRouteSession, Env>,
      ),
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const requireSession = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
  request: Request,
  env: WorkerEnv<Env>,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireBrowserSession(request, env, ctx),
      catch: error => new GoalSessionError({ error }),
    })

    if (session === undefined) {
      return yield* new GoalUnauthorized({})
    }

    return session
  })

const requireAdmin = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
  request: Request,
  env: WorkerEnv<Env>,
) =>
  Effect.gen(function* () {
    const ok = yield* Effect.tryPromise({
      try: () => dependencies.requireAdminApiToken(request, env),
      catch: error => new GoalSessionError({ error }),
    })

    if (!ok) {
      return yield* new GoalUnauthorized({})
    }
  })

const requireAgent = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
  request: Request,
  env: WorkerEnv<Env>,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const actor = yield* Effect.tryPromise({
      try: () => dependencies.authenticateRequestActor(request, env, ctx),
      catch: error => new GoalSessionError({ error }),
    })

    if (actor === undefined || actor.kind !== 'agent') {
      return yield* new GoalUnauthorized({})
    }

    return actor.agent
  })

const requireTeamScope = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
  env: WorkerEnv<Env>,
  userId: string,
  scope: Readonly<{
    teamId?: string | undefined
    projectId?: string | undefined
  }>,
) =>
  Effect.gen(function* () {
    if (scope.projectId !== undefined && scope.teamId === undefined) {
      return yield* new GoalBadRequest({
        reason: 'projectId requires teamId',
      })
    }

    if (scope.teamId === undefined) {
      return
    }

    const teamId = scope.teamId
    const role = yield* Effect.tryPromise({
      try: () =>
        dependencies.readActiveTeamMembershipRole(
          openAgentsDatabase(env),
          teamId,
          userId,
        ),
      catch: error =>
        new GoalTeamAccessError({
          operation: 'goal.teamMembership.read',
          error,
        }),
    })

    if (role === undefined) {
      return yield* new GoalForbidden({})
    }

    const projectId = scope.projectId

    if (projectId !== undefined) {
      const project = yield* Effect.tryPromise({
        try: () =>
          dependencies.readActiveTeamProject(
            openAgentsDatabase(env),
            teamId,
            projectId,
          ),
        catch: error =>
          new GoalTeamAccessError({
            operation: 'goal.teamProject.read',
            error,
          }),
      })

      if (project === undefined) {
        return yield* new AgentGoalNotFound({ goalId: projectId })
      }
    }
  })

const browserScope = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
  env: WorkerEnv<Env>,
  userId: string,
  selector: GoalScopeSelector | CreateGoalRequest,
) =>
  Effect.gen(function* () {
    yield* requireTeamScope(dependencies, env, userId, selector)

    return {
      agentId: selector.agentId ?? DEFAULT_BROWSER_AGENT_ID,
      userId,
      teamId: selector.teamId ?? null,
      projectId: selector.projectId ?? null,
    } satisfies AgentGoalScope
  })

const operatorScope = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
  env: WorkerEnv<Env>,
  selector: OperatorScopeSelector | OperatorCreateGoalRequest,
) =>
  Effect.gen(function* () {
    yield* requireTeamScope(dependencies, env, selector.userId, selector)

    return {
      agentId: selector.agentId ?? DEFAULT_BROWSER_AGENT_ID,
      userId: selector.userId,
      teamId: selector.teamId ?? null,
      projectId: selector.projectId ?? null,
    } satisfies AgentGoalScope
  })

const remainingTokens = (goal: AgentGoalRecord): number | null =>
  goal.tokenBudget === null
    ? null
    : Math.max(0, goal.tokenBudget - goal.tokensUsed)

const remainingPublicTokens = (goal: PublicAgentGoalRecord): number | null =>
  goal.tokenBudget === null
    ? null
    : Math.max(0, goal.tokenBudget - goal.tokensUsed)

const publicUrl = (request: Request, goalId: string): string =>
  `${new URL(request.url).origin}/api/public/goals/${encodeURIComponent(goalId)}`

const privateGoalDto = (
  request: Request,
  goal: AgentGoalRecord,
  actor: AgentGoalActor,
  access: AgentGoalAccessServiceShape,
): AgentGoalApiGoal => {
  const canWrite = access.canWrite(actor, goal)

  return new AgentGoalApiGoal({
    id: goal.id,
    agentId: goal.agentId,
    userId: goal.userId,
    teamId: goal.teamId,
    projectId: goal.projectId,
    objective: goal.objective,
    status: goal.status,
    visibility: goal.visibility,
    currentRunId: goal.currentRunId,
    tokenBudget: goal.tokenBudget,
    tokensUsed: goal.tokensUsed,
    timeUsedSeconds: goal.timeUsedSeconds,
    remainingTokens: remainingTokens(goal),
    createdAt: goal.createdAt,
    updatedAt: goal.updatedAt,
    completedAt: goal.completedAt,
    pausedAt: goal.pausedAt,
    blockedAt: goal.blockedAt,
    canEdit: canWrite,
    canPause: canWrite && goal.status === 'active',
    canResume:
      canWrite &&
      (goal.status === 'paused' ||
        goal.status === 'blocked' ||
        goal.status === 'usage_limited' ||
        goal.status === 'budget_limited'),
    canMakePublic: canWrite && goal.visibility !== 'public',
    publicUrl:
      goal.visibility === 'public' ? publicUrl(request, goal.id) : null,
  })
}

const publicGoalDto = (
  request: Request,
  goal: PublicAgentGoalRecord,
): PublicAgentGoalApiGoal =>
  new PublicAgentGoalApiGoal({
    id: goal.id,
    agentId: goal.agentId,
    objective: goal.objective,
    status: goal.status,
    currentRunId: goal.currentRunId,
    tokenBudget: goal.tokenBudget,
    tokensUsed: goal.tokensUsed,
    timeUsedSeconds: goal.timeUsedSeconds,
    remainingTokens: remainingPublicTokens(goal),
    createdAt: goal.createdAt,
    updatedAt: goal.updatedAt,
    completedAt: goal.completedAt,
    publicUrl: publicUrl(request, goal.id),
  })

const actorForGoal = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
  env: WorkerEnv<Env>,
  userId: string,
  goal: AgentGoalRecord,
) =>
  Effect.gen(function* () {
    if (goal.teamId === null) {
      return { userId } satisfies AgentGoalActor
    }

    const teamId = goal.teamId
    const role = yield* Effect.tryPromise({
      try: () =>
        dependencies.readActiveTeamMembershipRole(
          openAgentsDatabase(env),
          teamId,
          userId,
        ),
      catch: error =>
        new GoalTeamAccessError({
          operation: 'goal.teamMembership.read',
          error,
        }),
    })

    return {
      userId,
      teamIds: role === undefined ? [] : [teamId],
    } satisfies AgentGoalActor
  })

const respondPrivateGoal = (
  request: Request,
  goal: AgentGoalRecord,
  actor: AgentGoalActor,
  access: AgentGoalAccessServiceShape,
) => noStoreJsonResponse({ goal: privateGoalDto(request, goal, actor, access) })

const respondMaybePrivateGoal = (
  request: Request,
  goal: AgentGoalRecord | undefined,
  actor: AgentGoalActor,
  access: AgentGoalAccessServiceShape,
) =>
  noStoreJsonResponse({
    goal:
      goal === undefined ? null : privateGoalDto(request, goal, actor, access),
  })

const respondAgentToolResult = (
  request: Request,
  result: AgentGoalToolResult,
  actor: AgentGoalActor,
  access: AgentGoalAccessServiceShape,
) =>
  noStoreJsonResponse({
    completionBudgetReport: result.completionBudgetReport ?? null,
    goal:
      result.goal === null
        ? null
        : privateGoalDto(request, result.goal, actor, access),
    remainingTokens: result.remainingTokens,
  })

const applyPatch = (goal: AgentGoalRecord, body: PatchGoalRequest) =>
  Effect.gen(function* () {
    const repository = yield* AgentGoalRepository

    return yield* (
      body.objective === undefined
        ? Effect.succeed(goal)
        : repository.editObjective(goal.id, body.objective, goal.id)
    ).pipe(
      Effect.flatMap(updated =>
        body.tokenBudget === undefined
          ? Effect.succeed(updated)
          : body.tokenBudget === null
            ? repository.clearTokenBudget(updated.id, updated.id)
            : repository.setTokenBudget(
                updated.id,
                body.tokenBudget,
                updated.id,
              ),
      ),
      Effect.flatMap(updated =>
        body.visibility === undefined
          ? Effect.succeed(updated)
          : repository.changeVisibility(
              updated.id,
              body.visibility,
              updated.id,
            ),
      ),
    )
  })

export const makeAgentGoalRoutes = <
  Session extends GoalRouteSession,
  Env extends GoalRouteEnv,
>(
  dependencies: GoalRouteDependencies<Session, Env>,
) => {
  const browserCurrentResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const selector = yield* decodeUnknown(
          GoalScopeSelector,
          queryObject(request),
        )
        const scope = yield* browserScope(
          dependencies,
          env,
          session.user.userId,
          selector,
        )
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.getCurrent(scope)
        const actor = { userId: session.user.userId } satisfies AgentGoalActor

        return dependencies.appendRefreshedSessionCookies(
          respondMaybePrivateGoal(request, goal, actor, access),
          session,
        )
      }),
    )

  const browserCreateResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const body = yield* decodeJsonBody(request, CreateGoalRequest)
        const scope = yield* browserScope(
          dependencies,
          env,
          session.user.userId,
          body,
        )
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.setGoal({
          ...scope,
          objective: body.objective,
          tokenBudget: body.tokenBudget,
          visibility: body.visibility,
        })
        const actor = { userId: session.user.userId } satisfies AgentGoalActor
        yield* publishGoalSync(env, ctx, goal, session.user.userId)

        return dependencies.appendRefreshedSessionCookies(
          respondPrivateGoal(request, goal, actor, access),
          session,
        )
      }),
    )

  const browserGoalResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
    goalId: string,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.getById(goalId)
        const actor = yield* actorForGoal(
          dependencies,
          env,
          session.user.userId,
          goal,
        )

        yield* access.requireRead(actor, goal)

        if (request.method === 'GET') {
          return dependencies.appendRefreshedSessionCookies(
            respondPrivateGoal(request, goal, actor, access),
            session,
          )
        }

        const body = yield* decodeJsonBody(request, PatchGoalRequest)
        yield* access.requireWrite(actor, goal)
        const updated = yield* applyPatch(goal, body)
        yield* publishGoalSync(env, ctx, updated, session.user.userId)

        return dependencies.appendRefreshedSessionCookies(
          respondPrivateGoal(request, updated, actor, access),
          session,
        )
      }),
    )

  const browserActionResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
    goalId: string,
    action: 'pause' | 'resume' | 'clear' | 'visibility',
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const session = yield* requireSession(dependencies, request, env, ctx)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.getById(goalId)
        const actor = yield* actorForGoal(
          dependencies,
          env,
          session.user.userId,
          goal,
        )
        yield* access.requireWrite(actor, goal)

        const updated = yield* {
          pause: repository.setStatus(goal.id, 'paused', goal.id),
          resume: repository.setStatus(goal.id, 'active', goal.id),
          clear: repository.archiveGoal(goal.id, goal.id),
          visibility: decodeJsonBody(request, VisibilityGoalRequest).pipe(
            Effect.flatMap(body =>
              repository.changeVisibility(goal.id, body.visibility, goal.id),
            ),
          ),
        }[action]
        yield* publishGoalSync(env, ctx, updated, session.user.userId)

        return dependencies.appendRefreshedSessionCookies(
          respondPrivateGoal(request, updated, actor, access),
          session,
        )
      }),
    )

  const operatorCurrentResponse = (request: Request, env: WorkerEnv<Env>) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        yield* requireAdmin(dependencies, request, env)
        const selector = yield* decodeUnknown(
          OperatorScopeSelector,
          queryObject(request),
        )
        const scope = yield* operatorScope(dependencies, env, selector)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.getCurrent(scope)
        const actor = { operator: true } satisfies AgentGoalActor

        return respondMaybePrivateGoal(request, goal, actor, access)
      }),
    )

  const operatorCreateResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        yield* requireAdmin(dependencies, request, env)
        const body = yield* decodeJsonBody(request, OperatorCreateGoalRequest)
        const scope = yield* operatorScope(dependencies, env, body)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.setGoal({
          ...scope,
          objective: body.objective,
          tokenBudget: body.tokenBudget,
          visibility: body.visibility,
        })
        const actor = { operator: true } satisfies AgentGoalActor
        yield* publishGoalSync(env, ctx, goal, 'operator')

        return respondPrivateGoal(request, goal, actor, access)
      }),
    )

  const operatorGoalResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
    goalId: string,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        yield* requireAdmin(dependencies, request, env)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.getById(goalId)
        const actor = { operator: true } satisfies AgentGoalActor

        if (request.method === 'GET') {
          return respondPrivateGoal(request, goal, actor, access)
        }

        const body = yield* decodeJsonBody(request, PatchGoalRequest)
        const updated = yield* applyPatch(goal, body)
        yield* publishGoalSync(env, ctx, updated, 'operator')

        return respondPrivateGoal(request, updated, actor, access)
      }),
    )

  const operatorActionResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
    goalId: string,
    action: 'pause' | 'resume' | 'clear' | 'visibility',
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        yield* requireAdmin(dependencies, request, env)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.getById(goalId)
        const actor = { operator: true } satisfies AgentGoalActor
        const updated = yield* {
          pause: repository.setStatus(goal.id, 'paused', goal.id),
          resume: repository.setStatus(goal.id, 'active', goal.id),
          clear: repository.archiveGoal(goal.id, goal.id),
          visibility: decodeJsonBody(request, VisibilityGoalRequest).pipe(
            Effect.flatMap(body =>
              repository.changeVisibility(goal.id, body.visibility, goal.id),
            ),
          ),
        }[action]
        yield* publishGoalSync(env, ctx, updated, 'operator')

        return respondPrivateGoal(request, updated, actor, access)
      }),
    )

  const agentCurrentResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const agent = yield* requireAgent(dependencies, request, env, ctx)
        const selector = yield* decodeUnknown(
          GoalScopeSelector,
          queryObject(request),
        )
        const access = yield* AgentGoalAccessService
        const runtime = yield* AgentGoalRuntimeService
        const scope = {
          agentId: selector.agentId ?? agent.user.id,
          userId: agent.user.id,
          teamId: selector.teamId ?? null,
          projectId: selector.projectId ?? null,
        } satisfies AgentGoalScope
        const result = yield* runtime.getGoal({ scope })
        const actor = { userId: agent.user.id } satisfies AgentGoalActor

        return respondAgentToolResult(request, result, actor, access)
      }),
    )

  const agentCreateResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const agent = yield* requireAgent(dependencies, request, env, ctx)
        const body = yield* decodeJsonBody(request, AgentCreateGoalRequest)
        const access = yield* AgentGoalAccessService
        const runtime = yield* AgentGoalRuntimeService
        const result = yield* runtime.createGoal({
          agentId: body.agentId ?? agent.user.id,
          explicitRequest: body.explicitRequest,
          objective: body.objective,
          projectId: body.projectId ?? null,
          runId: body.runId,
          teamId: body.teamId ?? null,
          tokenBudget: body.tokenBudget,
          userId: agent.user.id,
        })
        const actor = { userId: agent.user.id } satisfies AgentGoalActor
        if (result.goal !== null) {
          yield* publishGoalSync(env, ctx, result.goal, agent.user.id)
        }

        return respondAgentToolResult(request, result, actor, access)
      }),
    )

  const agentGoalResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
    goalId: string,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const agent = yield* requireAgent(dependencies, request, env, ctx)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const goal = yield* repository.getById(goalId)
        const actor = { userId: agent.user.id } satisfies AgentGoalActor
        yield* access.requireRead(actor, goal)

        return respondPrivateGoal(request, goal, actor, access)
      }),
    )

  const agentTerminalResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
    goalId: string,
    status: 'complete' | 'blocked',
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const agent = yield* requireAgent(dependencies, request, env, ctx)
        const body = yield* decodeOptionalJsonBody(
          request,
          AgentTerminalGoalRequest,
          {},
        )
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const runtime = yield* AgentGoalRuntimeService
        const goal = yield* repository.getById(goalId)
        const actor = { userId: agent.user.id } satisfies AgentGoalActor
        yield* access.requireWrite(actor, goal)
        const result = yield* runtime.updateGoal({
          expectedGoalId: body.expectedGoalId ?? goal.id,
          goalId: goal.id,
          runId: body.runId,
          status,
          timeDeltaSeconds: body.timeDeltaSeconds,
          tokenDelta: body.tokenDelta,
        })
        if (result.goal !== null) {
          yield* publishGoalSync(env, ctx, result.goal, agent.user.id)
        }

        return respondAgentToolResult(request, result, actor, access)
      }),
    )

  const agentUpdateResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    ctx: ExecutionContext,
    goalId: string,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const agent = yield* requireAgent(dependencies, request, env, ctx)
        const body = yield* decodeJsonBody(request, AgentUpdateGoalRequest)
        const repository = yield* AgentGoalRepository
        const access = yield* AgentGoalAccessService
        const runtime = yield* AgentGoalRuntimeService
        const goal = yield* repository.getById(goalId)
        const actor = { userId: agent.user.id } satisfies AgentGoalActor
        yield* access.requireWrite(actor, goal)
        const result = yield* runtime.updateGoal({
          expectedGoalId: body.expectedGoalId ?? goal.id,
          goalId: goal.id,
          runId: body.runId,
          status: body.status,
          timeDeltaSeconds: body.timeDeltaSeconds,
          tokenDelta: body.tokenDelta,
        })
        if (result.goal !== null) {
          yield* publishGoalSync(env, ctx, result.goal, agent.user.id)
        }

        return respondAgentToolResult(request, result, actor, access)
      }),
    )

  const publicAgentGoalResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    agentId: string,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const projection = yield* AgentPublicProjectionService
        const snapshot = yield* projection.currentAgentGoal(agentId)

        return noStoreJsonResponse({
          agentId,
          events: snapshot.events,
          goal:
            snapshot.goal === null
              ? null
              : publicGoalDto(request, snapshot.goal),
        })
      }),
    )

  const publicGoalResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    goalId: string,
  ) =>
    runRoute(
      env,
      dependencies,
      Effect.gen(function* () {
        const projection = yield* AgentPublicProjectionService
        const snapshot = yield* projection.goalSnapshot(goalId)

        return noStoreJsonResponse({
          agentId: snapshot.agentId,
          events: snapshot.events,
          goal:
            snapshot.goal === null
              ? null
              : publicGoalDto(request, snapshot.goal),
        })
      }),
    )

  return {
    routeAgentGoalRequest: (
      request: Request,
      env: WorkerEnv<Env>,
      ctx: ExecutionContext,
    ): GoalRouteEffect | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/autopilot/goals/current') {
        return request.method === 'GET'
          ? browserCurrentResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      if (url.pathname === '/api/autopilot/goals') {
        return request.method === 'POST'
          ? browserCreateResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      const browserActionMatch =
        /^\/api\/autopilot\/goals\/([^/]+)\/(pause|resume|clear|visibility)$/.exec(
          url.pathname,
        )

      if (browserActionMatch !== null) {
        return request.method === 'POST'
          ? browserActionResponse(
              request,
              env,
              ctx,
              decodeURIComponent(browserActionMatch[1] ?? ''),
              browserActionMatch[2] as
                | 'pause'
                | 'resume'
                | 'clear'
                | 'visibility',
            )
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      const browserGoalMatch = /^\/api\/autopilot\/goals\/([^/]+)$/.exec(
        url.pathname,
      )

      if (browserGoalMatch !== null) {
        return request.method === 'GET' || request.method === 'PATCH'
          ? browserGoalResponse(
              request,
              env,
              ctx,
              decodeURIComponent(browserGoalMatch[1] ?? ''),
            )
          : Effect.succeed(methodNotAllowed(['GET', 'PATCH']))
      }

      if (url.pathname === '/api/operator/autopilot/goals/current') {
        return request.method === 'GET'
          ? operatorCurrentResponse(request, env)
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      if (url.pathname === '/api/operator/autopilot/goals') {
        return request.method === 'POST'
          ? operatorCreateResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      const operatorGoalMatch =
        /^\/api\/operator\/autopilot\/goals\/([^/]+)$/.exec(url.pathname)

      if (operatorGoalMatch !== null) {
        return request.method === 'GET' || request.method === 'PATCH'
          ? operatorGoalResponse(
              request,
              env,
              ctx,
              decodeURIComponent(operatorGoalMatch[1] ?? ''),
            )
          : Effect.succeed(methodNotAllowed(['GET', 'PATCH']))
      }

      const operatorActionMatch =
        /^\/api\/operator\/autopilot\/goals\/([^/]+)\/(pause|resume|clear|visibility)$/.exec(
          url.pathname,
        )

      if (operatorActionMatch !== null) {
        return request.method === 'POST'
          ? operatorActionResponse(
              request,
              env,
              ctx,
              decodeURIComponent(operatorActionMatch[1] ?? ''),
              operatorActionMatch[2] as
                | 'pause'
                | 'resume'
                | 'clear'
                | 'visibility',
            )
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      if (url.pathname === '/api/agents/goals/current') {
        return request.method === 'GET'
          ? agentCurrentResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      if (url.pathname === '/api/agents/goals') {
        return request.method === 'POST'
          ? agentCreateResponse(request, env, ctx)
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      const agentTerminalMatch =
        /^\/api\/agents\/goals\/([^/]+)\/(complete|blocked)$/.exec(url.pathname)

      if (agentTerminalMatch !== null) {
        return request.method === 'POST'
          ? agentTerminalResponse(
              request,
              env,
              ctx,
              decodeURIComponent(agentTerminalMatch[1] ?? ''),
              agentTerminalMatch[2] as 'complete' | 'blocked',
            )
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      const agentUpdateMatch = /^\/api\/agents\/goals\/([^/]+)\/update$/.exec(
        url.pathname,
      )

      if (agentUpdateMatch !== null) {
        return request.method === 'POST'
          ? agentUpdateResponse(
              request,
              env,
              ctx,
              decodeURIComponent(agentUpdateMatch[1] ?? ''),
            )
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      const agentGoalMatch = /^\/api\/agents\/goals\/([^/]+)$/.exec(
        url.pathname,
      )

      if (agentGoalMatch !== null) {
        return request.method === 'GET'
          ? agentGoalResponse(
              request,
              env,
              ctx,
              decodeURIComponent(agentGoalMatch[1] ?? ''),
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      const publicAgentGoalMatch =
        /^\/api\/public\/agents\/([^/]+)\/(?:goal|current-goal)$/.exec(
          url.pathname,
        )

      if (publicAgentGoalMatch !== null) {
        return request.method === 'GET'
          ? publicAgentGoalResponse(
              request,
              env,
              decodeURIComponent(publicAgentGoalMatch[1] ?? ''),
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      const publicGoalMatch = /^\/api\/public\/goals\/([^/]+)$/.exec(
        url.pathname,
      )

      if (publicGoalMatch !== null) {
        return request.method === 'GET'
          ? publicGoalResponse(
              request,
              env,
              decodeURIComponent(publicGoalMatch[1] ?? ''),
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      const publicGoalSnapshotMatch =
        /^\/api\/public\/goals\/([^/]+)\/snapshot$/.exec(url.pathname)

      if (publicGoalSnapshotMatch !== null) {
        return request.method === 'GET'
          ? publicGoalResponse(
              request,
              env,
              decodeURIComponent(publicGoalSnapshotMatch[1] ?? ''),
            )
          : Effect.succeed(methodNotAllowed(['GET']))
      }

      return undefined
    },
  }
}
