import { describe, expect, it } from 'vitest'

import {
  buildTrainingLeaderboardsProjection,
  TrainingLeaderboardLanes,
} from './training-leaderboards'
import {
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
  publicTrainingRunSummary,
} from './training-run-window-authority'
import {
  publicDeviceCapabilityProjection,
} from './training-device-capability'
import {
  publicCs336A5EvalProjection,
} from './cs336-a5-alignment-homework'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

describe('training leaderboards', () => {
  it('keeps every configured lane visible when no verified rows exist', () => {
    const projection = buildTrainingLeaderboardsProjection({
      a2Projections: [],
      a5Projections: [],
      runs: [],
      summaries: [],
    })

    expect(projection.schemaVersion).toBe('openagents.training.leaderboards.v1')
    expect(projection.lanes.map(lane => lane.lane)).toEqual(
      TrainingLeaderboardLanes,
    )
    expect(projection.lanes.every(lane => lane.rows.length === 0)).toBe(true)
    expect(projection.blockerRefs).toContain(
      'blocker.training_leaderboard.a1_loss.requires_verified_receipts',
    )
  })

  it('filters unverified rows before ranking public leaderboards', () => {
    const a1Run = buildTrainingRunRecord({
      makeId: () => 'a1',
      nowIso: '2026-06-10T14:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a1.leaderboard',
      },
    })
    const a1Window = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-10T14:00:00.000Z',
      request: {
        homeworkKind: 'admin_dispatched_homework',
        trainingRunRef: a1Run.trainingRunRef,
        windowRef: 'training.window.cs336.a1.leaderboard.1',
      },
    })
    const a1Challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'challenge',
      nowIso: '2026-06-10T14:01:00.000Z',
      request: {
        commitmentRefs: ['commitment.cs336.a1.leaderboard.1'],
        contributionRef: 'contribution.cs336.a1.leaderboard.1',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          expectedDigestRef: 'digest.cs336.a1.leaderboard.1',
          recomputedDigestRef: 'digest.cs336.a1.leaderboard.1',
        },
        trainingRunRef: a1Run.trainingRunRef,
        verificationClass: 'deterministic_recompute',
        windowRef: a1Window.windowRef,
      },
    }).challenge
    const leased = leaseTrainingVerificationChallengeRecord({
      challenge: a1Challenge,
      eventId: 'lease',
      nowIso: '2026-06-10T14:02:00.000Z',
      request: { validatorRef: 'validator.cs336.leaderboard' },
    }).challenge
    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: 'final',
      nowIso: '2026-06-10T14:03:00.000Z',
      request: { receiptRefs: ['receipt.cs336.leaderboard.verdict'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.cs336.leaderboard.1'],
      },
    }).challenge
    const a1Summary = publicTrainingRunSummary({
      challenges: [verified],
      leases: [
        {
          claimedAt: '2026-06-10T14:02:00.000Z',
          id: 'lease.cs336.a1.leaderboard.1',
          leaseExpiresAt: '2026-06-10T14:17:00.000Z',
          leaseRef: 'lease.cs336.a1.leaderboard.1',
          publicProjectionJson: '{}',
          pylonRef: 'pylon.public.leaderboard.1',
          receiptRefs: ['receipt.cs336.a1.lease.1'],
          state: 'active',
          trainingRunRef: a1Run.trainingRunRef,
          windowRef: a1Window.windowRef,
        },
      ],
      nowIso: '2026-06-10T14:04:00.000Z',
      run: {
        ...a1Run,
        publicProjectionJson: JSON.stringify({
          realGradient: {
            lossCurve: [
              {
                sourceRefs: ['artifact.cs336.a1.loss.1'],
                step: 4,
                validationLoss: 1.5,
              },
            ],
            lossUnderBudget: {
              budgetLabel: 'CS336 A1 bounded public run',
              finalValidationLoss: 1.5,
              maxValidationLoss: 2,
            },
          },
        }),
      },
      windows: [a1Window],
    })
    const a2Run = {
      ...buildTrainingRunRecord({
        makeId: () => 'a2',
        nowIso: '2026-06-10T14:00:00.000Z',
        request: {
          promiseRef: 'pylon.compute_revenue_modes.v1',
          trainingRunRef: 'training.run.cs336.a2.leaderboard',
        },
      }),
      publicProjectionJson: JSON.stringify({
        a2DeviceBenchmark: {
          measurements: [
            {
              deviceClassRef: 'device_class.apple_silicon.m3_pro_18gb',
              max: 2060,
              metric: 'tokens_per_second',
              min: 1710,
              p50: 1900,
              p90: 2025,
              receiptRefs: ['receipt.cs336.a2.measurement.1'],
              sampleCount: 4,
              unit: 'tokens_per_second',
              verificationRefs: ['challenge.cs336.a2.class_check.1'],
              workClass: 'small_model_local_training',
            },
            {
              deviceClassRef: 'device_class.unverified',
              max: 9999,
              metric: 'tokens_per_second',
              min: 9999,
              p50: 9999,
              p90: 9999,
              sampleCount: 1,
              unit: 'tokens_per_second',
              workClass: 'small_model_local_training',
            },
          ],
        },
      }),
    }
    const a5Run = {
      ...buildTrainingRunRecord({
        makeId: () => 'a5',
        nowIso: '2026-06-10T14:00:00.000Z',
        request: {
          promiseRef: 'pylon.compute_revenue_modes.v1',
          trainingRunRef: 'training.run.cs336.a5.leaderboard',
        },
      }),
      publicProjectionJson: JSON.stringify({
        a5Alignment: {
          evalSuites: [
            {
              evalSuiteRef: 'eval.cs336.a5.gsm8k.1',
              metric: 'accuracy',
              receiptRefs: ['receipt.cs336.a5.gsm8k.1'],
              sampleCount: 100,
              score: 0.42,
              splitRef: 'gsm8k.test.public_summary',
              taskSetRef: 'gsm8k',
              verificationRefs: ['challenge.cs336.a5.gsm8k.1'],
              verifiedSampleCount: 100,
            },
          ],
        },
      }),
    }
    const a4Run = {
      ...buildTrainingRunRecord({
        makeId: () => 'a4',
        nowIso: '2026-06-10T14:00:00.000Z',
        request: {
          promiseRef: 'pylon.compute_revenue_modes.v1',
          trainingRunRef: 'training.run.cs336.a4.leaderboard',
        },
      }),
      publicProjectionJson: JSON.stringify({
        a4DataRefinery: {
          leaderboardRows: [
            {
              contributorRef: 'pylon.public.data_refinery.1',
              evalDelta: 0.08,
              receiptRefs: ['receipt.cs336.a4.eval_delta.1'],
              sourceRefs: ['artifact.cs336.a4.eval_delta.1'],
              verificationRefs: ['challenge.cs336.a4.eval_delta.1'],
            },
            {
              contributorRef: 'pylon.public.data_refinery.unverified',
              evalDelta: 1,
            },
          ],
        },
      }),
    }
    const projection = buildTrainingLeaderboardsProjection({
      a2Projections: [
        publicDeviceCapabilityProjection({
          challenges: [],
          leases: [],
          run: a2Run,
          windows: [],
        }),
      ],
      a5Projections: [
        publicCs336A5EvalProjection({
          challenges: [],
          leases: [],
          run: a5Run,
          windows: [],
        }),
      ],
      runs: [a1Run, a2Run, a4Run, a5Run],
      summaries: [a1Summary],
    })

    expect(
      projection.lanes.find(lane => lane.lane === 'a1_loss')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'pylon.public.leaderboard.1',
        rank: 1,
        score: 1.5,
      }),
    ])
    expect(
      projection.lanes.find(lane => lane.lane === 'a2_throughput')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'device_class.apple_silicon.m3_pro_18gb',
        rank: 1,
        score: 2025,
      }),
    ])
    expect(
      projection.lanes.find(lane => lane.lane === 'a4_eval_delta')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'pylon.public.data_refinery.1',
        rank: 1,
        score: 0.08,
      }),
    ])
    expect(
      projection.lanes.find(lane => lane.lane === 'a5_accuracy')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'eval.cs336.a5.gsm8k.1',
        rank: 1,
        score: 0.42,
      }),
    ])
    expect(JSON.stringify(projection)).not.toContain('unverified')
  })
})
