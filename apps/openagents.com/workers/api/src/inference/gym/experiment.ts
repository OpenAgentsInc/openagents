import { Array, Schema as S } from 'effect'

import {
  SAMPLE_DECISION_SUITE_CONFIG,
  buildBenchmarkReport,
  checkReportPublicSafety,
  expectedCellCount,
  expandMatrix,
  makeFixtureLaneSeam,
  runBenchmark,
  type BenchmarkEngine,
  type BenchmarkLane,
  type BenchmarkMatrixConfig,
  type BenchmarkTarget,
  BenchmarkLane as BenchmarkLaneSchema,
  BenchmarkTransport,
  SamplingSettings,
  SequenceShape,
} from '../benchmark'

const isReadonlyArrayEmpty = <A>(items: ReadonlyArray<A>): boolean =>
  items.length === 0

export const GymEnvironmentRef = S.Literals(['bundled-decision-suite-v1'])
export type GymEnvironmentRef = typeof GymEnvironmentRef.Type

export const CoordinatorCandidateRef = S.Literals([
  'heuristic-v0',
  'trinity-v1',
  'conductor-v2',
])
export type CoordinatorCandidateRef = typeof CoordinatorCandidateRef.Type

export const GymFanoutMode = S.Literals([
  'single',
  'race',
  'best-of-n',
  'verifier-pick',
])
export type GymFanoutMode = typeof GymFanoutMode.Type

export const GymToolSetRef = S.Literals([
  'khala-fixture-tools',
  'khala-code-tools',
  'no-tools',
])
export type GymToolSetRef = typeof GymToolSetRef.Type

export const ProgramSignatureComposition = S.Struct({
  mode: S.Literals(['none', 'starter-catalog']),
  signatureRefs: S.Array(S.String),
  moduleRefs: S.Array(S.String),
})
export type ProgramSignatureComposition =
  typeof ProgramSignatureComposition.Type

export const GymQuantizationSpec = S.Struct({
  mode: S.Literals(['none', 'int8', 'fp8', 'nf4']),
  engineRef: S.optional(S.String),
})
export type GymQuantizationSpec = typeof GymQuantizationSpec.Type

export const GymSpeculationSpec = S.Struct({
  mode: S.Literals(['none', 'eagle', 'medusa', 'ngram']),
  draftModelRef: S.optional(S.String),
})
export type GymSpeculationSpec = typeof GymSpeculationSpec.Type

export const GymSamplingSettings = S.Struct({
  temperature: S.Number.check(S.isBetween({ minimum: 0, maximum: 2 })),
  reasoningEffort: SamplingSettings.fields.reasoningEffort,
  maxTokens: S.Number.check(S.isBetween({ minimum: 1, maximum: 128000 })),
  transport: BenchmarkTransport,
})
export type GymSamplingSettings = typeof GymSamplingSettings.Type

export const GymPolicy = S.Struct({
  coordinator: CoordinatorCandidateRef,
  fanout: S.Struct({
    lanes: S.Array(BenchmarkLaneSchema),
    mode: GymFanoutMode,
    concurrency: S.Number.check(S.isBetween({ minimum: 1, maximum: 64 })),
  }),
  tools: GymToolSetRef,
  modules: ProgramSignatureComposition,
  sampling: GymSamplingSettings,
  serving: S.Struct({
    quantization: S.optional(GymQuantizationSpec),
    speculation: S.optional(GymSpeculationSpec),
  }),
})
export type GymPolicy = typeof GymPolicy.Type

export const GymBudget = S.Struct({
  spendCapMsat: S.Number.check(S.isBetween({ minimum: 0, maximum: 1_000_000_000 })),
  maxBillableSamples: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 100_000 }),
  ),
  seam: S.Literals(['fixture', 'real']),
  ownerApprovalRef: S.optional(S.String),
})
export type GymBudget = typeof GymBudget.Type

export const GymExperiment = S.Struct({
  id: S.String,
  environment: GymEnvironmentRef,
  policy: GymPolicy,
  shapes: S.Array(SequenceShape),
  samplesPerCell: S.Number.check(S.isBetween({ minimum: 5, maximum: 10_000 })),
  budget: GymBudget,
})
export type GymExperiment = typeof GymExperiment.Type

export const decodeGymExperiment = S.decodeUnknownSync(GymExperiment)
export const encodeGymExperiment = S.encodeSync(GymExperiment)

export class GymExperimentCompileError extends S.TaggedErrorClass<GymExperimentCompileError>()(
  'GymExperimentCompileError',
  {
    reason: S.Literals([
      'real_seam_disabled_in_phase_0',
      'empty_fanout',
      'empty_shapes',
      'invalid_billable_sample_cap',
    ]),
    message: S.String,
  },
) {}

export type GymPolicySelection = Readonly<{
  coordinator: CoordinatorCandidateRef
  fanout: Readonly<{
    lanes: ReadonlyArray<BenchmarkLane>
    mode: GymFanoutMode
    concurrency: number
  }>
  tools: GymToolSetRef
  modules: ProgramSignatureComposition
  sampling: GymSamplingSettings
  serving: GymPolicy['serving']
  skippedCells: ReadonlyArray<{
    cellId: string
    lane: BenchmarkLane
    reason: string
  }>
}>

export type CompiledGymExperiment = Readonly<{
  matrixConfig: BenchmarkMatrixConfig
  policySelection: GymPolicySelection
  expectedCellCount: number
}>

export type GymFixtureRunResult = Readonly<{
  compiled: CompiledGymExperiment
  runSet: ReturnType<typeof runBenchmark>
  report: ReturnType<typeof buildBenchmarkReport>
  publicSafety: ReturnType<typeof checkReportPublicSafety>
}>

