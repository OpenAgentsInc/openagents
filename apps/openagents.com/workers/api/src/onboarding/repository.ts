import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  identityDbForEnv,
  type IdentityDb,
  type IdentityDbEnv,
} from '../identity-db'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  type OnboardingBillingState,
  type OnboardingGitHubRepository,
  type OnboardingRepositorySelection,
  type OnboardingStatus,
  type OnboardingStep,
} from './schema'

// CFG-4 Domain 2 (#8519): onboarding state lives on the `users` table,
// which is Postgres-AUTHORITATIVE now — every read/UPDATE in this module
// runs on the identity handle. The old D1 path and its fail-soft
// `identityAuthMirrorFromEnv` row mirror are DELETED (khala-sync migration
// `0042_identity_hard_cut.sql` widens the Postgres `users` twin with the
// worker-0025 onboarding columns).
type OnboardingEnv = IdentityDbEnv &
  Readonly<{
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

const storeEffect = <A>(
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

const text = (value: unknown): string | null =>
  value === null || value === undefined ? null : String(value)

/** Postgres bigint columns arrive as strings; keep the 0/1 flag numeric. */
const flag = (value: unknown): number | null =>
  value === null || value === undefined ? null : Number(value)

const toOnboardingUserRow = (
  row: Readonly<Record<string, unknown>>,
): OnboardingUserRow => ({
  onboarding_billing_skipped_at: text(row.onboarding_billing_skipped_at),
  onboarding_completed_at: text(row.onboarding_completed_at),
  onboarding_goal: text(row.onboarding_goal),
  onboarding_repository_default_branch: text(
    row.onboarding_repository_default_branch,
  ),
  onboarding_repository_description: text(
    row.onboarding_repository_description,
  ),
  onboarding_repository_full_name: text(row.onboarding_repository_full_name),
  onboarding_repository_html_url: text(row.onboarding_repository_html_url),
  onboarding_repository_id: text(row.onboarding_repository_id),
  onboarding_repository_name: text(row.onboarding_repository_name),
  onboarding_repository_owner: text(row.onboarding_repository_owner),
  onboarding_repository_private: flag(row.onboarding_repository_private),
  onboarding_repository_provider:
    row.onboarding_repository_provider === 'github' ? 'github' : null,
  onboarding_repository_selected_at: text(
    row.onboarding_repository_selected_at,
  ),
  onboarding_repository_skipped_at: text(row.onboarding_repository_skipped_at),
  onboarding_step: String(row.onboarding_step) as OnboardingStep,
  onboarding_updated_at: text(row.onboarding_updated_at),
  updated_at: String(row.updated_at),
})

const readStatus = (
  identityDb: IdentityDb,
  userId: string,
): Effect.Effect<
  OnboardingStatus,
  OnboardingStorageError | OnboardingUserNotFound
> =>
  Effect.gen(function* () {
    const rows = yield* storeEffect('onboarding.readStatus', () =>
      identityDb.query(
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
        [userId],
      ),
    )
    const row = rows[0]

    if (row === undefined) {
      return yield* new OnboardingUserNotFound({ userId })
    }

    return statusFromRow(toOnboardingUserRow(row))
  })

const selectRepository = (
  identityDb: IdentityDb,
  runtime: OnboardingRuntime,
  userId: string,
  repository: OnboardingGitHubRepository,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(identityDb, userId)

    if (current.step !== 'repository') {
      return yield* updateRepository(identityDb, runtime, userId, repository)
    }

    const now = runtime.nowIso()

    yield* storeEffect('onboarding.selectRepository', () =>
      identityDb.query(
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
        [
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
        ],
      ),
    )

    return yield* readStatus(identityDb, userId)
  })

const updateRepository = (
  identityDb: IdentityDb,
  runtime: OnboardingRuntime,
  userId: string,
  repository: OnboardingGitHubRepository,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    yield* readStatus(identityDb, userId)

    const now = runtime.nowIso()

    yield* storeEffect('onboarding.updateRepository', () =>
      identityDb.query(
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
        [
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
        ],
      ),
    )

    return yield* readStatus(identityDb, userId)
  })

