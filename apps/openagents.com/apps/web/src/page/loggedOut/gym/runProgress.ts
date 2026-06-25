// Browser-side live Gym / Harbor run progress (#6261).
//
// Mirrors the Worker's public-safe `openagents.gym.run_progress.v1` projection
// (`workers/api/src/inference/gym/run-progress.ts`) so the `/gym` follow-along
// view can render an active run with the existing three-effect run vocabulary
// plus an accessible text/table mirror. The Worker is the authority; this file
// holds the browser type mirror and the three-effect option adapter ONLY. It
// carries no seeded run: until a real Harbor/Khala run is ingested into the
// Worker, the `/gym` follow-along renders an honest empty state, never a
// fabricated run.
//
// Honesty discipline carries over verbatim: a progress object is ALWAYS
// `decisionGrade:false`, partial phases are `inProgress:true`, pass-rate is over
// COMPLETED tasks (never the official denominator), and `not_measured` is null,
// never 0.
import type {
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunContributorDefinition,
  TrainingRunMotionKind,
  TrainingRunNodeDefinition,
  TrainingRunOperatorSignalDefinition,
  TrainingRunVisualizationOptions,
  TrainingRunWorldItemDefinition,
} from '@openagentsinc/three-effect/core'

type TrainingRunBeamStyle = 'crackling_arc' | 'flow'

export const GYM_RUN_PROGRESS_SCHEMA =
  'openagents.gym.run_progress.v1' as const

export type GymRunPhase =
  | 'queued'
  | 'running'
  | 'completed'
  | 'cancelled'
  | 'errored'

export type GymRunPublication = 'local_only' | 'web_authorized'

export type GymRunProgressProfile = Readonly<{
  profileRef: string
  publicLabel: string
  model: string
  attribution: string
  hardwareProfile: string
  contextWindowTokens: number
}>

export type GymRunProgressCounts = Readonly<{
  officialDenominator: number
  completed: number
  completedPassed: number
  completedFailed: number
  running: number
  pending: number
  error: number
  cancelled: number
}>

export type GymRunProgressTokens = Readonly<{
  promptTokens: number | null
  completionTokens: number | null
  totalTokens: number | null
}>

