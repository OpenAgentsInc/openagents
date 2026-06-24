import { Array, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

export const GymEnvironmentRef = S.Literals(['bundled-decision-suite-v1'])
export type GymEnvironmentRef = typeof GymEnvironmentRef.Type

export const GymLaneRef = S.Literals([
  'pylon-whole-small',
  'psionic-shard-wan',
  'provider-baseline',
])
export type GymLaneRef = typeof GymLaneRef.Type

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

export const GymModuleCompositionMode = S.Literals([
  'none',
  'starter-catalog',
])
export type GymModuleCompositionMode =
  typeof GymModuleCompositionMode.Type

export const GymCoordinatorCandidateRef = S.Literals([
  'heuristic-v0',
  'trinity-v1',
  'conductor-v2',
])
export type GymCoordinatorCandidateRef =
  typeof GymCoordinatorCandidateRef.Type

export const GymReasoningEffort = S.Literals(['off', 'low', 'medium', 'high'])
export type GymReasoningEffort = typeof GymReasoningEffort.Type

export const GymTransport = S.Literals(['batch', 'streaming'])
export type GymTransport = typeof GymTransport.Type

export const GymSequenceShapeRef = S.Literals([
  'single-turn',
  'short-multi-turn',
  'long-context',
])
export type GymSequenceShapeRef = typeof GymSequenceShapeRef.Type

export const PublicGymSamplingSettings = S.Struct({
  temperature: S.Number,
  reasoningEffort: GymReasoningEffort,
  maxTokens: S.Number,
  transport: GymTransport,
})
export type PublicGymSamplingSettings =
  typeof PublicGymSamplingSettings.Type

export const PublicGymExperiment = S.Struct({
  environment: GymEnvironmentRef,
  fanout: S.Struct({
    lanes: S.Array(GymLaneRef),
    mode: GymFanoutMode,
    concurrency: S.Number,
  }),
  tools: GymToolSetRef,
  modules: S.Struct({
    mode: GymModuleCompositionMode,
    signatureRefs: S.Array(S.String),
    moduleRefs: S.Array(S.String),
  }),
  coordinators: S.Array(GymCoordinatorCandidateRef),
  sampling: PublicGymSamplingSettings,
  shapes: S.Array(GymSequenceShapeRef),
  samplesPerCell: S.Number,
  budget: S.Struct({
    seam: S.Literal('fixture'),
    spendCapMsat: S.Number,
    maxBillableSamples: S.Number,
  }),
})
export type PublicGymExperiment = typeof PublicGymExperiment.Type

export const PublicGymMetricSummary = S.Struct({
  label: S.String,
  mean: S.Number,
  p50: S.Number,
  p90: S.Number,
  p99: S.Number,
  measuredSampleCount: S.Number,
  notMeasuredDropped: S.Number,
})
export type PublicGymMetricSummary = typeof PublicGymMetricSummary.Type

export const PublicGymReportViewer = S.Struct({
  decisionGrade: S.Literal(false),
  illustrativeNotice: S.String,
  latency: S.Array(PublicGymMetricSummary),
  verificationRate: S.Number,
  cacheHitRate: S.Number,
  costPerAcceptedOutcomeUsd: S.NullOr(S.Number),
  nullCostFinding: S.String,
  zeroAcceptedFinding: S.Struct({
    group: S.String,
    costPerAcceptedOutcomeUsd: S.NullOr(S.Number),
    finding: S.Literal('money spent, nothing accepted'),
  }),
})
export type PublicGymReportViewer = typeof PublicGymReportViewer.Type

export const PublicGymSceneLaneStatus = S.Literals([
  'test_passed',
  'skipped_unavailable',
])
export type PublicGymSceneLaneStatus = typeof PublicGymSceneLaneStatus.Type

export const PublicGymSceneLane = S.Struct({
  lane: GymLaneRef,
  label: S.String,
  status: PublicGymSceneLaneStatus,
  attemptedCells: S.Number,
  acceptedCells: S.Number,
  skippedCells: S.Number,
  verdictBeam: S.Boolean,
})
export type PublicGymSceneLane = typeof PublicGymSceneLane.Type

export const PublicGymScene = S.Struct({
  schema: S.Literal('openagents.gym.fixture_scene.v1'),
  durationMs: S.Number,
  simulatedCostMsat: S.Number,
  billedCostMsat: S.Literal(0),
  lanes: S.Array(PublicGymSceneLane),
})
export type PublicGymScene = typeof PublicGymScene.Type

export const PublicGymFixtureResult = S.Struct({
  reportRef: S.String,
  viewerSchema: S.Literal('openagents.gym.fixture_report.v1'),
  generatedAt: S.String,
  expectedCellCount: S.Number,
  executedCellCount: S.Number,
  skippedCellCount: S.Number,
  publicSafety: S.Literal('passed'),
  metrics: S.Struct({
    acceptedOutcomeRate: S.Number,
    meanCostUsd: S.Number,
    p50WallMs: S.Number,
  }),
  reportViewer: PublicGymReportViewer,
  scene: PublicGymScene,
})
export type PublicGymFixtureResult = typeof PublicGymFixtureResult.Type

export const GymModel = ts('LoggedOutGymModel', {
  experiment: PublicGymExperiment,
  result: S.NullOr(PublicGymFixtureResult),
})
export type GymModel = typeof GymModel.Type

export const laneOptions: ReadonlyArray<{
  readonly label: string
  readonly value: GymLaneRef
}> = [
  { label: 'Pylon whole-small', value: 'pylon-whole-small' },
  { label: 'Psionic shard WAN', value: 'psionic-shard-wan' },
  { label: 'Provider baseline', value: 'provider-baseline' },
]

export const coordinatorOptions: ReadonlyArray<{
  readonly label: string
  readonly value: GymCoordinatorCandidateRef
}> = [
  { label: 'Heuristic v0', value: 'heuristic-v0' },
  { label: 'Trinity v1', value: 'trinity-v1' },
  { label: 'Conductor v2', value: 'conductor-v2' },
]

export const sequenceShapeOptions: ReadonlyArray<{
  readonly label: string
  readonly value: GymSequenceShapeRef
}> = [
  { label: 'Single turn', value: 'single-turn' },
  { label: 'Short multi-turn', value: 'short-multi-turn' },
  { label: 'Long context', value: 'long-context' },
]

export const gymLaneLabel = (lane: GymLaneRef): string =>
  laneOptions.find(option => option.value === lane)?.label ?? lane

export const initGymModel = (): GymModel =>
  GymModel({
    experiment: {
      environment: 'bundled-decision-suite-v1',
      fanout: {
        lanes: ['pylon-whole-small', 'psionic-shard-wan', 'provider-baseline'],
        mode: 'verifier-pick',
        concurrency: 4,
      },
      tools: 'khala-fixture-tools',
      modules: {
        mode: 'starter-catalog',
        signatureRefs: ['program-signature:khala-fixture-decision-suite'],
        moduleRefs: ['starter-module:artifact-verifier'],
      },
      coordinators: ['heuristic-v0', 'trinity-v1', 'conductor-v2'],
      sampling: {
        temperature: 0.2,
        reasoningEffort: 'off',
        maxTokens: 2048,
        transport: 'streaming',
      },
      shapes: ['single-turn', 'short-multi-turn'],
      samplesPerCell: 5,
      budget: {
        seam: 'fixture',
        spendCapMsat: 0,
        maxBillableSamples: 0,
      },
    },
    result: null,
  })

const clampNumber = (
  value: string,
  fallback: number,
  minimum: number,
  maximum: number,
): number => {
  const parsed = Number.parseFloat(value)

  if (!Number.isFinite(parsed)) {
    return fallback
  }

  return Math.min(maximum, Math.max(minimum, parsed))
}

const toggleAtLeastOne = <A>(
  items: ReadonlyArray<A>,
  value: A,
): ReadonlyArray<A> =>
  items.includes(value)
    ? items.length === 1
      ? items
      : items.filter(item => item !== value)
    : [...items, value]

const modulesForMode = (
  mode: GymModuleCompositionMode,
): PublicGymExperiment['modules'] =>
  mode === 'starter-catalog'
    ? {
        mode,
        signatureRefs: ['program-signature:khala-fixture-decision-suite'],
        moduleRefs: ['starter-module:artifact-verifier'],
      }
    : { mode, signatureRefs: [], moduleRefs: [] }

export const toggleGymLane = (
  model: GymModel,
  lane: GymLaneRef,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      fanout: {
        ...model.experiment.fanout,
        lanes: toggleAtLeastOne(model.experiment.fanout.lanes, lane),
      },
    },
  })

