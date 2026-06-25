import { Array, Schema as S } from 'effect'

import {
  OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG,
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
  BenchmarkWorkload,
} from '../benchmark'

const isReadonlyArrayEmpty = <A>(items: ReadonlyArray<A>): boolean =>
  items.length === 0

export const GymEnvironmentRef = S.Literals([
  'bundled-decision-suite-v1',
  'opencode-head-to-head-v1',
  'terminal-bench',
  'khala-code',
  'long-context-codebase-qa',
  'm8-head-to-head',
])
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
  'opencode-client-tools',
  'no-tools',
])
export type GymToolSetRef = typeof GymToolSetRef.Type

export const GymEnvironmentSurface = S.Literals([
  'benchmark-task-set',
  'client-surface',
  'artifact-acceptance',
  'retrieval-qa',
  'recorded-head-to-head',
])
export type GymEnvironmentSurface = typeof GymEnvironmentSurface.Type

export const GymEnvironmentTaskSet = S.Struct({
  ref: S.String,
  source: S.Literals([
    'worker-fixture',
    'harbor',
    'khala-acceptance',
    'retained-public-fixture',
    'recorded-manifest',
  ]),
  publicSafeTaskRefs: S.Array(S.String),
  harborDataset: S.optional(S.String),
})
export type GymEnvironmentTaskSet = typeof GymEnvironmentTaskSet.Type

export const GymVerifierMode = S.Literals([
  'fixture',
  'client-surface',
  'harbor-separate',
  'executed-acceptance',
  'seeded-reference',
])
export type GymVerifierMode = typeof GymVerifierMode.Type

export const GymEnvironmentVerifier = S.Struct({
  ref: S.String,
  mode: GymVerifierMode,
  expectedOutcome: S.Literals([
    'none',
    'seeded',
    'test_passed',
    'exact_trace_replay',
  ]),
})
export type GymEnvironmentVerifier = typeof GymEnvironmentVerifier.Type

export const GymAcceptanceContract = S.Struct({
  ref: S.String,
  verifierRef: S.String,
  scalarRewardPassThreshold: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 1 }),
  ),
  requiresExecutedVerifier: S.Boolean,
  publicClaimEligible: S.Boolean,
})
export type GymAcceptanceContract = typeof GymAcceptanceContract.Type

export const GymEnvironmentDefinition = S.Struct({
  ref: GymEnvironmentRef,
  surface: GymEnvironmentSurface,
  taskSet: GymEnvironmentTaskSet,
  workloads: S.Array(BenchmarkWorkload),
  verifier: GymEnvironmentVerifier,
  acceptance: GymAcceptanceContract,
  defaultShapes: S.Array(SequenceShape),
  defaultTools: GymToolSetRef,
})
export type GymEnvironmentDefinition = typeof GymEnvironmentDefinition.Type

export type GymEnvironmentRegistry = Readonly<
  Partial<Record<GymEnvironmentRef, GymEnvironmentDefinition>>
>

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
      'unregistered_environment',
      'missing_environment_verifier',
      'missing_environment_acceptance_contract',
      'empty_environment_workloads',
      'empty_environment_default_shapes',
      'real_seam_disabled_in_phase_0',
      'empty_fanout',
      'empty_shapes',
      'invalid_billable_sample_cap',
    ]),
    message: S.String,
  },
) {}

