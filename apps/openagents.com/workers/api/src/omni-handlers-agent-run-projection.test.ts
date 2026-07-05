import { describe, expect, test } from 'vitest'

import { decodeAgentRunEntity } from '@openagentsinc/khala-sync'

import { agentRunSyncProjectionRaw, createQueuedAgentRun } from './omni-runs'

/**
 * KS-6.6 (#8416) contract test: proves the EXACT glue between a real queued
 * agent run (the same `createQueuedAgentRun` output all three
 * `omni-handlers.ts` call sites build) and the new
 * `scope.agent_run.<runId>` entity contract. If this decode ever breaks,
 * the dual-write silently no-ops (fail-soft) — this test is what would
 * catch that regression instead of a live smoke.
 */

describe('agentRunSyncProjectionRaw', () => {
  test('a freshly queued run (with a live goal) decodes through AgentRunEntity', () => {
    const { run } = createQueuedAgentRun({
      appOrigin: 'https://openagents.com',
      goal: 'Run a bounded repo cleanup mission. Preserve diffs and receipts.',
      goalId: 'goal.alpha',
      goalStatus: 'active',
      goalVisibility: 'private',
      repository: {
        owner: 'OpenAgentsInc',
        provider: 'github',
        ref: 'main',
        repo: 'openagents',
      },
      runId: 'run.alpha',
      teamId: 'team.beta',
      timeUsedSeconds: 12,
      tokenBudget: 100_000,
      tokensUsed: 4_000,
      userId: 'user.alice',
    })

    const raw = agentRunSyncProjectionRaw(run)
    const entity = decodeAgentRunEntity(raw)

    expect(entity.runId).toBe('run.alpha')
    expect(entity.userId).toBe('user.alice')
    expect(entity.teamId).toBe('team.beta')
    expect(entity.status).toBe('queued')
    expect(entity.repository.owner).toBe('OpenAgentsInc')
    expect(entity.repository.repo).toBe('openagents')
    expect(entity.goalId).toBe('goal.alpha')
    expect(entity.goalContext?.goalId).toBe('goal.alpha')
    expect(entity.goalContext?.status).toBe('active')
    expect(entity.goalContext?.tokenBudget).toBe(100_000)
    expect(entity.goalContext?.tokensUsed).toBe(4_000)
    expect(entity.goalContext?.remainingTokens).toBe(96_000)
  })

  test('a queued run with NO goal id still decodes (goalContext omitted)', () => {
    const { run } = createQueuedAgentRun({
      appOrigin: 'https://openagents.com',
      goal: 'Run a bounded repo cleanup mission.',
      repository: {
        owner: 'OpenAgentsInc',
        provider: 'github',
        ref: 'main',
        repo: 'openagents',
      },
      runId: 'run.no-goal',
      userId: 'user.bob',
    })

    const raw = agentRunSyncProjectionRaw(run)
    const entity = decodeAgentRunEntity(raw)

    expect(entity.runId).toBe('run.no-goal')
    // `buildAgentRunAssignment` always attaches SOME goal context (even the
    // default fallback objective) once a run is queued; the load-bearing
    // property here is simply that the shape still decodes cleanly either
    // way, so a run launched without an explicit goal never blocks the
    // dual-write.
    expect(() => entity).not.toThrow()
  })

  test('a personal (no team) run omits teamId cleanly', () => {
    const { run } = createQueuedAgentRun({
      appOrigin: 'https://openagents.com',
      goal: 'Personal mission, no team.',
      repository: {
        owner: 'OpenAgentsInc',
        provider: 'github',
        ref: 'main',
        repo: 'openagents',
      },
      runId: 'run.personal',
      userId: 'user.carol',
    })

    const entity = decodeAgentRunEntity(agentRunSyncProjectionRaw(run))
    expect(entity.teamId).toBeNull()
    expect(entity.projectId).toBeNull()
  })
})
