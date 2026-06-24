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
  const expectedCellCount =
    Array.dedupe(experiment.fanout.lanes).length *
    Array.dedupe(experiment.coordinators).length *
    Array.dedupe(experiment.shapes).length *
    experiment.samplesPerCell

  return {
    reportRef: 'gym.fixture.decision-suite.phase0',
    viewerSchema: 'openagents.gym.fixture_report.v1',
    generatedAt: '2026-06-24T00:00:00.000Z',
    expectedCellCount,
    executedCellCount: expectedCellCount,
    skippedCellCount: 0,
    publicSafety: 'passed',
    metrics: {
      acceptedOutcomeRate: 1,
      meanCostUsd: 0,
      p50WallMs: 184,
    },
  }
}

export const runGymFixture = (model: GymModel): GymModel =>
  GymModel({
    ...model,
    result: buildGymFixtureResult(model.experiment),
  })
