import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentGoalContinuationQueueShape,
  type AgentGoalEventInput,
  AgentGoalEventRecord,
  type AgentGoalEventRepositoryShape,
  AgentGoalRuntimeEvent,
  makeAgentGoalAccountingService,
  makeAgentGoalCapacityPolicyService,
  makeAgentGoalContinuationService,
} from './agent-goal-runtime'
import {
  AgentGoalNotFound,
  AgentGoalRecord,
  type AgentGoalRepositoryShape,
  type AgentGoalScope,
  AgentGoalStaleUpdate,
  type AgentGoalStatus,
  AgentGoalValidationError,
} from './agent-goals'

const now = '2026-06-04T16:00:00.000Z'

const goalRecord = (
  overrides: Partial<AgentGoalRecord> = {},
): AgentGoalRecord =>
  new AgentGoalRecord({
    id: 'goal_1',
    agentId: 'agent_1',
    userId: 'github:1',
    teamId: null,
    projectId: null,
    objective: 'Keep working until verified.',
    status: 'active',
    visibility: 'private',
    currentRunId: null,
    tokenBudget: null,
    tokensUsed: 0,
    timeUsedSeconds: 0,
    createdAt: now,
    updatedAt: now,
    completedAt: null,
    pausedAt: null,
    blockedAt: null,
    archivedAt: null,
    ...overrides,
  })

const sameScope = (goal: AgentGoalRecord, scope: AgentGoalScope): boolean =>
  goal.agentId === scope.agentId &&
  goal.userId === (scope.userId ?? null) &&
  goal.teamId === (scope.teamId ?? null) &&
  goal.projectId === (scope.projectId ?? null)

const statusAfterUsage = (
  status: AgentGoalStatus,
  tokenBudget: number | null,
  tokensUsed: number,
): AgentGoalStatus =>
  status === 'active' && tokenBudget !== null && tokensUsed >= tokenBudget
    ? 'budget_limited'
    : status

const updateGoal = (
  goals: Map<string, AgentGoalRecord>,
  goalId: string,
  expectedGoalId: string | undefined,
  update: (goal: AgentGoalRecord) => AgentGoalRecord,
) =>
  Effect.gen(function* () {
    const goal = goals.get(goalId)

    if (goal === undefined || goal.archivedAt !== null) {
      return yield* new AgentGoalNotFound({ goalId })
    }

    if (expectedGoalId !== undefined && expectedGoalId !== goal.id) {
      return yield* new AgentGoalStaleUpdate({
        actualGoalId: goal.id,
        expectedGoalId,
      })
    }

    const updated = update(goal)
    goals.set(updated.id, updated)

    return updated
  })

const makeMemoryGoalRepository = (
  seed: ReadonlyArray<AgentGoalRecord>,
): AgentGoalRepositoryShape => {
  const goals = new Map(seed.map(goal => [goal.id, goal]))

  return {
    accountUsage: input =>
      updateGoal(goals, input.goalId, input.expectedGoalId, goal => {
        const tokensUsed =
          goal.tokensUsed + Math.max(0, Math.trunc(input.tokenDelta ?? 0))

        return new AgentGoalRecord({
          ...goal,
          status: statusAfterUsage(goal.status, goal.tokenBudget, tokensUsed),
          timeUsedSeconds:
            goal.timeUsedSeconds +
            Math.max(0, Math.trunc(input.timeDeltaSeconds ?? 0)),
          tokensUsed,
          updatedAt: now,
        })
      }),
    archiveGoal: (goalId, expectedGoalId) =>
      updateGoal(
        goals,
        goalId,
        expectedGoalId,
        goal =>
          new AgentGoalRecord({ ...goal, archivedAt: now, updatedAt: now }),
      ),
    attachRun: input =>
      updateGoal(
        goals,
        input.goalId,
        input.expectedGoalId,
        goal =>
          new AgentGoalRecord({
            ...goal,
            currentRunId: input.runId,
            updatedAt: now,
          }),
      ),
    changeVisibility: (goalId, visibility, expectedGoalId) =>
      updateGoal(
        goals,
        goalId,
        expectedGoalId,
        goal => new AgentGoalRecord({ ...goal, updatedAt: now, visibility }),
      ),
    clearTokenBudget: (goalId, expectedGoalId) =>
      updateGoal(
        goals,
        goalId,
        expectedGoalId,
        goal =>
          new AgentGoalRecord({ ...goal, tokenBudget: null, updatedAt: now }),
      ),
    editObjective: (goalId, objective, expectedGoalId) =>
      objective.trim() === ''
        ? Effect.fail(
            new AgentGoalValidationError({
              field: 'objective',
              message: 'Goal objective is required.',
            }),
          )
        : updateGoal(
            goals,
            goalId,
            expectedGoalId,
            goal => new AgentGoalRecord({ ...goal, objective, updatedAt: now }),
          ),
    getById: goalId =>
      Effect.gen(function* () {
        const goal = goals.get(goalId)

        if (goal === undefined || goal.archivedAt !== null) {
          return yield* new AgentGoalNotFound({ goalId })
        }

        return goal
      }),
    getCurrent: scope =>
      Effect.succeed(
        Array.from(goals.values()).find(
          goal => goal.archivedAt === null && sameScope(goal, scope),
        ),
      ),
    getPublicCurrentByAgentId: agentId =>
      Effect.succeed(
        Array.from(goals.values()).find(
          goal =>
            goal.archivedAt === null &&
            goal.agentId === agentId &&
            goal.visibility === 'public',
        ),
      ),
    setGoal: input =>
      Effect.gen(function* () {
        if (input.objective.trim() === '') {
          return yield* new AgentGoalValidationError({
            field: 'objective',
            message: 'Goal objective is required.',
          })
        }

        const goal = goalRecord({
          agentId: input.agentId,
          id: input.id ?? `goal_${goals.size + 1}`,
          objective: input.objective.trim(),
          projectId: input.projectId ?? null,
          teamId: input.teamId ?? null,
          tokenBudget: input.tokenBudget ?? null,
          userId: input.userId ?? null,
          visibility: input.visibility ?? 'private',
        })
        goals.set(goal.id, goal)

        return goal
      }),
    setStatus: (goalId, status, expectedGoalId) =>
      updateGoal(
        goals,
        goalId,
        expectedGoalId,
        goal =>
          new AgentGoalRecord({
            ...goal,
            blockedAt: status === 'blocked' ? now : null,
            completedAt: status === 'complete' ? now : null,
            pausedAt: status === 'paused' ? now : null,
            status,
            updatedAt: now,
          }),
      ),
    setTokenBudget: (goalId, tokenBudget, expectedGoalId) =>
      updateGoal(
        goals,
        goalId,
        expectedGoalId,
        goal => new AgentGoalRecord({ ...goal, tokenBudget, updatedAt: now }),
      ),
  }
}

