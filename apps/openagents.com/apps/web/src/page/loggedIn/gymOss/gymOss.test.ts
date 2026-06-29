import { describe, expect, test } from 'vitest'

import {
  buildSceneFrame,
  type SceneFrame,
} from '../../../scene/gymOssSceneElement'
import {
  DEFAULT_FORM,
  clampSamples,
  executeRun,
  planRun,
  renderAggregateTable,
  renderSampleCard,
  sceneFrameForSamples,
} from './controller'
import {
  CONCURRENCY_OPTIONS,
  GPT_OSS_MODEL_ID,
  MAX_IN_FLIGHT,
  NOT_MEASURED,
  aggregateSamples,
  clampConcurrency,
  deriveInterTokenLatencyMs,
  derivePerceivedTps,
  formatMeasured,
  formatSummaryNumber,
  mean,
  parseServerTelemetry,
  percentile,
  rampSweepSteps,
  reconcileSample,
  runConcurrent,
  runRampSweep,
  type ClientTiming,
  type SampleResult,
  type SampleStream,
  type ServerTelemetry,
} from './runner'

// A deterministic, NETWORK-FREE fake stream. Each sample resolves with the given
// per-index fixture; missing indices resolve as a measured OK sample with a
// stable TTFT/TPS so aggregation is hand-checkable.
const fakeStream = (
  fixtures: ReadonlyArray<Partial<SampleResult>> = [],
): SampleStream => {
  return async ({ index }) => {
    const fixture = fixtures[index] ?? {}
    return {
      index,
      status: fixture.status ?? 'ok',
      error: fixture.error ?? null,
      ttftMs: fixture.ttftMs ?? 100 + index,
      totalWallClockMs: fixture.totalWallClockMs ?? 1000,
      perceivedTps: fixture.perceivedTps ?? 50,
      interTokenLatencyMs: fixture.interTokenLatencyMs ?? 20,
      completionTokens: fixture.completionTokens ?? 200,
      source: fixture.source ?? {
        ttft: 'server',
        totalWallClock: 'server',
        tps: 'server',
      },
    }
  }
}

const okClient = (): ClientTiming => ({
  firstContentByteMs: 120,
  endMs: 1100,
  generationMs: 980,
  observedContentDeltas: 196,
})

describe('gym-oss percentile + mean (reused benchmark math)', () => {
  test('nearest-rank percentile matches the benchmark report formula', () => {
    const values = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
    // Nearest-rank: P50 -> ceil(0.5*10)=5 -> 5th value (50); P90 -> 9th (90);
    // P99 -> ceil(0.99*10)=10 -> 10th (100).
    expect(percentile(values, 50)).toBe(50)
    expect(percentile(values, 90)).toBe(90)
    expect(percentile(values, 99)).toBe(100)
    expect(mean(values)).toBe(55)
  })

  test('percentile / mean return null (honest absence) for empty input, never 0', () => {
    expect(percentile([], 50)).toBeNull()
    expect(mean([])).toBeNull()
  })
})

describe('gym-oss measured derivations (mirror khala-telemetry)', () => {
  test('perceived TPS = completion tokens / generation seconds', () => {
    expect(derivePerceivedTps(200, 1000)).toBe(200)
    expect(derivePerceivedTps(100, 2000)).toBe(50)
  })

  test('inter-token latency = generation ms / (tokens - 1)', () => {
    expect(deriveInterTokenLatencyMs(101, 1000)).toBe(10)
  })

  test('derivations return not_measured (never 0) for degenerate inputs', () => {
    expect(derivePerceivedTps(undefined, 1000)).toBe(NOT_MEASURED)
    expect(derivePerceivedTps(200, 0)).toBe(NOT_MEASURED)
    // A single token has undefined inter-token latency.
    expect(deriveInterTokenLatencyMs(1, 1000)).toBe(NOT_MEASURED)
  })
})

