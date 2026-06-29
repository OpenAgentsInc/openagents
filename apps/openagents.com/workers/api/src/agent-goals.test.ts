import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AgentGoalAccessDenied,
  AgentGoalStaleUpdate,
  makeAgentGoalAccessService,
  makeD1AgentGoalRepository,
} from './agent-goals'

type GoalRow = Readonly<{
  id: string
  agent_id: string
  user_id: string | null
  team_id: string | null
  project_id: string | null
  objective: string
  status:
    | 'active'
    | 'paused'
    | 'blocked'
    | 'usage_limited'
    | 'budget_limited'
    | 'complete'
  visibility: 'private' | 'team' | 'public'
  current_run_id: string | null
  token_budget: number | null
  tokens_used: number
  time_used_seconds: number
  created_at: string
  updated_at: string
  completed_at: string | null
  paused_at: string | null
  blocked_at: string | null
  archived_at: string | null
}>

type RunRow = Readonly<{
  goal_id: string | null
  id: string
  updated_at: string | null
}>

type QueryBinding = string | number | null

type MemoryD1State = Readonly<{
  goals: Map<string, GoalRow>
  runs: Map<string, RunRow>
}>

type MemoryPreparedStatement = D1PreparedStatement &
  Readonly<{
    query: string
    values: ReadonlyArray<QueryBinding>
  }>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 1,
  rows_read: 0,
  rows_written: 1,
  size_after: 0,
})

const d1Result = <T = unknown>(results: ReadonlyArray<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results: [...results],
  success: true,
})

const compactSql = (query: string): string => query.replace(/\s+/g, ' ').trim()

const sameScope = (
  row: GoalRow,
  agentId: string,
  userId: string | null,
  teamId: string | null,
  projectId: string | null,
): boolean =>
  row.agent_id === agentId &&
  row.user_id === userId &&
  row.team_id === teamId &&
  row.project_id === projectId

const copyGoal = (row: GoalRow): GoalRow => ({ ...row })

