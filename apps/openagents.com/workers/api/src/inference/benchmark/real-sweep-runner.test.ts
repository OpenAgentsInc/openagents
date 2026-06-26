import { describe, expect, test } from 'vitest'

import {
  buildBenchmarkReport,
  checkReportPublicSafety,
} from './report'
import {
  KHALA_ONLY_DECISION_SLICE,
  KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
} from './real-sweep-config'
import {
  BENCHMARK_REAL_SWEEP_DEMAND_SOURCE,
  RealLaneTransportMissingError,
  benchmarkRealSweepAttribution,
  makeSyncRealLaneExecutor,
} from './real-lane-executor'
import {
  makeKhalaPublicTransport,
  makeOpenAICompatibleTransport,
} from './real-lane-transports'
import { RealSweepNotArmedError, runRealSweep } from './real-sweep-runner'
import { preflightRealBenchmarkSweep } from './real-sweep-plan'
import type { RealSweepPreflightOptions } from './real-sweep-plan'
import { expandMatrix } from './matrix'
import type { BenchmarkLane } from './matrix'
import type { RealLaneHttpResult, RealLaneTransport } from './real-lane-executor'

// ---------------------------------------------------------------------------
// Test doubles: a fake fetch returning an OpenAI-compatible body, and a
// deterministic clock.
// ---------------------------------------------------------------------------

const makeFakeFetch = (
  usage: {
    prompt_tokens: number
    completion_tokens: number
    total_tokens?: number
    cached_tokens?: number
  },
  ok = true,
): { fetch: typeof fetch; calls: Array<{ url: string; headers: Headers }> } => {
  const calls: Array<{ url: string; headers: Headers }> = []
  const fakeFetch = (async (url: string, init?: RequestInit) => {
    calls.push({
      url: String(url),
      headers: new Headers(init?.headers ?? {}),
    })
    return {
      ok,
      status: ok ? 200 : 502,
      json: async () => ({
        choices: [{ message: { role: 'assistant', content: 'ok' } }],
        usage: {
          prompt_tokens: usage.prompt_tokens,
          completion_tokens: usage.completion_tokens,
          total_tokens:
            usage.total_tokens ??
            usage.prompt_tokens + usage.completion_tokens,
          ...(usage.cached_tokens === undefined
            ? {}
            : {
                prompt_tokens_details: { cached_tokens: usage.cached_tokens },
              }),
        },
      }),
    } as unknown as Response
  }) as unknown as typeof fetch
  return { fetch: fakeFetch, calls }
}

const stepClock = (stepMs: number): (() => number) => {
  let t = 0
  return () => {
    const current = t
    t += stepMs
    return current
  }
}

const armed: RealSweepPreflightOptions = {
  ownerConfirmed: true,
  ownerApprovalRef: 'owner-approved-real-sweep:oq5-test',
  budgetCapMsat: 5_000_000,
  maxBillableSamples: 1000,
}

describe('real decision sweep config + preflight', () => {
  test('the OQ5 decision suite is decision-grade eligible when owner-armed', () => {
    const preflight = preflightRealBenchmarkSweep(
      KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
      armed,
    )
    expect(preflight.canArmRealSeam).toBe(true)
    expect(preflight.decisionGradeEligible).toBe(true)
    expect(preflight.blockers).toEqual([])
    // All four realistic shapes carry observed evidence refs.
    expect(preflight.realTrafficEvidenceRefs.length).toBe(4)
    expect(preflight.syntheticShapes).toBe(0)
  })

  test('the suite refuses to arm without owner confirmation', () => {
    const preflight = preflightRealBenchmarkSweep(
      KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
      { ...armed, ownerConfirmed: false },
    )
    expect(preflight.canArmRealSeam).toBe(false)
    expect(preflight.blockers.map(b => b.code)).toContain(
      'owner_confirmation_missing',
    )
  })
})

describe('benchmark real sweep attribution (#6298)', () => {
  test('tags the sweep own-Khala load internal + segmented', () => {
    const attribution = benchmarkRealSweepAttribution()
    expect(attribution.demandKind).toBe('internal')
    expect(attribution.demandSource).toBe(BENCHMARK_REAL_SWEEP_DEMAND_SOURCE)
    expect(attribution.demandSource).toBe('benchmark_real_sweep')
  })
})

