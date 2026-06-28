import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import {
  buildMirrorCodeRun,
  MIRRORCODE_GENERALIZATION_SET,
  MirrorCodeRunError,
} from './mirrorcode-contract'
import {
  handleMirrorCodeRunByIdApi,
  handleMirrorCodeRunsApi,
  matchMirrorCodeRunByIdRequest,
} from './mirrorcode-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const validInput = {
  runId: 'mc-phase0-cal-py-0001',
  model: 'openagents/khala' as const,
  taskId: 'cal',
  bucket: 'S' as const,
  language: 'python',
  status: 'passed' as const,
  passRate: 0.42,
  tokens: { total: 12_345_678 },
  startedAt: '2026-06-27T00:00:00.000Z',
  finishedAt: '2026-06-27T01:00:00.000Z',
  summary: 'Phase-0 smoke of cal (S bucket) through openagents/khala.',
  grade: 'smoke' as const,
}

describe('buildMirrorCodeRun', () => {
  test('builds a public-safe run and forces smoke runs to decisionGrade:false', () => {
    const built = buildMirrorCodeRun(validInput)
    expect(built.runId).toBe('mc-phase0-cal-py-0001')
    expect(built.model).toBe('openagents/khala')
    expect(built.tokensTotal).toBe(12_345_678)
    expect(built.passRate).toBe(0.42)
    expect(built.grade).toBe('smoke')
    expect(built.decisionGrade).toBe(false)
    expect(built.demandKind).toBe('internal')
    expect(built.demandSource).toBe('gym_mirrorcode')
  })

  test('decision_grade scored run is decisionGrade:true', () => {
    const built = buildMirrorCodeRun({ ...validInput, grade: 'decision_grade' })
    expect(built.decisionGrade).toBe(true)
  })

  test('non-terminal status drops passRate', () => {
    const built = buildMirrorCodeRun({
      ...validInput,
      status: 'running',
      passRate: 0.9,
    })
    expect(built.status).toBe('running')
    expect(built.passRate).toBeNull()
  })

  test('rejects code fences (possible task contents)', () => {
    expect(() =>
      buildMirrorCodeRun({ ...validInput, summary: 'leak ```code``` here' }),
    ).toThrow(MirrorCodeRunError)
  })

  test('rejects canary strings', () => {
    expect(() =>
      buildMirrorCodeRun({ ...validInput, summary: 'mirrorcode:2b6c69c2' }),
    ).toThrow(MirrorCodeRunError)
  })

  test('rejects a malformed body', () => {
    expect(() => buildMirrorCodeRun({ runId: 'x' })).toThrow(MirrorCodeRunError)
  })
})

describe('handleMirrorCodeRunsApi GET', () => {
  test('public list returns runs + labeled illustrative comparators', async () => {
    const built = buildMirrorCodeRun(validInput)
    const response = await run(
      handleMirrorCodeRunsApi(
        new Request('https://openagents.com/api/gym/mirrorcode/runs'),
        {
          requireAdminApiToken: async () => false,
          listRuns: () => [built],
        },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      schemaVersion: string
      model: string
      runs: ReadonlyArray<{ runId: string }>
      comparators: ReadonlyArray<{ source: string }>
      generalizationSet: typeof MIRRORCODE_GENERALIZATION_SET
      staleness: { composition: string }
    }
    expect(body.schemaVersion).toBe('openagents.gym.mirrorcode_runs.v1')
    expect(body.model).toBe('openagents/khala')
    expect(body.generalizationSet).toEqual(MIRRORCODE_GENERALIZATION_SET)
    expect(body.generalizationSet.retrievalPolicy).toBe('no_rag_on_tasks')
    expect(body.generalizationSet.trainingPolicy).toBe(
      'no_training_or_optimization_on_tasks',
    )
    expect(body.runs[0]?.runId).toBe('mc-phase0-cal-py-0001')
    expect(body.comparators.length).toBeGreaterThan(0)
    expect(body.comparators.every(c => c.source === 'paper_reference_illustrative')).toBe(true)
    expect(body.staleness.composition).toBe('live_at_read')
  })
})

describe('handleMirrorCodeRunsApi POST', () => {
  const postRequest = (body: unknown) =>
    new Request('https://openagents.com/api/gym/mirrorcode/runs', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    })

  test('unauthorized POST is 401 and never stores', async () => {
    let stored = 0
    const response = await run(
      handleMirrorCodeRunsApi(postRequest(validInput), {
        requireAdminApiToken: async () => false,
        store: {
          listRuns: () => Effect.succeed([]),
          getRun: () => Effect.succeed(undefined),
          upsertRun: () => {
            stored += 1
            return Effect.void
          },
        },
      }),
    )
    expect(response.status).toBe(401)
    expect(stored).toBe(0)
  })

  test('authorized POST records the run', async () => {
    let stored = 0
    const response = await run(
      handleMirrorCodeRunsApi(postRequest(validInput), {
        requireAdminApiToken: async () => true,
        store: {
          listRuns: () => Effect.succeed([]),
          getRun: () => Effect.succeed(undefined),
          upsertRun: () => {
            stored += 1
            return Effect.void
          },
        },
      }),
    )
    expect(response.status).toBe(201)
    expect(stored).toBe(1)
    const body = (await response.json()) as { kind: string; run: { runId: string } }
    expect(body.kind).toBe('mirrorcode_run_recorded')
    expect(body.run.runId).toBe('mc-phase0-cal-py-0001')
  })

  test('authorized POST with task contents is rejected 400', async () => {
    let stored = 0
    const response = await run(
      handleMirrorCodeRunsApi(
        postRequest({ ...validInput, summary: 'x ```y``` z' }),
        {
          requireAdminApiToken: async () => true,
          store: {
            listRuns: () => Effect.succeed([]),
            getRun: () => Effect.succeed(undefined),
            upsertRun: () => {
            stored += 1
            return Effect.void
          },
          },
        },
      ),
    )
    expect(response.status).toBe(400)
    expect(stored).toBe(0)
  })
})

describe('handleMirrorCodeRunByIdApi', () => {
  test('returns the run when found', async () => {
    const built = buildMirrorCodeRun(validInput)
    const response = await run(
      handleMirrorCodeRunByIdApi(
        new Request('https://openagents.com/api/gym/mirrorcode/runs/x'),
        'mc-phase0-cal-py-0001',
        { getRun: id => (id === 'mc-phase0-cal-py-0001' ? built : undefined) },
      ),
    )
    expect(response.status).toBe(200)
  })

  test('404 when unknown', async () => {
    const response = await run(
      handleMirrorCodeRunByIdApi(
        new Request('https://openagents.com/api/gym/mirrorcode/runs/x'),
        'nope',
        { getRun: () => undefined },
      ),
    )
    expect(response.status).toBe(404)
  })
})

describe('matchMirrorCodeRunByIdRequest', () => {
  test('matches the by-id path and decodes', () => {
    expect(
      matchMirrorCodeRunByIdRequest(
        new Request('https://openagents.com/api/gym/mirrorcode/runs/abc-123'),
      ),
    ).toBe('abc-123')
  })

  test('does not match the base path', () => {
    expect(
      matchMirrorCodeRunByIdRequest(
        new Request('https://openagents.com/api/gym/mirrorcode/runs'),
      ),
    ).toBeUndefined()
  })
})
