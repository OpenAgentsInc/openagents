import { describe, expect, it } from 'vitest'

import {
  computeCs336A1TokenizerShard,
  computeCs336A1TrainingStepMatrix,
} from './cs336-a1-homework-workload'
import {
  type TrainingVerificationChallengeRecord,
  verifyDeterministicRecompute,
  verifyFreivaldsMerkle,
} from './training-verification'

const challenge = (
  verificationClass: 'deterministic_recompute' | 'freivalds_merkle',
): TrainingVerificationChallengeRecord => ({
  challengeRef: `training.verification.challenge.workload-${verificationClass}`,
  commitmentRefs: [],
  contributionRef: `contribution.cs336_a1.workload.${verificationClass}`,
  createdAt: '2026-06-11T00:00:00.000Z',
  failureCodes: [],
  homeworkKind: 'admin_dispatched_homework',
  id: `training_verification_challenge_workload_${verificationClass}`,
  leaseExpiresAt: null,
  leaseRef: null,
  leasedToRef: null,
  maxAttempts: 1,
  payloadJson: '{}',
  publicProjectionJson: '{}',
  rejectedAt: null,
  samplingPolicy: 'per_contribution',
  state: 'Leased',
  timedOutAt: null,
  trainingRunRef: 'run.cs336.a1.demo',
  updatedAt: '2026-06-11T00:00:00.000Z',
  verdictRefs: [],
  verificationClass,
  verifiedAt: null,
  windowRef: 'training.window.cs336_a1.demo',
})

describe('cs336 a1 homework workload', () => {
  it('produces a tokenizer shard that passes deterministic recompute', async () => {
    const shard = await computeCs336A1TokenizerShard()
    const recomputed = await computeCs336A1TokenizerShard()

    expect(shard.mergeCount).toBeGreaterThan(0)
    expect(shard.tokenCount).toBeGreaterThan(0)
    expect(recomputed.digestHex).toBe(shard.digestHex)

    const verdict = verifyDeterministicRecompute({
      challenge: challenge('deterministic_recompute'),
      payload: {
        expectedDigestRef: `digest.sha256.${shard.digestHex}`,
        recomputedDigestRef: `digest.sha256.${recomputed.digestHex}`,
      },
    })

    expect(verdict.state).toBe('Verified')
    expect(verdict.failureCodes).toEqual([])
  })

  it('produces a training-step matrix that passes freivalds merkle', async () => {
    const shard = await computeCs336A1TokenizerShard()
    const step = await computeCs336A1TrainingStepMatrix(shard.digestHex)

    expect(step.merkleProofValid).toBe(true)
    expect(step.rowDigestsHex).toHaveLength(step.claimedProductMatrix.length)

    const verdict = verifyFreivaldsMerkle({
      challenge: challenge('freivalds_merkle'),
      payload: {
        challengeVector: step.challengeVector,
        claimedProductMatrix: step.claimedProductMatrix,
        expectExactProduct: true,
        fieldModulus: step.fieldModulus,
        leftMatrix: step.leftMatrix,
        merkleProofValid: step.merkleProofValid,
        rightMatrix: step.rightMatrix,
        rowOpenings: step.rowDigestsHex.map((digestHex, row) => ({
          rowCommitmentRef: `commitment.cs336_a1.row_${row}.sha256_${digestHex.slice(0, 16)}`,
        })),
      },
    })

    expect(verdict.state).toBe('Verified')
    expect(verdict.failureCodes).toEqual([])
  })

  it('rejects a tampered training-step product', async () => {
    const shard = await computeCs336A1TokenizerShard()
    const step = await computeCs336A1TrainingStepMatrix(shard.digestHex)
    const tampered = step.claimedProductMatrix.map((row, rowIndex) =>
      rowIndex === 0
        ? row.map((cell, column) => (column === 0 ? cell + 1 : cell))
        : row,
    )

    const verdict = verifyFreivaldsMerkle({
      challenge: challenge('freivalds_merkle'),
      payload: {
        challengeVector: step.challengeVector,
        claimedProductMatrix: tampered,
        expectExactProduct: true,
        fieldModulus: step.fieldModulus,
        leftMatrix: step.leftMatrix,
        merkleProofValid: step.merkleProofValid,
        rightMatrix: step.rightMatrix,
        rowOpenings: step.rowDigestsHex.map((digestHex, row) => ({
          rowCommitmentRef: `commitment.cs336_a1.row_${row}.sha256_${digestHex.slice(0, 16)}`,
        })),
      },
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('FreivaldsMismatch')
  })
})
