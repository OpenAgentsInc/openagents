import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleOperatorGymRunProgressApi,
  handlePublicGymRunProgressApi,
} from './run-progress-routes'
import { buildGymRunProgress, type GymRunProgress } from './run-progress'

const localOnly: GymRunProgress = buildGymRunProgress({
  runRef: 'run.gym.terminal_bench.local.test',
  jobRef: 'job.gym.harbor_terminal_bench.local.test',
  configId: 'gym.terminal_bench.local.test',
  profileRef: 'khala-public-heuristic',
  agent: 'opencode',
  phase: 'running',
  publication: 'local_only',
  officialDenominator: 89,
  completedPassed: 9,
  completedFailed: 3,
  running: 2,
  pending: 75,
  error: 0,
  cancelled: 0,
  promptTokens: null,
  completionTokens: null,
  elapsedMs: 540_000,
  lastUpdatedAt: '2026-06-25T00:00:00.000Z',
  caveatRefs: [],
  blockerRefs: [],
})

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

describe('operator run-progress route', () => {
  test('returns 401 without an admin token', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        {
          requireAdminApiToken: () => Promise.resolve(false),
          listRunProgress: () => [localOnly],
        },
      ),
    )
    expect(response.status).toBe(401)
  })

  test('returns full progress objects including local_only for an operator', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          listRunProgress: () => [localOnly],
        },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      scope: string
      runs: ReadonlyArray<{ publication: string; counts?: unknown }>
    }
    expect(body.scope).toBe('operator')
    expect(body.runs).toHaveLength(1)
    expect(body.runs[0]?.publication).toBe('local_only')
    // Operators see the live counts even for local_only runs.
    expect(body.runs[0]?.counts).toBeDefined()
  })

  test('returns no runs by default for an authorized operator (live-only)', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress'),
        { requireAdminApiToken: () => Promise.resolve(true) },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { runs: ReadonlyArray<unknown> }
    expect(body.runs).toEqual([])
  })

  test('rejects non-GET methods', async () => {
    const response = await run(
      handleOperatorGymRunProgressApi(
        new Request('https://openagents.com/api/operator/gym/run-progress', {
          method: 'POST',
        }),
        { requireAdminApiToken: () => Promise.resolve(true) },
      ),
    )
    expect(response.status).toBe(405)
  })
})

describe('public run-progress route', () => {
  test('degrades a local_only run honestly with no live counts', async () => {
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
        { listRunProgress: () => [localOnly] },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      scope: string
      runs: ReadonlyArray<{ publication: string; counts?: unknown; blockerRefs: ReadonlyArray<string> }>
    }
    expect(body.scope).toBe('public')
    expect(body.runs[0]?.publication).toBe('local_only')
    expect(body.runs[0]?.counts).toBeUndefined()
    expect(body.runs[0]?.blockerRefs).toContain(
      'blocker.gym.run_progress.not_authorized_for_web_publication',
    )
  })

  test('returns no runs by default (live-only, no seeded fixture)', async () => {
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { runs: ReadonlyArray<unknown> }
    expect(body.runs).toEqual([])
  })

  test('declares generatedAt + the staleness contract on the public payload', async () => {
    const response = await run(
      handlePublicGymRunProgressApi(
        new Request('https://openagents.com/api/public/gym/run-progress'),
        { nowIso: () => '2026-06-25T12:00:00.000Z', listRunProgress: () => [localOnly] },
      ),
    )
    const body = (await response.json()) as {
      generatedAt: string
      staleness: { composition: string; maxStalenessSeconds: number; contractVersion: string }
    }
    expect(body.generatedAt).toBe('2026-06-25T12:00:00.000Z')
    expect(body.staleness.composition).toBe('stored_snapshot')
    expect(body.staleness.maxStalenessSeconds).toBeGreaterThan(0)
    expect(body.staleness.contractVersion).toBe('projection_staleness.v1')
  })
})
