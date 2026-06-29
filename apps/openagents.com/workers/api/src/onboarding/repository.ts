import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { openAgentsDatabase } from '../runtime'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  type OnboardingBillingState,
  type OnboardingGitHubRepository,
  type OnboardingRepositorySelection,
  type OnboardingStatus,
  type OnboardingStep,
} from './schema'

type OnboardingEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type OnboardingRuntime = Readonly<{
  nowIso: () => string
}>

export const systemOnboardingRuntime: OnboardingRuntime = {
  nowIso: currentIsoTimestamp,
}

type OnboardingUserRow = Readonly<{
  updated_at: string
  onboarding_completed_at: string | null
  onboarding_step: OnboardingStep
  onboarding_repository_provider: 'github' | null
  onboarding_repository_id: string | null
  onboarding_repository_owner: string | null
  onboarding_repository_name: string | null
  onboarding_repository_full_name: string | null
  onboarding_repository_private: number | null
  onboarding_repository_default_branch: string | null
  onboarding_repository_html_url: string | null
  onboarding_repository_description: string | null
  onboarding_repository_selected_at: string | null
  onboarding_repository_skipped_at: string | null
  onboarding_billing_skipped_at: string | null
  onboarding_goal: string | null
  onboarding_updated_at: string | null
}>

export class OnboardingUserNotFound extends S.TaggedErrorClass<OnboardingUserNotFound>()(
  'OnboardingUserNotFound',
  {
    userId: S.String,
  },
) {}

export class OnboardingInvalidStep extends S.TaggedErrorClass<OnboardingInvalidStep>()(
  'OnboardingInvalidStep',
  {
    step: S.String,
  },
) {}

export class OnboardingStorageError extends S.TaggedErrorClass<OnboardingStorageError>()(
  'OnboardingStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export const OnboardingRepositoryError = S.Union([
  OnboardingUserNotFound,
  OnboardingInvalidStep,
  OnboardingStorageError,
])
export type OnboardingRepositoryError = typeof OnboardingRepositoryError.Type

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OnboardingStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new OnboardingStorageError({ operation, error }),
  })

const repositoryFromRow = (
  row: OnboardingUserRow,
): OnboardingRepositorySelection => {
  if (
    row.onboarding_repository_selected_at !== null &&
    row.onboarding_repository_provider === 'github' &&
    row.onboarding_repository_id !== null &&
    row.onboarding_repository_owner !== null &&
    row.onboarding_repository_name !== null
  ) {
    const fullName =
      row.onboarding_repository_full_name ??
      `${row.onboarding_repository_owner}/${row.onboarding_repository_name}`

    return {
      _tag: 'RepositorySelected',
      repository: {
        id: row.onboarding_repository_id,
        provider: 'github',
        owner: row.onboarding_repository_owner,
        name: row.onboarding_repository_name,
        fullName,
        private: row.onboarding_repository_private === 1,
        defaultBranch: row.onboarding_repository_default_branch ?? 'main',
        htmlUrl:
          row.onboarding_repository_html_url ??
          `https://github.com/${fullName}`,
        description: row.onboarding_repository_description,
      },
      selectedAt: row.onboarding_repository_selected_at,
    }
  }

  return row.onboarding_repository_skipped_at === null
    ? { _tag: 'RepositoryUnselected' }
    : {
        _tag: 'RepositorySkipped',
        skippedAt: row.onboarding_repository_skipped_at,
      }
}

const billingFromRow = (row: OnboardingUserRow): OnboardingBillingState =>
  row.onboarding_billing_skipped_at === null
    ? { _tag: 'BillingPending' }
    : {
        _tag: 'BillingSkipped',
        skippedAt: row.onboarding_billing_skipped_at,
      }

const stepFromState = (
  repository: OnboardingRepositorySelection,
  row: OnboardingUserRow,
): OnboardingStep => {
  if (row.onboarding_completed_at !== null) {
    return 'complete'
  }

  if (repository._tag === 'RepositoryUnselected') {
    return 'repository'
  }

  if (row.onboarding_goal === null || row.onboarding_goal.trim() === '') {
    return 'goal'
  }

  return 'billing'
}

const statusFromRow = (row: OnboardingUserRow): OnboardingStatus => {
  const repository = repositoryFromRow(row)
  const billing = billingFromRow(row)

  return {
    step: stepFromState(repository, row),
    repository,
    billing,
    goal: row.onboarding_goal,
    completedAt: row.onboarding_completed_at,
    updatedAt: row.onboarding_updated_at ?? row.updated_at,
  }
}

const readStatus = (
  db: D1Database,
  userId: string,
): Effect.Effect<
  OnboardingStatus,
  OnboardingStorageError | OnboardingUserNotFound
> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('onboarding.readStatus', () =>
      db
        .prepare(
          `SELECT updated_at,
                  onboarding_step,
                  onboarding_completed_at,
                  onboarding_repository_provider,
                  onboarding_repository_id,
                  onboarding_repository_owner,
                  onboarding_repository_name,
                  onboarding_repository_full_name,
                  onboarding_repository_private,
                  onboarding_repository_default_branch,
                  onboarding_repository_html_url,
                  onboarding_repository_description,
                  onboarding_repository_selected_at,
                  onboarding_repository_skipped_at,
                  onboarding_billing_skipped_at,
                  onboarding_goal,
                  onboarding_updated_at
           FROM users
           WHERE id = ?
             AND kind = 'human'
             AND deleted_at IS NULL
           LIMIT 1`,
        )
        .bind(userId)
        .first<OnboardingUserRow>(),
    )

    if (row === null) {
      return yield* new OnboardingUserNotFound({ userId })
    }

    return statusFromRow(row)
  })

const selectRepository = (
  db: D1Database,
  runtime: OnboardingRuntime,
  userId: string,
  repository: OnboardingGitHubRepository,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(db, userId)

    if (current.step !== 'repository') {
      return yield* updateRepository(db, runtime, userId, repository)
    }

    const now = runtime.nowIso()

    yield* d1Effect('onboarding.selectRepository', () =>
      db
        .prepare(
          `UPDATE users
           SET onboarding_step = 'goal',
               onboarding_repository_provider = 'github',
               onboarding_repository_id = ?,
               onboarding_repository_owner = ?,
               onboarding_repository_name = ?,
               onboarding_repository_full_name = ?,
               onboarding_repository_private = ?,
               onboarding_repository_default_branch = ?,
               onboarding_repository_html_url = ?,
               onboarding_repository_description = ?,
               onboarding_repository_selected_at = ?,
               onboarding_repository_skipped_at = NULL,
               onboarding_updated_at = ?,
               updated_at = ?
           WHERE id = ?
             AND kind = 'human'
             AND deleted_at IS NULL`,
        )
        .bind(
          repository.id,
          repository.owner,
          repository.name,
          repository.fullName,
          repository.private ? 1 : 0,
          repository.defaultBranch,
          repository.htmlUrl,
          repository.description,
          now,
          now,
          now,
          userId,
        )
        .run(),
    )

    return yield* readStatus(db, userId)
  })

const updateRepository = (
  db: D1Database,
  runtime: OnboardingRuntime,
  userId: string,
  repository: OnboardingGitHubRepository,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    yield* readStatus(db, userId)

    const now = runtime.nowIso()

    yield* d1Effect('onboarding.updateRepository', () =>
      db
        .prepare(
          `UPDATE users
           SET onboarding_repository_provider = 'github',
               onboarding_repository_id = ?,
               onboarding_repository_owner = ?,
               onboarding_repository_name = ?,
               onboarding_repository_full_name = ?,
               onboarding_repository_private = ?,
               onboarding_repository_default_branch = ?,
               onboarding_repository_html_url = ?,
               onboarding_repository_description = ?,
               onboarding_repository_selected_at = ?,
               onboarding_repository_skipped_at = NULL,
               onboarding_updated_at = ?,
               updated_at = ?
           WHERE id = ?
             AND kind = 'human'
             AND deleted_at IS NULL`,
        )
        .bind(
          repository.id,
          repository.owner,
          repository.name,
          repository.fullName,
          repository.private ? 1 : 0,
          repository.defaultBranch,
          repository.htmlUrl,
          repository.description,
          now,
          now,
          now,
          userId,
        )
        .run(),
    )

    return yield* readStatus(db, userId)
  })

const skipRepository = (
  db: D1Database,
  runtime: OnboardingRuntime,
  userId: string,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(db, userId)

    if (current.step !== 'repository') {
      return yield* new OnboardingInvalidStep({ step: current.step })
    }

    const now = runtime.nowIso()

    yield* d1Effect('onboarding.skipRepository', () =>
      db
        .prepare(
          `UPDATE users
           SET onboarding_step = 'goal',
               onboarding_repository_provider = NULL,
               onboarding_repository_id = NULL,
               onboarding_repository_owner = NULL,
               onboarding_repository_name = NULL,
               onboarding_repository_full_name = NULL,
               onboarding_repository_private = NULL,
               onboarding_repository_default_branch = NULL,
               onboarding_repository_html_url = NULL,
               onboarding_repository_description = NULL,
               onboarding_repository_selected_at = NULL,
               onboarding_repository_skipped_at = ?,
               onboarding_updated_at = ?,
               updated_at = ?
           WHERE id = ?
             AND kind = 'human'
             AND deleted_at IS NULL`,
        )
        .bind(now, now, now, userId)
        .run(),
    )

    return yield* readStatus(db, userId)
  })