const makeMemoryGoalEventRepository = (
  events: Array<AgentGoalEventRecord>,
): AgentGoalEventRepositoryShape => {
  const record = (input: AgentGoalEventInput) =>
    Effect.succeed(
      new AgentGoalEventRecord({
        id: `agent_goal_event_${events.length + 1}`,
        goalId: input.goalId,
        runId: input.runId ?? null,
        expectedGoalId: input.expectedGoalId ?? null,
        externalEventId: input.externalEventId ?? null,
        callerType: input.callerType,
        eventType: input.eventType,
        status: input.status ?? null,
        tokenDelta: Math.max(0, Math.trunc(input.tokenDelta ?? 0)),
        timeDeltaSeconds: Math.max(0, Math.trunc(input.timeDeltaSeconds ?? 0)),
        payloadJson:
          input.payload === undefined ? null : JSON.stringify(input.payload),
        createdAt: now,
      }),
    ).pipe(
      Effect.tap(event =>
        Effect.sync(() => {
          events.push(event)
        }),
      ),
    )

  return {
    countByGoalAndType: (goalId, eventType) =>
      Effect.succeed(
        events.filter(
          event => event.goalId === goalId && event.eventType === eventType,
        ).length,
      ),
    hasExternalEvent: (goalId, externalEventId) =>
      Effect.succeed(
        events.some(
          event =>
            event.goalId === goalId &&
            event.externalEventId === externalEventId,
        ),
      ),
    listByGoal: (goalId, limit = 100) =>
      Effect.succeed(
        events
          .filter(event => event.goalId === goalId)
          .slice(0, Math.max(1, limit)),
      ),
    record,
    recordOnce: input =>
      input.externalEventId !== undefined &&
      events.some(
        event =>
          event.goalId === input.goalId &&
          event.externalEventId === input.externalEventId,
      )
        ? Effect.as(Effect.void, undefined)
        : record(input),
  }
}

