import { describe, expect, test } from 'vitest'

import { NOT_MEASURED } from '../khala-telemetry'
import {
  TERMINAL_BENCH_GYM_EXPERIMENT,
  type GymExperiment,
} from './experiment'
import {
  GLM_REAP_TERMINAL_BENCH_MODEL_ID,
  HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
  dispatchGymHarborTerminalBenchRun,
  type GymHarborTerminalBenchDispatchReceipt,
  type GymHarborVerifierPlacementEvidence,
  type GymTerminalBenchProfileRef,
  type HydraliskHarborTerminalBenchHarness,
  type HydraliskTerminalBenchSummary,
} from './harbor-dispatch'
import {
  buildGymHarborTerminalBenchRewardArtifacts,
  type GymHarborRealCostBasis,
} from './harbor-reward'
import {
  GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA,
  buildGymTerminalBenchComparisonReport,
  type GymTerminalBenchThroughputMeasurement,
} from './terminal-bench-comparison'

const GLM_TERMINAL_BENCH_GYM_EXPERIMENT: GymExperiment = {
  ...TERMINAL_BENCH_GYM_EXPERIMENT,
  id: 'gym-terminal-bench-glm-reap-comparison-v1',
  policy: {
    ...TERMINAL_BENCH_GYM_EXPERIMENT.policy,
    fanout: {
      ...TERMINAL_BENCH_GYM_EXPERIMENT.policy.fanout,
      lanes: ['glm-52'],
    },
  },
}

const verifierPlacement: GymHarborVerifierPlacementEvidence = {
  schemaVersion: 'openagents.gym.harbor_verifier_placement.v1',
  environmentMode: 'separate',
  agentHostRef: 'hydralisk.host.agent.g4.001',
  verifierHostRef: 'psionic.host.verifier.cpu.002',
  agentDeviceRef: 'gce.vm.hydralisk-agent-g4-001',
  verifierDeviceRef: 'gce.vm.psionic-verifier-cpu-002',
  workerModelFamily: 'zai-glm-5',
  verifierModelFamily: 'psionic-verifier',
  verifierPanelIndependence: {
    judgeCount: 9,
    effectiveVoteCount: 3,
    modelFamilyCount: 3,
    independenceMetricRef:
      'metric.gym.harbor.terminal_bench.panel_effective_independence.comparison.002',
  },
  agentChannelDefenses: {
    paraphraseBeforeVerification: true,
    crossModelVerifier: true,
    steganographyScreenRef:
      'screen.gym.harbor.terminal_bench.agent_channel_steganalysis.comparison.002',
  },
  verifierNetworkMode: 'no-network',
  artifactHandoffRefs: ['artifact.hydralisk.terminal_bench.answer_json.002'],
  rewardArtifactRef: 'artifact.hydralisk.terminal_bench.reward_txt.002',
  rewardReadFrom: 'verifier_artifact',
}

const summaryWithCounts = (input: {
  solved: number
  failing: number
  envBroken?: number
  notStarted?: number
  profileRef: GymTerminalBenchProfileRef
}): HydraliskTerminalBenchSummary => {
  const envBroken = input.envBroken ?? 0
  const notStarted = input.notStarted ?? 0
  const total = input.solved + input.failing + envBroken + notStarted
  const attempted = total - notStarted
  const properlyAttempted = attempted - envBroken
  return {
    schema: HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
    createdAt: '2026-06-25T00:00:00Z',
    publicSafe: true,
    benchmark: {
      name: 'Terminal-Bench',
      datasetRef: 'terminal-bench@2.0',
      version: '2.0',
      repository: 'https://github.com/harbor-framework/terminal-bench-2',
      harnessRepository: 'https://github.com/harbor-framework/harbor',
    },
    runner: {
      name: 'harbor',
      version: '0.15.0',
      agent: 'terminus-2',
      model: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
      nConcurrent: 1,
      timeoutSeconds: 3600,
      maxAttempts: 5,
      retryPolicy: 'pass@1 plus up to 4 queued retries for pass@5',
    },
    model: {
      alias: GLM_REAP_TERMINAL_BENCH_MODEL_ID,
      profileRef: input.profileRef,
      revision: '0xSero/GLM-5.2-504B@public-safe-ref',
      hardwareProfile: 'hydralisk-g4-benchmark-replica',
    },
    sampler: {
      minP: input.profileRef.includes('mtp2') ? null : 0.05,
      repetitionPenalty: input.profileRef.includes('250k') ? 1.1 : 1.05,
      maxTokens: 8192,
      enableThinking: false,
    },
    counts: {
      solved: input.solved,
      failing: input.failing,
      envBroken,
      notStarted,
      total,
      attempted,
      properlyAttempted,
    },
    rates: {
      fullDenominatorSolved: total === 0 ? null : input.solved / total,
      attemptedSolved: attempted === 0 ? null : input.solved / attempted,
      properlyAttemptedSolved:
        properlyAttempted === 0 ? null : input.solved / properlyAttempted,
      knownPassAt1: null,
      passAtN: total === 0 ? null : input.solved / total,
    },
    passAt: {
      passAt1Solved: null,
      passAt1KnownTasks: 0,
      passAtNAnySolved: input.solved,
      maxAttempts: 5,
    },
    denominatorDefinitions: {
      total: 'all Terminal-Bench 2.0 task IDs in the run set',
      attempted: 'total minus not-started tasks',
      properlyAttempted: 'attempted minus environment-broken tasks',
      fullDenominatorSolved: 'solved / total',
      attemptedSolved: 'solved / attempted',
      properlyAttemptedSolved: 'solved / properlyAttempted',
    },
    taskIds: {
      envBroken: [],
      notStarted: [],
      notableSolved: ['configure-git-webserver'],
    },
    claimStatus: 'decision_grade_full_official',
    inputSha256: 'c'.repeat(64),
    comparisonBoundary:
      'Public comparison requires full denominator, named sampler, named model alias, and no raw task content.',
    publicSafety: {
      containsSecrets: false,
      containsPrompts: false,
      containsResponses: false,
      containsHiddenReasoning: false,
      containsPrivateSource: false,
      containsRawBenchmarkLogs: false,
    },
  }
}