describe('Khala public transport', () => {
  test('calls /chat/completions and maps provider usage into a measured sample', async () => {
    const { fetch, calls } = makeFakeFetch({
      prompt_tokens: 600,
      completion_tokens: 1400,
      cached_tokens: 100,
    })
    const transport = makeKhalaPublicTransport(
      { fetch, now: stepClock(120) },
      { agentToken: 'agent-token-xyz' },
    )
    const cell = {
      ...expandFirstKhalaCell(),
    }
    const result = await transport.execute(
      cell,
      0,
      benchmarkRealSweepAttribution(),
    )
    expect(result.promptTokens).toBe(600)
    expect(result.completionTokens).toBe(1400)
    expect(result.totalTokens).toBe(2000)
    expect(result.cachedInputTokens).toBe(100)
    // Khala is own-capacity: zero third-party cost basis.
    expect(result.costBasisMsat).toBe(0)
    expect(result.region).toBe('openagents')
    // The request carried the internal benchmark-sweep attribution header.
    expect(calls).toHaveLength(1)
    expect(calls[0]!.url).toBe('https://openagents.com/api/v1/chat/completions')
    expect(calls[0]!.headers.get('x-openagents-demand-kind')).toBe('internal')
    expect(calls[0]!.headers.get('x-openagents-demand-source')).toBe(
      'benchmark_real_sweep',
    )
    expect(calls[0]!.headers.get('authorization')).toBe('Bearer agent-token-xyz')
  })

  test('throws a public-safe error on a non-200 provider response', async () => {
    const { fetch } = makeFakeFetch(
      { prompt_tokens: 1, completion_tokens: 1 },
      false,
    )
    const transport = makeKhalaPublicTransport({ fetch, now: stepClock(10) })
    await expect(
      transport.execute(expandFirstKhalaCell(), 0, null),
    ).rejects.toThrow(/HTTP 502/)
  })
})

describe('billable provider transport records a cost basis', () => {
  test('fireworks rate card yields a non-zero msat cost basis', async () => {
    const { fetch } = makeFakeFetch({
      prompt_tokens: 1000,
      completion_tokens: 1000,
    })
    const transport = makeOpenAICompatibleTransport({
      lane: 'fireworks',
      billable: true,
      baseUrl: 'https://api.fireworks.example/v1',
      apiKey: 'fw-key',
      wireModelRef: 'fireworks/deepseek-v4-flash',
      rateCard: {
        perKPromptMsat: 1500,
        perKCompletionMsat: 4500,
        cachedPromptBilledFraction: 0.5,
      },
      region: 'us-central',
      deps: { fetch, now: stepClock(200) },
    })
    const result = await transport.execute(
      expandFirstFireworksCell(),
      0,
      null,
    )
    // 1000/1000*1500 + 1000/1000*4500 = 6000 msat
    expect(result.costBasisMsat).toBe(6000)
  })
})

