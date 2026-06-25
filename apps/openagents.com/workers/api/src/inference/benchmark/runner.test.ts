import { describe, expect, test } from 'vitest'

import { isMeasured } from '../khala-telemetry'
import {
  OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG,
  SAMPLE_DECISION_SUITE_CONFIG,
  TINY_TEST_CONFIG,
} from './fixtures'
import { makeFixtureLaneSeam, makeRealLaneSeam } from './lane-seam'
import { runBenchmark } from './runner'

describe('benchmark runner — fixture lane', () => {
  test('produces samplesPerCell runs per executable cell, one skipped run per future lane', () => {
    const runSet = runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam())
    // 2 cells expanded (fireworks available, pylon not-yet-available).
    expect(runSet.cellsExpanded).toBe(2)
    expect(runSet.cellsExecuted).toBe(1)
    expect(runSet.cellsSkipped).toBe(1)
    // fireworks: 4 samples; pylon: 1 skipped run.
    expect(runSet.runs.length).toBe(5)
    const executed = runSet.runs.filter(r => r.record !== null)
    const skipped = runSet.runs.filter(r => r.record === null)
    expect(executed.length).toBe(4)
    expect(skipped.length).toBe(1)
    expect(skipped[0]?.skippedReason).toBe(
      'lane_not_yet_available:pylon-whole-small',
    )
  })

  test('the run is deterministic (same config + seam → identical run set)', () => {
    const seam = makeFixtureLaneSeam()
    const a = runBenchmark(TINY_TEST_CONFIG, seam)
    const b = runBenchmark(TINY_TEST_CONFIG, seam)
    expect(a).toEqual(b)
  })

  test('each executed run carries a canonical telemetry record with the P0-1 fields', () => {
    const runSet = runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam())
    const run = runSet.runs.find(r => r.record !== null)
    expect(run).toBeDefined()
    const record = run!.record!
    expect(record.schemaVersion).toBe('openagents.khala.telemetry.v1')
    // Token, latency, verification, and cost fields are all MEASURED.
    expect(isMeasured(record.promptTokens)).toBe(true)
    expect(isMeasured(record.completionTokens)).toBe(true)
    expect(isMeasured(record.ttftMs)).toBe(true)
    expect(isMeasured(record.totalWallClockMs)).toBe(true)
    expect(isMeasured(record.perceivedTps)).toBe(true)
    expect(isMeasured(record.costBasisMsat)).toBe(true)
    expect(record.executedVerdict).toBe('passed')
    expect(record.provider).toBe('fireworks')
    expect(record.route).toBe('khala-code-artifact-gen')
    // Reproducible, non-random request id.
    expect(record.requestId).toBe(
      'bench:tiny-test-v1:fireworks|provider-native|khala-code-artifact-gen|tiny-shape|streaming|t0|roff:s0',
    )
  })

  test('a verifier-run cell is classed as verifier_run; chat as interactive_stream; batch as batch', () => {
    const runSet = runBenchmark(SAMPLE_DECISION_SUITE_CONFIG, makeFixtureLaneSeam())
    const executed = runSet.runs.filter(r => r.record !== null)
    const verifier = executed.find(
      r => r.cell.workload === 'verifier-run' && r.cell.transport === 'streaming',
    )
    const chat = executed.find(
      r => r.cell.workload === 'chat' && r.cell.transport === 'streaming',
    )
    const batch = executed.find(r => r.cell.transport === 'batch')
    expect(verifier!.record!.requestClass).toBe('verifier_run')
    expect(chat!.record!.requestClass).toBe('interactive_stream')
    expect(batch!.record!.requestClass).toBe('batch')
  })

  test('future lanes are NEVER executed against the seam (no fabricated numbers)', () => {
    // A recording seam: the inner fixture runs available lanes, but it records
    // every lane it was asked to sample, so we can assert it was NEVER called for
    // a not-yet-available lane.
    const inner = makeFixtureLaneSeam()
    const sampledLanes = new Set<string>()
    const recordingSeam = {
      id: 'recording',
      canSpend: false,
      sample: (cell: Parameters<typeof inner.sample>[0], index: number) => {
        sampledLanes.add(cell.lane)
        return inner.sample(cell, index)
      },
    }
    const runSet = runBenchmark(SAMPLE_DECISION_SUITE_CONFIG, recordingSeam)
    const futureRuns = runSet.runs.filter(
      r => r.cell.laneAvailability === 'not_yet_available',
    )
    expect(futureRuns.length).toBeGreaterThan(0)
    expect(futureRuns.every(r => r.record === null)).toBe(true)
    // The seam was asked to sample the available lanes, but NEVER a future lane.
    expect(sampledLanes.has('fireworks')).toBe(true)
    expect(sampledLanes.has('pylon-whole-small')).toBe(false)
    expect(sampledLanes.has('psionic-shard-wan')).toBe(false)
  })

  test('fixture-only OpenCode competitor lanes execute in fixture but skip in a real seam', () => {
    const fixtureRunSet = runBenchmark(
      OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG,
      makeFixtureLaneSeam(),
    )
    const fixtureBigpickle = fixtureRunSet.runs.filter(
      run => run.cell.lane === 'bigpickle',
    )
    expect(fixtureBigpickle.length).toBe(5)
    expect(fixtureBigpickle.every(run => run.record !== null)).toBe(true)
    expect(
      fixtureBigpickle.every(run => run.clientSurface?.client === 'opencode'),
    ).toBe(true)

    const realRunSet = runBenchmark(
      OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG,
      makeRealLaneSeam({
        armRealSweep: true,
        executor: (cell, sampleIndex) =>
          makeFixtureLaneSeam().sample(cell, sampleIndex),
      }),
    )
    const realBigpickle = realRunSet.runs.filter(
      run => run.cell.lane === 'bigpickle',
    )
    expect(realBigpickle).toHaveLength(1)
    expect(realBigpickle[0]?.record).toBeNull()
    expect(realBigpickle[0]?.skippedReason).toBe('lane_fixture_only:bigpickle')
  })
})
