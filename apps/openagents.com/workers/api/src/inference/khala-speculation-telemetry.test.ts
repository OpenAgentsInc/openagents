// Integration tests for book P1-8 (#6091): speculation acceptance-rate telemetry
// populates from a fixture decode trace, the dynamic-disablement policy turns
// speculation on at low batch and off at high batch through the fixture lane, the
// speculation mode is disclosed in the telemetry record (the receipt-mode
// disclosure), and the real speculative-decoding engine stays flag-gated off (no
// real engine in tests).
import { describe, expect, test } from 'vitest'

import {
  buildKhalaTelemetryRecord,
  isMeasured,
} from './khala-telemetry'
import {
  RealLaneNotArmedError,
  makeFixtureLaneSeam,
  makeRealLaneSeam,
} from './benchmark/lane-seam'
import { fixtureSpeculationForCell } from './benchmark/speculation-lane'
import { runBenchmark } from './benchmark/runner'
import { buildBenchmarkReport } from './benchmark/report'
import { expandMatrix } from './benchmark/matrix'
import type { BenchmarkMatrixConfig } from './benchmark/matrix'
import { TINY_TEST_CONFIG } from './benchmark/fixtures'

// A high-concurrency code config: concurrency 32 is well above the policy's
// profitable batch threshold, so the fixture lane must DISABLE speculation.
const HIGH_BATCH_CODE_CONFIG: BenchmarkMatrixConfig = {
  id: 'high-batch-code-v1',
  description: 'High-concurrency code workload — speculation must be disabled.',
  targets: [{ lane: 'fireworks', engine: 'provider-native' }],
  workloads: ['khala-code-artifact-gen'],
  shapes: [
    {
      id: 'high-batch-shape',
      inputTokens: 1000,
      outputTokens: 200,
      cacheablePrefixTokens: 400,
      concurrency: 32,
      provenance: 'synthetic',
    },
  ],
  transports: ['streaming'],
  sampling: [{ temperature: 0, reasoningEffort: 'off' }],
  samplesPerCell: 3,
}

describe('telemetry record carries the speculation disclosure', () => {
  test('absent speculation input => honest-unknown shape (not_measured), never fabricated', () => {
    const record = buildKhalaTelemetryRecord({
      requestId: 'r1',
      requestedModel: 'openagents/khala-code',
      servedModel: 'fireworks/x',
      route: 'coding',
      provider: 'fireworks',
      requestClass: 'interactive_stream',
      verificationClass: 'none',
      executedVerdict: 'not_executed',
      settlementState: 'not_applicable',
    })
    expect(record.speculation.mode).toBe('not_measured')
    expect(record.speculation.active).toBe(false)
    expect(record.speculation.acceptanceRate).toBe('not_measured')
  })

  test('a disclosed drafting pass populates the acceptance rate on the record', () => {
    const record = buildKhalaTelemetryRecord({
      requestId: 'r2',
      requestedModel: 'openagents/khala-code',
      servedModel: 'fireworks/x',
      route: 'coding',
      provider: 'fireworks',
      requestClass: 'interactive_stream',
      verificationClass: 'none',
      executedVerdict: 'not_executed',
      settlementState: 'not_applicable',
      speculation: {
        mode: 'n_gram',
        draftTokensProposed: 200,
        draftTokensAccepted: 150,
      },
    })
    expect(record.speculation.mode).toBe('n_gram')
    expect(record.speculation.active).toBe(true)
    expect(record.speculation.acceptanceRate).toBe(0.75)
  })
})

describe('fixture decode trace: acceptance-rate telemetry populates honestly', () => {
  test('a low-batch code cell ENABLES speculation and records a measured acceptance rate', () => {
    const seam = makeFixtureLaneSeam()
    const runSet = runBenchmark(TINY_TEST_CONFIG, seam)
    // TINY_TEST_CONFIG is concurrency 1, khala-code-artifact-gen => enabled.
    const executed = runSet.runs.filter(
      run => run.record !== null && run.cell.laneAvailability === 'available',
    )
    expect(executed.length).toBeGreaterThan(0)
    for (const run of executed) {
      const spec = run.record?.speculation
      expect(spec?.mode).toBe('n_gram')
      expect(spec?.active).toBe(true)
      // A real count pair backs the rate (never a bare fabricated number).
      expect(isMeasured(spec?.acceptanceRate ?? 'not_measured')).toBe(true)
      expect(isMeasured(spec?.draftTokensProposed ?? 'not_measured')).toBe(true)
      expect(isMeasured(spec?.draftTokensAccepted ?? 'not_measured')).toBe(true)
    }
  })

  test('a chat cell does NOT request speculation => honest mode `none`, no fabricated rate', () => {
    const seam = makeFixtureLaneSeam()
    // The sample decision suite has a chat workload at concurrency 1.
    const chatCells = expandMatrix({
      id: 'chat-only',
      description: 'chat only',
      targets: [{ lane: 'fireworks', engine: 'provider-native' }],
      workloads: ['chat'],
      shapes: [
        {
          id: 's',
          inputTokens: 200,
          outputTokens: 100,
          cacheablePrefixTokens: 0,
          concurrency: 1,
          provenance: 'synthetic',
        },
      ],
      transports: ['streaming'],
      sampling: [{ temperature: 0.2, reasoningEffort: 'off' }],
      samplesPerCell: 1,
    })
    const sample = seam.sample(chatCells[0]!, 0)
    expect(sample.speculation?.mode).toBe('none')
    expect(sample.speculation?.active).toBe(false)
  })
})

