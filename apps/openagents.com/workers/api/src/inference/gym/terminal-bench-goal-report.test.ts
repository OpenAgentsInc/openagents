import { describe, expect, test } from 'vitest'

import {
  GLM_REAP_TERMINAL_BENCH_MODEL_ID,
  KHALA_PUBLIC_MODEL_ID,
  type GymTerminalBenchProfileRef,
} from './harbor-dispatch'
import {
  GLM_REAP_TERMINAL_BENCH_691_TARGET,
  GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA,
  type GymTerminalBenchComparisonReport,
  type GymTerminalBenchProfileComparisonRow,
} from './terminal-bench-comparison'
import {
  GYM_TERMINAL_BENCH_KHALA_ORCHESTRATION_REPORT_SCHEMA,
  type GymTerminalBenchKhalaOrchestrationReport,
} from './terminal-bench-khala-orchestration'
import {
  GYM_TERMINAL_BENCH_6253_GOAL_REPORT_SCHEMA,
  buildGymTerminalBench6253GoalReport,
} from './terminal-bench-goal-report'

const authority = {
  publicClaimAllowed: false,
  runtimePromotionAllowed: false,
  payoutAllowed: false,
  settlementAllowed: false,
  providerMutationAllowed: false,
} as const

const row = (input: {
  profileRef: GymTerminalBenchProfileRef
  lane: 'glm-52' | 'khala'
  solvedTasks: number
  costPerAcceptedOutcomeMsat: number | null
  decisionGrade?: boolean | undefined
  blockers?: ReadonlyArray<string> | undefined
}): GymTerminalBenchProfileComparisonRow => {
  const decisionGrade =
    input.decisionGrade ?? (input.blockers ?? []).length === 0
  const model =
    input.lane === 'khala'
      ? KHALA_PUBLIC_MODEL_ID
      : GLM_REAP_TERMINAL_BENCH_MODEL_ID
  const attribution =
    input.lane === 'khala'
      ? 'OpenAgents Khala orchestrator'
      : 'Z.ai GLM-5.2, REAP-pruned keep-168 NVFP4'

  return {
    profileRef: input.profileRef,
    lane: input.lane,
    model,
    serving: {
      publicLabel:
        input.lane === 'khala'
          ? 'Khala orchestrated Terminal-Bench route'
          : 'GLM-5.2 REAP raw profile',
      sourceModelRef:
        input.lane === 'khala' ? 'openagents/khala' : '0xSero/GLM-5.2-504B',
      attribution,
      hardwareProfile:
        input.lane === 'khala'
          ? 'khala-router'
          : 'hydralisk-g4-4x-rtx-pro-6000',
      tensorParallelism: input.lane === 'khala' ? 0 : 4,
      replicaTopology: input.lane === 'khala' ? 'khala_router' : 'single_tp4',
      contextWindowTokens: 250_000,
      quantization: input.lane === 'khala' ? 'router_mixed' : 'nvfp4',
      speculationMode: input.lane === 'khala' ? 'none' : 'mtp2',
      sampler: {
        minP: input.lane === 'khala' ? 0.05 : null,
        repetitionPenalty: 1.05,
        enableThinking: false,
      },
    },
    hydraliskRunRef: `hydralisk.run.terminal_bench.${input.profileRef}.001`,
    summarySchema: 'hydralisk.evals.terminal_bench.summary.v1',
    summaryArtifactRef: `artifact.hydralisk.terminal_bench.${input.profileRef}.summary`,
    costBasisRef: `cost.gym.terminal_bench.${input.profileRef}`,
    throughputMeasurementRef: `throughput.gym.terminal_bench.${input.profileRef}`,
    officialFullTaskSet: true,
    totalTasks: 89,
    attemptedTasks: 89,
    properlyAttemptedTasks: 89,
    solvedTasks: input.solvedTasks,
    fullDenominatorSolveRate: input.solvedTasks / 89,
    attemptedSolveRate: input.solvedTasks / 89,
    properlyAttemptedSolveRate: input.solvedTasks / 89,
    gapToClaimBps: Math.round(
      (input.solvedTasks / 89 -
        GLM_REAP_TERMINAL_BENCH_691_TARGET.claimedFullDenominatorSolveRate) *
        10_000,
    ),
    totalCostBasisMsat:
      input.costPerAcceptedOutcomeMsat === null
        ? 0
        : input.costPerAcceptedOutcomeMsat * input.solvedTasks,
    costPerAcceptedOutcomeMsat: input.costPerAcceptedOutcomeMsat,
    ttftMs: 380,
    totalWallClockMs: 120_000,
    perceivedTps: 51,
    interTokenLatencyMs: 19,
    aggregateTps: 51,
    decisionGrade,
    replicationClaimSatisfied:
      input.lane === 'glm-52' &&
      decisionGrade &&
      input.solvedTasks / 89 >=
        GLM_REAP_TERMINAL_BENCH_691_TARGET.claimedFullDenominatorSolveRate,
    blockers: [...(input.blockers ?? [])],
    caveats: [],
    evidenceRefs: [
      `job.gym.terminal_bench.${input.profileRef}`,
      `artifact.gym.terminal_bench.${input.profileRef}.summary`,
    ],
  }
}