describe('AgentGoal runtime policy', () => {
  test('accounts a runtime usage event exactly once by external event id', async () => {
    const events: Array<AgentGoalEventRecord> = []
    const repository = makeMemoryGoalRepository([goalRecord()])
    const accounting = makeAgentGoalAccountingService({
      events: makeMemoryGoalEventRepository(events),
      repository,
    })
    const event = new AgentGoalRuntimeEvent({
      type: 'UsageAccounted',
      goalId: 'goal_1',
      expectedGoalId: 'goal_1',
      externalEventId: 'usage:1',
      runId: 'run_1',
      tokenDelta: 30,
      timeDeltaSeconds: 4,
    })

    const first = await Effect.runPromise(accounting.applyRuntimeEvent(event))
    const second = await Effect.runPromise(accounting.applyRuntimeEvent(event))
    const goal = await Effect.runPromise(repository.getById('goal_1'))

    expect(first.duplicate).toBe(false)
    expect(second.duplicate).toBe(true)
    expect(goal.tokensUsed).toBe(30)
    expect(goal.timeUsedSeconds).toBe(4)
    expect(events).toHaveLength(1)
  })

  test('rejects stale expected goal ids before accounting', async () => {
    const repository = makeMemoryGoalRepository([goalRecord()])
    const accounting = makeAgentGoalAccountingService({
      events: makeMemoryGoalEventRepository([]),
      repository,
    })

    await expect(
      Effect.runPromise(
        accounting.applyRuntimeEvent(
          new AgentGoalRuntimeEvent({
            type: 'UsageAccounted',
            goalId: 'goal_1',
            expectedGoalId: 'goal_replaced',
            externalEventId: 'usage:stale',
            tokenDelta: 10,
          }),
        ),
      ),
    ).rejects.toMatchObject({ _tag: 'AgentGoalStaleUpdate' })
  })

  test('transitions exhausted budgets and usage failures to runtime-owned statuses', async () => {
    const repository = makeMemoryGoalRepository([
      goalRecord({ tokenBudget: 10 }),
      goalRecord({ id: 'goal_2' }),
    ])
    const accounting = makeAgentGoalAccountingService({
      events: makeMemoryGoalEventRepository([]),
      repository,
    })

    await Effect.runPromise(
      accounting.applyRuntimeEvent(
        new AgentGoalRuntimeEvent({
          type: 'UsageAccounted',
          goalId: 'goal_1',
          expectedGoalId: 'goal_1',
          externalEventId: 'usage:budget',
          tokenDelta: 15,
        }),
      ),
    )
    await Effect.runPromise(
      accounting.applyRuntimeEvent(
        new AgentGoalRuntimeEvent({
          type: 'UsageLimitReached',
          goalId: 'goal_2',
          expectedGoalId: 'goal_2',
          externalEventId: 'usage:limited',
        }),
      ),
    )

    await expect(
      Effect.runPromise(repository.getById('goal_1')),
    ).resolves.toMatchObject({ status: 'budget_limited', tokensUsed: 15 })
    await expect(
      Effect.runPromise(repository.getById('goal_2')),
    ).resolves.toMatchObject({ status: 'usage_limited' })
  })

  test('enqueues active eligible continuations and records the resumed run', async () => {
    const events: Array<AgentGoalEventRecord> = []
    const repository = makeMemoryGoalRepository([goalRecord()])
    const queue: AgentGoalContinuationQueueShape = {
      enqueue: input => Effect.succeed({ runId: `run_${input.attempt}` }),
    }
    const continuation = makeAgentGoalContinuationService({
      capacity: makeAgentGoalCapacityPolicyService({
        defaultTokenBudget: 100,
        maxContinuationAttempts: 2,
      }),
      events: makeMemoryGoalEventRepository(events),
      queue,
      repository,
    })

    const decision = await Effect.runPromise(
      continuation.requestContinuation({
        durableSnapshotWritten: true,
        goalId: 'goal_1',
        providerHealthy: true,
      }),
    )
    const goal = await Effect.runPromise(repository.getById('goal_1'))

    expect(decision).toMatchObject({
      action: 'enqueue',
      reason: 'eligible',
      runId: 'run_1',
    })
    expect(goal.currentRunId).toBe('run_1')
    expect(events).toMatchObject([
      {
        eventType: 'WorkerResumed',
        externalEventId: 'goal:goal_1:continuation:1',
        runId: 'run_1',
      },
    ])
  })

  test('does not continue paused, blocked, pending, or default-budget-limited goals', async () => {
    const repository = makeMemoryGoalRepository([
      goalRecord({ id: 'goal_paused', status: 'paused' }),
      goalRecord({ id: 'goal_blocked', status: 'blocked' }),
      goalRecord({ id: 'goal_budget', tokensUsed: 5 }),
      goalRecord({ id: 'goal_pending' }),
    ])
    const queue: AgentGoalContinuationQueueShape = {
      enqueue: () => Effect.succeed({ runId: 'should_not_run' }),
    }
    const continuation = makeAgentGoalContinuationService({
      capacity: makeAgentGoalCapacityPolicyService({
        defaultTokenBudget: 5,
        maxContinuationAttempts: 2,
      }),
      events: makeMemoryGoalEventRepository([]),
      queue,
      repository,
    })

    await expect(
      Effect.runPromise(
        continuation.requestContinuation({
          durableSnapshotWritten: true,
          goalId: 'goal_paused',
        }),
      ),
    ).resolves.toMatchObject({ action: 'skip', reason: 'status_paused' })
    await expect(
      Effect.runPromise(
        continuation.requestContinuation({
          durableSnapshotWritten: true,
          goalId: 'goal_blocked',
        }),
      ),
    ).resolves.toMatchObject({ action: 'skip', reason: 'status_blocked' })
    await expect(
      Effect.runPromise(
        continuation.requestContinuation({
          durableSnapshotWritten: true,
          goalId: 'goal_pending',
          pendingApproval: true,
        }),
      ),
    ).resolves.toMatchObject({ action: 'skip', reason: 'pending_approval' })
    await expect(
      Effect.runPromise(
        continuation.requestContinuation({
          durableSnapshotWritten: true,
          goalId: 'goal_budget',
        }),
      ),
    ).resolves.toMatchObject({ action: 'skip', reason: 'budget_limited' })
    await expect(
      Effect.runPromise(repository.getById('goal_budget')),
    ).resolves.toMatchObject({ status: 'budget_limited' })
  })
})