describe('fixture lane applies the dynamic-disablement policy end-to-end', () => {
  test('HIGH batch code cell => speculation disabled, honest mode `none`', () => {
    const cells = expandMatrix(HIGH_BATCH_CODE_CONFIG)
    const codeCell = cells.find(c => c.workload === 'khala-code-artifact-gen')
    expect(codeCell).toBeDefined()
    const outcome = fixtureSpeculationForCell(codeCell!)
    expect(outcome.mode).toBe('none')
    expect(outcome.active).toBe(false)

    // And through the full runner + telemetry: no measured acceptance rate.
    const runSet = runBenchmark(HIGH_BATCH_CODE_CONFIG, makeFixtureLaneSeam())
    for (const run of runSet.runs) {
      if (run.record === null) continue
      expect(run.record.speculation.mode).toBe('none')
      expect(run.record.speculation.acceptanceRate).toBe('not_measured')
    }
  })

  test('LOW batch code cell => speculation enabled with a measured rate (the contrast)', () => {
    const cells = expandMatrix(TINY_TEST_CONFIG)
    const codeCell = cells.find(
      c =>
        c.workload === 'khala-code-artifact-gen' &&
        c.laneAvailability === 'available',
    )
    expect(codeCell).toBeDefined()
    const outcome = fixtureSpeculationForCell(codeCell!)
    expect(outcome.active).toBe(true)
    expect(outcome.mode).toBe('n_gram')
    expect(typeof outcome.draftTokensProposed).toBe('number')
    expect(typeof outcome.draftTokensAccepted).toBe('number')
  })
})

describe('report discloses acceptance per (workload x model x temperature x route)', () => {
  test('the report carries a speculation-acceptance aggregate with the four keying axes', () => {
    const runSet = runBenchmark(TINY_TEST_CONFIG, makeFixtureLaneSeam())
    const report = buildBenchmarkReport(runSet)
    expect(report.speculationAcceptance.length).toBeGreaterThan(0)
    const cell = report.speculationAcceptance.find(
      c => c.workload === 'khala-code-artifact-gen',
    )
    expect(cell).toBeDefined()
    // The four axes are present.
    expect(cell?.workload).toBe('khala-code-artifact-gen')
    expect(typeof cell?.model).toBe('string')
    expect(typeof cell?.temperature).toBe('number')
    expect(cell?.route).toBe('khala-code-artifact-gen')
    // A low-batch code cell ran speculation => a measured aggregate rate + mode.
    expect(cell?.mode).toBe('n_gram')
    expect(cell?.acceptanceRate).not.toBeNull()
    expect(cell?.measuredRuns).toBeGreaterThan(0)
  })

  test('a high-batch report records a null acceptance rate (honest absence, not 0)', () => {
    const runSet = runBenchmark(HIGH_BATCH_CODE_CONFIG, makeFixtureLaneSeam())
    const report = buildBenchmarkReport(runSet)
    const cell = report.speculationAcceptance.find(
      c => c.workload === 'khala-code-artifact-gen',
    )
    expect(cell).toBeDefined()
    expect(cell?.mode).toBe('none')
    expect(cell?.acceptanceRate).toBeNull()
    expect(cell?.measuredRuns).toBe(0)
  })
})

describe('real speculative-decoding engine stays flag-gated OFF (no real engine in tests)', () => {
  test('the fixture seam never spends and never runs a real draft model', () => {
    const seam = makeFixtureLaneSeam()
    expect(seam.canSpend).toBe(false)
  })

  test('an un-armed real lane refuses to run (no real engine reachable from a test)', () => {
    const realSeam = makeRealLaneSeam({ armRealSweep: false })
    expect(realSeam.canSpend).toBe(false)
    const cells = expandMatrix(TINY_TEST_CONFIG)
    expect(() => realSeam.sample(cells[0]!, 0)).toThrow(RealLaneNotArmedError)
  })
})