const makeMemoryD1 = (
  state: MemoryD1State = {
    goals: new Map(),
    runs: new Map(),
  },
): D1Database & MemoryD1State => {
  const runStatement = (statement: MemoryPreparedStatement): D1Result => {
    const sql = compactSql(statement.query)
    const values = statement.values

    if (
      sql.startsWith('UPDATE agent_goals SET archived_at = ?') &&
      sql.includes('WHERE id = ?')
    ) {
      const goal = state.goals.get(String(values[2]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          archived_at: String(values[0]),
          updated_at: String(values[1]),
        })
      }

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET archived_at = ?')) {
      const now = String(values[0])
      const agentId = String(values[2])
      const userId = values[3] === null ? null : String(values[3])
      const teamId = values[4] === null ? null : String(values[4])
      const projectId = values[5] === null ? null : String(values[5])
      const nextGoals = Array.from(state.goals.values()).map(goal =>
        goal.archived_at === null &&
        sameScope(goal, agentId, userId, teamId, projectId)
          ? { ...goal, archived_at: now, updated_at: now }
          : goal,
      )

      state.goals.clear()
      nextGoals.map(goal => state.goals.set(goal.id, goal))

      return d1Result()
    }

    if (sql.startsWith('INSERT INTO agent_goals')) {
      const row: GoalRow = {
        id: String(values[0]),
        agent_id: String(values[1]),
        user_id: values[2] === null ? null : String(values[2]),
        team_id: values[3] === null ? null : String(values[3]),
        project_id: values[4] === null ? null : String(values[4]),
        objective: String(values[5]),
        status: 'active',
        visibility: values[6] as GoalRow['visibility'],
        current_run_id: null,
        token_budget: values[7] === null ? null : Number(values[7]),
        tokens_used: 0,
        time_used_seconds: 0,
        created_at: String(values[8]),
        updated_at: String(values[9]),
        completed_at: null,
        paused_at: null,
        blocked_at: null,
        archived_at: null,
      }

      state.goals.set(row.id, row)

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET tokens_used = ?')) {
      const goal = state.goals.get(String(values[4]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          tokens_used: Number(values[0]),
          time_used_seconds: Number(values[1]),
          status: values[2] as GoalRow['status'],
          updated_at: String(values[3]),
        })
      }

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET current_run_id = ?')) {
      const goal = state.goals.get(String(values[2]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          current_run_id: String(values[0]),
          updated_at: String(values[1]),
        })
      }

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_runs SET goal_id = ?')) {
      const runId = String(values[2])
      const run = state.runs.get(runId)

      state.runs.set(runId, {
        id: runId,
        ...run,
        goal_id: String(values[0]),
        updated_at: String(values[1]),
      })

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET visibility = ?')) {
      const goal = state.goals.get(String(values[2]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          visibility: values[0] as GoalRow['visibility'],
          updated_at: String(values[1]),
        })
      }

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET token_budget = NULL')) {
      const goal = state.goals.get(String(values[1]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          token_budget: null,
          status: goal.status === 'budget_limited' ? 'active' : goal.status,
          updated_at: String(values[0]),
        })
      }

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET objective = ?')) {
      const goal = state.goals.get(String(values[2]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          objective: String(values[0]),
          status:
            goal.status === 'complete' || goal.status === 'budget_limited'
              ? 'active'
              : goal.status,
          completed_at: goal.status === 'complete' ? null : goal.completed_at,
          updated_at: String(values[1]),
        })
      }

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET status = ?')) {
      const goal = state.goals.get(String(values[5]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          status: values[0] as GoalRow['status'],
          completed_at: values[1] === null ? null : String(values[1]),
          paused_at: values[2] === null ? null : String(values[2]),
          blocked_at: values[3] === null ? null : String(values[3]),
          updated_at: String(values[4]),
        })
      }

      return d1Result()
    }

    if (sql.startsWith('UPDATE agent_goals SET token_budget = ?')) {
      const goal = state.goals.get(String(values[3]))

      if (goal !== undefined && goal.archived_at === null) {
        state.goals.set(goal.id, {
          ...goal,
          token_budget: Number(values[0]),
          status: values[1] as GoalRow['status'],
          updated_at: String(values[2]),
        })
      }

      return d1Result()
    }

    throw new Error(`Unhandled memory D1 run query: ${sql}`)
  }

  const firstStatement = <T>(
    statement: MemoryPreparedStatement,
  ): T | null => {
    const sql = compactSql(statement.query)
    const values = statement.values

    if (sql.startsWith('SELECT * FROM agent_goals WHERE id = ?')) {
      const goal = state.goals.get(String(values[0]))

      return (goal === undefined ? null : copyGoal(goal)) as T | null
    }

    if (sql.startsWith('SELECT * FROM agent_goals WHERE agent_id = ?')) {
      const agentId = String(values[0])
      const publicOnly = sql.includes("visibility = 'public'")

      if (publicOnly) {
        const publicGoal = Array.from(state.goals.values()).find(row =>
          row.agent_id === agentId &&
          row.visibility === 'public' &&
          row.archived_at === null,
        )

        return (
          publicGoal === undefined ? null : copyGoal(publicGoal)
        ) as T | null
      }

      const userId = values[1] === null ? null : String(values[1])
      const teamId = values[2] === null ? null : String(values[2])
      const projectId = values[3] === null ? null : String(values[3])
      const goal = Array.from(state.goals.values()).find(row =>
        row.archived_at === null &&
        sameScope(row, agentId, userId, teamId, projectId)
      )

      return (goal === undefined ? null : copyGoal(goal)) as T | null
    }

    throw new Error(`Unhandled memory D1 first query: ${sql}`)
  }

  const makeStatement = (
    query: string,
    values: ReadonlyArray<QueryBinding> = [],
  ): MemoryPreparedStatement => {
    const statement = {
      query,
      values,
      all: <T = unknown>() => Promise.resolve(d1Result<T>()),
      bind: (...nextValues: ReadonlyArray<QueryBinding>) =>
        makeStatement(query, nextValues),
      first: <T = unknown>() => Promise.resolve(firstStatement<T>(statement)),
      raw: () => Promise.resolve([]),
      run: () => Promise.resolve(runStatement(statement)),
    } as unknown as MemoryPreparedStatement

    return statement
  }

  const db: D1Database & MemoryD1State = {
    goals: state.goals,
    runs: state.runs,
    batch: <T = unknown>(statements: ReadonlyArray<D1PreparedStatement>) =>
      Promise.all(
        statements.map(nextStatement =>
          (nextStatement as MemoryPreparedStatement).run(),
        ),
      ) as Promise<Array<D1Result<T>>>,
    dump: () => {
      throw new Error('dump is unused')
    },
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: query => makeStatement(query),
    withSession: () => {
      throw new Error('withSession is unused')
    },
  }

  return db
}

