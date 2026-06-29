import { Schema as S } from 'effect'

import {
  BenchmarkLane as BenchmarkLaneSchema,
  type BenchmarkLane,
} from '../benchmark'
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
export const GYM_HARBOR_VERIFIER_PLACEMENT_SCHEMA =
  'openagents.gym.harbor_verifier_placement.v1'
export const HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA =
  'hydralisk.evals.terminal_bench.summary.v1'

const TERMINAL_BENCH_HARBOR_DATASET_CLI_REF = 'terminal-bench/terminal-bench-2'
export const KHALA_PUBLIC_MODEL_ID = 'openagents/khala' as const
export const GLM_REAP_TERMINAL_BENCH_MODEL_ID =
  'openagents/glm-5.2-reap-504b' as const

export const GymHarborTerminalBenchModelId = S.Literals([
  KHALA_PUBLIC_MODEL_ID,
  GLM_REAP_TERMINAL_BENCH_MODEL_ID,
])
export type GymHarborTerminalBenchModelId =
  typeof GymHarborTerminalBenchModelId.Type

export const GymTerminalBenchProfileRef = S.Literals([
  'khala-public-heuristic',
  'glm-reap-504b-g4-tp4-minp-rp105',
  'glm-reap-504b-g4-tp8-minp-rp105',
  'glm-reap-504b-g4-dual-tp4-minp-rp105',
  'glm-reap-504b-g4-tp4-mtp2-rp105',
  'glm-reap-504b-g4-tp4-65k-fast',
  'glm-reap-504b-g4-tp4-250k-stable',
])
export type GymTerminalBenchProfileRef =
  typeof GymTerminalBenchProfileRef.Type

export const GymTerminalBenchSpeculationMode = S.Literals(['none', 'mtp2'])
export type GymTerminalBenchSpeculationMode =
  typeof GymTerminalBenchSpeculationMode.Type

export const GymTerminalBenchReplicaTopology = S.Literals([
  'single_tp4',
  'single_tp8',
  'dual_tp4',
  'khala_router',
])
export type GymTerminalBenchReplicaTopology =
  typeof GymTerminalBenchReplicaTopology.Type

export const GymTerminalBenchQuantization = S.Literals([
  'nvfp4',
  'router_mixed',
])
export type GymTerminalBenchQuantization =
  typeof GymTerminalBenchQuantization.Type

export const GymTerminalBenchServingProfile = S.Struct({
  profileRef: GymTerminalBenchProfileRef,
  lane: BenchmarkLaneSchema,
  publicLabel: S.String,
  model: GymHarborTerminalBenchModelId,
  modelEndpointRef: S.String,
  sourceModelRef: S.String,
  attribution: S.String,
  hardwareProfile: S.String,
  tensorParallelism: S.Number,
  replicaTopology: GymTerminalBenchReplicaTopology,
  contextWindowTokens: S.Number,
  quantization: GymTerminalBenchQuantization,
  speculationMode: GymTerminalBenchSpeculationMode,
  sampler: S.Struct({
    minP: S.NullOr(S.Number),
    repetitionPenalty: S.Number,
    enableThinking: S.Boolean,
  }),
  caveatRefs: S.Array(S.String),
})
export type GymTerminalBenchServingProfile =
  typeof GymTerminalBenchServingProfile.Type

const HYDRALISK_GLM_ENDPOINT_REF =
  'hydralisk.glm_52_reap_504b.private_openai_compat.v1'
const GLM_SOURCE_MODEL_REF = '0xSero/GLM-5.2-504B'
const GLM_ATTRIBUTION = 'Z.ai GLM-5.2, REAP-pruned keep-168 NVFP4'

export const GYM_TERMINAL_BENCH_PROFILE_CATALOG: Readonly<
  Record<GymTerminalBenchProfileRef, GymTerminalBenchServingProfile>
