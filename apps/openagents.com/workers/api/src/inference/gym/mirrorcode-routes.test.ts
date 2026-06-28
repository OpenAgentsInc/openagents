import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import {
  buildMirrorCodeTokenBurnReport,
  buildMirrorCodeLaunchRun,
  buildMirrorCodeRun,
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
  exactTokenUsageEventRefs: ['token_usage_event.gym_mirrorcode.cal.0001'],
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
    expect(built.exactTokenUsageEventRefs).toEqual([
      'token_usage_event.gym_mirrorcode.cal.0001',
    ])
    expect(built.tokenAttributionTruth).toBe('exact_rows_as_proof')
    expect(built.tokenAttributionProofRef).toBe(
      'proof.gym.mirrorcode.exact_token_rows.mc-phase0-cal-py-0001',
    )
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

  test('decision_grade scored run requires exact token row refs', () => {
    expect(() =>
      buildMirrorCodeRun({
        ...validInput,
        exactTokenUsageEventRefs: [],
        grade: 'decision_grade',
      }),
    ).toThrow(MirrorCodeRunError)
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

  test('rejects task ids outside the selected public bucket', () => {
    expect(() =>
      buildMirrorCodeRun({ ...validInput, taskId: 'ruff', bucket: 'S' }),
    ).toThrow(MirrorCodeRunError)
  })

  test('rejects a malformed body', () => {
    expect(() => buildMirrorCodeRun({ runId: 'x' })).toThrow(MirrorCodeRunError)
  })

  test('builds an owner-gated queued launch row', () => {
    const built = buildMirrorCodeLaunchRun(
      {
        kind: 'launch',
        taskId: 'qsv_select',
        bucket: 'S',
        language: 'python',
      },
      '2026-06-27T02:03:04.000Z',
    )
    expect(built.runId).toBe('mc-s-qsv-select-python-20260627020304')
    expect(built.status).toBe('queued')
    expect(built.tokensTotal).toBe(0)
    expect(built.passRate).toBeNull()
    expect(built.decisionGrade).toBe(false)
    expect(built.summary).toContain('Owner-gated MirrorCode launch queued')
  })

  test('rejects an owner-gated launch for an unknown public target', () => {
    expect(() =>
      buildMirrorCodeLaunchRun(
        {
          kind: 'launch',
          taskId: 'private_eval_target',
          bucket: 'S',
          language: 'python',
        },
        '2026-06-27T02:03:04.000Z',
      ),
    ).toThrow(MirrorCodeRunError)
  })
})

describe('buildMirrorCodeTokenBurnReport', () => {
  test('aggregates total burn while separating exact-backed tokens', () => {
    const smokeRun = buildMirrorCodeRun({
      ...validInput,
      runId: 'mc-s-cal-python-smoke',
      exactTokenUsageEventRefs: [],
      tokens: { total: 100 },
      grade: 'smoke',
    })
    const decisionRun = buildMirrorCodeRun({
      ...validInput,
      runId: 'mc-l-ruff-python-decision',
      taskId: 'ruff',
      bucket: 'L',
      exactTokenUsageEventRefs: [
        'token_usage_event.gym_mirrorcode.ruff.0002',
        'token_usage_event.gym_mirrorcode.ruff.0001',
      ],
      tokens: { total: 300 },
      grade: 'decision_grade',
    })

    const report = buildMirrorCodeTokenBurnReport([smokeRun, decisionRun])

    expect(report.schemaVersion).toBe(
      'openagents.gym.mirrorcode_token_burn_report.v1',
    )
    expect(report.runCount).toBe(2)
    expect(report.terminalRunCount).toBe(2)
    expect(report.decisionGradeRunCount).toBe(1)
    expect(report.totalTokensBurned).toBe(400)
    expect(report.exactTokenBackedTokens).toBe(300)
    expect(report.unprovenTokenTotal).toBe(100)
    expect(report.exactTokenUsageEventRefs).toEqual([
      'token_usage_event.gym_mirrorcode.ruff.0001',
      'token_usage_event.gym_mirrorcode.ruff.0002',
    ])
    expect(report.byBucket.find(bucket => bucket.bucket === 'S')).toMatchObject({
      runCount: 1,
      totalTokensBurned: 100,
      exactTokenBackedTokens: 0,
    })
    expect(report.byBucket.find(bucket => bucket.bucket === 'L')).toMatchObject({
      runCount: 1,
      totalTokensBurned: 300,
      exactTokenBackedTokens: 300,
    })
    expect(report.byGrade.find(grade => grade.grade === 'smoke')).toMatchObject({
      runCount: 1,
      totalTokensBurned: 100,
      exactTokenBackedTokens: 0,
    })
    expect(report.topRuns[0]?.runId).toBe('mc-l-ruff-python-decision')
    expect(report.demandSource).toBe('gym_mirrorcode')
    expect(report.caveatRefs).toContain(
      'caveat.public.gym.mirrorcode.exact_token_rows_required_for_proof',
    )
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
      staleness: { composition: string }
    }
    expect(body.schemaVersion).toBe('openagents.gym.mirrorcode_runs.v1')
    expect(body.model).toBe('openagents/khala')
    expect(body.runs[0]?.runId).toBe('mc-phase0-cal-py-0001')
    expect(body.comparators.length).toBeGreaterThan(0)
    expect(
      body.comparators.every(
        c => c.source === 'paper_reference_illustrative',
      ),
    ).toBe(true)
    expect(body.staleness.composition).toBe('live_at_read')
  })
})

