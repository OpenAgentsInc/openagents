import { describe, expect, it } from 'vitest'

import {
  DefaultMaxAllowedStaleSteps,
  TrainingAuthorityStoreError,
  type TrainingWindowSealMetadata,
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
  publicTrainingRunProjection,
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

  it('seals a window carrying staleness, churn, and verification-overhead fields and projects them', () => {
    const sealMetadata: TrainingWindowSealMetadata = {
      churn: {
        events: [
          { eventRef: 'event.churn.join.pylon.device3', kind: 'join' },
          { eventRef: 'event.churn.loss.pylon.device2', kind: 'loss' },
          {
            eventRef: 'event.churn.standby_promotion.pylon.device4',
            kind: 'standby_promotion',
          },
        ],
        joinCount: 1,
        lossCount: 1,
        standbyPromotionCount: 1,
      },
      staleness: {
        contributionCount: 3,
        contributions: [
          {
            contributionRef: 'contribution.window.0001.pylon.device1',
            stepsBehind: 0,
          },
          {
            contributionRef: 'contribution.window.0001.pylon.device3',
            stepsBehind: 2,
          },
          {
            contributionRef: 'contribution.window.0001.pylon.device4',
            stepsBehind: 4,
          },
        ],
        stepsBehindMax: 4,
        stepsBehindMin: 0,
        stepsBehindP50: 2,
        stepsBehindP90: 4,
      },
      verificationOverhead: {
        fraction: 0.18,
        ladderRungRef: 'ladder.rung.r1',
      },
    }
    const active = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'activate',
      nextState: 'active',
      nowIso: '2026-06-12T10:05:00.000Z',
      receiptRef: 'receipt.training.activate',
      transitionKind: 'window_activate',
      window: buildTrainingWindowRecord({
        makeId: () => 'window',
        nowIso: '2026-06-12T10:00:00.000Z',
        request: {
          trainingRunRef: 'training.run.0001',
          windowRef: 'training.window.0001',
        },
      }),
    }).window
    const sealed = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'seal',
      nextState: 'sealed',
      nowIso: '2026-06-12T10:10:00.000Z',
      receiptRef: 'receipt.training.seal',
      sealMetadata,
      transitionKind: 'window_seal',
      window: active,
    }).window

    expect(sealed.state).toBe('sealed')
    expect(sealed.sealMetadata).toEqual(sealMetadata)
    expect(JSON.parse(sealed.publicProjectionJson).sealMetadata).toEqual(
      sealMetadata,
    )

    const reconciled = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'reconcile',
      nextState: 'reconciled',
      nowIso: '2026-06-12T10:15:00.000Z',
      receiptRef: 'receipt.training.reconcile',
      transitionKind: 'window_reconcile',
      window: sealed,
    }).window

    expect(reconciled.sealMetadata).toEqual(sealMetadata)
  })

  it('keeps seal metadata optional so existing seal calls still work', () => {
    const active = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'activate',
      nextState: 'active',
      nowIso: '2026-06-12T10:05:00.000Z',
      receiptRef: 'receipt.training.activate',
      transitionKind: 'window_activate',
      window: buildTrainingWindowRecord({
        makeId: () => 'window',
        nowIso: '2026-06-12T10:00:00.000Z',
        request: {
          trainingRunRef: 'training.run.0001',
          windowRef: 'training.window.0001',
        },
      }),
    }).window
    const sealed = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'seal',
      nextState: 'sealed',
      nowIso: '2026-06-12T10:10:00.000Z',
      receiptRef: 'receipt.training.seal',
      transitionKind: 'window_seal',
      window: active,
    }).window

    expect(sealed.sealMetadata).toBeNull()
    expect(JSON.parse(sealed.publicProjectionJson).sealMetadata).toBeNull()
  })

  it('requires checkpoint-backed seals to carry a passing durable checkpoint descriptor', () => {
    const checkpointDigestRef = `sha256:${'a'.repeat(64)}`
    const sealMetadata: TrainingWindowSealMetadata = {
      checkpointDigestRef,
      churn: { joinCount: 0, lossCount: 0, standbyPromotionCount: 0 },
      durableCheckpointSeal: {
        checkpointDigestRef,
        readbackRehashReceiptRef:
          'receipt.training.checkpoint_readback_rehash.window.0001',
        replicationFactor: 2,
        remoteCheckpointObjectRef: 'r2.training_checkpoint.window.0001',
        remoteCheckpointStoreRef: 'r2.openagents_autopilot_artifacts.training',
        retrievalProofRef: 'receipt.training.checkpoint_readback.window.0001',
        retrievalVerified: true,
        sizeBytes: 1_048_576,
        storageClass: 'content_addressed_object_store',
        windowRef: 'training.window.0001',
      },
      staleness: {
        contributionCount: 0,
        stepsBehindMax: 0,
        stepsBehindMin: 0,
        stepsBehindP50: 0,
        stepsBehindP90: 0,
      },
      verificationOverhead: {
        fraction: 0.1,
        ladderRungRef: 'ladder.rung.r1',
      },
    }
    const active = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'activate',
      nextState: 'active',
      nowIso: '2026-06-12T10:05:00.000Z',
      receiptRef: 'receipt.training.activate',
      transitionKind: 'window_activate',
      window: buildTrainingWindowRecord({
        makeId: () => 'window',
        nowIso: '2026-06-12T10:00:00.000Z',
        request: {
          trainingRunRef: 'training.run.0001',
          windowRef: 'training.window.0001',
        },
      }),
    }).window

    const sealed = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'seal',
      nextState: 'sealed',
      nowIso: '2026-06-12T10:10:00.000Z',
      receiptRef: 'receipt.training.seal',
      sealMetadata,
      transitionKind: 'window_seal',
      window: active,
    }).window

    expect(sealed.sealMetadata?.durableCheckpointSeal).toMatchObject({
      checkpointDigestRef,
      retrievalVerified: true,
      storageClass: 'content_addressed_object_store',
      windowRef: 'training.window.0001',
    })
  })

  it('rejects malformed or misplaced seal metadata', () => {
    const validSealMetadata: TrainingWindowSealMetadata = {
      churn: { joinCount: 0, lossCount: 0, standbyPromotionCount: 0 },
      staleness: {
        contributionCount: 1,
        contributions: [
          {
            contributionRef: 'contribution.window.0001.pylon.device1',
            stepsBehind: 1,
          },
        ],
        stepsBehindMax: 1,
        stepsBehindMin: 1,
        stepsBehindP50: 1,
        stepsBehindP90: 1,
      },
      verificationOverhead: {
        fraction: 0.1,
        ladderRungRef: 'ladder.rung.r1',
      },
    }
    const planned = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-12T10:00:00.000Z',
      request: {
        trainingRunRef: 'training.run.0001',
        windowRef: 'training.window.0001',
      },
    })
    const sealAttempt = (
      sealMetadata: TrainingWindowSealMetadata,
    ): (() => unknown) => {
      const active = transitionTrainingWindowRecord({
        actorRef: 'operator.training',
        eventId: 'activate',
        nextState: 'active',
        nowIso: '2026-06-12T10:05:00.000Z',
        receiptRef: 'receipt.training.activate',
        transitionKind: 'window_activate',
        window: planned,
      }).window

      return () =>
        transitionTrainingWindowRecord({
          actorRef: 'operator.training',
          eventId: 'seal',
          nextState: 'sealed',
          nowIso: '2026-06-12T10:10:00.000Z',
          receiptRef: 'receipt.training.seal',
          sealMetadata,
          transitionKind: 'window_seal',
          window: active,
        })
    }

    const expectSealValidationError = (
      run: () => unknown,
      pattern: RegExp,
    ): void => {
      try {
        run()
      } catch (error) {
        expect(error).toBeInstanceOf(TrainingAuthorityStoreError)
        expect(error).toMatchObject({
          kind: 'validation_error',
          reason: expect.stringMatching(pattern),
        })

        return
      }

      expect.unreachable('expected a seal-metadata validation error')
    }

    expectSealValidationError(
      () =>
        transitionTrainingWindowRecord({
          actorRef: 'operator.training',
          eventId: 'activate',
          nextState: 'active',
          nowIso: '2026-06-12T10:05:00.000Z',
          receiptRef: 'receipt.training.activate',
          sealMetadata: validSealMetadata,
          transitionKind: 'window_activate',
          window: planned,
        }),
      /only accepted on the seal transition/,
    )
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        staleness: {
          ...validSealMetadata.staleness,
          contributions: [
            {
              contributionRef: 'contribution.window.0001.pylon.device1',
              stepsBehind: -1,
            },
          ],
          stepsBehindMin: 0,
        },
      }),
      /non-negative integer/,
    )
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        verificationOverhead: {
          fraction: 1.2,
          ladderRungRef: 'ladder.rung.r1',
        },
      }),
      /between 0 and 1/,
    )
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        staleness: {
          ...validSealMetadata.staleness,
          stepsBehindMin: 3,
        },
      }),
      /min <= p50 <= p90 <= max/,
    )
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        staleness: {
          ...validSealMetadata.staleness,
          contributionCount: 0,
        },
      }),
      /cannot exceed staleness.contributionCount/,
    )
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        churn: {
          events: [
            { eventRef: 'event.churn.join.pylon.device3', kind: 'join' },
          ],
          joinCount: 0,
          lossCount: 0,
          standbyPromotionCount: 0,
        },
      }),
      /more join refs than the declared join count/,
    )

    const checkpointDigestRef = `sha256:${'b'.repeat(64)}`
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        checkpointDigestRef,
      }),
      /requires a durableCheckpointSeal descriptor/,
    )
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        checkpointDigestRef,
        durableCheckpointSeal: {
          checkpointDigestRef,
          readbackRehashReceiptRef:
            'receipt.training.checkpoint_readback_rehash.window.0001',
          replicationFactor: 1,
          remoteCheckpointObjectRef: 'r2.training_checkpoint.window.0001',
          remoteCheckpointStoreRef:
            'r2.openagents_autopilot_artifacts.training',
          retrievalVerified: true,
          sizeBytes: 1_048_576,
          storageClass: 'content_addressed_object_store',
          windowRef: 'training.window.0001',
        },
      }),
      /replication_factor_below_durable_minimum/,
    )
    expectSealValidationError(
      sealAttempt({
        ...validSealMetadata,
        checkpointDigestRef,
        durableCheckpointSeal: {
          checkpointDigestRef,
          readbackRehashReceiptRef:
            'receipt.training.checkpoint_readback_rehash.window.other',
          replicationFactor: 2,
          remoteCheckpointObjectRef: 'r2.training_checkpoint.window.other',
          remoteCheckpointStoreRef:
            'r2.openagents_autopilot_artifacts.training',
          retrievalVerified: true,
          sizeBytes: 1_048_576,
          storageClass: 'content_addressed_object_store',
          windowRef: 'training.window.other',
        },
      }),
      /must match the sealed windowRef/,
    )
  })

  it('carries maxAllowedStale on the run record with the stated default', () => {
    const defaulted = buildTrainingRunRecord({
      makeId: () => 'run',
      nowIso: '2026-06-12T10:00:00.000Z',
      request: {
        promiseRef: 'pylon.first_real_model_training_run.v1',
        trainingRunRef: 'training.run.0001',
      },
    })
    const explicit = buildTrainingRunRecord({
      makeId: () => 'run',
      nowIso: '2026-06-12T10:00:00.000Z',
      request: {
        maxAllowedStale: 8,
        promiseRef: 'pylon.first_real_model_training_run.v1',
        trainingRunRef: 'training.run.0002',
      },
    })

    expect(DefaultMaxAllowedStaleSteps).toBe(5)
    expect(defaulted.maxAllowedStale).toBe(5)
    expect(explicit.maxAllowedStale).toBe(8)
    expect(
      publicTrainingRunProjection(defaulted, '2026-06-12T10:05:00.000Z')
        .maxAllowedStale,
    ).toBe(5)
    expect(
      JSON.parse(explicit.publicProjectionJson).maxAllowedStale,
    ).toBe(8)
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
    const replayChallenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'exact-replay',
      nowIso: '2026-06-10T10:05:10.000Z',
      request: {
        commitmentRefs: ['commitment.cs336.a1.replay.window.1'],
        contributionRef: 'contribution.cs336.a1.replay',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          pylonDeviceRef: 'pylon.device.worker.cs336.a1',
          replayDigestRef: 'digest.cs336.a1.replay',
          traceCommitmentDigestRef: 'digest.cs336.a1.commitment',
          validatorDeviceRef: 'pylon.device.validator.cs336.a1',
        },
        trainingRunRef: run.trainingRunRef,
        verificationClass: 'exact_trace_replay',
        windowRef: window.windowRef,
      },
    }).challenge
    const leasedReplayChallenge = leaseTrainingVerificationChallengeRecord({
      challenge: replayChallenge,
      eventId: 'exact-replay-lease',
      nowIso: '2026-06-10T10:05:20.000Z',
      request: { validatorRef: 'validator.cs336.a1' },
    }).challenge
    const verifiedReplayChallenge = finalizeTrainingVerificationChallengeRecord(
      {
        challenge: leasedReplayChallenge,
        eventId: 'exact-replay-final',
        nowIso: '2026-06-10T10:05:30.000Z',
        request: { receiptRefs: ['receipt.cs336.a1.replay'] },
        verdict: {
          failureCodes: [],
          state: 'Verified',
          verdictRefs: ['verdict.cs336.a1.replay'],
        },
      },
    ).challenge
    const summary = publicTrainingRunSummary({
      challenges: [verifiedChallenge, verifiedReplayChallenge],
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
    expect(summary.realGradient.verifiedReplayPairs).toEqual([
      {
        challengeRef: verifiedReplayChallenge.challengeRef,
        provenanceLabel:
          'Verified exact_trace_replay pair. The worker side is the public worker/device ref and the validator side is the public validator ref recorded on the challenge payload or lease/finalization path.',
        sourceRefs: [
          'contribution.cs336.a1.replay',
          'pylon.device.validator.cs336.a1',
          'pylon.device.worker.cs336.a1',
          verifiedReplayChallenge.challengeRef,
          'verdict.cs336.a1.replay',
        ].sort(),
        validatorRef: 'pylon.device.validator.cs336.a1',
        verdictRefs: ['verdict.cs336.a1.replay'],
        workerRef: 'pylon.device.worker.cs336.a1',
      },
    ])
  })

  it('reconciles run-level settlement state from the same settled-receipt source (#5316)', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'recon',
      nowIso: '2026-06-16T10:00:00.000Z',
      request: {
        promiseRef: 'pylon.first_real_model_training_run.v1',
        receiptRefs: ['receipt.settlement.recon.1'],
        trainingRunRef: 'training.run.recon',
      },
    })
    // Static owner launch-gate seed (migration 0185 shape): pending.
    const run = {
      ...runBase,
      manifest: { settlementState: 'pending' },
    }

    const idleSummary = publicTrainingRunSummary({
      challenges: [],
      leases: [],
      nowIso: '2026-06-16T10:05:00.000Z',
      run,
      windows: [],
    })

    expect(idleSummary.settlement.reconciledState).toBe('none')
    expect(idleSummary.settlement.settledPayoutSats).toBe(0)
    expect(idleSummary.settlement.settledReceiptCount).toBe(0)
    expect(idleSummary.settlement.launchManifestSettlementState).toBe('pending')

    const settledSummary = publicTrainingRunSummary({
      challenges: [],
      leases: [],
      nowIso: '2026-06-16T10:05:00.000Z',
      run,
      settledSatsByReceiptRef: new Map([['receipt.settlement.recon.1', 1010]]),
      windows: [],
    })

    expect(settledSummary.settlement.reconciledState).toBe('settling')
    expect(settledSummary.settlement.settledPayoutSats).toBe(1010)
    expect(settledSummary.settlement.settledReceiptCount).toBe(1)
    // The live reconciled status diverges from the stale static manifest seed.
    expect(settledSummary.settlement.launchManifestSettlementState).toBe(
      'pending',
    )
    expect(settledSummary.settlement.launchManifestSettlementState).not.toBe(
      settledSummary.settlement.reconciledState,
    )
    expect(settledSummary.metrics.providerConfirmedSettledPayoutSats.value).toBe(
      1010,
    )
  })

  it('emits a manifest settlement-state note only when the manifest carries a static settlementState', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'note',
      nowIso: '2026-06-16T10:00:00.000Z',
      request: {
        promiseRef: 'pylon.first_real_model_training_run.v1',
        trainingRunRef: 'training.run.note',
      },
    })
    const withManifest = {
      ...runBase,
      manifest: { settlementState: 'pending' },
    }
    const withoutManifest = { ...runBase, manifest: null }

    const noted = publicTrainingRunProjection(
      withManifest,
      '2026-06-16T10:05:00.000Z',
    )
    const unnoted = publicTrainingRunProjection(
      withoutManifest,
      '2026-06-16T10:05:00.000Z',
    )

    expect(noted.manifestSettlementStateNote).not.toBeNull()
    expect(noted.manifestSettlementStateNote).toContain('migration 0185')
    expect(unnoted.manifestSettlementStateNote).toBeNull()
  })
})
