import { describe, expect, it } from 'vitest'

import {
  TrainingAuthorityStoreError,
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
  publicTrainingRunSummary,
  selectTrainingLeaseCandidate,
  transitionTrainingWindowRecord,
} from './training-run-window-authority'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

describe('training run window authority', () => {
  it('prefers admin-dispatched homework before auto-launched starter windows', () => {
    const starter = buildTrainingWindowRecord({
      makeId: () => 'starter',
      nowIso: '2026-06-10T10:00:00.000Z',
      request: {
        homeworkKind: 'auto_starter',
        priority: 100,
        trainingRunRef: 'training.run.0001',
        windowRef: 'training.window.starter',
      },
    })
    const admin = buildTrainingWindowRecord({
      makeId: () => 'admin',
      nowIso: '2026-06-10T10:01:00.000Z',
      request: {
        homeworkKind: 'admin_dispatched_homework',
        priority: 0,
        trainingRunRef: 'training.run.0001',
        windowRef: 'training.window.admin',
      },
    })

    expect(
      selectTrainingLeaseCandidate([
        { ...starter, state: 'active' },
        { ...admin, state: 'active' },
      ])?.windowRef,
    ).toBe('training.window.admin')
  })

  it('requires planned active sealed reconciled window transition order', () => {
    const planned = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-10T10:00:00.000Z',
      request: {
        trainingRunRef: 'training.run.0001',
        windowRef: 'training.window.0001',
      },
    })
    const active = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'activate',
      nextState: 'active',
      nowIso: '2026-06-10T10:05:00.000Z',
      receiptRef: 'receipt.training.activate',
      transitionKind: 'window_activate',
      window: planned,
    }).window
    const sealed = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'seal',
      nextState: 'sealed',
      nowIso: '2026-06-10T10:10:00.000Z',
      receiptRef: 'receipt.training.seal',
      transitionKind: 'window_seal',
      window: active,
    }).window
    const reconciled = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'reconcile',
      nextState: 'reconciled',
      nowIso: '2026-06-10T10:15:00.000Z',
      receiptRef: 'receipt.training.reconcile',
      transitionKind: 'window_reconcile',
      window: sealed,
    }).window

    expect(reconciled.state).toBe('reconciled')
    expect(reconciled.receiptRefs).toEqual([
      'receipt.training.activate',
      'receipt.training.reconcile',
      'receipt.training.seal',
    ])
    expect(() =>
      transitionTrainingWindowRecord({
        actorRef: 'operator.training',
        eventId: 'invalid',
        nextState: 'reconciled',
        nowIso: '2026-06-10T10:20:00.000Z',
        receiptRef: 'receipt.training.invalid',
        transitionKind: 'window_reconcile',
        window: active,
      }),
    ).toThrow(TrainingAuthorityStoreError)
  })

  it('projects real-gradient blockers when Psionic and live-device evidence is missing', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a1',
      nowIso: '2026-06-10T10:00:00.000Z',
      request: {
        promiseRef: 'pylon.first_real_model_training_run.v1',
        trainingRunRef: 'training.run.cs336.a1.real_gradient',
      },
    })
    const summary = publicTrainingRunSummary({
      challenges: [],
      leases: [],
      nowIso: '2026-06-10T10:05:00.000Z',
      run,
      windows: [],
    })

    expect(summary.realGradient).toMatchObject({
      closeoutRequirement: { satisfied: false },
      deviceRequirement: {
        observedDistinctContributorDevices: 0,
        requiredDistinctContributorDevices: 2,
        satisfied: false,
      },
      externalAsk: {
        blockerRefs: [
          'blocker.cs336_a1.real_gradient_psionic_lane_external',
          'blocker.cs336_a1.requires_two_real_contributor_devices',
          'blocker.cs336_a1.operator_funded_settled_payouts_required',
        ],
        psionicLaneRef: 'psion_cs336_a1_real_gradient_v1',
        status: 'blocked_external',
      },
      lossUnderBudget: { satisfied: false },
    })
    expect(summary.realGradient.scopeBoundaryRefs).toContain(
      'scope.cs336_a1.no_first_real_training_run_green_copy_from_this_issue_alone',
    )
  })

  it('projects observed real-gradient status only with two devices, commitments, eval refs, and loss under budget', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'a1',
      nowIso: '2026-06-10T10:00:00.000Z',
      request: {
        promiseRef: 'pylon.first_real_model_training_run.v1',
        trainingRunRef: 'training.run.cs336.a1.real_gradient',
      },
    })
    const run = {
      ...runBase,
      publicProjectionJson: JSON.stringify({
        realGradient: {
          budgetLabel: 'CS336 A1 validation loss <= 2.4 under demo budget.',
          budgetRef: 'budget.cs336.a1.demo',
          evalRef: 'eval.cs336.a1.validation_loss.1',
          freivaldsCommitmentRefs: ['commitment.cs336.a1.gradient.window.1'],
          gradientCloseoutRefs: ['closeout.cs336.a1.gradient.window.1'],
          lossCurve: [
            {
              provenanceLabel: 'Psionic public eval receipt.',
              sourceRefs: ['eval.cs336.a1.validation_loss.1'],
              step: 1,
              validationLoss: 2.9,
            },
            {
              provenanceLabel: 'Psionic public eval receipt.',
              sourceRefs: ['eval.cs336.a1.validation_loss.2'],
              step: 2,
              validationLoss: 2.1,
            },
          ],
          maxValidationLoss: 2.4,
          mergeRef: 'merge.cs336.a1.gradient.1',
          psionicLaneRef: 'psion_cs336_a1_real_gradient_v1',
        },
      }),
    }
    const window = {
      ...buildTrainingWindowRecord({
        makeId: () => 'window',
        nowIso: '2026-06-10T10:00:00.000Z',
        request: {
          trainingRunRef: run.trainingRunRef,
          windowRef: 'training.window.cs336.a1.gradient.1',
        },
      }),
      state: 'reconciled' as const,
    }
    const leases = [
      {
        claimedAt: '2026-06-10T10:01:00.000Z',
        id: 'lease_1',
        leaseExpiresAt: '2026-06-10T10:11:00.000Z',
        leaseRef: 'training.lease.cs336.a1.device.1',
        publicProjectionJson: '{}',
        pylonRef: 'pylon.cs336.a1.device1',
        receiptRefs: ['receipt.lease.device1'],
        state: 'released' as const,
        trainingRunRef: run.trainingRunRef,
        windowRef: window.windowRef,
      },
      {
        claimedAt: '2026-06-10T10:02:00.000Z',
        id: 'lease_2',
        leaseExpiresAt: '2026-06-10T10:12:00.000Z',
        leaseRef: 'training.lease.cs336.a1.device.2',
        publicProjectionJson: '{}',
        pylonRef: 'pylon.cs336.a1.device2',
        receiptRefs: ['receipt.lease.device2'],
        state: 'released' as const,
        trainingRunRef: run.trainingRunRef,
        windowRef: window.windowRef,
      },
    ]
    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'challenge',
      nowIso: '2026-06-10T10:03:00.000Z',
      request: {
        commitmentRefs: ['commitment.cs336.a1.gradient.window.1'],
        contributionRef: 'contribution.cs336.a1.gradient.window.1',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          commitmentRef: 'commitment.cs336.a1.gradient.window.1',
          matrixShape: [4, 4],
          verifierSeedRef: 'seed.cs336.a1.freivalds.1',
        },
        trainingRunRef: run.trainingRunRef,
        verificationClass: 'freivalds_merkle',
        windowRef: window.windowRef,
      },
    }).challenge
    const leasedChallenge = leaseTrainingVerificationChallengeRecord({
      challenge,
      eventId: 'lease',
      nowIso: '2026-06-10T10:04:00.000Z',
      request: { validatorRef: 'validator.cs336.a1' },
    }).challenge
    const verifiedChallenge = finalizeTrainingVerificationChallengeRecord({
      challenge: leasedChallenge,
      eventId: 'final',
      nowIso: '2026-06-10T10:05:00.000Z',
      request: { receiptRefs: ['receipt.cs336.a1.verdict'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.cs336.a1.freivalds'],
      },
    }).challenge
    const summary = publicTrainingRunSummary({
      challenges: [verifiedChallenge],
      leases,
      nowIso: '2026-06-10T10:06:00.000Z',
      run,
      windows: [window],
    })

    expect(summary.realGradient.externalAsk.status).toBe('observed')
    expect(summary.realGradient.externalAsk.blockerRefs).toEqual([])
    expect(summary.realGradient.deviceRequirement.satisfied).toBe(true)
    expect(summary.realGradient.closeoutRequirement.satisfied).toBe(true)
    expect(summary.realGradient.lossUnderBudget).toMatchObject({
      finalValidationLoss: 2.1,
      maxValidationLoss: 2.4,
      satisfied: true,
    })
    expect(summary.realGradient.lossCurve.map(point => point.step)).toEqual([
      1, 2,
    ])
    expect(summary.realGradient.leaderboardRows).toHaveLength(2)
    expect(summary.realGradient.leaderboardRows[0]).toMatchObject({
      bestValidationLoss: 2.1,
      pylonRef: 'pylon.cs336.a1.device1',
      settledPayoutSats: 0,
      trainingRunRef: 'training.run.cs336.a1.real_gradient',
      verifiedWindowCount: 1,
    })
  })
})
