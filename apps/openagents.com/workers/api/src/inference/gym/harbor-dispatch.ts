import { Schema as S } from 'effect'

import type { BenchmarkLane } from '../benchmark'
import {
  GYM_ENVIRONMENT_REGISTRY,
  compileGymExperiment,
  type CompiledGymExperiment,
  type GymEnvironmentDefinition,
  type GymEnvironmentRegistry,
  type GymExperiment,
} from './experiment'

export const GYM_HARBOR_TERMINAL_BENCH_JOB_SPEC_SCHEMA =
  'openagents.gym.harbor_terminal_bench_job_spec.v1'
export const GYM_HARBOR_TERMINAL_BENCH_DISPATCH_RECEIPT_SCHEMA =
  'openagents.gym.harbor_terminal_bench_dispatch_receipt.v1'
export const GYM_HARBOR_TERMINAL_BENCH_INGEST_SCHEMA =
  'openagents.gym.harbor_terminal_bench_ingest.v1'
export const HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA =
  'hydralisk.evals.terminal_bench.summary.v1'

const TERMINAL_BENCH_HARBOR_DATASET_CLI_REF = 'terminal-bench/terminal-bench-2'
const KHALA_PUBLIC_MODEL_ID = 'openagents/khala'

export const HarborTerminalBenchAgent = S.Literals([
  'terminus-2',
  'opencode',
  'codex',
  'claude-code',
  'oracle',
])
export type HarborTerminalBenchAgent = typeof HarborTerminalBenchAgent.Type

export const GymHarborTerminalBenchArtifactPolicy = S.Struct({
  hydraliskSummarySchema: S.Literal(HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA),
  requestPublicSafeSummary: S.Literal(true),
  requestAtifTrajectory: S.Literal(true),
  requestRawHarborLogs: S.Literal(false),
  requestRawTaskPrompts: S.Literal(false),
  requestRawModelResponses: S.Literal(false),
})
export type GymHarborTerminalBenchArtifactPolicy =
  typeof GymHarborTerminalBenchArtifactPolicy.Type

export const GymHarborTerminalBenchCommandSpec = S.Struct({
  executable: S.Literal('harbor'),
  argv: S.Array(S.String),
  runnerMode: S.Literal('cli-artifact'),
})
export type GymHarborTerminalBenchCommandSpec =
  typeof GymHarborTerminalBenchCommandSpec.Type

export const GymHarborTerminalBenchJobSpec = S.Struct({
  schemaVersion: S.Literal(GYM_HARBOR_TERMINAL_BENCH_JOB_SPEC_SCHEMA),
  jobRef: S.String,
  experimentId: S.String,
  configId: S.String,
  environmentRef: S.Literal('terminal-bench'),
  taskSetRef: S.String,
  retainedPublicTaskRefs: S.Array(S.String),
  harborDataset: S.Literal('terminal-bench@2.0'),
  harborDatasetCliRef: S.Literal(TERMINAL_BENCH_HARBOR_DATASET_CLI_REF),
  harnessRef: S.Literal('hydralisk.harbor.terminal_bench.cli_artifact.v1'),
  runner: S.Literal('harbor'),
  agent: HarborTerminalBenchAgent,
  model: S.Literal(KHALA_PUBLIC_MODEL_ID),
  modelEndpointRef: S.Literal('openagents.khala.public_openai_compat.v1'),
  nConcurrent: S.Number.check(S.isBetween({ minimum: 1, maximum: 64 })),
  maxAttempts: S.Number.check(S.isBetween({ minimum: 1, maximum: 1000 })),
  ownerApprovalRef: S.NullOr(S.String),
  command: GymHarborTerminalBenchCommandSpec,
  artifacts: GymHarborTerminalBenchArtifactPolicy,
  publicSafetyBoundary: S.Struct({
    rawHarborArtifactsStayOnHydralisk: S.Literal(true),
    workerImportsHarborRuntime: S.Literal(false),
    publicSummaryOnly: S.Literal(true),
    noPublicClaimUntilReportProjection: S.Literal(true),
  }),
})
export type GymHarborTerminalBenchJobSpec =
  typeof GymHarborTerminalBenchJobSpec.Type

