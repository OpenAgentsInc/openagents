import type {
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunContributorDefinition,
  TrainingRunNodeDefinition,
  TrainingRunNodeStatus,
  TrainingRunOperatorSignalDefinition,
  TrainingRunVector,
  TrainingRunVisualizationOptions,
  TrainingRunWorldItemDefinition,
} from '@openagentsinc/three-effect/core'

export type TerminalBenchRunLaneState =
  | 'accepted'
  | 'failing'
  | 'not_started'

export type TerminalBenchRunLane = Readonly<{
  profileRef: string
  label: string
  model: string
  state: TerminalBenchRunLaneState
  acceptedTasks: number
  failingTasks: number
  notStartedTasks: number
  totalTasks: number
  costBasisMsat: number | null
  ttftMs: number | null
  perceivedTps: number | null
  aggregateTps: number | null
  verifierDeviceRef: string | null
  producerDeviceRef: string | null
  distinctVerifierDevice: boolean
  caveatRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
}>

export type TerminalBenchVisualReplay = Readonly<{
  schemaVersion: 'openagents.gym.terminal_bench_visual_replay.v1'
  replayRef: string
  generatedAt: string
  datasetRef: 'terminal-bench@2.0'
  taskSetLabel: string
  officialTotalTasks: number
  externalClaim: Readonly<{
    label: string
    claimedFullDenominatorSolveRate: number
    sourceRefs: ReadonlyArray<string>
    caveatRefs: ReadonlyArray<string>
  }>
  lanes: ReadonlyArray<TerminalBenchRunLane>
  decisionGrade: false
  publicSafe: true
  rawArtifactsIncluded: false
  caveatRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  deferredIntegration: Readonly<{
    surface: 'autopilot_verse'
    state: 'deferred'
    note: string
  }>
}>

// No fixture replay is exported. The comparison/visualizer renders only from a
// real published Terminal-Bench report ingested into the Worker; until one
// exists, the `/gym` page shows an honest empty state. The types and the
// `terminalBenchVisualizationOptions` adapter below stay so a real replay can be
// projected into the three-effect field without re-deriving the shape.

export type TerminalBenchReplayTotals = Readonly<{
  acceptedTasks: number
  failingTasks: number
  notStartedTasks: number
  totalTasks: number
  measuredLaneCount: number
  totalCostBasisMsat: number
}>

export const terminalBenchReplayTotals = (
  replay: TerminalBenchVisualReplay,
): TerminalBenchReplayTotals =>
  replay.lanes.reduce(
    (totals, lane) => ({
      acceptedTasks: totals.acceptedTasks + lane.acceptedTasks,
      failingTasks: totals.failingTasks + lane.failingTasks,
      notStartedTasks: totals.notStartedTasks + lane.notStartedTasks,
      totalTasks: totals.totalTasks + lane.totalTasks,
      measuredLaneCount:
        totals.measuredLaneCount +
        (lane.ttftMs !== null || lane.perceivedTps !== null ? 1 : 0),
      totalCostBasisMsat:
        totals.totalCostBasisMsat + (lane.costBasisMsat ?? 0),
    }),
    {
      acceptedTasks: 0,
      failingTasks: 0,
      notStartedTasks: 0,
      totalTasks: 0,
      measuredLaneCount: 0,
      totalCostBasisMsat: 0,
    },
  )

export const terminalBenchLaneRate = (lane: TerminalBenchRunLane): number =>
  lane.totalTasks <= 0 ? 0 : lane.acceptedTasks / lane.totalTasks

export const formatTerminalBenchPercent = (value: number): string =>
  `${(value * 100).toFixed(1)}%`

export const formatTerminalBenchMetric = (
  value: number | null,
  suffix: string,
): string => (value === null ? 'not measured' : `${value}${suffix}`)

const laneNodeStatus = (lane: TerminalBenchRunLane): TrainingRunNodeStatus =>
  lane.state === 'accepted'
    ? 'verified'
    : lane.state === 'failing'
      ? 'blocked'
      : 'queued'

const lanePosition = (
  index: number,
  count: number,
): TrainingRunVector => {
  const span = 6.4
  const x = count <= 1 ? 0 : -span / 2 + (span * index) / (count - 1)
  return [x, -1.48, 0]
}

const laneNode = (
  lane: TerminalBenchRunLane,
  index: number,
  count: number,
): TrainingRunNodeDefinition => ({
  id: `lane:${lane.profileRef}`,
  label: lane.label,
  detail: `${lane.acceptedTasks} accepted / ${lane.failingTasks} failing / ${lane.notStartedTasks} not started`,
  role: 'run',
  status: laneNodeStatus(lane),
  position: lanePosition(index, count),
  connectedTo:
    lane.verifierDeviceRef === null
      ? ['report:comparison']
      : [`verifier:${lane.profileRef}`, 'report:comparison'],
})

const verifierNode = (
  lane: TerminalBenchRunLane,
  index: number,
  count: number,
): TrainingRunNodeDefinition => ({
  id: `verifier:${lane.profileRef}`,
  label: lane.distinctVerifierDevice ? 'distinct verifier' : 'verifier needed',
  detail:
    lane.verifierDeviceRef === null
      ? 'no verifier fixture'
      : `${lane.producerDeviceRef ?? 'producer'} -> ${lane.verifierDeviceRef}`,
  role: 'proof',
  status: lane.distinctVerifierDevice ? 'verified' : 'blocked',
  position: [lanePosition(index, count)[0], 0.9, 0],
  connectedTo: ['report:comparison'],
})