describe('gym-oss telemetry parsing', () => {
  test('parses measured numbers and preserves not_measured (never coerces to 0)', () => {
    const parsed = parseServerTelemetry({
      ttftMs: 150,
      totalWallClockMs: 1200,
      promptTokens: 10,
      completionTokens: 'not_measured',
      totalTokens: 'not_measured',
      cachedInputTokens: 0,
    })
    expect(parsed).not.toBeNull()
    expect(parsed?.ttftMs).toBe(150)
    expect(parsed?.completionTokens).toBe(NOT_MEASURED)
    // A real measured 0 is preserved as 0 (distinct from not_measured).
    expect(parsed?.cachedInputTokens).toBe(0)
  })

  test('returns null for a missing block', () => {
    expect(parseServerTelemetry(undefined)).toBeNull()
    expect(parseServerTelemetry(null)).toBeNull()
  })
})

describe('gym-oss reconciliation (server-preferred, client fallback, honest absence)', () => {
  test('prefers server telemetry when present', () => {
    const server: ServerTelemetry = {
      ttftMs: 90,
      totalWallClockMs: 1090,
      promptTokens: 10,
      completionTokens: 200,
      totalTokens: 210,
      cachedInputTokens: 0,
    }
    const result = reconcileSample({
      index: 0,
      status: 'ok',
      client: okClient(),
      server,
    })
    expect(result.ttftMs).toBe(90)
    expect(result.source.ttft).toBe('server')
    // TPS derived over the server generation window (total - ttft = 1000ms).
    expect(result.perceivedTps).toBe(200)
    expect(result.source.tps).toBe('server')
  })

  test('falls back to client timing when the server block is absent', () => {
    const result = reconcileSample({
      index: 1,
      status: 'ok',
      client: okClient(),
      server: null,
    })
    expect(result.ttftMs).toBe(120)
    expect(result.source.ttft).toBe('client')
    expect(result.source.tps).toBe('client')
  })

  test('a failed sample carries the error and ALL not_measured metrics (never fake latency)', () => {
    const result = reconcileSample({
      index: 2,
      status: 'failed',
      error: 'gateway responded 503',
      client: {
        firstContentByteMs: undefined,
        endMs: undefined,
        generationMs: undefined,
        observedContentDeltas: 0,
      },
      server: null,
    })
    expect(result.status).toBe('failed')
    expect(result.error).toBe('gateway responded 503')
    expect(result.ttftMs).toBe(NOT_MEASURED)
    expect(result.totalWallClockMs).toBe(NOT_MEASURED)
    expect(result.perceivedTps).toBe(NOT_MEASURED)
  })
})

describe('gym-oss aggregation over a hand-checked sample set', () => {
  test('P50/P90/P99/mean + aggregate throughput over measured OK samples only', () => {
    const samples: ReadonlyArray<SampleResult> = [
      { index: 0, status: 'ok', error: null, ttftMs: 100, totalWallClockMs: 1000, perceivedTps: 40, interTokenLatencyMs: 25, completionTokens: 100, source: { ttft: 'server', totalWallClock: 'server', tps: 'server' } },
      { index: 1, status: 'ok', error: null, ttftMs: 200, totalWallClockMs: 1100, perceivedTps: 50, interTokenLatencyMs: 20, completionTokens: 110, source: { ttft: 'server', totalWallClock: 'server', tps: 'server' } },
      { index: 2, status: 'ok', error: null, ttftMs: 300, totalWallClockMs: 1200, perceivedTps: 60, interTokenLatencyMs: 16, completionTokens: 120, source: { ttft: 'server', totalWallClock: 'server', tps: 'server' } },
      // A failed sample contributes NOTHING to the metrics (no fake latency).
      { index: 3, status: 'failed', error: 'boom', ttftMs: NOT_MEASURED, totalWallClockMs: NOT_MEASURED, perceivedTps: NOT_MEASURED, interTokenLatencyMs: NOT_MEASURED, completionTokens: NOT_MEASURED, source: { ttft: 'not_measured', totalWallClock: 'not_measured', tps: 'not_measured' } },
    ]
    const aggregate = aggregateSamples(samples)
    expect(aggregate.totalSamples).toBe(4)
    expect(aggregate.okSamples).toBe(3)
    expect(aggregate.failedSamples).toBe(1)
    // TTFT over [100,200,300]: P50 -> ceil(0.5*3)=2 -> 200; mean -> 200.
    expect(aggregate.ttftMs.p50).toBe(200)
    expect(aggregate.ttftMs.mean).toBe(200)
    expect(aggregate.ttftMs.sampleCount).toBe(3)
    // Aggregate throughput = sum of measured TPS = 40+50+60 = 150.
    expect(aggregate.aggregateTps).toBe(150)
  })

  test('aggregate throughput is null (honest absence) when nothing measured TPS', () => {
    const samples: ReadonlyArray<SampleResult> = [
      { index: 0, status: 'failed', error: 'x', ttftMs: NOT_MEASURED, totalWallClockMs: NOT_MEASURED, perceivedTps: NOT_MEASURED, interTokenLatencyMs: NOT_MEASURED, completionTokens: NOT_MEASURED, source: { ttft: 'not_measured', totalWallClock: 'not_measured', tps: 'not_measured' } },
    ]
    expect(aggregateSamples(samples).aggregateTps).toBeNull()
  })
})

