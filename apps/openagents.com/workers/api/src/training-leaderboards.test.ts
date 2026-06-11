import { describe, expect, it } from 'vitest'

import {
  buildTrainingLeaderboardsProjection,
  settledSatsFromPaymentAuthorityReceipt,
  TrainingLeaderboardLanes,
} from './training-leaderboards'
import { publicScalingSweepProjection } from './training-scaling-sweep'
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
      a3Projections: [],
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
    const a3Run = {
      ...buildTrainingRunRecord({
        makeId: () => 'a3',
        nowIso: '2026-06-10T14:00:00.000Z',
        request: {
          promiseRef: 'pylon.compute_revenue_modes.v1',
          trainingRunRef: 'training.run.cs336.a3.leaderboard',
        },
      }),
      publicProjectionJson: JSON.stringify({
        a3ScalingSweep: {
          cells: [
            {
              cellRef: 'cell.cs336.a3.leaderboard.verified.1',
              computeBudgetFlops: 300000000,
              parameterCount: 8192,
              pylonRef: 'pylon.public.sweep.1',
              receiptRefs: ['receipt.cs336.a3.cell.1'],
              sourceRefs: ['commitment.cs336.a3.cell.1'],
              tokenCount: 6103,
              validationLoss: 4.91,
              verificationRefs: ['challenge.cs336.a3.cell.1'],
            },
            {
              cellRef: 'cell.cs336.a3.leaderboard.verified.2',
              computeBudgetFlops: 300000000,
              parameterCount: 16384,
              pylonRef: 'pylon.public.sweep.1',
              receiptRefs: ['receipt.cs336.a3.cell.2'],
              sourceRefs: ['commitment.cs336.a3.cell.2'],
              tokenCount: 3051,
              validationLoss: 5.2,
              verificationRefs: ['challenge.cs336.a3.cell.2'],
            },
            {
              cellRef: 'cell.cs336.a3.leaderboard.unverified',
              computeBudgetFlops: 300000000,
              parameterCount: 4096,
              pylonRef: 'pylon.public.sweep.unverified',
              receiptRefs: ['receipt.cs336.a3.cell.unverified'],
              sourceRefs: [],
              tokenCount: 12207,
              validationLoss: 0.01,
            },
            {
              cellRef: 'cell.cs336.a3.leaderboard.unreceipted',
              computeBudgetFlops: 300000000,
              parameterCount: 2048,
              pylonRef: 'pylon.public.sweep.unreceipted',
              receiptRefs: [],
              sourceRefs: [],
              tokenCount: 24414,
              validationLoss: 0.02,
              verificationRefs: ['challenge.cs336.a3.cell.unreceipted'],
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
      a3Projections: [
        publicScalingSweepProjection({
          challenges: [],
          leases: [],
          run: a3Run,
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
      runs: [a1Run, a2Run, a3Run, a4Run, a5Run],
      settledSatsByReceiptRef: new Map([
        ['receipt.cs336.a3.cell.1', 10],
        ['receipt.cs336.a3.cell.unverified', 500],
      ]),
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
      projection.lanes.find(lane => lane.lane === 'a3_isoflop')?.rows,
    ).toEqual([
      expect.objectContaining({
        contributorRef: 'pylon.public.sweep.1',
        metricRef: 'metric.cs336_a3.validation_loss.c_300000000',
        rank: 1,
        receiptRefs: ['receipt.cs336.a3.cell.1'],
        score: 4.91,
        scoreSortDirection: 'asc',
        settledPayoutSats: 10,
        trainingRunRef: 'training.run.cs336.a3.leaderboard',
      }),
    ])
    expect(
      projection.lanes
        .flatMap(lane => lane.rows)
        .every(row => row.provenanceLabel.includes('provider-confirmed')),
    ).toBe(true)
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

  it('counts settled sats only from provider-confirmed settlement receipts', () => {
    expect(
      settledSatsFromPaymentAuthorityReceipt({
        publicProjectionJson: JSON.stringify({ amountSats: 30, state: 'settled' }),
        receiptKind: 'settlement_recorded',
      }),
    ).toBe(30)
    expect(
      settledSatsFromPaymentAuthorityReceipt({
        publicProjectionJson: JSON.stringify({ amountSats: 30, state: 'pending' }),
        receiptKind: 'settlement_recorded',
      }),
    ).toBe(0)
    expect(
      settledSatsFromPaymentAuthorityReceipt({
        publicProjectionJson: JSON.stringify({ amountSats: 30, state: 'settled' }),
        receiptKind: 'payout_dispatched',
      }),
    ).toBe(0)
    expect(
      settledSatsFromPaymentAuthorityReceipt({
        publicProjectionJson: JSON.stringify({ amountSats: 12.5, state: 'settled' }),
        receiptKind: 'settlement_recorded',
      }),
    ).toBe(0)
    expect(
      settledSatsFromPaymentAuthorityReceipt({
        publicProjectionJson: 'not-json',
        receiptKind: 'settlement_recorded',
      }),
    ).toBe(0)
  })
})