const testRuntime = {
  nowIso: () => '2026-06-04T12:00:00.000Z',
  randomId: (prefix: string) => `${prefix}_1`,
}

const createGoal = (
  repository = makeD1AgentGoalRepository(makeMemoryD1(), testRuntime),
) =>
  Effect.runPromise(
    repository.setGoal({
      agentId: 'agent_artanis',
      objective: 'Build the public Artanis goal loop.',
      projectId: 'project_artanis',
      teamId: 'team_openagents_core',
      tokenBudget: 100,
      userId: 'github:1',
      visibility: 'team',
    }),
  )

describe('AgentGoalRepository', () => {
  test('creates and reads the current goal for an agent scope', async () => {
    const db = makeMemoryD1()
    const repository = makeD1AgentGoalRepository(db, testRuntime)
    const goal = await createGoal(repository)
    const current = await Effect.runPromise(
      repository.getCurrent({
        agentId: 'agent_artanis',
        projectId: 'project_artanis',
        teamId: 'team_openagents_core',
        userId: 'github:1',
      }),
    )

    expect(goal.id).toBe('agent_goal_1')
    expect(goal.status).toBe('active')
    expect(current?.id).toBe(goal.id)
  })

  test('archives the previous current goal when replacing an objective', async () => {
    const db = makeMemoryD1()
    const repository = makeD1AgentGoalRepository(db, {
      nowIso: () => '2026-06-04T12:01:00.000Z',
      randomId: prefix => `${prefix}_replacement`,
    })
    await createGoal(repository)
    const replacement = await Effect.runPromise(
      repository.setGoal({
        agentId: 'agent_artanis',
        objective: 'Ship the second goal.',
        projectId: 'project_artanis',
        teamId: 'team_openagents_core',
        userId: 'github:1',
        visibility: 'private',
      }),
    )

    expect(replacement.id).toBe('agent_goal_replacement')
    expect(Array.from(db.goals.values()).filter(goal => goal.archived_at === null))
      .toHaveLength(1)
  })

  test('supports objective edits, pause, resume, complete, and blocked states', async () => {
    const db = makeMemoryD1()
    const repository = makeD1AgentGoalRepository(db, testRuntime)
    const goal = await createGoal(repository)
    const edited = await Effect.runPromise(
      repository.editObjective(goal.id, 'Ship the goal UI.'),
    )
    const paused = await Effect.runPromise(repository.setStatus(goal.id, 'paused'))
    const resumed = await Effect.runPromise(repository.setStatus(goal.id, 'active'))
    const completed = await Effect.runPromise(
      repository.setStatus(goal.id, 'complete'),
    )
    const blocked = await Effect.runPromise(
      repository.setStatus(goal.id, 'blocked'),
    )

    expect(edited.objective).toBe('Ship the goal UI.')
    expect(paused.pausedAt).toBe('2026-06-04T12:00:00.000Z')
    expect(resumed.status).toBe('active')
    expect(completed.completedAt).toBe('2026-06-04T12:00:00.000Z')
    expect(blocked.blockedAt).toBe('2026-06-04T12:00:00.000Z')
  })

  test('normalizes usage accounting to budget_limited and rejects stale goal ids', async () => {
    const db = makeMemoryD1()
    const repository = makeD1AgentGoalRepository(db, testRuntime)
    const goal = await createGoal(repository)
    const accounted = await Effect.runPromise(
      repository.accountUsage({
        expectedGoalId: goal.id,
        goalId: goal.id,
        timeDeltaSeconds: 30,
        tokenDelta: 100,
      }),
    )
    const stale = await Effect.runPromise(
      Effect.flip(
        repository.accountUsage({
          expectedGoalId: 'agent_goal_old',
          goalId: goal.id,
          tokenDelta: 1,
        }),
      ),
    )

    expect(accounted.status).toBe('budget_limited')
    expect(accounted.tokensUsed).toBe(100)
    expect(accounted.timeUsedSeconds).toBe(30)
    expect(stale).toBeInstanceOf(AgentGoalStaleUpdate)
  })

  test('attaches a run to the goal and to agent_runs.goal_id', async () => {
    const db = makeMemoryD1({
      goals: new Map(),
      runs: new Map([
        ['run_1', { goal_id: null, id: 'run_1', updated_at: null }],
      ]),
    })
    const repository = makeD1AgentGoalRepository(db, testRuntime)
    const goal = await createGoal(repository)
    const attached = await Effect.runPromise(
      repository.attachRun({
        expectedGoalId: goal.id,
        goalId: goal.id,
        runId: 'run_1',
      }),
    )

    expect(attached.currentRunId).toBe('run_1')
    expect(db.runs.get('run_1')?.goal_id).toBe(goal.id)
  })

  test('archives goals so they are not returned as current', async () => {
    const db = makeMemoryD1()
    const repository = makeD1AgentGoalRepository(db, testRuntime)
    const goal = await createGoal(repository)
    const archived = await Effect.runPromise(repository.archiveGoal(goal.id))
    const current = await Effect.runPromise(
      repository.getCurrent({
        agentId: 'agent_artanis',
        projectId: 'project_artanis',
        teamId: 'team_openagents_core',
        userId: 'github:1',
      }),
    )

    expect(archived.archivedAt).toBe('2026-06-04T12:00:00.000Z')
    expect(current).toBeUndefined()
  })

  test('validates positive budgets', async () => {
    const db = makeMemoryD1()
    const repository = makeD1AgentGoalRepository(db, testRuntime)
    const failure = await Effect.runPromise(
      Effect.flip(
        repository.setGoal({
          agentId: 'agent_artanis',
          objective: 'Invalid budget.',
          tokenBudget: 0,
          userId: 'github:1',
        }),
      ),
    )

    expect(failure._tag).toBe('AgentGoalValidationError')
  })
})