export const GymHarborTerminalBenchDispatchReceipt = S.Struct({
  schemaVersion: S.Literal(GYM_HARBOR_TERMINAL_BENCH_DISPATCH_RECEIPT_SCHEMA),
  jobRef: S.String,
  hydraliskRunRef: S.String,
  state: S.Literals(['accepted', 'running', 'completed']),
  summaryArtifactRef: S.String,
  atifTraceRef: S.NullOr(S.String),
  rawHarborArtifactRef: S.NullOr(S.String),
})
export type GymHarborTerminalBenchDispatchReceipt =
  typeof GymHarborTerminalBenchDispatchReceipt.Type

export const HydraliskTerminalBenchSummary = S.Struct({
  schema: S.Literal(HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA),
  createdAt: S.String,
  publicSafe: S.Literal(true),
  benchmark: S.Struct({
    name: S.Literal('Terminal-Bench'),
    datasetRef: S.Literal('terminal-bench@2.0'),
    version: S.String,
    repository: S.String,
    harnessRepository: S.String,
  }),
  runner: S.Struct({
    name: S.Literal('harbor'),
    version: S.String,
    agent: HarborTerminalBenchAgent,
    model: S.Literal(KHALA_PUBLIC_MODEL_ID),
    nConcurrent: S.Number,
    timeoutSeconds: S.Number,
    maxAttempts: S.Number,
    retryPolicy: S.String,
  }),
  model: S.Struct({
    alias: S.String,
    profileRef: S.String,
    revision: S.String,
    hardwareProfile: S.String,
  }),
  sampler: S.Struct({
    minP: S.Number,
    repetitionPenalty: S.Number,
    maxTokens: S.Number,
    enableThinking: S.Boolean,
  }),
  counts: S.Struct({
    solved: S.Number,
    failing: S.Number,
    envBroken: S.Number,
    notStarted: S.Number,
    total: S.Number,
    attempted: S.Number,
    properlyAttempted: S.Number,
  }),
  rates: S.Struct({
    fullDenominatorSolved: S.NullOr(S.Number),
    attemptedSolved: S.NullOr(S.Number),
    properlyAttemptedSolved: S.NullOr(S.Number),
    knownPassAt1: S.NullOr(S.Number),
    passAtN: S.NullOr(S.Number),
  }),
  passAt: S.Struct({
    passAt1Solved: S.NullOr(S.Number),
    passAt1KnownTasks: S.Number,
    passAtNAnySolved: S.Number,
    maxAttempts: S.Number,
  }),
  denominatorDefinitions: S.Struct({
    total: S.String,
    attempted: S.String,
    properlyAttempted: S.String,
    fullDenominatorSolved: S.String,
    attemptedSolved: S.String,
    properlyAttemptedSolved: S.String,
  }),
  taskIds: S.Struct({
    envBroken: S.Array(S.String),
    notStarted: S.Array(S.String),
    notableSolved: S.Array(S.String),
  }),
  claimStatus: S.String,
  inputSha256: S.NullOr(S.String),
  comparisonBoundary: S.String,
  publicSafety: S.Struct({
    containsSecrets: S.Literal(false),
    containsPrompts: S.Literal(false),
    containsResponses: S.Literal(false),
    containsHiddenReasoning: S.Literal(false),
    containsPrivateSource: S.Literal(false),
    containsRawBenchmarkLogs: S.Literal(false),
  }),
})
export type HydraliskTerminalBenchSummary =
  typeof HydraliskTerminalBenchSummary.Type

