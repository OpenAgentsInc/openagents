import { describe, expect, it } from 'vitest'
import { Schema as S } from 'effect'

import {
  type PostTrainingVibeTestGradingChallengeSpec,
  PostTrainingVibeTestGradingChallengeError,
  PostTrainingVibeTestGradingHomeworkKind,
  buildPostTrainingVibeTestGradingChallengeCreateRequest,
  buildPostTrainingVibeTestGradingChallengeSpec,
  verifyPostTrainingVibeTestGradingResponse,
} from './post-training-vibe-test-grading-challenge'
import {
  PostTrainingVibeTestCloseoutRef,
  PostTrainingVibeTestRubricRef,
  runPostTrainingVibeTestCloseout,
} from './post-training-vibe-test-rubric'
import { TrainingVerificationChallengeCreateRequest } from './training-verification'

const buildSpec = (): Promise<PostTrainingVibeTestGradingChallengeSpec> =>
  buildPostTrainingVibeTestGradingChallengeSpec()

describe('post-training vibe-test grading challenge spec', () => {
  it('builds a deterministic_recompute answer-key spec from the closeout', async () => {
    const spec = await buildSpec()
    const closeout = await runPostTrainingVibeTestCloseout()

    expect(spec.verificationClass).toBe('deterministic_recompute')
    expect(spec.workloadRef).toBe(
      'workload.training_post_training.vibe_test_closeout_grading.v1',
    )
    expect(spec.rubricRef).toBe(PostTrainingVibeTestRubricRef)
    expect(spec.artifactRef).toBe(PostTrainingVibeTestCloseoutRef)
    expect(spec.challengeRef).toBe(
      'challenge.post_training_vibe_test_grading.fixture_closeout',
    )
    expect(spec.expectedDigestHex).toMatch(/^[0-9a-f]{64}$/)
    expect(spec.expectedDigestHex).toBe(closeout.closeoutDigestHex)
    expect(spec.transcriptCount).toBe(closeout.summary.transcriptCount)
    expect(spec.closeoutAcceptable).toBe(closeout.closeoutAcceptable)
    expect(spec.reviewerSigned).toBe(false)
  })

  it('produces the same spec on re-build (deterministic)', async () => {
    const first = await buildSpec()
    const second = await buildSpec()

    expect(second).toEqual(first)
  })

  it('rejects an out-of-range threshold', async () => {
    await expect(
      buildPostTrainingVibeTestGradingChallengeSpec({ threshold: 0 }),
    ).rejects.toBeInstanceOf(PostTrainingVibeTestGradingChallengeError)
    await expect(
      buildPostTrainingVibeTestGradingChallengeSpec({ threshold: 1.5 }),
    ).rejects.toBeInstanceOf(PostTrainingVibeTestGradingChallengeError)
  })

  it('exposes no prompts, completions, or transcripts in the public spec', async () => {
    const spec = await buildSpec()

    expect(JSON.stringify(spec)).not.toMatch(
      /prompt|completion|transcript\.|weight|reasoning/i,
    )
  })
})

