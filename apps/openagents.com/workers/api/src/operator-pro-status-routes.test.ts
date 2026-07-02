import { describe, expect, test } from 'vitest'

import { makeOperatorProStatusRoutes } from './operator-pro-status-routes'

type StoredStatus = {
  event_ref: string
  owner_agent_user_id: string
  runner_ref: string
  runner_kind: string
  pylon_ref: string | null
  assignment_ref: string | null
  state: string
  state_started_at: string
  updated_at: string
  retention_state: 'live' | 'retained'
  event_json: string
  created_at: string
  retained_at: string | null
  archived_at: string | null
}

class FakeStatement {
  constructor(
    private readonly db: FakeD1,
    private readonly sql: string,
    private readonly bindings: ReadonlyArray<unknown> = [],
  ) {}

  bind(...bindings: ReadonlyArray<unknown>): FakeStatement {
    return new FakeStatement(this.db, this.sql, bindings)
  }

  async all<T>(): Promise<{ results: ReadonlyArray<T> }> {
    return { results: this.db.all(this.sql, this.bindings) as ReadonlyArray<T> }
  }

  async first<T>(): Promise<T | null> {
    return (this.db.all(this.sql, this.bindings)[0] ?? null) as T | null
  }

  async run(): Promise<{ success: true }> {
    this.db.run(this.sql, this.bindings)
    return { success: true }
  }
}

class FakeD1 {
  readonly statuses: Array<StoredStatus> = []

  constructor(
    readonly pylonOwners: Record<string, string> = {
      'pylon.public.codex': 'agent_user.owner',
    },
  ) {}

  prepare(sql: string): FakeStatement {
    return new FakeStatement(this, sql)
  }

  all(sql: string, bindings: ReadonlyArray<unknown>): ReadonlyArray<unknown> {
    if (sql.includes('FROM pylon_api_registrations')) {
      const pylonRef = String(bindings[0])
      const owner = this.pylonOwners[pylonRef]
      return owner === undefined ? [] : [{ owner_agent_user_id: owner }]
    }

    if (sql.includes('FROM pylon_agent_runner_status_events')) {
      const retention = String(bindings[0])
      const owner = bindings[1] === undefined ? undefined : String(bindings[1])
      return this.statuses
        .filter(row => row.archived_at === null)
        .filter(row => row.retention_state === retention)
        .filter(row => owner === undefined || row.owner_agent_user_id === owner)
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
    }

    return []
  }

  run(sql: string, bindings: ReadonlyArray<unknown>): void {
    if (sql.startsWith('UPDATE pylon_agent_runner_status_events')) {
      const [retainedAt, owner, runnerRef, eventRef] = bindings.map(value =>
        String(value),
      )
      for (const row of this.statuses) {
        if (
          row.owner_agent_user_id === owner &&
          row.runner_ref === runnerRef &&
          row.event_ref !== eventRef &&
          row.retention_state === 'live' &&
          row.archived_at === null
        ) {
          row.retention_state = 'retained'
          row.retained_at = row.retained_at ?? retainedAt ?? null
        }
      }
      return
    }

    if (sql.startsWith('INSERT INTO pylon_agent_runner_status_events')) {
      const [
        eventRef,
        owner,
        runnerRef,
        runnerKind,
        pylonRef,
        assignmentRef,
        state,
        stateStartedAt,
        updatedAt,
        retentionState,
        eventJson,
        createdAt,
        retainedAt,
      ] = bindings
      const existing = this.statuses.find(row => row.event_ref === eventRef)
      const next = {
        event_ref: String(eventRef),
        owner_agent_user_id: String(owner),
        runner_ref: String(runnerRef),
        runner_kind: String(runnerKind),
        pylon_ref: pylonRef === null ? null : String(pylonRef),
        assignment_ref: assignmentRef === null ? null : String(assignmentRef),
        state: String(state),
        state_started_at: String(stateStartedAt),
        updated_at: String(updatedAt),
        retention_state: retentionState as 'live' | 'retained',
        event_json: String(eventJson),
        created_at: String(createdAt),
        retained_at: retainedAt === null ? null : String(retainedAt),
        archived_at: null,
      }

      if (existing === undefined) {
        this.statuses.push(next)
      } else {
        Object.assign(existing, next)
      }
    }
  }
}