export type GymRunProgress = Readonly<{
  schemaVersion: typeof GYM_RUN_PROGRESS_SCHEMA
  runRef: string
  jobRef: string
  configId: string
  environmentRef: 'terminal-bench'
  datasetRef: 'terminal-bench@2.0'
  runner: 'harbor'
  agent: string
  profile: GymRunProgressProfile
  phase: GymRunPhase
  decisionGrade: false
  inProgress: boolean
  publication: GymRunPublication
  counts: GymRunProgressCounts
  passRateOverCompleted: number | null
  completionFraction: number
  tokens: GymRunProgressTokens
  elapsedMs: number | null
  lastUpdatedAt: string
  caveatRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

// ---------------------------------------------------------------------------
// Formatting helpers (shared by the text/table mirror).
// ---------------------------------------------------------------------------

export const formatRunProgressPercent = (value: number | null): string =>
  value === null ? 'not measured' : `${(value * 100).toFixed(1)}%`

export const formatRunProgressCount = (value: number | null): string =>
  value === null ? 'not measured' : value.toLocaleString('en-US')

export const formatRunProgressDuration = (value: number | null): string => {
  if (value === null) {
    return 'not measured'
  }
  const totalSeconds = Math.round(value / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes <= 0 ? `${seconds}s` : `${minutes}m ${seconds}s`
}

export const runPhaseLabel = (progress: GymRunProgress): string =>
  progress.inProgress
    ? `in progress · ${progress.phase}`
    : `finished · ${progress.phase} (not decision-grade)`

// ---------------------------------------------------------------------------
// Three-effect run-field adapter.
// ---------------------------------------------------------------------------
//
// Reuses the same `trainingRunView` vocabulary the #6257 Terminal-Bench replay
// uses (run hub → outcome buckets → verifier/report nodes, beams, bursts), but
// drives it from the LIVE counts so the field animates progress (a fan-out into
// passed / failed / running / pending, with a cost/elapsed world item).

const bucketNode = (
  id: string,
  label: string,
  detail: string,
  status: TrainingRunNodeDefinition['status'],
  position: TrainingRunNodeDefinition['position'],
): TrainingRunNodeDefinition => ({
  id,
  label,
  detail,
  role: 'run',
  status,
  position,
  connectedTo: ['report:progress'],
})

const operatorSignals = (
  progress: GymRunProgress,
): ReadonlyArray<TrainingRunOperatorSignalDefinition> => [
  {
    id: 'run-progress.in-progress',
    label: 'in progress',
    state: progress.inProgress ? 'info' : 'success',
    detail: runPhaseLabel(progress),
  },
  {
    id: 'run-progress.decision-grade',
    label: 'decision grade',
    state: 'info',
    detail: 'false — partial run, never a final benchmark claim',
  },
  {
    id: 'run-progress.publication',
    label: 'publication',
    state: progress.publication === 'web_authorized' ? 'success' : 'info',
    detail:
      progress.publication === 'web_authorized'
        ? 'authorized for web'
        : 'local only — awaiting authorization',
  },
]

const worldItems = (
  progress: GymRunProgress,
): ReadonlyArray<TrainingRunWorldItemDefinition> => [
  {
    id: 'bulletin:run-progress',
    kind: 'bulletin_board',
    label: 'Run progress board',
    title: progress.profile.publicLabel,
    detail: 'Live Terminal-Bench follow-along',
    position: [0, 2.72, 0],
    status: 'active',
    lines: [
      `${progress.counts.completed} / ${progress.counts.officialDenominator} completed`,
      `pass rate over completed: ${formatRunProgressPercent(
        progress.passRateOverCompleted,
      )}`,
      `tokens: ${formatRunProgressCount(progress.tokens.totalTokens)}`,
      runPhaseLabel(progress),
    ],
    sourceRefs: [progress.runRef, ...progress.caveatRefs],
  },
]

const bucketContributor = (
  id: string,
  phase: number,
  lifecycleState: TrainingRunContributorDefinition['lifecycleState'],
): TrainingRunContributorDefinition => ({
  id,
  label: id,
  lifecycleState,
  phase,
})

export const runProgressVisualizationOptions = (
  progress: GymRunProgress,
): TrainingRunVisualizationOptions => {
  const { counts } = progress
  const nodes: ReadonlyArray<TrainingRunNodeDefinition> = [
    {
      id: 'run:progress',
      label: 'Terminal-Bench run',
      detail: `${counts.completed}/${counts.officialDenominator} completed; decision-grade false`,
      role: 'run',
      status: 'active',
      position: [0, 1.4, 0],
      connectedTo: [
        'bucket:passed',
        'bucket:failed',
        'bucket:running',
        'bucket:pending',
      ],
    },
    bucketNode(
      'bucket:passed',
      'passed',
      `${counts.completedPassed} accepted`,
      'verified',
      [-3.0, -0.4, 0],
    ),
    bucketNode(
      'bucket:failed',
      'failing',
      `${counts.completedFailed} failed`,
      'blocked',
      [-1.0, -0.4, 0],
    ),
    bucketNode(
      'bucket:running',
      'running',
      `${counts.running} in flight`,
      'sync',
      [1.0, -0.4, 0],
    ),
    bucketNode(
      'bucket:pending',
      'pending',
      `${counts.pending} queued`,
      'queued',
      [3.0, -0.4, 0],
    ),
    {
      id: 'report:progress',
      label: 'progress projection',
      detail: progress.schemaVersion,
      role: 'receipt',
      status: 'sealed',
      position: [0, -2.4, 0],
    },
  ]

  const beam = (
    toId: string,
    style: TrainingRunBeamStyle,
    motionKind: TrainingRunMotionKind,
  ): TrainingRunBeamDefinition => ({
    fromId: 'run:progress',
    toId,
    style,
    motionKind,
    simulated: true,
    sourceRefs: [progress.runRef],
    generatedAt: progress.lastUpdatedAt,
  })

  const burst = (atId: string): TrainingRunBurstDefinition => ({
    atId,
    motionKind: 'replay_verified',
    simulated: true,
    sourceRefs: [progress.runRef],
    generatedAt: progress.lastUpdatedAt,
  })

  return {
    backgroundColor: 0x030609,
    cameraMode: 'orthographic_map',
    controller: 'none',
    nodes,
    contributors: [
      bucketContributor('passed', 0, 'active'),
      bucketContributor('failed', 0.33, 'sync_reentry'),
      bucketContributor('running', 0.66, 'warmup'),
      bucketContributor('pending', 1, 'warmup'),
    ],
    lossCurve: [
      { step: 0, validationLoss: 0.9 },
      { step: 1, validationLoss: 0.62 },
      { step: 2, validationLoss: 0.41 },
      { step: 3, validationLoss: 0.3 },
    ],
    operatorSignals: operatorSignals(progress),
    promiseSignals: [],
    entities: [],
    worldItems: worldItems(progress),
    remoteAvatars: [],
    beams: [
      beam('bucket:passed', 'crackling_arc', 'replay_verified'),
      beam('bucket:failed', 'flow', 'replay_rejected'),
      beam('bucket:running', 'flow', 'assignment'),
      beam('bucket:pending', 'flow', 'assignment'),
    ],
    bursts: counts.completedPassed > 0 ? [burst('bucket:passed')] : [],
    motionPolicy: {
      structuralEdges: 'animated',
      ambient: 'animated',
      evidence: 'required',
      bursts: 'loop',
    },
    sceneChrome: {
      contributorOrbit: 'visible',
      lossPanel: 'visible',
      staleRing: 'hidden',
      statusChart: 'visible',
    },
    stageNodeGlyph: 'compact_gate',
    worldLabelDensity: 'compact',
    keyboardTargeting: { enabled: true },
    pulseSpeed: 0.7,
  }
}
