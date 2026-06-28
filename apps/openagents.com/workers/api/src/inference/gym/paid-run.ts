import { Effect } from 'effect'

import {
  buildBenchmarkReport,
  type BenchmarkLane,
  type BenchmarkLaneSample,
  type BenchmarkLaneSeam,
  type BenchmarkRunSet,
  expandMatrix,
  makeRealLaneSeam,
  modelIdForBenchmarkCell,
  preflightRealBenchmarkSweep,
  runBenchmark,
  type RealSweepBlocker,
  type RealSweepBlockerCode,
  type RealSweepPreflight,
  type RealTrafficShapeEvidence,
} from '../benchmark'
import { isMeasured } from '../khala-telemetry'
import type {
  MeteringContext,
  MeteringHook,
  MeteringOutcome,
} from '../metering-hook'
import type { FundingKind } from '../pricing'
import { priceRequest } from '../pricing'
import type { InferenceUsage } from '../provider-adapter'
import { usdToMsatCeil } from '../usd-msat-conversion'
import {
  compileGymExperiment,
  type CompiledGymExperiment,
  type GymEnvironmentRegistry,
  type GymExperiment,
} from './experiment'

export type GymPaidRunBlockerCode =
  | RealSweepBlockerCode
  | 'real_seam_required'
  | 'quoted_cost_exceeds_budget_cap'
  | 'insufficient_balance'

export type GymPaidRunBlocker = Readonly<{
  code: GymPaidRunBlockerCode
  message: string
}>

export type GymPaidRunQuoteLine = Readonly<{
  cellId: string
  lane: BenchmarkLane
  laneAvailability: 'available' | 'fixture_only' | 'not_yet_available'
  workload: string
  model: string
  samples: number
  quotedMsat: number
}>

export type GymPaidRunQuote = Readonly<{
  schemaVersion: 'openagents.gym.paid-run-quote.v1'
  quoteRef: string
  configId: string
  fundingKind: FundingKind
  quotedMsat: number
  billableSampleUpperBound: number
  executableCells: number
  executableFixtureOnlyLanes: ReadonlyArray<BenchmarkLane>
  lines: ReadonlyArray<GymPaidRunQuoteLine>
}>

export type GymPaidRunBalanceGate = Readonly<
  | {
      status: 'passed'
      statusCode: null
      availableBalanceMsat: number
      requiredMsat: number
      challengeRef: null
    }
  | {
      status: 'payment_required'
      statusCode: 402
      availableBalanceMsat: number
      requiredMsat: number
      challengeRef: string
    }
>

export type GymPaidRunPlan = Readonly<{
  schemaVersion: 'openagents.gym.paid-run-plan.v1'
  accountRef: string
  compiled: CompiledGymExperiment
  quote: GymPaidRunQuote
  preflight: RealSweepPreflight
  balanceGate: GymPaidRunBalanceGate
  canStartRealRun: boolean
  decisionGradeEligible: boolean
  blockers: ReadonlyArray<GymPaidRunBlocker>
}>

export type PrepareGymPaidRunInput = Readonly<{
  accountRef: string
  experiment: GymExperiment
  availableBalanceMsat: number
  fundingKind: FundingKind
  ownerConfirmed: boolean
  trafficEvidence?: ReadonlyArray<RealTrafficShapeEvidence> | undefined
  executableFixtureOnlyLanes?: ReadonlyArray<BenchmarkLane> | undefined
  registry?: GymEnvironmentRegistry | undefined
}>

export type GymPaidRunReportReceipt = Readonly<{
  schemaVersion: 'openagents.gym.paid-run-report-receipt.v1'
  receiptRef: string
  quoteRef: string
  reportRef: string
  reportDigestRef: string
  configId: string
  ownerApprovalRef: string
  decisionGrade: boolean
  quoteMsat: number
  billableSampleUpperBound: number
  executedSamples: number
  skippedSamples: number
  realTrafficEvidenceRefs: ReadonlyArray<string>
  meteringReceiptRefs: ReadonlyArray<string>
}>

export type GymPaidRunExecutor = (
  cell: Parameters<BenchmarkLaneSeam['sample']>[0],
  sampleIndex: number,
) => BenchmarkLaneSample

const samplesForCell = (samplesPerCell: number): number =>
  Math.max(1, Math.floor(samplesPerCell))

const safeRefSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, '_')
    .replace(/^_+|_+$/g, '')

const fnv1a32 = (value: string): string => {
  let hash = 0x811c9dc5
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash.toString(16).padStart(8, '0')
}