describe('runRealSweep gating + decision-grade rule', () => {
  test('refuses to start without a green preflight (never spends unarmed)', async () => {
    const { fetch } = makeFakeFetch({ prompt_tokens: 10, completion_tokens: 10 })
    await expect(
      runRealSweep({
        config: KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
        preflight: { ...armed, ownerConfirmed: false },
        transports: [makeKhalaPublicTransport({ fetch, now: stepClock(50) })],
      }),
    ).rejects.toBeInstanceOf(RealSweepNotArmedError)
  })

  test('Khala-only slice runs now, is honestly NOT decision-grade', async () => {
    const { fetch } = makeFakeFetch({
      prompt_tokens: 600,
      completion_tokens: 1400,
    })
    const runSet = await runRealSweep({
      config: KHALA_ONLY_DECISION_SLICE,
      preflight: armed,
      transports: [makeKhalaPublicTransport({ fetch, now: stepClock(80) })],
    })
    // Khala executed across all four workloads, no billable comparator.
    expect(runSet.seamCanSpend).toBe(false)
    expect(runSet.cellsExecuted).toBeGreaterThan(0)
    const report = buildBenchmarkReport(runSet)
    // No billable comparator ran → not decision-grade (honest).
    expect(report.decisionGrade).toBe(false)
    expect(report.groups.every(g => g.lane === 'khala')).toBe(true)
    // The notice is honest: a REAL run that isn't decision-grade does NOT claim
    // to be the synthetic fixture lane.
    expect(report.illustrativeNotice).toContain('REAL MEASUREMENT')
    expect(report.illustrativeNotice).not.toContain('FIXTURE lane')
    // Public-safety tripwire still passes on a real-lane report.
    expect(checkReportPublicSafety(report).safe).toBe(true)
  })

  test('Khala + billable comparators over realistic traffic earns decisionGrade:true', async () => {
    const khalaFetch = makeFakeFetch({
      prompt_tokens: 600,
      completion_tokens: 1400,
    }).fetch
    const billable = (lane: BenchmarkLane): RealLaneTransport => {
      const { fetch } = makeFakeFetch({
        prompt_tokens: 600,
        completion_tokens: 1400,
      })
      return makeOpenAICompatibleTransport({
        lane,
        billable: true,
        baseUrl: `https://api.${lane}.example/v1`,
        apiKey: `${lane}-key`,
        rateCard: {
          perKPromptMsat: 1500,
          perKCompletionMsat: 4500,
          cachedPromptBilledFraction: 0.5,
        },
        deps: { fetch, now: stepClock(150) },
      })
    }
    const runSet = await runRealSweep({
      config: KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
      preflight: armed,
      transports: [
        makeKhalaPublicTransport({ fetch: khalaFetch, now: stepClock(80) }),
        billable('fireworks'),
        billable('vertex-anthropic'),
        billable('vertex-gemini'),
      ],
    })
    expect(runSet.seamCanSpend).toBe(true)
    const report = buildBenchmarkReport(runSet)
    // Real seam + realistic-only traffic + a billable comparator → decision-grade.
    expect(report.decisionGrade).toBe(true)
    expect(report.illustrativeNotice).toBe('')
    // All four lanes present.
    const lanes = new Set(report.groups.map(g => g.lane))
    expect(lanes.has('khala')).toBe(true)
    expect(lanes.has('fireworks')).toBe(true)
    expect(lanes.has('vertex-anthropic')).toBe(true)
    expect(lanes.has('vertex-gemini')).toBe(true)
    // No group is synthetic-only (every shape is realistic).
    expect(report.groups.every(g => g.syntheticOnly === false)).toBe(true)
    expect(checkReportPublicSafety(report).safe).toBe(true)
  })

  test('an un-armed lane is skipped (never fabricated)', async () => {
    const { fetch } = makeFakeFetch({
      prompt_tokens: 600,
      completion_tokens: 1400,
    })
    // Only Khala armed; Fireworks/Vertex have no transport.
    const runSet = await runRealSweep({
      config: KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE,
      preflight: armed,
      transports: [makeKhalaPublicTransport({ fetch, now: stepClock(80) })],
    })
    // 3 un-armed lanes (fireworks + 2 vertex) × 4 workloads × 4 shapes = 48 cells
    // skipped (no transport). The matrix expands targets × workloads × shapes.
    expect(runSet.cellsSkipped).toBe(48)
    const skipReasons = new Set(
      runSet.runs
        .filter(r => r.skippedReason !== null)
        .map(r => r.skippedReason),
    )
    expect(
      [...skipReasons].some(r => r?.startsWith('real_transport_not_armed:')),
    ).toBe(true)
  })
})

describe('sync real lane executor', () => {
  test('maps a sync transport result into a sample and refuses unarmed lanes', () => {
    const result: RealLaneHttpResult = {
      promptTokens: 100,
      completionTokens: 50,
      totalTokens: 150,
      cachedInputTokens: 0,
      ttftMs: 200,
      totalWallClockMs: 500,
      region: 'openagents',
      costBasisMsat: 0,
    }
    const executor = makeSyncRealLaneExecutor([
      { lane: 'khala', billable: false, execute: () => result },
    ])
    const cell = expandFirstKhalaCell()
    const sample = executor(cell, 0)
    expect(sample.promptTokens).toBe(100)
    expect(sample.generationWallClockMs).toBe(300)
    // Unverified by default (no verifier ran): honest none/not_executed.
    expect(sample.executedVerdict).toBe('not_executed')
    expect(() =>
      executor({ ...cell, lane: 'fireworks' }, 0),
    ).toThrow(RealLaneTransportMissingError)
  })
})

// ---------------------------------------------------------------------------
// Helpers to grab a concrete cell for transport-level tests.
// ---------------------------------------------------------------------------

const expandFirstKhalaCell = () => {
  const cell = expandMatrix(KHALA_ONLY_DECISION_SLICE)[0]
  if (cell === undefined) {
    throw new Error('expected at least one khala cell')
  }
  return cell
}

const expandFirstFireworksCell = () => {
  const cell = expandMatrix(KHALA_VS_FIREWORKS_VERTEX_DECISION_SUITE).find(
    c => c.lane === 'fireworks',
  )
  if (cell === undefined) {
    throw new Error('expected at least one fireworks cell')
  }
  return cell
}
