import { readFileSync } from 'node:fs'

import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { AgentUpdateGoalRequest } from './agent-goal-routes'
import { AgentGoalEventRecord } from './agent-goal-runtime'
import { AgentGoalRecord } from './agent-goals'
import {
  agentGoalSyncValue,
  publicAgentGoalEventSyncValue,
  publicAgentGoalSyncValue,
} from './sync-notifier'

const now = '2026-06-04T18:00:00.000Z'

const goalRecord = (
  overrides: Partial<AgentGoalRecord> = {},
): AgentGoalRecord =>
  new AgentGoalRecord({
    id: 'goal_1',
    agentId: 'agent_artanis',
    userId: 'github:14167547',
    teamId: 'team_openagents_core',
    projectId: 'project_artanis',
    objective: 'Publish safe Artanis progress.',
    status: 'active',
    visibility: 'public',
    currentRunId: 'agent_run_1',
    tokenBudget: 10000,
    tokensUsed: 250,
    timeUsedSeconds: 60,
    createdAt: now,
    updatedAt: now,
    completedAt: null,
    pausedAt: null,
    blockedAt: null,
    archivedAt: null,
    ...overrides,
  })

const eventRecord = (
  overrides: Partial<AgentGoalEventRecord> = {},
): AgentGoalEventRecord =>
  new AgentGoalEventRecord({
    id: 'goal_event_1',
    goalId: 'goal_1',
    runId: 'agent_run_1',
    expectedGoalId: 'goal_1',
    externalEventId: 'callback-token-ref-secret',
    callerType: 'runtime',
    eventType: 'RunAccepted',
    status: 'active',
    tokenDelta: 0,
    timeDeltaSeconds: 0,
    payloadJson: JSON.stringify({
      authGrantRef: 'auth.json',
      callbackTokenRef: 'callback-token-ref-secret',
      hiddenSteering: 'private goal steering',
    }),
    createdAt: now,
    ...overrides,
  })

const source = (path: string): string =>
  readFileSync(new URL(path, import.meta.url), 'utf8')

describe('agent goal hardening guardrails', () => {
  test('keeps SQL out of goal route handlers', () => {
    const routeSource = source('./agent-goal-routes.ts')

    expect(routeSource).not.toContain('.prepare(')
    expect(routeSource).not.toMatch(/\b(SELECT|INSERT|UPDATE|DELETE)\b/)
    expect(routeSource).toContain('AgentGoalRepository')
    expect(routeSource).toContain('AgentGoalRuntimeService')
  })

  test('keeps public sync projections free of private goal and event fields', async () => {
    const goal = goalRecord()
    const privateGoal = agentGoalSyncValue(goal)
    const publicGoal = publicAgentGoalSyncValue(goal)
    const publicEvent = await publicAgentGoalEventSyncValue(eventRecord())
    const publicJson = JSON.stringify({ publicEvent, publicGoal })

    expect(privateGoal).toMatchObject({
      projectId: 'project_artanis',
      teamId: 'team_openagents_core',
      userId: 'github:14167547',
    })
    expect(Object.keys(publicGoal)).not.toContain('projectId')
    expect(Object.keys(publicGoal)).not.toContain('teamId')
    expect(Object.keys(publicGoal)).not.toContain('userId')
    expect(Object.keys(publicEvent)).not.toContain('payloadJson')
    expect(Object.keys(publicEvent)).not.toContain('externalEventId')
    expect(publicJson).not.toContain('authGrantRef')
    expect(publicJson).not.toContain('auth.json')
    expect(publicJson).not.toContain('callback-token-ref-secret')
    expect(publicJson).not.toContain('hiddenSteering')
  })

  test('keeps agent-facing update_goal terminal-only', async () => {
    const decoded = await Effect.runPromise(
      S.decodeUnknownEffect(AgentUpdateGoalRequest)({
        status: 'complete',
      }),
    )
    const rejectedPause = await Effect.runPromise(
      Effect.flip(
        S.decodeUnknownEffect(AgentUpdateGoalRequest)({
          status: 'paused',
        }),
      ),
    )
    const rejectedBudgetLimit = await Effect.runPromise(
      Effect.flip(
        S.decodeUnknownEffect(AgentUpdateGoalRequest)({
          status: 'budget_limited',
        }),
      ),
    )

    expect(decoded.status).toBe('complete')
    expect(String(rejectedPause)).toContain('complete')
    expect(String(rejectedPause)).toContain('blocked')
    expect(String(rejectedBudgetLimit)).toContain('complete')
    expect(String(rejectedBudgetLimit)).toContain('blocked')
  })

  test('keeps browser code away from SHC and OpenCode control endpoints', () => {
    const browserSources = [
      '../../../apps/web/src/page/loggedIn/goals/commands.ts',
      '../../../apps/web/src/page/loggedIn/goals/transitions.ts',
      '../../../apps/web/src/page/loggedIn/sync/projection.ts',
      '../../../apps/web/src/page/loggedOut/page/publicAgent.ts',
    ].map(source)
    const joined = browserSources.join('\n')

    expect(joined).not.toContain('SHC_CONTROL_API_URL')
    expect(joined).not.toContain('/opencode')
    expect(joined).not.toContain('authGrantRef')
    expect(joined).not.toContain('callbackTokenRef')
  })
})