describe('gym-oss concurrency runner shape', () => {
  test('runs N samples and preserves index order', async () => {
    const samples = await runConcurrent({
      samples: 5,
      concurrency: 4,
      stream: fakeStream(),
    })
    expect(samples).toHaveLength(5)
    expect(samples.map(s => s.index)).toEqual([0, 1, 2, 3, 4])
  })

  test('clamps concurrency to the hard in-flight cap', () => {
    expect(clampConcurrency(100)).toBe(MAX_IN_FLIGHT)
    expect(clampConcurrency(0)).toBe(1)
    expect(clampConcurrency(4)).toBe(4)
  })

  test('never exceeds the requested concurrency in flight', async () => {
    let inFlight = 0
    let peak = 0
    const observingStream: SampleStream = async ({ index }) => {
      inFlight += 1
      peak = Math.max(peak, inFlight)
      await Promise.resolve()
      await Promise.resolve()
      inFlight -= 1
      return {
        index,
        status: 'ok',
        error: null,
        ttftMs: 100,
        totalWallClockMs: 1000,
        perceivedTps: 50,
        interTokenLatencyMs: 20,
        completionTokens: 200,
        source: { ttft: 'server', totalWallClock: 'server', tps: 'server' },
      }
    }
    await runConcurrent({ samples: 8, concurrency: 2, stream: observingStream })
    expect(peak).toBeLessThanOrEqual(2)
  })
})

describe('gym-oss ramp sweep', () => {
  test('rampSweepSteps yields the 1->2->4->8 prefix up to the clamped top', () => {
    expect(rampSweepSteps(8)).toEqual([1, 2, 4, 8])
    expect(rampSweepSteps(4)).toEqual([1, 2, 4])
    expect(rampSweepSteps(100)).toEqual([1, 2, 4, 8])
    expect(rampSweepSteps(1)).toEqual([1])
  })

  test('runRampSweep produces one aggregated step per concurrency level', async () => {
    const steps = await runRampSweep({
      samples: 3,
      topConcurrency: 4,
      stream: fakeStream(),
    })
    expect(steps.map(s => s.concurrency)).toEqual([1, 2, 4])
    for (const step of steps) {
      expect(step.samples).toHaveLength(3)
      expect(step.aggregate.okSamples).toBe(3)
    }
  })
})

describe('gym-oss run planning', () => {
  test('single run plan reflects the form concurrency', () => {
    const plan = planRun({ ...DEFAULT_FORM, ramp: false, concurrency: 4 })
    expect(plan).toEqual({ kind: 'single', samples: 5, concurrency: 4 })
  })

  test('ramp run plan sweeps the concurrency steps', () => {
    const plan = planRun({ ...DEFAULT_FORM, ramp: true, concurrency: 8 })
    expect(plan).toEqual({ kind: 'ramp', samples: 5, steps: [1, 2, 4, 8] })
  })

  test('executeRun over a fake stream returns a single aggregate', async () => {
    const outcome = await executeRun(
      { kind: 'single', samples: 5, concurrency: 4 },
      fakeStream(),
    )
    expect(outcome.kind).toBe('single')
    if (outcome.kind === 'single') {
      expect(outcome.aggregate.okSamples).toBe(5)
    }
  })

  test('clampSamples enforces a sane, percentile-readable bound', () => {
    expect(clampSamples(0)).toBe(1)
    expect(clampSamples(5)).toBe(5)
    expect(clampSamples(1000)).toBe(64)
  })

  test('concurrency dial options and the lane id are the documented values', () => {
    expect(CONCURRENCY_OPTIONS).toEqual([1, 2, 4, 8])
    expect(GPT_OSS_MODEL_ID).toBe('openagents/khala-oss-20b')
  })
})