const skipBilling = (
  db: D1Database,
  runtime: OnboardingRuntime,
  userId: string,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(db, userId)

    if (current.step !== 'billing') {
      return yield* new OnboardingInvalidStep({ step: current.step })
    }

    const now = runtime.nowIso()

    yield* d1Effect('onboarding.skipBilling', () =>
      db
        .prepare(
          `UPDATE users
           SET onboarding_step = 'complete',
               onboarding_billing_skipped_at = ?,
               onboarding_completed_at = COALESCE(onboarding_completed_at, ?),
               onboarding_updated_at = ?,
               updated_at = ?
           WHERE id = ?
             AND kind = 'human'
             AND deleted_at IS NULL`,
        )
        .bind(now, now, now, now, userId)
        .run(),
    )

    return yield* readStatus(db, userId)
  })

const submitGoal = (
  db: D1Database,
  runtime: OnboardingRuntime,
  userId: string,
  goal: string,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(db, userId)

    if (current.step !== 'goal') {
      return yield* new OnboardingInvalidStep({ step: current.step })
    }

    const trimmedGoal = goal.trim()

    if (trimmedGoal === '') {
      return yield* new OnboardingInvalidStep({ step: 'empty_goal' })
    }

    const now = runtime.nowIso()

    yield* d1Effect('onboarding.submitGoal', () =>
      db
        .prepare(
          `UPDATE users
           SET onboarding_step = 'billing',
               onboarding_goal = ?,
               onboarding_updated_at = ?,
               updated_at = ?
           WHERE id = ?
             AND kind = 'human'
             AND deleted_at IS NULL`,
        )
        .bind(trimmedGoal, now, now, userId)
        .run(),
    )

    return yield* readStatus(db, userId)
  })

export class OnboardingStateStore extends Context.Service<
  OnboardingStateStore,
  {
    readonly readStatus: (
      userId: string,
    ) => Effect.Effect<
      OnboardingStatus,
      OnboardingStorageError | OnboardingUserNotFound
    >
    readonly selectRepository: (
      userId: string,
      repository: OnboardingGitHubRepository,
    ) => Effect.Effect<OnboardingStatus, OnboardingRepositoryError>
    readonly updateRepository: (
      userId: string,
      repository: OnboardingGitHubRepository,
    ) => Effect.Effect<OnboardingStatus, OnboardingRepositoryError>
    readonly skipRepository: (
      userId: string,
    ) => Effect.Effect<OnboardingStatus, OnboardingRepositoryError>
    readonly skipBilling: (
      userId: string,
    ) => Effect.Effect<OnboardingStatus, OnboardingRepositoryError>
    readonly submitGoal: (
      userId: string,
      goal: string,
    ) => Effect.Effect<OnboardingStatus, OnboardingRepositoryError>
  }
>()('@openagentsinc/autopilot-omega/OnboardingStateStore') {
  static readonly layer = (
    env: OnboardingEnv,
    runtime: OnboardingRuntime = systemOnboardingRuntime,
  ) =>
    Layer.succeed(OnboardingStateStore, {
      readStatus: Effect.fn('OnboardingStateStore.readStatus')(userId =>
        readStatus(openAgentsDatabase(env), userId),
      ),
      selectRepository: Effect.fn('OnboardingStateStore.selectRepository')(
        (userId, repository) =>
          selectRepository(
            openAgentsDatabase(env),
            runtime,
            userId,
            repository,
          ),
      ),
      updateRepository: Effect.fn('OnboardingStateStore.updateRepository')(
        (userId, repository) =>
          updateRepository(
            openAgentsDatabase(env),
            runtime,
            userId,
            repository,
          ),
      ),
      skipRepository: Effect.fn('OnboardingStateStore.skipRepository')(userId =>
        skipRepository(openAgentsDatabase(env), runtime, userId),
      ),
      skipBilling: Effect.fn('OnboardingStateStore.skipBilling')(userId =>
        skipBilling(openAgentsDatabase(env), runtime, userId),
      ),
      submitGoal: Effect.fn('OnboardingStateStore.submitGoal')((userId, goal) =>
        submitGoal(openAgentsDatabase(env), runtime, userId, goal),
      ),
    })
}

export const readOnboardingStatusForUser = (
  env: OnboardingEnv,
  userId: string,
): Promise<OnboardingStatus> =>
  Effect.runPromise(
    Effect.gen(function* () {
      const store = yield* OnboardingStateStore

      return yield* store.readStatus(userId)
    }).pipe(Effect.provide(OnboardingStateStore.layer(env))),
  )