export const toggleGymCoordinator = (
  model: GymModel,
  candidate: GymCoordinatorCandidateRef,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      coordinators: toggleAtLeastOne(model.experiment.coordinators, candidate),
    },
  })

export const toggleGymSequenceShape = (
  model: GymModel,
  shape: GymSequenceShapeRef,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      shapes: toggleAtLeastOne(model.experiment.shapes, shape),
    },
  })

export const setGymFanoutMode = (
  model: GymModel,
  mode: GymFanoutMode,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      fanout: { ...model.experiment.fanout, mode },
    },
  })

export const setGymToolSet = (
  model: GymModel,
  tools: GymToolSetRef,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: { ...model.experiment, tools },
  })

export const setGymModuleComposition = (
  model: GymModel,
  mode: GymModuleCompositionMode,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: { ...model.experiment, modules: modulesForMode(mode) },
  })

export const setGymConcurrency = (model: GymModel, value: string): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      fanout: {
        ...model.experiment.fanout,
        concurrency: Math.round(
          clampNumber(value, model.experiment.fanout.concurrency, 1, 8),
        ),
      },
    },
  })

export const setGymTemperature = (model: GymModel, value: string): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      sampling: {
        ...model.experiment.sampling,
        temperature: clampNumber(
          value,
          model.experiment.sampling.temperature,
          0,
          2,
        ),
      },
    },
  })

