import { Effect, Layer } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeAgentGoalRoutes } from './agent-goal-routes'
import {
  type AgentGoalEventInput,
  AgentGoalEventRecord,
  AgentGoalEventRepository,
  type AgentGoalEventRepositoryShape,
} from './agent-goal-runtime'
import {
  AgentGoalNotFound,
  AgentGoalRecord,
  AgentGoalRepository,
  type AgentGoalRepositoryShape,
  type AgentGoalScope,
  AgentGoalStaleUpdate,
  type AgentGoalStatus,
  AgentGoalValidationError,
} from './agent-goals'
import type { TeamRole, UserTeamProject } from './team-repository'

type TestSession = Readonly<{ user: Readonly<{ userId: string }> }>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

const testEnv = {
  OPENAGENTS_DB: {} as D1Database,
}

type MemoryState = Readonly<{
  goals: Map<string, AgentGoalRecord>
  counter: { value: number }
}>

const now = '2026-06-04T13:00:00.000Z'

const sameScope = (goal: AgentGoalRecord, scope: AgentGoalScope): boolean =>
  goal.agentId === scope.agentId &&
  goal.userId === (scope.userId ?? null) &&
  goal.teamId === (scope.teamId ?? null) &&
  goal.projectId === (scope.projectId ?? null)

const remainingStatus = (
  status: AgentGoalStatus,
  tokenBudget: number | null,
  tokensUsed: number,
): AgentGoalStatus =>
  status === 'active' && tokenBudget !== null && tokensUsed >= tokenBudget
    ? 'budget_limited'
    : status

const updateGoal = (
  state: MemoryState,
  goalId: string,
  expectedGoalId: string | undefined,
  update: (goal: AgentGoalRecord) => AgentGoalRecord,
) =>
  Effect.gen(function* () {
    const goal = state.goals.get(goalId)

    if (goal === undefined || goal.archivedAt !== null) {
      return yield* new AgentGoalNotFound({ goalId })
    }

    if (expectedGoalId !== undefined && expectedGoalId !== goal.id) {
      return yield* new AgentGoalStaleUpdate({
        expectedGoalId,
        actualGoalId: goal.id,
      })
    }

    const updated = update(goal)
    state.goals.set(updated.id, updated)

    return updated
  })

const validateObjective = (
  objective: string,
): Effect.Effect<string, AgentGoalValidationError> => {
  const normalized = objective.replace(/\s+/g, ' ').trim()

  return normalized === ''
    ? Effect.fail(
        new AgentGoalValidationError({
          field: 'objective',
          message: 'Goal objective is required.',
        }),
      )
    : Effect.succeed(normalized)
}

const validateBudget = (
  tokenBudget: number | null | undefined,
): Effect.Effect<number | null, AgentGoalValidationError> =>
  tokenBudget === null || tokenBudget === undefined
    ? Effect.succeed(null)
    : Number.isInteger(tokenBudget) && tokenBudget > 0
      ? Effect.succeed(tokenBudget)
      : Effect.fail(
          new AgentGoalValidationError({
            field: 'tokenBudget',
            message: 'Token budget must be a positive integer.',
          }),
        )