describe('gym-oss formatting never shows not_measured as 0', () => {
  test('formatMeasured renders not_measured explicitly, and a real 0 as 0', () => {
    expect(formatMeasured(NOT_MEASURED)).toBe('not measured')
    expect(formatMeasured(NOT_MEASURED, { unit: 'ms' })).toBe('not measured')
    expect(formatMeasured(0)).toBe('0')
    expect(formatMeasured(150, { unit: 'ms' })).toBe('150 ms')
  })

  test('formatSummaryNumber renders null as not measured, never 0', () => {
    expect(formatSummaryNumber(null)).toBe('not measured')
    expect(formatSummaryNumber(0, { unit: 'ms' })).toBe('0 ms')
  })

  test('a failed sample card shows the failure, not a fabricated latency', () => {
    const card = renderSampleCard({
      index: 0,
      status: 'failed',
      error: 'gateway responded 503',
      ttftMs: NOT_MEASURED,
      totalWallClockMs: NOT_MEASURED,
      perceivedTps: NOT_MEASURED,
      interTokenLatencyMs: NOT_MEASURED,
      completionTokens: NOT_MEASURED,
      source: { ttft: 'not_measured', totalWallClock: 'not_measured', tps: 'not_measured' },
    })
    expect(card).toContain('FAILED')
    expect(card).toContain('gateway responded 503')
    // The failure card never prints a "0 ms" latency.
    expect(card).not.toContain('0 ms')
  })

  test('the aggregate table renders not_measured (never 0) for an all-failed set', () => {
    const aggregate = aggregateSamples([
      { index: 0, status: 'failed', error: 'x', ttftMs: NOT_MEASURED, totalWallClockMs: NOT_MEASURED, perceivedTps: NOT_MEASURED, interTokenLatencyMs: NOT_MEASURED, completionTokens: NOT_MEASURED, source: { ttft: 'not_measured', totalWallClock: 'not_measured', tps: 'not_measured' } },
    ])
    const table = renderAggregateTable(aggregate)
    expect(table).toContain('not measured')
    expect(table).toContain('Aggregate throughput: not measured')
  })
})

describe('gym-oss scene mapping (pure visual geometry)', () => {
  test('bar fill is proportional to TPS, normalized to the busiest request', () => {
    const frame: SceneFrame = {
      requests: [
        { index: 0, status: 'ok', perceivedTps: 25 },
        { index: 1, status: 'ok', perceivedTps: 50 },
        { index: 2, status: 'running', perceivedTps: null },
      ],
      aggregateTps: 75,
    }
    const geometry = buildSceneFrame(frame, 75)
    expect(geometry.bars[0]?.fillFraction).toBeCloseTo(0.5)
    expect(geometry.bars[1]?.fillFraction).toBe(1)
    // An unmeasured (running) request is an empty bar, never a fabricated fill.
    expect(geometry.bars[2]?.fillFraction).toBe(0)
    expect(geometry.meterFraction).toBe(1)
  })

  test('sceneFrameForSamples marks unstarted slots as running and sums measured TPS', () => {
    const samples: ReadonlyArray<SampleResult | undefined> = [
      { index: 0, status: 'ok', error: null, ttftMs: 100, totalWallClockMs: 1000, perceivedTps: 40, interTokenLatencyMs: 20, completionTokens: 100, source: { ttft: 'server', totalWallClock: 'server', tps: 'server' } },
      undefined,
    ]
    const frame = sceneFrameForSamples(samples, 2)
    expect(frame.requests[0]?.status).toBe('ok')
    expect(frame.requests[1]?.status).toBe('running')
    expect(frame.aggregateTps).toBe(40)
  })
})