> = {
  'khala-public-heuristic': {
    profileRef: 'khala-public-heuristic',
    lane: 'khala',
    publicLabel: 'Khala heuristic public route',
    model: KHALA_PUBLIC_MODEL_ID,
    modelEndpointRef: 'openagents.khala.public_openai_compat.v1',
    sourceModelRef: 'openagents/khala',
    attribution: 'OpenAgents Khala orchestrator',
    hardwareProfile: 'khala-router',
    tensorParallelism: 0,
    replicaTopology: 'khala_router',
    contextWindowTokens: 250_000,
    quantization: 'router_mixed',
    speculationMode: 'none',
    sampler: {
      minP: 0.05,
      repetitionPenalty: 1.05,
      enableThinking: false,
    },
    caveatRefs: ['caveat.gym.terminal_bench.public_khala_route'],
  },
  'glm-reap-504b-g4-tp4-minp-rp105': {
    profileRef: 'glm-reap-504b-g4-tp4-minp-rp105',
    lane: 'glm-52',
    publicLabel: 'GLM-5.2 REAP 504B G4 TP4 min-p guardrail',
    model: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
    modelEndpointRef: HYDRALISK_GLM_ENDPOINT_REF,
    sourceModelRef: GLM_SOURCE_MODEL_REF,
    attribution: GLM_ATTRIBUTION,
    hardwareProfile: 'hydralisk-g4-4x-rtx-pro-6000',
    tensorParallelism: 4,
    replicaTopology: 'single_tp4',
    contextWindowTokens: 250_000,
    quantization: 'nvfp4',
    speculationMode: 'none',
    sampler: {
      minP: 0.05,
      repetitionPenalty: 1.05,
      enableThinking: false,
    },
    caveatRefs: ['caveat.gym.terminal_bench.raw_glm_not_public_catalog_model'],
  },
  'glm-reap-504b-g4-tp8-minp-rp105': {
    profileRef: 'glm-reap-504b-g4-tp8-minp-rp105',
    lane: 'glm-52',
    publicLabel: 'GLM-5.2 REAP 504B G4 TP8 min-p guardrail',
    model: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
    modelEndpointRef: HYDRALISK_GLM_ENDPOINT_REF,
    sourceModelRef: GLM_SOURCE_MODEL_REF,
    attribution: GLM_ATTRIBUTION,
    hardwareProfile: 'hydralisk-g4-8x-rtx-pro-6000',
    tensorParallelism: 8,
    replicaTopology: 'single_tp8',
    contextWindowTokens: 250_000,
    quantization: 'nvfp4',
    speculationMode: 'none',
    sampler: {
      minP: 0.05,
      repetitionPenalty: 1.05,
      enableThinking: false,
    },
    caveatRefs: ['caveat.gym.terminal_bench.raw_glm_not_public_catalog_model'],
  },
  'glm-reap-504b-g4-dual-tp4-minp-rp105': {
    profileRef: 'glm-reap-504b-g4-dual-tp4-minp-rp105',
    lane: 'glm-52',
    publicLabel: 'GLM-5.2 REAP 504B dual TP4 replicas',
    model: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
    modelEndpointRef: HYDRALISK_GLM_ENDPOINT_REF,
    sourceModelRef: GLM_SOURCE_MODEL_REF,
    attribution: GLM_ATTRIBUTION,
    hardwareProfile: 'hydralisk-g4-dual-4x-rtx-pro-6000',
    tensorParallelism: 4,
    replicaTopology: 'dual_tp4',
    contextWindowTokens: 250_000,
    quantization: 'nvfp4',
    speculationMode: 'none',
    sampler: {
      minP: 0.05,
      repetitionPenalty: 1.05,
      enableThinking: false,
    },
    caveatRefs: ['caveat.gym.terminal_bench.raw_glm_not_public_catalog_model'],
  },
  'glm-reap-504b-g4-tp4-mtp2-rp105': {
    profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
    lane: 'glm-52',
    publicLabel: 'GLM-5.2 REAP 504B TP4 MTP-2 speculative decoding',
    model: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
    modelEndpointRef: HYDRALISK_GLM_ENDPOINT_REF,
    sourceModelRef: GLM_SOURCE_MODEL_REF,
    attribution: GLM_ATTRIBUTION,
    hardwareProfile: 'hydralisk-g4-4x-rtx-pro-6000',
    tensorParallelism: 4,
    replicaTopology: 'single_tp4',
    contextWindowTokens: 250_000,
    quantization: 'nvfp4',
    speculationMode: 'mtp2',
    sampler: {
      minP: null,
      repetitionPenalty: 1.05,
      enableThinking: false,
    },
    caveatRefs: [
      'caveat.gym.terminal_bench.raw_glm_not_public_catalog_model',
      'caveat.gym.terminal_bench.mtp2_vllm_min_p_disabled',
    ],
  },
  'glm-reap-504b-g4-tp4-65k-fast': {
    profileRef: 'glm-reap-504b-g4-tp4-65k-fast',
    lane: 'glm-52',
    publicLabel: 'GLM-5.2 REAP 504B TP4 65K fast context',
    model: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
    modelEndpointRef: HYDRALISK_GLM_ENDPOINT_REF,
    sourceModelRef: GLM_SOURCE_MODEL_REF,
    attribution: GLM_ATTRIBUTION,
    hardwareProfile: 'hydralisk-g4-4x-rtx-pro-6000',
    tensorParallelism: 4,
    replicaTopology: 'single_tp4',
    contextWindowTokens: 65_000,
    quantization: 'nvfp4',
    speculationMode: 'none',
    sampler: {
      minP: 0.05,
      repetitionPenalty: 1.05,
      enableThinking: false,
    },
    caveatRefs: ['caveat.gym.terminal_bench.raw_glm_not_public_catalog_model'],
  },
  'glm-reap-504b-g4-tp4-250k-stable': {
    profileRef: 'glm-reap-504b-g4-tp4-250k-stable',
    lane: 'glm-52',
    publicLabel: 'GLM-5.2 REAP 504B TP4 250K stable context',
    model: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
    modelEndpointRef: HYDRALISK_GLM_ENDPOINT_REF,
    sourceModelRef: GLM_SOURCE_MODEL_REF,
    attribution: GLM_ATTRIBUTION,
    hardwareProfile: 'hydralisk-g4-4x-rtx-pro-6000',
    tensorParallelism: 4,
    replicaTopology: 'single_tp4',
    contextWindowTokens: 250_000,
    quantization: 'nvfp4',
    speculationMode: 'none',
    sampler: {
      minP: 0.05,
      repetitionPenalty: 1.10,
      enableThinking: false,
    },
    caveatRefs: ['caveat.gym.terminal_bench.raw_glm_not_public_catalog_model'],
  },
}