export const setGymReasoningEffort = (
  model: GymModel,
  reasoningEffort: GymReasoningEffort,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      sampling: { ...model.experiment.sampling, reasoningEffort },
    },
  })

export const setGymMaxTokens = (model: GymModel, value: string): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      sampling: {
        ...model.experiment.sampling,
        maxTokens: Math.round(
          clampNumber(value, model.experiment.sampling.maxTokens, 128, 8192),
        ),
      },
    },
  })

export const setGymTransport = (
  model: GymModel,
  transport: GymTransport,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      sampling: { ...model.experiment.sampling, transport },
    },
  })

export const setGymSamplesPerCell = (
  model: GymModel,
  value: string,
): GymModel =>
  GymModel({
    ...model,
    result: null,
    experiment: {
      ...model.experiment,
      samplesPerCell: Math.round(
        clampNumber(value, model.experiment.samplesPerCell, 1, 25),
      ),
    },
  })

export const buildGymFixtureResult = (
  experiment: PublicGymExperiment,
): PublicGymFixtureResult => {
  const lanes = Array.dedupe(experiment.fanout.lanes)
  const coordinators = Array.dedupe(experiment.coordinators)
  const shapes = Array.dedupe(experiment.shapes)
  const cellsPerLane =
    coordinators.length * shapes.length * experiment.samplesPerCell
  const unavailableLane: GymLaneRef = 'psionic-shard-wan'
  const expectedCellCount =
    lanes.length * coordinators.length * shapes.length * experiment.samplesPerCell
  const skippedCellCount = lanes.includes(unavailableLane) ? cellsPerLane : 0
  const executedCellCount = expectedCellCount - skippedCellCount
  const acceptedOutcomeRate =
    executedCellCount === 0 ? 0 : Math.min(1, executedCellCount / expectedCellCount)
  const simulatedCostMsat = executedCellCount * 21

  const latency = (
    label: string,
    mean: number,
    p50: number,
    p90: number,
    p99: number,
  ): PublicGymMetricSummary => ({
    label,
    mean,
    p50,
    p90,
    p99,
    measuredSampleCount: executedCellCount,
    notMeasuredDropped: skippedCellCount,
  })

  return {
    reportRef: 'gym.fixture.decision-suite.phase0',
    viewerSchema: 'openagents.gym.fixture_report.v1',
    generatedAt: '2026-06-24T00:00:00.000Z',
    expectedCellCount,
    executedCellCount,
    skippedCellCount,
    publicSafety: 'passed',
    metrics: {
      acceptedOutcomeRate,
      meanCostUsd: 0,
      p50WallMs: 184,
    },
    reportViewer: {
      decisionGrade: false,
      illustrativeNotice:
        'Fixture report only: synthetic public-safe aggregates, no provider accounts, no billing, not decision-grade.',
      latency: [
        latency('TTFT', 142, 128, 176, 211),
        latency('Wall clock', 184, 174, 231, 284),
        latency('Perceived TPS', 44.2, 42, 52, 57),
        latency('Inter-token latency', 22.6, 21, 28, 34),
      ],
      verificationRate: acceptedOutcomeRate,
      cacheHitRate: 0.37,
      costPerAcceptedOutcomeUsd: null,
      nullCostFinding:
        'Fixture seam has zero billed spend, so cost-per-accepted-outcome is not a real dollar figure.',
      zeroAcceptedFinding: {
        group: 'fixture.zero-accepted-edge',
        costPerAcceptedOutcomeUsd: null,
        finding: 'money spent, nothing accepted',
      },
    },
    scene: {
      schema: 'openagents.gym.fixture_scene.v1',
      durationMs: 2400,
      simulatedCostMsat,
      billedCostMsat: 0,
      lanes: lanes.map(
        (lane): PublicGymSceneLane =>
          lane === unavailableLane
            ? {
                lane,
                label: gymLaneLabel(lane),
                status: 'skipped_unavailable',
                attemptedCells: cellsPerLane,
                acceptedCells: 0,
                skippedCells: cellsPerLane,
                verdictBeam: false,
              }
            : {
                lane,
                label: gymLaneLabel(lane),
                status: 'test_passed',
                attemptedCells: cellsPerLane,
                acceptedCells: cellsPerLane,
                skippedCells: 0,
                verdictBeam: true,
              },
      ),
    },
  }
}

export const runGymFixture = (model: GymModel): GymModel =>
  GymModel({
    ...model,
    result: buildGymFixtureResult(model.experiment),
  })