const laneContributor = (
  lane: TerminalBenchRunLane,
  index: number,
  count: number,
): TrainingRunContributorDefinition => ({
  id: `contributor:${lane.profileRef}`,
  label: lane.state === 'not_started' ? 'pilot' : `P${index + 1}`,
  lifecycleState:
    lane.state === 'accepted'
      ? 'active'
      : lane.state === 'failing'
        ? 'sync_reentry'
        : 'warmup',
  phase: count <= 0 ? 0 : index / count,
})

const laneBeam = (
  lane: TerminalBenchRunLane,
  generatedAt: string,
): TrainingRunBeamDefinition => ({
  fromId: 'run:terminal-bench',
  toId: `lane:${lane.profileRef}`,
  style: lane.state === 'accepted' ? 'crackling_arc' : 'flow',
  motionKind:
    lane.state === 'accepted'
      ? 'replay_verified'
      : lane.state === 'failing'
        ? 'replay_rejected'
        : 'assignment',
  simulated: true,
  sourceRefs: lane.evidenceRefs,
  generatedAt,
})

const laneBurst = (
  lane: TerminalBenchRunLane,
  generatedAt: string,
): TrainingRunBurstDefinition => ({
  atId: `lane:${lane.profileRef}`,
  motionKind: lane.state === 'accepted' ? 'replay_verified' : 'replay_rejected',
  simulated: true,
  sourceRefs: lane.evidenceRefs,
  generatedAt,
})

const statusSignals = (
  replay: TerminalBenchVisualReplay,
): ReadonlyArray<TrainingRunOperatorSignalDefinition> => [
  {
    id: 'terminal-bench.public-safe',
    label: 'public safe',
    state: replay.publicSafe ? 'success' : 'error',
    detail: replay.rawArtifactsIncluded
      ? 'raw artifacts included'
      : 'no raw prompts or completions',
  },
  {
    id: 'terminal-bench.decision-grade',
    label: 'decision grade',
    state: replay.decisionGrade ? 'success' : 'info',
    detail: replay.decisionGrade
      ? 'replication-ready'
      : 'partial replay, not decision-grade',
  },
  {
    id: 'terminal-bench.verse',
    label: 'Verse',
    state: 'info',
    detail: replay.deferredIntegration.note,
  },
]

const worldItems = (
  replay: TerminalBenchVisualReplay,
): ReadonlyArray<TrainingRunWorldItemDefinition> => [
  {
    id: 'bulletin:terminal-bench',
    kind: 'bulletin_board',
    label: 'Terminal-Bench board',
    title: 'Terminal-Bench 2.0',
    detail: 'Public-safe Gym visualization',
    position: [0, 2.72, 0],
    status: 'active',
    lines: [
      `${replay.officialTotalTasks} official tasks`,
      replay.externalClaim.label,
      'Web visualization first',
      'Verse integration deferred',
    ],
    sourceRefs: [
      replay.replayRef,
      ...replay.externalClaim.sourceRefs,
      ...replay.caveatRefs,
    ],
  },
]

export const terminalBenchVisualizationOptions = (
  replay: TerminalBenchVisualReplay,
): TrainingRunVisualizationOptions => {
  const laneCount = replay.lanes.length
  const verifierNodes = replay.lanes
    .filter(lane => lane.verifierDeviceRef !== null || !lane.distinctVerifierDevice)
    .map((lane, index) => verifierNode(lane, index, laneCount))
  const nodes: ReadonlyArray<TrainingRunNodeDefinition> = [
    {
      id: 'claim:external',
      label: 'external target',
      detail: replay.externalClaim.label,
      role: 'rung',
      status: 'sealed',
      position: [0, 1.9, 0],
      connectedTo: ['run:terminal-bench'],
    },
    {
      id: 'run:terminal-bench',
      label: 'Terminal-Bench Gym',
      detail: `${replay.taskSetLabel}; decision-grade false`,
      role: 'run',
      status: 'active',
      position: [0, 0.08, 0],
      connectedTo: replay.lanes.map(lane => `lane:${lane.profileRef}`),
    },
    ...replay.lanes.map((lane, index) => laneNode(lane, index, laneCount)),
    ...verifierNodes,
    {
      id: 'report:comparison',
      label: 'comparison report',
      detail: replay.schemaVersion,
      role: 'receipt',
      status: 'sealed',
      position: [0, -2.64, 0],
    },
  ]

  return {
    backgroundColor: 0x030609,
    cameraMode: 'orthographic_map',
    controller: 'none',
    nodes,
    contributors: replay.lanes.map((lane, index) =>
      laneContributor(lane, index, laneCount),
    ),
    lossCurve: [
      { step: 0, validationLoss: 0.92 },
      { step: 1, validationLoss: 0.71 },
      { step: 2, validationLoss: 0.49 },
      { step: 3, validationLoss: 0.31 },
    ],
    operatorSignals: statusSignals(replay),
    promiseSignals: [],
    entities: [],
    worldItems: worldItems(replay),
    remoteAvatars: [],
    beams: replay.lanes.map(lane => laneBeam(lane, replay.generatedAt)),
    bursts: replay.lanes
      .filter(lane => lane.state !== 'not_started')
      .map(lane => laneBurst(lane, replay.generatedAt)),
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
    pulseSpeed: 0.62,
  }
}
