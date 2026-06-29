import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OnboardingInvalidStep,
  type OnboardingRuntime,
  OnboardingStateStore,
} from './repository'
import { type OnboardingGitHubRepository } from './schema'

type UserRow = Record<string, unknown>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const repository = {
  defaultBranch: 'main',
  description: 'Foldkit and Effect application',
  fullName: 'OpenAgentsInc/autopilot-omega',
  htmlUrl: 'https://github.com/OpenAgentsInc/autopilot-omega',
  id: 'repo_omega',
  name: 'autopilot-omega',
  owner: 'OpenAgentsInc',
  private: true,
  provider: 'github',
} satisfies OnboardingGitHubRepository

const makeUserRow = (): UserRow => ({
  id: 'github:1',
  kind: 'human',
  deleted_at: null,
  updated_at: '2026-06-04T00:00:00.000Z',
  onboarding_step: 'repository',
  onboarding_completed_at: null,
  onboarding_repository_provider: null,
  onboarding_repository_id: null,
  onboarding_repository_owner: null,
  onboarding_repository_name: null,
  onboarding_repository_full_name: null,
  onboarding_repository_private: null,
  onboarding_repository_default_branch: null,
  onboarding_repository_html_url: null,
  onboarding_repository_description: null,
  onboarding_repository_selected_at: null,
  onboarding_repository_skipped_at: null,
  onboarding_billing_skipped_at: null,
  onboarding_goal: null,
  onboarding_updated_at: null,
})

const makeRuntime = (): OnboardingRuntime => {
  const timestamps = [
    '2026-06-04T00:00:01.000Z',
    '2026-06-04T00:00:02.000Z',
    '2026-06-04T00:00:03.000Z',
    '2026-06-04T00:00:04.000Z',
  ]
  let index = 0

  return {
    nowIso: () => timestamps[index++] ?? '2026-06-04T00:00:05.000Z',
  }
}

const makeMemoryD1 = (row: UserRow): D1Database => {
  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => {
      let values: ReadonlyArray<unknown> = []

      function raw<T = unknown[]>(options: {
        columnNames: true
      }): Promise<[Array<string>, ...Array<T>]>
      function raw<T = unknown[]>(options?: {
        columnNames?: false
      }): Promise<Array<T>>
      function raw<T = unknown[]>(options?: {
        columnNames?: boolean
      }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
        return options?.columnNames === true
          ? Promise.resolve([[]])
          : Promise.resolve([])
      }

      const statement: D1PreparedStatement = {
        all: <T = Record<string, unknown>>() =>
          Promise.resolve(makeResult<T>()),
        bind: (...nextValues: ReadonlyArray<unknown>) => {
          values = nextValues

          return statement
        },
        first: <T = Record<string, unknown>>() => {
          if (values[0] !== row.id || row.deleted_at !== null) {
            return Promise.resolve(null)
          }

          return Promise.resolve(jsonFixture<T>(row))
        },
        raw,
        run: <T = Record<string, unknown>>() => {
          if (
            query.includes("SET onboarding_step = 'goal'") &&
            query.includes("onboarding_repository_provider = 'github'")
          ) {
            row.onboarding_step = 'goal'
            row.onboarding_repository_provider = 'github'
            row.onboarding_repository_id = values[0]
            row.onboarding_repository_owner = values[1]
            row.onboarding_repository_name = values[2]
            row.onboarding_repository_full_name = values[3]
            row.onboarding_repository_private = values[4]
            row.onboarding_repository_default_branch = values[5]
            row.onboarding_repository_html_url = values[6]
            row.onboarding_repository_description = values[7]
            row.onboarding_repository_selected_at = values[8]
            row.onboarding_repository_skipped_at = null
            row.onboarding_updated_at = values[9]
            row.updated_at = values[10]
          } else if (
            query.includes("SET onboarding_repository_provider = 'github'")
          ) {
            row.onboarding_repository_provider = 'github'
            row.onboarding_repository_id = values[0]
            row.onboarding_repository_owner = values[1]
            row.onboarding_repository_name = values[2]
            row.onboarding_repository_full_name = values[3]
            row.onboarding_repository_private = values[4]
            row.onboarding_repository_default_branch = values[5]
            row.onboarding_repository_html_url = values[6]
            row.onboarding_repository_description = values[7]
            row.onboarding_repository_selected_at = values[8]
            row.onboarding_repository_skipped_at = null
            row.onboarding_updated_at = values[9]
            row.updated_at = values[10]
          } else if (
            query.includes("SET onboarding_step = 'goal'") &&
            query.includes('onboarding_repository_skipped_at = ?')
          ) {
            row.onboarding_step = 'goal'
            row.onboarding_repository_provider = null
            row.onboarding_repository_id = null
            row.onboarding_repository_owner = null
            row.onboarding_repository_name = null
            row.onboarding_repository_full_name = null
            row.onboarding_repository_private = null
            row.onboarding_repository_default_branch = null
            row.onboarding_repository_html_url = null
            row.onboarding_repository_description = null
            row.onboarding_repository_selected_at = null
            row.onboarding_repository_skipped_at = values[0]
            row.onboarding_updated_at = values[1]
            row.updated_at = values[2]
          } else if (query.includes("SET onboarding_step = 'complete'")) {
            row.onboarding_step = 'complete'
            row.onboarding_billing_skipped_at = values[0]
            row.onboarding_completed_at ??= values[1]
            row.onboarding_updated_at = values[2]
            row.updated_at = values[3]
          } else if (query.includes("SET onboarding_step = 'billing'")) {
            row.onboarding_step = 'billing'
            row.onboarding_goal = values[0]
            row.onboarding_updated_at = values[1]
            row.updated_at = values[2]
          }

          return Promise.resolve(makeResult<T>())
        },
      }

      return statement
    },
    withSession: () => ({
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      getBookmark: () => null,
      prepare: query => db.prepare(query),
    }),
  }

  return db
}

