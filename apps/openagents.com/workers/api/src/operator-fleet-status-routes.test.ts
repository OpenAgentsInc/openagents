import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import {
  clearOperatorFleetStatusCacheForTests,
  makeOperatorFleetStatusRoutes,
} from './operator-fleet-status-routes'
import { makeOperatorProStatusRoutes } from './operator-pro-status-routes'

type QueryLog = Array<Readonly<{ sql: string; bindings: ReadonlyArray<unknown> }>>

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
    this.db.log.push({ sql: this.sql, bindings: this.bindings })
    return { results: this.db.all(this.sql, this.bindings) as ReadonlyArray<T> }
  }

  async first<T>(): Promise<T | null> {
    this.db.log.push({ sql: this.sql, bindings: this.bindings })
    return (this.db.all(this.sql, this.bindings)[0] ?? null) as T | null
  }
}

class FakeD1 {
  readonly log: QueryLog = []
  readonly statuses: Array<StoredStatus> = [
    storedStatus({
      event_ref: 'event.public.runner_status.working',
      runner_ref: 'runner.public.codex.working',
      assignment_ref: 'assignment.public.issue_7879',
      state: 'working',
      state_started_at: '2026-07-01T12:00:00.000Z',
      updated_at: '2026-07-01T12:02:00.000Z',
      event_json: JSON.stringify(statusEvent({
        eventRef: 'event.public.runner_status.working',
        runnerRef: 'runner.public.codex.working',
        assignmentRef: 'assignment.public.issue_7879',
        refs: [
          'status.public.runner.working',
          'capacity.coding.codex.available=2',
          'capacity.coding.codex.ready=2',
          'load.coding.codex.busy=1',
        ],
      })),
    }),
    storedStatus({
      event_ref: 'event.public.runner_status.blocked',
      runner_ref: 'runner.public.claude.blocked',
      runner_kind: 'claude_agent',
      pylon_ref: 'pylon.public.claude',
      assignment_ref: 'assignment.public.issue_7880',
      state: 'blocked',
      state_started_at: '2026-07-01T12:01:00.000Z',
      updated_at: '2026-07-01T12:02:30.000Z',
      event_json: JSON.stringify(statusEvent({
        eventRef: 'event.public.runner_status.blocked',
        runnerRef: 'runner.public.claude.blocked',
        runnerKind: 'claude_agent',
        state: 'blocked',
        assignmentRef: 'assignment.public.issue_7880',
        pylonRef: 'pylon.public.claude',
        blockerRefs: ['blocker.public.runner.needs_owner'],
      })),
    }),
    storedStatus({
      event_ref: 'event.public.runner_status.done',
      runner_ref: 'runner.public.codex.done',
      assignment_ref: 'assignment.public.done',
      state: 'done',
      retention_state: 'retained',
      state_started_at: '2026-07-01T11:00:00.000Z',
      updated_at: '2026-07-01T11:30:00.000Z',
      retained_at: '2026-07-01T11:30:00.000Z',
      event_json: JSON.stringify(statusEvent({
        eventRef: 'event.public.runner_status.done',
        runnerRef: 'runner.public.codex.done',
        assignmentRef: 'assignment.public.done',
        state: 'done',
        stateStartedAt: '2026-07-01T11:00:00.000Z',
        updatedAt: '2026-07-01T11:30:00.000Z',
      })),
    }),
    storedStatus({
      event_ref: 'event.public.runner_status.other_owner',
      owner_agent_user_id: 'agent_user.other',
      runner_ref: 'runner.public.codex.other',
      assignment_ref: 'assignment.public.other',
      state: 'working',
      event_json: JSON.stringify(statusEvent({
        eventRef: 'event.public.runner_status.other_owner',
        runnerRef: 'runner.public.codex.other',
        assignmentRef: 'assignment.public.other',
      })),
    }),
  ]

  prepare(sql: string): FakeStatement {
    return new FakeStatement(this, sql)
  }

