import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildPublicArtanisActivity,
  handlePublicArtanisActivityApi,
  makeD1PublicArtanisActivityStore,
  type PublicArtanisActivityStore,
} from './artanis-activity-routes'

const nowIso = '2026-06-27T18:00:00.000Z'

const sampleStore: PublicArtanisActivityStore = {
  listAssignments: async () => [
    {
      acceptance_criteria_refs_json: JSON.stringify([
        'task.public.issue_6411.verify',
        'private:/Users/example/raw-diff.patch',
      ]),
      assignment_ref: 'assignment.private.actual-row-id',
      closeout_refs_json: '[]',
      job_kind: 'codex_agent_task',
      proof_refs_json: JSON.stringify(['proof.public.issue_6411.route_added']),
      public_projection_json: JSON.stringify({
        sourceRefs: [
          'https://github.com/OpenAgentsInc/openagents/issues/6411',
          'email:owner@example.com',
        ],
      }),
      pylon_ref: 'pylon.private.codex.owner',
      rejection_refs_json: '[]',
      state: 'running',
      task_refs_json: JSON.stringify([
        'OpenAgentsInc/openagents#6411',
        'prompt: implement from private prompt text',
      ]),
      updated_at: '2026-06-27T17:55:00.000Z',
    },
    {
      acceptance_criteria_refs_json: '[]',
      assignment_ref: 'assignment.private.failed-row-id',
      closeout_refs_json: '[]',
      job_kind: 'claude_agent_task',
      proof_refs_json: '[]',
      public_projection_json: JSON.stringify({
        sourceRefs: ['OpenAgentsInc/openagents#6386'],
      }),
      pylon_ref: 'pylon.private.claude.owner',
      rejection_refs_json: JSON.stringify([
        'blocker.public.artanis.assignment.local_run_interrupted',
        'secret.private.owner-token',
      ]),
      state: 'failed',
      task_refs_json: JSON.stringify(['OpenAgentsInc/openagents#6386']),
      updated_at: '2026-06-27T17:45:00.000Z',
    },
  ],
  listEvents: async () => [
    {
      assignment_ref: 'assignment.private.actual-row-id',
      created_at: '2026-06-27T17:56:00.000Z',
      event_kind: 'progress',
      event_ref: 'event.private.progress',
      public_projection_json: JSON.stringify({
        sourceRefs: [
          'route:/api/public/artanis/activity',
          'diff:/Users/example/secret.patch',
        ],
      }),
      pylon_ref: 'pylon.private.codex.owner',
      status: 'accepted',
    },
    {
      assignment_ref: 'assignment.private.failed-row-id',
      created_at: '2026-06-27T17:50:00.000Z',
      event_kind: 'worker_closeout',
      event_ref: 'event.private.closeout',
      public_projection_json: JSON.stringify({
        sourceRefs: ['OpenAgentsInc/openagents#6386'],
      }),
      pylon_ref: 'pylon.private.claude.owner',
      status: 'failed',
    },
  ],
  listPylons: async () => [
    {
      capability_refs_json: JSON.stringify(['capability.pylon.local_codex']),
      latest_heartbeat_at: '2026-06-27T17:59:00.000Z',
      public_projection_json: JSON.stringify({
        capacityRefs: [
          'capacity.coding.codex.available=2',
          'owner_agent_user_id.private',
        ],
      }),
      pylon_ref: 'pylon.private.codex.owner',
      resource_mode: 'dedicated',
      status: 'available',
      updated_at: '2026-06-27T17:59:00.000Z',
    },
    {
      capability_refs_json: JSON.stringify(['capability.pylon.local_claude']),
      latest_heartbeat_at: '2026-06-27T17:58:00.000Z',
      public_projection_json: JSON.stringify({
        capacityRefs: ['capacity.coding.claude.available=1'],
      }),
      pylon_ref: 'pylon.private.claude.owner',
      resource_mode: 'background',
      status: 'online',
      updated_at: '2026-06-27T17:58:00.000Z',
    },
  ],
  readBurnPace: async () => ({
    tokens_1h: 1200,
    tokens_24h: 9800,
    turns_1h: 3,
    turns_24h: 18,
  }),
}

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

