import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_BENCHMARK_CLOUD_READ_ONLY_AUTHORITY,
  OmniBenchmarkCloudProjection,
  OmniBenchmarkCloudRecord,
  OmniBenchmarkCloudUnsafe,
  exampleOmniBenchmarkCloud,
  omniBenchmarkCloudProjectionHasPrivateMaterial,
  projectOmniBenchmarkCloud,
} from './omni-model-lab-benchmark-cloud'

const nowIso = '2026-06-07T00:00:00.000Z'

const benchmarkRecord = (
  overrides: Partial<OmniBenchmarkCloudRecord> = {},
): OmniBenchmarkCloudRecord =>
  S.decodeUnknownSync(OmniBenchmarkCloudRecord)({
    ...exampleOmniBenchmarkCloud(),
    ...overrides,
  })

describe('Omni Benchmark Cloud evidence contract', () => {
  test('projects passed benchmark evidence without benchmark, eval, provider, spend, runtime, routing, payout, settlement, raw-input, or public-claim authority', () => {
    const projection = projectOmniBenchmarkCloud(
      exampleOmniBenchmarkCloud(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniBenchmarkCloudProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      benchmarkLaunchAllowed: false,
      comparisonCount: 1,
      createdAtDisplay: '20 minutes ago',
      evalExecutionAllowed: false,
      evalJobCount: 2,
      failedScorecardCount: 0,
      flaky: false,
      paymentSpendAllowed: false,
      payoutMutationAllowed: false,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      rawBenchmarkInputCopyAllowed: false,
      runtimePromotionAllowed: false,
      scorecardCount: 2,
      settlementMutationAllowed: false,
      stateLabel: 'Passed evidence',
      suiteCount: 1,
      taskCount: 1,
      updatedAtDisplay: '6 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_BENCHMARK_CLOUD_READ_ONLY_AUTHORITY,
    )
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(omniBenchmarkCloudProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('validates suite, task, eval, scorecard, comparison, threshold, and duplicate linkage', () => {
    const base = exampleOmniBenchmarkCloud()

    for (const badRecord of [
      benchmarkRecord({ suites: [] }),
      benchmarkRecord({ tasks: [] }),
      benchmarkRecord({ evalJobs: [] }),
      benchmarkRecord({ scorecards: [] }),
      benchmarkRecord({
        suites: [
          { ...base.suites[0]!, taskRefs: ['task.public.missing'] },
        ],
      }),
      benchmarkRecord({
        tasks: [
          { ...base.tasks[0]!, suiteRefs: ['suite.public.missing'] },
        ],
      }),
      benchmarkRecord({
        evalJobs: [
          {
            ...base.evalJobs[0]!,
            taskRefs: ['task.public.missing'],
          },
          base.evalJobs[1]!,
        ],
      }),
      benchmarkRecord({
        scorecards: [
          {
            ...base.scorecards[0]!,
            observedScoreBps: 500,
            passThresholdBps: 9000,
            state: 'passed',
          },
          base.scorecards[1]!,
        ],
      }),
      benchmarkRecord({
        scorecards: [
          {
            ...base.scorecards[0]!,
            passThresholdBps: 10_001,
          },
          base.scorecards[1]!,
        ],
      }),
      benchmarkRecord({
        comparisons: [
          {
            ...base.comparisons[0]!,
            candidateEvalRefs: ['eval.public.missing'],
          },
        ],
      }),
      benchmarkRecord({
        tasks: [
          { ...base.tasks[0]!, taskRef: 'task.public.duplicate' },
          { ...base.tasks[0]!, taskRef: 'task.public.duplicate' },
        ],
      }),
    ]) {
      expect(() =>
        projectOmniBenchmarkCloud(badRecord, 'operator', nowIso),
      ).toThrow(OmniBenchmarkCloudUnsafe)
    }
  })

  test('labels regression and flaky evidence and requires promotion-blocking gate refs', () => {
    const base = exampleOmniBenchmarkCloud()
    const failedEval = {
      ...base.evalJobs[0]!,
      regressionRefs: ['regression.public.visual_grounding_drop'],
      state: 'failed' as const,
    }
    const failedScorecard = {
      ...base.scorecards[0]!,
      observedScoreBps: 7600,
      passThresholdBps: 9000,
      receiptRefs: [],
      state: 'failed' as const,
    }
    const regression = {
      affectedTaskRefs: ['task.public.site_revision_image_grounding'],
      baselineEvalRef: 'eval.public.autopilot_baseline_cloud',
      caveatRefs: ['caveat.public.regression_blocks_promotion'],
      evidenceRefs: ['evidence.public.visual_grounding_regression'],
      promotionBlocking: true,
      promotionGateRefs: ['gate.public.model_lab_promotion_review'],
      regressionRef: 'regression.public.visual_grounding_drop',
      severity: 'high' as const,
      sourceEvalRef: 'eval.public.autopilot_candidate_cloud',
    }
    const projection = projectOmniBenchmarkCloud(
      benchmarkRecord({
        blockerRefs: ['blocker.public.visual_grounding_regression'],
        evalJobs: [failedEval, base.evalJobs[1]!],
        promotionGateRefs: ['gate.public.model_lab_promotion_review'],
        regressions: [regression],
        scorecards: [failedScorecard, base.scorecards[1]!],
        state: 'failed',
      }),
      'operator',
      nowIso,
    )

    expect(projection.promotionBlocked).toBe(true)
    expect(projection.failedScorecardCount).toBe(1)
    expect(projection.regressionCount).toBe(1)

    const flake = {
      caveatRefs: ['caveat.public.flake_rate_needs_rerun'],
      evalJobRefs: ['eval.public.autopilot_candidate_cloud'],
      evidenceRefs: ['evidence.public.flake_replay_summary'],
      flakeRateBps: 1200,
      flakeRef: 'flake.public.visual_grounding_intermittent',
      taskRefs: ['task.public.site_revision_image_grounding'],
    }
    const flakyProjection = projectOmniBenchmarkCloud(
      benchmarkRecord({
        evalJobs: [
          {
            ...base.evalJobs[0]!,
            flakeRefs: ['flake.public.visual_grounding_intermittent'],
            state: 'flaky',
          },
          base.evalJobs[1]!,
        ],
        flakes: [flake],
        state: 'flaky',
      }),
      'operator',
      nowIso,
    )

    expect(flakyProjection.flaky).toBe(true)
    expect(flakyProjection.flakeCount).toBe(1)

    expect(() =>
      projectOmniBenchmarkCloud(
        benchmarkRecord({
          blockerRefs: ['blocker.public.visual_grounding_regression'],
          evalJobs: [failedEval, base.evalJobs[1]!],
          regressions: [{ ...regression, promotionGateRefs: [] }],
          scorecards: [failedScorecard, base.scorecards[1]!],
          state: 'failed',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniBenchmarkCloudUnsafe)
  })

  test('redacts private benchmark refs, providers, runners, datasets, tasks, scorecards, regressions, flakes, comparisons, and evidence publicly', () => {
    const base = exampleOmniBenchmarkCloud()
    const projection = projectOmniBenchmarkCloud(
      benchmarkRecord({
        benchmarkRef: 'benchmark.private.operator_cloud',
        candidateRefs: ['candidate.private.operator_candidate'],
        caveatRefs: [
          'caveat.public.benchmark_cloud_evidence_only',
          'caveat.private.operator_note',
        ],
        comparisons: [
          base.comparisons[0]!,
          {
            ...base.comparisons[0]!,
            candidateEvalRefs: ['eval.private.operator_candidate'],
            comparisonRef: 'comparison.private.operator_comparison',
            evidenceRefs: [
              'evidence.public.comparison_summary',
              'evidence.private.operator_comparison',
            ],
            scorecardRefs: ['scorecard.private.operator_scorecard'],
          },
        ],
        evalJobs: [
          base.evalJobs[0]!,
          base.evalJobs[1]!,
          {
            ...base.evalJobs[0]!,
            comparisonRefs: ['comparison.private.operator_comparison'],
            evalJobRef: 'eval.private.operator_candidate',
            providerRefs: ['provider.private.operator_payload'],
            runnerRefs: ['runner.private.operator_runner'],
            scorecardRefs: ['scorecard.private.operator_scorecard'],
            suiteRefs: ['suite.private.operator_suite'],
            taskRefs: ['task.private.operator_task'],
          },
        ],
        id: 'benchmark.private.operator_cloud',
        scorecards: [
          base.scorecards[0]!,
          base.scorecards[1]!,
          {
            ...base.scorecards[0]!,
            evalJobRefs: ['eval.private.operator_candidate'],
            scorecardRef: 'scorecard.private.operator_scorecard',
          },
        ],
        suites: [
          base.suites[0]!,
          {
            ...base.suites[0]!,
            suiteRef: 'suite.private.operator_suite',
            taskRefs: ['task.private.operator_task'],
          },
        ],
        tasks: [
          base.tasks[0]!,
          {
            ...base.tasks[0]!,
            datasetRefs: ['dataset.private.operator_fixture_ref'],
            suiteRefs: ['suite.private.operator_suite'],
            taskRef: 'task.private.operator_task',
          },
        ],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.benchmarkRef).toBe('benchmark.redacted.cloud')
    expect(projection.id).toBe('benchmark-cloud.redacted')
    expect(
      projection.evalJobs.some(
        evalJob => evalJob.evalJobRef === 'eval.redacted.benchmark_cloud',
      ),
    ).toBe(true)
    expect(
      projection.scorecards.some(
        scorecard =>
          scorecard.scorecardRef === 'scorecard.redacted.benchmark_cloud',
      ),
    ).toBe(true)
    expect(serialized).not.toContain('private')
    expect(serialized).not.toContain('operator')
    expect(serialized).not.toContain('provider.')
    expect(omniBenchmarkCloudProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('rejects private prompts, raw benchmark inputs, provider payloads, datasets, model weights, secrets, payment material, raw timestamps, and mutable authority', () => {
    for (const badRecord of [
      benchmarkRecord({ caveatRefs: ['raw_prompt.customer'] }),
      benchmarkRecord({ blockerRefs: ['raw_benchmark_input.customer'] }),
      benchmarkRecord({ caveatRefs: ['provider_payload.raw'] }),
      benchmarkRecord({ blockerRefs: ['dataset.raw.customer'] }),
      benchmarkRecord({ blockerRefs: ['weights.safetensors'] }),
      benchmarkRecord({ caveatRefs: ['secret.benchmark_cloud_token'] }),
      benchmarkRecord({ caveatRefs: ['payment_preimage.raw'] }),
      benchmarkRecord({ caveatRefs: ['caveat.public.2026-06-06T23:00:00'] }),
      benchmarkRecord({
        authority: {
          ...OMNI_BENCHMARK_CLOUD_READ_ONLY_AUTHORITY,
          noBenchmarkLaunch: false,
        },
      }),
      benchmarkRecord({
        authority: {
          ...OMNI_BENCHMARK_CLOUD_READ_ONLY_AUTHORITY,
          noPaymentSpend: false,
        },
      }),
    ]) {
      expect(() =>
        projectOmniBenchmarkCloud(badRecord, 'operator', nowIso),
      ).toThrow(OmniBenchmarkCloudUnsafe)
    }
  })
})
