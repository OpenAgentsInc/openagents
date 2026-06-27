import { describe, expect, test } from 'vitest'

import { PublicAgentRoute } from '../../../route'
import {
  SucceededLoadPublicAgentGoal,
  SucceededLoadPublicKhalaTokensServedHistory,
} from '../message'
import { PublicKhalaTokensServedHistory, init } from '../model'
import { update } from '../update'
import * as PublicAgent from './publicAgent'

const sampleHistory = PublicKhalaTokensServedHistory.make({
  window: '30d',
  bucket: 'day',
  timezone: 'America/Chicago',
  generatedAt: '2026-06-27T17:00:00.000Z',
  series: [
    { day: '2026-06-25', tokensServed: 50_000_000 },
    { day: '2026-06-26', tokensServed: 328_100_000 },
    { day: '2026-06-27', tokensServed: 100_000_000 },
  ],
})

const loadedArtanisModel = () => {
  const [withGoal] = update(
    init(PublicAgentRoute({ agentRef: 'artanis' })),
    SucceededLoadPublicAgentGoal({
      agentRef: 'artanis',
      response: {
        agentId: 'agent_artanis',
        events: [],
        goal: {
          id: 'goal_artanis',
          agentId: 'agent_artanis',
          objective: 'Drive the public Khala improvement loop.',
          status: 'active',
          currentRunId: 'run_artanis',
          tokenBudget: null,
          tokensUsed: 1,
          timeUsedSeconds: 1,
          remainingTokens: null,
          createdAt: '2026-06-27T00:00:00.000Z',
          updatedAt: '2026-06-27T17:00:00.000Z',
          completedAt: null,
          publicUrl: '/artanis',
        },
      },
    }),
  )

  const [withHistory] = update(
    withGoal,
    SucceededLoadPublicKhalaTokensServedHistory({ history: sampleHistory }),
  )

  return withHistory
}

describe('public Artanis Pulse panel', () => {
  test('renders aggregate token-burn sparkline and daily pace targets', () => {
    const markup = JSON.stringify(
      PublicAgent.view(loadedArtanisModel(), 'artanis'),
    )

    expect(markup).toContain('The Pulse')
    expect(markup).toContain('Live token burn')
    expect(markup).toContain('Recent daily token burn sparkline')
    expect(markup).toContain('Behind 4x floor')
    expect(markup).toContain('Today 100M')
    expect(markup).toContain('Projected 200M')
    expect(markup).toContain('Yesterday 328.1M')
    expect(markup).toContain('Daily target')
    expect(markup).toContain('10x yesterday / 4x floor 1.3B')
    expect(markup).toContain('3% of 10x')
    expect(markup).toContain('Gap 1.1B')
    expect(markup).toContain('2026-06-27: 100,000,000 tokens')
    expect(markup).toContain('no user, prompt, or provider rows exposed')
    expect(markup).not.toContain('accountRef')
    expect(markup).not.toContain('raw prompt')
  })
})
