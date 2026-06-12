import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Context, Effect, Layer, Schema as S } from 'effect'

import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const AgentGoalStatus = S.Literals([
  'active',
  'paused',
  'blocked',
  'usage_limited',
  'budget_limited',
  'complete',
])
export type AgentGoalStatus = typeof AgentGoalStatus.Type

export const AgentGoalVisibility = S.Literals([
  'private',
  'team',
  'public',
])
export type AgentGoalVisibility = typeof AgentGoalVisibility.Type

export class AgentGoalRecord extends S.Class<AgentGoalRecord>(
  'AgentGoalRecord',
)({
  id: S.String,
  agentId: S.String,
  userId: S.NullOr(S.String),
  teamId: S.NullOr(S.String),
  projectId: S.NullOr(S.String),
  objective: S.String,
  status: AgentGoalStatus,
  visibility: AgentGoalVisibility,
  currentRunId: S.NullOr(S.String),
  tokenBudget: S.NullOr(S.Int),
  tokensUsed: S.Int,
  timeUsedSeconds: S.Int,
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  pausedAt: S.NullOr(S.String),
  blockedAt: S.NullOr(S.String),
  archivedAt: S.NullOr(S.String),
}) {}

export class PublicAgentGoalRecord extends S.Class<PublicAgentGoalRecord>(
  'PublicAgentGoalRecord',
)({
  id: S.String,
  agentId: S.String,
  objective: S.String,
  status: AgentGoalStatus,
  currentRunId: S.NullOr(S.String),
  tokenBudget: S.NullOr(S.Int),
  tokensUsed: S.Int,
  timeUsedSeconds: S.Int,
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
}) {}

export class AgentGoalStorageError extends S.TaggedErrorClass<AgentGoalStorageError>()(
  'AgentGoalStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AgentGoalValidationError extends S.TaggedErrorClass<AgentGoalValidationError>()(
  'AgentGoalValidationError',
  {
    field: S.String,
    message: S.String,
  },
) {}

export class AgentGoalNotFound extends S.TaggedErrorClass<AgentGoalNotFound>()(
  'AgentGoalNotFound',
  {
    goalId: S.String,
  },
) {}

export class AgentGoalStaleUpdate extends S.TaggedErrorClass<AgentGoalStaleUpdate>()(
  'AgentGoalStaleUpdate',
  {
    expectedGoalId: S.String,
    actualGoalId: S.NullOr(S.String),
  },
) {}

export class AgentGoalAccessDenied extends S.TaggedErrorClass<AgentGoalAccessDenied>()(
  'AgentGoalAccessDenied',
  {
    actorUserId: S.NullOr(S.String),
    goalId: S.String,
  },
) {}

export class AgentGoalPublicProjectionUnsafe extends S.TaggedErrorClass<AgentGoalPublicProjectionUnsafe>()(
  'AgentGoalPublicProjectionUnsafe',
  {
    goalId: S.String,
  },
) {}

export const AgentGoalError = S.Union([
  AgentGoalStorageError,
  AgentGoalValidationError,
  AgentGoalNotFound,
  AgentGoalStaleUpdate,
  AgentGoalAccessDenied,
  AgentGoalPublicProjectionUnsafe,
])
export type AgentGoalError = typeof AgentGoalError.Type

type AgentGoalRow = Readonly<{
  id: string
  agent_id: string
  user_id: string | null
  team_id: string | null
  project_id: string | null
  objective: string
  status: AgentGoalStatus
  visibility: AgentGoalVisibility
  current_run_id: string | null
  token_budget: number | null
  tokens_used: number
  time_used_seconds: number
  created_at: string
  updated_at: string
  completed_at: string | null
  paused_at: string | null
  blocked_at: string | null
  archived_at: string | null
}>

export type AgentGoalScope = Readonly<{
  agentId: string
  userId?: string | null | undefined
  teamId?: string | null | undefined
  projectId?: string | null | undefined
}>

export type AgentGoalRuntime = Readonly<{
  nowIso: () => string
  randomId: (prefix: string) => string
}>

export const systemAgentGoalRuntime: AgentGoalRuntime = {
  nowIso: currentIsoTimestamp,
  randomId: compactRandomId,
}

