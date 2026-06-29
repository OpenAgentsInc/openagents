import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ThreadAccessService,
  findAuthorizedAgentRunBundle,
} from './thread-access'

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 0,
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

const cloneForD1 = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

type TestRunRow = Readonly<{
  archived_at: string | null
  assignment_json: string
  assignment_kind: string
  auth_grant_ref: string | null
  backend: string
  canceled_at: string | null
  completed_at: string | null
  created_at: string
  event_cursor: number
  external_run_id: string | null
  failed_at: string | null
  goal: string
  id: string
  project_id: string | null
  provider_account_ref: string | null
  repository_owner: string
  repository_provider: string
  repository_ref: string
  repository_repo: string
  runner_id: string
  runtime: string
  started_at: string | null
  status: string
  team_id: string | null
  updated_at: string
  user_id: string
}>

type TestTeamChatMessage = Readonly<{
  agent_run_id: string | null
  autopilot_thread_id: string
  deleted_at: string | null
}>

type TestTeamMembership = Readonly<{
  role: string
  teamId: string
  userId: string
}>

type TestData = Readonly<{
  memberships: ReadonlyArray<TestTeamMembership>
  messages: ReadonlyArray<TestTeamChatMessage>
  runs: Record<string, TestRunRow>
}>

const baseRunRow = (overrides: Partial<TestRunRow> = {}): TestRunRow => ({
  archived_at: null,
  assignment_json: '{}',
  assignment_kind: 'workroom_agent',
  auth_grant_ref: null,
  backend: 'shc_vm',
  canceled_at: null,
  completed_at: null,
  created_at: '2026-06-04T00:00:00.000Z',
  event_cursor: 0,
  external_run_id: null,
  failed_at: null,
  goal: 'Fix the thread route',
  id: 'agent_run_1',
  project_id: null,
  provider_account_ref: null,
  repository_owner: 'OpenAgentsInc',
  repository_provider: 'github',
  repository_ref: 'main',
  repository_repo: 'autopilot-omega',
  runner_id: 'oa-shc-katy-01',
  runtime: 'opencode_codex',
  started_at: null,
  status: 'running',
  team_id: null,
  updated_at: '2026-06-04T00:00:00.000Z',
  user_id: 'github:owner',
  ...overrides,
})

const makePreparedStatement = (
  query: string,
  data: TestData,
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []

  const first = <T = Record<string, unknown>>(
    _colName?: string,
  ): Promise<T | null> => {
    if (query.includes('FROM team_chat_messages')) {
      const message = data.messages.find(
        message =>
          message.autopilot_thread_id === values[0] &&
          message.agent_run_id !== null &&
          message.deleted_at === null,
      )

      return Promise.resolve(
        message === undefined
          ? null
          : cloneForD1<T>({ agent_run_id: message.agent_run_id }),
      )
    }

    if (
      query.includes('SELECT id, team_id, user_id') &&
      query.includes('FROM agent_runs')
    ) {
      const run = data.runs[String(values[0])]

      return Promise.resolve(
        run === undefined || run.archived_at !== null
          ? null
          : cloneForD1<T>({
              id: run.id,
              team_id: run.team_id,
              user_id: run.user_id,
            }),
      )
    }

    if (query.includes('FROM team_memberships')) {
      const membership = data.memberships.find(
        membership =>
          membership.teamId === values[0] && membership.userId === values[1],
      )

      return Promise.resolve(
        membership === undefined
          ? null
          : cloneForD1<T>({ role: membership.role }),
      )
    }

    if (query.includes('SELECT * FROM agent_runs')) {
      const run = data.runs[String(values[1])]

      return Promise.resolve(
        run === undefined ||
          run.archived_at !== null ||
          run.user_id !== values[0]
          ? null
          : cloneForD1<T>(run),
      )
    }

    return Promise.resolve(null)
  }

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
    all: <T = Record<string, unknown>>() => {
      if (query.includes('FROM agent_run_events')) {
        return Promise.resolve(makeResult<T>())
      }

      return Promise.resolve(makeResult<T>())
    },
    bind: (...nextValues: ReadonlyArray<unknown>) => {
      values = nextValues

      return statement
    },
    first,
    raw,
    run: <T = Record<string, unknown>>() => Promise.resolve(makeResult<T>()),
  }

  return statement
}