describe('AgentGoalAccessService', () => {
  test('enforces owner/team/public reads and owner/operator writes', async () => {
    const goal = await createGoal()
    const access = makeAgentGoalAccessService()

    expect(
      access.canRead(
        { teamIds: ['team_openagents_core'], userId: 'github:2' },
        goal,
      ),
    ).toBe(true)
    expect(access.canWrite({ userId: 'github:2' }, goal)).toBe(false)
    expect(access.canWrite({ operator: true }, goal)).toBe(true)
  })

  test('public projection excludes private scope fields', async () => {
    const db = makeMemoryD1()
    const repository = makeD1AgentGoalRepository(db, testRuntime)
    const access = makeAgentGoalAccessService()
    const goal = await createGoal(repository)
    const publicGoal = await Effect.runPromise(
      repository.changeVisibility(goal.id, 'public').pipe(
        Effect.flatMap(updated => access.publicProjection(updated)),
      ),
    )
    const privateProjection = await Effect.runPromise(
      Effect.flip(access.publicProjection(goal)),
    )

    expect(Object.keys(publicGoal)).not.toContain('userId')
    expect(Object.keys(publicGoal)).not.toContain('teamId')
    expect(Object.keys(publicGoal)).not.toContain('projectId')
    expect(Object.keys(publicGoal)).not.toContain('visibility')
    expect(privateProjection).toBeInstanceOf(AgentGoalAccessDenied)
  })
})
