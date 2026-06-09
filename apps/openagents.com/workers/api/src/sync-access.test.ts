import { describe, expect, test } from 'vitest'

import { findAuthorizedAgentRunBundle } from './thread-access'

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

const runRow = {
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
  team_id: 'team_openagents_core',
  updated_at: '2026-06-04T00:00:00.000Z',
  user_id: 'github:owner',
}

const makePreparedStatement = (
  query: string,
  memberUserIds: ReadonlyArray<string>,
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []

  const first = <T = Record<string, unknown>>(
    _colName?: string,
  ): Promise<T | null> => {
    if (query.includes('FROM team_chat_messages') && values[0] === 'thread_1') {
      return Promise.resolve(cloneForD1<T>({ agent_run_id: 'agent_run_1' }))
    }

    if (
      query.includes('SELECT id, team_id, user_id') &&
      query.includes('FROM agent_runs') &&
      values[0] === 'agent_run_1'
    ) {
      return Promise.resolve(
        cloneForD1<T>({
          id: runRow.id,
          team_id: runRow.team_id,
          user_id: runRow.user_id,
        }),
      )
    }

    if (
      query.includes('FROM team_memberships') &&
      values[0] === 'team_openagents_core' &&
      memberUserIds.includes(String(values[1]))
    ) {
      return Promise.resolve(cloneForD1<T>({ role: 'member' }))
    }

    if (
      query.includes('SELECT * FROM agent_runs') &&
      values[0] === runRow.user_id &&
      values[1] === runRow.id
    ) {
      return Promise.resolve(cloneForD1<T>(runRow))
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
    all: <T = Record<string, unknown>>() => Promise.resolve(makeResult<T>()),
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

const makeMemoryD1 = (memberUserIds: ReadonlyArray<string>): D1Database => {
  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: (_query: string) => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => makePreparedStatement(query, memberUserIds),
    withSession: () => makeMemoryD1Session(db),
  }

  return db
}

const envWithDb = (OPENAGENTS_DB: D1Database) => ({
  OPENAGENTS_DB,
})

describe('sync access', () => {
  test('resolves team autopilot thread ids for active team members', async () => {
    const bundle = await findAuthorizedAgentRunBundle(
      envWithDb(makeMemoryD1(['github:member'])),
      'github:member',
      'thread_1',
    )

    expect(bundle?.run.id).toBe('agent_run_1')
  })

  test('denies team autopilot thread ids for non-members', async () => {
    const bundle = await findAuthorizedAgentRunBundle(
      envWithDb(makeMemoryD1([])),
      'github:outsider',
      'thread_1',
    )

    expect(bundle).toBeUndefined()
  })
})