const makeMemoryD1Session = (db: D1Database): D1DatabaseSession => ({
  batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
    Promise.all(statements.map(statement => statement.run<T>())),
  getBookmark: () => null,
  prepare: (query: string) => db.prepare(query),
})

const makeMemoryD1 = (data: TestData): D1Database => {
  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: (_query: string) => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => makePreparedStatement(query, data),
    withSession: () => makeMemoryD1Session(db),
  }

  return db
}

const envWithDb = (OPENAGENTS_DB: D1Database) => ({
  OPENAGENTS_DB,
})

const accessResult = (
  data: TestData,
  userId: string,
  routeId: string,
): Promise<string> =>
  Effect.runPromise(
    Effect.gen(function* () {
      const threadAccess = yield* ThreadAccessService

      return yield* threadAccess.findAuthorizedBundle({ routeId, userId }).pipe(
        Effect.match({
          onFailure: error => error._tag,
          onSuccess: bundle => bundle.run.id,
        }),
      )
    }).pipe(
      Effect.provide(ThreadAccessService.layer(envWithDb(makeMemoryD1(data)))),
    ),
  )

describe('ThreadAccessService', () => {
  test('authorizes personal agent runs for the owning user', async () => {
    const data = {
      memberships: [],
      messages: [],
      runs: {
        agent_run_1: baseRunRow(),
      },
    }

    await expect(
      accessResult(data, 'github:owner', 'agent_run_1'),
    ).resolves.toBe('agent_run_1')
  })

  test('resolves team autopilot thread ids for active team members', async () => {
    const data = {
      memberships: [
        {
          role: 'member',
          teamId: 'team_openagents_core',
          userId: 'github:member',
        },
      ],
      messages: [
        {
          agent_run_id: 'agent_run_1',
          autopilot_thread_id: 'thread_1',
          deleted_at: null,
        },
      ],
      runs: {
        agent_run_1: baseRunRow({ team_id: 'team_openagents_core' }),
      },
    }

    await expect(accessResult(data, 'github:member', 'thread_1')).resolves.toBe(
      'agent_run_1',
    )
  })

  test('treats archived runs as unavailable', async () => {
    const data = {
      memberships: [],
      messages: [],
      runs: {
        agent_run_1: baseRunRow({
          archived_at: '2026-06-04T00:01:00.000Z',
        }),
      },
    }

    await expect(
      accessResult(data, 'github:owner', 'agent_run_1'),
    ).resolves.toBe('RouteAccessNotFound')
  })

  test('treats deleted team autopilot messages as unavailable', async () => {
    const data = {
      memberships: [
        {
          role: 'member',
          teamId: 'team_openagents_core',
          userId: 'github:member',
        },
      ],
      messages: [
        {
          agent_run_id: 'agent_run_1',
          autopilot_thread_id: 'thread_1',
          deleted_at: '2026-06-04T00:01:00.000Z',
        },
      ],
      runs: {
        agent_run_1: baseRunRow({ team_id: 'team_openagents_core' }),
      },
    }

    await expect(accessResult(data, 'github:member', 'thread_1')).resolves.toBe(
      'RouteAccessNotFound',
    )
  })

  test('denies team runs when the user is not a team member', async () => {
    const data = {
      memberships: [],
      messages: [
        {
          agent_run_id: 'agent_run_1',
          autopilot_thread_id: 'thread_1',
          deleted_at: null,
        },
      ],
      runs: {
        agent_run_1: baseRunRow({ team_id: 'team_openagents_core' }),
      },
    }

    await expect(
      accessResult(data, 'github:outsider', 'thread_1'),
    ).resolves.toBe('RouteAccessForbidden')
  })

  test('rejects stale bootstrap ids as unavailable', async () => {
    const data = {
      memberships: [],
      messages: [],
      runs: {},
    }

    await expect(
      accessResult(
        data,
        'github:owner',
        'cf44c410-3f0a-40a1-a3f6-4086091bc28a',
      ),
    ).resolves.toBe('RouteAccessNotFound')
  })

  test('keeps the compatibility helper on the service path', async () => {
    const bundle = await findAuthorizedAgentRunBundle(
      envWithDb(
        makeMemoryD1({
          memberships: [],
          messages: [],
          runs: {
            agent_run_1: baseRunRow(),
          },
        }),
      ),
      'github:owner',
      'agent_run_1',
    )

    expect(bundle?.run.id).toBe('agent_run_1')
  })
})
