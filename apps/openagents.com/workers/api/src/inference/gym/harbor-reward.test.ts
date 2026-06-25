import { describe, expect, test } from 'vitest'

import { TERMINAL_BENCH_GYM_EXPERIMENT } from './experiment'
import {
  HYDRALISK_TERMINAL_BENCH_SUMMARY_SCHEMA,
  dispatchGymHarborTerminalBenchRun,
  type GymHarborTerminalBenchDispatchReceipt,
  type GymHarborVerifierPlacementEvidence,
  type HydraliskHarborTerminalBenchHarness,
  type HydraliskTerminalBenchSummary,
} from './harbor-dispatch'
import {
  buildGymHarborTerminalBenchRewardArtifacts,
  type GymHarborRealCostBasis,
} from './harbor-reward'

const verifierPlacement: GymHarborVerifierPlacementEvidence = {
  schemaVersion: 'openagents.gym.harbor_verifier_placement.v1',
  environmentMode: 'separate',
  agentHostRef: 'hydralisk.host.agent.l4.001',
  verifierHostRef: 'psionic.host.verifier.cpu.001',
  agentDeviceRef: 'gce.vm.hydralisk-agent-l4-001',
  verifierDeviceRef: 'gce.vm.psionic-verifier-cpu-001',
  verifierNetworkMode: 'no-network',
  artifactHandoffRefs: ['artifact.hydralisk.terminal_bench.answer_json.001'],
  rewardArtifactRef: 'artifact.hydralisk.terminal_bench.reward_txt.001',
  rewardReadFrom: 'verifier_artifact',
}