const makeMemoryGoalRepository = (
  state: MemoryState = {
    goals: new Map(),
    counter: { value: 1 },
  },
): AgentGoalRepositoryShape => ({
  accountUsage: input =>
    updateGoal(state, input.goalId, input.expectedGoalId, goal => {
      const tokensUsed = goal.tokensUsed + Math.max(0, input.tokenDelta ?? 0)
      const tokenBudget = goal.tokenBudget

      return new AgentGoalRecord({
        ...goal,
        tokensUsed,
        timeUsedSeconds:
          goal.timeUsedSeconds + Math.max(0, input.timeDeltaSeconds ?? 0),
        status: remainingStatus(goal.status, tokenBudget, tokensUsed),
        updatedAt: now,
      })
    }),
  archiveGoal: (goalId, expectedGoalId) =>
    updateGoal(
      state,
      goalId,
      expectedGoalId,
      goal =>
        new AgentGoalRecord({
          ...goal,
          archivedAt: now,
          updatedAt: now,
        }),
    ),
  attachRun: input =>
    updateGoal(
      state,
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
      state,
      goalId,
      expectedGoalId,
      goal =>
        new AgentGoalRecord({
          ...goal,
          visibility,
          updatedAt: now,
        }),
    ),
  clearTokenBudget: (goalId, expectedGoalId) =>
    updateGoal(
      state,
      goalId,
      expectedGoalId,
      goal =>
        new AgentGoalRecord({
          ...goal,
          status: goal.status === 'budget_limited' ? 'active' : goal.status,
          tokenBudget: null,
          updatedAt: now,
        }),
    ),
  editObjective: (goalId, objective, expectedGoalId) =>
    validateObjective(objective).pipe(
      Effect.flatMap(normalized =>
        updateGoal(
          state,
          goalId,
          expectedGoalId,
          goal =>
            new AgentGoalRecord({
              ...goal,
              objective: normalized,
              status:
                goal.status === 'complete' || goal.status === 'budget_limited'
                  ? 'active'
                  : goal.status,
              completedAt: goal.status === 'complete' ? null : goal.completedAt,
              updatedAt: now,
            }),
        ),
      ),
    ),
  getById: goalId =>
    Effect.gen(function* () {
      const goal = state.goals.get(goalId)

      if (goal === undefined || goal.archivedAt !== null) {
        return yield* new AgentGoalNotFound({ goalId })
      }

      return goal
    }),
  getCurrent: scope =>
    Effect.succeed(
      Array.from(state.goals.values()).find(
        goal => goal.archivedAt === null && sameScope(goal, scope),
      ),
    ),
  getPublicCurrentByAgentId: agentId =>
    Effect.succeed(
      Array.from(state.goals.values()).find(
        goal =>
          goal.archivedAt === null &&
          goal.agentId === agentId &&
          goal.visibility === 'public',
      ),
    ),
  setGoal: input =>
    Effect.gen(function* () {
      const objective = yield* validateObjective(input.objective)
      const tokenBudget = yield* validateBudget(input.tokenBudget)
      const id = input.id ?? `goal_${state.counter.value}`
      state.counter.value += 1
      const scope = {
        agentId: input.agentId,
        userId: input.userId ?? null,
        teamId: input.teamId ?? null,
        projectId: input.projectId ?? null,
      } satisfies AgentGoalScope
      Array.from(state.goals.values())
        .filter(goal => goal.archivedAt === null && sameScope(goal, scope))
        .map(goal =>
          state.goals.set(
            goal.id,
            new AgentGoalRecord({
              ...goal,
              archivedAt: now,
              updatedAt: now,
            }),
          ),
        )
      const goal = new AgentGoalRecord({
        id,
        agentId: input.agentId,
        userId: input.userId ?? null,
        teamId: input.teamId ?? null,
        projectId: input.projectId ?? null,
        objective,
        status: 'active',
        visibility: input.visibility ?? 'private',
        currentRunId: null,
        tokenBudget,
        tokensUsed: 0,
        timeUsedSeconds: 0,
        createdAt: now,
        updatedAt: now,
        completedAt: null,
        pausedAt: null,
        blockedAt: null,
        archivedAt: null,
      })
      state.goals.set(goal.id, goal)

      return goal
    }),
  setStatus: (goalId, status, expectedGoalId) =>
    updateGoal(
      state,
      goalId,
      expectedGoalId,
      goal =>
        new AgentGoalRecord({
          ...goal,
          status,
          completedAt: status === 'complete' ? now : null,
          pausedAt: status === 'paused' ? now : null,
          blockedAt: status === 'blocked' ? now : null,
          updatedAt: now,
        }),
    ),
  setTokenBudget: (goalId, tokenBudget, expectedGoalId) =>
    validateBudget(tokenBudget).pipe(
      Effect.flatMap(budget =>
        updateGoal(
          state,
          goalId,
          expectedGoalId,
          goal =>
            new AgentGoalRecord({
              ...goal,
              tokenBudget: budget,
              status: remainingStatus(goal.status, budget, goal.tokensUsed),
              updatedAt: now,
            }),
        ),
      ),
    ),
})