export const GymHarborTerminalBenchIngest = S.Struct({
  schemaVersion: S.Literal(GYM_HARBOR_TERMINAL_BENCH_INGEST_SCHEMA),
  ingestRef: S.String,
  jobRef: S.String,
  hydraliskRunRef: S.String,
  configId: S.String,
  environmentRef: S.Literal('terminal-bench'),
  summarySchema: S.Literal(HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA),
  summaryArtifactRef: S.String,
  atifTraceRef: S.NullOr(S.String),
  datasetRef: S.Literal('terminal-bench@2.0'),
  runner: S.Literal('harbor'),
  agent: HarborTerminalBenchAgent,
  model: S.Literal(KHALA_PUBLIC_MODEL_ID),
  publicSafe: S.Literal(true),
  publicClaimEligible: S.Literal(false),
  decisionGradeReportReady: S.Literal(false),
  acceptedOutcomes: S.Number,
  attemptedOutcomes: S.Number,
  totalTasks: S.Number,
  fullDenominatorSolved: S.NullOr(S.Number),
  passAtN: S.NullOr(S.Number),
  caveats: S.Array(S.String),
})
export type GymHarborTerminalBenchIngest =
  typeof GymHarborTerminalBenchIngest.Type

export class GymHarborDispatchError extends S.TaggedErrorClass<GymHarborDispatchError>()(
  'GymHarborDispatchError',
  {
    reason: S.Literals([
      'unsupported_environment',
      'unsupported_dataset',
      'unsupported_lane',
      'dispatch_job_ref_mismatch',
      'unsafe_summary',
    ]),
    message: S.String,
  },
) {}

export type HydraliskHarborTerminalBenchHarness = Readonly<{
  dispatchTerminalBenchJob: (
    job: GymHarborTerminalBenchJobSpec,
  ) => Promise<GymHarborTerminalBenchDispatchReceipt>
  readTerminalBenchSummary: (
    receipt: GymHarborTerminalBenchDispatchReceipt,
  ) => Promise<unknown>
}>

export type GymHarborTerminalBenchRun = Readonly<{
  compiled: CompiledGymExperiment
  job: GymHarborTerminalBenchJobSpec
  dispatch: GymHarborTerminalBenchDispatchReceipt
  summary: HydraliskTerminalBenchSummary
  ingest: GymHarborTerminalBenchIngest
  publicSafety: HydraliskTerminalBenchSummaryPublicSafety
}>

export type HydraliskTerminalBenchSummaryPublicSafety = Readonly<{
  safe: boolean
  violations: ReadonlyArray<string>
}>

const decodeJobSpec = S.decodeUnknownSync(GymHarborTerminalBenchJobSpec)
const decodeDispatchReceipt = S.decodeUnknownSync(
  GymHarborTerminalBenchDispatchReceipt,
)
const decodeSummary = S.decodeUnknownSync(HydraliskTerminalBenchSummary)
const decodeIngest = S.decodeUnknownSync(GymHarborTerminalBenchIngest)

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

const makeJobRef = (experiment: GymExperiment): string =>
  `job.gym.harbor_terminal_bench.${safeRefSegment(experiment.id)}.${fnv1a32(
    stableJson({
      environment: experiment.environment,
      lanes: experiment.policy.fanout.lanes,
      samplesPerCell: experiment.samplesPerCell,
      shapes: experiment.shapes.map(shape => shape.id),
    }),
  )}`

const makeIngestRef = (
  job: GymHarborTerminalBenchJobSpec,
  summary: HydraliskTerminalBenchSummary,
): string =>
  `ingest.gym.harbor_terminal_bench.${safeRefSegment(job.jobRef)}.${fnv1a32(
    stableJson({
      counts: summary.counts,
      rates: summary.rates,
      runner: summary.runner,
      summaryInput: summary.inputSha256,
    }),
  )}`

const terminalBenchDefinition = (
  registry: GymEnvironmentRegistry,
): GymEnvironmentDefinition => {
  const definition = registry['terminal-bench']
  if (definition === undefined) {
    throw new GymHarborDispatchError({
      reason: 'unsupported_environment',
      message: 'The terminal-bench Gym environment is not registered.',
    })
  }
  return definition
}