const stableJson = (value: unknown): string => {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value) ?? 'null'
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`
  }
  const entries = Object.entries(value as Record<string, unknown>).sort(
    ([a], [b]) => a.localeCompare(b),
  )
  return `{${entries
    .map(([key, entry]) => `${JSON.stringify(key)}:${stableJson(entry)}`)
    .join(',')}}`
}

const quoteRefForConfig = (configId: string): string =>
  `quote.gym.paid_run.${safeRefSegment(configId)}`

const paymentChallengeRefForQuote = (quoteRef: string): string =>
  `challenge.gym.paid_run.${safeRefSegment(quoteRef)}`

const executableFixtureOnlySet = (
  lanes: ReadonlyArray<BenchmarkLane> | undefined,
): ReadonlySet<BenchmarkLane> => new Set(lanes ?? [])

const isExecutableForPaidRun = (
  laneAvailability: GymPaidRunQuoteLine['laneAvailability'],
  lane: BenchmarkLane,
  executableFixtureOnlyLanes: ReadonlySet<BenchmarkLane>,
): boolean =>
  laneAvailability === 'available' ||
  (laneAvailability === 'fixture_only' &&
    executableFixtureOnlyLanes.has(lane))

const quoteUsageForCell = (
  cell: ReturnType<typeof expandMatrix>[number],
): InferenceUsage => ({
  promptTokens: cell.shape.inputTokens,
  completionTokens: cell.shape.outputTokens,
  totalTokens: cell.shape.inputTokens + cell.shape.outputTokens,
  cachedPromptTokens: cell.shape.cacheablePrefixTokens,
})

export const quoteGymPaidRun = (
  compiled: CompiledGymExperiment,
  input: Readonly<{
    fundingKind: FundingKind
    executableFixtureOnlyLanes?: ReadonlyArray<BenchmarkLane> | undefined
  }>,
): GymPaidRunQuote => {
  const executableFixtureOnlyLanes = executableFixtureOnlySet(
    input.executableFixtureOnlyLanes,
  )
  const lines: Array<GymPaidRunQuoteLine> = []
  let quotedMsat = 0
  let billableSampleUpperBound = 0

  for (const cell of expandMatrix(compiled.matrixConfig)) {
    if (
      !isExecutableForPaidRun(
        cell.laneAvailability,
        cell.lane,
        executableFixtureOnlyLanes,
      )
    ) {
      continue
    }

    const samples = samplesForCell(cell.samplesPerCell)
    const price = priceRequest({
      model: modelIdForBenchmarkCell(cell),
      usage: quoteUsageForCell(cell),
      fundingKind: input.fundingKind,
      batch: cell.transport === 'batch',
    })
    const lineQuoteMsat = usdToMsatCeil(price.chargeUsd) * samples
    quotedMsat += lineQuoteMsat
    billableSampleUpperBound += samples
    lines.push({
      cellId: cell.cellId,
      lane: cell.lane,
      laneAvailability: cell.laneAvailability,
      workload: cell.workload,
      model: price.model,
      samples,
      quotedMsat: lineQuoteMsat,
    })
  }

  return {
    schemaVersion: 'openagents.gym.paid-run-quote.v1',
    quoteRef: quoteRefForConfig(compiled.matrixConfig.id),
    configId: compiled.matrixConfig.id,
    fundingKind: input.fundingKind,
    quotedMsat,
    billableSampleUpperBound,
    executableCells: lines.length,
    executableFixtureOnlyLanes: [...executableFixtureOnlyLanes].sort(),
    lines,
  }
}

const balanceGateForQuote = (
  quote: GymPaidRunQuote,
  availableBalanceMsat: number,
): GymPaidRunBalanceGate =>
  availableBalanceMsat >= quote.quotedMsat
    ? {
        status: 'passed',
        statusCode: null,
        availableBalanceMsat,
        requiredMsat: quote.quotedMsat,
        challengeRef: null,
      }
    : {
        status: 'payment_required',
        statusCode: 402,
        availableBalanceMsat,
        requiredMsat: quote.quotedMsat,
        challengeRef: paymentChallengeRefForQuote(quote.quoteRef),
      }

const paidRunBlockerFromPreflight = (
  blocker: RealSweepBlocker,
): GymPaidRunBlocker => ({
  code: blocker.code,
  message: blocker.message,
})

export const prepareGymPaidRun = (
  input: PrepareGymPaidRunInput,
): GymPaidRunPlan => {
  const compiled = compileGymExperiment(input.experiment, input.registry)
  const executableFixtureOnlyLanes = input.executableFixtureOnlyLanes ?? []
  const quote = quoteGymPaidRun(compiled, {
    fundingKind: input.fundingKind,
    executableFixtureOnlyLanes,
  })
  const preflight = preflightRealBenchmarkSweep(compiled.matrixConfig, {
    ownerConfirmed: input.ownerConfirmed,
    ownerApprovalRef: input.experiment.budget.ownerApprovalRef,
    budgetCapMsat: input.experiment.budget.spendCapMsat,
    maxBillableSamples: input.experiment.budget.maxBillableSamples,
    trafficEvidence: input.trafficEvidence,
    executableFixtureOnlyLanes,
    billableLanes: quote.lines.map(line => line.lane),
  })
  const balanceGate = balanceGateForQuote(quote, input.availableBalanceMsat)
  const blockers: Array<GymPaidRunBlocker> = [
    ...preflight.blockers.map(paidRunBlockerFromPreflight),
  ]

  if (input.experiment.budget.seam !== 'real') {
    blockers.push({
      code: 'real_seam_required',
      message: 'Paid Gym runs must set budget.seam to real.',
    })
  }

  if (
    preflight.budgetCapMsat !== null &&
    quote.quotedMsat > preflight.budgetCapMsat
  ) {
    blockers.push({
      code: 'quoted_cost_exceeds_budget_cap',
      message:
        'The upfront Gym quote exceeds the owner-approved msat budget cap.',
    })
  }

  if (balanceGate.status === 'payment_required') {
    blockers.push({
      code: 'insufficient_balance',
      message:
        'The account balance is below the upfront Gym quote; return HTTP 402 before running.',
    })
  }

  const canStartRealRun = blockers.length === 0
  return {
    schemaVersion: 'openagents.gym.paid-run-plan.v1',
    accountRef: input.accountRef,
    compiled,
    quote,
    preflight,
    balanceGate,
    canStartRealRun,
    decisionGradeEligible: canStartRealRun && preflight.decisionGradeEligible,
    blockers,
  }
}

export const makeGymPaidRunRealSeam = (
  plan: GymPaidRunPlan,
  executor: GymPaidRunExecutor,
): BenchmarkLaneSeam =>
  makeRealLaneSeam({
    armRealSweep: plan.canStartRealRun,
    executor,
    executableFixtureOnlyLanes: plan.quote.executableFixtureOnlyLanes,
  })

export const runPreparedGymPaidRun = (
  plan: GymPaidRunPlan,
  executor: GymPaidRunExecutor,
): BenchmarkRunSet =>
  runBenchmark(
    plan.compiled.matrixConfig,
    makeGymPaidRunRealSeam(plan, executor),
  )

export const buildGymPaidRunMeteringContexts = (
  plan: GymPaidRunPlan,
  runSet: BenchmarkRunSet,
): ReadonlyArray<MeteringContext> => {
  const contexts: Array<MeteringContext> = []
  for (const run of runSet.runs) {
    const record = run.record
    if (record === null) {
      continue
    }
    if (
      !isMeasured(record.promptTokens) ||
      !isMeasured(record.completionTokens) ||
      !isMeasured(record.totalTokens)
    ) {
      continue
    }
    contexts.push({
      accountRef: plan.accountRef,
      requestedModel: record.requestedModel,
      servedModel: record.servedModel,
      adapterId: record.provider,
      usage: {
        promptTokens: record.promptTokens,
        completionTokens: record.completionTokens,
        totalTokens: record.totalTokens,
        ...(isMeasured(record.cachedInputTokens)
          ? { cachedPromptTokens: record.cachedInputTokens }
          : {}),
      },
      streamed: record.requestClass === 'interactive_stream',
      fundingKind: plan.quote.fundingKind,
      requestId: record.requestId,
      batch: record.requestClass === 'batch',
    })
  }
  return contexts
}

export const meterGymPaidRunSet = (
  plan: GymPaidRunPlan,
  runSet: BenchmarkRunSet,
  meteringHook: MeteringHook,
): Effect.Effect<ReadonlyArray<MeteringOutcome>> =>
  Effect.forEach(
    buildGymPaidRunMeteringContexts(plan, runSet),
    context => meteringHook(context),
    { concurrency: 1 },
  )

export const buildGymPaidRunReportReceipt = (
  input: Readonly<{
    plan: GymPaidRunPlan
    runSet: BenchmarkRunSet
    report: ReturnType<typeof buildBenchmarkReport>
    meteringOutcomes: ReadonlyArray<MeteringOutcome>
  }>,
): GymPaidRunReportReceipt => {
  const reportDigestRef = `digest.fnv1a32.gym_report.${fnv1a32(
    stableJson(input.report),
  )}`
  const receiptRef = `receipt.gym.paid_run.${fnv1a32(
    stableJson({
      quoteRef: input.plan.quote.quoteRef,
      reportDigestRef,
      meteringReceiptRefs: input.meteringOutcomes.map(
        outcome => outcome.receiptRef,
      ),
    }),
  )}`

  return {
    schemaVersion: 'openagents.gym.paid-run-report-receipt.v1',
    receiptRef,
    quoteRef: input.plan.quote.quoteRef,
    reportRef: `report.gym.${safeRefSegment(input.report.configId)}`,
    reportDigestRef,
    configId: input.report.configId,
    ownerApprovalRef: input.plan.preflight.ownerApprovalRef ?? 'missing',
    decisionGrade: input.report.decisionGrade,
    quoteMsat: input.plan.quote.quotedMsat,
    billableSampleUpperBound: input.plan.quote.billableSampleUpperBound,
    executedSamples: input.runSet.runs.filter(run => run.record !== null).length,
    skippedSamples: input.runSet.runs.filter(run => run.record === null).length,
    realTrafficEvidenceRefs: input.plan.preflight.realTrafficEvidenceRefs,
    meteringReceiptRefs: input.meteringOutcomes.flatMap(outcome =>
      outcome.receiptRef === null ? [] : [outcome.receiptRef],
    ),
  }
}