  all(sql: string, bindings: ReadonlyArray<unknown>): ReadonlyArray<unknown> {
    if (sql.includes('FROM pylon_agent_runner_status_events')) {
      const retention = sql.includes('retention_state = ?')
        ? String(bindings[0])
        : null
      const owners = sql.includes('owner_agent_user_id = ?')
        ? [String(bindings[0])]
        : sql.includes('owner_agent_user_id IN')
          ? bindings.slice(1).map(value => String(value))
          : []
      return this.statuses
        .filter(row => row.archived_at === null)
        .filter(row => retention === null || row.retention_state === retention)
        .filter(row => owners.length === 0 || owners.includes(row.owner_agent_user_id))
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
    }

    return []
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
  assignmentRef: 'assignment.public.issue_7879',
  pylonRef: 'pylon.public.codex',
  refs: ['status.public.runner.working'],
  capabilityRefs: ['capability.pylon.local_codex'],
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

const storedStatus = (overrides: Partial<StoredStatus> = {}): StoredStatus => ({
  event_ref: 'event.public.runner_status.1',
  owner_agent_user_id: 'agent_user.owner',
  runner_ref: 'runner.public.codex.1',
  runner_kind: 'codex_sdk',
  pylon_ref: 'pylon.public.codex',
  assignment_ref: 'assignment.public.issue_7879',
  state: 'working',
  state_started_at: '2026-07-01T12:00:00.000Z',
  updated_at: '2026-07-01T12:02:00.000Z',
  retention_state: 'live',
  event_json: JSON.stringify(statusEvent()),
  created_at: '2026-07-01T12:02:00.000Z',
  retained_at: null,
  archived_at: null,
  ...overrides,
})

const request = (method = 'GET', path = '/api/operator/fleet/state'): Request =>
  new Request(`https://openagents.com${path}`, { method })

describe('operator fleet status route', () => {
  test('requires GET', async () => {
    clearOperatorFleetStatusCacheForTests()
    const routes = makeOperatorFleetStatusRoutes({
      requireAdminApiToken: async () => true,
    })
    const response = await routes.handleOperatorFleetStatusApi(
      request('POST'),
      { OPENAGENTS_DB: new FakeD1() as unknown as D1Database },
    )

    expect(response.status).toBe(405)
  })

  test('requires operator auth', async () => {
    clearOperatorFleetStatusCacheForTests()
    const routes = makeOperatorFleetStatusRoutes({
      requireAdminApiToken: async () => false,
    })
    const response = await routes.handleOperatorFleetStatusApi(
      request(),
      { OPENAGENTS_DB: new FakeD1() as unknown as D1Database },
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('returns one cached fleet status shape derived from the runner spine', async () => {
    clearOperatorFleetStatusCacheForTests()
    const db = new FakeD1()
    const routes = makeOperatorFleetStatusRoutes({
      currentIsoTimestamp: () => '2026-07-01T12:03:00.000Z',
      requireAdminApiToken: async () => true,
    })
    const env = { OPENAGENTS_DB: db as unknown as D1Database }
    const first = await routes.handleOperatorFleetStatusApi(request(), env)
    const second = await routes.handleOperatorFleetStatusApi(request(), env)
    const body = await first.json() as Record<string, any>
    const cachedBody = await second.json() as Record<string, any>

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    expect(first.headers.get('x-openagents-cache')).toBe('miss')
    expect(second.headers.get('x-openagents-cache')).toBe('hit')
    expect(cachedBody).toEqual(body)
    expect(db.log).toHaveLength(1)
    expect(db.log[0]?.sql).toContain('FROM pylon_agent_runner_status_events')
    expect(body).toMatchObject({
      authority: {
        buyerChargeMutationAllowed: false,
        dispatchMutationAllowed: false,
        payoutMutationAllowed: false,
        settlementMutationAllowed: false,
      },
      brain: {
        loopHealth: 'stalled',
      },
      fleet: {
        activeAssignmentCount: 3,
        activeAssignments: [
          {
            assignmentRef: 'assignment.public.issue_7880',
            phase: 'blocked',
            state: 'blocked',
          },
          {
            assignmentRef: 'assignment.public.issue_7879',
            elapsedMs: 180000,
            jobKind: 'codex_sdk',
            phase: 'running',
            state: 'running',
          },
          {
            assignmentRef: 'assignment.public.other',
            state: 'running',
          },
        ],
        sourceRefs: ['d1:pylon_agent_runner_status_events'],
      },
      generatedAt: '2026-07-01T12:03:00.000Z',
      schemaVersion: 'operator.fleet_status.v1',
      spine: {
        liveRunnerCount: 3,
        retainedRunnerCount: 1,
        schemaVersion: 'openagents.pylon.agent_runner_status_event.v1',
      },
      watchdog: {
        activeLeases: 3,
        state: 'STALLED',
      },
    })
    expect(JSON.stringify(body)).not.toContain('/Users/')
    expect(JSON.stringify(body)).not.toContain('auth.json')
  })

  test('keeps owner-scoped state reads filtered to the registered agent user id', async () => {
    clearOperatorFleetStatusCacheForTests()
    const db = new FakeD1()
    const routes = makeOperatorFleetStatusRoutes({
      authenticateAgentToken: async () => ({ userId: 'agent_user.owner' }),
      currentIsoTimestamp: () => '2026-07-01T12:03:00.000Z',
      requireAdminApiToken: async () => false,
    })
    const response = await routes.handleOperatorFleetStatusApi(
      request('GET', '/api/operator/fleet/state'),
      { OPENAGENTS_DB: db as unknown as D1Database },
    )
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(200)
    expect(db.log[0]?.sql).toContain('owner_agent_user_id = ?')
    expect(db.log[0]?.bindings).toEqual(['agent_user.owner', 200])
    expect(body.fleet.activeAssignments.map((row: { assignmentRef: string }) => row.assignmentRef))
      .toEqual([
        'assignment.public.issue_7880',
        'assignment.public.issue_7879',
      ])
  })

  test('legacy fleet status path remains admin-token only and carries deprecation markers', async () => {
    clearOperatorFleetStatusCacheForTests()
    const routes = makeOperatorFleetStatusRoutes({
      authenticateAgentToken: async () => ({ userId: 'agent_user.owner' }),
      currentIsoTimestamp: () => '2026-07-01T12:03:00.000Z',
      requireAdminApiToken: async request =>
        request.headers.get('authorization') === 'Bearer admin',
    })

    const agentResponse = await routes.handleOperatorFleetStatusApi(
      request('GET', '/api/operator/fleet/status'),
      { OPENAGENTS_DB: new FakeD1() as unknown as D1Database },
    )
    const adminResponse = await routes.handleOperatorFleetStatusApi(
      new Request('https://openagents.com/api/operator/fleet/status', {
        headers: { authorization: 'Bearer admin' },
      }),
      { OPENAGENTS_DB: new FakeD1() as unknown as D1Database },
    )
    const body = await adminResponse.json() as Record<string, any>

    expect(agentResponse.status).toBe(401)
    expect(agentResponse.headers.get('cache-control')).toBe('no-store')
    expect(adminResponse.status).toBe(200)
    expect(adminResponse.headers.get('deprecation')).toBe('true')
    expect(adminResponse.headers.get('link')).toContain('/api/operator/pro/status')
    expect(body.deprecation).toMatchObject({
      deprecated: true,
      replacementPath: '/api/operator/pro/status',
      sourceOfTruth: 'operator_pro_status_spine',
    })
    expect(body.deprecation.removalCondition).toContain('T11.1')
  })

  test('fleet compat active assignments match the pro status spine projection', async () => {
    clearOperatorFleetStatusCacheForTests()
    const db = new FakeD1()
    const fleetRoutes = makeOperatorFleetStatusRoutes({
      currentIsoTimestamp: () => '2026-07-01T12:03:00.000Z',
      requireAdminApiToken: async () => true,
    })
    const proRoutes = makeOperatorProStatusRoutes({
      currentIsoTimestamp: () => '2026-07-01T12:03:00.000Z',
      requireAdminApiToken: async () => true,
    })
    const env = { OPENAGENTS_DB: db as unknown as D1Database }

    const fleet = await fleetRoutes.handleOperatorFleetStatusApi(
      request('GET', '/api/operator/fleet/status'),
      env,
    )
    const pro = await proRoutes.handleOperatorProStatusApi(
      new Request('https://openagents.com/api/operator/pro/status'),
      env,
      {} as ExecutionContext,
    )
    const fleetBody = await fleet.json() as {
      fleet: { activeAssignments: ReadonlyArray<{ assignmentRef: string }> }
    }
    const proBody = await pro.json() as {
      liveEntries: ReadonlyArray<{ prompt: string }>
    }

    expect(fleetBody.fleet.activeAssignments.map(entry => entry.assignmentRef))
      .toEqual(proBody.liveEntries.map(entry => entry.prompt))
  })

  test('old fleet status route has no remaining direct reads from bespoke snapshot sources', () => {
    const source = readFileSync(
      new URL('./operator-fleet-status-routes.ts', import.meta.url),
      'utf8',
    )

    expect(source).toContain('makePylonAgentRunnerStatusReadStoreForEnv')
    expect(source).toContain('PylonAgentRunnerStatusReadStore')
    expect(source).not.toContain('FROM pylon_api_registrations')
    expect(source).not.toContain('FROM pylon_api_assignments')
    expect(source).not.toContain('FROM pylon_api_events')
    expect(source).not.toContain('FROM provider_accounts')
    expect(source).not.toContain('FROM fleet_alerts')
    expect(source).not.toContain('FROM glm_fleet_readiness_heartbeats')
    expect(source).not.toContain('FROM artanis_owner_memory')
  })
})
