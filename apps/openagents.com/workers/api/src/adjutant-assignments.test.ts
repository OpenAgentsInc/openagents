import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AdjutantAssignmentActiveExists,
  AdjutantAssignmentGoalNotFound,
  AdjutantAssignmentRunGoalRequired,
  AdjutantAssignmentUnsafePayload,
  makeAdjutantAssignmentService,
} from './adjutant-assignments'
import {
  AgentGoalNotFound,
  AgentGoalRecord,
  type AgentGoalRepositoryShape,
} from './agent-goals'

type StoredSoftwareOrder = Readonly<{
  archived_at: string | null
  id: string
}>

type StoredSiteProject = Readonly<{
  archived_at: string | null
  id: string
  software_order_id: string | null
}>

type StoredAdjutantAssignment = Readonly<{
  agent_id: string
  archived_at: string | null
  assigned_by_user_id: string | null
  assignment_kind:
    | 'site_generation'
    | 'site_adjustment'
    | 'site_review'
    | 'site_deployment'
    | 'general_order_fulfillment'
  blocked_at: string | null
  commit_sha: string | null
  completed_at: string | null
  created_at: string
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  project_id: string | null
  site_id: string | null
  software_order_id: string | null
  status:
    | 'draft'
    | 'preflight_pending'
    | 'blocked'
    | 'queued'
    | 'running'
    | 'review_needed'
    | 'deployed'
    | 'delivered'
    | 'complete'
    | 'canceled'
  task_spec_path: string | null
  team_id: string | null
  updated_at: string
  visibility: 'private' | 'team' | 'public'
}>

type StoredAdjutantAssignmentEvent = Readonly<{
  actor_user_id: string | null
  assignment_id: string
  created_at: string
  event_type: string
  goal_id: string | null
  id: string
  payload_json: string | null
  run_id: string | null
  site_id: string | null
  software_order_id: string | null
  summary: string
  visibility: 'private' | 'team' | 'public'
}>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

class AdjutantAssignmentTestRuntime {
  private assignmentCounter = 0
  private eventCounter = 0

  makeAssignmentId = (): string => {
    this.assignmentCounter += 1

    return `adjutant_assignment_${this.assignmentCounter}`
  }

  makeEventId = (): string => {
    this.eventCounter += 1

    return `adjutant_assignment_event_${this.eventCounter}`
  }

  nowIso = (): string => '2026-06-05T12:00:00.000Z'
}

class AdjutantAssignmentDbStore {
  assignments: Array<StoredAdjutantAssignment> = []
  events: Array<StoredAdjutantAssignmentEvent> = []
  goals: Array<AgentGoalRecord> = []
  runtime = new AdjutantAssignmentTestRuntime()
  softwareOrders: Array<StoredSoftwareOrder> = [
    { archived_at: null, id: 'software_order_otec' },
  ]
  sites: Array<StoredSiteProject> = [
    {
      archived_at: null,
      id: 'site_project_otec',
      software_order_id: 'software_order_otec',
    },
  ]
}

class AdjutantAssignmentStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: AdjutantAssignmentDbStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM software_orders')) {
      const [softwareOrderId] = this.values
      const row = this.store.softwareOrders.find(
        order => order.id === softwareOrderId && order.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_projects')) {
      const [siteId] = this.values
      const row = this.store.sites.find(
        site => site.id === siteId && site.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM adjutant_assignments') &&
      this.query.includes('WHERE id = ?')
    ) {
      const [assignmentId] = this.values
      const row = this.store.assignments.find(
        assignment =>
          assignment.id === assignmentId && assignment.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM adjutant_assignments') &&
      this.query.includes("status NOT IN ('complete', 'canceled')")
    ) {
      const [softwareOrderId, siteId] = this.values
      const row = this.store.assignments.find(
        assignment =>
          assignment.archived_at === null &&
          assignment.status !== 'complete' &&
          assignment.status !== 'canceled' &&
          ((softwareOrderId !== null &&
            assignment.software_order_id === softwareOrderId) ||
            (siteId !== null && assignment.site_id === siteId)),
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO adjutant_assignments')) {
      const [
        id,
        softwareOrderId,
        siteId,
        goalId,
        currentRunId,
        teamId,
        projectId,
        agentId,
        assignedByUserId,
        assignmentKind,
        status,
        visibility,
        taskSpecPath,
        commitSha,
        objective,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.assignments.push({
        agent_id: String(agentId),
        archived_at: null,
        assigned_by_user_id:
          typeof assignedByUserId === 'string' ? assignedByUserId : null,
        assignment_kind:
          assignmentKind as StoredAdjutantAssignment['assignment_kind'],
        blocked_at: null,
        commit_sha: typeof commitSha === 'string' ? commitSha : null,
        completed_at: null,
        created_at: String(createdAt),
        current_run_id: typeof currentRunId === 'string' ? currentRunId : null,
        goal_id: typeof goalId === 'string' ? goalId : null,
        id: String(id),
        objective: String(objective),
        project_id: typeof projectId === 'string' ? projectId : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        status: status as StoredAdjutantAssignment['status'],
        task_spec_path: typeof taskSpecPath === 'string' ? taskSpecPath : null,
        team_id: typeof teamId === 'string' ? teamId : null,
        updated_at: String(updatedAt),
        visibility: visibility as StoredAdjutantAssignment['visibility'],
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO adjutant_assignment_events')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        goalId,
        runId,
        eventType,
        visibility,
        summary,
        actorUserId,
        payloadJson,
        createdAt,
      ] = this.values

      this.store.events.push({
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        assignment_id: String(assignmentId),
        created_at: String(createdAt),
        event_type: String(eventType),
        goal_id: typeof goalId === 'string' ? goalId : null,
        id: String(id),
        payload_json: typeof payloadJson === 'string' ? payloadJson : null,
        run_id: typeof runId === 'string' ? runId : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        summary: String(summary),
        visibility: visibility as StoredAdjutantAssignmentEvent['visibility'],
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE adjutant_assignments')) {
      const [
        goalId,
        currentRunId,
        status,
        taskSpecPath,
        commitSha,
        objective,
        updatedAt,
        completedAt,
        blockedAt,
        assignmentId,
      ] = this.values
      const index = this.store.assignments.findIndex(
        assignment => assignment.id === assignmentId,
      )

      if (index !== -1) {
        const current = this.store.assignments[index]

        if (current !== undefined) {
          this.store.assignments[index] = {
            ...current,
            blocked_at: typeof blockedAt === 'string' ? blockedAt : null,
            commit_sha: typeof commitSha === 'string' ? commitSha : null,
            completed_at: typeof completedAt === 'string' ? completedAt : null,
            current_run_id:
              typeof currentRunId === 'string' ? currentRunId : null,
            goal_id: typeof goalId === 'string' ? goalId : null,
            objective: String(objective),
            status: status as StoredAdjutantAssignment['status'],
            task_spec_path:
              typeof taskSpecPath === 'string' ? taskSpecPath : null,
            updated_at: String(updatedAt),
          }
        }
      }

      return Promise.resolve(makeResult<T>())
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM adjutant_assignments')) {
      return Promise.resolve(
        makeResult<T>(
          this.store.assignments
            .filter(assignment => assignment.archived_at === null)
            .slice(0, Number(this.values[0] ?? 100)) as Array<T>,
        ),
      )
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

const adjutantAssignmentDb = (
  store: AdjutantAssignmentDbStore,
): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new AdjutantAssignmentStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const makeGoal = (
  store: AdjutantAssignmentDbStore,
  input: Readonly<{
    agentId: string
    objective: string
    projectId?: string | null | undefined
    teamId?: string | null | undefined
    userId?: string | null | undefined
    visibility?: 'private' | 'team' | 'public' | undefined
  }>,
): AgentGoalRecord =>
  new AgentGoalRecord({
    agentId: input.agentId,
    archivedAt: null,
    blockedAt: null,
    completedAt: null,
    createdAt: store.runtime.nowIso(),
    currentRunId: null,
    id: `agent_goal_${store.goals.length + 1}`,
    objective: input.objective,
    pausedAt: null,
    projectId: input.projectId ?? null,
    status: 'active',
    teamId: input.teamId ?? null,
    timeUsedSeconds: 0,
    tokenBudget: null,
    tokensUsed: 0,
    updatedAt: store.runtime.nowIso(),
    userId: input.userId ?? null,
    visibility: input.visibility ?? 'private',
  })

const sameGoalScope = (
  goal: AgentGoalRecord,
  input: Readonly<{
    agentId: string
    projectId?: string | null | undefined
    teamId?: string | null | undefined
    userId?: string | null | undefined
  }>,
): boolean =>
  goal.agentId === input.agentId &&
  goal.userId === (input.userId ?? null) &&
  goal.teamId === (input.teamId ?? null) &&
  goal.projectId === (input.projectId ?? null)

const makeMemoryGoalRepository = (
  store: AdjutantAssignmentDbStore,
): AgentGoalRepositoryShape => ({
  accountUsage: () => Effect.die(new Error('accountUsage is not used')),
  archiveGoal: () => Effect.die(new Error('archiveGoal is not used')),
  attachRun: () => Effect.die(new Error('attachRun is not used')),
  changeVisibility: () => Effect.die(new Error('changeVisibility is not used')),
  clearTokenBudget: () => Effect.die(new Error('clearTokenBudget is not used')),
  editObjective: () => Effect.die(new Error('editObjective is not used')),
  getById: goalId =>
    Effect.gen(function* () {
      const goal = store.goals.find(
        item => item.id === goalId && item.archivedAt === null,
      )

      if (goal === undefined) {
        return yield* new AgentGoalNotFound({ goalId })
      }

      return goal
    }),
  getCurrent: scope =>
    Effect.succeed(
      store.goals.find(
        goal => goal.archivedAt === null && sameGoalScope(goal, scope),
      ),
    ),
  getPublicCurrentByAgentId: agentId =>
    Effect.succeed(
      store.goals.find(
        goal =>
          goal.agentId === agentId &&
          goal.visibility === 'public' &&
          goal.archivedAt === null,
      ),
    ),
  setGoal: input =>
    Effect.gen(function* () {
      store.goals = store.goals.map(goal =>
        goal.archivedAt === null && sameGoalScope(goal, input)
          ? new AgentGoalRecord({
              ...goal,
              archivedAt: store.runtime.nowIso(),
              updatedAt: store.runtime.nowIso(),
            })
          : goal,
      )
      const goal = makeGoal(store, input)

      store.goals.push(goal)

      return goal
    }),
  setStatus: () => Effect.die(new Error('setStatus is not used')),
  setTokenBudget: () => Effect.die(new Error('setTokenBudget is not used')),
})

const serviceFor = (store: AdjutantAssignmentDbStore) =>
  makeAdjutantAssignmentService(
    adjutantAssignmentDb(store),
    store.runtime,
    makeMemoryGoalRepository(store),
  )

describe('AdjutantAssignmentService', () => {
  test('creates an assignment from a software order', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)
    const assignment = await Effect.runPromise(
      service.createAssignment({
        assignmentKind: 'site_generation',
        assignedByUserId: 'github:core',
        objective: 'Generate the OTEC Site.',
        softwareOrderId: 'software_order_otec',
        visibility: 'public',
      }),
    )

    expect(assignment).toMatchObject({
      id: 'adjutant_assignment_1',
      softwareOrderId: 'software_order_otec',
      siteId: null,
      agentId: 'agent_adjutant',
      goalId: 'agent_goal_1',
      teamId: 'team_openagents_core',
      projectId: 'project_adjutant',
      assignmentKind: 'site_generation',
      status: 'draft',
      visibility: 'public',
      objective: 'Generate the OTEC Site.',
    })
    expect(store.assignments).toHaveLength(1)
    expect(store.events).toEqual([
      {
        actor_user_id: 'github:core',
        assignment_id: 'adjutant_assignment_1',
        created_at: '2026-06-05T12:00:00.000Z',
        event_type: 'adjutant.assignment_created',
        goal_id: 'agent_goal_1',
        id: 'adjutant_assignment_event_1',
        payload_json: JSON.stringify({
          assignmentKind: 'site_generation',
          status: 'draft',
          visibility: 'public',
        }),
        run_id: null,
        site_id: null,
        software_order_id: 'software_order_otec',
        summary: 'Autopilot assignment created.',
        visibility: 'public',
      },
    ])
    expect(store.goals).toEqual([
      expect.objectContaining({
        agentId: 'agent_adjutant',
        id: 'agent_goal_1',
        projectId: 'project_adjutant',
        teamId: 'team_openagents_core',
        visibility: 'public',
      }),
    ])
  })

  test('creates an assignment from a Site and infers the software order', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)
    const assignment = await Effect.runPromise(
      service.createAssignment({
        assignmentKind: 'site_adjustment',
        objective: 'Adjust the OTEC Site hero.',
        siteId: 'site_project_otec',
      }),
    )

    expect(assignment).toMatchObject({
      goalId: 'agent_goal_1',
      softwareOrderId: 'software_order_otec',
      siteId: 'site_project_otec',
      assignmentKind: 'site_adjustment',
    })
  })

  test('reuses the durable Adjutant goal for related Site follow-up work', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)
    const assignments = await Effect.runPromise(
      Effect.gen(function* () {
        const first = yield* service.createAssignment({
          assignmentKind: 'site_generation',
          objective: 'Generate the OTEC Site.',
          siteId: 'site_project_otec',
          status: 'complete',
        })
        const second = yield* service.createAssignment({
          assignmentKind: 'site_adjustment',
          objective: 'Adjust the OTEC Site hero.',
          siteId: 'site_project_otec',
        })

        return [first, second] as const
      }),
    )

    expect(assignments[0].goalId).toBe('agent_goal_1')
    expect(assignments[1].goalId).toBe('agent_goal_1')
    expect(store.goals).toHaveLength(1)
  })

  test('rejects explicit goal IDs that do not exist', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)
    const error = await Effect.runPromise(
      service
        .createAssignment({
          assignmentKind: 'site_generation',
          goalId: 'agent_goal_missing',
          objective: 'Generate the OTEC Site.',
          softwareOrderId: 'software_order_otec',
        })
        .pipe(Effect.flip),
    )

    expect(error).toBeInstanceOf(AdjutantAssignmentGoalNotFound)
    expect(store.assignments).toHaveLength(0)
  })

  test('blocks duplicate active assignments for the same order or Site', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)

    await Effect.runPromise(
      service.createAssignment({
        assignmentKind: 'site_generation',
        objective: 'Generate the OTEC Site.',
        siteId: 'site_project_otec',
      }),
    )

    const error = await Effect.runPromise(
      service
        .createAssignment({
          assignmentKind: 'site_adjustment',
          objective: 'Adjust the OTEC Site.',
          softwareOrderId: 'software_order_otec',
        })
        .pipe(Effect.flip),
    )

    expect(error).toBeInstanceOf(AdjutantAssignmentActiveExists)
    expect(store.assignments).toHaveLength(1)
  })

  test('updates lifecycle pointers and lists assignments', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)
    const assignments = await Effect.runPromise(
      Effect.gen(function* () {
        const created = yield* service.createAssignment({
          assignmentKind: 'site_generation',
          objective: 'Generate the OTEC Site.',
          softwareOrderId: 'software_order_otec',
        })

        yield* service.updateAssignment({
          assignmentId: created.id,
          commitSha: '5b8262d',
          currentRunId: 'agent_run_1',
          goalId: 'agent_goal_1',
          status: 'queued',
          taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
        })

        return yield* service.listAssignments(10)
      }),
    )

    expect(assignments).toHaveLength(1)
    expect(assignments[0]).toMatchObject({
      commitSha: '5b8262d',
      currentRunId: 'agent_run_1',
      goalId: 'agent_goal_1',
      status: 'queued',
      taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
    })
  })

  test('rejects run-linked assignment updates without a durable goal', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)
    store.assignments.push({
      agent_id: 'agent_adjutant',
      archived_at: null,
      assigned_by_user_id: null,
      assignment_kind: 'site_generation',
      blocked_at: null,
      commit_sha: null,
      completed_at: null,
      created_at: store.runtime.nowIso(),
      current_run_id: null,
      goal_id: null,
      id: 'adjutant_assignment_legacy',
      objective: 'Legacy assignment without a goal.',
      project_id: 'project_adjutant',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
      status: 'draft',
      task_spec_path: null,
      team_id: 'team_openagents_core',
      updated_at: store.runtime.nowIso(),
      visibility: 'team',
    })

    const error = await Effect.runPromise(
      service
        .updateAssignment({
          assignmentId: 'adjutant_assignment_legacy',
          currentRunId: 'agent_run_stopped',
          status: 'review_needed',
        })
        .pipe(Effect.flip),
    )

    expect(error).toBeInstanceOf(AdjutantAssignmentRunGoalRequired)
  })

  test('rejects secret-shaped assignment payloads', async () => {
    const store = new AdjutantAssignmentDbStore()
    const service = serviceFor(store)
    const error = await Effect.runPromise(
      service
        .createAssignment({
          assignmentKind: 'site_generation',
          objective: 'Generate with OPENAI_API_KEY=sk-test-secret.',
          softwareOrderId: 'software_order_otec',
        })
        .pipe(Effect.flip),
    )

    expect(error).toBeInstanceOf(AdjutantAssignmentUnsafePayload)
    expect(store.assignments).toHaveLength(0)
    expect(store.goals).toHaveLength(0)
  })
})