const runWithSummary = async (
  summary: HydraliskTerminalBenchSummary,
  profileRef: GymTerminalBenchProfileRef,
) => {
  const harness: HydraliskHarborTerminalBenchHarness = {
    async dispatchTerminalBenchJob(
      job,
    ): Promise<GymHarborTerminalBenchDispatchReceipt> {
      return {
        schemaVersion: 'openagents.gym.harbor_terminal_bench_dispatch_receipt.v1',
        jobRef: job.jobRef,
        hydraliskRunRef: `hydralisk.run.terminal_bench.${profileRef}.001`,
        state: 'completed',
        summaryArtifactRef: `artifact.hydralisk.terminal_bench.${profileRef}.summary`,
        atifTraceRef: `trace.hydralisk.terminal_bench.${profileRef}.001`,
        rawHarborArtifactRef: null,
        verifierPlacement,
      }
    },
    async readTerminalBenchSummary() {
      return summary
    },
  }

  return dispatchGymHarborTerminalBenchRun(GLM_TERMINAL_BENCH_GYM_EXPERIMENT, {
    harness,
    ownerApprovalRef: `approval.gym.harbor.terminal_bench.${profileRef}`,
    profileRef,
  })
}

const costBasis = (
  profileRef: GymTerminalBenchProfileRef,
  totalCostBasisMsat: number,
): GymHarborRealCostBasis => ({
  schemaVersion: 'openagents.gym.harbor_real_cost_basis.v1',
  costBasisRef: `cost.gym.harbor_terminal_bench.${profileRef}.${totalCostBasisMsat}`,
  source: 'served_tokens_recorder',
  totalCostBasisMsat,
  tokenUsageEventRefs: [
    `token_usage_event.harbor_terminal_bench.${profileRef}.001`,
  ],
  gpuContention: {
    state: 'cleared',
    schedulingMode: 'benchmark_replica',
    liveServingLaneRefs: ['hydralisk.glm_52.live'],
    benchmarkReplicaRef: `hydralisk.benchmark.replica.${profileRef}`,
    schedulingWindowRef: null,
    blockers: [],
  },
})

const throughput = (
  profileRef: GymTerminalBenchProfileRef,
  input: Partial<
    Omit<GymTerminalBenchThroughputMeasurement, 'profileRef' | 'measurementRef'>
  > = {},
): GymTerminalBenchThroughputMeasurement => ({
  profileRef,
  measurementRef: `throughput.gym.terminal_bench.${profileRef}.001`,
  ttftMs: input.ttftMs ?? 420,
  totalWallClockMs: input.totalWallClockMs ?? 120_000,
  perceivedTps: input.perceivedTps ?? 44,
  interTokenLatencyMs: input.interTokenLatencyMs ?? 22,
  aggregateTps: input.aggregateTps ?? 44,
})

const comparisonInput = async (input: {
  profileRef: GymTerminalBenchProfileRef
  solved: number
  failing: number
  totalCostBasisMsat: number
  envBroken?: number
  notStarted?: number
  throughput?: GymTerminalBenchThroughputMeasurement
}) => {
  const summaryInput: Parameters<typeof summaryWithCounts>[0] = {
    solved: input.solved,
    failing: input.failing,
    profileRef: input.profileRef,
  }
  if (input.envBroken !== undefined) {
    summaryInput.envBroken = input.envBroken
  }
  if (input.notStarted !== undefined) {
    summaryInput.notStarted = input.notStarted
  }
  const run = await runWithSummary(
    summaryWithCounts(summaryInput),
    input.profileRef,
  )
  const { report } = buildGymHarborTerminalBenchRewardArtifacts({
    run,
    costBasis: costBasis(input.profileRef, input.totalCostBasisMsat),
  })
  return {
    run,
    rewardReport: report,
    throughput: input.throughput ?? throughput(input.profileRef),
  }
}