const skipRepository = (
  identityDb: IdentityDb,
  runtime: OnboardingRuntime,
  userId: string,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(identityDb, userId)

    if (current.step !== 'repository') {
      return yield* new OnboardingInvalidStep({ step: current.step })
    }

    const now = runtime.nowIso()

    yield* storeEffect('onboarding.skipRepository', () =>
      identityDb.query(
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
        [now, now, now, userId],
      ),
    )

    return yield* readStatus(identityDb, userId)
  })

const skipBilling = (
  identityDb: IdentityDb,
  runtime: OnboardingRuntime,
  userId: string,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(identityDb, userId)

    if (current.step !== 'billing') {
      return yield* new OnboardingInvalidStep({ step: current.step })
    }

    const now = runtime.nowIso()

    yield* storeEffect('onboarding.skipBilling', () =>
      identityDb.query(
        `UPDATE users
           SET onboarding_step = 'complete',
               onboarding_billing_skipped_at = ?,
               onboarding_completed_at = COALESCE(onboarding_completed_at, ?),
               onboarding_updated_at = ?,
               updated_at = ?
         WHERE id = ?
           AND kind = 'human'
           AND deleted_at IS NULL`,
        [now, now, now, now, userId],
      ),
    )

    return yield* readStatus(identityDb, userId)
  })

const submitGoal = (
  identityDb: IdentityDb,
  runtime: OnboardingRuntime,
  userId: string,
  goal: string,
): Effect.Effect<OnboardingStatus, OnboardingRepositoryError> =>
  Effect.gen(function* () {
    const current = yield* readStatus(identityDb, userId)

    if (current.step !== 'goal') {
      return yield* new OnboardingInvalidStep({ step: current.step })
    }

    const trimmedGoal = goal.trim()

    if (trimmedGoal === '') {
      return yield* new OnboardingInvalidStep({ step: 'empty_goal' })
    }

    const now = runtime.nowIso()

    yield* storeEffect('onboarding.submitGoal', () =>
      identityDb.query(
        `UPDATE users
           SET onboarding_step = 'billing',
               onboarding_goal = ?,
               onboarding_updated_at = ?,
               updated_at = ?
         WHERE id = ?
           AND kind = 'human'
           AND deleted_at IS NULL`,
        [trimmedGoal, now, now, userId],
      ),
    )

    return yield* readStatus(identityDb, userId)
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
    // CFG-4 Domain 2 (#8519): injectable identity handle for tests (the
    // SQLite adapters in test/payments-ledger-sqlite.ts); production uses
    // the KHALA_SYNC_DB Hyperdrive binding.
    identityDbOverride?: IdentityDb | undefined,
  ) => {
    const identityDb = identityDbOverride ?? identityDbForEnv(env)

    return Layer.succeed(OnboardingStateStore, {
      readStatus: Effect.fn('OnboardingStateStore.readStatus')(userId =>
        readStatus(identityDb, userId),
      ),
      selectRepository: Effect.fn('OnboardingStateStore.selectRepository')(
        (userId, repository) =>
          selectRepository(identityDb, runtime, userId, repository),
      ),
      updateRepository: Effect.fn('OnboardingStateStore.updateRepository')(
        (userId, repository) =>
          updateRepository(identityDb, runtime, userId, repository),
      ),
      skipRepository: Effect.fn('OnboardingStateStore.skipRepository')(userId =>
        skipRepository(identityDb, runtime, userId),
      ),
      skipBilling: Effect.fn('OnboardingStateStore.skipBilling')(userId =>
        skipBilling(identityDb, runtime, userId),
      ),
      submitGoal: Effect.fn('OnboardingStateStore.submitGoal')((userId, goal) =>
        submitGoal(identityDb, runtime, userId, goal),
      ),
    })
  }
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
