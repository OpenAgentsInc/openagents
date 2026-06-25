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

export const TERMINAL_BENCH_VISUAL_REPLAY: TerminalBenchVisualReplay = {
  schemaVersion: 'openagents.gym.terminal_bench_visual_replay.v1',
  replayRef: 'replay.gym.terminal_bench.glm_reap.visual_fixture.v1',
  generatedAt: '2026-06-25T00:00:00.000Z',
  datasetRef: 'terminal-bench@2.0',
  taskSetLabel: 'Terminal-Bench 2.0 official denominator',
  officialTotalTasks: 89,
  externalClaim: {
    label: 'GLM-5.2 REAP claimed 69.1%',
    claimedFullDenominatorSolveRate: 0.691,
    sourceRefs: [
      'source.huggingface.0xsero.glm_5_2_504b',
      'source.x.glm_reap_terminal_bench_2_691_claim',
    ],
    caveatRefs: [
      'caveat.external_claim.not_openagents_result',
      'caveat.external_claim.requires_source_review',
    ],
  },
  lanes: [
    {
      profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
      label: 'GLM REAP G4 TP4 MTP2',
      model: 'zai/glm-5.2-504b-reap-nvfp4',
      state: 'accepted',
      acceptedTasks: 62,
      failingTasks: 27,
      notStartedTasks: 0,
      totalTasks: 89,
      costBasisMsat: 620_000,
      ttftMs: 380,
      perceivedTps: 51,
      aggregateTps: 51,
      producerDeviceRef: 'hydralisk.harbor.g4.producer.fixture',
      verifierDeviceRef: 'hydralisk.harbor.cpu.verifier.fixture',
      distinctVerifierDevice: true,
      caveatRefs: ['caveat.gym.terminal_bench.visual_fixture'],
      blockerRefs: ['blocker.gym.terminal_bench.live_hydralisk_run_required'],
      evidenceRefs: [
        'report.gym.terminal_bench_comparison.fixture',
        'artifact.hydralisk.terminal_bench.glm_reap_mtp2.summary.fixture',
      ],
    },
    {
      profileRef: 'glm-reap-504b-g4-tp4-minp-rp105',
      label: 'GLM REAP G4 TP4 minP',
      model: 'zai/glm-5.2-504b-reap-nvfp4',
      state: 'failing',
      acceptedTasks: 60,
      failingTasks: 29,
      notStartedTasks: 0,
      totalTasks: 89,
      costBasisMsat: 570_000,
      ttftMs: null,
      perceivedTps: null,
      aggregateTps: null,
      producerDeviceRef: 'hydralisk.harbor.g4.producer.fixture',
      verifierDeviceRef: 'hydralisk.harbor.cpu.verifier.fixture',
      distinctVerifierDevice: true,
      caveatRefs: [
        'caveat.gym.terminal_bench.visual_fixture',
        'caveat.gym.terminal_bench.throughput_not_fully_measured',
      ],
      blockerRefs: [
        'blocker.gym.terminal_bench.replication_gap_to_claim',
        'blocker.gym.terminal_bench.live_hydralisk_run_required',
      ],
      evidenceRefs: [
        'report.gym.terminal_bench_comparison.fixture',
        'artifact.hydralisk.terminal_bench.glm_reap_minp.summary.fixture',
      ],
    },
    {
      profileRef: 'glm-reap-504b-g4-dual-tp4-minp-rp105',
      label: 'Dual G4 pilot',
      model: 'zai/glm-5.2-504b-reap-nvfp4',
      state: 'not_started',
      acceptedTasks: 7,
      failingTasks: 0,
      notStartedTasks: 3,
      totalTasks: 10,
      costBasisMsat: 70_000,
      ttftMs: 0,
      perceivedTps: null,
      aggregateTps: 0,
      producerDeviceRef: 'hydralisk.harbor.dual_g4.producer.fixture',
      verifierDeviceRef: null,
      distinctVerifierDevice: false,
      caveatRefs: [
        'caveat.gym.terminal_bench.visual_fixture',
        'caveat.gym.terminal_bench.throughput_not_fully_measured',
      ],
      blockerRefs: [
        'blocker.gym.terminal_bench.official_full_task_set_required',
        'blocker.gym.terminal_bench.distinct_device_verifier_missing',
      ],
      evidenceRefs: [
        'report.gym.terminal_bench_comparison.pilot_fixture',
        'artifact.hydralisk.terminal_bench.dual_g4.summary.fixture',
      ],
    },
  ],
  decisionGrade: false,
  publicSafe: true,
  rawArtifactsIncluded: false,
  caveatRefs: [
    'caveat.gym.terminal_bench.visual_fixture',
    'caveat.gym.terminal_bench.no_raw_prompts_or_completions',
    'caveat.gym.terminal_bench.not_live_verse_integration',
  ],
  blockerRefs: [
    'blocker.gym.terminal_bench.live_hydralisk_run_required',
    'blocker.gym.terminal_bench.owner_replication_approval_required',
    'blocker.gym.terminal_bench.distinct_device_verifier_required',
  ],
  deferredIntegration: {
    surface: 'autopilot_verse',
    state: 'deferred',
    note: 'Full Autopilot Verse/world integration is intentionally deferred; this route proves the web visualization contract first.',
  },
}

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

const laneBeam = (lane: TerminalBenchRunLane): TrainingRunBeamDefinition => ({
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
  generatedAt: TERMINAL_BENCH_VISUAL_REPLAY.generatedAt,
})

const laneBurst = (lane: TerminalBenchRunLane): TrainingRunBurstDefinition => ({
  atId: `lane:${lane.profileRef}`,
  motionKind: lane.state === 'accepted' ? 'replay_verified' : 'replay_rejected',
  simulated: true,
  sourceRefs: lane.evidenceRefs,
  generatedAt: TERMINAL_BENCH_VISUAL_REPLAY.generatedAt,
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
      : 'fixture visualization only',
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
    detail: 'Public-safe Gym visualization fixture',
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
  replay: TerminalBenchVisualReplay = TERMINAL_BENCH_VISUAL_REPLAY,
): TrainingRunVisualizationOptions => {
  const laneCount = replay.lanes.length
  const verifierNodes = replay.lanes
    .filter(lane => lane.verifierDeviceRef !== null || !lane.distinctVerifierDevice)
    .map((lane, index) => verifierNode(lane, index, laneCount))
  const nodes: ReadonlyArray<TrainingRunNodeDefinition> = [
    {
      id: 'claim:external',
      label: '69.1% target',
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
    beams: replay.lanes.map(laneBeam),
    bursts: replay.lanes
      .filter(lane => lane.state !== 'not_started')
      .map(laneBurst),
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
