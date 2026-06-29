import { describe, expect, it } from 'vitest'

import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
  retryTrainingVerificationChallengeRecord,
  runTrainingVerificationClass,
  timeOutTrainingVerificationChallengeRecord,
} from './training-verification'

const buildChallenge = (
  verificationClass:
    | 'deterministic_recompute'
    | 'exact_trace_replay'
    | 'freivalds_merkle'
    | 'seeded_replication'
    | 'statistical_cross_check',
  payload: Record<string, unknown>,
  extra: Partial<Parameters<typeof buildTrainingVerificationChallengeRecord>[0]['request']> = {},
) =>
  buildTrainingVerificationChallengeRecord({
    makeId: () => verificationClass,
    nowIso: '2026-06-10T10:00:00.000Z',
    request: {
      homeworkKind: 'admin_dispatched_homework',
      payload,
      trainingRunRef: 'training.run.4674',
      verificationClass,
      ...extra,
    },
  }).challenge

describe('training verification registry', () => {
  it('verifies ported Freivalds/Merkle fixtures and rejects mismatches with typed failures', async () => {
    const verified = await runTrainingVerificationClass({
      challenge: buildChallenge('freivalds_merkle', {
        challengeVector: [5, 11],
        claimedProductMatrix: [
          [19, 22],
          [43, 50],
        ],
        contributionRefs: ['contribution.training.1'],
        expectExactProduct: true,
        leftMatrix: [
          [1, 2],
          [3, 4],
        ],
        merkleProofValid: true,
        rightMatrix: [
          [5, 6],
          [7, 8],
        ],
        rowOpenings: [{ rowCommitmentRef: 'commitment.row.0' }],
      }),
    })
    expect(verified).toMatchObject({
      failureCodes: [],
      state: 'Verified',
    })

    const rejected = await runTrainingVerificationClass({
      challenge: buildChallenge('freivalds_merkle', {
        challengeVector: [5, 11],
        claimedProductMatrix: [
          [19, 23],
          [43, 50],
        ],
        contributionRefs: ['contribution.training.1'],
        expectExactProduct: true,
        leftMatrix: [
          [1, 2],
          [3, 4],
        ],
        merkleProofValid: true,
        rightMatrix: [
          [5, 6],
          [7, 8],
        ],
        rowOpenings: [{ rowCommitmentRef: 'commitment.row.0' }],
      }),
    })
    expect(rejected).toMatchObject({
      failureCodes: ['FreivaldsMismatch'],
      state: 'Rejected',
    })
  })

  it('verifies deterministic recompute and exact trace replay as registry entries', async () => {
    await expect(
      runTrainingVerificationClass({
        challenge: buildChallenge('deterministic_recompute', {
          contributionRefs: ['contribution.training.1'],
          expectedDigestRef: 'digest.output.abc',
          recomputedDigestRef: 'digest.output.abc',
        }),
      }),
    ).resolves.toMatchObject({ state: 'Verified' })

    await expect(
      runTrainingVerificationClass({
        challenge: buildChallenge('exact_trace_replay', {
          contributionRefs: ['contribution.training.1'],
          replayDigestRef: 'digest.trace.abc',
          sampledWindow: { endStep: 20, startStep: 10 },
          traceCommitmentDigestRef: 'digest.trace.abc',
        }),
      }),
    ).resolves.toMatchObject({ state: 'Verified' })

    await expect(
      runTrainingVerificationClass({
        challenge: buildChallenge('exact_trace_replay', {
          contributionRefs: ['contribution.training.1'],
          replayDigestRef: 'digest.trace.wrong',
          sampledWindow: { endStep: 20, startStep: 10 },
          traceCommitmentDigestRef: 'digest.trace.abc',
        }),
      }),
    ).resolves.toMatchObject({
      failureCodes: ['ExecutorTraceMismatch'],
      state: 'Rejected',
    })
  })

  it('verifies exact trace replay when Pylon submits namespaced commitment and replay refs for the same digest', async () => {
    await expect(
      runTrainingVerificationClass({
        challenge: buildChallenge('exact_trace_replay', {
          contributionRefs: ['contribution.tassadar.executor.1'],
          replayDigestRef:
            'trace.tassadar.replay.f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b',
          sampledWindow: { endStep: 80, startStep: 0 },
          sampledWindowRef: 'trace.tassadar.window.0_80',
          traceCommitmentDigestRef:
            'trace.tassadar.commitment.f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b',
        }),
      }),
    ).resolves.toMatchObject({
      failureCodes: [],
      state: 'Verified',
    })

    await expect(
      runTrainingVerificationClass({
        challenge: buildChallenge('exact_trace_replay', {
          contributionRefs: ['contribution.tassadar.executor.2'],
          replayDigestRef:
            'trace.tassadar.replay.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          sampledWindow: { endStep: 80, startStep: 0 },
          sampledWindowRef: 'trace.tassadar.window.0_80',
          traceCommitmentDigestRef:
            'trace.tassadar.commitment.f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b',
        }),
      }),
    ).resolves.toMatchObject({
      failureCodes: ['ExecutorTraceMismatch'],
      state: 'Rejected',
    })
  })

  it('enforces per-kind sampling policy configuration', async () => {
    await expect(
      runTrainingVerificationClass({
        challenge: buildChallenge(
          'deterministic_recompute',
          {
            expectedDigestRef: 'digest.output.abc',
            recomputedDigestRef: 'digest.output.abc',
          },
          { samplingPolicy: 'per_contribution' },
        ),
      }),
    ).resolves.toMatchObject({
      failureCodes: ['SamplePolicyRejected'],
      state: 'Rejected',
    })

    await expect(
      runTrainingVerificationClass({
        challenge: buildChallenge(
          'deterministic_recompute',
          {
            contributionRefs: ['contribution.training.1'],
            expectedDigestRef: 'digest.output.abc',
            recomputedDigestRef: 'digest.output.abc',
          },
          { samplingPolicy: 'per_contribution' },
        ),
      }),
    ).resolves.toMatchObject({ failureCodes: [], state: 'Verified' })
  })
})