describe('Terminal-Bench comparison report', () => {
  test('compares GLM-REAP profiles against the external 69.1% target', async () => {
    const mtp2Profile = 'glm-reap-504b-g4-tp4-mtp2-rp105'
    const baseProfile = 'glm-reap-504b-g4-tp4-minp-rp105'
    const report = buildGymTerminalBenchComparisonReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rows: [
        await comparisonInput({
          profileRef: mtp2Profile,
          solved: 62,
          failing: 27,
          totalCostBasisMsat: 620_000,
          throughput: throughput(mtp2Profile, {
            ttftMs: 380,
            perceivedTps: 51,
            interTokenLatencyMs: 19,
            aggregateTps: 51,
          }),
        }),
        await comparisonInput({
          profileRef: baseProfile,
          solved: 60,
          failing: 29,
          totalCostBasisMsat: 570_000,
          throughput: throughput(baseProfile, {
            perceivedTps: NOT_MEASURED,
            aggregateTps: NOT_MEASURED,
          }),
        }),
      ],
    })

    expect(report.schemaVersion).toBe(
      GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA,
    )
    expect(report.externalClaim.kind).toBe('external_claim')
    expect(report.externalClaim.claimedFullDenominatorSolveRate).toBe(0.691)
    expect(report.decisionGrade).toBe(true)
    expect(report.replicationClaimSatisfied).toBe(true)
    expect(report.bestDecisionGradeSolveRateProfileRef).toBe(mtp2Profile)
    expect(report.bestDecisionGradeCostProfileRef).toBe(baseProfile)

    const [mtp2Row, baseRow] = report.rows
    expect(mtp2Row?.fullDenominatorSolveRate).toBeCloseTo(62 / 89)
    expect(mtp2Row?.gapToClaimBps).toBe(56)
    expect(mtp2Row?.replicationClaimSatisfied).toBe(true)
    expect(mtp2Row?.costPerAcceptedOutcomeMsat).toBe(10_000)
    expect(mtp2Row?.decisionGrade).toBe(true)

    expect(baseRow?.fullDenominatorSolveRate).toBeCloseTo(60 / 89)
    expect(baseRow?.gapToClaimBps).toBe(-168)
    expect(baseRow?.replicationClaimSatisfied).toBe(false)
    expect(baseRow?.caveats).toContain(
      'caveat.gym.terminal_bench.throughput_not_fully_measured',
    )

    expect(JSON.stringify(report)).not.toMatch(
      /bearer|api[_-]?key|https:\/\/private|rawHarbor|task prompt|completion/i,
    )
  })

  test('does not let pilot denominators satisfy decision-grade replication', async () => {
    const pilotProfile = 'glm-reap-504b-g4-tp4-minp-rp105'
    const report = buildGymTerminalBenchComparisonReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rows: [
        await comparisonInput({
          profileRef: pilotProfile,
          solved: 7,
          failing: 0,
          notStarted: 3,
          totalCostBasisMsat: 70_000,
        }),
      ],
    })
    const [row] = report.rows

    expect(row?.totalTasks).toBe(10)
    expect(row?.attemptedSolveRate).toBe(1)
    expect(row?.fullDenominatorSolveRate).toBe(0.7)
    expect(row?.officialFullTaskSet).toBe(false)
    expect(row?.decisionGrade).toBe(false)
    expect(row?.replicationClaimSatisfied).toBe(false)
    expect(row?.blockers).toContain(
      'blocker.gym.terminal_bench.official_full_task_set_required',
    )
    expect(report.decisionGrade).toBe(false)
    expect(report.replicationClaimSatisfied).toBe(false)
  })

  test('keeps measured zero distinct from not_measured throughput fields', async () => {
    const profileRef = 'glm-reap-504b-g4-dual-tp4-minp-rp105'
    const report = buildGymTerminalBenchComparisonReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rows: [
        await comparisonInput({
          profileRef,
          solved: 58,
          failing: 31,
          totalCostBasisMsat: 870_000,
          throughput: throughput(profileRef, {
            ttftMs: 0,
            aggregateTps: 0,
            perceivedTps: NOT_MEASURED,
          }),
        }),
      ],
    })
    const [row] = report.rows

    expect(row?.ttftMs).toBe(0)
    expect(row?.aggregateTps).toBe(0)
    expect(row?.perceivedTps).toBe(NOT_MEASURED)
    expect(row?.caveats).toContain(
      'caveat.gym.terminal_bench.throughput_not_fully_measured',
    )
  })
})