const makeMemoryGoalEventRepository = (
  events: Array<AgentGoalEventRecord> = [],
): AgentGoalEventRepositoryShape => ({
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
          event.goalId === goalId && event.externalEventId === externalEventId,
      ),
    ),
  listByGoal: (goalId, limit = 100) =>
    Effect.succeed(
      events
        .filter(event => event.goalId === goalId)
        .slice(0, Math.max(1, limit)),
    ),
  record: (input: AgentGoalEventInput) =>
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
    ),
  recordOnce: (input: AgentGoalEventInput) => {
    if (
      input.externalEventId !== undefined &&
      events.some(
        event =>
          event.goalId === input.goalId &&
          event.externalEventId === input.externalEventId,
      )
    ) {
      return Effect.as(Effect.void, undefined)
    }

    return makeMemoryGoalEventRepository(events).record(input)
  },
})

const project = (teamId: string, projectId: string): UserTeamProject => ({
  id: projectId,
  teamId,
  name: 'Artanis',
  slug: 'artanis',
  description: 'Artanis project',
  status: 'active',
})

const bearer = (token: string): string => `Bearer ${token}`

const jsonRequest = (url: string, method: string, body?: unknown): Request =>
  new Request(
    url,
    body === undefined
      ? { method }
      : {
          method,
          headers: {
            'content-type': 'application/json',
          },
          body: JSON.stringify(body),
        },
  )

const makeRoutes = (
  input: Readonly<{
    repository?: AgentGoalRepositoryShape
    events?: Array<AgentGoalEventRecord>
    session?: TestSession | null
    teamRoles?: Readonly<Record<string, TeamRole | undefined>>
    projectExists?: boolean
    agentAuthenticated?: boolean
  }> = {},
) => {
  const repository = input.repository ?? makeMemoryGoalRepository()
  const events = input.events ?? []

  return makeAgentGoalRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    authenticateRequestActor: request =>
      request.headers.get('authorization') === bearer('agent-token') ||
      input.agentAuthenticated === true
        ? Promise.resolve({
            kind: 'agent' as const,
            agent: {
              user: {
                id: 'agent_user_1',
              },
            },
          })
        : Promise.resolve(undefined),
    makeEventRepositoryLayer: () =>
      Layer.succeed(
        AgentGoalEventRepository,
        makeMemoryGoalEventRepository(events),
      ),
    makeRepositoryLayer: () => Layer.succeed(AgentGoalRepository, repository),
    readActiveTeamMembershipRole: async (_db, teamId) =>
      input.teamRoles?.[teamId],
    readActiveTeamProject: async (_db, teamId, projectId) =>
      input.projectExists === false ? undefined : project(teamId, projectId),
    requireAdminApiToken: request =>
      Promise.resolve(request.headers.get('authorization') === bearer('admin')),
    requireBrowserSession: () =>
      Promise.resolve(
        input.session === null
          ? undefined
          : (input.session ?? {
              user: { userId: 'github:1' },
            }),
      ),
  })
}

const runRoute = async (
  routes: ReturnType<typeof makeRoutes>,
  request: Request,
): Promise<Response> => {
  const routed = routes.routeAgentGoalRequest(
    request,
    testEnv,
    executionContext(),
  )

  if (routed === undefined) {
    throw new Error('goal route did not match')
  }

  return Effect.runPromise(routed)
}

