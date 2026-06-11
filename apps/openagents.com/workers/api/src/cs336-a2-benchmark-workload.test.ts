import { describe, expect, it } from 'vitest'

import {
  aggregateCs336A2Samples,
  cs336A2CrossDeviceAgreementScore,
  cs336A2ModeledSatsPerHour,
  runCs336A2BenchmarkSuite,
} from './cs336-a2-benchmark-workload'
import { Cs336A2BenchmarkMeasurements } from './training-device-capability'

const makeCountingClock = (stepMs: number): (() => number) => {
  let ticks = 0

  return () => {
    ticks += 1

    return ticks * stepMs
  }
}

describe('CS336 A2 benchmark workload', () => {
  it('runs every measurement kind per repetition with finite positive values and stable output digests', async () => {
    const first = await runCs336A2BenchmarkSuite({
      now: makeCountingClock(5),
      repetitions: 2,
    })
    const second = await runCs336A2BenchmarkSuite({
      now: makeCountingClock(5),
      repetitions: 1,
    })

    expect(first.benchmarkSuiteRef).toBe(
      'benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1',
    )
    expect(first.samples.length).toBe(2 * Cs336A2BenchmarkMeasurements.length)
    expect(first.suiteElapsedMs).toBeGreaterThan(0)

    for (const metric of Cs336A2BenchmarkMeasurements) {
      const samples = first.samples.filter(sample => sample.metric === metric)

      expect(samples.length).toBe(2)

      for (const sample of samples) {
        expect(Number.isFinite(sample.value)).toBe(true)
        expect(sample.value).toBeGreaterThan(0)
        expect(sample.outputDigestHex).toMatch(/^[0-9a-f]{64}$/)
      }

      const replaySample = second.samples.find(
        sample => sample.metric === metric,
      )

      expect(replaySample?.outputDigestHex).toBe(samples[0]!.outputDigestHex)
    }
  })

  it('aggregates samples into honest nearest-rank class distributions', () => {
    const samples = [10, 30, 20, 50, 40, 60].map(value => ({
      elapsedMs: 1,
      metric: 'tokens_per_second' as const,
      outputDigestHex: 'a'.repeat(64),
      unit: 'tokens_per_second',
      value,
    }))
    const aggregates = aggregateCs336A2Samples(samples)

    expect(aggregates).toEqual([
      {
        max: 60,
        metric: 'tokens_per_second',
        min: 10,
        p50: 30,
        p90: 60,
        sampleCount: 6,
        unit: 'tokens_per_second',
      },
    ])
  })

  it('scores cross-device agreement as min over max median and refuses degenerate inputs', () => {
    expect(cs336A2CrossDeviceAgreementScore([100, 80])).toBeCloseTo(0.8)
    expect(cs336A2CrossDeviceAgreementScore([100, 100])).toBe(1)
    expect(cs336A2CrossDeviceAgreementScore([100])).toBe(0)
    expect(cs336A2CrossDeviceAgreementScore([100, 0])).toBe(0)
    expect(cs336A2CrossDeviceAgreementScore([100, Number.NaN])).toBe(0)
  })

  it('models sats per hour only from positive measured time and pay', () => {
    expect(
      cs336A2ModeledSatsPerHour({ paidSats: 30, suiteElapsedMs: 60_000 }),
    ).toBe(1800)
    expect(
      cs336A2ModeledSatsPerHour({ paidSats: 0, suiteElapsedMs: 60_000 }),
    ).toBeNull()
    expect(cs336A2ModeledSatsPerHour({ paidSats: 30, suiteElapsedMs: 0 })).toBe(
      null,
    )
  })
})