const comparisonReport = (
  reportRef: string,
  rows: ReadonlyArray<GymTerminalBenchProfileComparisonRow>,
): GymTerminalBenchComparisonReport => {
  const decisionRows = rows.filter(row => row.decisionGrade)
  return {
    schemaVersion: GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA,
    reportRef,
    generatedAt: '2026-06-25T00:00:00Z',
    datasetRef: 'terminal-bench@2.0',
    externalClaim: GLM_REAP_TERMINAL_BENCH_691_TARGET,
    rows,
    decisionGrade: rows.every(row => row.decisionGrade),
    replicationClaimSatisfied: rows.some(row => row.replicationClaimSatisfied),
    bestDecisionGradeSolveRateProfileRef:
      decisionRows.sort(
        (left, right) =>
          (right.fullDenominatorSolveRate ?? -1) -
          (left.fullDenominatorSolveRate ?? -1),
      )[0]?.profileRef ?? null,
    bestDecisionGradeCostProfileRef:
      decisionRows
        .filter(row => row.costPerAcceptedOutcomeMsat !== null)
        .sort(
          (left, right) =>
            (left.costPerAcceptedOutcomeMsat ?? Number.MAX_SAFE_INTEGER) -
            (right.costPerAcceptedOutcomeMsat ?? Number.MAX_SAFE_INTEGER),
        )[0]?.profileRef ?? null,
    publicSafe: true,
    rawArtifactsIncluded: false,
    blockers: rows.flatMap(row => row.blockers),
    caveats: [],
  }
}