describe('handleMirrorCodeTokenBurnReportApi', () => {
  test('public GET returns an automated live token-burn report', async () => {
    const built = buildMirrorCodeRun(validInput)
    const response = await run(
      handleMirrorCodeRunsApi(
        new Request('https://openagents.com/api/gym/mirrorcode/token-burn'),
        {
          requireAdminApiToken: async () => false,
          listRuns: () => [built],
          nowIso: () => '2026-06-28T00:00:00.000Z',
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      schemaVersion: string
      scope: string
      generatedAt: string
      staleness: { composition: string }
      report: {
        runCount: number
        totalTokensBurned: number
        exactTokenBackedTokens: number
        exactTokenUsageEventRefs: ReadonlyArray<string>
      }
    }
    expect(body.schemaVersion).toBe(
      'openagents.gym.mirrorcode_token_burn_report.v1',
    )
    expect(body.scope).toBe('public')
    expect(body.generatedAt).toBe('2026-06-28T00:00:00.000Z')
    expect(body.staleness.composition).toBe('live_at_read')
    expect(body.report.runCount).toBe(1)
    expect(body.report.totalTokensBurned).toBe(12_345_678)
    expect(body.report.exactTokenBackedTokens).toBe(12_345_678)
    expect(body.report.exactTokenUsageEventRefs).toEqual([
      'token_usage_event.gym_mirrorcode.cal.0001',
    ])
  })

  test('non-GET is rejected', async () => {
    const response = await run(
      handleMirrorCodeRunsApi(
        new Request('https://openagents.com/api/gym/mirrorcode/token-burn', {
          method: 'POST',
        }),
        { requireAdminApiToken: async () => false, listRuns: () => [] },
      ),
    )

    expect(response.status).toBe(405)
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
          getRun: () => Effect.sync(() => undefined),
          upsertRun: () => Effect.sync(() => {
            stored += 1
          }),
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
          getRun: () => Effect.sync(() => undefined),
          upsertRun: () => Effect.sync(() => {
            stored += 1
          }),
        },
      }),
    )
    expect(response.status).toBe(201)
    expect(stored).toBe(1)
    const body = (await response.json()) as { kind: string; run: { runId: string } }
    expect(body.kind).toBe('mirrorcode_run_recorded')
    expect(body.run.runId).toBe('mc-phase0-cal-py-0001')
  })

  test('authorized POST can launch a queued owner-gated run', async () => {
    let storedRunId = ''
    const response = await run(
      handleMirrorCodeRunsApi(
        postRequest({
          kind: 'launch',
          taskId: 'qsv_select',
          bucket: 'S',
          language: 'python',
        }),
        {
          requireAdminApiToken: async () => true,
          nowIso: () => '2026-06-27T02:03:04.000Z',
          store: {
            listRuns: () => Effect.succeed([]),
            getRun: () => Effect.sync(() => undefined),
            upsertRun: run => Effect.sync(() => {
              storedRunId = run.runId
            }),
          },
        },
      ),
    )
    expect(response.status).toBe(202)
    const body = (await response.json()) as {
      kind: string
      run: { runId: string; status: string; tokensTotal: number }
    }
    expect(body.kind).toBe('mirrorcode_run_launched')
    expect(body.run.runId).toBe('mc-s-qsv-select-python-20260627020304')
    expect(body.run.status).toBe('queued')
    expect(body.run.tokensTotal).toBe(0)
    expect(storedRunId).toBe('mc-s-qsv-select-python-20260627020304')
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
            getRun: () => Effect.sync(() => undefined),
            upsertRun: () => Effect.sync(() => {
              stored += 1
            }),
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

  test('rejects unsafe or oversized run id path segments', () => {
    expect(
      matchMirrorCodeRunByIdRequest(
        new Request('https://openagents.com/api/gym/mirrorcode/runs/abc%2F123'),
      ),
    ).toBeUndefined()
    expect(
      matchMirrorCodeRunByIdRequest(
        new Request(
          `https://openagents.com/api/gym/mirrorcode/runs/${'x'.repeat(129)}`,
        ),
      ),
    ).toBeUndefined()
  })
})