const summaryWithCounts = (input: {
  solved: number
  failing: number
  envBroken?: number
  notStarted?: number
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
      model: 'openagents/khala',
      nConcurrent: 1,
      timeoutSeconds: 3600,
      maxAttempts: 5,
      retryPolicy: 'pass@1 plus up to 4 queued retries for pass@5',
    },
    model: {
      alias: 'openagents/khala',
      profileRef: 'profiles/openagents-khala.terminal-bench.v1',
      revision: 'openagents/khala@public',
      hardwareProfile: 'Hydralisk benchmark harness',
    },
    sampler: {
      minP: 0.05,
      repetitionPenalty: 1.05,
      maxTokens: 1024,
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
    claimStatus: 'preliminary_pilot_partial',
    inputSha256: 'b'.repeat(64),
    comparisonBoundary:
      'Do not compare publicly unless benchmark version, Harbor version, agent, model alias, sampler settings, retry policy, timeout, and denominator definitions are all named.',
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

const costBasis = (
  totalCostBasisMsat: number,
  guard: GymHarborRealCostBasis['gpuContention'] = {
    state: 'cleared',
    schedulingMode: 'benchmark_replica',
    liveServingLaneRefs: ['hydralisk.khala.l4.live'],
    benchmarkReplicaRef: 'hydralisk.benchmark.replica.terminal_bench.001',
    schedulingWindowRef: null,
    blockers: [],
  },
): GymHarborRealCostBasis => ({
  schemaVersion: 'openagents.gym.harbor_real_cost_basis.v1',
  costBasisRef: `cost.gym.harbor_terminal_bench.${totalCostBasisMsat}`,
  source: 'served_tokens_recorder',
  totalCostBasisMsat,
  tokenUsageEventRefs: ['token_usage_event.harbor_terminal_bench.khala.001'],
  gpuContention: guard,
})

const runWithSummary = async (
  summary: HydraliskTerminalBenchSummary,
  input: { atifTraceRef?: string | null } = {},
) => {
  const harness: HydraliskHarborTerminalBenchHarness = {
    async dispatchTerminalBenchJob(
      job,
    ): Promise<GymHarborTerminalBenchDispatchReceipt> {
      return {
        schemaVersion: 'openagents.gym.harbor_terminal_bench_dispatch_receipt.v1',
        jobRef: job.jobRef,
        hydraliskRunRef: 'hydralisk.run.terminal_bench.khala.reward.001',
        state: 'completed',
        summaryArtifactRef: 'artifact.hydralisk.terminal_bench.summary.001',
        atifTraceRef:
          input.atifTraceRef === undefined
            ? 'trace.hydralisk.terminal_bench.khala.001'
            : input.atifTraceRef,
        rawHarborArtifactRef: null,
        verifierPlacement,
      }
    },
    async readTerminalBenchSummary() {
      return summary
    },
  }

  return dispatchGymHarborTerminalBenchRun(TERMINAL_BENCH_GYM_EXPERIMENT, {
    harness,
    ownerApprovalRef: 'approval.gym.harbor.terminal_bench.reward.001',
  })
}

describe('Gym Harbor reward and trajectory projection', () => {
  test('maps Harbor rewards to cost-per-accepted-outcome and a training trajectory', async () => {
    const run = await runWithSummary(
      summaryWithCounts({ solved: 3, failing: 1, envBroken: 1 }),
    )
    const { report, trainingTrajectory } =
      buildGymHarborTerminalBenchRewardArtifacts({
        run,
        costBasis: costBasis(1_200_000),
      })

    expect(report.schemaVersion).toBe('openagents.gym.harbor_reward_report.v1')
    expect(report.decisionGrade).toBe(true)
    expect(report.publicClaimEligible).toBe(false)
    expect(report.acceptedOutcomes).toBe(3)
    expect(report.attemptedVerifications).toBe(4)
    expect(report.totalCostBasisMsat).toBe(1_200_000)
    expect(report.costPerAcceptedOutcomeMsat).toBe(400_000)
    expect(report.scalarRewardMean).toBe(0.75)
    expect(report.gpuContentionCleared).toBe(true)
    expect(report.blockers).toEqual([])

    expect(trainingTrajectory.schemaVersion).toBe(
      'openagents.gym.harbor_training_trajectory.v1',
    )
    expect(trainingTrajectory.atifTraceRef).toBe(
      'trace.hydralisk.terminal_bench.khala.001',
    )
    expect(trainingTrajectory.rewardSource).toBe('harbor_verifier_artifact')
    expect(trainingTrajectory.rewardArtifactRef).toBe(
      'artifact.hydralisk.terminal_bench.reward_txt.001',
    )
    expect(trainingTrajectory.readyForTraining).toBe(true)
    expect(trainingTrajectory.rawTraceIncluded).toBe(false)
  })

  test('renders null cost-per-outcome when Harbor accepts no outcomes', async () => {
    const run = await runWithSummary(summaryWithCounts({ solved: 0, failing: 5 }))
    const { report, trainingTrajectory } =
      buildGymHarborTerminalBenchRewardArtifacts({
        run,
        costBasis: costBasis(900_000),
      })

    expect(report.acceptedOutcomes).toBe(0)
    expect(report.costPerAcceptedOutcomeMsat).toBeNull()
    expect(report.scalarRewardMean).toBe(0)
    expect(trainingTrajectory.scalarRewardMean).toBe(0)
    expect(trainingTrajectory.readyForTraining).toBe(true)
  })

  test('blocks decision-grade report and training when GPU contention is not cleared', async () => {
    const run = await runWithSummary(summaryWithCounts({ solved: 2, failing: 2 }))
    const { report, trainingTrajectory } =
      buildGymHarborTerminalBenchRewardArtifacts({
        run,
        costBasis: costBasis(800_000, {
          state: 'blocked',
          schedulingMode: 'unknown_live_lane',
          liveServingLaneRefs: ['hydralisk.khala.g4.live'],
          benchmarkReplicaRef: null,
          schedulingWindowRef: null,
          blockers: ['blocker.gym.harbor.live_lane_contention_unknown'],
        }),
      })

    expect(report.decisionGrade).toBe(false)
    expect(report.gpuContentionCleared).toBe(false)
    expect(report.costPerAcceptedOutcomeMsat).toBe(400_000)
    expect(report.blockers).toEqual([
      'blocker.gym.harbor.gpu_contention_not_cleared',
      'blocker.gym.harbor.live_lane_contention_unknown',
    ])
    expect(trainingTrajectory.readyForTraining).toBe(false)
    expect(trainingTrajectory.blockers).toEqual(report.blockers)
  })

  test('requires an ATIF trace ref for training trajectory ingestion', async () => {
    const run = await runWithSummary(summaryWithCounts({ solved: 1, failing: 0 }), {
      atifTraceRef: null,
    })

    expect(() =>
      buildGymHarborTerminalBenchRewardArtifacts({
        run,
        costBasis: costBasis(100_000),
      }),
    ).toThrow(/ATIF trace ref/)
  })
})
