import { describe, expect, it } from 'vitest'

import {
  type Cs336A5DpoGradingChallengeSpec,
  Cs336A5DpoGradingChallengeError,
  Cs336A5DpoGradingHomeworkKind,
  buildCs336A5DpoGradingChallengeCreateRequest,
  buildCs336A5DpoGradingChallengeSpec,
  verifyCs336A5DpoGradingResponse,
} from './cs336-a5-dpo-grading-challenge'
import { runCs336A5DpoPreferenceGrading } from './cs336-a5-dpo-preference-workload'
import { TrainingVerificationChallengeCreateRequest } from './training-verification'
import { Schema as S } from 'effect'

const buildSpec = (): Promise<Cs336A5DpoGradingChallengeSpec> =>
  buildCs336A5DpoGradingChallengeSpec({ splitRef: 'split_a' })

describe('CS336 A5 DPO grading challenge spec', () => {
  it('builds a deterministic_recompute answer-key spec from the reference grade', async () => {
    const spec = await buildSpec()
    const reference = await runCs336A5DpoPreferenceGrading({ splitRef: 'split_a' })

    expect(spec.verificationClass).toBe('deterministic_recompute')
    expect(spec.workloadRef).toBe(
      'workload.cs336_a5.dpo_preference_pair_reference_grading.v1',
    )
    expect(spec.splitRef).toBe('split_a')
    expect(spec.challengeRef).toBe('challenge.cs336_a5_dpo_grading.split_a')
    expect(spec.expectedDigestHex).toMatch(/^[0-9a-f]{64}$/)
    expect(spec.expectedDigestHex).toBe(reference.outputDigestHex)
    expect(spec.pairCount).toBe(reference.pairCount)
    expect(spec.betaMicro).toBe(Math.round(reference.beta * 1_000_000))
  })

  it('produces the same spec on re-build (deterministic)', async () => {
    const first = await buildSpec()
    const second = await buildSpec()

    expect(second).toEqual(first)
  })

  it('rejects a non-positive beta', async () => {
    await expect(
      buildCs336A5DpoGradingChallengeSpec({ beta: 0, splitRef: 'split_a' }),
    ).rejects.toBeInstanceOf(Cs336A5DpoGradingChallengeError)
  })

  it('exposes no prompts, completions, or log-probs in the public spec', async () => {
    const spec = await buildSpec()

    expect(JSON.stringify(spec)).not.toMatch(
      /prompt|completion|logp|weight|reasoning/i,
    )
  })
})