const assertTerminalBenchKhalaExperiment = (
  experiment: GymExperiment,
  definition: GymEnvironmentDefinition,
): void => {
  if (experiment.environment !== 'terminal-bench') {
    throw new GymHarborDispatchError({
      reason: 'unsupported_environment',
      message:
        'Hydralisk Harbor dispatch is only wired for the terminal-bench Gym environment.',
    })
  }
  if (
    definition.taskSet.source !== 'harbor' ||
    definition.taskSet.harborDataset !== 'terminal-bench@2.0'
  ) {
    throw new GymHarborDispatchError({
      reason: 'unsupported_dataset',
      message:
        'Hydralisk Harbor dispatch requires the typed terminal-bench@2.0 Harbor task set.',
    })
  }
  if (
    experiment.policy.fanout.lanes.length !== 1 ||
    experiment.policy.fanout.lanes[0] !== 'khala'
  ) {
    throw new GymHarborDispatchError({
      reason: 'unsupported_lane',
      message:
        'The first Hydralisk Harbor dispatch seam is intentionally scoped to openagents/khala.',
    })
  }
}

const commandForJob = (input: {
  agent: HarborTerminalBenchAgent
  nConcurrent: number
}): GymHarborTerminalBenchCommandSpec => ({
  executable: 'harbor',
  runnerMode: 'cli-artifact',
  argv: [
    'run',
    '-d',
    TERMINAL_BENCH_HARBOR_DATASET_CLI_REF,
    '--agent',
    input.agent,
    '--model',
    KHALA_PUBLIC_MODEL_ID,
    '--n-concurrent',
    `${input.nConcurrent}`,
  ],
})

export const buildGymHarborTerminalBenchJobSpec = (
  experiment: GymExperiment,
  input: Readonly<{
    agent?: HarborTerminalBenchAgent | undefined
    ownerApprovalRef?: string | undefined
    registry?: GymEnvironmentRegistry | undefined
  }> = {},
): Readonly<{
  compiled: CompiledGymExperiment
  job: GymHarborTerminalBenchJobSpec
}> => {
  const registry = input.registry ?? GYM_ENVIRONMENT_REGISTRY
  const definition = terminalBenchDefinition(registry)
  assertTerminalBenchKhalaExperiment(experiment, definition)
  const compiled = compileGymExperiment(experiment, registry)
  const agent = input.agent ?? 'terminus-2'
  const job = decodeJobSpec({
    schemaVersion: GYM_HARBOR_TERMINAL_BENCH_JOB_SPEC_SCHEMA,
    jobRef: makeJobRef(experiment),
    experimentId: experiment.id,
    configId: compiled.matrixConfig.id,
    environmentRef: 'terminal-bench',
    taskSetRef: definition.taskSet.ref,
    retainedPublicTaskRefs: definition.taskSet.publicSafeTaskRefs,
    harborDataset: 'terminal-bench@2.0',
    harborDatasetCliRef: TERMINAL_BENCH_HARBOR_DATASET_CLI_REF,
    harnessRef: 'hydralisk.harbor.terminal_bench.cli_artifact.v1',
    runner: 'harbor',
    agent,
    model: KHALA_PUBLIC_MODEL_ID,
    modelEndpointRef: 'openagents.khala.public_openai_compat.v1',
    nConcurrent: experiment.policy.fanout.concurrency,
    maxAttempts: experiment.samplesPerCell,
    ownerApprovalRef: input.ownerApprovalRef ?? null,
    command: commandForJob({
      agent,
      nConcurrent: experiment.policy.fanout.concurrency,
    }),
    artifacts: {
      hydraliskSummarySchema: HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
      requestPublicSafeSummary: true,
      requestAtifTrajectory: true,
      requestRawHarborLogs: false,
      requestRawTaskPrompts: false,
      requestRawModelResponses: false,
    },
    publicSafetyBoundary: {
      rawHarborArtifactsStayOnHydralisk: true,
      workerImportsHarborRuntime: false,
      publicSummaryOnly: true,
      noPublicClaimUntilReportProjection: true,
    },
  })

  return { compiled, job }
}

