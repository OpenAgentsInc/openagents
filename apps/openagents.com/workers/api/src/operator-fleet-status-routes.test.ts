import { describe, expect, test } from 'vitest'

import {
  clearOperatorFleetStatusCacheForTests,
  makeOperatorFleetStatusRoutes,
} from './operator-fleet-status-routes'

type QueryLog = Array<string>

class FakeStatement {
  constructor(
    private readonly sql: string,
    private readonly log: QueryLog,
  ) {}

  bind(): FakeStatement {
    return this
  }

  async all<T>(): Promise<{ results: ReadonlyArray<T> }> {
    this.log.push(this.sql)
    return { results: rowsForSql(this.sql) as ReadonlyArray<T> }
  }

  async first<T>(): Promise<T | null> {
    this.log.push(this.sql)
    const rows = rowsForSql(this.sql)
    return (rows[0] ?? null) as T | null
  }
}

const rowsForSql = (sql: string): ReadonlyArray<Record<string, unknown>> => {
  if (sql.includes('FROM pylon_api_registrations')) {
    return [
      {
        capability_refs_json: JSON.stringify(['capability.pylon.local_codex']),
        latest_capacity_refs_json: JSON.stringify([
          'capacity.coding.codex.available=3',
          'capacity.coding.codex.ready=2',
        ]),
        latest_heartbeat_at: '2026-06-27T18:40:55.000Z',
        latest_load_refs_json: JSON.stringify([
          'load.coding.codex.busy=1',
          'load.coding.codex.queued=0',
        ]),
        owner_agent_user_id: 'agent_owner_public',
        pylon_ref: 'pylon.public.codex_one',
        status: 'active',
      },
    ]
  }

  if (sql.includes('FROM pylon_api_assignments')) {
    return [
      {
        assignment_ref: 'assignment.public.issue_6427',
        created_at: '2026-06-27T18:30:00.000Z',
        job_kind: 'codex_agent_task',
        lease_expires_at: '2026-06-27T19:30:00.000Z',
        pylon_ref: 'pylon.public.codex_one',
        state: 'running',
        updated_at: '2026-06-27T18:39:00.000Z',
      },
      {
        assignment_ref: 'assignment.public.issue_6428',
        created_at: '2026-06-27T18:35:00.000Z',
        job_kind: 'codex_agent_task',
        lease_expires_at: '2026-06-27T19:35:00.000Z',
        pylon_ref: 'pylon.public.codex_one',
        state: 'running',
        updated_at: '2026-06-27T18:40:00.000Z',
      },
    ]
  }

  if (sql.includes('FROM pylon_api_events')) {
    return [
      {
        assignment_ref: 'assignment.public.issue_6427',
        created_at: '2026-06-27T18:40:30.000Z',
        event_body_json: JSON.stringify({
          elapsedMs: 630000,
          lastProgressEvent: 'turn.completed',
          message: 'Runtime phase: testing.',
          phase: 'testing',
          tokensSoFar: 4242,
        }),
      },
    ]
  }

  if (sql.includes('FROM pylon_codex_raw_event_chunks')) {
    return [
      {
        assignment_ref: 'assignment.public.issue_6427',
        latest_observed_at: '2026-06-27T18:40:40.000Z',
        raw_chunk_bytes: 16000,
        raw_chunk_count: 4,
      },
      {
        assignment_ref: 'assignment.public.issue_6428',
        latest_observed_at: '2026-06-27T18:40:50.000Z',
        raw_chunk_bytes: 32000,
        raw_chunk_count: 8,
      },
    ]
  }

  if (sql.includes('FROM provider_accounts')) {
    return [
      {
        cooldown_until: '2026-06-27T19:00:00.000Z',
        health: 'healthy',
        lease_limit: 2,
        low_credit_flag: 0,
        provider: 'chatgpt_codex',
        provider_account_ref: 'acct_hash_codex_3',
        reauth_required_reason: null,
        recent_failure_class: 'rate_limited',
        status: 'connected',
      },
      {
        cooldown_until: null,
        health: 'healthy',
        lease_limit: 1,
        low_credit_flag: 0,
        provider: 'chatgpt_codex',
        provider_account_ref: 'acct_hash_codex_4',
        reauth_required_reason: null,
        recent_failure_class: null,
        status: 'connected',
      },
    ]
  }

  if (sql.includes('FROM fleet_alerts')) {
    if (sql.includes('serving_rate_low')) {
      return [
        {
          active_assignments: 0,
          alert_ref: 'serving_rate_alert.serving_rate_low.2026-06-27T18:39:00.000Z.feedface',
          classification: 'serving_rate_low',
          detected_at: '2026-06-27T18:39:00.000Z',
          queued_assignments: 0,
          reason_ref: 'blocker.public.serving_rate.tokens_per_hour_below_floor',
        },
      ]
    }
    return [
      {
        active_assignments: 1,
        alert_ref: 'alert.public.fleet.stalled',
        classification: 'stalled',
        detected_at: '2026-06-27T18:39:30.000Z',
        queued_assignments: 0,
        reason_ref: 'reason.public.fleet.no_token_burn',
      },
    ]
  }

  if (sql.includes('tokens_today')) {
    return [{ tokens_today: 1200 }]
  }

  if (sql.includes('tokens_yesterday')) {
    return [{ tokens_yesterday: 250 }]
  }

  if (sql.includes('tokens_window')) {
    if (sql.includes('pylon-codex-own-capacity')) {
      return [{
        assignments_window: 2,
        tokens_window: 900,
        turns_window: 3,
      }]
    }
    return [{ tokens_window: 600 }]
  }

  if (sql.includes('FROM artanis_owner_memory')) {
    return [
      {
        body: 'Fan out bounded public issue work through caller-owned Codex capacity.',
        created_at: '2026-06-27T18:38:00.000Z',
        memory_ref: 'decision.public.artanis.codex_burndown',
        note_category: 'decision',
      },
    ]
  }

  if (sql.includes('FROM glm_fleet_readiness_heartbeats')) {
    return [
      { health_status: 'ok', replica_id: 'glm-1' },
      { health_status: 'degraded', replica_id: 'glm-2' },
    ]
  }

  return []
}