const decodeProfileRef = S.decodeUnknownSync(GymTerminalBenchProfileRef)
const decodeServingProfile = S.decodeUnknownSync(GymTerminalBenchServingProfile)

export const resolveGymTerminalBenchServingProfile = (
  profileRef: unknown,
): GymTerminalBenchServingProfile => {
  const ref = decodeProfileRef(profileRef)
  return decodeServingProfile(GYM_TERMINAL_BENCH_PROFILE_CATALOG[ref])
}

const defaultProfileRefForLane = (
  lane: BenchmarkLane,
): GymTerminalBenchProfileRef =>
  lane === 'khala'
    ? 'khala-public-heuristic'
    : 'glm-reap-504b-g4-tp4-minp-rp105'

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

export const GymHarborVerifierPlacementRequirement = S.Struct({
  requestedEnvironmentMode: S.Literal('separate'),
  requestedVerifierNetworkMode: S.Literal('no-network'),
  requireDistinctDevice: S.Literal(true),
  requireArtifactHandoff: S.Literal(true),
  requireRewardArtifact: S.Literal(true),
})
export type GymHarborVerifierPlacementRequirement =
  typeof GymHarborVerifierPlacementRequirement.Type

export const GymHarborVerifierPlacementEvidence = S.Struct({
  schemaVersion: S.Literal(GYM_HARBOR_VERIFIER_PLACEMENT_SCHEMA),
  environmentMode: S.Literal('separate'),
  agentHostRef: S.String,
  verifierHostRef: S.String,
  agentDeviceRef: S.String,
  verifierDeviceRef: S.String,
  verifierNetworkMode: S.Literal('no-network'),
  artifactHandoffRefs: S.Array(S.String),
  rewardArtifactRef: S.String,
  rewardReadFrom: S.Literal('verifier_artifact'),
})
export type GymHarborVerifierPlacementEvidence =
  typeof GymHarborVerifierPlacementEvidence.Type

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
  profileRef: GymTerminalBenchProfileRef,
  servingProfile: GymTerminalBenchServingProfile,
  environmentRef: S.Literal('terminal-bench'),
  taskSetRef: S.String,
  retainedPublicTaskRefs: S.Array(S.String),
  harborDataset: S.Literal('terminal-bench@2.0'),
  harborDatasetCliRef: S.Literal(TERMINAL_BENCH_HARBOR_DATASET_CLI_REF),
  harnessRef: S.Literal('hydralisk.harbor.terminal_bench.cli_artifact.v1'),
  runner: S.Literal('harbor'),
  agent: HarborTerminalBenchAgent,
  model: GymHarborTerminalBenchModelId,
  modelEndpointRef: S.String,
  nConcurrent: S.Number.check(S.isBetween({ minimum: 1, maximum: 64 })),
  maxAttempts: S.Number.check(S.isBetween({ minimum: 1, maximum: 1000 })),
  ownerApprovalRef: S.NullOr(S.String),
  command: GymHarborTerminalBenchCommandSpec,
  artifacts: GymHarborTerminalBenchArtifactPolicy,
  verifierPlacement: GymHarborVerifierPlacementRequirement,
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
  verifierPlacement: GymHarborVerifierPlacementEvidence,
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
    model: GymHarborTerminalBenchModelId,
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
    minP: S.NullOr(S.Number),
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
  profileRef: GymTerminalBenchProfileRef,
  environmentRef: S.Literal('terminal-bench'),
  summarySchema: S.Literal(HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA),
  summaryArtifactRef: S.String,
  atifTraceRef: S.NullOr(S.String),
  datasetRef: S.Literal('terminal-bench@2.0'),
  runner: S.Literal('harbor'),
  agent: HarborTerminalBenchAgent,
  model: GymHarborTerminalBenchModelId,
  publicSafe: S.Literal(true),
  publicClaimEligible: S.Literal(false),
  decisionGradeReportReady: S.Literal(false),
  verifierPlacementVerified: S.Literal(true),
  environmentMode: S.Literal('separate'),
  agentHostRef: S.String,
  verifierHostRef: S.String,
  verifierNetworkMode: S.Literal('no-network'),
  artifactHandoffRefs: S.Array(S.String),
  rewardArtifactRef: S.String,
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
      'invalid_verifier_placement',
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

