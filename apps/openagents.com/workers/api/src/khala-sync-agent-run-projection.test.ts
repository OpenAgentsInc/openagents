import { describe, expect, test } from 'vitest'

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import { projectAgentRun } from './khala-sync-agent-run-projection'

const FORBIDDEN =
  /apikey|authorization:|bearer[:\s]|mnemonic|secret|providerAccountRef|authGrantRef|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/i

const queuedRun = (runId: string, overrides: Record<string, unknown> = {}) => ({
  backend: 'shc_vm',
  canceledAt: null,
  completedAt: null,
  createdAt: '2026-07-05T12:00:00.000Z',
  failedAt: null,
  goal: 'Run a bounded repo cleanup mission.',
  goalContext: {
    goalId: 'goal.alpha',
    objective: 'Run a bounded repo cleanup mission.',
    remainingTokens: 50_000,
    status: 'active',
    timeUsedSeconds: 0,
    tokenBudget: 100_000,
    tokensUsed: 0,
    visibility: 'private',
  },
  goalId: 'goal.alpha',
  projectId: null,
  repository: {
    owner: 'OpenAgentsInc',
    provider: 'github',
    ref: 'main',
    repo: 'openagents',
  },
  routeId: `agent_run_${runId}`,
  runId,
  runtime: 'opencode_codex',
  startedAt: null,
  status: 'queued',
  teamId: null,
  updatedAt: '2026-07-05T12:00:00.000Z',
  userId: 'user.alice',
  ...overrides,
})

/**
 * A fake transaction-mode SQL client that answers the projector's
 * statements (scope-version allocation, changelog append) and records every
 * bound parameter value for redaction checks.
 */
const makeFakeSqlClient = () => {
  const boundValues: Array<unknown> = []
  const statements: Array<string> = []
  let ended = false
  const tx = (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ): Promise<Array<Record<string, unknown>>> => {
    const text = strings.join('$')
    statements.push(text)
    boundValues.push(...values)
    if (text.includes('khala_sync_scopes')) {
      return Promise.resolve([{ last_version: 1 }])
    }
    if (text.includes('khala_sync_changelog')) {
      return Promise.resolve([{ committed_at: '2026-07-05T12:00:01.000Z' }])
    }
    return Promise.resolve([])
  }
  const sql = Object.assign(tx, {
    begin: <A>(fn: (t: typeof tx) => Promise<A>): Promise<A> => fn(tx),
  }) as unknown as SyncSql
  return {
    boundValues,
    client: {
      end: () => {
        ended = true
        return Promise.resolve()
      },
      sql,
    },
    isEnded: () => ended,
    statements,
  }
}

describe('projectAgentRun', () => {
  test('skips when the KHALA_SYNC_DB binding is absent (no client created)', async () => {
    const outcome = await projectAgentRun(
      {
        binding: undefined,
        makeSqlClient: () => {
          throw new Error('must not be called')
        },
      },
      'run.web.alpha',
      queuedRun('run.web.alpha'),
    )
    expect(outcome).toEqual({ outcome: 'skipped_no_binding' })
  })

  test('projects a redacted agent_run post-image and closes the client', async () => {
    const fake = makeFakeSqlClient()
    const outcome = await projectAgentRun(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => Promise.resolve(fake.client),
      },
      'run.web.alpha',
      queuedRun('run.web.alpha'),
    )
    expect(outcome).toEqual({ outcome: 'projected', runId: 'run.web.alpha' })
    expect(fake.isEnded()).toBe(true)

    const serialized = JSON.stringify(fake.boundValues)
    expect(serialized).not.toMatch(FORBIDDEN)
    expect(serialized).toContain('run.web.alpha')
    expect(serialized).toContain('goal.alpha')
    expect(
      fake.statements.some(text => text.includes('khala_sync_changelog')),
    ).toBe(true)
  })

  test('a failing database is fail-soft: typed outcome, log called, no throw', async () => {
    const logged: Array<{ event: string; reason: string }> = []
    const outcome = await projectAgentRun(
      {
        binding: { connectionString: 'postgres://x' },
        log: (event, fields) => {
          logged.push({ event, reason: fields.reason })
        },
        makeSqlClient: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: Object.assign(() => Promise.reject(new Error('boom')), {
              begin: () => Promise.reject(new Error('connection refused')),
            }) as unknown as SyncSql,
          }),
      },
      'run.web.gamma',
      queuedRun('run.web.gamma'),
    )
    expect(outcome.outcome).toBe('failed')
    expect(logged).toEqual([
      {
        event: 'khala_sync_agent_run_projection_failed',
        reason: 'projection_failed',
      },
    ])
  })

  test('a throwing client FACTORY is also fail-soft', async () => {
    const outcome = await projectAgentRun(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => Promise.reject(new Error('driver import failed')),
      },
      'run.web.delta',
      queuedRun('run.web.delta'),
    )
    expect(outcome.outcome).toBe('failed')
  })

  test('a malformed raw shape refuses without touching storage', async () => {
    const outcome = await projectAgentRun(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => {
          throw new Error('must not be reached')
        },
      },
      'run.web.epsilon',
      queuedRun('run.web.epsilon', { status: 'exploded' }),
    )
    expect(outcome.outcome).toBe('failed')
  })
})