const fakeDb = (log: QueryLog): D1Database =>
  ({
    prepare: (sql: string) => new FakeStatement(sql, log),
  }) as unknown as D1Database

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
      { OPENAGENTS_DB: fakeDb([]) },
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
      { OPENAGENTS_DB: fakeDb([]) },
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('returns one cached public-safe fleet snapshot', async () => {
    clearOperatorFleetStatusCacheForTests()
    const log: QueryLog = []
    const routes = makeOperatorFleetStatusRoutes({
      currentIsoTimestamp: () => '2026-06-27T18:41:00.000Z',
      requireAdminApiToken: async () => true,
    })
    const env = { OPENAGENTS_DB: fakeDb(log) }
    const first = await routes.handleOperatorFleetStatusApi(request(), env)
    const second = await routes.handleOperatorFleetStatusApi(request(), env)
    const body = await first.json() as Record<string, any>
    const cachedBody = await second.json() as Record<string, any>

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    expect(first.headers.get('x-openagents-cache')).toBe('miss')
    expect(second.headers.get('x-openagents-cache')).toBe('hit')
    expect(log.length).toBeGreaterThan(0)
    expect(log.length).toBe(13)
    expect(cachedBody).toEqual(body)
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
        activeAssignmentCount: 2,
        activeAssignments: [
          {
            assignmentRef: 'assignment.public.issue_6427',
            elapsedMs: 630000,
            lastLog: 'Runtime phase: testing.',
            lastProgressEvent: 'turn.completed',
            lastUpdateAgeMs: 120000,
            phase: 'testing',
            progressAgeMs: 30000,
            progressObservedAt: '2026-06-27T18:40:30.000Z',
            tokensSoFar: 4242,
          },
          {
            assignmentRef: 'assignment.public.issue_6428',
            elapsedMs: 360000,
            lastLog: null,
            lastProgressEvent: null,
            lastUpdateAgeMs: 60000,
            phase: 'running',
            progressAgeMs: null,
            progressObservedAt: null,
            tokensSoFar: null,
          },
        ],
        activeSlots: 3,
        busySlots: 1,
        queuedSlots: 0,
        readySlots: 2,
      },
      generatedAt: '2026-06-27T18:41:00.000Z',
      glm: {
        readyReplicas: 1,
        status: 'degraded',
        totalReplicas: 2,
      },
      pace: {
        activeAdjustedTokensPerMinute: 1797,
        activeSessionTokenEstimate: {
          activeAssignmentCount: 2,
          assignments: [
            {
              assignmentRef: 'assignment.public.issue_6427',
              rawChunkBytes: 16000,
              rawChunkCount: 4,
              rawChunkObservedAt: '2026-06-27T18:40:40.000Z',
              source: 'assignment_progress.tokensSoFar',
              tokenCountKind: 'estimated',
              tokens: 4242,
              tokensPerMinute: 404,
            },
            {
              assignmentRef: 'assignment.public.issue_6428',
              rawChunkBytes: 32000,
              rawChunkCount: 8,
              rawChunkObservedAt: '2026-06-27T18:40:50.000Z',
              source: 'pylon_codex_raw_event_chunks.byte_length',
              tokenCountKind: 'estimated',
              tokens: 8000,
              tokensPerMinute: 1333,
            },
          ],
          caveatRefs: ['caveat.public.operator_fleet_status.active_session_tokens_estimated'],
          inFlightTokens: 12242,
          inFlightTokensPerMinute: 1737,
        },
        liveBurnRateTokensPerMinute: 60,
        ownCapacityCodex: {
          assignmentsWindow: 2,
          sourceRefs: ['d1:token_usage_events.pylon_codex_own_capacity_exact'],
          tokensPerMinute: 90,
          tokensWindow: 900,
          turnsWindow: 3,
          windowSeconds: 600,
        },
        paceToFloor: 'ahead',
        targetFloorTokens: 1000,
        todayTokens: 1200,
        yesterdayTokens: 250,
      },
      accounts: {
        healthyCount: 1,
        limitedCount: 1,
        status: [
          {
            accountRefHash: 'acct_hash_codex_3',
            concurrency: 2,
            provider: 'codex',
            reason: 'rate_limited',
            resetAt: '2026-06-27T19:00:00.000Z',
            status: 'rate_limited',
          },
          {
            accountRefHash: 'acct_hash_codex_4',
            concurrency: 1,
            provider: 'codex',
            reason: null,
            resetAt: null,
            status: 'healthy',
          },
        ],
      },
      schemaVersion: 'operator.fleet_status.v1',
      servingRateMonitor: {
        latestAlert: {
          classification: 'serving_rate_low',
          reasonRef: 'blocker.public.serving_rate.tokens_per_hour_below_floor',
        },
        state: 'SERVING_RATE_LOW',
      },
      supervisor: {
        availableCodexSlots: 3,
        desiredCodexSlots: 2,
        queueDepth: 0,
        state: 'ready',
      },
      watchdog: {
        activeLeases: 2,
        state: 'STALLED',
      },
    })
    expect(JSON.stringify(body)).not.toContain('/Users/')
    expect(JSON.stringify(body)).not.toContain('auth.json')
    expect(JSON.stringify(body)).not.toContain('Fan out bounded')
  })

  test('accepts a registered agent token through the owner-scoped state path', async () => {
    clearOperatorFleetStatusCacheForTests()
    const log: QueryLog = []
    const routes = makeOperatorFleetStatusRoutes({
      authenticateAgentToken: async () => ({ userId: 'agent_owner_public' }),
      currentIsoTimestamp: () => '2026-06-27T18:41:00.000Z',
      requireAdminApiToken: async () => false,
    })
    const response = await routes.handleOperatorFleetStatusApi(
      request('GET', '/api/operator/fleet/state'),
      { OPENAGENTS_DB: fakeDb(log) },
    )
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(200)
    expect(body.fleet.sourceRefs).toContain('d1:pylon_api_registrations')
    expect(log.some(sql => sql.includes('owner_agent_user_id = ?'))).toBe(true)
    expect(log.some(sql => sql.includes('AND user_id = ?'))).toBe(true)
    expect(log.some(sql => sql.includes('FROM pylon_api_events') && sql.includes('pylon_ref IN'))).toBe(true)
    expect(log.some(sql =>
      sql.includes('FROM token_usage_events') &&
      sql.includes("provider = 'pylon-codex-own-capacity'") &&
      sql.includes('AND actor_user_id = ?'),
    )).toBe(true)
  })

  test('keeps the legacy fleet status path admin-token only', async () => {
    clearOperatorFleetStatusCacheForTests()
    const routes = makeOperatorFleetStatusRoutes({
      authenticateAgentToken: async () => ({ userId: 'agent_owner_public' }),
      currentIsoTimestamp: () => '2026-06-27T18:41:00.000Z',
      requireAdminApiToken: async () => false,
    })

    const response = await routes.handleOperatorFleetStatusApi(
      request('GET', '/api/operator/fleet/status'),
      { OPENAGENTS_DB: fakeDb([]) },
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })
})