const khalaReport = (input: {
  reportRef: string
  rawBaselineReportRef: string
  khalaReportRef?: string | undefined
  primaryOutcome: 'beats_on_solve_rate' | 'no_win' | 'not_measured' | 'blocked'
  beatsSolveRate: boolean
  beatsCostPerAcceptedOutcome: boolean
  decisionGrade?: boolean | undefined
  blockerRefs?: ReadonlyArray<string> | undefined
}): GymTerminalBenchKhalaOrchestrationReport => ({
  schemaVersion: GYM_TERMINAL_BENCH_KHALA_ORCHESTRATION_REPORT_SCHEMA,
  reportRef: input.reportRef,
  generatedAt: '2026-06-25T00:00:00Z',
  datasetRef: 'terminal-bench@2.0',
  rawBaselineReportRef: input.rawBaselineReportRef,
  khalaReportRef: input.khalaReportRef ?? 'report.gym.terminal_bench.khala',
  rawBaselineProfileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
  khalaProfileRef: 'khala-public-heuristic',
  rawBaselineAttribution: 'Z.ai GLM-5.2, REAP-pruned keep-168 NVFP4',
  khalaAttribution: 'OpenAgents Khala orchestrator',
  policyProfile: {
    schemaVersion: 'openagents.gym.terminal_bench_khala_policy_profile.v1',
    policyProfileRef: 'khala-terminal-bench-conductor-v2',
    label: 'Khala Terminal-Bench Conductor verifier-pick router',
    coordinator: 'conductor-v2',
    fanout: {
      mode: 'verifier-pick',
      lanes: ['khala', 'glm-52'],
      concurrency: 2,
      bestOfN: 2,
      verifierPick: true,
    },
    tools: 'khala-code-tools',
    modules: {
      mode: 'starter-catalog',
      signatureRefs: ['program-signature.khala-terminal-bench-conductor-v2'],
      moduleRefs: ['module.khala-terminal-bench-conductor-v2.router_policy'],
    },
    sampling: {
      temperature: 0.2,
      reasoningEffort: 'off',
      maxTokens: 8192,
      transport: 'streaming',
    },
    serving: {
      quantization: { mode: 'none' },
      speculation: { mode: 'ngram', draftModelRef: 'glm-52.mtp2' },
    },
    trainingConsumers: ['gepa', 'trinity', 'conductor'],
    sourceRefs: ['source.gym.terminal_bench.policy.conductor_v2'],
    caveatRefs: [],
    publicSafe: true,
    rawArtifactsIncluded: false,
  },
  primaryOutcome: input.primaryOutcome,
  outcomes: [input.primaryOutcome],
  beatsSolveRate: input.beatsSolveRate,
  beatsCostPerAcceptedOutcome: input.beatsCostPerAcceptedOutcome,
  solveRateComparison: {
    outcome: input.primaryOutcome,
    rawBaselineSolveRate: 62 / 89,
    khalaSolveRate: input.beatsSolveRate ? 66 / 89 : 60 / 89,
    deltaBps: input.beatsSolveRate ? 449 : -225,
  },
  costComparison: {
    outcome: input.beatsCostPerAcceptedOutcome
      ? 'beats_on_cost_per_accepted_outcome'
      : 'no_win',
    rawBaselineCostPerAcceptedOutcomeMsat: 10_000,
    khalaCostPerAcceptedOutcomeMsat: input.beatsCostPerAcceptedOutcome
      ? 8_000
      : 12_000,
    improvementBps: input.beatsCostPerAcceptedOutcome ? 2_000 : -2_000,
  },
  decisionGrade: input.decisionGrade ?? true,
  publicSafe: true,
  rawArtifactsIncluded: false,
  evidenceRefs: [
    input.rawBaselineReportRef,
    input.khalaReportRef ?? 'report.gym.terminal_bench.khala',
  ],
  blockerRefs: [...(input.blockerRefs ?? [])],
  caveatRefs: [],
  flywheelProjection: {
    schemaVersion: 'openagents.gym.terminal_bench_khala_flywheel_projection.v1',
    projectionRef: 'projection.gym.terminal_bench.khala_flywheel.test',
    reportRef: input.reportRef,
    policyProfileRef: 'khala-terminal-bench-conductor-v2',
    state:
      input.beatsSolveRate || input.beatsCostPerAcceptedOutcome
        ? 'ready_for_training'
        : 'blocked',
    consumers: ['gepa', 'trinity', 'conductor'],
    rewardBundleRefs: ['bundle.gym.training_reward.terminal_bench.conductor'],
    leaderboardProjectionRefs: ['projection.gym.leaderboard.terminal_bench'],
    flywheelEvaluationRefs: ['evaluation.gym.flywheel.terminal_bench'],
    blockerRefs: [],
    caveatRefs: [],
    authority,
  },
  authority,
})

