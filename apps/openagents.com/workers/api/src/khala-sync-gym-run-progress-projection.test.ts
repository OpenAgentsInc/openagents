import { describe, expect, test } from 'vitest'

import type { SyncSql } from '@openagentsinc/khala-sync-server'
import type { RawGymRunProgressProjection } from '@openagentsinc/khala-sync-server'

import { projectGymRunProgress } from './khala-sync-gym-run-progress-projection'

const FORBIDDEN = /apikey|authorization:|bearer[:\s]|mnemonic|secret|\/users\/|https?:\/\//i

const webAuthorized = (
  runRef: string,
): RawGymRunProgressProjection => ({
  agent: 'opencode',
  blockerRefs: [],
  caveatRefs: [],
  completionFraction: 0.1685,
  configId: `config.${runRef}`,
  counts: {
    cancelled: 0,
    completed: 13,
    completedFailed: 0,
    completedPassed: 13,
    error: 0,
    officialDenominator: 89,
    pending: 74,
    running: 2,
  },
  decisionGrade: false,
  elapsedMs: 540_000,
  inProgress: true,
  jobRef: `job.${runRef}`,
  lastUpdatedAt: '2026-07-04T15:20:11.412Z',
  passRateOverCompleted: 1,
  phase: 'running',
  profile: {
    attribution: 'Z.ai GLM-5.2 (REAP-504B)',
    contextWindowTokens: 65_536,
    hardwareProfile: 'hydralisk-g4-4x-rtx-pro-6000',
    model: 'openagents/glm-5.2-reap-504b',
    profileRef: 'khala-public-heuristic',
    publicLabel: 'Khala public heuristic',
  },
  publication: 'web_authorized',
  runRef,
  tokens: {
    completionTokens: null,
    promptTokens: null,
    totalTokens: null,
  },
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
      return Promise.resolve([{ committed_at: '2026-07-04T15:20:12.000Z' }])
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

describe('projectGymRunProgress', () => {
  test('skips when the KHALA_SYNC_DB binding is absent (no client created)', async () => {
    const outcome = await projectGymRunProgress(
      {
        binding: undefined,
        makeSqlClient: () => {
          throw new Error('must not be called')
        },
      },
      webAuthorized('run.web.alpha'),
    )
    expect(outcome).toEqual({ outcome: 'skipped_no_binding' })
  })

  test('projects a redacted gym_run_progress post-image and closes the client', async () => {
    const fake = makeFakeSqlClient()
    const outcome = await projectGymRunProgress(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => Promise.resolve(fake.client),
      },
      webAuthorized('run.web.alpha'),
    )
    expect(outcome).toEqual({ outcome: 'projected', runRef: 'run.web.alpha' })
    expect(fake.isEnded()).toBe(true)

    const serialized = JSON.stringify(fake.boundValues)
    expect(serialized).not.toMatch(FORBIDDEN)
    expect(serialized).toContain('run.web.alpha')
    expect(
      fake.statements.some(text => text.includes('khala_sync_changelog')),
    ).toBe(true)
  })

  test('a failing database is fail-soft: typed outcome, log called, no throw', async () => {
    const logged: Array<{ event: string; reason: string }> = []
    const outcome = await projectGymRunProgress(
      {
        binding: { connectionString: 'postgres://x' },
        log: (event, fields) => {
          logged.push({ event, reason: fields.reason })
        },
        makeSqlClient: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: Object.assign(
              () => Promise.reject(new Error('boom')),
              {
                begin: () => Promise.reject(new Error('connection refused')),
              },
            ) as unknown as SyncSql,
          }),
      },
      webAuthorized('run.web.gamma'),
    )
    expect(outcome.outcome).toBe('failed')
    expect(logged).toEqual([
      {
        event: 'khala_sync_gym_run_progress_projection_failed',
        reason: 'projection_failed',
      },
    ])
  })

  test('a throwing client FACTORY is also fail-soft', async () => {
    const outcome = await projectGymRunProgress(
      {
        binding: { connectionString: 'postgres://x' },
        makeSqlClient: () => Promise.reject(new Error('driver import failed')),
      },
      webAuthorized('run.web.delta'),
    )
    expect(outcome.outcome).toBe('failed')
  })
})