describe('post-training vibe-test grading challenge verifier', () => {
  it('Verifies a claim that reproduces the recomputed digest', async () => {
    const spec = await buildSpec()
    const verdict = await verifyPostTrainingVibeTestGradingResponse({
      claim: {
        claimedDigestHex: spec.expectedDigestHex,
        transcriptCount: spec.transcriptCount,
      },
      spec,
    })

    expect(verdict.state).toBe('Verified')
    expect(verdict.failureCodes).toEqual([])
    expect(verdict.verdictRefs).toContain(spec.challengeRef)
  })

  it('accepts an upper-cased claimed digest (case-insensitive)', async () => {
    const spec = await buildSpec()
    const verdict = await verifyPostTrainingVibeTestGradingResponse({
      claim: { claimedDigestHex: spec.expectedDigestHex.toUpperCase() },
      spec,
    })

    expect(verdict.state).toBe('Verified')
  })

  it('Rejects a tampered claimed digest with DigestMismatch', async () => {
    const spec = await buildSpec()
    const verdict = await verifyPostTrainingVibeTestGradingResponse({
      claim: { claimedDigestHex: `${'0'.repeat(63)}1` },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DigestMismatch')
  })

  it('Rejects a malformed (non-hex) claimed digest with OutputDigestMissing', async () => {
    const spec = await buildSpec()
    const verdict = await verifyPostTrainingVibeTestGradingResponse({
      claim: { claimedDigestHex: 'not-a-digest' },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('OutputDigestMissing')
  })

  it('Rejects a claimed transcript count that disagrees with the recompute', async () => {
    const spec = await buildSpec()
    const verdict = await verifyPostTrainingVibeTestGradingResponse({
      claim: {
        claimedDigestHex: spec.expectedDigestHex,
        transcriptCount: spec.transcriptCount + 1,
      },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DimensionMismatch')
  })

  it('Rejects a stale spec whose stored digest no longer recomputes', async () => {
    const spec = await buildSpec()
    const stale: PostTrainingVibeTestGradingChallengeSpec = {
      ...spec,
      expectedDigestHex: `${'a'.repeat(63)}b`,
    }
    const verdict = await verifyPostTrainingVibeTestGradingResponse({
      claim: { claimedDigestHex: stale.expectedDigestHex },
      spec: stale,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DigestMismatch')
  })

  it('Rejects a spec with a malformed expected digest as VerificationClassUnknown', async () => {
    const spec = await buildSpec()
    const malformed: PostTrainingVibeTestGradingChallengeSpec = {
      ...spec,
      expectedDigestHex: 'xyz',
    }
    const verdict = await verifyPostTrainingVibeTestGradingResponse({
      claim: { claimedDigestHex: spec.expectedDigestHex },
      spec: malformed,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('VerificationClassUnknown')
  })
})

describe('post-training vibe-test grading challenge create-request', () => {
  const decode = S.decodeUnknownSync(TrainingVerificationChallengeCreateRequest)

  it('builds a schema-valid deterministic_recompute challenge request', async () => {
    const spec = await buildSpec()
    const request = buildPostTrainingVibeTestGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.post_training.vibe_test.demo',
      windowRef: 'window.post_training.vibe_test.demo.1',
    })

    expect(request.verificationClass).toBe('deterministic_recompute')
    expect(request.homeworkKind).toBe(PostTrainingVibeTestGradingHomeworkKind)
    expect(request.samplingPolicy).toBe('per_contribution')
    expect(request.trainingRunRef).toBe('run.post_training.vibe_test.demo')
    expect(request.windowRef).toBe('window.post_training.vibe_test.demo.1')
    expect(request.payload.expectedDigestHex).toBe(spec.expectedDigestHex)
    expect(request.payload.transcriptCount).toBe(spec.transcriptCount)
    expect(request.payload.jobKind).toBe('post_training_vibe_test_grading')
    expect(request.payload.reviewerSigned).toBe(false)
    // Round-trips through the real training-verification schema unchanged.
    expect(decode(request)).toEqual(request)
  })

  it('omits windowRef when none is supplied', async () => {
    const spec = await buildSpec()
    const request = buildPostTrainingVibeTestGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.post_training.vibe_test.demo',
    })

    expect(request.windowRef).toBeUndefined()
    expect(decode(request)).toEqual(request)
  })

  it('exposes no prompts, completions, or transcripts in the request payload', async () => {
    const spec = await buildSpec()
    const request = buildPostTrainingVibeTestGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.post_training.vibe_test.demo',
    })

    expect(JSON.stringify(request)).not.toMatch(
      /prompt|completion|transcript\.|weight|reasoning/i,
    )
  })

  it('rejects a non-public-safe trainingRunRef via schema validation', async () => {
    const spec = await buildSpec()

    expect(() =>
      buildPostTrainingVibeTestGradingChallengeCreateRequest({
        spec,
        trainingRunRef: 'run with spaces!',
      }),
    ).toThrow(PostTrainingVibeTestGradingChallengeError)
  })

  it('rejects building a request from a spec with a malformed expected digest', async () => {
    const spec = await buildSpec()
    const malformed: PostTrainingVibeTestGradingChallengeSpec = {
      ...spec,
      expectedDigestHex: 'xyz',
    }

    expect(() =>
      buildPostTrainingVibeTestGradingChallengeCreateRequest({
        spec: malformed,
        trainingRunRef: 'run.post_training.vibe_test.demo',
      }),
    ).toThrow(PostTrainingVibeTestGradingChallengeError)
  })
})