export const checkHydraliskTerminalBenchSummaryPublicSafety = (
  summary: HydraliskTerminalBenchSummary,
): HydraliskTerminalBenchSummaryPublicSafety => {
  const violations: Array<string> = []
  if (summary.publicSafe !== true) {
    violations.push('public_safe_false')
  }
  for (const [key, value] of Object.entries(summary.publicSafety)) {
    if (value !== false) {
      violations.push(`public_safety.${key}`)
    }
  }
  const serialized = JSON.stringify(summary).toLowerCase()
  const forbiddenMarkers = [
    '"rawprompt"',
    '"raw_prompt"',
    '"prompttext"',
    '"rawresponse"',
    '"raw_response"',
    '"rawbenchmarklog"',
    '"raw_benchmark_log"',
    '"bearer"',
    '"api_key"',
    '"apikey"',
    '"mnemonic"',
    '"hiddenreasoning"',
    '"hidden_reasoning"',
  ]
  for (const marker of forbiddenMarkers) {
    if (serialized.includes(marker)) {
      violations.push(marker.replaceAll('"', ''))
    }
  }
  return { safe: violations.length === 0, violations }
}

export const buildGymHarborTerminalBenchIngest = (input: {
  job: GymHarborTerminalBenchJobSpec
  dispatch: GymHarborTerminalBenchDispatchReceipt
  summary: HydraliskTerminalBenchSummary
}): GymHarborTerminalBenchIngest => {
  const publicSafety = checkHydraliskTerminalBenchSummaryPublicSafety(
    input.summary,
  )
  if (!publicSafety.safe) {
    throw new GymHarborDispatchError({
      reason: 'unsafe_summary',
      message:
        'Hydralisk Terminal-Bench summary did not satisfy the public-safety boundary.',
    })
  }

  return decodeIngest({
    schemaVersion: GYM_HARBOR_TERMINAL_BENCH_INGEST_SCHEMA,
    ingestRef: makeIngestRef(input.job, input.summary),
    jobRef: input.job.jobRef,
    hydraliskRunRef: input.dispatch.hydraliskRunRef,
    configId: input.job.configId,
    environmentRef: 'terminal-bench',
    summarySchema: HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
    summaryArtifactRef: input.dispatch.summaryArtifactRef,
    atifTraceRef: input.dispatch.atifTraceRef,
    datasetRef: input.summary.benchmark.datasetRef,
    runner: input.summary.runner.name,
    agent: input.summary.runner.agent,
    model: input.summary.runner.model,
    publicSafe: true,
    publicClaimEligible: false,
    decisionGradeReportReady: false,
    acceptedOutcomes: input.summary.counts.solved,
    attemptedOutcomes: input.summary.counts.properlyAttempted,
    totalTasks: input.summary.counts.total,
    fullDenominatorSolved: input.summary.rates.fullDenominatorSolved,
    passAtN: input.summary.rates.passAtN,
    caveats: [
      'summary_ingest_only',
      'raw_harbor_artifacts_excluded',
      'cost_per_accepted_outcome_mapping_deferred_to_6242',
      'public_claim_projection_deferred_to_owner_armed_report',
    ],
  })
}

export const dispatchGymHarborTerminalBenchRun = async (
  experiment: GymExperiment,
  input: Readonly<{
    harness: HydraliskHarborTerminalBenchHarness
    agent?: HarborTerminalBenchAgent | undefined
    ownerApprovalRef?: string | undefined
    registry?: GymEnvironmentRegistry | undefined
  }>,
): Promise<GymHarborTerminalBenchRun> => {
  const { compiled, job } = buildGymHarborTerminalBenchJobSpec(experiment, input)
  const dispatch = decodeDispatchReceipt(
    await input.harness.dispatchTerminalBenchJob(job),
  )
  if (dispatch.jobRef !== job.jobRef) {
    throw new GymHarborDispatchError({
      reason: 'dispatch_job_ref_mismatch',
      message:
        `Hydralisk dispatch receipt ${dispatch.hydraliskRunRef} belongs to ` +
        `${dispatch.jobRef}, not ${job.jobRef}.`,
    })
  }

  const summary = decodeSummary(
    await input.harness.readTerminalBenchSummary(dispatch),
  )
  const ingest = buildGymHarborTerminalBenchIngest({
    job,
    dispatch,
    summary,
  })
  const publicSafety = checkHydraliskTerminalBenchSummaryPublicSafety(summary)

  return {
    compiled,
    job,
    dispatch,
    summary,
    ingest,
    publicSafety,
  }
}

export type GymHarborTerminalBenchDispatchLane = Extract<BenchmarkLane, 'khala'>
