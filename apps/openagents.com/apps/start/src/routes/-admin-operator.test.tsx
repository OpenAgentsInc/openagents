import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { OverviewDashboard } from './-admin-operator-page'
import {
  fetchOperatorOverview,
  type OverviewSnapshot,
} from './-admin-operator-fetch'

const snapshot: OverviewSnapshot = {
  ok: true,
  generatedAt: '2026-07-22T12:00:00.000Z',
  agentChains: {
    activeCount: 1,
    recentCount: 1,
    chains: [
      {
        assignmentRef: 'assign:1',
        pylonRef: 'pylon:alpha',
        ownerUserId: 'agent:owner',
        jobKind: 'codex_agent_task',
        state: 'running',
        active: true,
        leaseExpiresAt: '2026-07-22T12:30:00.000Z',
        createdAt: '2026-07-22T11:00:00.000Z',
        updatedAt: '2026-07-22T11:45:00.000Z',
        projection: { objective: 'Implement issue #9188' },
        events: [
          {
            eventRef: 'event:1',
            eventKind: 'assignment_progress',
            status: 'ok',
            createdAt: '2026-07-22T11:40:00.000Z',
            projection: { phase: 'proof-ready' },
          },
        ],
      },
    ],
  },
  tokens: {
    total: { events: 2, tokens: 2000 },
    last24h: { events: 2, tokens: 2000 },
    byDemandSource: [
      { demandSource: 'khala_coding_delegation', events: 1, tokens: 1200 },
    ],
    byProvider: [
      { provider: 'pylon-codex-own-capacity', events: 1, tokens: 1200 },
    ],
    recent: [],
  },
  traces: [
    {
      traceUuid: 'trace-uuid-1',
      ownerUserId: 'agent:owner',
      agentRef: 'agent:owner',
      schemaVersion: 'ATIF-v1.7',
      visibility: 'owner_only',
      stepCount: 12,
      demandKind: 'own_capacity',
      demandSource: 'khala_coding_delegation',
      createdAt: '2026-07-22T11:42:00.000Z',
    },
  ],
  fleet: {
    pylons: [
      {
        pylonRef: 'pylon:alpha',
        displayName: 'Alpha node',
        status: 'online',
        resourceMode: 'own_capacity',
        walletReady: true,
        latestHeartbeatAt: '2026-07-22T11:30:00.000Z',
        updatedAt: '2026-07-22T11:30:00.000Z',
      },
    ],
    onlineCount: 1,
    totalCount: 1,
  },
  cloudHealth: {
    lastOrgCloudTurnAt: { status: 'ok', value: '2026-07-22T11:00:00.000Z' },
  },
}

const okResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })

describe('admin operator dashboard page (#9188)', () => {
  test('renders the agent chains, tokens, fleet, and traces at a glance', () => {
    const html = renderToStaticMarkup(
      <OverviewDashboard
        snapshot={snapshot}
        nowMs={Date.parse('2026-07-22T12:00:00.000Z')}
        side={{}}
        lastRefreshedAt={Date.parse('2026-07-22T12:00:00.000Z')}
        refreshing={false}
      />,
    )
    expect(html).toContain('Operator dashboard')
    expect(html).toContain('Agent chains')
    expect(html).toContain('assign:1')
    expect(html).toContain('assignment_progress')
    expect(html).toContain('Implement issue #9188')
    expect(html).toContain('Alpha node')
    expect(html).toContain('/trace/trace-uuid-1')
    // Token totals are formatted with a thousands separator.
    expect(html).toContain('2,000')
    expect(html).toContain('codex_agent_task')
  })

  test('fetchOperatorOverview maps admin/non-admin/anon responses', async () => {
    const loaded = await fetchOperatorOverview(async () => okResponse(snapshot))
    expect(loaded.tag).toBe('loaded')

    const forbidden = await fetchOperatorOverview(async () =>
      okResponse({ error: 'forbidden' }, 403),
    )
    expect(forbidden.tag).toBe('forbidden')

    const unauthorized = await fetchOperatorOverview(async () =>
      okResponse({ error: 'unauthorized' }, 401),
    )
    expect(unauthorized.tag).toBe('unauthorized')

    const failed = await fetchOperatorOverview(async () =>
      okResponse({ error: 'boom' }, 500),
    )
    expect(failed.tag).toBe('failed')
  })
})
