import { describe, expect, test } from 'vitest'

import { NOT_MEASURED } from '../khala-telemetry'
import {
  buildGymThroughputReport,
  decodeGymThroughputEnvironmentSpec,
  decodeGymThroughputSample,
  type GymThroughputEnvironmentSpec,
  type GymThroughputSample,
} from './throughput'

const spec = (
  overrides: Partial<GymThroughputEnvironmentSpec> = {},
): GymThroughputEnvironmentSpec => ({
  schemaVersion: 'openagents.gym.throughput_environment.v1',
  environmentRef: 'throughput-concurrency',
  target: {
    lane: 'gpt-oss-20b',
    engine: 'vllm',
    modelRef: 'openagents/khala-oss-20b',
  },
  promptProfile: 'gym-oss-short-code-prompt',
  concurrencyRamp: [1, 2, 4],
  samplesPerConcurrency: 2,
  degradationThresholdMultiplier: 1.5,
  serving: {
    speculationMode: 'n_gram',
  },
  ...overrides,
})

const sample = (
  concurrency: number,
  sampleIndex: number,
  overrides: Partial<GymThroughputSample> = {},
): GymThroughputSample => ({
  schemaVersion: 'openagents.gym.throughput_sample.v1',
  lane: 'gpt-oss-20b',
  engine: 'vllm',
  modelRef: 'openagents/khala-oss-20b',
  concurrency,
  sampleIndex,
  status: 'ok',
  ttftMs: 100 + concurrency,
  totalWallClockMs: 1000,
  perceivedTps: 50,
  interTokenLatencyMs: 20,
  completionTokens: 200,
  speculationMode: 'n_gram',
  speculationAcceptanceRate: 0.7,
  ...overrides,
})

describe('Gym throughput/concurrency report (#6244)', () => {
  test('builds a repeatable lane report and detects latency degradation', () => {
    const input = {
      generatedAt: '2026-06-25T12:00:00.000Z',
      specs: [spec()],
      samples: [
        sample(1, 0, { totalWallClockMs: 1000, perceivedTps: 50 }),
        sample(1, 1, { totalWallClockMs: 1100, perceivedTps: 52 }),
        sample(2, 0, { totalWallClockMs: 1250, perceivedTps: 55 }),
        sample(2, 1, { totalWallClockMs: 1300, perceivedTps: 56 }),
        sample(4, 0, { totalWallClockMs: 1900, perceivedTps: 57 }),
        sample(4, 1, {
          totalWallClockMs: 2200,
          perceivedTps: 58,
          speculationAcceptanceRate: 0.8,
        }),
      ],
    }
    const report = buildGymThroughputReport(input)
    const reportAgain = buildGymThroughputReport(input)

    expect(report.schemaVersion).toBe(
      'openagents.gym.throughput_concurrency_report.v1',
    )
    expect(JSON.stringify(report)).toBe(JSON.stringify(reportAgain))
    const lane = report.lanes[0]
    expect(lane?.degradation).toEqual({
      concurrency: 4,
      reason: 'latency_degraded',
    })
    expect(lane?.concurrencyPoints.map(point => point.concurrency)).toEqual([
      1, 2, 4,
    ])
    expect(lane?.concurrencyPoints[0]?.totalWallClockMs.p90).toBe(1100)
    expect(lane?.concurrencyPoints[0]?.aggregateTps).toBe(102)
    expect(
      lane?.concurrencyPoints[2]?.speculationAcceptanceRate.sampleCount,
    ).toBe(2)
    expect(lane?.concurrencyPoints[2]?.speculationAcceptanceRate.p50).toBe(0.7)
    expect(lane?.speculationMode).toBe('n_gram')
  })

  test('reports quota-limited concurrency before latency degradation', () => {
    const report = buildGymThroughputReport({
      generatedAt: '2026-06-25T12:00:00.000Z',
      specs: [spec()],
      samples: [
        sample(1, 0),
        sample(2, 0, {
          status: 'quota_limited',
          errorClass: 'provider_quota_exceeded',
          ttftMs: NOT_MEASURED,
          totalWallClockMs: NOT_MEASURED,
          perceivedTps: NOT_MEASURED,
          interTokenLatencyMs: NOT_MEASURED,
          completionTokens: NOT_MEASURED,
          speculationAcceptanceRate: NOT_MEASURED,
        }),
        sample(4, 0, { totalWallClockMs: 4000 }),
      ],
    })

    const lane = report.lanes[0]
    expect(lane?.degradation).toEqual({
      concurrency: 2,
      reason: 'quota_limited',
    })
    expect(lane?.concurrencyPoints[1]?.quotaLimitedSamples).toBe(1)
    expect(lane?.concurrencyPoints[1]?.totalWallClockMs.sampleCount).toBe(0)
  })

  test('keeps measured zero distinct from not_measured', () => {
    const report = buildGymThroughputReport({
      generatedAt: '2026-06-25T12:00:00.000Z',
      specs: [spec({ concurrencyRamp: [1], samplesPerConcurrency: 2 })],
      samples: [
        sample(1, 0, {
          ttftMs: 0,
          perceivedTps: 0,
          interTokenLatencyMs: NOT_MEASURED,
          speculationAcceptanceRate: NOT_MEASURED,
        }),
        sample(1, 1, {
          ttftMs: NOT_MEASURED,
          perceivedTps: NOT_MEASURED,
          interTokenLatencyMs: NOT_MEASURED,
          speculationAcceptanceRate: NOT_MEASURED,
        }),
      ],
    })

    const point = report.lanes[0]?.concurrencyPoints[0]
    expect(point?.ttftMs.p50).toBe(0)
    expect(point?.ttftMs.sampleCount).toBe(1)
    expect(point?.perceivedTps.p50).toBe(0)
    expect(point?.perceivedTps.sampleCount).toBe(1)
    expect(point?.interTokenLatencyMs.p50).toBeNull()
    expect(point?.interTokenLatencyMs.sampleCount).toBe(0)
    expect(point?.speculationAcceptanceRate.p50).toBeNull()
  })

  test('decodes environment specs and samples at the schema boundary', () => {
    expect(decodeGymThroughputEnvironmentSpec(spec()).environmentRef).toBe(
      'throughput-concurrency',
    )
    expect(decodeGymThroughputSample(sample(1, 0)).ttftMs).toBe(101)
    expect(() =>
      decodeGymThroughputSample({
        ...sample(1, 0),
        status: 'not_a_status',
      }),
    ).toThrow()
  })
})
