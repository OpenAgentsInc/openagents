import { describe, expect, test } from 'vitest'

import { compileGymExperiment } from './experiment'
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
  buildGymTerminalBenchKhalaOrchestrationReport,
  buildGymTerminalBenchKhalaPolicyExperiment,
  resolveGymTerminalBenchKhalaPolicyProfile,
} from './terminal-bench-khala-orchestration'

const row = (input: {
  profileRef: GymTerminalBenchProfileRef
  lane: 'glm-52' | 'khala'
  solvedTasks: number
  totalTasks?: number | undefined
  costPerAcceptedOutcomeMsat: number | null
  decisionGrade?: boolean | undefined
  blockers?: ReadonlyArray<string> | undefined
}): GymTerminalBenchProfileComparisonRow => {
  const totalTasks = input.totalTasks ?? 89
  const officialFullTaskSet = totalTasks === 89
  const decisionGrade =
    input.decisionGrade ?? (officialFullTaskSet && (input.blockers ?? []).length === 0)
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
    officialFullTaskSet,
    totalTasks,
    attemptedTasks: totalTasks,
    properlyAttemptedTasks: totalTasks,
    solvedTasks: input.solvedTasks,
    fullDenominatorSolveRate:
      totalTasks <= 0 ? null : input.solvedTasks / totalTasks,
    attemptedSolveRate: totalTasks <= 0 ? null : input.solvedTasks / totalTasks,
    properlyAttemptedSolveRate:
      totalTasks <= 0 ? null : input.solvedTasks / totalTasks,
    gapToClaimBps:
      totalTasks <= 0
        ? null
        : Math.round(
            (input.solvedTasks / totalTasks -
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
    replicationClaimSatisfied: false,
    blockers: [...(input.blockers ?? [])],
    caveats: [],
    evidenceRefs: [
      `job.gym.terminal_bench.${input.profileRef}`,
      `report.gym.terminal_bench.${input.profileRef}`,
    ],
  }
}

const comparisonReport = (
  reportRef: string,
  comparisonRow: GymTerminalBenchProfileComparisonRow,
): GymTerminalBenchComparisonReport => ({
  schemaVersion: GYM_TERMINAL_BENCH_COMPARISON_REPORT_SCHEMA,
  reportRef,
  generatedAt: '2026-06-25T00:00:00Z',
  datasetRef: 'terminal-bench@2.0',
  externalClaim: GLM_REAP_TERMINAL_BENCH_691_TARGET,
  rows: [comparisonRow],
  decisionGrade: comparisonRow.decisionGrade,
  replicationClaimSatisfied: false,
  bestDecisionGradeSolveRateProfileRef: comparisonRow.decisionGrade
    ? comparisonRow.profileRef
    : null,
  bestDecisionGradeCostProfileRef:
    comparisonRow.decisionGrade &&
    comparisonRow.costPerAcceptedOutcomeMsat !== null
      ? comparisonRow.profileRef
      : null,
  publicSafe: true,
  rawArtifactsIncluded: false,
  blockers: comparisonRow.blockers,
  caveats: [],
})

describe('Terminal-Bench Khala orchestration comparison', () => {
  test('defines Khala policy profiles over existing Gym fanout axes', () => {
    const profile = resolveGymTerminalBenchKhalaPolicyProfile(
      'khala-terminal-bench-conductor-v2',
    )
    const experiment = buildGymTerminalBenchKhalaPolicyExperiment(
      profile.policyProfileRef,
    )
    const compiled = compileGymExperiment(experiment)

    expect(profile.coordinator).toBe('conductor-v2')
    expect(profile.fanout.mode).toBe('verifier-pick')
    expect(profile.fanout.lanes).toEqual([
      'khala',
      'glm-52',
      'gpt-oss-20b',
      'gpt-oss-120b',
      'vertex-gemini',
      'fireworks',
    ])
    expect(compiled.policySelection.fanout.lanes).toEqual(profile.fanout.lanes)
    expect(compiled.policySelection.environment.ref).toBe('terminal-bench')
  })

  test('marks a decision-grade Khala policy as beating raw GLM on solve rate and cost', () => {
    const rawBaselineReport = comparisonReport(
      'report.gym.terminal_bench.raw_glm_baseline',
      row({
        profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
        lane: 'glm-52',
        solvedTasks: 62,
        costPerAcceptedOutcomeMsat: 10_000,
      }),
    )
    const khalaReport = comparisonReport(
      'report.gym.terminal_bench.khala_conductor',
      row({
        profileRef: 'khala-public-heuristic',
        lane: 'khala',
        solvedTasks: 66,
        costPerAcceptedOutcomeMsat: 8_000,
      }),
    )

    const report = buildGymTerminalBenchKhalaOrchestrationReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rawBaselineReport,
      khalaReport,
      policyProfileRef: 'khala-terminal-bench-conductor-v2',
      rewardBundleRefs: ['bundle.gym.training_reward.terminal_bench.conductor'],
      leaderboardProjectionRefs: ['projection.gym.leaderboard.terminal_bench'],
      flywheelEvaluationRefs: ['evaluation.gym.flywheel.terminal_bench'],
      evidenceRefs: ['evidence.gym.terminal_bench.khala_conductor.public'],
    })

    expect(report.schemaVersion).toBe(
      GYM_TERMINAL_BENCH_KHALA_ORCHESTRATION_REPORT_SCHEMA,
    )
    expect(report.decisionGrade).toBe(true)
    expect(report.primaryOutcome).toBe('beats_on_solve_rate')
    expect(report.outcomes).toEqual([
      'beats_on_solve_rate',
      'beats_on_cost_per_accepted_outcome',
    ])
    expect(report.solveRateComparison.deltaBps).toBe(449)
    expect(report.costComparison.improvementBps).toBe(2_000)
    expect(report.beatsSolveRate).toBe(true)
    expect(report.beatsCostPerAcceptedOutcome).toBe(true)
    expect(report.rawBaselineAttribution).toContain('Z.ai GLM-5.2')
    expect(report.flywheelProjection.state).toBe('ready_for_training')
    expect(report.flywheelProjection.authority).toEqual({
      publicClaimAllowed: false,
      runtimePromotionAllowed: false,
      payoutAllowed: false,
      settlementAllowed: false,
      providerMutationAllowed: false,
    })
    expect(report.authority).toEqual(report.flywheelProjection.authority)
  })

  test('returns no_win when Khala does not beat the raw baseline', () => {
    const report = buildGymTerminalBenchKhalaOrchestrationReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rawBaselineReport: comparisonReport(
        'report.gym.terminal_bench.raw_glm_stronger',
        row({
          profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
          lane: 'glm-52',
          solvedTasks: 66,
          costPerAcceptedOutcomeMsat: 8_000,
        }),
      ),
      khalaReport: comparisonReport(
        'report.gym.terminal_bench.khala_no_win',
        row({
          profileRef: 'khala-public-heuristic',
          lane: 'khala',
          solvedTasks: 62,
          costPerAcceptedOutcomeMsat: 10_000,
        }),
      ),
      policyProfileRef: 'khala-terminal-bench-trinity-v1',
      rewardBundleRefs: ['bundle.gym.training_reward.terminal_bench.trinity'],
    })

    expect(report.decisionGrade).toBe(true)
    expect(report.primaryOutcome).toBe('no_win')
    expect(report.outcomes).toEqual(['no_win'])
    expect(report.beatsSolveRate).toBe(false)
    expect(report.beatsCostPerAcceptedOutcome).toBe(false)
    expect(report.flywheelProjection.state).toBe('blocked')
    expect(report.flywheelProjection.blockerRefs).toContain(
      'blocker.gym.terminal_bench.khala_orchestration_no_win',
    )
  })

  test('blocks pilot or non-decision-grade reports from becoming wins', () => {
    const report = buildGymTerminalBenchKhalaOrchestrationReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rawBaselineReport: comparisonReport(
        'report.gym.terminal_bench.raw_glm_baseline',
        row({
          profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
          lane: 'glm-52',
          solvedTasks: 62,
          costPerAcceptedOutcomeMsat: 10_000,
        }),
      ),
      khalaReport: comparisonReport(
        'report.gym.terminal_bench.khala_pilot',
        row({
          profileRef: 'khala-public-heuristic',
          lane: 'khala',
          solvedTasks: 8,
          totalTasks: 10,
          costPerAcceptedOutcomeMsat: 6_000,
          decisionGrade: false,
          blockers: [
            'blocker.gym.terminal_bench.official_full_task_set_required',
          ],
        }),
      ),
      policyProfileRef: 'khala-terminal-bench-conductor-v2',
      rewardBundleRefs: ['bundle.gym.training_reward.terminal_bench.pilot'],
    })

    expect(report.decisionGrade).toBe(false)
    expect(report.primaryOutcome).toBe('blocked')
    expect(report.outcomes).toEqual(['blocked'])
    expect(report.beatsSolveRate).toBe(false)
    expect(report.beatsCostPerAcceptedOutcome).toBe(false)
    expect(report.blockerRefs).toContain(
      'blocker.gym.terminal_bench.khala_report_not_decision_grade',
    )
    expect(report.blockerRefs).toContain(
      'blocker.gym.terminal_bench.khala_official_full_task_set_required',
    )
  })

  test('surfaces not_measured when cost-per-accepted-outcome is absent', () => {
    const report = buildGymTerminalBenchKhalaOrchestrationReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rawBaselineReport: comparisonReport(
        'report.gym.terminal_bench.raw_glm_same_solve',
        row({
          profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
          lane: 'glm-52',
          solvedTasks: 62,
          costPerAcceptedOutcomeMsat: 10_000,
        }),
      ),
      khalaReport: comparisonReport(
        'report.gym.terminal_bench.khala_unpriced',
        row({
          profileRef: 'khala-public-heuristic',
          lane: 'khala',
          solvedTasks: 62,
          costPerAcceptedOutcomeMsat: null,
        }),
      ),
      policyProfileRef: 'khala-terminal-bench-heuristic-v0',
      rewardBundleRefs: ['bundle.gym.training_reward.terminal_bench.heuristic'],
    })

    expect(report.decisionGrade).toBe(true)
    expect(report.primaryOutcome).toBe('not_measured')
    expect(report.costComparison.outcome).toBe('not_measured')
    expect(report.outcomes).toEqual(['not_measured'])
    expect(report.flywheelProjection.state).toBe('blocked')
  })

  test('rejects unsafe comparison reports instead of presenting a Khala win', () => {
    const rawBaselineReport = comparisonReport(
      'report.gym.terminal_bench.raw_glm_baseline',
      row({
        profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
        lane: 'glm-52',
        solvedTasks: 62,
        costPerAcceptedOutcomeMsat: 10_000,
      }),
    )
    const unsafeKhalaReport = {
      ...comparisonReport(
        'report.gym.terminal_bench.khala_unsafe',
        row({
          profileRef: 'khala-public-heuristic',
          lane: 'khala',
          solvedTasks: 66,
          costPerAcceptedOutcomeMsat: 8_000,
        }),
      ),
      rawArtifactsIncluded: true,
    }

    expect(() =>
      buildGymTerminalBenchKhalaOrchestrationReport({
        generatedAt: '2026-06-25T00:00:00Z',
        rawBaselineReport,
        khalaReport: unsafeKhalaReport,
        policyProfileRef: 'khala-terminal-bench-conductor-v2',
      }),
    ).toThrow()
  })

  test('keeps the public artifact free of secret-bearing material', () => {
    const report = buildGymTerminalBenchKhalaOrchestrationReport({
      generatedAt: '2026-06-25T00:00:00Z',
      rawBaselineReport: comparisonReport(
        'report.gym.terminal_bench.raw_glm_baseline',
        row({
          profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
          lane: 'glm-52',
          solvedTasks: 62,
          costPerAcceptedOutcomeMsat: 10_000,
        }),
      ),
      khalaReport: comparisonReport(
        'report.gym.terminal_bench.khala_public_safe',
        row({
          profileRef: 'khala-public-heuristic',
          lane: 'khala',
          solvedTasks: 66,
          costPerAcceptedOutcomeMsat: 8_000,
        }),
      ),
      policyProfileRef: 'khala-terminal-bench-conductor-v2',
      rewardBundleRefs: ['bundle.gym.training_reward.terminal_bench.conductor'],
    })

    expect(JSON.stringify(report)).not.toMatch(
      /bearer|api[_-]?key|raw prompt|completion text|wallet|mnemonic|token:|private_endpoint/i,
    )
  })
})