describe('Terminal-Bench #6253 goal report', () => {
  test('marks #6253 accepted when replication, inference comparison, and Khala win evidence are present', () => {
    const comparison = comparisonReport('report.gym.terminal_bench.glm_methods', [
      row({
        profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
        lane: 'glm-52',
        solvedTasks: 62,
        costPerAcceptedOutcomeMsat: 10_000,
      }),
      row({
        profileRef: 'glm-reap-504b-g4-tp4-minp-rp105',
        lane: 'glm-52',
        solvedTasks: 60,
        costPerAcceptedOutcomeMsat: 9_000,
      }),
    ])
    const report = buildGymTerminalBench6253GoalReport({
      generatedAt: '2026-06-25T00:00:00Z',
      comparisonReport: comparison,
      khalaOrchestrationReport: khalaReport({
        reportRef: 'report.gym.terminal_bench.khala_beats_glm',
        rawBaselineReportRef: comparison.reportRef,
        primaryOutcome: 'beats_on_solve_rate',
        beatsSolveRate: true,
        beatsCostPerAcceptedOutcome: true,
      }),
      evidenceRefs: ['evidence.gym.terminal_bench.issue_6253.public'],
    })

    expect(report.schemaVersion).toBe(
      GYM_TERMINAL_BENCH_6253_GOAL_REPORT_SCHEMA,
    )
    expect(report.acceptanceSatisfied).toBe(true)
    expect(report.decisionGrade).toBe(true)
    expect(report.glmReplication.status).toBe('replicated_at_or_above_claim')
    expect(report.inferenceMethodComparison.status).toBe(
      'comparison_table_ready',
    )
    expect(report.khalaOrchestration.status).toBe('khala_beats_raw_baseline')
    expect(report.authority).toEqual(authority)
    expect(JSON.stringify(report)).not.toMatch(
      /bearer|api[_-]?key|raw prompt|completion text|wallet|mnemonic|private_endpoint/i,
    )
  })

  test('accepts honest documented gap and no-win outcome without pretending Khala beat GLM', () => {
    const comparison = comparisonReport('report.gym.terminal_bench.glm_gap', [
      row({
        profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
        lane: 'glm-52',
        solvedTasks: 60,
        costPerAcceptedOutcomeMsat: 10_000,
      }),
      row({
        profileRef: 'glm-reap-504b-g4-tp4-minp-rp105',
        lane: 'glm-52',
        solvedTasks: 58,
        costPerAcceptedOutcomeMsat: 9_000,
      }),
    ])

    const report = buildGymTerminalBench6253GoalReport({
      generatedAt: '2026-06-25T00:00:00Z',
      comparisonReport: comparison,
      khalaOrchestrationReport: khalaReport({
        reportRef: 'report.gym.terminal_bench.khala_no_win',
        rawBaselineReportRef: comparison.reportRef,
        primaryOutcome: 'no_win',
        beatsSolveRate: false,
        beatsCostPerAcceptedOutcome: false,
      }),
    })

    expect(report.acceptanceSatisfied).toBe(true)
    expect(report.decisionGrade).toBe(true)
    expect(report.glmReplication.status).toBe('honest_gap_documented')
    expect(report.khalaOrchestration.status).toBe('no_win_documented')
    expect(report.caveatRefs).toContain(
      'caveat.gym.terminal_bench.issue_6253_khala_no_win_documented',
    )
  })

  test('rejects mismatched baseline reports and unsafe evidence refs', () => {
    const comparison = comparisonReport('report.gym.terminal_bench.glm_methods', [
      row({
        profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
        lane: 'glm-52',
        solvedTasks: 62,
        costPerAcceptedOutcomeMsat: 10_000,
      }),
      row({
        profileRef: 'glm-reap-504b-g4-tp4-minp-rp105',
        lane: 'glm-52',
        solvedTasks: 60,
        costPerAcceptedOutcomeMsat: 9_000,
      }),
    ])

    expect(() =>
      buildGymTerminalBench6253GoalReport({
        generatedAt: '2026-06-25T00:00:00Z',
        comparisonReport: comparison,
        khalaOrchestrationReport: khalaReport({
          reportRef: 'report.gym.terminal_bench.khala_mismatch',
          rawBaselineReportRef: 'report.gym.terminal_bench.other_baseline',
          primaryOutcome: 'beats_on_solve_rate',
          beatsSolveRate: true,
          beatsCostPerAcceptedOutcome: true,
        }),
      }),
    ).toThrow()

    expect(() =>
      buildGymTerminalBench6253GoalReport({
        generatedAt: '2026-06-25T00:00:00Z',
        comparisonReport: comparison,
        khalaOrchestrationReport: khalaReport({
          reportRef: 'report.gym.terminal_bench.khala_unsafe',
          rawBaselineReportRef: comparison.reportRef,
          primaryOutcome: 'beats_on_solve_rate',
          beatsSolveRate: true,
          beatsCostPerAcceptedOutcome: true,
        }),
        evidenceRefs: ['https://private.example.invalid/raw-log'],
      }),
    ).toThrow()
  })
})