describe('GET /api/public/artanis/activity', () => {
  test('builds a public-safe Artanis activity envelope', async () => {
    const body = await buildPublicArtanisActivity(sampleStore, {
      limit: 12,
      nowIso,
    })

    expect(body.schemaVersion).toBe('openagents.public_artanis_activity.v1')
    expect(body.generatedAt).toBe(nowIso)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
    expect(body.fleet).toMatchObject({
      capacityAvailable: 3,
      claudeReady: 1,
      codexReady: 1,
      onlineNow: 2,
      registeredTotal: 2,
    })
    expect(body.fleet.genericAgents.map(agent => agent.agentId)).toEqual([
      'Codex-1',
      'Claude-1',
    ])
    expect(body.activeAssignments).toEqual([
      {
        agentId: 'Codex-1',
        assignmentId: 'assignment-1',
        publicIssue: '#6411',
        repo: 'OpenAgentsInc/openagents',
        sourceRefs: [
          'OpenAgentsInc/openagents#6411',
          'task.public.issue_6411.verify',
          'proof.public.issue_6411.route_added',
          'https://github.com/OpenAgentsInc/openagents/issues/6411',
        ],
        state: 'running',
        updatedAt: '2026-06-27T17:55:00.000Z',
        workerFamily: 'codex',
      },
    ])
    expect(body.recentDecisions[0]).toMatchObject({
      agentId: 'Codex-1',
      assignmentId: 'assignment-1',
      decisionId: 'decision-1',
      kind: 'progress',
      sourceRefs: ['route:/api/public/artanis/activity'],
      status: 'accepted',
    })
    expect(body.burnPace).toMatchObject({
      tokensLast24h: 9800,
      tokensLastHour: 1200,
      turnsLast24h: 18,
      turnsLastHour: 3,
    })
    expect(body.failureModes).toEqual([
      {
        count: 1,
        exampleSourceRefs: [
          'OpenAgentsInc/openagents#6386',
        ],
        modeRef: 'blocker.public.artanis.assignment.local_run_interrupted',
      },
      {
        count: 1,
        exampleSourceRefs: [
          'OpenAgentsInc/openagents#6386',
        ],
        modeRef: 'status.public.artanis.assignment.failed',
      },
      {
        count: 1,
        exampleSourceRefs: [
          'OpenAgentsInc/openagents#6386',
        ],
        modeRef: 'status.public.artanis.event.worker_closeout',
      },
    ])
  })

  test('route returns only redacted aggregate and public-source fields', async () => {
    const response = await run(
      handlePublicArtanisActivityApi(
        new Request('https://openagents.com/api/public/artanis/activity'),
        { nowIso: () => nowIso, store: sampleStore },
      ),
    )
    const body = (await response.json()) as Record<string, unknown>
    const wire = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')
    expect(wire).toContain('Codex-1')
    expect(wire).not.toContain('pylon.private')
    expect(wire).not.toContain('assignment.private')
    expect(wire).not.toContain('owner@example.com')
    expect(wire).not.toContain('/Users/example')
    expect(wire).not.toContain('raw-diff')
    expect(wire).not.toContain('owner-token')
    expect(wire).not.toContain('private prompt')
  })

  test('rejects non-GET methods and invalid limits', async () => {
    const post = await run(
      handlePublicArtanisActivityApi(
        new Request('https://openagents.com/api/public/artanis/activity', {
          method: 'POST',
        }),
        { store: sampleStore },
      ),
    )
    const badLimit = await run(
      handlePublicArtanisActivityApi(
        new Request('https://openagents.com/api/public/artanis/activity?limit=999'),
        { store: sampleStore },
      ),
    )

    expect(post.status).toBe(405)
    expect(badLimit.status).toBe(400)
  })
})

const fakeD1 = (): D1Database => {
  const allForSql = (sql: string): ReadonlyArray<Record<string, unknown>> => {
    if (sql.includes('FROM pylon_api_registrations')) {
      return [
        {
          capability_refs_json: JSON.stringify(['capability.pylon.local_codex']),
          latest_heartbeat_at: nowIso,
          public_projection_json: JSON.stringify({
            capacityRefs: ['capacity.coding.codex.available=1'],
          }),
          pylon_ref: 'pylon.db.private',
          resource_mode: 'dedicated',
          status: 'available',
          updated_at: nowIso,
        },
      ]
    }
    if (sql.includes('FROM pylon_api_assignments')) {
      return [
        {
          acceptance_criteria_refs_json: '[]',
          assignment_ref: 'assignment.db.private',
          closeout_refs_json: '[]',
          job_kind: 'codex_agent_task',
          proof_refs_json: '[]',
          public_projection_json: '{}',
          pylon_ref: 'pylon.db.private',
          rejection_refs_json: '[]',
          state: 'running',
          task_refs_json: JSON.stringify(['OpenAgentsInc/openagents#6411']),
          updated_at: nowIso,
        },
      ]
    }
    if (sql.includes('FROM pylon_api_events')) {
      return []
    }
    return []
  }

  return {
    prepare: (sql: string) => ({
      bind: () => ({
        all: async () => ({ results: allForSql(sql) }),
        first: async () => ({
          tokens_1h: 10,
          tokens_24h: 20,
          turns_1h: 1,
          turns_24h: 2,
        }),
      }),
    }),
  } as unknown as D1Database
}

describe('makeD1PublicArtanisActivityStore', () => {
  test('maps D1 rows through the same redacted public projection', async () => {
    const body = await buildPublicArtanisActivity(
      makeD1PublicArtanisActivityStore(fakeD1()),
      { limit: 12, nowIso },
    )

    expect(body.fleet.genericAgents).toEqual([
      {
        agentId: 'Codex-1',
        capacityAvailable: 1,
        family: 'codex',
        onlineNow: true,
        status: 'available',
      },
    ])
    expect(body.activeAssignments[0]).toMatchObject({
      agentId: 'Codex-1',
      publicIssue: '#6411',
      repo: 'OpenAgentsInc/openagents',
    })
    expect(JSON.stringify(body)).not.toContain('pylon.db.private')
    expect(JSON.stringify(body)).not.toContain('assignment.db.private')
  })
})