export type GymPolicySelection = Readonly<{
  environment: Readonly<{
    ref: GymEnvironmentRef
    surface: GymEnvironmentSurface
    taskSetRef: string
    verifierRef: string
    acceptanceContractRef: string
    defaultShapeIds: ReadonlyArray<string>
  }>
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

const TERMINAL_BENCH_RETAINED_SHAPE: typeof SequenceShape.Type = {
  id: 'terminal-bench-retained-harbor-lite',
  inputTokens: 5200,
  outputTokens: 900,
  cacheablePrefixTokens: 1100,
  concurrency: 1,
  provenance: 'realistic',
}

const KHALA_CODE_CROSSY_ROAD_SHAPE: typeof SequenceShape.Type = {
  id: 'khala-code-crossy-road-acceptance',
  inputTokens: 2100,
  outputTokens: 7800,
  cacheablePrefixTokens: 1350,
  concurrency: 1,
  provenance: 'realistic',
}

const LONG_CONTEXT_CODEBASE_QA_SHAPE: typeof SequenceShape.Type = {
  id: 'long-context-codebase-qa-32k',
  inputTokens: 32000,
  outputTokens: 750,
  cacheablePrefixTokens: 28000,
  concurrency: 2,
  provenance: 'realistic',
}

const M8_HEAD_TO_HEAD_SHAPE: typeof SequenceShape.Type = {
  id: 'm8-crossy-road-head-to-head-recorded',
  inputTokens: 2400,
  outputTokens: 9000,
  cacheablePrefixTokens: 1500,
  concurrency: 1,
  provenance: 'realistic',
}

export const GYM_ENVIRONMENT_REGISTRY: Readonly<
  Record<GymEnvironmentRef, GymEnvironmentDefinition>
> = {
  'bundled-decision-suite-v1': {
    ref: 'bundled-decision-suite-v1',
    surface: 'benchmark-task-set',
    taskSet: {
      ref: 'taskset.khala.fixture.decision_suite.v1',
      source: 'worker-fixture',
      publicSafeTaskRefs: ['fixture:chat-code-verifier-long-context:v1'],
    },
    workloads: SAMPLE_DECISION_SUITE_CONFIG.workloads,
    verifier: {
      ref: 'verifier.fixture.khala-decision-suite.v1',
      mode: 'fixture',
      expectedOutcome: 'test_passed',
    },
    acceptance: {
      ref: 'acceptance.fixture.khala-decision-suite.v1',
      verifierRef: 'verifier.fixture.khala-decision-suite.v1',
      scalarRewardPassThreshold: 1,
      requiresExecutedVerifier: true,
      publicClaimEligible: false,
    },
    defaultShapes: SAMPLE_DECISION_SUITE_CONFIG.shapes,
    defaultTools: 'khala-fixture-tools',
  },
  'opencode-head-to-head-v1': {
    ref: 'opencode-head-to-head-v1',
    surface: 'client-surface',
    taskSet: {
      ref: 'taskset.opencode.edit-run-smoke.v1',
      source: 'worker-fixture',
      publicSafeTaskRefs: ['gym.fixture.opencode.edit-run-smoke.v1'],
    },
    workloads: OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG.workloads,
    verifier: {
      ref: 'verifier.fixture.opencode.tool-call-command.v1',
      mode: 'client-surface',
      expectedOutcome: 'test_passed',
    },
    acceptance: {
      ref: 'acceptance.opencode.tool-call-command.v1',
      verifierRef: 'verifier.fixture.opencode.tool-call-command.v1',
      scalarRewardPassThreshold: 1,
      requiresExecutedVerifier: true,
      publicClaimEligible: false,
    },
    defaultShapes: OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG.shapes,
    defaultTools: 'opencode-client-tools',
  },
  'terminal-bench': {
    ref: 'terminal-bench',
    surface: 'benchmark-task-set',
    taskSet: {
      ref: 'taskset.harbor.terminal-bench.retained-stage0.v1',
      source: 'harbor',
      harborDataset: 'terminal-bench@2.0',
      publicSafeTaskRefs: [
        'configure-git-webserver',
        'db-wal-recovery',
        'filter-js-from-html',
        'gcode-to-text',
        'pypi-server',
        'query-optimize',
        'runner-stall-supervision',
      ],
    },
    workloads: ['verifier-run'],
    verifier: {
      ref: 'verifier.harbor.terminal_bench.summary.v1',
      mode: 'harbor-separate',
      expectedOutcome: 'test_passed',
    },
    acceptance: {
      ref: 'acceptance.harbor.terminal_bench.reward_1.v1',
      verifierRef: 'verifier.harbor.terminal_bench.summary.v1',
      scalarRewardPassThreshold: 1,
      requiresExecutedVerifier: true,
      publicClaimEligible: false,
    },
    defaultShapes: [TERMINAL_BENCH_RETAINED_SHAPE],
    defaultTools: 'khala-code-tools',
  },
  'khala-code': {
    ref: 'khala-code',
    surface: 'artifact-acceptance',
    taskSet: {
      ref: 'taskset.khala-code.crossy-road-artifact.v1',
      source: 'khala-acceptance',
      publicSafeTaskRefs: ['khala-code.crossy-road.single-html.v1'],
    },
    workloads: ['khala-code-artifact-gen'],
    verifier: {
      ref: 'verifier.khala_code.executed_acceptance_suite.v1',
      mode: 'executed-acceptance',
      expectedOutcome: 'test_passed',
    },
    acceptance: {
      ref: 'acceptance.khala_code.executed_crossy_road_6_checks.v1',
      verifierRef: 'verifier.khala_code.executed_acceptance_suite.v1',
      scalarRewardPassThreshold: 1,
      requiresExecutedVerifier: true,
      publicClaimEligible: false,
    },
    defaultShapes: [KHALA_CODE_CROSSY_ROAD_SHAPE],
    defaultTools: 'khala-code-tools',
  },
  'long-context-codebase-qa': {
    ref: 'long-context-codebase-qa',
    surface: 'retrieval-qa',
    taskSet: {
      ref: 'taskset.khala.long_context_codebase_qa.v1',
      source: 'retained-public-fixture',
      publicSafeTaskRefs: ['long-context-codebase-qa.seeded-answer.v1'],
    },
    workloads: ['long-context-codebase-question'],
    verifier: {
      ref: 'verifier.seeded.long_context_answer.v1',
      mode: 'seeded-reference',
      expectedOutcome: 'seeded',
    },
    acceptance: {
      ref: 'acceptance.seeded.long_context_answer.partial_reward.v1',
      verifierRef: 'verifier.seeded.long_context_answer.v1',
      scalarRewardPassThreshold: 0.6,
      requiresExecutedVerifier: false,
      publicClaimEligible: false,
    },
    defaultShapes: [LONG_CONTEXT_CODEBASE_QA_SHAPE],
    defaultTools: 'khala-fixture-tools',
  },
  'm8-head-to-head': {
    ref: 'm8-head-to-head',
    surface: 'recorded-head-to-head',
    taskSet: {
      ref: 'taskset.khala.m8.crossy_road_head_to_head.v1',
      source: 'recorded-manifest',
      publicSafeTaskRefs: [
        'recorded.khala.head_to_head.crossy_road.verified_run.v1',
      ],
    },
    workloads: ['khala-code-artifact-gen'],
    verifier: {
      ref: 'verifier.khala_code.executed_acceptance_suite.v1',
      mode: 'executed-acceptance',
      expectedOutcome: 'test_passed',
    },
    acceptance: {
      ref: 'acceptance.khala.m8.executed_pass_or_honest_fail.v1',
      verifierRef: 'verifier.khala_code.executed_acceptance_suite.v1',
      scalarRewardPassThreshold: 1,
      requiresExecutedVerifier: true,
      publicClaimEligible: false,
    },
    defaultShapes: [M8_HEAD_TO_HEAD_SHAPE],
    defaultTools: 'khala-code-tools',
  },
}

export const listGymEnvironmentDefinitions =
  (): ReadonlyArray<GymEnvironmentDefinition> =>
    Object.values(GYM_ENVIRONMENT_REGISTRY)

const resolveGymEnvironmentDefinition = (
  environment: GymEnvironmentRef,
  registry: GymEnvironmentRegistry = GYM_ENVIRONMENT_REGISTRY,
): GymEnvironmentDefinition => {
  const definition = registry[environment]
  if (definition === undefined) {
    throw new GymExperimentCompileError({
      reason: 'unregistered_environment',
      message: `GymEnvironment ${environment} is not registered in the typed Gym environment registry.`,
    })
  }
  return definition
}

export const getGymEnvironmentDefinition = (
  environment: GymEnvironmentRef,
): GymEnvironmentDefinition => resolveGymEnvironmentDefinition(environment)

const validateGymEnvironmentDefinition = (
  definition: GymEnvironmentDefinition,
): GymExperimentCompileError | null => {
  if (definition.verifier.ref.trim() === '') {
    return new GymExperimentCompileError({
      reason: 'missing_environment_verifier',
      message: `GymEnvironment ${definition.ref} cannot run without a verifier ref.`,
    })
  }
  if (definition.acceptance.ref.trim() === '') {
    return new GymExperimentCompileError({
      reason: 'missing_environment_acceptance_contract',
      message: `GymEnvironment ${definition.ref} cannot run without an acceptance contract.`,
    })
  }
  if (definition.acceptance.verifierRef !== definition.verifier.ref) {
    return new GymExperimentCompileError({
      reason: 'missing_environment_verifier',
      message:
        `GymEnvironment ${definition.ref} acceptance contract is bound to ` +
        `${definition.acceptance.verifierRef}, not verifier ${definition.verifier.ref}.`,
    })
  }
  if (isReadonlyArrayEmpty(definition.workloads)) {
    return new GymExperimentCompileError({
      reason: 'empty_environment_workloads',
      message: `GymEnvironment ${definition.ref} must bind at least one benchmark workload.`,
    })
  }
  if (isReadonlyArrayEmpty(definition.defaultShapes)) {
    return new GymExperimentCompileError({
      reason: 'empty_environment_default_shapes',
      message: `GymEnvironment ${definition.ref} must publish default sequence shapes.`,
    })
  }
  return null
}

const engineForLane = (lane: BenchmarkLane): BenchmarkEngine => {
  if (
    lane === 'pylon-whole-small' ||
    lane === 'gpt-oss-20b' ||
    lane === 'gpt-oss-120b' ||
    lane === 'glm-52'
  ) {
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

export const OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT: GymExperiment = {
  id: 'gym-opencode-khala-vs-bigpickle-fixture-v1',
  environment: 'opencode-head-to-head-v1',
  policy: {
    coordinator: 'heuristic-v0',
    fanout: {
      lanes: OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG.targets.map(
        target => target.lane,
      ),
      mode: 'verifier-pick',
      concurrency: 1,
    },
    tools: 'opencode-client-tools',
    modules: {
      mode: 'none',
      signatureRefs: [],
      moduleRefs: [],
    },
    sampling: {
      temperature:
        OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG.sampling[0]?.temperature ??
        0.2,
      reasoningEffort:
        OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG.sampling[0]
          ?.reasoningEffort ?? 'off',
      maxTokens: 2048,
      transport: 'streaming',
    },
    serving: {
      quantization: { mode: 'none' },
      speculation: { mode: 'none' },
    },
  },
  shapes: OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG.shapes,
  samplesPerCell: OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG.samplesPerCell,
  budget: {
    spendCapMsat: 0,
    maxBillableSamples: 0,
    seam: 'fixture',
  },
}

export const TERMINAL_BENCH_GYM_EXPERIMENT: GymExperiment = {
  id: 'gym-terminal-bench-retained-fixture-v1',
  environment: 'terminal-bench',
  policy: {
    coordinator: 'heuristic-v0',
    fanout: {
      lanes: ['khala'],
      mode: 'verifier-pick',
      concurrency: 1,
    },
    tools: GYM_ENVIRONMENT_REGISTRY['terminal-bench'].defaultTools,
    modules: {
      mode: 'none',
      signatureRefs: [],
      moduleRefs: [],
    },
    sampling: {
      temperature: 0.2,
      reasoningEffort: 'off',
      maxTokens: 4096,
      transport: 'streaming',
    },
    serving: {
      quantization: { mode: 'none' },
      speculation: { mode: 'none' },
    },
  },
  shapes: GYM_ENVIRONMENT_REGISTRY['terminal-bench'].defaultShapes,
  samplesPerCell: 5,
  budget: {
    spendCapMsat: 0,
    maxBillableSamples: 0,
    seam: 'fixture',
  },
}

export const KHALA_CODE_GYM_EXPERIMENT: GymExperiment = {
  ...TERMINAL_BENCH_GYM_EXPERIMENT,
  id: 'gym-khala-code-crossy-road-fixture-v1',
  environment: 'khala-code',
  policy: {
    ...TERMINAL_BENCH_GYM_EXPERIMENT.policy,
    tools: GYM_ENVIRONMENT_REGISTRY['khala-code'].defaultTools,
    sampling: {
      ...TERMINAL_BENCH_GYM_EXPERIMENT.policy.sampling,
      maxTokens: 12000,
    },
  },
  shapes: GYM_ENVIRONMENT_REGISTRY['khala-code'].defaultShapes,
}

export const LONG_CONTEXT_CODEBASE_QA_GYM_EXPERIMENT: GymExperiment = {
  ...TERMINAL_BENCH_GYM_EXPERIMENT,
  id: 'gym-long-context-codebase-qa-fixture-v1',
  environment: 'long-context-codebase-qa',
  policy: {
    ...TERMINAL_BENCH_GYM_EXPERIMENT.policy,
    tools: GYM_ENVIRONMENT_REGISTRY['long-context-codebase-qa'].defaultTools,
    sampling: {
      ...TERMINAL_BENCH_GYM_EXPERIMENT.policy.sampling,
      maxTokens: 2048,
    },
  },
  shapes: GYM_ENVIRONMENT_REGISTRY['long-context-codebase-qa'].defaultShapes,
}

export const M8_HEAD_TO_HEAD_GYM_EXPERIMENT: GymExperiment = {
  ...KHALA_CODE_GYM_EXPERIMENT,
  id: 'gym-m8-crossy-road-head-to-head-fixture-v1',
  environment: 'm8-head-to-head',
  shapes: GYM_ENVIRONMENT_REGISTRY['m8-head-to-head'].defaultShapes,
}

export const PHASE_1_GYM_ENVIRONMENT_FIXTURE_EXPERIMENTS: ReadonlyArray<GymExperiment> =
  [
    TERMINAL_BENCH_GYM_EXPERIMENT,
    KHALA_CODE_GYM_EXPERIMENT,
    LONG_CONTEXT_CODEBASE_QA_GYM_EXPERIMENT,
    M8_HEAD_TO_HEAD_GYM_EXPERIMENT,
  ]

const validateGymExperiment = (
  experiment: GymExperiment,
  environmentDefinition: GymEnvironmentDefinition,
): GymExperimentCompileError | null => {
  const maybeEnvironmentError =
    validateGymEnvironmentDefinition(environmentDefinition)
  if (maybeEnvironmentError !== null) {
    return maybeEnvironmentError
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
  registry: GymEnvironmentRegistry = GYM_ENVIRONMENT_REGISTRY,
): CompiledGymExperiment => {
  const environmentDefinition = resolveGymEnvironmentDefinition(
    experiment.environment,
    registry,
  )
  const maybeError = validateGymExperiment(experiment, environmentDefinition)
  if (maybeError !== null) {
    throw maybeError
  }

  const matrixConfig: BenchmarkMatrixConfig = {
    id: `gym:${experiment.id}`,
    description:
      `OpenAgents Gym fixture experiment ${experiment.id}: typed policy ` +
      `${experiment.policy.coordinator} over ${experiment.policy.fanout.mode} fanout, no spend.`,
    targets: targetsForLanes(experiment.policy.fanout.lanes),
    workloads: environmentDefinition.workloads,
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
      environment: {
        ref: environmentDefinition.ref,
        surface: environmentDefinition.surface,
        taskSetRef: environmentDefinition.taskSet.ref,
        verifierRef: environmentDefinition.verifier.ref,
        acceptanceContractRef: environmentDefinition.acceptance.ref,
        defaultShapeIds: environmentDefinition.defaultShapes.map(
          shape => shape.id,
        ),
      },
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
  registry: GymEnvironmentRegistry = GYM_ENVIRONMENT_REGISTRY,
): GymFixtureRunResult => {
  const compiled = compileGymExperiment(experiment, registry)
  const runSet = runBenchmark(compiled.matrixConfig, makeFixtureLaneSeam())
  const report = buildBenchmarkReport(runSet)
  return {
    compiled,
    runSet,
    report,
    publicSafety: checkReportPublicSafety(report),
  }
}