const runWithStore = <A>(
  db: D1Database,
  runtime: OnboardingRuntime,
  effect: Effect.Effect<A, unknown, OnboardingStateStore>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(
      Effect.provide(
        OnboardingStateStore.layer({ OPENAGENTS_DB: db }, runtime),
      ),
    ),
  )

describe('OnboardingStateStore', () => {
  test('keeps a newly created user incomplete by default', async () => {
    const row = makeUserRow()
    const db = makeMemoryD1(row)
    const runtime = makeRuntime()
    const status = await runWithStore(
      db,
      runtime,
      Effect.gen(function* () {
        const store = yield* OnboardingStateStore

        return yield* store.readStatus('github:1')
      }),
    )

    expect(status).toMatchObject({
      billing: { _tag: 'BillingPending' },
      completedAt: null,
      repository: { _tag: 'RepositoryUnselected' },
      step: 'repository',
    })
  })

  test('persists repository, goal, and billing steps in order', async () => {
    const row = makeUserRow()
    const db = makeMemoryD1(row)
    const runtime = makeRuntime()
    const status = await runWithStore(
      db,
      runtime,
      Effect.gen(function* () {
        const store = yield* OnboardingStateStore
        yield* store.selectRepository('github:1', repository)
        yield* store.submitGoal(
          'github:1',
          '  Review the first production task  ',
        )

        return yield* store.skipBilling('github:1')
      }),
    )

    expect(status).toMatchObject({
      completedAt: '2026-06-04T00:00:03.000Z',
      goal: 'Review the first production task',
      repository: {
        _tag: 'RepositorySelected',
        repository: { fullName: 'OpenAgentsInc/autopilot-omega' },
        selectedAt: '2026-06-04T00:00:01.000Z',
      },
      step: 'complete',
    })
  })

  test('updates repository after onboarding is complete', async () => {
    const row = makeUserRow()
    const db = makeMemoryD1(row)
    const runtime = makeRuntime()
    const status = await runWithStore(
      db,
      runtime,
      Effect.gen(function* () {
        const store = yield* OnboardingStateStore
        yield* store.selectRepository('github:1', repository)
        yield* store.submitGoal('github:1', 'Review the first production task')
        yield* store.skipBilling('github:1')

        return yield* store.updateRepository('github:1', {
          ...repository,
          fullName: 'OpenAgentsInc/openagents',
          id: 'repo_openagents',
          name: 'openagents',
        })
      }),
    )

    expect(status).toMatchObject({
      completedAt: '2026-06-04T00:00:03.000Z',
      repository: {
        _tag: 'RepositorySelected',
        repository: { fullName: 'OpenAgentsInc/openagents' },
        selectedAt: '2026-06-04T00:00:04.000Z',
      },
      step: 'complete',
    })
  })

  test('selects repository after repository was skipped', async () => {
    const row = makeUserRow()
    const db = makeMemoryD1(row)
    const runtime = makeRuntime()
    const status = await runWithStore(
      db,
      runtime,
      Effect.gen(function* () {
        const store = yield* OnboardingStateStore
        yield* store.skipRepository('github:1')

        return yield* store.selectRepository('github:1', repository)
      }),
    )

    expect(status).toMatchObject({
      completedAt: null,
      repository: {
        _tag: 'RepositorySelected',
        repository: { fullName: 'OpenAgentsInc/autopilot-omega' },
        selectedAt: '2026-06-04T00:00:02.000Z',
      },
      step: 'goal',
    })
  })

  test('rejects out-of-order onboarding transitions', async () => {
    const row = makeUserRow()
    const db = makeMemoryD1(row)
    const runtime = makeRuntime()

    await expect(
      runWithStore(
        db,
        runtime,
        Effect.gen(function* () {
          const store = yield* OnboardingStateStore

          return yield* store.skipBilling('github:1')
        }),
      ),
    ).rejects.toBeInstanceOf(OnboardingInvalidStep)

    await runWithStore(
      db,
      runtime,
      Effect.gen(function* () {
        const store = yield* OnboardingStateStore

        return yield* store.skipRepository('github:1')
      }),
    )

    await expect(
      runWithStore(
        db,
        runtime,
        Effect.gen(function* () {
          const store = yield* OnboardingStateStore

          return yield* store.skipBilling('github:1')
        }),
      ),
    ).rejects.toMatchObject({ _tag: 'OnboardingInvalidStep', step: 'goal' })

    await runWithStore(
      db,
      runtime,
      Effect.gen(function* () {
        const store = yield* OnboardingStateStore

        return yield* store.submitGoal('github:1', 'Ship the thing')
      }),
    )

    await expect(
      runWithStore(
        db,
        runtime,
        Effect.gen(function* () {
          const store = yield* OnboardingStateStore

          return yield* store.submitGoal('github:1', 'Ship the thing again')
        }),
      ),
    ).rejects.toMatchObject({ _tag: 'OnboardingInvalidStep', step: 'billing' })
  })
})