export type CreateAgentGoalInput = AgentGoalScope &
  Readonly<{
    id?: string | undefined
    objective: string
    tokenBudget?: number | null | undefined
    visibility?: AgentGoalVisibility | undefined
  }>

export type AccountAgentGoalUsageInput = Readonly<{
  expectedGoalId?: string | undefined
  goalId: string
  timeDeltaSeconds?: number | undefined
  tokenDelta?: number | undefined
}>

export type AttachAgentGoalRunInput = Readonly<{
  expectedGoalId?: string | undefined
  goalId: string
  runId: string
}>

export type AgentGoalActor = Readonly<{
  teamIds?: ReadonlyArray<string> | undefined
  userId?: string | undefined
  operator?: boolean | undefined
}>

export type AgentGoalRepositoryShape = Readonly<{
  accountUsage: (
    input: AccountAgentGoalUsageInput,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  archiveGoal: (
    goalId: string,
    expectedGoalId?: string,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  attachRun: (
    input: AttachAgentGoalRunInput,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  changeVisibility: (
    goalId: string,
    visibility: AgentGoalVisibility,
    expectedGoalId?: string,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  clearTokenBudget: (
    goalId: string,
    expectedGoalId?: string,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  editObjective: (
    goalId: string,
    objective: string,
    expectedGoalId?: string,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  getById: (
    goalId: string,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  getCurrent: (
    scope: AgentGoalScope,
  ) => Effect.Effect<AgentGoalRecord | undefined, AgentGoalStorageError>
  getPublicCurrentByAgentId: (
    agentId: string,
  ) => Effect.Effect<AgentGoalRecord | undefined, AgentGoalStorageError>
  setGoal: (
    input: CreateAgentGoalInput,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  setStatus: (
    goalId: string,
    status: AgentGoalStatus,
    expectedGoalId?: string,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
  setTokenBudget: (
    goalId: string,
    tokenBudget: number,
    expectedGoalId?: string,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalError>
}>

export type AgentGoalAccessServiceShape = Readonly<{
  canRead: (actor: AgentGoalActor, goal: AgentGoalRecord) => boolean
  canWrite: (actor: AgentGoalActor, goal: AgentGoalRecord) => boolean
  publicProjection: (
    goal: AgentGoalRecord,
  ) => Effect.Effect<
    PublicAgentGoalRecord,
    AgentGoalAccessDenied | AgentGoalPublicProjectionUnsafe
  >
  requireRead: (
    actor: AgentGoalActor,
    goal: AgentGoalRecord,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalAccessDenied>
  requireWrite: (
    actor: AgentGoalActor,
    goal: AgentGoalRecord,
  ) => Effect.Effect<AgentGoalRecord, AgentGoalAccessDenied>
}>

export class AgentGoalRepository extends Context.Service<
  AgentGoalRepository,
  AgentGoalRepositoryShape
>()('@openagentsinc/AgentGoalRepository') {}

export class AgentGoalAccessService extends Context.Service<
  AgentGoalAccessService,
  AgentGoalAccessServiceShape
>()('@openagentsinc/AgentGoalAccessService') {}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AgentGoalStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new AgentGoalStorageError({ operation, error }),
  })

const toRecord = (row: AgentGoalRow): AgentGoalRecord =>
  new AgentGoalRecord({
    id: row.id,
    agentId: row.agent_id,
    userId: row.user_id,
    teamId: row.team_id,
    projectId: row.project_id,
    objective: row.objective,
    status: row.status,
    visibility: row.visibility,
    currentRunId: row.current_run_id,
    tokenBudget: row.token_budget,
    tokensUsed: row.tokens_used,
    timeUsedSeconds: row.time_used_seconds,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    completedAt: row.completed_at,
    pausedAt: row.paused_at,
    blockedAt: row.blocked_at,
    archivedAt: row.archived_at,
  })

const currentScopeBindings = (
  scope: AgentGoalScope,
): ReadonlyArray<string | null> => [
  scope.agentId,
  scope.userId ?? null,
  scope.teamId ?? null,
  scope.projectId ?? null,
]

const normalizeObjective = (
  objective: string,
): Effect.Effect<string, AgentGoalValidationError> => {
  const normalized = objective.replace(/\s+/g, ' ').trim()

  return normalized === ''
    ? Effect.fail(
        new AgentGoalValidationError({
          field: 'objective',
          message: 'Goal objective is required.',
        }),
      )
    : Effect.succeed(normalized.slice(0, 8_000))
}

const normalizeTokenBudget = (
  tokenBudget: number | null | undefined,
): Effect.Effect<number | null, AgentGoalValidationError> => {
  if (tokenBudget === null || tokenBudget === undefined) {
    return Effect.succeed(null)
  }

  if (!Number.isInteger(tokenBudget) || tokenBudget <= 0) {
    return Effect.fail(
      new AgentGoalValidationError({
        field: 'tokenBudget',
        message: 'Token budget must be a positive integer.',
      }),
    )
  }

  return Effect.succeed(tokenBudget)
}

const nonNegativeInteger = (
  field: string,
  value: number | undefined,
): Effect.Effect<number, AgentGoalValidationError> => {
  if (value === undefined) {
    return Effect.succeed(0)
  }

  if (!Number.isFinite(value)) {
    return Effect.fail(
      new AgentGoalValidationError({
        field,
        message: `${field} must be a finite number.`,
      }),
    )
  }

  return Effect.succeed(Math.max(0, Math.trunc(value)))
}

const terminalTimestamps = (
  status: AgentGoalStatus,
  now: string,
): Readonly<{
  blockedAt: string | null
  completedAt: string | null
  pausedAt: string | null
}> => ({
  blockedAt: status === 'blocked' ? now : null,
  completedAt: status === 'complete' ? now : null,
  pausedAt: status === 'paused' ? now : null,
})

const statusAfterUsage = (
  status: AgentGoalStatus,
  tokenBudget: number | null,
  tokensUsed: number,
): AgentGoalStatus =>
  status === 'active' && tokenBudget !== null && tokensUsed >= tokenBudget
    ? 'budget_limited'
    : status

export const publicAgentGoalRecord = (
  goal: AgentGoalRecord,
): PublicAgentGoalRecord =>
  new PublicAgentGoalRecord({
    id: goal.id,
    agentId: goal.agentId,
    objective: goal.objective,
    status: goal.status,
    currentRunId: goal.currentRunId,
    tokenBudget: goal.tokenBudget,
    tokensUsed: goal.tokensUsed,
    timeUsedSeconds: goal.timeUsedSeconds,
    createdAt: goal.createdAt,
    updatedAt: goal.updatedAt,
    completedAt: goal.completedAt,
  })

export const makeD1AgentGoalRepository = (
  db: D1Database,
  runtime: AgentGoalRuntime = systemAgentGoalRuntime,
): AgentGoalRepositoryShape => {
  const readById = (
    goalId: string,
  ): Effect.Effect<AgentGoalRecord | undefined, AgentGoalStorageError> =>
    d1Effect('agentGoals.readById', () =>
      db
        .prepare(
          `SELECT *
           FROM agent_goals
           WHERE id = ?
           LIMIT 1`,
        )
        .bind(goalId)
        .first<AgentGoalRow>(),
    ).pipe(Effect.map(row => (row === null ? undefined : toRecord(row))))

  const requireGoal = (
    goalId: string,
  ): Effect.Effect<AgentGoalRecord, AgentGoalError> =>
    Effect.gen(function* () {
      const goal = yield* readById(goalId)

      if (goal === undefined || goal.archivedAt !== null) {
        return yield* new AgentGoalNotFound({ goalId })
      }

      return goal
    })

  const requireExpectedGoal = (
    goalId: string,
    expectedGoalId: string | undefined,
  ): Effect.Effect<AgentGoalRecord, AgentGoalError> =>
    Effect.gen(function* () {
      const goal = yield* requireGoal(goalId)

      if (expectedGoalId !== undefined && expectedGoalId !== goal.id) {
        return yield* new AgentGoalStaleUpdate({
          expectedGoalId,
          actualGoalId: goal.id,
        })
      }

      return goal
    })

  const readUpdatedGoal = (
    goalId: string,
  ): Effect.Effect<AgentGoalRecord, AgentGoalError> => requireGoal(goalId)

  return {
    accountUsage: Effect.fn('AgentGoalRepository.accountUsage')(
      function* (input: AccountAgentGoalUsageInput) {
        const goal = yield* requireExpectedGoal(
          input.goalId,
          input.expectedGoalId,
        )
        const tokenDelta = yield* nonNegativeInteger(
          'tokenDelta',
          input.tokenDelta,
        )
        const timeDeltaSeconds = yield* nonNegativeInteger(
          'timeDeltaSeconds',
          input.timeDeltaSeconds,
        )
        const tokensUsed = goal.tokensUsed + tokenDelta
        const timeUsedSeconds = goal.timeUsedSeconds + timeDeltaSeconds
        const status = statusAfterUsage(
          goal.status,
          goal.tokenBudget,
          tokensUsed,
        )
        const now = runtime.nowIso()

        yield* d1Effect('agentGoals.accountUsage', () =>
          db
            .prepare(
              `UPDATE agent_goals
               SET tokens_used = ?,
                   time_used_seconds = ?,
                   status = ?,
                   updated_at = ?
               WHERE id = ?
                 AND archived_at IS NULL`,
            )
            .bind(tokensUsed, timeUsedSeconds, status, now, goal.id)
            .run(),
        )

        return yield* readUpdatedGoal(goal.id)
      },
    ),

    archiveGoal: Effect.fn('AgentGoalRepository.archiveGoal')(
      function* (goalId: string, expectedGoalId?: string) {
        const goal = yield* requireExpectedGoal(goalId, expectedGoalId)
        const now = runtime.nowIso()

        yield* d1Effect('agentGoals.archiveGoal', () =>
          db
            .prepare(
              `UPDATE agent_goals
               SET archived_at = ?,
                   updated_at = ?
               WHERE id = ?
                 AND archived_at IS NULL`,
            )
            .bind(now, now, goal.id)
            .run(),
        )

        return yield* readById(goal.id).pipe(
          Effect.flatMap(updated =>
            updated === undefined
              ? Effect.fail(new AgentGoalNotFound({ goalId: goal.id }))
              : Effect.succeed(updated),
          ),
        )
      },
    ),

    attachRun: Effect.fn('AgentGoalRepository.attachRun')(
      function* (input: AttachAgentGoalRunInput) {
        const goal = yield* requireExpectedGoal(
          input.goalId,
          input.expectedGoalId,
        )
        const now = runtime.nowIso()

        yield* d1Effect('agentGoals.attachRun', () =>
          db.batch([
            db
              .prepare(
                `UPDATE agent_goals
                 SET current_run_id = ?,
                     updated_at = ?
                 WHERE id = ?
                   AND archived_at IS NULL`,
              )
              .bind(input.runId, now, goal.id),
            db
              .prepare(
                `UPDATE agent_runs
                 SET goal_id = ?,
                     updated_at = ?
                 WHERE id = ?`,
              )
              .bind(goal.id, now, input.runId),
          ]),
        )

        return yield* readUpdatedGoal(goal.id)
      },
    ),

    changeVisibility: Effect.fn('AgentGoalRepository.changeVisibility')(
      function* (
        goalId: string,
        visibility: AgentGoalVisibility,
        expectedGoalId?: string,
      ) {
        const goal = yield* requireExpectedGoal(goalId, expectedGoalId)
        const now = runtime.nowIso()

        yield* d1Effect('agentGoals.changeVisibility', () =>
          db
            .prepare(
              `UPDATE agent_goals
               SET visibility = ?,
                   updated_at = ?
               WHERE id = ?
                 AND archived_at IS NULL`,
            )
            .bind(visibility, now, goal.id)
            .run(),
        )

        return yield* readUpdatedGoal(goal.id)
      },
    ),

    clearTokenBudget: Effect.fn('AgentGoalRepository.clearTokenBudget')(
      function* (goalId: string, expectedGoalId?: string) {
        const goal = yield* requireExpectedGoal(goalId, expectedGoalId)
        const now = runtime.nowIso()

        yield* d1Effect('agentGoals.clearTokenBudget', () =>
          db
            .prepare(
              `UPDATE agent_goals
               SET token_budget = NULL,
                   status = CASE WHEN status = 'budget_limited' THEN 'active' ELSE status END,
                   updated_at = ?
               WHERE id = ?
                 AND archived_at IS NULL`,
            )
            .bind(now, goal.id)
            .run(),
        )

        return yield* readUpdatedGoal(goal.id)
      },
    ),

    editObjective: Effect.fn('AgentGoalRepository.editObjective')(
      function* (
        goalId: string,
        objective: string,
        expectedGoalId?: string,
      ) {
        const goal = yield* requireExpectedGoal(goalId, expectedGoalId)
        const normalizedObjective = yield* normalizeObjective(objective)
        const now = runtime.nowIso()

        yield* d1Effect('agentGoals.editObjective', () =>
          db
            .prepare(
              `UPDATE agent_goals
               SET objective = ?,
                   status = CASE WHEN status IN ('complete', 'budget_limited') THEN 'active' ELSE status END,
                   completed_at = CASE WHEN status = 'complete' THEN NULL ELSE completed_at END,
                   updated_at = ?
               WHERE id = ?
                 AND archived_at IS NULL`,
            )
            .bind(normalizedObjective, now, goal.id)
            .run(),
        )

        return yield* readUpdatedGoal(goal.id)
      },
    ),

    getById: goalId => requireGoal(goalId),

    getCurrent: scope =>
      d1Effect('agentGoals.getCurrent', () =>
        db
          .prepare(
            `SELECT *
             FROM agent_goals
             WHERE agent_id = ?
               AND COALESCE(user_id, '') = COALESCE(?, '')
               AND COALESCE(team_id, '') = COALESCE(?, '')
               AND COALESCE(project_id, '') = COALESCE(?, '')
               AND archived_at IS NULL
             ORDER BY updated_at DESC
             LIMIT 1`,
          )
          .bind(...currentScopeBindings(scope))
          .first<AgentGoalRow>(),
      ).pipe(Effect.map(row => (row === null ? undefined : toRecord(row)))),

    getPublicCurrentByAgentId: agentId =>
      d1Effect('agentGoals.getPublicCurrentByAgentId', () =>
        db
          .prepare(
            `SELECT *
             FROM agent_goals
             WHERE agent_id = ?
               AND visibility = 'public'
               AND archived_at IS NULL
             ORDER BY updated_at DESC
             LIMIT 1`,
          )
          .bind(agentId)
          .first<AgentGoalRow>(),
      ).pipe(Effect.map(row => (row === null ? undefined : toRecord(row)))),

    setGoal: Effect.fn('AgentGoalRepository.setGoal')(
      function* (input: CreateAgentGoalInput) {
        const objective = yield* normalizeObjective(input.objective)
        const tokenBudget = yield* normalizeTokenBudget(input.tokenBudget)
        const now = runtime.nowIso()
        const id = input.id ?? runtime.randomId('agent_goal')
        const visibility = input.visibility ?? 'private'
        const scopeBindings = currentScopeBindings(input)

        yield* d1Effect('agentGoals.setGoal', () =>
          db.batch([
            db
              .prepare(
                `UPDATE agent_goals
                 SET archived_at = ?,
                     updated_at = ?
                 WHERE agent_id = ?
                   AND COALESCE(user_id, '') = COALESCE(?, '')
                   AND COALESCE(team_id, '') = COALESCE(?, '')
                   AND COALESCE(project_id, '') = COALESCE(?, '')
                   AND archived_at IS NULL`,
              )
              .bind(now, now, ...scopeBindings),
            db
              .prepare(
                `INSERT INTO agent_goals
                  (id, agent_id, user_id, team_id, project_id, objective,
                   status, visibility, current_run_id, token_budget, tokens_used,
                   time_used_seconds, created_at, updated_at, completed_at,
                   paused_at, blocked_at, archived_at)
                 VALUES (?, ?, ?, ?, ?, ?, 'active', ?, NULL, ?, 0, 0, ?, ?, NULL, NULL, NULL, NULL)`,
              )
              .bind(
                id,
                input.agentId,
                input.userId ?? null,
                input.teamId ?? null,
                input.projectId ?? null,
                objective,
                visibility,
                tokenBudget,
                now,
                now,
              ),
          ]),
        )

        return yield* readUpdatedGoal(id)
      },
    ),

    setStatus: Effect.fn('AgentGoalRepository.setStatus')(
      function* (
        goalId: string,
        status: AgentGoalStatus,
        expectedGoalId?: string,
      ) {
        const goal = yield* requireExpectedGoal(goalId, expectedGoalId)
        const now = runtime.nowIso()
        const timestamps = terminalTimestamps(status, now)

        yield* d1Effect('agentGoals.setStatus', () =>
          db
            .prepare(
              `UPDATE agent_goals
               SET status = ?,
                   completed_at = ?,
                   paused_at = ?,
                   blocked_at = ?,
                   updated_at = ?
               WHERE id = ?
                 AND archived_at IS NULL`,
            )
            .bind(
              status,
              timestamps.completedAt,
              timestamps.pausedAt,
              timestamps.blockedAt,
              now,
              goal.id,
            )
            .run(),
        )

        return yield* readUpdatedGoal(goal.id)
      },
    ),

    setTokenBudget: Effect.fn('AgentGoalRepository.setTokenBudget')(
      function* (
        goalId: string,
        tokenBudget: number,
        expectedGoalId?: string,
      ) {
        const goal = yield* requireExpectedGoal(goalId, expectedGoalId)
        const normalizedBudget = yield* normalizeTokenBudget(tokenBudget)
        const status = statusAfterUsage(
          goal.status,
          normalizedBudget,
          goal.tokensUsed,
        )
        const now = runtime.nowIso()

        yield* d1Effect('agentGoals.setTokenBudget', () =>
          db
            .prepare(
              `UPDATE agent_goals
               SET token_budget = ?,
                   status = ?,
                   updated_at = ?
               WHERE id = ?
                 AND archived_at IS NULL`,
            )
            .bind(normalizedBudget, status, now, goal.id)
            .run(),
        )

        return yield* readUpdatedGoal(goal.id)
      },
    ),
  }
}

export const makeAgentGoalRepositoryLayer = (
  db: D1Database,
  runtime?: AgentGoalRuntime,
) => Layer.succeed(AgentGoalRepository, makeD1AgentGoalRepository(db, runtime))

export const makeAgentGoalAccessService =
  (): AgentGoalAccessServiceShape => {
    const teamReadable = (actor: AgentGoalActor, goal: AgentGoalRecord) =>
      goal.teamId !== null && (actor.teamIds ?? []).includes(goal.teamId)
    const ownerReadable = (actor: AgentGoalActor, goal: AgentGoalRecord) =>
      actor.userId !== undefined && actor.userId === goal.userId
    const canRead = (actor: AgentGoalActor, goal: AgentGoalRecord): boolean =>
      actor.operator === true ||
      goal.visibility === 'public' ||
      ownerReadable(actor, goal) ||
      (goal.visibility === 'team' && teamReadable(actor, goal))
    const canWrite = (actor: AgentGoalActor, goal: AgentGoalRecord): boolean =>
      actor.operator === true || ownerReadable(actor, goal)
    const deny = (actor: AgentGoalActor, goal: AgentGoalRecord) =>
      new AgentGoalAccessDenied({
        actorUserId: actor.userId ?? null,
        goalId: goal.id,
      })

    return {
      canRead,
      canWrite,
      publicProjection: goal => {
        const projected = publicAgentGoalRecord(goal)

        if (goal.visibility !== 'public') {
          return Effect.fail(deny({}, goal))
        }

        return containsProviderSecretMaterial(JSON.stringify(projected))
          ? Effect.fail(new AgentGoalPublicProjectionUnsafe({ goalId: goal.id }))
          : Effect.succeed(projected)
      },
      requireRead: (actor, goal) =>
        canRead(actor, goal)
          ? Effect.succeed(goal)
          : Effect.fail(deny(actor, goal)),
      requireWrite: (actor, goal) =>
        canWrite(actor, goal)
          ? Effect.succeed(goal)
          : Effect.fail(deny(actor, goal)),
    }
  }

export const AgentGoalAccessServiceLayer = Layer.succeed(
  AgentGoalAccessService,
  makeAgentGoalAccessService(),
)