export type GymHarborVerifierPlacementCheck = Readonly<{
  valid: boolean
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

const makeJobRef = (
  experiment: GymExperiment,
  profileRef: GymTerminalBenchProfileRef,
): string =>
  `job.gym.harbor_terminal_bench.${safeRefSegment(experiment.id)}.${fnv1a32(
    stableJson({
      environment: experiment.environment,
      lanes: experiment.policy.fanout.lanes,
      profileRef,
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

const assertTerminalBenchExperiment = (
  experiment: GymExperiment,
  definition: GymEnvironmentDefinition,
  profile: GymTerminalBenchServingProfile,
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
    !['khala', 'glm-52'].includes(experiment.policy.fanout.lanes[0] ?? '')
  ) {
    throw new GymHarborDispatchError({
      reason: 'unsupported_lane',
      message:
        'Hydralisk Harbor Terminal-Bench dispatch requires exactly one supported benchmark lane.',
    })
  }
  if (profile.lane !== experiment.policy.fanout.lanes[0]) {
    throw new GymHarborDispatchError({
      reason: 'unsupported_lane',
      message:
        'Hydralisk Harbor Terminal-Bench profile must match the selected benchmark lane.',
    })
  }
}

const commandForJob = (input: {
  agent: HarborTerminalBenchAgent
  model: GymHarborTerminalBenchModelId
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
    input.model,
    '--n-concurrent',
    `${input.nConcurrent}`,
  ],
})

export const buildGymHarborTerminalBenchJobSpec = (
  experiment: GymExperiment,
  input: Readonly<{
    agent?: HarborTerminalBenchAgent | undefined
    ownerApprovalRef?: string | undefined
    profileRef?: GymTerminalBenchProfileRef | undefined
    registry?: GymEnvironmentRegistry | undefined
  }> = {},
): Readonly<{
  compiled: CompiledGymExperiment
  job: GymHarborTerminalBenchJobSpec
}> => {
  const registry = input.registry ?? GYM_ENVIRONMENT_REGISTRY
  const definition = terminalBenchDefinition(registry)
  const selectedLane = experiment.policy.fanout.lanes[0]
  const profile = resolveGymTerminalBenchServingProfile(
    input.profileRef ??
      (selectedLane === undefined
        ? 'khala-public-heuristic'
        : defaultProfileRefForLane(selectedLane)),
  )
  assertTerminalBenchExperiment(experiment, definition, profile)
  const compiled = compileGymExperiment(experiment, registry)
  const agent = input.agent ?? 'terminus-2'
  const job = decodeJobSpec({
    schemaVersion: GYM_HARBOR_TERMINAL_BENCH_JOB_SPEC_SCHEMA,
    jobRef: makeJobRef(experiment, profile.profileRef),
    experimentId: experiment.id,
    configId: compiled.matrixConfig.id,
    profileRef: profile.profileRef,
    servingProfile: profile,
    environmentRef: 'terminal-bench',
    taskSetRef: definition.taskSet.ref,
    retainedPublicTaskRefs: definition.taskSet.publicSafeTaskRefs,
    harborDataset: 'terminal-bench@2.0',
    harborDatasetCliRef: TERMINAL_BENCH_HARBOR_DATASET_CLI_REF,
    harnessRef: 'hydralisk.harbor.terminal_bench.cli_artifact.v1',
    runner: 'harbor',
    agent,
    model: profile.model,
    modelEndpointRef: profile.modelEndpointRef,
    nConcurrent: experiment.policy.fanout.concurrency,
    maxAttempts: experiment.samplesPerCell,
    ownerApprovalRef: input.ownerApprovalRef ?? null,
    command: commandForJob({
      agent,
      model: profile.model,
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
    verifierPlacement: {
      requestedEnvironmentMode: 'separate',
      requestedVerifierNetworkMode: 'no-network',
      requireDistinctDevice: true,
      requireArtifactHandoff: true,
      requireRewardArtifact: true,
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

export const checkGymHarborVerifierPlacement = (
  placement: GymHarborVerifierPlacementEvidence,
): GymHarborVerifierPlacementCheck => {
  const violations: Array<string> = []
  if (placement.environmentMode !== 'separate') {
    violations.push('environment_mode_not_separate')
  }
  if (placement.verifierNetworkMode !== 'no-network') {
    violations.push('verifier_network_not_no_network')
  }
  if (placement.agentHostRef.trim() === '') {
    violations.push('agent_host_ref_empty')
  }
  if (placement.verifierHostRef.trim() === '') {
    violations.push('verifier_host_ref_empty')
  }
  if (
    placement.agentHostRef.trim() !== '' &&
    placement.agentHostRef === placement.verifierHostRef
  ) {
    violations.push('agent_and_verifier_same_host')
  }
  if (placement.agentDeviceRef.trim() === '') {
    violations.push('agent_device_ref_empty')
  }
  if (placement.verifierDeviceRef.trim() === '') {
    violations.push('verifier_device_ref_empty')
  }
  if (
    placement.agentDeviceRef.trim() !== '' &&
    placement.agentDeviceRef === placement.verifierDeviceRef
  ) {
    violations.push('agent_and_verifier_same_device')
  }
  if (placement.artifactHandoffRefs.length === 0) {
    violations.push('artifact_handoff_missing')
  }
  if (placement.rewardArtifactRef.trim() === '') {
    violations.push('reward_artifact_ref_empty')
  }
  if (placement.rewardReadFrom !== 'verifier_artifact') {
    violations.push('reward_not_read_from_verifier_artifact')
  }
  return { valid: violations.length === 0, violations }
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
  const placementCheck = checkGymHarborVerifierPlacement(
    input.dispatch.verifierPlacement,
  )
  if (!placementCheck.valid) {
    throw new GymHarborDispatchError({
      reason: 'invalid_verifier_placement',
      message:
        'Hydralisk Terminal-Bench dispatch did not prove distinct-device verifier placement.',
    })
  }
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
    profileRef: input.job.profileRef,
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
    verifierPlacementVerified: true,
    environmentMode: input.dispatch.verifierPlacement.environmentMode,
    agentHostRef: input.dispatch.verifierPlacement.agentHostRef,
    verifierHostRef: input.dispatch.verifierPlacement.verifierHostRef,
    verifierNetworkMode: input.dispatch.verifierPlacement.verifierNetworkMode,
    artifactHandoffRefs: input.dispatch.verifierPlacement.artifactHandoffRefs,
    rewardArtifactRef: input.dispatch.verifierPlacement.rewardArtifactRef,
    acceptedOutcomes: input.summary.counts.solved,
    attemptedOutcomes: input.summary.counts.properlyAttempted,
    totalTasks: input.summary.counts.total,
    fullDenominatorSolved: input.summary.rates.fullDenominatorSolved,
    passAtN: input.summary.rates.passAtN,
    caveats: [
      ...input.job.servingProfile.caveatRefs,
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
    profileRef?: GymTerminalBenchProfileRef | undefined
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

export type GymHarborTerminalBenchDispatchLane = Extract<
  BenchmarkLane,
  'khala' | 'glm-52'
>
