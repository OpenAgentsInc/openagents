import { describe, expect, it } from 'vitest'
import { Schema as S } from 'effect'

import {
  type PsionInstructSftGradingChallengeSpec,
  PsionInstructSftCompletedSteps,
  PsionInstructSftGradingChallengeError,
  PsionInstructSftGradingHomeworkKind,
  PsionInstructSftManifestDigest,
  PsionInstructSftReportDigest,
  PsionInstructSftTemplateDigest,
  buildPsionInstructSftGradingChallengeCreateRequest,
  buildPsionInstructSftGradingChallengeSpec,
  verifyPsionInstructSftGradingResponse,
} from './psion-instruct-sft-grading-challenge'
import { projectTrainingPostTrainingInstructSft } from './training-post-training-instruct-sft'
import { TrainingVerificationChallengeCreateRequest } from './training-verification'

describe('Psion instruct-SFT grading challenge spec', () => {
  it('builds a deterministic_recompute answer-key spec from the committed lane report digest', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()

    expect(spec.verificationClass).toBe('deterministic_recompute')
    expect(spec.workloadRef).toBe(
      'workload.psion_instruct_sft.lane_report_grading.v1',
    )
    expect(spec.laneId).toBe('psion_instruct_sft_v1')
    expect(spec.runId).toBe('psion-instruct-sft-smoke-001')
    expect(spec.challengeRef).toBe(
      'challenge.psion_instruct_sft_grading.psion_instruct_sft_v1',
    )
    expect(spec.expectedReportDigest).toMatch(/^sha256:[0-9a-f]{64}$/)
    expect(spec.expectedReportDigest).toBe(PsionInstructSftReportDigest)
    expect(spec.completedSteps).toBe(PsionInstructSftCompletedSteps)
  })

  it('produces the same spec on re-build (deterministic)', () => {
    expect(buildPsionInstructSftGradingChallengeSpec()).toEqual(
      buildPsionInstructSftGradingChallengeSpec(),
    )
  })

  it('exposes no prompts, completions, or weights in the public spec', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()

    expect(JSON.stringify(spec)).not.toMatch(/prompt|completion|logp|weight/i)
  })

  it('stays in sync with the published instruct-SFT lane receipt answer key', () => {
    const projection = projectTrainingPostTrainingInstructSft({
      generatedAt: '2026-06-20T00:00:00.000Z',
    })
    const receipt = projection.receipts[0]

    expect(receipt).toBeDefined()
    if (receipt === undefined) return

    expect(PsionInstructSftReportDigest).toBe(receipt.reportDigest)
    expect(PsionInstructSftTemplateDigest).toBe(receipt.template.templateDigest)
    expect(PsionInstructSftManifestDigest).toBe(receipt.corpus.manifestDigest)
    expect(PsionInstructSftCompletedSteps).toBe(receipt.completedSteps)
  })
})

describe('Psion instruct-SFT grading challenge verifier', () => {
  it('Verifies a claim that reproduces the committed report digest', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const verdict = verifyPsionInstructSftGradingResponse({
      claim: {
        claimedReportDigest: spec.expectedReportDigest,
        completedSteps: spec.completedSteps,
      },
      spec,
    })

    expect(verdict.state).toBe('Verified')
    expect(verdict.failureCodes).toEqual([])
    expect(verdict.verdictRefs).toContain(spec.challengeRef)
  })

  it('accepts an upper-cased claimed digest (case-insensitive)', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const verdict = verifyPsionInstructSftGradingResponse({
      claim: { claimedReportDigest: spec.expectedReportDigest.toUpperCase() },
      spec,
    })

    expect(verdict.state).toBe('Verified')
  })

  it('Rejects a tampered claimed digest with DigestMismatch', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const verdict = verifyPsionInstructSftGradingResponse({
      claim: { claimedReportDigest: `sha256:${'0'.repeat(63)}1` },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DigestMismatch')
  })

  it('Rejects a malformed (non-sha256) claimed digest with OutputDigestMissing', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const verdict = verifyPsionInstructSftGradingResponse({
      claim: { claimedReportDigest: 'not-a-digest' },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('OutputDigestMissing')
  })

  it('Rejects a claimed completed-step count that disagrees with the answer key', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const verdict = verifyPsionInstructSftGradingResponse({
      claim: {
        claimedReportDigest: spec.expectedReportDigest,
        completedSteps: spec.completedSteps + 1,
      },
      spec,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('DimensionMismatch')
  })

  it('Rejects a spec with a malformed expected digest as VerificationClassUnknown', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const malformed: PsionInstructSftGradingChallengeSpec = {
      ...spec,
      expectedReportDigest: 'sha256:xyz',
    }
    const verdict = verifyPsionInstructSftGradingResponse({
      claim: { claimedReportDigest: spec.expectedReportDigest },
      spec: malformed,
    })

    expect(verdict.state).toBe('Rejected')
    expect(verdict.failureCodes).toContain('VerificationClassUnknown')
  })
})

describe('Psion instruct-SFT grading challenge create-request', () => {
  const decode = S.decodeUnknownSync(TrainingVerificationChallengeCreateRequest)

  it('builds a schema-valid deterministic_recompute challenge request from the spec', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const request = buildPsionInstructSftGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.psion.instruct_sft.demo',
      windowRef: 'window.psion.instruct_sft.demo.1',
    })

    expect(request.verificationClass).toBe('deterministic_recompute')
    expect(request.homeworkKind).toBe(PsionInstructSftGradingHomeworkKind)
    expect(request.samplingPolicy).toBe('per_contribution')
    expect(request.trainingRunRef).toBe('run.psion.instruct_sft.demo')
    expect(request.windowRef).toBe('window.psion.instruct_sft.demo.1')
    expect(request.payload.expectedReportDigest).toBe(spec.expectedReportDigest)
    expect(request.payload.jobKind).toBe('psion_instruct_sft_grading')
    // Round-trips through the real training-verification schema unchanged.
    expect(decode(request)).toEqual(request)
  })

  it('omits windowRef when none is supplied', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const request = buildPsionInstructSftGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.psion.instruct_sft.demo',
    })

    expect(request.windowRef).toBeUndefined()
    expect(decode(request)).toEqual(request)
  })

  it('exposes no prompts, completions, or weights in the request payload', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const request = buildPsionInstructSftGradingChallengeCreateRequest({
      spec,
      trainingRunRef: 'run.psion.instruct_sft.demo',
    })

    expect(JSON.stringify(request)).not.toMatch(/prompt|completion|logp|weight/i)
  })

  it('rejects a non-public-safe trainingRunRef via schema validation', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()

    expect(() =>
      buildPsionInstructSftGradingChallengeCreateRequest({
        spec,
        trainingRunRef: 'run with spaces!',
      }),
    ).toThrow(PsionInstructSftGradingChallengeError)
  })

  it('rejects building a request from a spec with a malformed expected digest', () => {
    const spec = buildPsionInstructSftGradingChallengeSpec()
    const malformed: PsionInstructSftGradingChallengeSpec = {
      ...spec,
      expectedReportDigest: 'sha256:xyz',
    }

    expect(() =>
      buildPsionInstructSftGradingChallengeCreateRequest({
        spec: malformed,
        trainingRunRef: 'run.psion.instruct_sft.demo',
      }),
    ).toThrow(PsionInstructSftGradingChallengeError)
  })
})