const statusEvent = (overrides: Record<string, unknown> = {}) => ({
  schemaVersion: 'openagents.pylon.agent_runner_status_event.v1',
  eventRef: 'event.public.runner_status.1',
  runnerRef: 'runner.public.codex.1',
  runnerKind: 'codex_sdk',
  state: 'working',
  stateStartedAt: '2026-07-01T12:00:00.000Z',
  updatedAt: '2026-07-01T12:02:00.000Z',
  assignmentRef: 'assignment.public.issue_7878',
  taskId: 'task.public.t10_2',
  pylonRef: 'pylon.public.codex',
  worktreeId: 'issue-7878',
  refs: ['status.public.runner.working'],
  stateHistory: [
    {
      state: 'waiting',
      stateStartedAt: '2026-07-01T11:59:00.000Z',
    },
    {
      state: 'working',
      stateStartedAt: '2026-07-01T12:00:00.000Z',
    },
  ],
  ...overrides,
})

const request = (method: string, body?: unknown) =>
  new Request('https://openagents.com/api/operator/pro/status', {
    method,
    ...(body === undefined
      ? {}
      : {
          body: JSON.stringify(body),
          headers: { 'content-type': 'application/json' },
        }),
  })

describe('operator pro status route', () => {
  test('ingests public-safe runner status events and projects live rows', async () => {
    const db = new FakeD1()
    const routes = makeOperatorProStatusRoutes({
      authenticateAgentToken: async () => ({ userId: 'agent_user.owner' }),
      currentIsoTimestamp: () => '2026-07-01T12:03:00.000Z',
    })

    const ingest = await routes.handleOperatorProStatusApi(
      request('POST', statusEvent()),
      { OPENAGENTS_DB: db as unknown as D1Database },
      {} as ExecutionContext,
    )
    expect(ingest.status).toBe(200)

    const read = await routes.handleOperatorProStatusApi(
      request('GET'),
      { OPENAGENTS_DB: db as unknown as D1Database },
      {} as ExecutionContext,
    )
    const body = await read.json() as {
      liveEntries: ReadonlyArray<{ stateStartedAt: string; stateHistory: unknown[] }>
      retainedEntries: ReadonlyArray<unknown>
    }

    expect(body.liveEntries).toHaveLength(1)
    expect(body.retainedEntries).toHaveLength(0)
    expect(body.liveEntries[0]?.stateStartedAt).toBe('2026-07-01T12:00:00.000Z')
    expect(body.liveEntries[0]?.stateHistory).toHaveLength(2)
  })

  test('moves previous live event to retained when the runner posts a new state', async () => {
    const db = new FakeD1()
    const routes = makeOperatorProStatusRoutes({
      authenticateAgentToken: async () => ({ userId: 'agent_user.owner' }),
      currentIsoTimestamp: () => '2026-07-01T12:05:00.000Z',
    })

    await routes.handleOperatorProStatusApi(
      request('POST', statusEvent()),
      { OPENAGENTS_DB: db as unknown as D1Database },
      {} as ExecutionContext,
    )
    await routes.handleOperatorProStatusApi(
      request('POST', statusEvent({
        eventRef: 'event.public.runner_status.2',
        state: 'done',
        stateStartedAt: '2026-07-01T12:04:00.000Z',
        updatedAt: '2026-07-01T12:04:30.000Z',
        stateHistory: [
          ...statusEvent().stateHistory,
          { state: 'done', stateStartedAt: '2026-07-01T12:04:00.000Z' },
        ],
      })),
      { OPENAGENTS_DB: db as unknown as D1Database },
      {} as ExecutionContext,
    )

    const read = await routes.handleOperatorProStatusApi(
      request('GET'),
      { OPENAGENTS_DB: db as unknown as D1Database },
      {} as ExecutionContext,
    )
    const body = await read.json() as {
      liveEntries: ReadonlyArray<unknown>
      retainedEntries: ReadonlyArray<{ state: string }>
    }

    expect(body.liveEntries).toHaveLength(0)
    expect(body.retainedEntries.map(entry => entry.state)).toEqual([
      'done',
      'working',
    ])
  })

  test('rejects private local refs in public projections', async () => {
    const routes = makeOperatorProStatusRoutes({
      authenticateAgentToken: async () => ({ userId: 'agent_user.owner' }),
    })

    const response = await routes.handleOperatorProStatusApi(
      request('POST', statusEvent({ worktreeRef: '/Users/operator/private' })),
      { OPENAGENTS_DB: new FakeD1() as unknown as D1Database },
      {} as ExecutionContext,
    )

    expect(response.status).toBe(400)
  })
})
