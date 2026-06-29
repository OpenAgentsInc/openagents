import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildBenchmarkReport,
  type BenchmarkCell,
  type BenchmarkLaneSample,
  expandMatrix,
  RealLaneNotArmedError,
} from '../benchmark'
import {
  OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  type GymExperiment,
} from './experiment'
import {
  buildGymPaidRunReportReceipt,
  makeGymPaidRunRealSeam,
  meterGymPaidRunSet,
  prepareGymPaidRun,
  runPreparedGymPaidRun,
} from './paid-run'

const REALISTIC_OPENCODE_SHAPE = {
  id: 'observed-opencode-edit-run',
  inputTokens: 1800,
  outputTokens: 700,
  cacheablePrefixTokens: 900,
  concurrency: 1,
  provenance: 'realistic',
} as const

const PAID_OPENCODE_EXPERIMENT: GymExperiment = {
  ...OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT,
  id: 'gym-opencode-khala-vs-bigpickle-paid-test-v1',
  shapes: [REALISTIC_OPENCODE_SHAPE],
  budget: {
    spendCapMsat: 100_000_000,
    maxBillableSamples: 10,
    seam: 'real',
    ownerApprovalRef: 'approval.public.gym.opencode.test',
  },
}

const REAL_TRAFFIC_EVIDENCE = [
  {
    shapeId: REALISTIC_OPENCODE_SHAPE.id,
    evidenceRef: 'receipt.public.khala_traffic_shape.opencode_edit_run',
    observedRequestCount: 12,
    source: 'gateway_telemetry',
    publicSafe: true,
  },
] as const

const realOpenCodeExecutor = (
  cell: BenchmarkCell,
  sampleIndex: number,
): BenchmarkLaneSample => {
  const promptTokens = cell.shape.inputTokens
  const completionTokens = cell.shape.outputTokens
  const generationWallClockMs =
    (cell.lane === 'bigpickle' ? 9 : 7) * completionTokens + sampleIndex
  const ttftMs = (cell.lane === 'bigpickle' ? 310 : 240) + sampleIndex
  const providerTimeMs = ttftMs + generationWallClockMs
  return {
    promptTokens,
    completionTokens,
    totalTokens: promptTokens + completionTokens,
    cachedInputTokens: Math.floor(cell.shape.cacheablePrefixTokens * 0.75),
    ttftMs,
    totalWallClockMs: providerTimeMs + 35,
    generationWallClockMs,
    providerTimeMs,
    gatewayOverheadMs: 35,
    verificationClass: 'test_passed',
    executedVerdict: 'passed',
    scalarReward: 1,
    verifierTimeMs: 1200,
    costBasisMsat: cell.lane === 'bigpickle' ? 640 : 520,
    region: cell.lane === 'bigpickle' ? 'opencode' : 'openagents',
    clientSurface: {
      client: 'opencode',
      taskRef: 'gym.realistic.opencode.edit-run-smoke.v1',
      configRef: `opencode.real.${cell.lane}.v1`,
      toolCallsAttempted: 3,
      toolCallsSucceeded: 3,
    },
  }
}

describe('Gym paid run plan', () => {
  test('returns a 402-style balance gate and refuses to arm the seam when unpaid', () => {
    const plan = prepareGymPaidRun({
      accountRef: 'agent:test-underfunded',
      experiment: PAID_OPENCODE_EXPERIMENT,
      availableBalanceMsat: 0,
      fundingKind: 'card',
      ownerConfirmed: true,
      trafficEvidence: REAL_TRAFFIC_EVIDENCE,
      executableFixtureOnlyLanes: ['bigpickle'],
    })

    expect(plan.balanceGate.status).toBe('payment_required')
    expect(plan.balanceGate.statusCode).toBe(402)
    expect(plan.blockers.map(blocker => blocker.code)).toContain(
      'insufficient_balance',
    )
    expect(plan.canStartRealRun).toBe(false)

    const seam = makeGymPaidRunRealSeam(plan, realOpenCodeExecutor)
    const firstCell = expandMatrix(plan.compiled.matrixConfig)[0]
    expect(() => seam.sample(firstCell!, 0)).toThrow(RealLaneNotArmedError)
  })

  test('keeps fixture-only competitors skipped unless the paid executor explicitly covers them', () => {
    const plan = prepareGymPaidRun({
      accountRef: 'agent:test-funded',
      experiment: PAID_OPENCODE_EXPERIMENT,
      availableBalanceMsat: 100_000_000,
      fundingKind: 'card',
      ownerConfirmed: true,
      trafficEvidence: REAL_TRAFFIC_EVIDENCE,
    })

    expect(plan.canStartRealRun).toBe(true)
    expect(plan.quote.lines.map(line => line.lane)).toEqual(['khala'])
    expect(plan.preflight.skippedFutureCells).toBe(1)

    const runSet = runPreparedGymPaidRun(plan, realOpenCodeExecutor)
    expect(runSet.cellsExecuted).toBe(1)
    expect(runSet.cellsSkipped).toBe(1)
  })

  test('runs a funded owner-armed Khala vs BigPickle sweep through metering and a decision-grade receipt', () => {
    const plan = prepareGymPaidRun({
      accountRef: 'agent:test-funded',
      experiment: PAID_OPENCODE_EXPERIMENT,
      availableBalanceMsat: 100_000_000,
      fundingKind: 'card',
      ownerConfirmed: true,
      trafficEvidence: REAL_TRAFFIC_EVIDENCE,
      executableFixtureOnlyLanes: ['bigpickle'],
    })

    expect(plan.canStartRealRun).toBe(true)
    expect(plan.decisionGradeEligible).toBe(true)
    expect(plan.quote.lines.map(line => line.lane)).toEqual([
      'khala',
      'bigpickle',
    ])

    const runSet = runPreparedGymPaidRun(plan, realOpenCodeExecutor)
    const report = buildBenchmarkReport(runSet)
    expect(report.decisionGrade).toBe(true)
    expect(runSet.cellsExecuted).toBe(2)
    expect(runSet.cellsSkipped).toBe(0)

    const meteringOutcomes = Effect.runSync(
      meterGymPaidRunSet(plan, runSet, context =>
        Effect.succeed({
          metered: true,
          receiptRef: `receipt.inference.charge.${context.requestId}`,
        }),
      ),
    )
    const receipt = buildGymPaidRunReportReceipt({
      plan,
      runSet,
      report,
      meteringOutcomes,
    })

    expect(meteringOutcomes).toHaveLength(10)
    expect(receipt.decisionGrade).toBe(true)
    expect(receipt.meteringReceiptRefs).toHaveLength(10)
    expect(receipt.realTrafficEvidenceRefs).toEqual([
      'receipt.public.khala_traffic_shape.opencode_edit_run',
    ])
    expect(JSON.stringify(receipt)).not.toContain('agent:test-funded')
  })
})