describe('agent goal API routes', () => {
  test('creates, reads, patches, pauses, resumes, and clears browser goals', async () => {
    const repository = makeMemoryGoalRepository()
    const routes = makeRoutes({ repository })
    const created = await runRoute(
      routes,
      jsonRequest('https://openagents.com/api/autopilot/goals', 'POST', {
        objective: 'Ship goal UI',
        tokenBudget: 100,
      }),
    )
    const createdJson = (await created.json()) as { goal: { id: string } }
    const patched = await runRoute(
      routes,
      jsonRequest(
        `https://openagents.com/api/autopilot/goals/${createdJson.goal.id}`,
        'PATCH',
        {
          objective: 'Ship better goal UI',
          tokenBudget: 150,
        },
      ),
    )
    const paused = await runRoute(
      routes,
      jsonRequest(
        `https://openagents.com/api/autopilot/goals/${createdJson.goal.id}/pause`,
        'POST',
      ),
    )
    const resumed = await runRoute(
      routes,
      jsonRequest(
        `https://openagents.com/api/autopilot/goals/${createdJson.goal.id}/resume`,
        'POST',
      ),
    )
    const current = await runRoute(
      routes,
      new Request('https://openagents.com/api/autopilot/goals/current'),
    )
    const cleared = await runRoute(
      routes,
      jsonRequest(
        `https://openagents.com/api/autopilot/goals/${createdJson.goal.id}/clear`,
        'POST',
      ),
    )

    expect(created.status).toBe(200)
    await expect(patched.json()).resolves.toMatchObject({
      goal: {
        objective: 'Ship better goal UI',
        remainingTokens: 150,
      },
    })
    await expect(paused.json()).resolves.toMatchObject({
      goal: { status: 'paused' },
    })
    await expect(resumed.json()).resolves.toMatchObject({
      goal: { status: 'active' },
    })
    await expect(current.json()).resolves.toMatchObject({
      goal: { id: createdJson.goal.id },
    })
    await expect(cleared.json()).resolves.toMatchObject({
      goal: { id: createdJson.goal.id },
    })
  })

  test('requires team membership and active project for team scoped goals', async () => {
    const denied = await runRoute(
      makeRoutes({ teamRoles: {} }),
      jsonRequest('https://openagents.com/api/autopilot/goals', 'POST', {
        objective: 'Team goal',
        teamId: 'team_openagents_core',
      }),
    )
    const accepted = await runRoute(
      makeRoutes({
        teamRoles: { team_openagents_core: 'member' },
        projectExists: true,
      }),
      jsonRequest('https://openagents.com/api/autopilot/goals', 'POST', {
        objective: 'Artanis project goal',
        agentId: 'agent_artanis',
        teamId: 'team_openagents_core',
        projectId: 'project_artanis',
      }),
    )

    expect(denied.status).toBe(403)
    expect(accepted.status).toBe(200)
    await expect(accepted.json()).resolves.toMatchObject({
      goal: {
        agentId: 'agent_artanis',
        projectId: 'project_artanis',
        teamId: 'team_openagents_core',
      },
    })
  })

  test('prevents another browser user from reading a private goal', async () => {
    const repository = makeMemoryGoalRepository()
    const ownerRoutes = makeRoutes({ repository })
    const created = await runRoute(
      ownerRoutes,
      jsonRequest('https://openagents.com/api/autopilot/goals', 'POST', {
        objective: 'Private owner goal',
      }),
    )
    const createdJson = (await created.json()) as { goal: { id: string } }
    const otherRoutes = makeRoutes({
      repository,
      session: { user: { userId: 'github:2' } },
    })
    const response = await runRoute(
      otherRoutes,
      new Request(
        `https://openagents.com/api/autopilot/goals/${createdJson.goal.id}`,
      ),
    )

    expect(response.status).toBe(403)
  })

  test('supports operator create and read with admin bearer auth', async () => {
    const repository = makeMemoryGoalRepository()
    const routes = makeRoutes({ repository })
    const request = jsonRequest(
      'https://openagents.com/api/operator/autopilot/goals',
      'POST',
      {
        userId: 'github:target',
        objective: 'Operator smoke goal',
        visibility: 'private',
      },
    )
    request.headers.set('authorization', bearer('admin'))
    const created = await runRoute(routes, request)
    const createdJson = (await created.json()) as { goal: { id: string } }
    const readRequest = new Request(
      `https://openagents.com/api/operator/autopilot/goals/${createdJson.goal.id}`,
      {
        headers: { authorization: bearer('admin') },
      },
    )
    const read = await runRoute(routes, readRequest)
    const pauseRequest = jsonRequest(
      `https://openagents.com/api/operator/autopilot/goals/${createdJson.goal.id}/pause`,
      'POST',
    )
    pauseRequest.headers.set('authorization', bearer('admin'))
    const paused = await runRoute(routes, pauseRequest)

    expect(created.status).toBe(200)
    await expect(read.json()).resolves.toMatchObject({
      goal: {
        objective: 'Operator smoke goal',
        userId: 'github:target',
      },
    })
    await expect(paused.json()).resolves.toMatchObject({
      goal: { status: 'paused' },
    })
  })

  test('supports programmatic agent create and terminal complete', async () => {
    const repository = makeMemoryGoalRepository()
    const events: Array<AgentGoalEventRecord> = []
    const routes = makeRoutes({ events, repository })
    const createRequest = jsonRequest(
      'https://openagents.com/api/agents/goals',
      'POST',
      {
        explicitRequest: true,
        objective: 'Finish model-owned goal',
      },
    )
    createRequest.headers.set('authorization', bearer('agent-token'))
    const created = await runRoute(routes, createRequest)
    const createdJson = (await created.json()) as { goal: { id: string } }
    const completeRequest = jsonRequest(
      `https://openagents.com/api/agents/goals/${createdJson.goal.id}/update`,
      'POST',
      {
        runId: 'agent_run_1',
        status: 'complete',
        timeDeltaSeconds: 30,
        tokenDelta: 25,
      },
    )
    completeRequest.headers.set('authorization', bearer('agent-token'))
    const completed = await runRoute(routes, completeRequest)
    const readRequest = new Request(
      `https://openagents.com/api/agents/goals/${createdJson.goal.id}`,
      {
        headers: { authorization: bearer('agent-token') },
      },
    )
    const read = await runRoute(routes, readRequest)

    expect(read.status).toBe(200)
    await expect(completed.json()).resolves.toMatchObject({
      completionBudgetReport: {
        remainingTokens: null,
        status: 'complete',
        timeUsedSeconds: 30,
        tokenBudget: null,
        tokensUsed: 25,
      },
      goal: {
        status: 'complete',
        timeUsedSeconds: 30,
        tokensUsed: 25,
        userId: 'agent_user_1',
      },
    })
    expect(events).toMatchObject([
      {
        callerType: 'agent_tool',
        eventType: 'tool.create_goal',
        goalId: createdJson.goal.id,
      },
      {
        callerType: 'agent_tool',
        eventType: 'tool.update_goal',
        expectedGoalId: createdJson.goal.id,
        goalId: createdJson.goal.id,
        runId: 'agent_run_1',
        status: 'complete',
        timeDeltaSeconds: 30,
        tokenDelta: 25,
      },
    ])
  })

  test('rejects unsafe programmatic agent goal mutations', async () => {
    const repository = makeMemoryGoalRepository()
    const routes = makeRoutes({ repository })
    const createRequest = jsonRequest(
      'https://openagents.com/api/agents/goals',
      'POST',
      {
        explicitRequest: true,
        objective: 'Only active agent goal',
      },
    )
    createRequest.headers.set('authorization', bearer('agent-token'))
    const created = await runRoute(routes, createRequest)
    const duplicateRequest = jsonRequest(
      'https://openagents.com/api/agents/goals',
      'POST',
      {
        explicitRequest: true,
        objective: 'Replacement must fail',
      },
    )
    duplicateRequest.headers.set('authorization', bearer('agent-token'))
    const duplicate = await runRoute(routes, duplicateRequest)
    const implicitRequest = jsonRequest(
      'https://openagents.com/api/agents/goals',
      'POST',
      {
        explicitRequest: false,
        objective: 'Implicit goal must fail',
      },
    )
    implicitRequest.headers.set('authorization', bearer('agent-token'))
    const implicit = await runRoute(makeRoutes({ repository }), implicitRequest)
    const createdJson = (await created.json()) as { goal: { id: string } }
    const invalidStatusRequest = jsonRequest(
      `https://openagents.com/api/agents/goals/${createdJson.goal.id}/update`,
      'POST',
      {
        status: 'paused',
      },
    )
    invalidStatusRequest.headers.set('authorization', bearer('agent-token'))
    const invalidStatus = await runRoute(routes, invalidStatusRequest)

    expect(created.status).toBe(200)
    expect(duplicate.status).toBe(409)
    expect(implicit.status).toBe(400)
    expect(invalidStatus.status).toBe(400)
  })

  test('returns public projections without private scope fields', async () => {
    const repository = makeMemoryGoalRepository()
    const events: Array<AgentGoalEventRecord> = []
    const routes = makeRoutes({ events, repository })
    const created = await Effect.runPromise(
      repository.setGoal({
        agentId: 'agent_artanis',
        objective: 'Public Artanis goal',
        userId: 'github:1',
        teamId: 'team_openagents_core',
        projectId: 'project_artanis',
        visibility: 'public',
      }),
    )
    await Effect.runPromise(
      makeMemoryGoalEventRepository(events).record({
        callerType: 'runtime',
        eventType: 'RunAccepted',
        externalEventId: 'callback-token-ref-secret',
        goalId: created.id,
        payload: {
          artifactRefs: [
            'artifact_public-release-notes',
            'callback-token-ref-secret',
          ],
          authGrantRef: 'secret://openagents/provider-account/auth',
          commitRefs: [
            'ae7912549301df1a0df78353d47f64196ad6faf6',
            'provider-ref-secret',
          ],
          hiddenSteering: 'private dispatch prompt',
          rawPayload: 'auth.json',
          receiptRefs: ['sha256:1234567890abcdef', 'token-secret'],
        },
        runId: 'agent_run_public_1',
      }),
    )
    const response = await runRoute(
      routes,
      new Request(`https://openagents.com/api/public/goals/${created.id}`),
    )
    const byAgent = await runRoute(
      routes,
      new Request(
        'https://openagents.com/api/public/agents/agent_artanis/current-goal',
      ),
    )
    const snapshot = await runRoute(
      routes,
      new Request(
        `https://openagents.com/api/public/goals/${created.id}/snapshot`,
      ),
    )
    const json = (await response.json()) as {
      events: ReadonlyArray<Record<string, unknown>>
      goal: Record<string, unknown>
    }
    const serialized = JSON.stringify(json)

    expect(response.status).toBe(200)
    expect(byAgent.status).toBe(200)
    expect(snapshot.status).toBe(200)
    expect(json.goal.objective).toBe('Public Artanis goal')
    expect(json.goal.userId).toBeUndefined()
    expect(json.goal.teamId).toBeUndefined()
    expect(json.goal.projectId).toBeUndefined()
    expect(json.goal.visibility).toBeUndefined()
    expect(json.events[0]).toMatchObject({
      goalId: created.id,
      runId: 'agent_run_public_1',
      summary: 'Run accepted.',
      type: 'RunAccepted',
    })
    expect(json.events[0]?.artifactRefs).toEqual([
      'artifact_public-release-notes',
    ])
    expect(json.events[0]?.commitRefs).toEqual([
      'ae7912549301df1a0df78353d47f64196ad6faf6',
    ])
    expect(json.events[0]?.receiptRefs).toEqual(['sha256:1234567890abcdef'])
    expect(json.events[0]?.payloadJson).toBeUndefined()
    expect(json.events[0]?.externalEventId).toBeUndefined()
    expect(serialized).not.toContain('authGrantRef')
    expect(serialized).not.toContain('provider-ref-secret')
    expect(serialized).not.toContain('hiddenSteering')
    expect(serialized).not.toContain('auth.json')
    expect(serialized).not.toContain('callback-token-ref-secret')
    expect(serialized).not.toContain('token-secret')
  })

  test('validates browser request bodies', async () => {
    const invalidObjective = await runRoute(
      makeRoutes(),
      jsonRequest('https://openagents.com/api/autopilot/goals', 'POST', {
        objective: '   ',
      }),
    )
    const invalidBudget = await runRoute(
      makeRoutes(),
      jsonRequest('https://openagents.com/api/autopilot/goals', 'POST', {
        objective: 'Goal with invalid budget',
        tokenBudget: 0,
      }),
    )

    expect(invalidObjective.status).toBe(400)
    expect(invalidBudget.status).toBe(400)
  })
})