describe('CS336 A5 DPO grading challenge verifier', () => {
  it('Verifies a claim that reproduces the recomputed digest', async () => {
    const spec = await buildSpec()
    const verdict = await verifyCs336A5DpoGradingResponse({
      claim: { claimedDigestHex: spec.expectedDigestHex, pairCount: spec.pairCount },
      spec,
    })

    expect(verdict.state).toBe('Verified')
    expect(verdict.failureCodes).toEqual([])
    expect(verdict.verdictRefs).toContain(spec.challengeRef)
  })

  it('accepts an upper-cased claimed digest (case-insensitive)', async () => {
    const spec = await buildSpec()
    const verdict = await verifyCs336A5DpoGradingResponse({
      claim: { claimedDigestHex: spec.expectedDigestHex.toUpperCase() },
      spec,
    })

    expect(verdict.state).toBe('Verified')
  })

  it('Rejects a tampered claimed digest with DigestMismatch', async () => {
    const spec = await buildSpec()
    const tampered = `${'0'.repeat(63)}1`
    const verdict = await verifyCs336A5DpoGradingResponse({
      claim: { claimedDigestHex: tampered },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DigestMismatch')
  })

  it('Rejects a malformed (non-hex) claimed digest with OutputDigestMissing', async () => {
    const spec = await buildSpec()
    const verdict = await verifyCs336A5DpoGradingResponse({
      claim: { claimedDigestHex: 'not-a-digest' },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('OutputDigestMissing')
  })

  it('Rejects a claimed pair count that disagrees with the recompute', async () => {
    const spec = await buildSpec()
    const verdict = await verifyCs336A5DpoGradingResponse({
      claim: {
        claimedDigestHex: spec.expectedDigestHex,
        pairCount: spec.pairCount + 1,
      },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DimensionMismatch')
  })

  it('Rejects a stale spec whose stored digest no longer recomputes', async () => {
    const spec = await buildSpec()
    const stale: Cs336A5DpoGradingChallengeSpec = {
      ...spec,
      expectedDigestHex: `${'a'.repeat(63)}b`,
    }
    const verdict = await verifyCs336A5DpoGradingResponse({
      claim: { claimedDigestHex: stale.expectedDigestHex },
      spec: stale,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DigestMismatch')
  })

  it('Rejects a spec with a malformed expected digest as VerificationClassUnknown', async () => {
    const spec = await buildSpec()
    const malformed: Cs336A5DpoGradingChallengeSpec = {
      ...spec,
      expectedDigestHex: 'xyz',
    }
    const verdict = await verifyCs336A5DpoGradingResponse({
      claim: { claimedDigestHex: spec.expectedDigestHex },
      spec: malformed,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('VerificationClassUnknown')
  })
})

describe('CS336 A5 DPO grading challenge create-request', () => {
  const decode = S.decodeUnknownSync(TrainingVerificationChallengeCreateRequest)

  it('builds a schema-valid deterministic_recompute challenge request from the spec', async () => {
    const spec = await buildSpec()
    const request = buildCs336A5DpoGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.cs336.a5.dpo.demo',
      windowRef: 'window.cs336.a5.dpo.demo.1',
    })

    expect(request.verificationClass).toBe('deterministic_recompute')
    expect(request.homeworkKind).toBe(Cs336A5DpoGradingHomeworkKind)
    expect(request.samplingPolicy).toBe('per_contribution')
    expect(request.trainingRunRef).toBe('run.cs336.a5.dpo.demo')
    expect(request.windowRef).toBe('window.cs336.a5.dpo.demo.1')
    expect(request.payload.expectedDigestHex).toBe(spec.expectedDigestHex)
    expect(request.payload.pairCount).toBe(spec.pairCount)
    expect(request.payload.jobKind).toBe('cs336_a5_dpo_grading')
    // Round-trips through the real training-verification schema unchanged.
    expect(decode(request)).toEqual(request)
  })

  it('omits windowRef when none is supplied', async () => {
    const spec = await buildSpec()
    const request = buildCs336A5DpoGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.cs336.a5.dpo.demo',
    })

    expect(request.windowRef).toBeUndefined()
    expect(decode(request)).toEqual(request)
  })

  it('exposes no prompts, completions, or log-probs in the request payload', async () => {
    const spec = await buildSpec()
    const request = buildCs336A5DpoGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.cs336.a5.dpo.demo',
    })

    expect(JSON.stringify(request)).not.toMatch(
      /prompt|completion|logp|weight|reasoning/i,
    )
  })

  it('rejects a non-public-safe trainingRunRef via schema validation', async () => {
    const spec = await buildSpec()

    expect(() =>
      buildCs336A5DpoGradingChallengeCreateRequest({
        spec,
        trainingRunRef: 'run with spaces!',
      }),
    ).toThrow(Cs336A5DpoGradingChallengeError)
  })

  it('rejects building a request from a spec with a malformed expected digest', async () => {
    const spec = await buildSpec()
    const malformed: Cs336A5DpoGradingChallengeSpec = {
      ...spec,
      expectedDigestHex: 'xyz',
    }

    expect(() =>
      buildCs336A5DpoGradingChallengeCreateRequest({
        spec: malformed,
        trainingRunRef: 'run.cs336.a5.dpo.demo',
      }),
    ).toThrow(Cs336A5DpoGradingChallengeError)
  })
})