const engineForLane = (lane: BenchmarkLane): BenchmarkEngine => {
  if (lane === 'pylon-whole-small') {
    return 'vllm'
  }
  if (lane === 'psionic-shard-wan') {
    return 'sglang'
  }
  return 'provider-native'
}

const targetsForLanes = (
  lanes: ReadonlyArray<BenchmarkLane>,
): ReadonlyArray<BenchmarkTarget> =>
  Array.dedupe(lanes).map(lane => ({ lane, engine: engineForLane(lane) }))

const environmentWorkloads = (
  environment: GymEnvironmentRef,
): BenchmarkMatrixConfig['workloads'] => {
  if (environment === 'bundled-decision-suite-v1') {
    return SAMPLE_DECISION_SUITE_CONFIG.workloads
  }
  return []
}

export const BUNDLED_GYM_EXPERIMENT: GymExperiment = {
  id: 'gym-fixture-decision-suite-v1',
  environment: 'bundled-decision-suite-v1',
  policy: {
    coordinator: 'heuristic-v0',
    fanout: {
      lanes: SAMPLE_DECISION_SUITE_CONFIG.targets.map(target => target.lane),
      mode: 'verifier-pick',
      concurrency: 4,
    },
    tools: 'khala-fixture-tools',
    modules: {
      mode: 'starter-catalog',
      signatureRefs: ['program-signature:khala-fixture-decision-suite'],
      moduleRefs: ['starter-module:artifact-verifier'],
    },
    sampling: {
      temperature: SAMPLE_DECISION_SUITE_CONFIG.sampling[0]?.temperature ?? 0.2,
      reasoningEffort:
        SAMPLE_DECISION_SUITE_CONFIG.sampling[0]?.reasoningEffort ?? 'off',
      maxTokens: 2048,
      transport: 'streaming',
    },
    serving: {
      quantization: { mode: 'none' },
      speculation: { mode: 'none' },
    },
  },
  shapes: SAMPLE_DECISION_SUITE_CONFIG.shapes,
  samplesPerCell: SAMPLE_DECISION_SUITE_CONFIG.samplesPerCell,
  budget: {
    spendCapMsat: 0,
    maxBillableSamples: 0,
    seam: 'fixture',
  },
}

const validateGymExperiment = (
  experiment: GymExperiment,
): GymExperimentCompileError | null => {
  if (experiment.budget.seam === 'real') {
    return new GymExperimentCompileError({
      reason: 'real_seam_disabled_in_phase_0',
      message:
        'OpenAgents Gym Phase 0 is fixture-only; seam: real is owner-gated future work and is rejected here.',
    })
  }
  if (isReadonlyArrayEmpty(experiment.policy.fanout.lanes)) {
    return new GymExperimentCompileError({
      reason: 'empty_fanout',
      message: 'GymExperiment policy.fanout.lanes must select at least one lane.',
    })
  }
  if (isReadonlyArrayEmpty(experiment.shapes)) {
    return new GymExperimentCompileError({
      reason: 'empty_shapes',
      message: 'GymExperiment shapes must include at least one sequence shape.',
    })
  }
  if (
    experiment.budget.maxBillableSamples > 0 &&
    experiment.budget.maxBillableSamples <
      experiment.policy.fanout.lanes.length * experiment.samplesPerCell
  ) {
    return new GymExperimentCompileError({
      reason: 'invalid_billable_sample_cap',
      message:
        'GymExperiment budget.maxBillableSamples must either be 0 for fixture-only runs or cover the selected fanout samples.',
    })
  }
  return null
}

export const compileGymExperiment = (
  experiment: GymExperiment,
): CompiledGymExperiment => {
  const maybeError = validateGymExperiment(experiment)
  if (maybeError !== null) {
    throw maybeError
  }

  const matrixConfig: BenchmarkMatrixConfig = {
    id: `gym:${experiment.id}`,
    description:
      `OpenAgents Gym fixture experiment ${experiment.id}: typed policy ` +
      `${experiment.policy.coordinator} over ${experiment.policy.fanout.mode} fanout, no spend.`,
    targets: targetsForLanes(experiment.policy.fanout.lanes),
    workloads: environmentWorkloads(experiment.environment),
    shapes: experiment.shapes,
    transports: [experiment.policy.sampling.transport],
    sampling: [
      {
        temperature: experiment.policy.sampling.temperature,
        reasoningEffort: experiment.policy.sampling.reasoningEffort,
      },
    ],
    samplesPerCell: experiment.samplesPerCell,
  }
  const cells = expandMatrix(matrixConfig)
  const skippedCells = cells
    .filter(cell => cell.laneAvailability === 'not_yet_available')
    .map(cell => ({
      cellId: cell.cellId,
      lane: cell.lane,
      reason: `lane_not_yet_available:${cell.lane}`,
    }))

  return {
    matrixConfig,
    policySelection: {
      coordinator: experiment.policy.coordinator,
      fanout: {
        lanes: Array.dedupe(experiment.policy.fanout.lanes),
        mode: experiment.policy.fanout.mode,
        concurrency: experiment.policy.fanout.concurrency,
      },
      tools: experiment.policy.tools,
      modules: experiment.policy.modules,
      sampling: experiment.policy.sampling,
      serving: experiment.policy.serving,
      skippedCells,
    },
    expectedCellCount: expectedCellCount(matrixConfig),
  }
}

export const runGymFixtureExperiment = (
  experiment: GymExperiment,
): GymFixtureRunResult => {
  const compiled = compileGymExperiment(experiment)
  const runSet = runBenchmark(compiled.matrixConfig, makeFixtureLaneSeam())
  const report = buildBenchmarkReport(runSet)
  return {
    compiled,
    runSet,
    report,
    publicSafety: checkReportPublicSafety(report),
  }
}