describe('training verification challenge queue', () => {
  it('runs Queued -> Leased -> Retrying -> Leased -> Verified', async () => {
    const queued = buildChallenge('deterministic_recompute', {
      contributionRefs: ['contribution.training.1'],
      expectedDigestRef: 'digest.output.abc',
      recomputedDigestRef: 'digest.output.abc',
    })
    expect(queued.state).toBe('Queued')

    const firstLease = leaseTrainingVerificationChallengeRecord({
      challenge: queued,
      eventId: 'lease1',
      nowIso: '2026-06-10T10:01:00.000Z',
      request: { validatorRef: 'validator.training.1' },
    }).challenge
    expect(firstLease.state).toBe('Leased')

    const retrying = retryTrainingVerificationChallengeRecord({
      challenge: firstLease,
      eventId: 'retry1',
      nowIso: '2026-06-10T10:02:00.000Z',
      request: { failureCodes: ['LeaseExpired'] },
    }).challenge
    expect(retrying.state).toBe('Retrying')
    expect(retrying.failureCodes).toEqual(['LeaseExpired'])

    const secondLease = leaseTrainingVerificationChallengeRecord({
      challenge: retrying,
      eventId: 'lease2',
      nowIso: '2026-06-10T10:03:00.000Z',
      request: { validatorRef: 'validator.training.2' },
    }).challenge
    const verdict = await runTrainingVerificationClass({
      challenge: secondLease,
    })
    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: secondLease,
      eventId: 'final',
      nowIso: '2026-06-10T10:04:00.000Z',
      request: { receiptRefs: ['receipt.training.verified'] },
      verdict,
    }).challenge

    expect(verified.state).toBe('Verified')
    expect(verified.verdictRefs[0]).toContain('verdict.training')
  })

  it('rejects failed verdicts and times out exhausted retry budgets', async () => {
    const leased = leaseTrainingVerificationChallengeRecord({
      challenge: buildChallenge(
        'deterministic_recompute',
        {
          contributionRefs: ['contribution.training.1'],
          expectedDigestRef: 'digest.output.abc',
          recomputedDigestRef: 'digest.output.wrong',
        },
        { maxAttempts: 1 },
      ),
      eventId: 'lease',
      nowIso: '2026-06-10T10:01:00.000Z',
      request: { validatorRef: 'validator.training.1' },
    }).challenge
    const verdict = await runTrainingVerificationClass({ challenge: leased })
    const rejected = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: 'reject',
      nowIso: '2026-06-10T10:02:00.000Z',
      request: { receiptRefs: ['receipt.training.rejected'] },
      verdict,
    }).challenge

    expect(rejected.state).toBe('Rejected')
    expect(rejected.failureCodes).toEqual(['DigestMismatch'])

    const timedOut = retryTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: 'timeout',
      nowIso: '2026-06-10T10:20:00.000Z',
      request: { failureCodes: ['LeaseExpired'] },
    }).challenge
    expect(timedOut.state).toBe('TimedOut')
    expect(timedOut.failureCodes).toEqual([
      'LeaseExpired',
      'RetryBudgetExhausted',
    ])

    expect(() =>
      timeOutTrainingVerificationChallengeRecord({
        challenge: rejected,
        eventId: 'terminal',
        nowIso: '2026-06-10T10:21:00.000Z',
      }),
    ).toThrow()
  })
})
